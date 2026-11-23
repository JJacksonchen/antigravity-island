//! Native macOS / Windows / Linux tray status item with live at-a-glance quota indicator.
//!
//! - On macOS: renders a crisp Antigravity "A" arch template icon; avoids visual duplication while the floating
//!   Island is visible (unless `always_show_menubar_text` is enabled) and automatically shows live
//!   quota text (`G 65%/64% · C 100%`) whenever the Island is hidden.
//! - On Windows & Linux: dynamically renders a color-coded circular progress ring icon reflecting
//!   the live remaining quota percentage, paired with a rich multi-line hover tooltip. Left-click
//!   on Windows directly toggles Show / Hide of the floating Island.

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
    ResetPosition,
    RefreshNow,
    SetMode(DisplayMode),
    SetPillDensity(PillDensity),
    ToggleMenuBarText,
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
            let icon = build_antigravity_template_icon()?;
            let title = tray_title_from_state(config, state);
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
                .with_menu_on_left_click(false)
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
            let title = tray_title_from_state(config, state);
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
fn tray_title_from_state(config: &AppConfig, state: &SyncState) -> String {
    // Keep the macOS menu bar clean (icon-only) while the Dynamic Island is visible so it never
    // duplicates the Island directly below it; automatically display live quota text when hidden.
    if config.visible && !config.always_show_menubar_text {
        return String::new();
    }
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

    let reset_pos = MenuItem::new("Reset Position (Top Center)", true, None);
    actions.insert(reset_pos.id().clone(), TrayAction::ResetPosition);
    let _ = menu.append(&reset_pos);

    let refresh_item = MenuItem::new("Refresh Quota Now", true, None);
    actions.insert(refresh_item.id().clone(), TrayAction::RefreshNow);
    let _ = menu.append(&refresh_item);

    let _ = menu.append(&PredefinedMenuItem::separator());

    // Display mode submenu
    let mode_menu = Submenu::new("Display Mode", true);
    let island_item = CheckMenuItem::new(
        "Dynamic Island (Hover Expand · Draggable)",
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
        "Desktop Widget (Click Expand · Draggable)",
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
        ("Glance (5h + Weekly + Countdown)", PillDensity::Glance),
        ("Compact (Minimal Pill)", PillDensity::Compact),
    ] {
        let item = CheckMenuItem::new(label, true, config.pill_density == density, None);
        actions.insert(item.id().clone(), TrayAction::SetPillDensity(density));
        let _ = density_menu.append(&item);
    }
    let _ = menu.append(&density_menu);

    #[cfg(target_os = "macos")]
    {
        let menubar_text_item = CheckMenuItem::new(
            "Always Show Menu Bar Quota Text",
            true,
            config.always_show_menubar_text,
            None,
        );
        actions.insert(
            menubar_text_item.id().clone(),
            TrayAction::ToggleMenuBarText,
        );
        let _ = menu.append(&menubar_text_item);
    }

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

/// Samples the authentic Antigravity "A" arch Bezier contour in normalized 0..24 coordinate space.
fn antigravity_logo_polygon() -> Vec<(f32, f32)> {
    type Pt = (f32, f32);
    type CubicSeg = (Pt, Pt, Pt, Pt);

    const SEGMENTS: [CubicSeg; 10] = [
        ((12.0, 2.8), (14.8, 2.8), (16.3, 6.2), (17.6, 11.8)),
        ((17.6, 11.8), (18.5, 15.7), (19.9, 18.2), (21.6, 19.9)),
        ((21.6, 19.9), (22.2, 20.5), (21.8, 21.4), (20.8, 21.4)),
        ((20.8, 21.4), (19.6, 21.4), (18.3, 20.1), (17.0, 18.0)),
        ((17.0, 18.0), (15.5, 15.5), (14.4, 12.6), (12.0, 12.6)),
        ((12.0, 12.6), (9.6, 12.6), (8.5, 15.5), (7.0, 18.0)),
        ((7.0, 18.0), (5.7, 20.1), (4.4, 21.4), (3.2, 21.4)),
        ((3.2, 21.4), (2.2, 21.4), (1.8, 20.5), (2.4, 19.9)),
        ((2.4, 19.9), (4.1, 18.2), (5.5, 15.7), (6.4, 11.8)),
        ((6.4, 11.8), (7.7, 6.2), (9.2, 2.8), (12.0, 2.8)),
    ];

    const STEPS: usize = 10;
    let mut poly = Vec::with_capacity(SEGMENTS.len() * STEPS);
    for (p0, p1, p2, p3) in SEGMENTS {
        for i in 0..STEPS {
            let t = (i as f32) / (STEPS as f32);
            let u = 1.0 - t;
            let x = u * u * u * p0.0
                + 3.0 * u * u * t * p1.0
                + 3.0 * u * t * t * p2.0
                + t * t * t * p3.0;
            let y = u * u * u * p0.1
                + 3.0 * u * u * t * p1.1
                + 3.0 * u * t * t * p2.1
                + t * t * t * p3.1;
            poly.push((x, y));
        }
    }
    poly
}

fn point_in_polygon(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % n];
        if ((y1 > y) != (y2 > y)) && (x < (x2 - x1) * (y - y1) / (y2 - y1) + x1) {
            inside = !inside;
        }
    }
    inside
}

