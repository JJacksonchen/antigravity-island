//! Native macOS/desktop menu bar tray status item with live quota badge and quick menu.

use std::collections::HashMap;

use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::{AppConfig, DisplayMode};
use crate::quota::SyncState;
use crate::skin::SkinManifest;

/// Actions triggered by clicking items in the tray context menu.
#[derive(Debug, Clone)]
pub enum TrayAction {
    ToggleVisible,
    ToggleExpand,
    RefreshNow,
    SetMode(DisplayMode),
    SelectSkin(String),
    ToggleDemo,
    OpenSkinsDir,
    Quit,
}

/// Manages the OS status bar icon, live compact quota title, and dynamic submenu state.
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
        let icon = build_orbital_icon()?;
        let (menu, actions) = build_menu(config, skins, state);

        let title = tray_title_from_state(state);
        let tray = TrayIconBuilder::new()
            .with_icon_templated(icon)
            .with_title(&title)
            .with_tooltip("Antigravity Island — Real-time Quota Monitor")
            .with_menu(Box::new(menu))
            .build()?;

        Ok(Self { tray, actions })
    }

    /// Updates the menu bar title and rebuilds menu checkmarks when quota, config, or skins change.
    pub fn sync_state(&mut self, config: &AppConfig, skins: &[SkinManifest], state: &SyncState) {
        let title = tray_title_from_state(state);
        self.tray.set_title(Some(&title));
        let (menu, actions) = build_menu(config, skins, state);
        self.tray.set_menu(Some(Box::new(menu)));
        self.actions = actions;
    }

    /// Resolves a menu click event to its configured `TrayAction`.
    pub fn action_for(&self, event: &MenuEvent) -> Option<TrayAction> {
        self.actions.get(event.id()).cloned()
    }
}

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
            format!("{} · {}", plan, snap.source.label())
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
    for (label, mode) in [
        ("Dynamic Island (Top Floating)", DisplayMode::Island),
        ("Notch Hug (Camera Notch)", DisplayMode::NotchHug),
        ("Desktop Widget (Draggable)", DisplayMode::Widget),
    ] {
        let item = CheckMenuItem::new(label, true, config.mode == mode, None);
        actions.insert(item.id().clone(), TrayAction::SetMode(mode));
        let _ = mode_menu.append(&item);
    }
    let _ = menu.append(&mode_menu);

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

/// Procedurally renders a crisp 32x32 anti-aliased Antigravity orbital template icon in pure Rust.
fn build_orbital_icon() -> anyhow::Result<Icon> {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let center = (size as f32 - 1.0) * 0.5;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            // Outer orbital ring (radius ~11.0, thickness ~2.2) with slight gap at top-right.
            let ring_diff = (dist - 10.8).abs();
            let mut alpha = (1.35 - ring_diff).clamp(0.0, 1.0);

            // Inner satellite dot offset slightly toward upper-right like an orbital icon.
            let sx = dx - 4.2;
            let sy = dy + 4.2;
            let sdist = (sx * sx + sy * sy).sqrt();
            let dot_alpha = (3.1 - sdist).clamp(0.0, 1.0);

            // Center core dot.
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
