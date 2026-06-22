use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager};

const STORE_FILE: &str = "playtime.json";
/// In-flight sessions, persisted so a Meteor crash/close doesn't lose time.
const ACTIVE_FILE: &str = "active_sessions.json";
/// Snapshot of the library the watcher matches processes against.
const LIBRARY_CACHE: &str = "library_cache.json";
/// Poll interval for the global process watcher.
const POLL_SECS: u64 = 5;
/// How often the watcher reloads the library index from disk.
const INDEX_REFRESH_SECS: u64 = 60;
/// Sessions shorter than this are ignored (a crash, a wrong-process match…).
const MIN_SESSION_SECS: u64 = 30;

/// Substrings of exe names that are never the game itself even when found under
/// the install dir (crash handlers, redistributables, anti-cheat services…).
const EXCLUDE: &[&str] = &[
    "crashhandler",
    "crashpad",
    "crashreport",
    "unitycrashhandler",
    "vcredist",
    "vc_redist",
    "redist",
    "dxsetup",
    "directx",
    "dotnet",
    "setup",
    "installer",
    "uninstall",
    "easanticheat",
    "battleye",
    "be_service",
];

/// One finished play session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub start: u64,
    pub end: u64,
}

/// Accumulated play stats for one game, keyed by `Game.id`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayStat {
    /// Total seconds played (cached sum of `history`).
    pub seconds: u64,
    /// Unix timestamp of the last session end, if ever played.
    pub last_played: Option<u64>,
    /// Full per-session history (newest appended last).
    #[serde(default)]
    pub history: Vec<Session>,
}

/// An in-flight session being tracked right now, flushed to disk for recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveSession {
    id: String,
    start: u64,
    last_seen: u64,
}

