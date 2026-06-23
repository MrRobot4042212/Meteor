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
            if let Some(game) = from_entry(&entry, &sub) {
                games.push(game);
            }
        }
    }

    Ok(games)
}

#[cfg(windows)]
fn from_entry(entry: &winreg::RegKey, key_name: &str) -> Option<Game> {
    let raw: String = entry.get_value("DisplayName").ok()?;
    let raw = raw.trim();
    // Show a clean product name: many entries append version/arch (e.g. "OBS
    // Studio 30.0.2", "7-Zip (x64)", "Git version 2.43.0").
    let cleaned = clean_name(raw);
    let name = if cleaned.is_empty() { raw.to_string() } else { cleaned };
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

    if is_junk(&name, &publisher) {
        return None;
    }

    let executable = exe_from_icon(&display_icon).or_else(|| find_launch_exe(&install_location))?;

    let source = classify(&name, &publisher, &display_icon, &install_location);

    Some(Game {
        // Stable id from the uninstall registry key (a GUID/product code or the
        // installer's own key) — survives DisplayName changes/cleaning, so user
        // overlays (hidden/favorites/categories/playtime) don't get lost.
        id: format!("windows:{}", key_name.to_lowercase()),
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

/// Decide the source for a registry entry: a known game launcher, an
/// auto-detected application, or the generic (likely-game) catch-all.
#[cfg(windows)]
fn classify(
    name: &str,
    publisher: &str,
    display_icon: &str,
    install_location: &str,
) -> crate::models::GameSource {
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
    } else if hay.contains("battlestate games") || hay.contains("escape from tarkov") || hay.contains("bsglauncher") {
        GameSource::Battlestate
    } else if crate::apps_db::is_app(name, publisher, install_location) {
        GameSource::App
    } else {
        GameSource::Windows
    }
}

/// True for entries that are never launchable library items: runtimes, drivers,
/// system tooling, hardware-vendor software and the store launcher clients
/// themselves (we want their games, not the client entry). Apps the user might
/// actually want (browsers, office, dev tools…) are NOT dropped here — they are
/// surfaced and tagged `GameSource::App` by `classify`/`is_app`.
#[cfg(windows)]
fn is_junk(name: &str, publisher: &str) -> bool {
    let n = name.to_lowercase();
    let p = publisher.to_lowercase();

    const NAME_BLOCK: &[&str] = &[
        "redistributable", "redist", "directx", "vcredist", "visual c++", ".net",
        "dotnet", "framework", " sdk", "runtime", "driver", "geforce", "nvidia",
        "radeon", "vulkan", "update for", "hotfix", "service pack", "python",
        "java(tm)", "java se", "microsoft edge", "webview2", "onedrive",
        "windows software", "debugging tools", "setup", "installer", "uninstall",
        "maintenance service", "update health", "google update", "active directory",
        // Launcher clients (their games are separate entries / native scanners).
        "steam", "epic games launcher", "gog galaxy", "ea app", "ea desktop",
        "origin", "ubisoft connect", "uplay", "battle.net", "riot client",
        "rockstar games launcher", "amazon games", "discord", "overwolf",
        "playnite", "geforce now", "meteor","blender","7-zip"
    ];
    if NAME_BLOCK.iter().any(|b| n.contains(b)) {
        return true;
    }

    // Hardware/driver and pure-runtime vendors only — consumer-app publishers
    // (Microsoft, Google, Adobe, Mozilla…) are handled as apps, not blocked.
    const PUB_BLOCK: &[&str] = &[
        "nvidia", "advanced micro devices", "intel", "realtek", "oracle",
        "python software foundation",
    ];
    PUB_BLOCK.iter().any(|b| p.contains(b))
}

/// Strip version/architecture noise from a registry `DisplayName`, leaving the
/// plain product name (e.g. "OBS Studio 30.0.2" → "OBS Studio", "7-Zip (x64)" →
/// "7-Zip", "Git version 2.43.0" → "Git"). Conservative: keeps meaningful numbers
/// without a dot (editions/years like "Office 365", "Visual Studio 2022").
#[cfg(windows)]
fn clean_name(raw: &str) -> String {
    strip_version_tail(&strip_noise_brackets(raw))
}

/// Drop bracketed chunks `(...)`/`[...]` that are version/arch noise (contain a
/// digit or "bit"/"x64"/"x86"); keep purely descriptive ones.
#[cfg(windows)]
fn strip_noise_brackets(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        let close = match c {
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        };
        let Some(close) = close else {
            out.push(c);
            continue;
        };
        let mut inner = String::new();
        let mut closed = false;
        for ic in chars.by_ref() {
            if ic == close {
                closed = true;
                break;
            }
            inner.push(ic);
        }
        let low = inner.to_lowercase();
        let noise = low.chars().any(|c| c.is_ascii_digit())
            || low.contains("bit")
            || low.contains("x64")
            || low.contains("x86");
        if !closed {
            // Unmatched bracket: keep the text as-is, don't lose content.
            out.push(c);
            out.push_str(&inner);
        } else if !noise {
            out.push(c);
            out.push_str(&inner);
            out.push(close);
        }
        // else: drop the noisy bracket entirely
    }
    out
}

/// Drop trailing version/arch tokens ("2.43.0", "v1.2", "x64", "64-bit",
/// "version"), stopping at the first token that's part of the real name.
#[cfg(windows)]
fn strip_version_tail(s: &str) -> String {
    let mut tokens: Vec<&str> = s.split_whitespace().collect();
    while let Some(last) = tokens.last() {
        let low = last.to_lowercase();
        let no_v = last.trim_start_matches(['v', 'V']);
        let dotted_version = !no_v.is_empty()
            && no_v.contains('.')
            && no_v.chars().all(|c| c.is_ascii_digit() || c == '.');
        let arch = matches!(
            low.as_str(),
            "x64" | "x86" | "64-bit" | "32-bit" | "win64" | "win32" | "amd64" | "(x64)" | "(x86)"
        );
        if dotted_version || arch || low == "version" {
            tokens.pop();
        } else {
            break;
        }
    }
    tokens
        .join(" ")
        .trim_end_matches([' ', '-', ',', ':', '·'])
        .trim()
        .to_string()
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
