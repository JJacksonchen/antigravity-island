//! Customizable TOML skin engine with built-in themes and live filesystem hot-reloading.

pub mod builtins;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::config::skins_dir;

/// Visual meter style used across all quota bucket cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GaugeStyle {
    #[default]
    Ring,
    Bar,
    Segments,
    Wave,
}

/// Complete CSS color token palette for a skin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkinPalette {
    pub bg: String,
    pub bg_expanded: String,
    pub surface: String,
    pub border: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub accent_gemini: String,
    pub accent_claude: String,
    pub accent_argon: String,
    pub ok_color: String,
    pub warn_color: String,
    pub danger_color: String,
    pub track_bg: String,
}

impl Default for SkinPalette {
    fn default() -> Self {
        builtins::builtin_skins()
            .into_iter()
            .next()
            .expect("obsidian builtin exists")
            .palette
    }
}

/// Shape, corner radius, blur, and scaling parameters for a skin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkinGeometry {
    pub pill_radius_px: u16,
    pub card_radius_px: u16,
    pub border_width_px: f32,
    pub blur_px: u16,
    pub scale: f32,
}

impl Default for SkinGeometry {
    fn default() -> Self {
        Self {
            pill_radius_px: 20,
            card_radius_px: 26,
            border_width_px: 1.0,
            blur_px: 20,
            scale: 1.0,
        }
    }
}

/// Typography, animation, and gauge rendering options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkinStyle {
    pub gauge: GaugeStyle,
    pub glow: bool,
    pub glow_intensity: f32,
    pub font_family: String,
    pub mono_font: String,
    pub animations: bool,
    pub noise_overlay: bool,
}

impl Default for SkinStyle {
    fn default() -> Self {
        Self {
            gauge: GaugeStyle::Ring,
            glow: true,
            glow_intensity: 0.45,
            font_family:
                "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'PingFang SC', sans-serif"
                    .to_string(),
            mono_font: "'SF Mono', ui-monospace, Menlo, monospace".to_string(),
            animations: true,
            noise_overlay: false,
        }
    }
}

/// Full skin manifest stored as TOML and streamed to the WebView renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinManifest {
    pub id: String,
    pub name: String,
    #[serde(default = "default_author")]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(skip_serializing, default)]
    pub builtin: bool,
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub palette: SkinPalette,
    #[serde(default)]
    pub geometry: SkinGeometry,
    #[serde(default)]
    pub style: SkinStyle,
    #[serde(default)]
    pub custom_css: Option<String>,
}

fn default_author() -> String {
    "Custom".to_string()
}

impl SkinManifest {
    pub fn sanitize(&mut self) {
        self.id = sanitize_skin_id(&self.id);
        if self.name.trim().is_empty() {
            self.name = self.id.clone();
        }
        self.geometry.pill_radius_px = self.geometry.pill_radius_px.min(36);
        self.geometry.card_radius_px = self.geometry.card_radius_px.min(40);
        self.geometry.border_width_px = self.geometry.border_width_px.clamp(0.0, 4.0);
        self.geometry.blur_px = self.geometry.blur_px.min(48);
        self.geometry.scale = self.geometry.scale.clamp(0.8, 1.35);
        self.style.glow_intensity = self.style.glow_intensity.clamp(0.0, 1.5);
    }
}

/// Registry holding built-in skins merged with user TOML skins from `skins_dir()`.
#[derive(Debug, Clone)]
pub struct SkinRegistry {
    dir: PathBuf,
    skins: Vec<SkinManifest>,
}

impl SkinRegistry {
    pub fn load_default() -> Self {
        Self::load_from_dir(skins_dir())
    }

    pub fn load_from_dir(dir: PathBuf) -> Self {
        let mut reg = Self {
            dir,
            skins: Vec::new(),
        };
        reg.reload();
        reg
    }

    pub fn skins_dir(&self) -> &Path {
        &self.dir
    }

    /// Creates the user skins directory with documentation and a sample TOML skin if missing.
    pub fn ensure_initialized(&self) -> Result<()> {
        if !self.dir.exists() {
            std::fs::create_dir_all(&self.dir)?;
        }
        let readme = self.dir.join("README-SKINS.txt");
        if !readme.exists() {
            let guide = "\
Antigravity Island — Custom Skins Directory
===========================================

Drop any `.toml` file here to add or override a skin!
Changes are hot-reloaded instantly while the app is running.

You can also tweak colors, corner radius, blur, and gauge styles (`ring`, `bar`,
`segments`, `wave`) live inside the Skin Studio drawer and click Save.

See `cyberpunk-neon-sample.toml.example` or the GitHub README for all fields.
";
            let _ = std::fs::write(&readme, guide);
        }
        let sample = self.dir.join("cyberpunk-neon-sample.toml.example");
        if !sample.exists() {
            let _ = std::fs::write(&sample, SAMPLE_SKIN_TOML);
        }
        Ok(())
    }

