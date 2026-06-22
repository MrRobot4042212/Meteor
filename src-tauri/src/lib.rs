mod art;
mod battlenet;
mod ea;
mod epic;
mod files;
mod gog;
mod igdb;
mod launcher;
mod models;
mod playtime;
mod screenshots;
mod steam;
mod storage;
mod translate;
mod ubisoft;
mod windows_apps;
mod xbox;

use models::{Category, Game, GameSource};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

/// Return the unified library across every supported source, sorted by name.
///
/// Each scanner runs independently: a failure in one source (store not
/// installed, corrupt manifest…) degrades to an empty list for that source
/// instead of failing the whole call. Sources are merged in priority order and
/// deduplicated by name, so a game owned on several stores shows up once with
/// the best available metadata (Steam first, since it ships CDN cover art).
#[tauri::command]
fn get_library(app: AppHandle) -> Result<Vec<Game>, String> {
    let mut games: Vec<Game> = Vec::new();
    games.extend(steam::scan().unwrap_or_default());
    games.extend(epic::scan().unwrap_or_default());
    games.extend(gog::scan().unwrap_or_default());
    games.extend(xbox::scan().unwrap_or_default());
    games.extend(ea::scan().unwrap_or_default());
    games.extend(ubisoft::scan().unwrap_or_default());
    // Battle.net (WoW flavors) before the generic scan so its richer per-flavor
    // entries win the dedup over the single "World of Warcraft" registry entry.
    games.extend(battlenet::scan().unwrap_or_default());
    // Generic registry scan last, so dedup keeps the richer native entry when a
    // game is also found by a dedicated scanner.
    games.extend(windows_apps::scan().unwrap_or_default());
    games.extend(storage::load_manual(&app)?);

    // Deduplicate by name, keeping the first (highest-priority) occurrence.
    let mut seen = HashSet::new();
    games.retain(|g| seen.insert(g.name.to_lowercase()));

    // Drop entries the user has hidden (false positives from the generic scan).
    let hidden = storage::load_hidden(&app);
    if !hidden.is_empty() {
        games.retain(|g| !hidden.iter().any(|h| h == &g.id));
    }

    // User-set cover overrides win over whatever each source provided.
    let overrides = storage::load_cover_overrides(&app);
    if !overrides.is_empty() {
        for game in &mut games {
            if let Some(url) = overrides.get(&game.id) {
                game.cover_url = Some(url.clone());
            }
        }
    }

    // User overlays: favorites and manual categories, keyed by game id.
    let favorites = storage::load_favorites(&app);
    let categories = storage::load_categories(&app);
    if !favorites.is_empty() || !categories.is_empty() {
        for game in &mut games {
            if favorites.iter().any(|f| f == &game.id) {
                game.favorite = true;
            }
            if let Some(cats) = categories.get(&game.id) {
                game.categories = cats.clone();
            }
        }
    }

    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(games)
}

/// Set a manual cover URL for a game id (empty/None clears it). Overrides always
/// take precedence over auto-resolved artwork.
#[tauri::command]
fn set_cover(app: AppHandle, id: String, url: Option<String>) -> Result<(), String> {
    storage::set_cover_override(&app, &id, url.as_deref())
}

/// Save a dropped/picked local image as a game's cover and set it as the override.
/// Returns the saved local path (rendered via the asset protocol).
#[tauri::command]
fn set_cover_image(
    app: AppHandle,
    id: String,
    data: Vec<u8>,
    ext: String,
) -> Result<String, String> {
    let path = storage::save_cover_image(&app, &id, &data, &ext)?;
    storage::set_cover_override(&app, &id, Some(&path))?;
    Ok(path)
}

/// Resolve a cover image for a game name via IGDB, cached on disk. The frontend
/// calls this lazily for entries without artwork.
#[tauri::command]
fn resolve_cover(app: AppHandle, name: String) -> Result<Option<String>, String> {
    Ok(art::resolve(&app, &name))
}

/// Wipe the cover cache (URLs + downloaded images) so everything re-resolves.
#[tauri::command]
fn clear_cover_cache(app: AppHandle) -> Result<(), String> {
    art::clear_cache(&app)
}

/// Hide a game from the library (e.g. a non-game picked up by the registry scan).
#[tauri::command]
fn hide_game(app: AppHandle, id: String) -> Result<(), String> {
    storage::set_hidden(&app, &id, true)
}

