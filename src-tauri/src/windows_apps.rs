use crate::models::Game;

/// Scan installed programs from the Windows uninstall registry as a catch-all
/// for games from launchers we don't parse natively (Battle.net, Riot, Rockstar,
/// Amazon, itch, standalone installers…).
///
/// This is inherently noisy — the registry lists every program, not just games —
/// so we filter aggressively (skip system components, runtimes, drivers, the
/// launcher clients themselves…). False positives can be hidden from the UI.
/// Entries from known launchers are tagged with a specific `GameSource`; the
/// rest fall back to `GameSource::Windows`.
#[cfg(windows)]
pub fn scan() -> Result<Vec<Game>, String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    const ROOTS: &[(isize, &str)] = &[
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    let mut games = Vec::new();
    for (hive, path) in ROOTS {
        let root = match RegKey::predef(*hive).open_subkey(path) {
            Ok(k) => k,
            Err(_) => continue,
        };
        for sub in root.enum_keys().flatten() {
            let Ok(entry) = root.open_subkey(&sub) else {
                continue;
            };
            if let Some(game) = from_entry(&entry) {
                games.push(game);
            }
        }
    }

    Ok(games)
}

#[cfg(windows)]
fn from_entry(entry: &winreg::RegKey) -> Option<Game> {
    let name: String = entry.get_value("DisplayName").ok()?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return None;
    }

    // Skip OS/update components and sub-entries of other installs.
    if entry.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
        return None;
    }
    if entry.get_value::<String, _>("ParentKeyName").map(|s| !s.is_empty()).unwrap_or(false) {
        return None;
    }
    if let Ok(release) = entry.get_value::<String, _>("ReleaseType") {
        let r = release.to_lowercase();
        if r.contains("update") || r.contains("hotfix") {
            return None;
        }
    }

    let publisher: String = entry.get_value("Publisher").unwrap_or_default();
    let install_location: String = entry.get_value("InstallLocation").unwrap_or_default();
    let display_icon: String = entry.get_value("DisplayIcon").unwrap_or_default();

    if is_excluded(&name, &publisher) {
        return None;
    }

    let executable = exe_from_icon(&display_icon).or_else(|| find_launch_exe(&install_location))?;

    let source = classify(&publisher, &display_icon, &install_location);

    Some(Game {
        id: format!("windows:{}", name.to_lowercase()),
        name,
        source,
        app_id: None,
        executable: Some(executable),
        install_dir: (!install_location.trim().is_empty()).then_some(install_location),
        cover_url: None,
        launch_uri: None,
        favorite: false,
        categories: Vec::new(),
    })
}

/// Tag entries that clearly belong to a known launcher; otherwise generic.
#[cfg(windows)]
fn classify(publisher: &str, display_icon: &str, install_location: &str) -> crate::models::GameSource {
    use crate::models::GameSource;
    let hay = format!("{publisher} {display_icon} {install_location}").to_lowercase();
    if hay.contains("blizzard") || hay.contains("battle.net") {
        GameSource::Battlenet
    } else if hay.contains("riot games") || hay.contains("riot client") {
        GameSource::Riot
    } else if hay.contains("rockstar") {
        GameSource::Rockstar
    } else if hay.contains("amazon games") || hay.contains(r"amazon games\library") {
        GameSource::Amazon
    } else {
        GameSource::Windows
    }
}

/// True for entries that are almost certainly not games: runtimes, drivers,
/// system tooling and the launcher clients themselves (we want their games, not
/// the client entry).
#[cfg(windows)]
fn is_excluded(name: &str, publisher: &str) -> bool {
    let n = name.to_lowercase();
    let p = publisher.to_lowercase();

    const NAME_BLOCK: &[&str] = &[
        "redistributable", "redist", "directx", "vcredist", "visual c++", ".net",
        "dotnet", "framework", " sdk", "runtime", "driver", "geforce", "nvidia",
        "radeon", "vulkan", "update for", "hotfix", "service pack", "python",
        "java(tm)", "java se", "microsoft edge", "webview2", "onedrive",
        "visual studio", "windows software", "debugging tools", "setup",
        "installer", "uninstall",
        // Launcher clients (their games are separate entries / native scanners).
        "steam", "epic games launcher", "gog galaxy", "ea app", "ea desktop",
        "origin", "ubisoft connect", "uplay", "battle.net", "riot client",
        "rockstar games launcher", "amazon games", "discord", "overwolf",
        "playnite", "geforce now",
    ];
    if NAME_BLOCK.iter().any(|b| n.contains(b)) {
        return true;
    }

    const PUB_BLOCK: &[&str] = &[
        "microsoft corporation", "nvidia", "advanced micro devices", "intel",
        "realtek", "google llc", "mozilla", "valve", "oracle", "adobe",
        "python software foundation",
    ];
    PUB_BLOCK.iter().any(|b| p.contains(b))
}

/// Extract an `.exe` path from a `DisplayIcon` value (`C:\game\g.exe,0`).
#[cfg(windows)]
fn exe_from_icon(display_icon: &str) -> Option<String> {
    let path = display_icon.split(',').next().unwrap_or("").trim().trim_matches('"');
    if path.to_lowercase().ends_with(".exe") && std::path::Path::new(path).is_file() {
        Some(path.to_string())
    } else {
        None
    }
}

/// Pick a plausible game executable from `dir`, skipping helpers/installers.
#[cfg(windows)]
fn find_launch_exe(dir: &str) -> Option<String> {
    if dir.trim().is_empty() {
        return None;
    }
    let skip = ["unins", "setup", "vcredist", "redist", "crash", "launcher_installer", "dxsetup"];
    let mut best: Option<std::path::PathBuf> = None;
    let mut best_size = 0u64;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("exe")) != Some(true) {
            continue;
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        if skip.iter().any(|s| stem.contains(s)) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size >= best_size {
            best_size = size;
            best = Some(path);
        }
    }
    best.map(|p| p.to_string_lossy().to_string())
}

#[cfg(not(windows))]
pub fn scan() -> Result<Vec<Game>, String> {
    Ok(Vec::new())
}
