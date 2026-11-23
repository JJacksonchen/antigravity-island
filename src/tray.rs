//! Native macOS / Windows / Linux tray status item with live at-a-glance quota indicator.
//!
//! - On macOS: renders a crisp template orbital icon alongside live menu bar text (`G 76%/67% · C 100%`).
//! - On Windows & Linux: dynamically renders a color-coded circular progress ring icon reflecting
//!   the live remaining quota percentage, paired with a rich multi-line hover tooltip.

use std::collections::HashMap;

use chrono::Utc;
use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::{AppConfig, DisplayMode, PillDensity};
use crate::quota::SyncState;
use crate::skin::SkinManifest;

/// Actions triggered by clicking items in the tray context menu.
#[derive(Debug, Clone)]
pub enum TrayAction {
    ToggleVisible,
    ToggleExpand,
    RefreshNow,
    SetMode(DisplayMode),
    SetPillDensity(PillDensity),
    SelectSkin(String),
    ToggleDemo,
    OpenSkinsDir,
    Quit,
}

/// Manages the OS tray/menu bar icon, live at-a-glance indicator, and dynamic context menu.
pub struct TrayController {
    tray: TrayIcon,
    actions: HashMap<MenuId, TrayAction>,
}

impl TrayController {
    pub fn new(
        config: &AppConfig,
        skins: &[SkinManifest],
        state: &SyncState,
    ) -> anyhow::Result<Self> {
        let (menu, actions) = build_menu(config, skins, state);
        let tooltip = tray_tooltip_from_state(state);

        #[cfg(target_os = "macos")]
        let tray = {
            let icon = build_orbital_template_icon()?;
            let title = tray_title_from_state(state);
            TrayIconBuilder::new()
                .with_icon_templated(icon)
                .with_title(&title)
                .with_tooltip(&tooltip)
                .with_menu(Box::new(menu))
                .build()?
        };

        #[cfg(not(target_os = "macos"))]
        let tray = {
            let icon = build_live_quota_ring_icon(config, state)?;
            TrayIconBuilder::new()
                .with_icon(icon)
                .with_tooltip(&tooltip)
                .with_menu(Box::new(menu))
                .build()?
        };

        Ok(Self { tray, actions })
    }

    /// Updates tray icon/title/tooltip and rebuilds menu checkmarks when quota, config, or skins change.
    pub fn sync_state(&mut self, config: &AppConfig, skins: &[SkinManifest], state: &SyncState) {
        let tooltip = tray_tooltip_from_state(state);
        let _ = self.tray.set_tooltip(Some(&tooltip));

        #[cfg(target_os = "macos")]
        {
            let title = tray_title_from_state(state);
            self.tray.set_title(Some(&title));
        }

        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(icon) = build_live_quota_ring_icon(config, state) {
                let _ = self.tray.set_icon(Some(icon));
            }
        }

        let (menu, actions) = build_menu(config, skins, state);
        self.tray.set_menu(Some(Box::new(menu)));
        self.actions = actions;
    }

    /// Resolves a menu click event to its configured `TrayAction`.
    pub fn action_for(&self, event: &MenuEvent) -> Option<TrayAction> {
        self.actions.get(event.id()).cloned()
    }
}

#[cfg(target_os = "macos")]
fn tray_title_from_state(state: &SyncState) -> String {
    match &state.snapshot {
        Some(snap) => {
            let prefix = if state.demo_mode { "DEMO " } else { "" };
            format!("{prefix}{}", snap.tray_summary())
        }
        None if state.syncing => "AGY ...".to_string(),
        None => "AGY Off".to_string(),
    }
}

fn tray_tooltip_from_state(state: &SyncState) -> String {
    match &state.snapshot {
        Some(snap) => snap.tray_tooltip(Utc::now()),
        None if state.syncing => "Antigravity Island — Syncing quota...".to_string(),
        None => "Antigravity Island — Waiting for Antigravity".to_string(),
    }
}

