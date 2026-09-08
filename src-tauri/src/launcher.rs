//! Launching library entries.
//!
//! Two paths, both validated before anything reaches the shell:
//!
//! - **Protocol URIs** (Steam, Epic, Ubisoft, Battle.net, Xbox AUMIDs) go through
//!   `ShellExecuteW` with an explicit **scheme allowlist**. Never through
//!   `cmd /C start`: that hands the string to the command interpreter, where a
//!   crafted `launch_uri` (e.g. from a poisoned library cache) becomes command
//!   injection, and it also spawns a `cmd.exe` that inherits our token.
//! - **Executables** are canonicalized, checked to be a real file with an
//!   allowed extension, and — when the entry has a known `install_dir` —
//!   required to live inside it.
//!
//! The `Game` handed to `launch` is always re-resolved in Rust from the library
//! cache or the manual store (`lib.rs::launch_game` takes an **id**), so the
//! webview cannot fabricate a target.

use crate::models::{Game, GameSource};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Protocol schemes Meteor is allowed to open. One entry per store scanner that
/// emits a `launch_uri` (`launcher.rs` for Steam, `epic.rs`, `ubisoft.rs`,
/// `battlenet.rs`); `shell:` (Xbox AUMIDs) is handled separately below because
/// it is opened through Explorer rather than a protocol handler.
const ALLOWED_SCHEMES: &[&str] = &["steam", "com.epicgames.launcher", "uplay", "battlenet"];

/// Extensions we will start directly. Matches the picker in `AddAppDialog`
/// (`extensions: ['exe', 'lnk', 'bat']`).
const ALLOWED_EXE_EXT: &[&str] = &["exe", "lnk", "bat"];

/// Prefix of the Xbox/Store AUMID launch target (`shell:appsFolder\<aumid>`).
const APPS_FOLDER_PREFIX: &str = "shell:appsfolder\\";

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
                spawn_exe(game, exe)
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
                spawn_exe(game, exe)
            } else {
                Err(format!("No hay forma de lanzar «{}»", game.name))
            }
        }
    }
}

/// The lowercased scheme of a URI (the part before the first `:`), if it looks
/// like one at all (RFC 3986: `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`).
fn scheme_of(uri: &str) -> Option<String> {
    let (scheme, rest) = uri.split_once(':')?;
    if scheme.is_empty() || rest.is_empty() {
        return None;
    }
    let mut chars = scheme.chars();
    if !chars.next()?.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

/// Whether this URI may be handed to the shell: a known store scheme, no control
/// characters, and no embedded quote a handler might re-split on.
pub(crate) fn is_allowed_uri(uri: &str) -> bool {
    if uri.chars().any(|c| c.is_control() || c == '"') {
        return false;
    }
    if uri.to_ascii_lowercase().starts_with(APPS_FOLDER_PREFIX) {
        // `shell:appsFolder\<PackageFamilyName>!<AppId>` — no further path
        // separators, so it cannot be pointed at an arbitrary shell location.
        let aumid = &uri[APPS_FOLDER_PREFIX.len()..];
        return !aumid.is_empty() && !aumid.contains('/') && !aumid.contains('\\');
    }
    let Some(scheme) = scheme_of(uri) else {
        return false;
    };
    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        return false;
    }
    // Require an actual target after the scheme: a bare "steam://" carries no
    // game and only pops the store open.
    !uri[scheme.len() + 1..].trim_start_matches('/').trim().is_empty()
}

/// Open a protocol URI through the OS handler, after the allowlist check.
fn open_uri(uri: &str) -> Result<(), String> {
    if !is_allowed_uri(uri) {
        return Err(format!("URI de lanzamiento no permitida: «{uri}»"));
    }
    #[cfg(target_os = "windows")]
    {
        // Xbox/Store apps launch by AUMID through the shell app folder, which
        // only Explorer resolves. Absolute path: never resolve a system binary
        // through PATH (this process may be elevated).
        if uri.to_ascii_lowercase().starts_with(APPS_FOLDER_PREFIX) {
            Command::new(crate::files::system_exe("explorer.exe"))
                .arg(uri)
                .spawn()
                .map_err(|e| format!("No se pudo abrir «{uri}»: {e}"))?;
            return Ok(());
        }
        shell_execute(uri, None)?;
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

/// `ShellExecuteW(open)` on an already-verified target, optionally with a working
/// directory. Used for protocol URIs and for `.lnk` shortcuts, which
/// `CreateProcess` (and therefore `Command::spawn`) cannot start.
#[cfg(target_os = "windows")]
fn shell_execute(target: &str, dir: Option<&Path>) -> Result<(), String> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let target_w = HSTRING::from(target);
    let dir_w = dir.map(|d| HSTRING::from(d.as_os_str()));
    // SAFETY: both HSTRINGs outlive the call, and ShellExecuteW only reads them.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(target_w.as_ptr()),
            PCWSTR::null(),
            dir_w
                .as_ref()
                .map(|d| PCWSTR(d.as_ptr()))
                .unwrap_or_else(PCWSTR::null),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns an HINSTANCE; values <= 32 indicate failure.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!(
            "No se pudo abrir «{target}» (error {})",
            result.0 as isize
        ))
    }
}

