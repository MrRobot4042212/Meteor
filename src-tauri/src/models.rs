use serde::{Deserialize, Serialize};

/// Where a library entry comes from. Each variant is produced by its own scanner
/// module (`steam.rs`, `epic.rs`, `gog.rs`, …) and merged in `get_library`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameSource {
    Steam,
    Epic,
    Gog,
    Ea,
    Ubisoft,
    Xbox,
    Battlenet,
    Riot,
    Rockstar,
    Amazon,
    Battlestate,
    /// Generic catch-all: anything found via the Windows uninstall registry that
    /// isn't claimed by a more specific scanner.
    Windows,
    /// A non-game application auto-detected from the uninstall registry (browsers,
    /// office suites, dev tools, media players…). Classified in `windows_apps.rs`.
    App,
    Manual,
}

/// A user-created category, optionally with a chosen icon key (resolved to an
/// SVG on the frontend from a bundled icon set). Persisted in `storage.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// A single launchable entry in the unified library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    /// Stable unique id, e.g. "steam:440", "epic:Fortnite", "manual:1718900000000".
    pub id: String,
    pub name: String,
    pub source: GameSource,

    /// Steam AppID (only for `GameSource::Steam`).
    #[serde(default)]
    pub app_id: Option<u32>,

    /// Absolute path to the executable, when the game is launched directly
    /// (manual apps, GOG, Xbox, some EA games).
    #[serde(default)]
    pub executable: Option<String>,

    /// Install directory on disk, when known.
    #[serde(default)]
    pub install_dir: Option<String>,

    /// Remote cover image URL. For Steam this is the vertical capsule; for other
    /// stores it is resolved lazily via `art.rs` (SteamGridDB / Steam CDN).
    #[serde(default)]
    pub cover_url: Option<String>,

    /// Protocol URI used to launch store-managed games (Epic / Ubisoft), so the
    /// store handles DRM/updates/overlay. Takes precedence over `executable`.
    #[serde(default)]
    pub launch_uri: Option<String>,

    /// User overlay: whether the user marked this entry as a favorite. Not set by
    /// scanners; applied in `get_library` from `favorites.json` (`storage.rs`).
    #[serde(default)]
    pub favorite: bool,

    /// User overlay: manual categories the user assigned to this entry. Not set by
    /// scanners; applied in `get_library` from `categories.json` (`storage.rs`).
    #[serde(default)]
    pub categories: Vec<String>,
}

/// Global settings configured by the user or the app itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Whether the user has completed the first-run onboarding setup.
    #[serde(default)]
    pub setup_completed: bool,
    /// Whether the app hides to the system tray when the main window is closed.
    #[serde(default = "default_minimize_to_tray")]
    pub minimize_to_tray: bool,
}

fn default_minimize_to_tray() -> bool {
    true
}

