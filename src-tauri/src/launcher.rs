use crate::models::{Game, GameSource};
use std::path::Path;
use std::process::Command;

/// Launch a library entry.
///
/// - Steam games go through `steam://rungameid/<id>` so Steam handles
///   updates/DRM/overlay.
/// - Other store games with a `launch_uri` (Epic, Ubisoft) are opened through
///   their client protocol for the same reason.
/// - Everything else (manual apps, GOG, Xbox, EA) is spawned directly from its
///   executable, with the working directory set to the executable's folder.
pub fn launch(game: &Game) -> Result<(), String> {
    crate::playtime::notify_launched(&game.id);
    match game.source {
        GameSource::Steam => {
            let app_id = game
                .app_id
                .ok_or_else(|| "Juego de Steam sin AppID".to_string())?;
            open_uri(&format!("steam://rungameid/{app_id}"))
        }
        // Battle.net's `battlenet://` protocol only focuses the launcher, it
        // doesn't start the game — so run the flavor's exe directly (which also
        // lets us track its process for playtime). Fall back to the protocol.
        GameSource::Battlenet => {
            if let Some(exe) = game.executable.as_deref().filter(|e| !e.trim().is_empty()) {
                spawn_exe(exe, &game.name)
            } else if let Some(uri) = game.launch_uri.as_deref().filter(|u| !u.trim().is_empty()) {
                open_uri(uri)
            } else {
                Err(format!("No hay forma de lanzar «{}»", game.name))
            }
        }
        _ => {
            if let Some(uri) = game.launch_uri.as_deref().filter(|u| !u.trim().is_empty()) {
                open_uri(uri)
            } else if let Some(exe) = game.executable.as_deref().filter(|e| !e.trim().is_empty()) {
                spawn_exe(exe, &game.name)
            } else {
                Err(format!("No hay forma de lanzar «{}»", game.name))
            }
        }
    }
}

/// Open a protocol URI through the OS handler.
fn open_uri(uri: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Xbox/Store apps launch by AUMID through the shell app folder.
        if uri.starts_with("shell:") {
            Command::new("explorer.exe")
                .arg(uri)
                .spawn()
                .map_err(|e| format!("No se pudo abrir «{uri}»: {e}"))?;
            return Ok(());
        }
        // `start` needs an (empty) title argument before the URL.
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", uri]);
        // Don't flash a console window for the brief `cmd` invocation.
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.spawn()
            .map_err(|e| format!("No se pudo abrir «{uri}»: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(uri)
            .spawn()
            .map_err(|e| format!("No se pudo abrir «{uri}»: {e}"))?;
    }
    Ok(())
}

/// Spawn an executable directly, running it from its own folder (many games
/// expect their install directory as the current working directory).
fn spawn_exe(exe: &str, name: &str) -> Result<(), String> {
    let path = Path::new(exe);
    let mut cmd = Command::new(path);
    if let Some(parent) = path.parent() {
        cmd.current_dir(parent);
    }
    cmd.spawn()
        .map_err(|e| format!("No se pudo iniciar «{name}»: {e}"))?;
    Ok(())
}
