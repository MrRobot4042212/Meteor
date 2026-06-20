mod launcher;
mod models;
mod steam;
mod storage;

use models::{Game, GameSource};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

/// Return the unified library: Steam games + manually-added apps, sorted by name.
/// A Steam scan failure (e.g. Steam not installed) degrades gracefully to just
/// the manual apps instead of failing the whole call.
#[tauri::command]
fn get_library(app: AppHandle) -> Result<Vec<Game>, String> {
    let mut games = steam::scan().unwrap_or_default();
    let manual = storage::load_manual(&app)?;
    games.extend(manual);
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(games)
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
    };

    manual.push(game.clone());
    storage::save_manual(&app, &manual)?;
    Ok(game)
}

/// Remove a manually-added app. Steam entries are managed by Steam and ignored.
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

#[tauri::command]
fn launch_game(game: Game) -> Result<(), String> {
    launcher::launch(&game)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_library,
            add_manual_app,
            remove_game,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación Tauri");
}
