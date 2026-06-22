use crate::models::{Category, Game};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const STORE_FILE: &str = "manual_apps.json";
const OVERRIDES_FILE: &str = "cover_overrides.json";
const HIDDEN_FILE: &str = "hidden.json";
const FAVORITES_FILE: &str = "favorites.json";
const CATEGORIES_FILE: &str = "categories.json";
const CATEGORY_NAMES_FILE: &str = "category_names.json";
const CATEGORY_ICONS_FILE: &str = "category_icons.json";

/// Resolve a file inside the app data dir, creating the dir if needed.
fn data_file(app: &AppHandle, file: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(file))
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(STORE_FILE))
}

/// Load manually-added apps. Returns an empty list if nothing is stored yet.
pub fn load_manual(app: &AppHandle) -> Result<Vec<Game>, String> {
    let path = store_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("manual_apps.json corrupto: {e}"))
}

/// Persist the full list of manually-added apps.
pub fn save_manual(app: &AppHandle, games: &[Game]) -> Result<(), String> {
    let path = store_path(app)?;
    let data = serde_json::to_string_pretty(games).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Allowed image extensions for a user-supplied (dropped/picked) cover.
const COVER_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Save a user-supplied cover image (dropped or picked from disk) into the
/// `user_covers` dir under app data, replacing any previous one for this id, and
/// return its absolute path. The dir is in the asset-protocol scope so the
/// webview can render it; it survives `clear_cover_cache` (which only wipes the
/// auto-resolved `covers/` dir).
pub fn save_cover_image(
    app: &AppHandle,
    id: &str,
    data: &[u8],
    ext: &str,
) -> Result<String, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    if data.is_empty() {
        return Err("La imagen está vacía".into());
    }
    let ext = ext.trim().trim_start_matches('.').to_lowercase();
    let ext = if COVER_EXTS.contains(&ext.as_str()) {
        ext
    } else {
        "jpg".to_string()
    };

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?
        .join("user_covers");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut h = DefaultHasher::new();
    id.hash(&mut h);
    let stem = format!("{:016x}", h.finish());

    // Drop any previous cover for this id (possibly a different extension).
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&stem) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    let path = dir.join(format!("{stem}.{ext}"));
    fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn overrides_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(OVERRIDES_FILE))
}

/// User-set cover overrides keyed by game id. These win over auto-resolution, so
/// a cover can always be fixed by hand. Returns an empty map if none are stored.
pub fn load_cover_overrides(app: &AppHandle) -> HashMap<String, String> {
    overrides_path(app)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Set (or, with an empty url, clear) the cover override for a game id.
pub fn set_cover_override(app: &AppHandle, id: &str, url: Option<&str>) -> Result<(), String> {
    let mut map = load_cover_overrides(app);
    match url.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => {
            map.insert(id.to_string(), u.to_string());
        }
        None => {
            map.remove(id);
        }
    }
    let path = overrides_path(app)?;
    let data = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

fn hidden_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(HIDDEN_FILE))
}

