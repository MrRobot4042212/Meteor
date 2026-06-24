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
    /// In-game metrics overlay configuration.
    #[serde(default)]
    pub overlay: OverlaySettings,
}

fn default_minimize_to_tray() -> bool {
    true
}

/// Configuration for the in-game performance/telemetry overlay. Persisted as part
/// of `AppSettings`; applied live by `metrics.rs` and read by the overlay window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlaySettings {
    /// Master switch. When off, the sampler idles and the overlay window stays hidden.
    #[serde(default)]
    pub enabled: bool,
    /// Corner of the screen: "top-left" | "top-right" | "bottom-left" | "bottom-right".
    #[serde(default = "default_overlay_position")]
    pub position: String,
    /// Sampling/emit interval in milliseconds.
    #[serde(default = "default_overlay_interval")]
    pub interval_ms: u64,
    #[serde(default = "yes")]
    pub show_fps: bool,
    #[serde(default = "yes")]
    pub show_frametime: bool,
    #[serde(default = "yes")]
    pub show_gpu: bool,
    #[serde(default = "yes")]
    pub show_gpu_temp: bool,
    /// CPU temperature (needs the LHM sidecar + admin). Off by default since it
    /// requires elevation; users opt in.
    #[serde(default)]
    pub show_cpu_temp: bool,
    #[serde(default = "yes")]
    pub show_vram: bool,
    #[serde(default = "yes")]
    pub show_cpu: bool,
    #[serde(default = "yes")]
    pub show_ram: bool,
    /// Which GPU to sample: "auto" | "nvml:<i>" | "adlx:<i>".
    #[serde(default = "default_overlay_gpu")]
    pub gpu: String,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            position: default_overlay_position(),
            interval_ms: default_overlay_interval(),
            show_fps: true,
            show_frametime: true,
            show_gpu: true,
            show_gpu_temp: true,
            show_cpu_temp: false,
            show_vram: true,
            show_cpu: true,
            show_ram: true,
            gpu: default_overlay_gpu(),
        }
    }
}

fn default_overlay_position() -> String {
    "top-left".to_string()
}

fn default_overlay_gpu() -> String {
    "auto".to_string()
}

fn default_overlay_interval() -> u64 {
    1000
}

fn yes() -> bool {
    true
}