fn build_menu(
    config: &AppConfig,
    skins: &[SkinManifest],
    state: &SyncState,
) -> (Menu, HashMap<MenuId, TrayAction>) {
    let menu = Menu::new();
    let mut actions = HashMap::new();

    let status_text = match &state.snapshot {
        Some(snap) => {
            let plan = snap.account_plan.as_deref().unwrap_or("Antigravity");
            format!("{} · {}", plan, snap.tray_summary())
        }
        None => state
            .last_error
            .as_deref()
            .map(|e| {
                let short: String = e.chars().take(42).collect();
                format!("Offline: {short}")
            })
            .unwrap_or_else(|| "Waiting for Antigravity...".to_string()),
    };
    let status_item = MenuItem::new(status_text, false, None);
    let _ = menu.append(&status_item);
    let _ = menu.append(&PredefinedMenuItem::separator());

    let visible_label = if config.visible {
        "Hide Island / Widget"
    } else {
        "Show Island / Widget"
    };
    let toggle_visible = MenuItem::new(visible_label, true, None);
    actions.insert(toggle_visible.id().clone(), TrayAction::ToggleVisible);
    let _ = menu.append(&toggle_visible);

    let expand_label = if config.pinned_expanded {
        "Collapse Pill"
    } else {
        "Pin Expanded Card"
    };
    let toggle_expand = MenuItem::new(expand_label, true, None);
    actions.insert(toggle_expand.id().clone(), TrayAction::ToggleExpand);
    let _ = menu.append(&toggle_expand);

    let refresh_item = MenuItem::new("Refresh Quota Now", true, None);
    actions.insert(refresh_item.id().clone(), TrayAction::RefreshNow);
    let _ = menu.append(&refresh_item);

    let _ = menu.append(&PredefinedMenuItem::separator());

    // Display mode submenu
    let mode_menu = Submenu::new("Display Mode", true);
    let island_item = CheckMenuItem::new(
        "Dynamic Island (Top Floating)",
        true,
        config.mode == DisplayMode::Island,
        None,
    );
    actions.insert(
        island_item.id().clone(),
        TrayAction::SetMode(DisplayMode::Island),
    );
    let _ = mode_menu.append(&island_item);

    #[cfg(target_os = "macos")]
    {
        let notch_item = CheckMenuItem::new(
            "Notch Hug (Camera Notch)",
            true,
            config.mode == DisplayMode::NotchHug,
            None,
        );
        actions.insert(
            notch_item.id().clone(),
            TrayAction::SetMode(DisplayMode::NotchHug),
        );
        let _ = mode_menu.append(&notch_item);
    }

    let widget_item = CheckMenuItem::new(
        "Desktop Widget (Draggable)",
        true,
        config.mode == DisplayMode::Widget,
        None,
    );
    actions.insert(
        widget_item.id().clone(),
        TrayAction::SetMode(DisplayMode::Widget),
    );
    let _ = mode_menu.append(&widget_item);
    let _ = menu.append(&mode_menu);

    // Pill density submenu
    let density_menu = Submenu::new("Pill Style", true);
    for (label, density) in [
        ("Glance (At-a-Glance Rings & Timers)", PillDensity::Glance),
        ("Compact (Minimal Pill)", PillDensity::Compact),
    ] {
        let item = CheckMenuItem::new(label, true, config.pill_density == density, None);
        actions.insert(item.id().clone(), TrayAction::SetPillDensity(density));
        let _ = density_menu.append(&item);
    }
    let _ = menu.append(&density_menu);

    // Skins submenu
    let skin_menu = Submenu::new("Skins", true);
    for skin in skins {
        let label = if skin.builtin {
            skin.name.clone()
        } else {
            format!("{} (Custom)", skin.name)
        };
        let item = CheckMenuItem::new(label, true, config.skin_id == skin.id, None);
        actions.insert(item.id().clone(), TrayAction::SelectSkin(skin.id.clone()));
        let _ = skin_menu.append(&item);
    }
    let _ = skin_menu.append(&PredefinedMenuItem::separator());
    let open_skins = MenuItem::new("Open Skins Folder...", true, None);
    actions.insert(open_skins.id().clone(), TrayAction::OpenSkinsDir);
    let _ = skin_menu.append(&open_skins);
    let _ = menu.append(&skin_menu);

    let demo_item = CheckMenuItem::new("Demo Mode (Simulated Quota)", true, state.demo_mode, None);
    actions.insert(demo_item.id().clone(), TrayAction::ToggleDemo);
    let _ = menu.append(&demo_item);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let quit_item = MenuItem::new("Quit Antigravity Island", true, None);
    actions.insert(quit_item.id().clone(), TrayAction::Quit);
    let _ = menu.append(&quit_item);

    (menu, actions)
}