/// Validate an executable before starting it: it must resolve to an existing
/// file with an allowed extension and, when the entry declares an `install_dir`
/// that still exists, live inside that directory.
fn validate_exe(game: &Game, exe: &str) -> Result<PathBuf, String> {
    let canon =
        std::fs::canonicalize(exe).map_err(|e| format!("No se pudo resolver «{exe}»: {e}"))?;
    if !canon.is_file() {
        return Err(format!("«{exe}» no es un archivo ejecutable"));
    }
    let ext_ok = canon
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| ALLOWED_EXE_EXT.contains(&e.as_str()));
    if !ext_ok {
        return Err(format!("Extensión no permitida para lanzar: «{exe}»"));
    }
    if let Some(dir) = game.install_dir.as_deref().filter(|d| !d.trim().is_empty()) {
        // Only enforced when the install dir still resolves: an entry whose
        // folder was deleted should fail on the exe check above, not here.
        if let Ok(root) = std::fs::canonicalize(dir) {
            if !canon.starts_with(&root) {
                return Err(format!(
                    "El ejecutable de «{}» está fuera de su carpeta de instalación",
                    game.name
                ));
            }
        }
    }
    Ok(canon)
}

/// Spawn a validated executable, running it from its own folder (many games
/// expect their install directory as the current working directory).
fn spawn_exe(game: &Game, exe: &str) -> Result<(), String> {
    let canon = validate_exe(game, exe)?;
    let path = crate::files::strip_verbatim(&canon);
    let dir = path.parent().map(|p| p.to_path_buf());

    #[cfg(target_os = "windows")]
    {
        // `.lnk` shortcuts are resolved by the shell, not by CreateProcess.
        let is_lnk = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
        if is_lnk {
            return shell_execute(&path.to_string_lossy(), dir.as_deref());
        }
    }

    let mut cmd = Command::new(&path);
    if let Some(parent) = dir {
        cmd.current_dir(parent);
    }
    cmd.spawn()
        .map_err(|e| format!("No se pudo iniciar «{}»: {e}", game.name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_is_lowercased_and_validated() {
        assert_eq!(scheme_of("STEAM://rungameid/440").as_deref(), Some("steam"));
        assert_eq!(
            scheme_of("com.epicgames.launcher://apps/x").as_deref(),
            Some("com.epicgames.launcher")
        );
        assert_eq!(scheme_of("no-scheme"), None);
        assert_eq!(scheme_of("1nvalid://x"), None);
        assert_eq!(scheme_of("steam://").as_deref(), Some("steam"));
    }

    #[test]
    fn only_known_store_schemes_are_allowed() {
        assert!(is_allowed_uri("steam://rungameid/440"));
        assert!(is_allowed_uri("uplay://launch/123/0"));
        assert!(is_allowed_uri("battlenet://WoW"));
        assert!(is_allowed_uri("com.epicgames.launcher://apps/fortnite"));
        assert!(!is_allowed_uri("file:///C:/Windows/System32/cmd.exe"));
        assert!(!is_allowed_uri("https://example.com"));
        assert!(!is_allowed_uri("javascript:alert(1)"));
        assert!(!is_allowed_uri("C:\\Windows\\System32\\cmd.exe"));
    }

    #[test]
    fn rejects_injection_shaped_uris() {
        // Regression: the old `cmd /C start "" <uri>` path executed these.
        assert!(!is_allowed_uri("steam://run\" & calc.exe & \""));
        assert!(!is_allowed_uri("steam://run\ncalc.exe"));
        assert!(!is_allowed_uri("steam://run\u{0}calc"));
    }

    #[test]
    fn appsfolder_aumids_are_constrained() {
        assert!(is_allowed_uri("shell:appsFolder\\Microsoft.Game_8wek!App"));
        assert!(!is_allowed_uri("shell:appsFolder\\..\\..\\Windows\\System32"));
        assert!(!is_allowed_uri("shell:appsFolder\\"));
        assert!(!is_allowed_uri("shell:startup"));
    }
}
