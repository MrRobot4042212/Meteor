use crate::models::{Game, GameSource};
use std::path::{Path, PathBuf};

/// Scan Xbox / Microsoft Store (Game Pass) games.
///
/// Primary path: query installed AppX packages whose folder ships a
/// `MicrosoftGame.config` (the reliable "this is an Xbox game" marker), which
/// also catches games installed to `WindowsApps`, not just `XboxGames`. We
/// resolve each game's AUMID so it launches through the shell like the Store
/// does. If PowerShell is unavailable we fall back to scanning `XboxGames`.
pub fn scan() -> Result<Vec<Game>, String> {
    let appx = scan_appx();
    if !appx.is_empty() {
        return Ok(appx);
    }
    Ok(scan_folders())
}

/// One game as emitted by the PowerShell helper.
#[derive(serde::Deserialize)]
struct AppxGame {
    name: Option<String>,
    aumid: Option<String>,
    loc: Option<String>,
}

/// Enumerate AppX packages, keep those that are games (have a
/// `MicrosoftGame.config`), and resolve name + AUMID for each.
fn scan_appx() -> Vec<Game> {
    const SCRIPT: &str = r#"
$out = foreach ($p in Get-AppxPackage) {
  $loc = $p.InstallLocation
  if (-not $loc) { continue }
  $cfg = Join-Path $loc 'MicrosoftGame.config'
  $cfg2 = Join-Path $loc 'Content\MicrosoftGame.config'
  if ((Test-Path $cfg) -or (Test-Path $cfg2)) {
    try {
      $app = (Get-AppxPackageManifest $p.PackageFullName).Package.Applications.Application | Select-Object -First 1
      $id = $app.Id
      $name = $app.VisualElements.DisplayName
      [pscustomobject]@{ name = $name; aumid = ($p.PackageFamilyName + '!' + $id); loc = $loc }
    } catch {}
  }
}
ConvertTo-Json -Compress -InputObject @($out)
"#;

    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT]);
    // Don't flash a console window when spawning PowerShell from the GUI.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // `@(...)` forces an array, but PowerShell 5.1 still emits a bare object for
    // a single element — accept both.
    let entries: Vec<AppxGame> = serde_json::from_str(trimmed)
        .or_else(|_| serde_json::from_str::<AppxGame>(trimmed).map(|g| vec![g]))
        .unwrap_or_default();

    entries
        .into_iter()
        .filter_map(|e| {
            let aumid = e.aumid.filter(|s| !s.trim().is_empty())?;
            let loc = e.loc.unwrap_or_default();
            // Manifest names are often `ms-resource:…` placeholders; fall back to
            // the display name inside MicrosoftGame.config.
            let name = e
                .name
                .filter(|n| !n.trim().is_empty() && !n.starts_with("ms-resource"))
                .or_else(|| display_name_from_config(&loc))?;

            Some(Game {
                id: format!("xbox:{}", aumid),
                name,
                source: GameSource::Xbox,
                app_id: None,
                executable: None,
                install_dir: (!loc.is_empty()).then_some(loc),
                cover_url: None,
                launch_uri: Some(format!("shell:appsFolder\\{aumid}")),
                favorite: false,
                categories: Vec::new(),
            })
        })
        .collect()
}

/// Read `DefaultDisplayName` from a package's `MicrosoftGame.config`.
fn display_name_from_config(loc: &str) -> Option<String> {
    if loc.is_empty() {
        return None;
    }
    let base = Path::new(loc);
    for cfg in [base.join("MicrosoftGame.config"), base.join("Content/MicrosoftGame.config")] {
        if let Ok(xml) = std::fs::read_to_string(&cfg) {
            if let Some(name) = attr(&xml, "DefaultDisplayName") {
                return Some(name);
            }
        }
    }
    None
}

/// Fallback: scan `<drive>:\XboxGames\*\Content` for `MicrosoftGame.config`,
/// launching via `gamelaunchhelper.exe`.
fn scan_folders() -> Vec<Game> {
    let mut games = Vec::new();
    for root in xbox_game_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let content = entry.path().join("Content");
            let config = content.join("MicrosoftGame.config");
            if !config.is_file() {
                continue;
            }
            let Ok(xml) = std::fs::read_to_string(&config) else {
                continue;
            };
            let name = attr(&xml, "DefaultDisplayName")
                .or_else(|| entry.path().file_name().map(|s| s.to_string_lossy().to_string()))
                .unwrap_or_default();
            if name.trim().is_empty() {
                continue;
            }
            games.push(Game {
                id: format!("xbox:{}", name.to_lowercase()),
                name,
                source: GameSource::Xbox,
                app_id: None,
                executable: pick_executable(&content, &xml),
                install_dir: Some(content.to_string_lossy().to_string()),
                cover_url: None,
                launch_uri: None,
                favorite: false,
                categories: Vec::new(),
            });
        }
    }
    games
}

fn xbox_game_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for letter in b'A'..=b'Z' {
        let path = PathBuf::from(format!("{}:\\XboxGames", letter as char));
        if path.is_dir() {
            roots.push(path);
        }
    }
    roots
}

fn pick_executable(content: &Path, xml: &str) -> Option<String> {
    let helper = content.join("gamelaunchhelper.exe");
    if helper.is_file() {
        return Some(helper.to_string_lossy().to_string());
    }
    let exe = attr(xml, "Executable Name")?;
    let path = content.join(exe);
    path.is_file().then(|| path.to_string_lossy().to_string())
}

/// Minimal XML attribute extractor: value of the first `key="value"`.
fn attr(xml: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = xml.find(&needle)? + needle.len();
    let rest = &xml[start..];
    let end = rest.find('"')?;
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}