/// 4x4 supersampled coverage for the Antigravity "A" arch mapped around `(cx, cy)` with span `box_size`.
fn antigravity_logo_coverage(
    px: f32,
    py: f32,
    cx: f32,
    cy: f32,
    box_size: f32,
    poly: &[(f32, f32)],
) -> f32 {
    const OFFSETS: [f32; 4] = [0.125, 0.375, 0.625, 0.875];
    let mut hits = 0u32;
    for &oy in &OFFSETS {
        for &ox in &OFFSETS {
            let u = 12.0 + ((px + ox - cx) / box_size) * 24.0;
            let v = 12.0 + ((py + oy - cy) / box_size) * 24.0;
            if point_in_polygon(u, v, poly) {
                hits += 1;
            }
        }
    }
    (hits as f32) / 16.0
}

/// Procedurally renders a crisp 32x32 anti-aliased Antigravity "A" arch template icon for macOS.
#[cfg(target_os = "macos")]
fn build_antigravity_template_icon() -> anyhow::Result<Icon> {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let poly = antigravity_logo_polygon();
    let center = (size as f32) * 0.5;

    for y in 0..size {
        for x in 0..size {
            let alpha = antigravity_logo_coverage(x as f32, y as f32, center, center, 25.5, &poly);
            let idx = (y * size + x) * 4;
            rgba[idx] = 0;
            rgba[idx + 1] = 0;
            rgba[idx + 2] = 0;
            rgba[idx + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    Ok(Icon::from_rgba(rgba, size as u32, size as u32)?)
}

/// Renders a live 32x32 colored circular progress gauge icon with the Antigravity "A" arch emblem
/// for Windows & Linux system trays so users can gauge remaining quota with a single glance.
#[cfg(not(target_os = "macos"))]
fn build_live_quota_ring_icon(config: &AppConfig, state: &SyncState) -> anyhow::Result<Icon> {
    let pct_opt = state
        .snapshot
        .as_ref()
        .and_then(|s| s.min_remaining_percent());
    let (r, g, b) = match pct_opt {
        None => (148u8, 163u8, 184u8),
        Some(p) if p <= config.danger_threshold => (251u8, 113u8, 133u8),
        Some(p) if p <= config.warn_threshold => (251u8, 191u8, 36u8),
        Some(p) if p >= 75 => (52u8, 211u8, 153u8),
        Some(_) => (56u8, 189u8, 248u8),
    };
    let fill_frac = pct_opt.map(|p| (p as f32) / 100.0).unwrap_or(0.0);

    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let center = (size as f32 - 1.0) * 0.5;
    let poly = antigravity_logo_polygon();

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let disc_alpha = (15.2 - dist).clamp(0.0, 1.0) * 0.88;
            let mut out_r = 14.0f32;
            let mut out_g = 17.0f32;
            let mut out_b = 24.0f32;
            let mut out_a = disc_alpha;

            let ring_cov = (1.9 - (dist - 12.6).abs()).clamp(0.0, 1.0);
            if ring_cov > 0.0 {
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

            let logo_cov = antigravity_logo_coverage(
                x as f32,
                y as f32,
                center + 0.5,
                center + 0.8,
                15.0,
                &poly,
            );
            if logo_cov > 0.0 {
                out_r = 248.0 * logo_cov + out_r * (1.0 - logo_cov);
                out_g = 250.0 * logo_cov + out_g * (1.0 - logo_cov);
                out_b = 252.0 * logo_cov + out_b * (1.0 - logo_cov);
                out_a = out_a.max(logo_cov);
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