/// Procedurally renders a crisp 32x32 anti-aliased Antigravity orbital template icon for macOS.
#[cfg(target_os = "macos")]
fn build_orbital_template_icon() -> anyhow::Result<Icon> {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let center = (size as f32 - 1.0) * 0.5;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let ring_diff = (dist - 10.8).abs();
            let mut alpha = (1.35 - ring_diff).clamp(0.0, 1.0);

            let sx = dx - 4.2;
            let sy = dy + 4.2;
            let sdist = (sx * sx + sy * sy).sqrt();
            let dot_alpha = (3.1 - sdist).clamp(0.0, 1.0);

            let core_alpha = (3.4 - dist).clamp(0.0, 1.0);

            alpha = alpha.max(dot_alpha).max(core_alpha);
            let idx = (y * size + x) * 4;
            rgba[idx] = 0;
            rgba[idx + 1] = 0;
            rgba[idx + 2] = 0;
            rgba[idx + 3] = (alpha * 255.0).round() as u8;
        }
    }

    Ok(Icon::from_rgba(rgba, size as u32, size as u32)?)
}

/// Renders a live 32x32 colored circular progress gauge icon for Windows & Linux system trays
/// so users can gauge remaining quota with a single glance at the taskbar.
#[cfg(not(target_os = "macos"))]
fn build_live_quota_ring_icon(config: &AppConfig, state: &SyncState) -> anyhow::Result<Icon> {
    let pct_opt = state
        .snapshot
        .as_ref()
        .and_then(|s| s.min_remaining_percent());
    let (r, g, b) = match pct_opt {
        None => (148u8, 163u8, 184u8), // slate offline
        Some(p) if p <= config.danger_threshold => (251u8, 113u8, 133u8), // coral red
        Some(p) if p <= config.warn_threshold => (251u8, 191u8, 36u8), // amber
        Some(p) if p >= 75 => (52u8, 211u8, 153u8), // emerald
        Some(_) => (56u8, 189u8, 248u8), // sky blue
    };
    let fill_frac = pct_opt.map(|p| (p as f32) / 100.0).unwrap_or(0.0);

    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let center = (size as f32 - 1.0) * 0.5;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            // Subtle dark translucent disc behind ring for high contrast on both light/dark Windows taskbars.
            let disc_alpha = (14.8 - dist).clamp(0.0, 1.0) * 0.82;
            let mut out_r = 14.0f32;
            let mut out_g = 17.0f32;
            let mut out_b = 24.0f32;
            let mut out_a = disc_alpha;

            // Progress ring centered at radius 10.6, stroke width ~3.4px.
            let ring_cov = (2.1 - (dist - 10.6).abs()).clamp(0.0, 1.0);
            if ring_cov > 0.0 {
                // Clockwise angle starting from 12 o'clock (0.0..1.0).
                let angle =
                    (dx.atan2(-dy).rem_euclid(std::f32::consts::TAU)) / std::f32::consts::TAU;
                let filled = pct_opt.is_some() && angle <= fill_frac.max(0.03);
                let (rr, gg, bb, ra) = if filled {
                    (r as f32, g as f32, b as f32, ring_cov)
                } else {
                    (255.0, 255.0, 255.0, ring_cov * 0.22)
                };
                out_r = rr * ra + out_r * (1.0 - ra);
                out_g = gg * ra + out_g * (1.0 - ra);
                out_b = bb * ra + out_b * (1.0 - ra);
                out_a = out_a.max(ra);
            }

            // Center glowing status dot.
            let dot_cov = (3.4 - dist).clamp(0.0, 1.0);
            if dot_cov > 0.0 {
                out_r = (r as f32) * dot_cov + out_r * (1.0 - dot_cov);
                out_g = (g as f32) * dot_cov + out_g * (1.0 - dot_cov);
                out_b = (b as f32) * dot_cov + out_b * (1.0 - dot_cov);
                out_a = out_a.max(dot_cov);
            }

            let idx = (y * size + x) * 4;
            rgba[idx] = out_r.round().clamp(0.0, 255.0) as u8;
            rgba[idx + 1] = out_g.round().clamp(0.0, 255.0) as u8;
            rgba[idx + 2] = out_b.round().clamp(0.0, 255.0) as u8;
            rgba[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    Ok(Icon::from_rgba(rgba, size as u32, size as u32)?)
}
