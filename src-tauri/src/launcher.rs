use crate::models::{Game, GameSource};
use std::path::Path;
use std::process::Command;

/// Launch a library entry.
///
/// - Steam games are launched through the `steam://rungameid/<id>` protocol so
///   Steam handles updates/DRM/overlay correctly.
/// - Manual apps are spawned directly, with the working directory set to the
///   executable's folder (many games expect their own folder as CWD).
pub fn launch(game: &Game) -> Result<(), String> {
    match game.source {
        GameSource::Steam => {
            let app_id = game
                .app_id
                .ok_or_else(|| "Juego de Steam sin AppID".to_string())?;
            let url = format!("steam://rungameid/{app_id}");

            #[cfg(target_os = "windows")]
            {
                // `start` needs an (empty) title argument before the URL.
                Command::new("cmd")
                    .args(["/C", "start", "", &url])
                    .spawn()
                    .map_err(|e| format!("No se pudo abrir Steam: {e}"))?;
            }
            #[cfg(not(target_os = "windows"))]
            {
                Command::new("xdg-open")
                    .arg(&url)
                    .spawn()
                    .map_err(|e| format!("No se pudo abrir Steam: {e}"))?;
            }
            Ok(())
        }
        GameSource::Manual => {
            let exe = game
                .executable
                .as_ref()
                .ok_or_else(|| "La app no tiene ejecutable".to_string())?;
            let path = Path::new(exe);
            let mut cmd = Command::new(path);
            if let Some(parent) = path.parent() {
                cmd.current_dir(parent);
            }
            cmd.spawn()
                .map_err(|e| format!("No se pudo iniciar «{}»: {e}", game.name))?;
            Ok(())
        }
    }
}
