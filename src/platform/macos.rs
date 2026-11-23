//! macOS-specific AppKit window styling, notch geometry detection, and Accessory mode setup.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSColor, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWindowLevel};
use tao::platform::macos::WindowExtMacOS;
use tao::window::Window;

use super::ScreenMetrics;
use crate::config::DisplayMode;

/// Window level above the macOS menu bar — used ONLY in explicit NotchHug mode.
const NOTCH_HUG_WINDOW_LEVEL: NSWindowLevel = 27;
/// Floating overlay level above all normal app windows without overlapping the macOS menu bar.
const FLOATING_WINDOW_LEVEL: NSWindowLevel = 3;

/// Configures the `tao` window as a transparent, shadow-free, all-Spaces floating surface.
pub fn configure_window(window: &Window, mode: DisplayMode) {
    window.set_has_shadow(false);
    window.set_titlebar_transparent(true);
    window.set_fullsize_content_view(true);

    let ns_win_ptr = window.ns_window() as *mut NSWindow;
    if ns_win_ptr.is_null() {
        return;
    }

    // SAFETY: called on the main AppKit thread with a live NSWindow owned by `tao::window::Window`.
    unsafe {
        let ns_win: &NSWindow = &*ns_win_ptr;
        ns_win.setOpaque(false);
        let clear = NSColor::clearColor();
        ns_win.setBackgroundColor(Some(&clear));
        ns_win.setHasShadow(false);
        ns_win.setAcceptsMouseMovedEvents(true);
        ns_win.setIgnoresMouseEvents(false);
        ns_win.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_win.setLevel(window_level_for_mode(mode));
    }
}

/// Updates the `NSWindow` z-order level when switching between Island / NotchHug and Widget modes.
pub fn update_window_mode(window: &Window, mode: DisplayMode) {
    let ns_win_ptr = window.ns_window() as *mut NSWindow;
    if ns_win_ptr.is_null() {
        return;
    }
    unsafe {
        let ns_win: &NSWindow = &*ns_win_ptr;
        ns_win.setLevel(window_level_for_mode(mode));
    }
}

fn window_level_for_mode(mode: DisplayMode) -> NSWindowLevel {
    match mode {
        DisplayMode::NotchHug => NOTCH_HUG_WINDOW_LEVEL,
        DisplayMode::Island | DisplayMode::Widget => FLOATING_WINDOW_LEVEL,
    }
}

/// Reads primary display dimensions and MacBook camera notch geometry from AppKit `NSScreen`.
pub fn primary_screen_metrics() -> ScreenMetrics {
    let Some(mtm) = MainThreadMarker::new() else {
        return ScreenMetrics::default();
    };
    let screens = NSScreen::screens(mtm);
    let Some(screen) = screens.firstObject().or_else(|| NSScreen::mainScreen(mtm)) else {
        return ScreenMetrics::default();
    };

    let frame = screen.frame();
    let visible = screen.visibleFrame();
    let top_inset = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);

    let aux_left = screen.auxiliaryTopLeftArea();
    let aux_right = screen.auxiliaryTopRightArea();
    let has_notch = aux_left.size.width > 0.0 && aux_right.size.width > 0.0;
    let notch_width = if has_notch {
        (aux_right.origin.x - (aux_left.origin.x + aux_left.size.width)).max(0.0)
    } else {
        0.0
    };
    let menu_bar_height = if has_notch && aux_left.size.height > 0.0 {
        aux_left.size.height
    } else if top_inset > 0.0 {
        top_inset
    } else {
        25.0
    };

    ScreenMetrics {
        origin_x: 0.0,
        origin_y: 0.0,
        width: frame.size.width,
        height: frame.size.height,
        menu_bar_height,
        has_notch,
        notch_width,
    }
}
