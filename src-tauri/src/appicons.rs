//! Extract the real icon embedded in an application's executable, so any
//! installed app shows its correct icon — no curated list needed.
//!
//! The exe's primary icon group is read straight from its PE resources (pure
//! Rust, no native libs) and written to a cached `.ico` in `app_icons/`, then
//! authorized in the asset-protocol scope at runtime so the webview can render
//! it via `convertFileSrc` (WebView2/Chromium displays `.ico` in `<img>`). Used
//! by the frontend as a fallback when an entry has neither a cover nor a known
//! brand logo.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?.join("app_icons");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// A `DisplayIcon`/exe value can carry an index (`C:\app\app.exe,0`) — keep the
/// path part only.
fn icon_source(raw: &str) -> String {
    raw.split(',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .to_string()
}

fn hashed(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.to_lowercase().hash(&mut h);
    format!("{:x}", h.finish())
}

/// Extract (or reuse the cached) icon for an executable/icon path. Returns a
/// local `.ico` path authorized in the asset scope, or None if extraction fails.
#[cfg(windows)]
pub fn extract(app: &AppHandle, source: &str) -> Option<String> {
    use pelite::{FileMap, PeFile};

    let path = icon_source(source);
    if path.is_empty() || !std::path::Path::new(&path).exists() {
        return None;
    }

    // If the source is already an .ico, serve it directly.
    if path.to_lowercase().ends_with(".ico") {
        let _ = app.asset_protocol_scope().allow_file(&path);
        return Some(path);
    }

    let dir = cache_dir(app)?;
    let out = dir.join(format!("{}.ico", hashed(&path)));

    if !out.exists() {
        let map = FileMap::open(&path).ok()?;
        let file = PeFile::from_bytes(&map).ok()?;
        let resources = file.resources().ok()?;
        // The first icon group is the app's primary icon.
        let mut ico = Vec::new();
        let mut wrote = false;
        for (_, group) in resources.icons().filter_map(Result::ok) {
            if group.write(&mut ico).is_ok() {
                wrote = true;
                break;
            }
        }
        if !wrote {
            return None;
        }
        fs::write(&out, &ico).ok()?;
    }

    // Authorize every call (cached too) so it survives restarts.
    let _ = app.asset_protocol_scope().allow_file(&out);
    Some(out.to_string_lossy().to_string())
}

#[cfg(not(windows))]
pub fn extract(_app: &AppHandle, _source: &str) -> Option<String> {
    None
}
