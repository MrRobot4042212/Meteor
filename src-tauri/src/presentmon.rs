//! FPS / frametime via **PresentMon** (Intel/Microsoft, ETW-based — no DLL
//! injection, so anti-cheat safe). A controller thread spawns `PresentMon.exe`
//! targeting the running game's PID, streams its CSV from stdout, and keeps a
//! ~1s rolling window of frame times to derive FPS and average frametime.
//!
//! Requirements (both needed for FPS to appear; everything degrades silently to
//! `None` otherwise, so the rest of the overlay always works):
//!   1. The `PresentMon.exe` binary present (see `binaries/README.md`).
//!   2. Meteor running **elevated** — ETW realtime sessions require admin.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Latest FPS / frametime as hundredths (0 = no data), so they fit in atomics.
static FPS_X100: AtomicU32 = AtomicU32::new(0);
static FRAMETIME_X100: AtomicU32 = AtomicU32::new(0);
/// PID of the spawned PresentMon.exe process, to clean it up on exit.
static CHILD_PID: AtomicU32 = AtomicU32::new(0);

/// Current FPS and average frametime (ms), if PresentMon is producing data.
pub fn current() -> (Option<f32>, Option<f32>) {
    let f = FPS_X100.load(Ordering::Relaxed);
    let ft = FRAMETIME_X100.load(Ordering::Relaxed);
    let opt = |v: u32| if v == 0 { None } else { Some(v as f32 / 100.0) };
    (opt(f), opt(ft))
}

fn reset() {
    FPS_X100.store(0, Ordering::Relaxed);
    FRAMETIME_X100.store(0, Ordering::Relaxed);
}

/// Locate the PresentMon binary: bundled resource, next to our exe, or the dev
/// `binaries/` folder (cwd is `src-tauri` under `tauri dev`).
fn find_binary(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("binaries/PresentMon.exe"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("PresentMon.exe"));
        }
    }
    candidates.push(PathBuf::from("binaries/PresentMon.exe"));
    candidates.into_iter().find(|p| p.exists())
}

/// Spawn PresentMon for a PID, with a reader thread parsing its stdout CSV.
fn spawn(bin: &Path, pid: u32) -> std::io::Result<Child> {
    // PresentMon 2.x uses GNU-style `--` flags. `--v1_metrics` keeps the stable
    // `msBetweenPresents` column (frametime) the parser looks for.
    let mut cmd = Command::new(bin);
    cmd.args([
        "--process_id",
        &pid.to_string(),
        "--output_stdout",
        "--stop_existing_session",
        "--no_console_stats",
        "--terminate_on_proc_exit",
        "--v1_metrics",
    ]);
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
    CHILD_PID.store(child.id(), Ordering::Relaxed);
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || parse_stdout(out));
    }
    Ok(child)
}

/// Cleanup any running PresentMon process spawned by us. Called on app exit.
pub fn cleanup() {
    let pid = CHILD_PID.load(Ordering::Relaxed);
    if pid != 0 {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }
}

/// Read PresentMon's CSV stream and maintain a ~1s rolling window of frame times.
fn parse_stdout(out: impl std::io::Read) {
    let reader = BufReader::new(out);
    // Index of the "...BetweenPresents" column (frametime in ms), found from the header.
    let mut ft_col: Option<usize> = None;
    // Rolling window of recent frametimes (ms) and their running sum.
    let mut window: VecDeque<f32> = VecDeque::new();
    let mut sum = 0.0f32;

    for line in reader.lines() {
        let Ok(line) = line else { break };
        let cols: Vec<&str> = line.split(',').collect();

        // Header: locate the frametime column (name varies across versions:
        // "msBetweenPresents" / "MsBetweenPresents").
        if ft_col.is_none() {
            if let Some(i) = cols
                .iter()
                .position(|c| c.trim().to_ascii_lowercase().contains("betweenpresents"))
            {
                ft_col = Some(i);
            }
            continue;
        }

        let idx = ft_col.unwrap();
        let Some(ft) = cols.get(idx).and_then(|v| v.trim().parse::<f32>().ok()) else {
            continue;
        };
        if !(ft.is_finite() && ft > 0.0) {
            continue;
        }

        window.push_back(ft);
        sum += ft;
        // Keep roughly the last second of frames.
        while sum > 1000.0 && window.len() > 1 {
            if let Some(old) = window.pop_front() {
                sum -= old;
            }
        }

        let n = window.len() as f32;
        let avg_ft = sum / n;
        let fps = if avg_ft > 0.0 { 1000.0 / avg_ft } else { 0.0 };
        FRAMETIME_X100.store((avg_ft * 100.0) as u32, Ordering::Relaxed);
        FPS_X100.store((fps * 100.0) as u32, Ordering::Relaxed);
    }
    // Stream ended (game closed / PresentMon stopped): clear stale numbers.
    reset();
}

