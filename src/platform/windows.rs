//! Windows-specific Win32 window chrome, taskbar/Alt-Tab hiding, console attach, and monitor metrics.

use std::os::windows::process::CommandExt;

use tao::platform::windows::WindowExtWindows;
use tao::window::Window;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_TOOLWINDOW,
};

use super::ScreenMetrics;
use crate::config::DisplayMode;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Attaches stdout/stderr to the invoking PowerShell/cmd console when launched with CLI flags
/// (while keeping GUI launches completely free of a black console window).
pub fn attach_parent_console_if_cli() {
    let is_cli = std::env::args().skip(1).any(|arg| {
        matches!(
            arg.as_str(),
            "--probe"
                | "--json"
                | "--list-skins"
                | "--help"
                | "-h"
                | "--version"
                | "-V"
                | "ingest-statusline"
                | "help"
        )
    });
    if is_cli {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// Configures the `tao` window as a borderless, transparent, always-on-top tool window that stays
/// out of the Windows taskbar and Alt-Tab switcher.
pub fn configure_window(window: &Window, _mode: DisplayMode) {
    let _ = window.set_skip_taskbar(true);
    window.set_undecorated_shadow(false);
    apply_toolwindow_topmost(window);
}

pub fn update_window_mode(window: &Window, _mode: DisplayMode) {
    apply_toolwindow_topmost(window);
}

fn apply_toolwindow_topmost(window: &Window) {
    let hwnd = window.hwnd() as HWND;
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | (WS_EX_TOOLWINDOW as isize));
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Reads primary Windows monitor dimensions in logical pixels (no top menu bar inset).
pub fn primary_screen_metrics(window: Option<&Window>) -> ScreenMetrics {
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
                menu_bar_height: 0.0,
                has_notch: false,
                notch_width: 0.0,
            };
        }
    }
    ScreenMetrics {
        menu_bar_height: 0.0,
        ..ScreenMetrics::default()
    }
}

/// Sends a lightweight Windows notification via PowerShell without flashing a console window.
pub fn send_notification(title: &str, body: &str) {
    let esc = |s: &str| s.replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $n = New-Object System.Windows.Forms.NotifyIcon; \
         $n.Icon = [System.Drawing.SystemIcons]::Information; \
         $n.BalloonTipTitle = '{}'; \
         $n.BalloonTipText = '{}'; \
         $n.Visible = $true; \
         $n.ShowBalloonTip(4000); \
         Start-Sleep -Milliseconds 4200; \
         $n.Dispose()",
        esc(title),
        esc(body)
    );
    let _ = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Opens a folder in Windows Explorer silently.
pub fn open_in_file_manager(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer")
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
