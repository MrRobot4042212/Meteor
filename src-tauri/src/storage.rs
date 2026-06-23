use crate::models::{AppSettings, Category, Game};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const STORE_FILE: &str = "manual_apps.json";
const OVERRIDES_FILE: &str = "cover_overrides.json";
const HIDDEN_FILE: &str = "hidden.json";
const HIDDEN_CACHE_FILE: &str = "hidden_cache.json";
const FAVORITES_FILE: &str = "favorites.json";
const CATEGORIES_FILE: &str = "categories.json";
const CATEGORY_NAMES_FILE: &str = "category_names.json";
const CATEGORY_ICONS_FILE: &str = "category_icons.json";
const DISCORD_FILE: &str = "discord.json";
/// User overrides for an entry's kind: id → "app" | "game". Lets the user fix a
/// mis-classified item (a game detected as an app, or vice versa).
const TYPE_OVERRIDES_FILE: &str = "type_overrides.json";
const SETTINGS_FILE: &str = "app_settings.json";


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

/// The Discord application client id for Rich Presence (empty = disabled).
pub fn load_discord_client_id(app: &AppHandle) -> String {
    data_file(app, DISCORD_FILE)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str::<String>(&d).ok())
        .unwrap_or_default()
}

/// Persist the Discord client id (trimmed).
pub fn save_discord_client_id(app: &AppHandle, id: &str) -> Result<(), String> {
    let path = data_file(app, DISCORD_FILE)?;
    let data = serde_json::to_string(id.trim()).map_err(|e| e.to_string())?;
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

/// Save the cached metadata of hidden games, so the UI can show a list to unhide.
pub fn save_hidden_cache(app: &AppHandle, games: &[Game]) -> Result<(), String> {
    let path = data_file(app, HIDDEN_CACHE_FILE)?;
    let data = serde_json::to_string_pretty(games).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Load the cached metadata of hidden games.
pub fn load_hidden_cache(app: &AppHandle) -> Result<Vec<Game>, String> {
    let path = data_file(app, HIDDEN_CACHE_FILE)?;
    if path.exists() {
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    }
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

/// User overrides for an entry's kind (id → "app" | "game"), applied as an
/// overlay in `get_library`. Returns an empty map if none are stored.
pub fn load_type_overrides(app: &AppHandle) -> HashMap<String, String> {
    data_file(app, TYPE_OVERRIDES_FILE)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Set (or clear, with `None`/`""`) the kind override for a game id. Accepts only
/// "app" or "game"; anything else clears the override (back to auto-detection).
pub fn set_type_override(app: &AppHandle, id: &str, kind: Option<&str>) -> Result<(), String> {
    let mut map = load_type_overrides(app);
    match kind {
        Some("app") => {
            map.insert(id.to_string(), "app".to_string());
        }
        Some("game") => {
            map.insert(id.to_string(), "game".to_string());
        }
        _ => {
            map.remove(id);
        }
    }
    let path = data_file(app, TYPE_OVERRIDES_FILE)?;
    let data = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

pub fn load_settings(app: &AppHandle) -> AppSettings {
    let Ok(path) = data_file(app, SETTINGS_FILE) else {
        return AppSettings { setup_completed: false, minimize_to_tray: true };
    };
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(settings) = serde_json::from_str(&data) {
            return settings;
        }
    }
    AppSettings { setup_completed: false, minimize_to_tray: true }
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) {
    if let Ok(path) = data_file(app, SETTINGS_FILE) {
        let _ = fs::write(path, serde_json::to_string_pretty(settings).unwrap_or_default());
    }
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

/// Persist the explicit category order. Trims, drops empties and de-dupes
/// (case-insensitive, first wins). Promotes any in-use names passed in to
/// explicit categories so the chosen order sticks.
pub fn set_category_order(app: &AppHandle, names: &[String]) -> Result<(), String> {
    let mut clean: Vec<String> = Vec::new();
    for n in names {
        let n = n.trim();
        if !n.is_empty() && !clean.iter().any(|c| c.eq_ignore_ascii_case(n)) {
            clean.push(n.to_string());
        }
    }
    let path = data_file(app, CATEGORY_NAMES_FILE)?;
    let data = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

/// Rename a category everywhere: the names list (keeping its position), its icon,
/// and every game's assigned list. If `new` already exists the two **merge**.
pub fn rename_category_name(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    let new = new.trim();
    if new.is_empty() {
        return Err("El nombre no puede estar vacío".into());
    }

    // 1. Names list — replace old with new in place, normalizing/merging.
    let names = load_category_names(app);
    let new_preexists = names
        .iter()
        .any(|n| n.eq_ignore_ascii_case(new) && !n.eq_ignore_ascii_case(old));
    let mut out: Vec<String> = Vec::new();
    let mut placed = false;
    for n in names {
        if n.eq_ignore_ascii_case(old) {
            if !new_preexists && !placed {
                out.push(new.to_string());
                placed = true;
            }
            // merging into an existing target → drop the old entry
        } else if n.eq_ignore_ascii_case(new) {
            if !placed {
                out.push(new.to_string());
                placed = true;
            }
        } else {
            out.push(n);
        }
    }
    if !placed {
        out.push(new.to_string()); // old was in-use only → make it explicit
    }
    let path = data_file(app, CATEGORY_NAMES_FILE)?;
    let data = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())?;

    // 2. Icon — carry old's icon over to new if new doesn't have one already.
    let mut icons = load_category_icons(app);
    if let Some(icon) = icons.remove(old) {
        icons.entry(new.to_string()).or_insert(icon);
    }
    let ip = data_file(app, CATEGORY_ICONS_FILE)?;
    let id = serde_json::to_string_pretty(&icons).map_err(|e| e.to_string())?;
    fs::write(&ip, id).map_err(|e| e.to_string())?;

    // 3. Every game's list — old → new, de-duplicated case-insensitively.
    let mut map = load_categories(app);
    for cats in map.values_mut() {
        let mut nc: Vec<String> = Vec::new();
        for c in cats.drain(..) {
            let name = if c.eq_ignore_ascii_case(old) { new.to_string() } else { c };
            if !nc.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                nc.push(name);
            }
        }
        *cats = nc;
    }
    let cp = data_file(app, CATEGORIES_FILE)?;
    let cd = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&cp, cd).map_err(|e| e.to_string())?;
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
