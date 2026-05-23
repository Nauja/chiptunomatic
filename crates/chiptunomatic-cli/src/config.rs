use std::path::{Path, PathBuf};

use chiptunomatic::StemOutput;
use serde::Deserialize;

const DEFAULT_CONFIG: &str = "\
# chiptunomatic configuration
# https://github.com/Nauja/chiptunomatic

# Enabled music modes (at least one required).
# Comment out the whole block to enable all modes.
#modes:
#  - chiptune
#  - rock
#  - metal
#  - rap
#  - trap
#  - toy
#  - samba
#  - koto

# Default music mode (must be one of the enabled modes above)
#mode: chiptune

# Start with autoplay enabled (automatically advance to the next file when playback ends)
#autoplay: false

# Master volume (0.0 = silent, 1.0 = full)
#volume: 0.25

# Mute master output
#muted: false

# Per-stem settings (volume, muted, solo)
#voice:
#  volume: 1.0
#  muted: false
#  solo: false

#square:
#  volume: 1.0
#  muted: false
#  solo: false

#triangle:
#  volume: 1.0
#  muted: false
#  solo: false

#noise:
#  volume: 1.0
#  muted: false
#  solo: false

#sfx:
#  volume: 1.0
#  muted: false
#  solo: false
";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StemConfig {
    pub volume: Option<f32>,
    pub muted: Option<bool>,
    pub solo: Option<bool>,
}

impl Into<StemOutput> for StemConfig {
    fn into(self) -> StemOutput {
        StemOutput {
            volume: self.volume.unwrap_or(1.0),
            muted: self.muted.unwrap_or(false),
            solo: self.solo.unwrap_or(false),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub modes: Option<Vec<String>>,
    pub mode: Option<String>,
    pub autoplay: Option<bool>,
    pub volume: Option<f32>,
    pub muted: Option<bool>,
    pub voice: StemConfig,
    pub square: StemConfig,
    pub triangle: StemConfig,
    pub noise: StemConfig,
    pub sfx: StemConfig,
}

impl Config {
    fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "chiptunomatic")
            .map(|dirs| dirs.config_local_dir().join("config.yml"))
    }

    /// Write the default config file if it does not already exist.
    pub fn create_default_if_missing() {
        let Some(path) = Self::config_path() else {
            return;
        };
        if path.exists() {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, DEFAULT_CONFIG);
    }

    /// Load from the well-known default path, silently returning defaults on any error.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Config::default();
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Config::default();
        };
        serde_yaml::from_str(&content).unwrap_or_default()
    }

    /// Load from an explicit path, returning an error if the file cannot be read or parsed.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read config '{}': {e}", path.display()))?;
        serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("cannot parse config '{}': {e}", path.display()))
    }
}
