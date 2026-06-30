use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager};

static LAUNCHED_FROM_METEOR: std::sync::Mutex<Vec<(String, u64)>> = std::sync::Mutex::new(Vec::new());

const STORE_FILE: &str = "playtime.json";
/// In-flight sessions, persisted so a Meteor crash/close doesn't lose time.
const ACTIVE_FILE: &str = "active_sessions.json";
/// Snapshot of the library the watcher matches processes against.
const LIBRARY_CACHE: &str = "library_cache.json";
/// Poll interval for the global process watcher.
const POLL_SECS: u64 = 5;
/// While a game is confirmed running, do the expensive full process enumeration only
/// this often; cheap per-PID liveness checks (`proc_alive`) cover the polls in between.
const FULL_SCAN_SECS: u64 = 20;
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
    source: crate::models::GameSource,
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

/// Registra que un juego fue lanzado a través de Meteor, para que sus métricas
/// sean mostradas en el overlay.
pub fn notify_launched(id: &str) {
    if let Ok(mut list) = LAUNCHED_FROM_METEOR.lock() {
        list.retain(|(i, _)| i != id);
        list.push((id.to_string(), now()));
    }
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

/// PID of a running process belonging to this entry (for matching + the metrics
/// overlay / PresentMon). `procs` is the `(pid, lowercased exe path)` list captured
/// once per full scan; a `Some` result doubles as "this entry is running".
fn find_pid(procs: &[(u32, String)], install_dir: Option<&str>, exe: Option<&str>) -> Option<u32> {
    let dir = install_dir.map(|s| s.to_lowercase());
    let exe = exe.map(|s| s.to_lowercase());
    for (pid, path) in procs {
        if let Some(e) = &exe {
            if path == e {
                return Some(*pid);
            }
        }
        if let Some(d) = &dir {
            if !d.is_empty() && path.starts_with(d.as_str()) {
                let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
                if !EXCLUDE.iter().any(|x| name.contains(x)) {
                    return Some(*pid);
                }
            }
        }
    }
    None
}

/// Whether a process with this PID is still alive, via a single cheap Win32 query, so
/// a confirmed-running game can be re-checked between full scans without walking every
/// process on the system.
#[cfg(windows)]
fn proc_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // GetExitCodeProcess reports 259 (STILL_ACTIVE) while the process runs. A process
    // that genuinely exits with 259 is a rare collision the periodic full scan corrects.
    const STILL_ACTIVE: u32 = 259;
    if pid == 0 {
        return false;
    }
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            // Can't open → treat as gone; if it was actually a live, protected process
            // the next full scan re-adds it by path.
            return false;
        };
        if handle.is_invalid() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code).is_ok();
        let _ = CloseHandle(handle);
        // On a query failure, err towards "alive" so we never drop a running session.
        !ok || code == STILL_ACTIVE
    }
}