/// Minimal view of a library entry, read from the on-disk library cache.
#[derive(Debug, Clone, Deserialize)]
struct IndexEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    install_dir: Option<String>,
    #[serde(default)]
    executable: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn load(app: &AppHandle) -> HashMap<String, PlayStat> {
    data_dir(app)
        .map(|d| d.join(STORE_FILE))
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

/// Play stats for every game that has any (for sorting the whole library).
pub fn all(app: &AppHandle) -> HashMap<String, PlayStat> {
    load(app)
}

/// Persist one finished session for a game.
fn record_session(app: &AppHandle, id: &str, start: u64, end: u64) -> Result<(), String> {
    let seconds = end.saturating_sub(start);
    let mut map = load(app);
    let stat = map.entry(id.to_string()).or_default();
    stat.seconds += seconds;
    stat.last_played = Some(end);
    stat.history.push(Session { start, end });
    let path = data_dir(app)?.join(STORE_FILE);
    let data = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

// --- Active session persistence (crash recovery) ---------------------------

fn active_load(app: &AppHandle) -> Vec<ActiveSession> {
    data_dir(app)
        .map(|d| d.join(ACTIVE_FILE))
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn active_save(app: &AppHandle, sessions: &[ActiveSession]) {
    if let Ok(dir) = data_dir(app) {
        if let Ok(data) = serde_json::to_string(sessions) {
            let _ = fs::write(dir.join(ACTIVE_FILE), data);
        }
    }
}

/// On startup, close any sessions left dangling by a previous crash/force-quit:
/// record them up to their last confirmed-alive timestamp, then clear the file.
/// Call once from the Tauri `setup` hook, before `start`.
pub fn reconcile(app: &AppHandle) {
    let leftovers = active_load(app);
    if leftovers.is_empty() {
        return;
    }
    active_save(app, &[]);
    for s in &leftovers {
        if s.last_seen.saturating_sub(s.start) >= MIN_SESSION_SECS {
            let _ = record_session(app, &s.id, s.start, s.last_seen);
        }
    }
    let _ = app.emit("playtime-updated", "");
}

// --- Global process watcher ------------------------------------------------

/// Library entries to watch, read from the on-disk cache written by
/// `get_library`. Empty until the first scan completes.
fn library_index(app: &AppHandle) -> Vec<IndexEntry> {
    let entries: Vec<IndexEntry> = data_dir(app)
        .map(|d| d.join(LIBRARY_CACHE))
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();
    // Keep only entries we can actually match a process against.
    entries
        .into_iter()
        .filter(|e| {
            e.install_dir.as_deref().is_some_and(|s| !s.trim().is_empty())
                || e.executable.as_deref().is_some_and(|s| !s.trim().is_empty())
        })
        .collect()
}

/// True if one of the running exe paths belongs to this entry.
fn entry_running(paths: &[String], install_dir: Option<&str>, exe: Option<&str>) -> bool {
    let dir = install_dir.map(|s| s.to_lowercase());
    let exe = exe.map(|s| s.to_lowercase());
    paths.iter().any(|path| {
        if let Some(e) = &exe {
            if path == e {
                return true;
            }
        }
        if let Some(d) = &dir {
            if !d.is_empty() && path.starts_with(d.as_str()) {
                let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
                if !EXCLUDE.iter().any(|x| name.contains(x)) {
                    return true;
                }
            }
        }
        false
    })
}

/// Start the global playtime watcher: a background thread that polls every
/// running process and matches them against the **whole library**, so a game is
/// timed no matter how it was launched (Meteor, Steam, a desktop shortcut…).
/// Sessions are accumulated per game id and the frontend is notified on end.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut sys = System::new();
        // id -> (start, last_seen) for sessions currently in progress.
        let mut active: HashMap<String, (u64, u64)> = HashMap::new();
        let mut index = library_index(&app);
        let mut since_index = 0u64;
        // Game id currently shown in Discord Rich Presence (None = nothing).
        let mut presence: Option<String> = None;

        loop {
            std::thread::sleep(Duration::from_secs(POLL_SECS));

            since_index += POLL_SECS;
            if since_index >= INDEX_REFRESH_SECS {
                index = library_index(&app);
                since_index = 0;
            }

            sys.refresh_processes();
            let paths: Vec<String> = sys
                .processes()
                .values()
                .filter_map(|p| p.exe().map(|e| e.to_string_lossy().to_lowercase()))
                .collect();

            let ts = now();
            let mut running: HashSet<String> = HashSet::new();
            for e in &index {
                if entry_running(&paths, e.install_dir.as_deref(), e.executable.as_deref()) {
                    running.insert(e.id.clone());
                    active
                        .entry(e.id.clone())
                        .and_modify(|v| v.1 = ts)
                        .or_insert((ts, ts));
                }
            }

            // Close sessions whose game is no longer running.
            let ended: Vec<String> = active
                .keys()
                .filter(|id| !running.contains(*id))
                .cloned()
                .collect();
            for id in ended {
                if let Some((start, last)) = active.remove(&id) {
                    if last.saturating_sub(start) >= MIN_SESSION_SECS {
                        let _ = record_session(&app, &id, start, last);
                        let _ = app.emit("playtime-updated", &id);
                    }
                }
            }

            // Discord Rich Presence: show the most recently started running game.
            let primary = running
                .iter()
                .filter_map(|id| active.get(id).map(|(s, _)| (id.clone(), *s)))
                .max_by_key(|(_, s)| *s)
                .map(|(id, _)| id);
            if primary != presence {
                match &primary {
                    Some(id) => {
                        let name = index
                            .iter()
                            .find(|e| &e.id == id)
                            .map(|e| e.name.clone())
                            .unwrap_or_default();
                        let start = active.get(id).map(|(s, _)| *s).unwrap_or(ts);
                        // Only commit `presence` once Discord actually accepted it,
                        // so we keep retrying if Discord isn't up yet.
                        if crate::discord::set_playing(&name, start) {
                            presence = primary.clone();
                        }
                    }
                    None => {
                        crate::discord::clear();
                        presence = None;
                    }
                }
            }

            // Flush in-progress sessions for crash recovery.
            let snapshot: Vec<ActiveSession> = active
                .iter()
                .map(|(id, (start, last))| ActiveSession {
                    id: id.clone(),
                    start: *start,
                    last_seen: *last,
                })
                .collect();
            active_save(&app, &snapshot);
        }
    });
}
