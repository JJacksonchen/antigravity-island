//! Application configuration and standard user paths (`~/.config/antigravity-island`).

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Visual display mode for the floating quota surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    /// Apple-style Dynamic Island floating pill anchored at top-center.
    #[default]
    Island,
    /// Winged layout that hugs left & right of the MacBook camera notch.
    NotchHug,
    /// Freely draggable desktop floating widget anywhere on screen.
    Widget,
}

/// UI localization preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Auto,
    Zh,
    En,
}

/// Persisted user settings (`~/.config/antigravity-island/config.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub skin_id: String,
    pub mode: DisplayMode,
    pub poll_interval_secs: u64,
    pub language: Language,
    pub warn_threshold: u8,
    pub danger_threshold: u8,
    pub notifications: bool,
    pub pinned_expanded: bool,
    pub visible: bool,
    pub widget_x: Option<f64>,
    pub widget_y: Option<f64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            skin_id: "obsidian".to_string(),
            mode: DisplayMode::Island,
            poll_interval_secs: 15,
            language: Language::Auto,
            warn_threshold: 30,
            danger_threshold: 10,
            notifications: true,
            pinned_expanded: false,
            visible: true,
            widget_x: None,
            widget_y: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        Self::load_from_path(&config_file_path())
    }

    pub fn load_from_path(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let mut cfg: Self = toml::from_str(&raw).unwrap_or_default();
        cfg.sanitize();
        cfg
    }

    pub fn save(&self) -> Result<()> {
        self.save_to_path(&config_file_path())
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut copy = self.clone();
        copy.sanitize();
        let text = toml::to_string_pretty(&copy)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn sanitize(&mut self) {
        self.poll_interval_secs = self.poll_interval_secs.clamp(5, 300);
        self.warn_threshold = self.warn_threshold.clamp(5, 95);
        self.danger_threshold = self.danger_threshold.clamp(1, self.warn_threshold);
        if self.skin_id.trim().is_empty() {
            self.skin_id = "obsidian".to_string();
        }
    }
}

/// Base configuration directory (`~/.config/antigravity-island`).
pub fn config_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("ANTIGRAVITY_ISLAND_CONFIG_DIR") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("antigravity-island")
}

pub fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn skins_dir() -> PathBuf {
    config_dir().join("skins")
}

pub fn live_statusline_path() -> PathBuf {
    config_dir().join("live-statusline.json")
}
