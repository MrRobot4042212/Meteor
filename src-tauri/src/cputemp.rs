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
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Latest CPU temperature in °C (0 = no data).
static CPU_TEMP_C: AtomicU32 = AtomicU32::new(0);

/// The running sidecar, shared with `shutdown()` so a clean app exit can release
/// the kernel driver before the Job Object resorts to terminating the process.
static CHILD: Mutex<Option<Child>> = Mutex::new(None);

/// How long to wait for the sidecar to release its driver and exit before killing
/// it. It wakes on stdin EOF, so the normal case is milliseconds; this is only the
/// ceiling for a sidecar wedged inside the driver.
const GRACEFUL_STOP: Duration = Duration::from_millis(1500);

fn child_lock() -> MutexGuard<'static, Option<Child>> {
    CHILD.lock().unwrap_or_else(PoisonError::into_inner)
}

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
    // stdin is piped and kept open on purpose: closing it is the sidecar's shutdown
    // signal, and the only way it ever unloads its kernel driver (see `stop`).
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn()?;
    // Kill-on-close job: last-resort backstop so an orphaned elevated sidecar cannot
    // outlive Meteor after a crash. Note it terminates rather than stops the child,
    // which does NOT unload the driver — that is what `stop` is for.
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

/// Stop the sidecar so it unloads its kernel driver, killing it only if it refuses.
///
/// Dropping its stdin is the agreed shutdown signal: the sidecar sees EOF, calls
/// `computer.Close()` (which unloads the LibreHardwareMonitor driver) and exits.
/// `kill()` alone is TerminateProcess, which skips .NET finalizers and leaves the
/// driver loaded and registered for the rest of the boot — a documented local
/// privilege-escalation primitive and a kernel-anti-cheat blocklist trigger.
fn stop(mut child: Child) {
    drop(child.stdin.take());

    let deadline = Instant::now() + GRACEFUL_STOP;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            // Can't observe it any more; fall through to the kill.
            Err(_) => break,
        }
        if Instant::now() >= deadline {
            eprintln!("cputemp did not stop within {GRACEFUL_STOP:?}; terminating (its driver may stay loaded)");
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let _ = child.kill();
    let _ = child.wait();
}

/// Stop the sidecar on application exit. Called from `RunEvent::Exit`, where the
/// Job Object would otherwise terminate it and strand the driver.
pub fn shutdown() {
    let child = child_lock().take();
    if let Some(child) = child {
        stop(child);
        reset();
    }
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

        let mut bin_missing_logged = false;
        let mut seen: u64 = 0;

        loop {
            // Park until the overlay config or the running game changes; only
            // poll periodically while the sidecar is actually up (to reap it).
            let running = child_lock().is_some();
            let want_now = crate::metrics::want_cpu_temp() && crate::metrics::has_game();
            crate::metrics::wait_sidecar(
                &mut seen,
                (want_now || running).then(|| Duration::from_millis(500)),
            );

            let want = crate::metrics::want_cpu_temp() && crate::metrics::has_game();

            if want && child_lock().is_none() {
                match find_binary(&app) {
                    Some(bin) => match spawn(&bin) {
                        Ok(c) => *child_lock() = Some(c),
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
                // Take the child out before stopping it: `stop` waits up to
                // GRACEFUL_STOP and must not hold the lock while it does.
                let child = child_lock().take();
                if let Some(child) = child {
                    stop(child);
                }
                reset();
            }

            // Reap a sidecar that exited on its own (driver blocked, no admin, …).
            let exited = matches!(
                child_lock().as_mut().map(|c| c.try_wait()),
                Some(Ok(Some(_)))
            );
            if exited {
                *child_lock() = None;
                reset();
            }
        }
    });
}
