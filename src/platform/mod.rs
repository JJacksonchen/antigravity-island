//! Cross-platform window chrome, screen geometry, notification, and shell helpers.

#[cfg(target_os = "macos")]
pub mod macos;

use std::path::Path;

use serde::{Deserialize, Serialize};
use tao::window::Window;

use crate::config::DisplayMode;

/// Primary screen geometry used to anchor the Dynamic Island or clamp floating widgets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScreenMetrics {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub menu_bar_height: f64,
    pub has_notch: bool,
    pub notch_width: f64,
}

impl Default for ScreenMetrics {
    fn default() -> Self {
        Self {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1440.0,
            height: 900.0,
            menu_bar_height: 25.0,
            has_notch: false,
            notch_width: 0.0,
        }
    }
}

/// Applies native transparent floating island/widget properties to the `tao` window.
pub fn configure_window(window: &Window, mode: DisplayMode) {
    #[cfg(target_os = "macos")]
    macos::configure_window(window, mode);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, mode);
    }
}

/// Updates the window level dynamically when switching between Island / NotchHug and Widget mode.
pub fn update_window_mode(window: &Window, mode: DisplayMode) {
    #[cfg(target_os = "macos")]
    macos::update_window_mode(window, mode);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, mode);
    }
}

/// Returns geometry metrics for the primary display (including MacBook camera notch dimensions).
pub fn primary_screen_metrics(window: Option<&Window>) -> ScreenMetrics {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        macos::primary_screen_metrics()
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = window {
            if let Some(m) = w.primary_monitor().or_else(|| w.current_monitor()) {
                let scale = m.scale_factor();
                let size = m.size().to_logical::<f64>(scale);
                let pos = m.position().to_logical::<f64>(scale);
                return ScreenMetrics {
                    origin_x: pos.x,
                    origin_y: pos.y,
                    width: size.width,
                    height: size.height,
                    menu_bar_height: 28.0,
                    has_notch: false,
                    notch_width: 0.0,
                };
            }
        }
        ScreenMetrics::default()
    }
}

/// Sends a lightweight native desktop notification without blocking the caller.
pub fn send_notification(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                esc(&body),
                esc(&title)
            );
            let _ = std::process::Command::new("/usr/bin/osascript")
                .args(["-e", &script])
                .output();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send")
                .args([&title, &body])
                .output();
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (title, body);
        }
    });
}

/// Opens a folder or file in the system file manager.
pub fn open_in_file_manager(path: &Path) {
    let target = path.to_path_buf();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("/usr/bin/open")
                .arg(&target)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&target).spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(&target).spawn();
        }
    });
}
