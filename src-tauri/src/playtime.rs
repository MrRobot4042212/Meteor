use crate::models::Game;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager};

const STORE_FILE: &str = "playtime.json";
/// Wait at most this long for a launched game's process to appear before giving
/// up (store launchers can take a while to start the real game).
const APPEAR_GRACE_SECS: u64 = 120;
/// Poll interval while waiting for start / for the game to exit.
const POLL_SECS: u64 = 5;
/// Sessions shorter than this are ignored (a crash, a wrong-process match…).
const MIN_SESSION_SECS: u64 = 30;

/// Accumulated play stats for one game, keyed by `Game.id`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayStat {
    /// Total seconds played.
    pub seconds: u64,
    /// Unix timestamp of the last session end, if ever played.
    pub last_played: Option<u64>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(STORE_FILE))
}

fn load(app: &AppHandle) -> HashMap<String, PlayStat> {
    store_path(app)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Play stats for a single game id (zeroed if never played).
pub fn get(app: &AppHandle, id: &str) -> PlayStat {
    load(app).get(id).cloned().unwrap_or_default()
}

fn record(app: &AppHandle, id: &str, seconds: u64) -> Result<(), String> {
    let mut map = load(app);
    let stat = map.entry(id.to_string()).or_default();
    stat.seconds += seconds;
    stat.last_played = Some(now());
    let path = store_path(app)?;
    let data = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// True if any running process belongs to the game: its exe path matches the
/// game's executable, or lives under its install directory.
fn is_running(sys: &mut System, install_dir: Option<&str>, exe: Option<&str>) -> bool {
    sys.refresh_processes();
    let dir = install_dir.map(|s| s.to_lowercase());
    let exe = exe.map(|s| s.to_lowercase());
    sys.processes().values().any(|p| {
        let Some(path) = p.exe().map(|e| e.to_string_lossy().to_lowercase()) else {
            return false;
        };
        if let Some(d) = &dir {
            if !d.is_empty() && path.starts_with(d.as_str()) {
                return true;
            }
        }
        if let Some(e) = &exe {
            if path == *e {
                return true;
            }
        }
        false
    })
}

/// Start watching a just-launched game in a background thread: wait for its
/// process to appear, time how long it runs, then persist the session and notify
/// the frontend (`playtime-updated` event). Best-effort and heuristic — store
/// launchers spawn the real game as a separate process, matched by install dir.
pub fn track(app: AppHandle, game: Game) {
    let install_dir = game.install_dir.clone();
    let exe = game.executable.clone();
    if install_dir.is_none() && exe.is_none() {
        return; // nothing to match a process against
    }
    let id = game.id.clone();

    std::thread::spawn(move || {
        let mut sys = System::new();

        // Wait for the game to actually start (store launchers add latency).
        let mut waited = 0;
        loop {
            if is_running(&mut sys, install_dir.as_deref(), exe.as_deref()) {
                break;
            }
            if waited >= APPEAR_GRACE_SECS {
                return; // never showed up; don't record anything
            }
            std::thread::sleep(Duration::from_secs(POLL_SECS));
            waited += POLL_SECS;
        }

        let start = now();

        // Wait until every matching process is gone.
        loop {
            std::thread::sleep(Duration::from_secs(POLL_SECS));
            if !is_running(&mut sys, install_dir.as_deref(), exe.as_deref()) {
                break;
            }
        }

        let elapsed = now().saturating_sub(start);
        if elapsed < MIN_SESSION_SECS {
            return;
        }
        let _ = record(&app, &id, elapsed);
        let _ = app.emit("playtime-updated", &id);
    });
}
