use crate::models::Game;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const STORE_FILE: &str = "manual_apps.json";

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