/// Start the PresentMon controller thread. Idle until the overlay wants FPS and a
/// game is running; it (re)targets PresentMon at the current game's PID.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // PresentMon's ETW realtime session requires admin. Elevation can't change at
        // runtime, so check once: when not elevated we never even attempt to spawn it
        // (no access-denied spam, no overhead). FPS on NVIDIA therefore only appears
        // when Meteor is already running as admin; AMD gets FPS from ADLX regardless.
        let elevated = {
            #[cfg(windows)]
            {
                crate::elevation::is_elevated()
            }
            #[cfg(not(windows))]
            {
                false
            }
        };
        if !elevated {
            return;
        }
        let mut child: Option<Child> = None;
        let mut child_pid: u32 = 0;
        // Once we fail to find the binary, stop retrying every tick (logged once).
        let mut bin_missing_logged = false;
        // PID we already failed to attach to (no admin → ETW access denied, or no
        // binary). Without this we'd respawn PresentMon.exe every 500ms for the whole
        // session — a real hitch source for users without elevation (esp. NVIDIA,
        // where PresentMon is the only FPS source). Cleared when the target changes.
        let mut failed_pid: u32 = 0;

        loop {
            std::thread::sleep(Duration::from_millis(500));

            let want_pid = if crate::metrics::want_fps() {
                crate::metrics::current_pid()
            } else {
                0
            };

            // A new target clears the previous failure so the new game gets a try.
            if want_pid != failed_pid {
                failed_pid = 0;
            }

            // Target changed (new game / stopped): tear down the old instance. Skip
            // re-attempting a PID we already failed on (failed_pid) to avoid respawning.
            if want_pid != child_pid && want_pid != failed_pid {
                if let Some(mut c) = child.take() {
                    let _ = c.kill();
                }
                reset();
                child_pid = 0;

                if want_pid != 0 {
                    match find_binary(&app) {
                        Some(bin) => match spawn(&bin, want_pid) {
                            Ok(c) => {
                                child = Some(c);
                                child_pid = want_pid;
                            }
                            Err(e) => {
                                // Typically "access denied" without elevation. Mark the
                                // PID failed so we don't hammer respawns every tick.
                                eprintln!("PresentMon no pudo iniciarse: {e}");
                                failed_pid = want_pid;
                            }
                        },
                        None => {
                            // No binary: don't re-scan the filesystem every tick either.
                            failed_pid = want_pid;
                            if !bin_missing_logged {
                                eprintln!(
                                    "PresentMon.exe no encontrado: FPS/frametime deshabilitados."
                                );
                                bin_missing_logged = true;
                            }
                        }
                    }
                }
            }

            // Reap a child that exited on its own (game closed, ETW denied, …).
            if let Some(c) = &mut child {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    let dead = child_pid;
                    child = None;
                    child_pid = 0;
                    reset();
                    // If the game is still running, PresentMon died by itself (e.g. ETW
                    // denied at runtime) — mark the PID failed so we don't respawn every
                    // tick. If the game closed (want_pid changed), this is just cleanup.
                    if dead != 0 && want_pid == dead {
                        failed_pid = dead;
                    }
                }
            }
        }
    });
}