/// Number of currently-hidden games (shown in settings so they can be restored).
#[tauri::command]
fn hidden_count(app: AppHandle) -> Result<usize, String> {
    Ok(storage::load_hidden(&app).len())
}

/// Restore every hidden game.
#[tauri::command]
fn restore_hidden(app: AppHandle) -> Result<(), String> {
    storage::clear_hidden(&app)
}

#[tauri::command]
fn add_manual_app(
    app: AppHandle,
    name: String,
    executable: String,
    cover_url: Option<String>,
) -> Result<Game, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre no puede estar vacío".into());
    }

    let mut manual = storage::load_manual(&app)?;
    let id = format!(
        "manual:{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis()
    );

    let game = Game {
        id,
        name,
        source: GameSource::Manual,
        app_id: None,
        executable: Some(executable),
        install_dir: None,
        cover_url: cover_url.filter(|s| !s.trim().is_empty()),
        launch_uri: None,
        favorite: false,
        categories: Vec::new(),
    };

    manual.push(game.clone());
    storage::save_manual(&app, &manual)?;
    Ok(game)
}

/// Remove a manually-added app. Store-managed entries are ignored.
#[tauri::command]
fn remove_game(app: AppHandle, id: String) -> Result<(), String> {
    let mut manual = storage::load_manual(&app)?;
    let before = manual.len();
    manual.retain(|g| g.id != id);
    if manual.len() != before {
        storage::save_manual(&app, &manual)?;
    }
    Ok(())
}

/// Mark or unmark a game as favorite (applies to any source, not just manual).
#[tauri::command]
fn set_favorite(app: AppHandle, id: String, favorite: bool) -> Result<(), String> {
    storage::set_favorite(&app, &id, favorite)
}

/// Replace the manual category list for a game id (empty list clears it).
#[tauri::command]
fn set_categories(app: AppHandle, id: String, categories: Vec<String>) -> Result<(), String> {
    storage::set_categories(&app, &id, &categories)
}

/// Every explicitly-created category with its icon (persist even with zero games).
#[tauri::command]
fn list_categories(app: AppHandle) -> Result<Vec<Category>, String> {
    Ok(storage::load_categories_meta(&app))
}

/// Create a category by name, optionally with an icon key from the bundled set.
#[tauri::command]
fn add_category(app: AppHandle, name: String, icon: Option<String>) -> Result<(), String> {
    storage::add_category_name(&app, &name, icon.as_deref())
}

/// Set (or clear, with None) the icon key for an existing category.
#[tauri::command]
fn set_category_icon(app: AppHandle, name: String, icon: Option<String>) -> Result<(), String> {
    storage::set_category_icon(&app, &name, icon.as_deref())
}

/// Delete a category and strip it from every game.
#[tauri::command]
fn remove_category(app: AppHandle, name: String) -> Result<(), String> {
    storage::remove_category_name(&app, &name)
}

/// Rich IGDB metadata for the detail page (summary, genres, rating, shots…).
#[tauri::command]
fn game_details(app: AppHandle, name: String) -> Result<Option<igdb::GameDetails>, String> {
    Ok(art::details(&app, &name))
}

/// Accumulated play stats (seconds + last played) for a game id.
#[tauri::command]
fn get_playtime(app: AppHandle, id: String) -> Result<playtime::PlayStat, String> {
    Ok(playtime::get(&app, &id))
}

/// Total size in bytes of a directory (for the detail page's file info).
#[tauri::command]
fn dir_size(path: String) -> Result<u64, String> {
    files::dir_size(&path)
}

/// Open a folder in the OS file manager.
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    files::open_path(&path)
}

/// The user's own screenshots for a game (Steam + Windows Game Bar).
#[tauri::command]
fn user_screenshots(app: AppHandle, game: Game) -> Result<Vec<String>, String> {
    Ok(screenshots::user_screenshots(&app, &game))
}

#[tauri::command]
fn launch_game(app: AppHandle, game: Game) -> Result<(), String> {
    launcher::launch(&game)?;
    // Watch the launched game in the background to accumulate playtime.
    playtime::track(app, game);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_library,
            resolve_cover,
            set_cover,
            set_cover_image,
            clear_cover_cache,
            hide_game,
            hidden_count,
            restore_hidden,
            add_manual_app,
            remove_game,
            set_favorite,
            set_categories,
            list_categories,
            add_category,
            set_category_icon,
            remove_category,
            game_details,
            get_playtime,
            dir_size,
            open_path,
            user_screenshots,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación Tauri");
}