    /// Reloads built-in skins and merges any `.toml` skin files from disk.
    pub fn reload(&mut self) {
        let mut map: HashMap<String, SkinManifest> = HashMap::new();
        let mut order: Vec<String> = Vec::new();

        for mut skin in builtins::builtin_skins() {
            skin.builtin = true;
            order.push(skin.id.clone());
            map.insert(skin.id.clone(), skin);
        }

        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|ent| ent.path()))
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
                .collect();
            paths.sort();

            for path in paths {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(mut parsed) = toml::from_str::<SkinManifest>(&raw) {
                        if parsed.id.trim().is_empty() {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                parsed.id = stem.to_string();
                            }
                        }
                        parsed.builtin = false;
                        parsed.source_path = Some(path);
                        parsed.sanitize();
                        if !map.contains_key(&parsed.id) {
                            order.push(parsed.id.clone());
                        }
                        map.insert(parsed.id.clone(), parsed);
                    }
                }
            }
        }

        self.skins = order.into_iter().filter_map(|id| map.remove(&id)).collect();
    }

    pub fn list(&self) -> &[SkinManifest] {
        &self.skins
    }

    /// Resolves a skin by ID, falling back to `"obsidian"` or the first available skin.
    pub fn resolve(&self, id: &str) -> SkinManifest {
        self.skins
            .iter()
            .find(|s| s.id == id)
            .or_else(|| self.skins.iter().find(|s| s.id == "obsidian"))
            .or_else(|| self.skins.first())
            .cloned()
            .expect("at least one builtin skin exists")
    }

    /// Saves a custom skin manifest as TOML in `skins_dir()` and reloads the registry.
    pub fn save_custom_skin(&mut self, mut skin: SkinManifest) -> Result<SkinManifest> {
        std::fs::create_dir_all(&self.dir)?;
        skin.builtin = false;
        skin.sanitize();
        let file_path = self.dir.join(format!("{}.toml", skin.id));
        let serialized = toml::to_string_pretty(&skin)?;
        let tmp = file_path.with_extension("toml.tmp");
        std::fs::write(&tmp, serialized)?;
        std::fs::rename(tmp, &file_path)?;
        skin.source_path = Some(file_path);
        self.reload();
        Ok(self.resolve(&skin.id))
    }
}

/// Starts a filesystem watcher on `dir` that invokes `on_change()` whenever `.toml` skins change.
pub fn start_skin_watcher<F>(dir: PathBuf, on_change: F) -> Option<RecommendedWatcher>
where
    F: Fn() + Send + 'static,
{
    let _ = std::fs::create_dir_all(&dir);
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if !ev.kind.is_access() {
                on_change();
            }
        }
    })
    .ok()?;
    watcher.watch(&dir, RecursiveMode::NonRecursive).ok()?;
    Some(watcher)
}

pub fn sanitize_skin_id(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch == ' ') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "custom-skin".to_string()
    } else {
        out
    }
}

pub const SAMPLE_SKIN_TOML: &str = r##"# Example custom skin for Antigravity Island.
# Rename to `<your-skin-id>.toml` inside `~/.config/antigravity-island/skins/`
# and edits will hot-reload instantaneously.

id = "midnight-berry"
name = "Midnight Berry"
author = "Community"
description = "Deep plum obsidian shell with berry-violet and mint gauges"

[palette]
bg = "rgba(18, 10, 26, 0.94)"
bg_expanded = "rgba(21, 12, 30, 0.96)"
surface = "rgba(255, 255, 255, 0.06)"
border = "rgba(232, 121, 249, 0.28)"
text_primary = "#fdf4ff"
text_secondary = "#f0abfc"
text_muted = "#a78bfa"
accent_gemini = "#34d399"
accent_claude = "#f472b6"
accent_argon = "#c084fc"
ok_color = "#34d399"
warn_color = "#fbbf24"
danger_color = "#fb7185"
track_bg = "rgba(255, 255, 255, 0.10)"

[geometry]
pill_radius_px = 20
card_radius_px = 24
border_width_px = 1.2
blur_px = 22
scale = 1.0

[style]
gauge = "wave" # "ring" | "bar" | "segments" | "wave"
glow = true
glow_intensity = 0.65
font_family = "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'PingFang SC', sans-serif"
mono_font = "'SF Mono', ui-monospace, Menlo, monospace"
animations = true
noise_overlay = false

custom_css = """
.island-shell {
  background-image: radial-gradient(circle at 80% 0%, rgba(232,121,249,0.16), transparent 55%);
}
"""
"##;