/// Start the global playtime watcher: a background thread that polls every
/// running process and matches them against the **whole library**, so a game is
/// timed no matter how it was launched (Meteor, Steam, a desktop shortcut…).
/// Sessions are accumulated per game id and the frontend is notified on end.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut sys = System::new();
        // id -> (start, last_seen, pid) for sessions currently in progress.
        let mut active: HashMap<String, (u64, u64, u32)> = HashMap::new();
        let mut index = library_index(&app);
        let mut since_index = 0u64;
        // Seconds since the last full process enumeration. While a game is confirmed
        // running we only do cheap per-PID liveness checks between full scans, so the
        // watcher costs O(active games) instead of O(all processes) during play.
        let mut since_full = 0u64;
        // Game id currently shown in Discord Rich Presence (None = nothing).
        let mut presence: Option<String> = None;
        // Debug: last game name published to the overlay, to log only on change.
        let mut dbg_overlay_game: Option<String> = None;

        loop {
            std::thread::sleep(Duration::from_secs(POLL_SECS));

            since_index += POLL_SECS;
            if since_index >= INDEX_REFRESH_SECS {
                index = library_index(&app);
                since_index = 0;
            }

            let ts = now();

            // Mantenemos en la lista de "lanzados" a los juegos que sigan en progreso
            // o que hayan sido lanzados hace menos de 2 minutos (por si tardan en abrir).
            let mut launched_list = LAUNCHED_FROM_METEOR.lock().unwrap();
            launched_list.retain(|(id, launch_ts)| {
                active.contains_key(id) || ts.saturating_sub(*launch_ts) < 120
            });
            // A Meteor-launched game we haven't matched to a process yet → keep scanning
            // promptly until it shows up (don't wait for the slow full-scan cadence).
            let pending_launch = launched_list
                .iter()
                .any(|(id, _)| !active.contains_key(id));

            // Full enumeration vs. cheap liveness. Full scan when nothing is tracked
            // (idle: cheap, and the only way to catch a game launched outside Meteor), a
            // launch is still pending, or the periodic refresh is due. Off-Windows there
            // is no cheap liveness primitive, so always scan.
            #[cfg(windows)]
            let do_full = active.is_empty() || pending_launch || since_full >= FULL_SCAN_SECS;
            #[cfg(not(windows))]
            let do_full = true;

            let mut running: HashSet<String> = HashSet::new();
            if do_full {
                since_full = 0;
                sys.refresh_processes_specifics(
                    sysinfo::ProcessRefreshKind::new().with_exe(sysinfo::UpdateKind::Always),
                );
                // (pid, lowercased exe path) captured once, reused for matching + pid.
                let procs: Vec<(u32, String)> = sys
                    .processes()
                    .iter()
                    .filter_map(|(pid, p)| {
                        p.exe()
                            .map(|e| (pid.as_u32(), e.to_string_lossy().to_lowercase()))
                    })
                    .collect();

                for e in &index {
                    // OPT-IN: only games that are active or were launched via Meteor.
                    let is_active = active.contains_key(&e.id);
                    let was_launched = launched_list.iter().any(|(l_id, _)| l_id == &e.id);
                    if !is_active && !was_launched {
                        continue;
                    }
                    if let Some(pid) =
                        find_pid(&procs, e.install_dir.as_deref(), e.executable.as_deref())
                    {
                        running.insert(e.id.clone());
                        active
                            .entry(e.id.clone())
                            .and_modify(|v| {
                                v.1 = ts;
                                v.2 = pid;
                            })
                            .or_insert((ts, ts, pid));
                    }
                }
            } else {
                since_full += POLL_SECS;
                // Cheap path: confirm each tracked game's PID is still alive (1 syscall
                // each) instead of enumerating every process on the system.
                #[cfg(windows)]
                for (id, v) in active.iter_mut() {
                    if proc_alive(v.2) {
                        v.1 = ts;
                        running.insert(id.clone());
                    }
                }
            }

            // Close sessions whose game is no longer running.
            let ended: Vec<String> = active
                .keys()
                .filter(|id| !running.contains(*id))
                .cloned()
                .collect();
            for id in ended {
                if let Some((start, last, _pid)) = active.remove(&id) {
                    if last.saturating_sub(start) >= MIN_SESSION_SECS {
                        let _ = record_session(&app, &id, start, last);
                        let _ = app.emit("playtime-updated", &id);
                    }
                }
            }

            // Discord Rich Presence: show the most recently started running game.
            let primary = running
                .iter()
                .filter_map(|id| active.get(id).map(|(s, _, _)| (id.clone(), *s)))
                .filter(|(id, _)| {
                    index
                        .iter()
                        .find(|e| &e.id == id)
                        .map(|e| e.source != crate::models::GameSource::App)
                        .unwrap_or(true)
                })
                .max_by_key(|(_, s)| *s)
                .map(|(id, _)| id);

            // Publish the foreground game (name + pid) to the metrics overlay
            // ONLY if it was launched from Meteor.
            let show_metrics_for = primary
                .as_ref()
                .filter(|id| launched_list.iter().any(|(l_id, _)| l_id == *id));
            let game_name = show_metrics_for
                .and_then(|id| index.iter().find(|e| e.id == **id))
                .map(|e| e.name.clone());
            // PID comes from the active map (resolved at scan time) — no extra walk.
            let game_pid = show_metrics_for.and_then(|id| active.get(id).map(|(_, _, pid)| *pid));
            // Debug: surface why the overlay is/ isn't fed a game (transition-only).
            if game_name != dbg_overlay_game {
                eprintln!(
                    "[overlay] watcher: running_primary={:?} launched_from_meteor={} -> publish={:?} pid={:?}",
                    primary,
                    show_metrics_for.is_some(),
                    game_name,
                    game_pid
                );
                dbg_overlay_game = game_name.clone();
            }
            crate::metrics::set_current_game(game_name, game_pid);

            if primary != presence {
                match &primary {
                    Some(id) => {
                        let name = index
                            .iter()
                            .find(|e| &e.id == id)
                            .map(|e| e.name.clone())
                            .unwrap_or_default();
                        let start = active.get(id).map(|(s, _, _)| *s).unwrap_or(ts);
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
                .map(|(id, (start, last, _pid))| ActiveSession {
                    id: id.clone(),
                    start: *start,
                    last_seen: *last,
                })
                .collect();
            active_save(&app, &snapshot);
        }
    });
}