/// Ids of games the user has hidden from the library (mostly false positives
/// from the generic registry scan). Returns an empty list if none.
pub fn load_hidden(app: &AppHandle) -> Vec<String> {
    hidden_path(app)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Hide or unhide a game id.
pub fn set_hidden(app: &AppHandle, id: &str, hidden: bool) -> Result<(), String> {
    let mut ids = load_hidden(app);
    if hidden {
        if !ids.iter().any(|x| x == id) {
            ids.push(id.to_string());
        }
    } else {
        ids.retain(|x| x != id);
    }
    let path = hidden_path(app)?;
    let data = serde_json::to_string_pretty(&ids).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Unhide everything.
pub fn clear_hidden(app: &AppHandle) -> Result<(), String> {
    let path = hidden_path(app)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Ids the user marked as favorites. Applied as an overlay in `get_library`.
/// Returns an empty list if none.
pub fn load_favorites(app: &AppHandle) -> Vec<String> {
    data_file(app, FAVORITES_FILE)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Mark or unmark a game id as favorite.
pub fn set_favorite(app: &AppHandle, id: &str, favorite: bool) -> Result<(), String> {
    let mut ids = load_favorites(app);
    if favorite {
        if !ids.iter().any(|x| x == id) {
            ids.push(id.to_string());
        }
    } else {
        ids.retain(|x| x != id);
    }
    let path = data_file(app, FAVORITES_FILE)?;
    let data = serde_json::to_string_pretty(&ids).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// User-assigned categories keyed by game id. Applied as an overlay in
/// `get_library`. Returns an empty map if none are stored.
pub fn load_categories(app: &AppHandle) -> HashMap<String, Vec<String>> {
    data_file(app, CATEGORIES_FILE)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Replace the full category list for a game id (an empty list clears the entry).
/// Names are trimmed and de-duplicated, preserving order.
pub fn set_categories(app: &AppHandle, id: &str, categories: &[String]) -> Result<(), String> {
    let mut clean: Vec<String> = Vec::new();
    for name in categories {
        let name = name.trim();
        if !name.is_empty() && !clean.iter().any(|c| c.eq_ignore_ascii_case(name)) {
            clean.push(name.to_string());
        }
    }
    let mut map = load_categories(app);
    if clean.is_empty() {
        map.remove(id);
    } else {
        map.insert(id.to_string(), clean);
    }
    let path = data_file(app, CATEGORIES_FILE)?;
    let data = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Explicitly-created category names. These persist even with zero games, so a
/// category can be created from the sidebar and used to assign games afterwards.
pub fn load_category_names(app: &AppHandle) -> Vec<String> {
    data_file(app, CATEGORY_NAMES_FILE)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Icon key chosen for each category (resolved to an SVG on the frontend).
pub fn load_category_icons(app: &AppHandle) -> HashMap<String, String> {
    data_file(app, CATEGORY_ICONS_FILE)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Set (or, with None/empty, clear) the icon key for a category name.
pub fn set_category_icon(app: &AppHandle, name: &str, icon: Option<&str>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("El nombre no puede estar vacío".into());
    }
    let mut map = load_category_icons(app);
    match icon.map(str::trim).filter(|i| !i.is_empty()) {
        Some(i) => {
            map.insert(name.to_string(), i.to_string());
        }
        None => {
            map.remove(name);
        }
    }
    let path = data_file(app, CATEGORY_ICONS_FILE)?;
    let data = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Every explicitly-created category with its icon (zips names + icon map).
pub fn load_categories_meta(app: &AppHandle) -> Vec<Category> {
    let icons = load_category_icons(app);
    load_category_names(app)
        .into_iter()
        .map(|name| {
            let icon = icons.get(&name).cloned();
            Category { name, icon }
        })
        .collect()
}

/// Create a category by name (trimmed, deduped case-insensitively), optionally
/// with an icon key.
pub fn add_category_name(app: &AppHandle, name: &str, icon: Option<&str>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("El nombre no puede estar vacío".into());
    }
    let mut names = load_category_names(app);
    if !names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        names.push(name.to_string());
    }
    let path = data_file(app, CATEGORY_NAMES_FILE)?;
    let data = serde_json::to_string_pretty(&names).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())?;
    if icon.map(str::trim).is_some_and(|i| !i.is_empty()) {
        set_category_icon(app, name, icon)?;
    }
    Ok(())
}

/// Delete a category: remove the name, its icon, and strip it from every game.
pub fn remove_category_name(app: &AppHandle, name: &str) -> Result<(), String> {
    let mut names = load_category_names(app);
    names.retain(|n| !n.eq_ignore_ascii_case(name));
    let path = data_file(app, CATEGORY_NAMES_FILE)?;
    let data = serde_json::to_string_pretty(&names).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())?;

    set_category_icon(app, name, None)?;

    let mut map = load_categories(app);
    for cats in map.values_mut() {
        cats.retain(|c| !c.eq_ignore_ascii_case(name));
    }
    map.retain(|_, v| !v.is_empty());
    let p = data_file(app, CATEGORIES_FILE)?;
    let d = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&p, d).map_err(|e| e.to_string())?;
    Ok(())
}
