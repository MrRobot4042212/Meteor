//! CPU temperature via the LibreHardwareMonitor sidecar (`binaries/cputemp.exe`,
//! built from `sidecar/cputemp/`). LHM reads Ryzen Tctl / Intel core temps through
//! a kernel driver, so this needs **admin** and an HVCI-compatible driver; without
//! them the sidecar prints nothing and CPU temp degrades to `None` — same
//! best-effort contract as the PresentMon (FPS) integration.
//!
//! A controller thread runs the sidecar only while the overlay wants CPU temp and
//! a game is running, parses the one-int-per-line °C stream from its stdout, and
//! keeps the latest value in an atomic for the sampler.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Latest CPU temperature in °C (0 = no data).
static CPU_TEMP_C: AtomicU32 = AtomicU32::new(0);

/// Current CPU temperature, if the sidecar is producing data.
pub fn current() -> Option<u32> {
    match CPU_TEMP_C.load(Ordering::Relaxed) {
        0 => None,
        v => Some(v),
    }
}

fn reset() {
    CPU_TEMP_C.store(0, Ordering::Relaxed);
}

/// Locate the sidecar: bundled resource, next to our exe, or the dev `binaries/`.
fn find_binary(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("binaries/cputemp.exe"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("cputemp.exe"));
        }
    }
    candidates.push(PathBuf::from("binaries/cputemp.exe"));
    candidates.into_iter().find(|p| p.exists())
}

/// Spawn the sidecar with a reader thread parsing its stdout (one °C int per line).
fn spawn(bin: &PathBuf) -> std::io::Result<Child> {
    let mut cmd = Command::new(bin);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn()?;
    // Kill-on-close job: an orphaned elevated sidecar would keep the
    // LibreHardwareMonitor kernel driver loaded after Meteor is gone.
    #[cfg(windows)]
    crate::jobobj::assign(&child);
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if let Ok(v) = line.trim().parse::<u32>() {
                    // Guard against obviously bogus values.
                    if v > 0 && v < 200 {
                        CPU_TEMP_C.store(v, Ordering::Relaxed);
                    }
                }
            }
            // Stream ended (sidecar exited): clear the stale reading.
            reset();
        });
    }
    Ok(child)
}

/// Start the controller thread. Idle until the overlay wants CPU temp and a game
/// is running; tears the sidecar down (unloading its driver) otherwise.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // The sidecar needs admin to load its kernel driver. Elevation cannot
        // change while we run, so check once instead of waking twice a second
        // for a process that could never start (this mirrors what the PresentMon
        // controller already did).
        #[cfg(windows)]
        if !crate::elevation::is_elevated() {
            return;
        }

        let mut child: Option<Child> = None;
        let mut bin_missing_logged = false;
        let mut seen: u64 = 0;

        loop {
            // Park until the overlay config or the running game changes; only
            // poll periodically while the sidecar is actually up (to reap it).
            let want_now = crate::metrics::want_cpu_temp() && crate::metrics::has_game();
            crate::metrics::wait_sidecar(
                &mut seen,
                (want_now || child.is_some()).then(|| Duration::from_millis(500)),
            );

            let want = crate::metrics::want_cpu_temp() && crate::metrics::has_game();

            if want && child.is_none() {
                match find_binary(&app) {
                    Some(bin) => match spawn(&bin) {
                        Ok(c) => child = Some(c),
                        Err(e) => eprintln!("cputemp no pudo iniciarse: {e}"),
                    },
                    None => {
                        if !bin_missing_logged {
                            eprintln!("cputemp.exe no encontrado: temp. de CPU deshabilitada.");
                            bin_missing_logged = true;
                        }
                    }
                }
            } else if !want {
                if let Some(mut c) = child.take() {
                    let _ = c.kill();
                }
                reset();
            }

            // Reap a sidecar that exited on its own (driver blocked, no admin, …).
            if let Some(c) = &mut child {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    child = None;
                    reset();
                }
            }
        }
    });
}
