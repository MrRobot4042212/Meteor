use serde::{Deserialize, Serialize};

/// Where a library entry comes from. New sources (Epic, GOG, Xbox, registry
/// apps...) should be added here and handled in `steam.rs`-style modules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameSource {
    Steam,
    Manual,
}

/// A single launchable entry in the unified library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    /// Stable unique id, e.g. "steam:440" or "manual:1718900000000".
    pub id: String,
    pub name: String,
    pub source: GameSource,

    /// Steam AppID (only for `GameSource::Steam`).
    #[serde(default)]
    pub app_id: Option<u32>,

    /// Absolute path to the executable (only for `GameSource::Manual`).
    #[serde(default)]
    pub executable: Option<String>,

    /// Install directory on disk, when known.
    #[serde(default)]
    pub install_dir: Option<String>,

    /// Remote cover image URL. For Steam this is the vertical capsule.
    #[serde(default)]
    pub cover_url: Option<String>,
}
