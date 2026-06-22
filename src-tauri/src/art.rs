use crate::igdb::{self, GameDetails};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const CACHE_FILE: &str = "cover_cache.json";
const DETAILS_CACHE_FILE: &str = "details_cache.json";
const COVERS_DIR: &str = "covers";
/// A "no cover found" result is only trusted for a while, then re-tried — so a
/// transient network blip (or IGDB creds added later) self-heals.
const NEGATIVE_TTL: Duration = Duration::from_secs(3 * 24 * 60 * 60);

/// One URL-cache entry: the resolved IGDB image URL (empty string = looked up,
/// nothing found) plus the unix timestamp of when it was resolved.
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    url: String,
    ts: u64,
}

/// Resolve a vertical cover for a game by name, using **IGDB** as the only
/// source (needs Twitch creds in settings), and **cache the image file on disk**.
///
/// Three layers, fastest first:
/// 1. Local image file already downloaded → return its path (no API, no network).
/// 2. Cached IGDB URL → just download the image (no API call).
/// 3. Ask IGDB once, cache the URL, then download.
///
/// The returned value is a local file path once the image is cached; if the
/// download fails it falls back to the remote URL so the card still renders.
/// URL hits are cached forever; misses only for `NEGATIVE_TTL`.
pub fn resolve(app: &AppHandle, name: &str) -> Option<String> {
    let key = name.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }

    // 1. Already downloaded? Serve the local file — no network at all.
    let file = cover_file(app, &key).ok()?;
    if file.exists() {
        return Some(file.to_string_lossy().to_string());
    }

    // 2. Known URL (cached) avoids re-hitting the IGDB search API.
    let mut cache = load_cache(app);
    let url = match cache.get(&key) {
        Some(entry) if !entry.url.is_empty() => entry.url.clone(),
        Some(entry) if !is_stale(entry.ts) => return None, // recent miss
        _ => {
            // 3. Ask IGDB once and remember the result (URL or miss).
            let variants = name_variants(name);
            let resolved = igdb::resolve_cover(&variants);
            cache.insert(
                key.clone(),
                Entry {
                    url: resolved.clone().unwrap_or_default(),
                    ts: now(),
                },
            );
            let _ = save_cache(app, &cache);
            resolved?
        }
    };

    // Download to disk; serve the local file, or the remote URL if it failed.
    if download(&url, &file) {
        Some(file.to_string_lossy().to_string())
    } else {
        Some(url)
    }
}

/// One cached IGDB detail lookup: whether a match was found, the data, and when.
#[derive(Clone, Serialize, Deserialize)]
struct DetailEntry {
    found: bool,
    #[serde(default)]
    details: GameDetails,
    ts: u64,
}

/// Resolve rich metadata for a game by name via IGDB, cached on disk. Misses are
/// only trusted for `NEGATIVE_TTL` so they self-heal. Returns `None` if no match.
pub fn details(app: &AppHandle, name: &str) -> Option<GameDetails> {
    let key = name.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }

    let mut cache = load_details_cache(app);
    if let Some(entry) = cache.get(&key) {
        if entry.found {
            return Some(entry.details.clone());
        }
        if !is_stale(entry.ts) {
            return None; // recent miss
        }
    }

    let mut resolved = igdb::fetch_details(&name_variants(name));
    // Translate the English summary to Spanish (cached separately by translate.rs).
    if let Some(d) = resolved.as_mut() {
        if let Some(summary) = d.summary.as_ref().filter(|s| !s.trim().is_empty()) {
            d.summary = Some(crate::translate::to_spanish(app, summary));
        }
    }
    cache.insert(
        key,
        DetailEntry {
            found: resolved.is_some(),
            details: resolved.clone().unwrap_or_default(),
            ts: now(),
        },
    );
    let _ = save_details_cache(app, &cache);
    resolved
}

fn details_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_dir(app)?.join(DETAILS_CACHE_FILE))
}

fn load_details_cache(app: &AppHandle) -> HashMap<String, DetailEntry> {
    details_cache_path(app)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save_details_cache(app: &AppHandle, cache: &HashMap<String, DetailEntry>) -> Result<(), String> {
    let path = details_cache_path(app)?;
    let data = serde_json::to_string(cache).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Drop the URL cache and every downloaded image (e.g. after the IGDB
/// credentials change) so covers are resolved and re-fetched from scratch.
pub fn clear_cache(app: &AppHandle) -> Result<(), String> {
    if let Ok(path) = cache_path(app) {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    if let Ok(path) = details_cache_path(app) {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    if let Ok(dir) = covers_dir(app) {
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Progressively looser search terms: the exact name, then without trademark
/// symbols, then without edition/qualifier suffixes. Deduplicated, order kept.
fn name_variants(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if !s.is_empty() && !out.iter().any(|e| e.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    };

    push(name.to_string());

    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '™' | '®' | '©' | '℠'))
        .collect();
    push(cleaned.clone());

    // Strip common edition/qualifier suffixes for a last, looser attempt.
    const SUFFIXES: &[&str] = &[
        "game of the year edition",
        "goty edition",
        "definitive edition",
        "complete edition",
        "ultimate edition",
        "deluxe edition",
        "gold edition",
        "remastered",
        "directors cut",
        "director's cut",
    ];
    let lower = cleaned.to_lowercase();
    for suffix in SUFFIXES {
        if let Some(pos) = lower.rfind(suffix) {
            if pos + suffix.len() >= lower.len().saturating_sub(1) {
                push(cleaned[..pos].trim_end_matches([' ', '-', ':', '–']).to_string());
                break;
            }
        }
    }

    out
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(15))
        .build()
}

/// Download an image to `dest`. Returns true on success.
fn download(url: &str, dest: &PathBuf) -> bool {
    let Ok(resp) = agent().get(url).call() else {
        return false;
    };
    let mut bytes = Vec::new();
    if resp.into_reader().read_to_end(&mut bytes).is_err() || bytes.is_empty() {
        return false;
    }
    if let Some(parent) = dest.parent() {
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    fs::write(dest, &bytes).is_ok()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_stale(ts: u64) -> bool {
    now().saturating_sub(ts) > NEGATIVE_TTL.as_secs()
}

fn app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_dir(app)?.join(CACHE_FILE))
}

fn covers_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_dir(app)?.join(COVERS_DIR))
}

/// Deterministic on-disk path for a game's cached cover image.
fn cover_file(app: &AppHandle, key: &str) -> Result<PathBuf, String> {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    Ok(covers_dir(app)?.join(format!("{:016x}.jpg", hasher.finish())))
}

fn load_cache(app: &AppHandle) -> HashMap<String, Entry> {
    cache_path(app)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save_cache(app: &AppHandle, cache: &HashMap<String, Entry>) -> Result<(), String> {
    let path = cache_path(app)?;
    let data = serde_json::to_string(cache).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}
