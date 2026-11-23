//! Desktop window host, WKWebView/WebView2 lifecycle, bidirectional IPC, and event loop controller.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::menu::MenuEvent;
use wry::WebViewBuilder;

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS, WindowBuilderExtMacOS};

#[cfg(target_os = "windows")]
use tao::platform::windows::WindowBuilderExtWindows;

use crate::config::{AppConfig, DisplayMode, Language, PillDensity};
use crate::platform::{self, ScreenMetrics};
use crate::quota::sync::{spawn_sync_engine, SyncCommand};
use crate::quota::SyncState;
use crate::skin::{start_skin_watcher, SkinManifest, SkinRegistry};
use crate::tray::{TrayAction, TrayController};

const INDEX_HTML: &str = include_str!("assets/index.html");
const STYLE_CSS: &str = include_str!("assets/style.css");
const APP_JS: &str = include_str!("assets/app.js");

/// User events waking the native desktop UI loop.
#[derive(Debug, Clone)]
pub enum AppEvent {
    QuotaUpdated(SyncState),
    SkinsReloaded,
    IpcMessage(String),
    MenuEvent(MenuEvent),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum IpcCommand {
    Ready,
    Resize { width: f64, height: f64 },
    Drag,
    Refresh,
    SetSkin { id: String },
    SaveSkin { skin: Box<SkinManifest> },
    SetMode { mode: DisplayMode },
    SetPillDensity { density: PillDensity },
    SetPinned { pinned: bool },
    SetPollInterval { secs: u64 },
    SetLanguage { lang: Language },
    SetNotifications { enabled: bool },
    SetDemo { enabled: bool },
    OpenSkinsDir,
}

#[derive(Debug, Serialize)]
struct UiPayload<'a> {
    platform: &'static str,
    status: &'a SyncState,
    skin: &'a SkinManifest,
    all_skins: &'a [SkinManifest],
    config: &'a AppConfig,
    screen: ScreenMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    force_expanded: Option<bool>,
}

/// Launches the Dynamic Island / Floating Widget desktop experience + OS tray companion.
pub fn run_app(mut config: AppConfig, demo_mode: bool) -> anyhow::Result<()> {
    #[cfg(not(target_os = "macos"))]
    if config.mode == DisplayMode::NotchHug {
        config.mode = DisplayMode::Island;
    }

    let mut registry = SkinRegistry::load_default();
    let _ = registry.ensure_initialized();
    registry.reload();

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

    #[cfg(target_os = "macos")]
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy = event_loop.create_proxy();

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |ev| {
        let _ = menu_proxy.send_event(AppEvent::MenuEvent(ev));
    }));

    let mut sync_state = SyncState::initial(demo_mode);
    let mut tray = TrayController::new(&config, registry.list(), &sync_state)?;

    let screen = platform::primary_screen_metrics(None);
    let init_w = match config.mode {
        DisplayMode::NotchHug => 380.0,
        _ => 340.0,
    };
    let init_h = 64.0;
    let init_pos = compute_window_position(&config, screen, init_w, init_h);

    let builder = WindowBuilder::new()
        .with_title("Antigravity Island")
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_always_on_top(true)
        .with_inner_size(LogicalSize::new(init_w, init_h))
        .with_position(LogicalPosition::new(init_pos.0, init_pos.1))
        .with_visible(config.visible);

    #[cfg(target_os = "macos")]
    let builder = builder
        .with_has_shadow(false)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true);

    #[cfg(target_os = "windows")]
    let builder = builder
        .with_skip_taskbar(true)
        .with_undecorated_shadow(false);

    let window = builder.build(&event_loop)?;
    platform::configure_window(&window, config.mode);

    // Re-position using the actual monitor handle (important on Windows & Linux).
    let actual_screen = platform::primary_screen_metrics(Some(&window));
    let actual_pos = compute_window_position(&config, actual_screen, init_w, init_h);
    window.set_outer_position(LogicalPosition::new(actual_pos.0, actual_pos.1));

    let html = INDEX_HTML
        .replace("/*__STYLE__*/", STYLE_CSS)
        .replace("/*__SCRIPT__*/", APP_JS);

    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_background_color((0, 0, 0, 0))
        .with_html(html)
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(AppEvent::IpcMessage(req.body().clone()));
        })
        .build(&window)?;

    let skin_proxy = proxy.clone();
    let _skin_watcher = start_skin_watcher(registry.skins_dir().to_path_buf(), move || {
        let _ = skin_proxy.send_event(AppEvent::SkinsReloaded);
    });

    let sync_proxy = proxy.clone();
    let sync_tx = spawn_sync_engine(
        config.poll_interval_secs,
        demo_mode,
        config.notifications,
        config.warn_threshold,
        config.danger_threshold,
        move |next_state| {
            let _ = sync_proxy.send_event(AppEvent::QuotaUpdated(next_state));
        },
    );

    let mut current_size = (init_w, init_h);
    let mut ready = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(app_ev) => match app_ev {
                AppEvent::QuotaUpdated(next_state) => {
                    sync_state = next_state;
                    tray.sync_state(&config, registry.list(), &sync_state);
                    if ready {
                        push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                    }
                }
                AppEvent::SkinsReloaded => {
                    registry.reload();
                    tray.sync_state(&config, registry.list(), &sync_state);
                    if ready {
                        push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                    }
                }
                AppEvent::MenuEvent(ev) => {
                    if let Some(action) = tray.action_for(&ev) {
                        match action {
                            TrayAction::ToggleVisible => {
                                config.visible = !config.visible;
                                window.set_visible(config.visible);
                                let _ = config.save();
                                tray.sync_state(&config, registry.list(), &sync_state);
                            }
                            TrayAction::ToggleExpand => {
                                config.pinned_expanded = !config.pinned_expanded;
                                if !config.visible {
                                    config.visible = true;
                                    window.set_visible(true);
                                }
                                let _ = config.save();
                                tray.sync_state(&config, registry.list(), &sync_state);
                                if ready {
                                    push_ui_state(
                                        &webview,
                                        &window,
                                        &sync_state,
                                        &registry,
                                        &config,
                                        Some(config.pinned_expanded),
                                    );
                                }
                            }
                            TrayAction::RefreshNow => {
                                let _ = sync_tx.send(SyncCommand::RefreshNow);
                            }
                            TrayAction::SetMode(mode) => {
                                apply_mode_change(&mut config, mode, &window, current_size);
                                tray.sync_state(&config, registry.list(), &sync_state);
                                if ready {
                                    push_ui_state(
                                        &webview,
                                        &window,
                                        &sync_state,
                                        &registry,
                                        &config,
                                        None,
                                    );
                                }
                            }
                            TrayAction::SetPillDensity(density) => {
                                config.pill_density = density;
                                let _ = config.save();
                                tray.sync_state(&config, registry.list(), &sync_state);
                                if ready {
                                    push_ui_state(
                                        &webview,
                                        &window,
                                        &sync_state,
                                        &registry,
                                        &config,
                                        None,
                                    );
                                }
                            }
                            TrayAction::SelectSkin(id) => {
                                config.skin_id = id;
                                let _ = config.save();
                                tray.sync_state(&config, registry.list(), &sync_state);
                                if ready {
                                    push_ui_state(
                                        &webview,
                                        &window,
                                        &sync_state,
                                        &registry,
                                        &config,
                                        None,
                                    );
                                }
                            }
                            TrayAction::ToggleDemo => {
                                let next = !sync_state.demo_mode;
                                let _ = sync_tx.send(SyncCommand::SetDemoMode(next));
                            }
                            TrayAction::OpenSkinsDir => {
                                platform::open_in_file_manager(registry.skins_dir());
                            }
                            TrayAction::Quit => {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                    }
                }
                AppEvent::IpcMessage(raw) => {
                    let Ok(cmd) = serde_json::from_str::<IpcCommand>(&raw) else {
                        return;
                    };
                    match cmd {
                        IpcCommand::Ready => {
                            ready = true;
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::Resize { width, height } => {
                            let w = width.clamp(120.0, 680.0);
                            let h = height.clamp(46.0, 820.0);
                            current_size = (w, h);
                            window.set_inner_size(LogicalSize::new(w, h));
                            let screen = platform::primary_screen_metrics(Some(&window));
                            let (x, y) = compute_window_position(&config, screen, w, h);
                            window.set_outer_position(LogicalPosition::new(x, y));
                        }
                        IpcCommand::Drag => {
                            if config.mode == DisplayMode::Widget {
                                let _ = window.drag_window();
                            }
                        }
                        IpcCommand::Refresh => {
                            let _ = sync_tx.send(SyncCommand::RefreshNow);
                        }
                        IpcCommand::SetSkin { id } => {
                            config.skin_id = id;
                            let _ = config.save();
                            tray.sync_state(&config, registry.list(), &sync_state);
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SaveSkin { skin } => {
                            if let Ok(saved) = registry.save_custom_skin(*skin) {
                                config.skin_id = saved.id;
                                let _ = config.save();
                                tray.sync_state(&config, registry.list(), &sync_state);
                                push_ui_state(
                                    &webview,
                                    &window,
                                    &sync_state,
                                    &registry,
                                    &config,
                                    None,
                                );
                            }
                        }
                        IpcCommand::SetMode { mode } => {
                            apply_mode_change(&mut config, mode, &window, current_size);
                            tray.sync_state(&config, registry.list(), &sync_state);
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SetPillDensity { density } => {
                            config.pill_density = density;
                            let _ = config.save();
                            tray.sync_state(&config, registry.list(), &sync_state);
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SetPinned { pinned } => {
                            config.pinned_expanded = pinned;
                            let _ = config.save();
                            tray.sync_state(&config, registry.list(), &sync_state);
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SetPollInterval { secs } => {
                            config.poll_interval_secs = secs.clamp(5, 300);
                            let _ = config.save();
                            let _ = sync_tx
                                .send(SyncCommand::SetPollInterval(config.poll_interval_secs));
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SetLanguage { lang } => {
                            config.language = lang;
                            let _ = config.save();
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SetNotifications { enabled } => {
                            config.notifications = enabled;
                            let _ = config.save();
                            let _ = sync_tx.send(SyncCommand::SetNotifications(enabled));
                            push_ui_state(&webview, &window, &sync_state, &registry, &config, None);
                        }
                        IpcCommand::SetDemo { enabled } => {
                            let _ = sync_tx.send(SyncCommand::SetDemoMode(enabled));
                        }
                        IpcCommand::OpenSkinsDir => {
                            platform::open_in_file_manager(registry.skins_dir());
                        }
                    }
                }
            },
            Event::WindowEvent {
                event: WindowEvent::Moved(pos),
                ..
            } => {
                if config.mode == DisplayMode::Widget {
                    let scale = window.scale_factor();
                    let logical = pos.to_logical::<f64>(scale);
                    config.widget_x = Some(logical.x);
                    config.widget_y = Some(logical.y);
                    let _ = config.save();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn apply_mode_change(
    config: &mut AppConfig,
    mode: DisplayMode,
    window: &tao::window::Window,
    current_size: (f64, f64),
) {
    #[cfg(not(target_os = "macos"))]
    let mode = if mode == DisplayMode::NotchHug {
        DisplayMode::Island
    } else {
        mode
    };

    config.mode = mode;
    platform::update_window_mode(window, mode);
    let screen = platform::primary_screen_metrics(Some(window));
    let (x, y) = compute_window_position(config, screen, current_size.0, current_size.1);
    window.set_outer_position(LogicalPosition::new(x, y));
    let _ = config.save();
}

fn compute_window_position(
    config: &AppConfig,
    screen: ScreenMetrics,
    width: f64,
    height: f64,
) -> (f64, f64) {
    match config.mode {
        DisplayMode::Island => {
            let x = screen.origin_x + ((screen.width - width) / 2.0).round();
            let y = screen.origin_y + 4.0;
            (x, y)
        }
        DisplayMode::NotchHug => {
            let x = screen.origin_x + ((screen.width - width) / 2.0).round();
            let y = screen.origin_y + 2.0;
            (x, y)
        }
        DisplayMode::Widget => {
            let default_x = screen.origin_x + (screen.width - width - 24.0).max(16.0);
            let default_y = screen.origin_y + screen.menu_bar_height + 16.0;
            let raw_x = config.widget_x.unwrap_or(default_x);
            let raw_y = config.widget_y.unwrap_or(default_y);
            let x = raw_x.clamp(
                screen.origin_x + 8.0,
                (screen.origin_x + screen.width - width - 8.0).max(8.0),
            );
            let y = raw_y.clamp(
                screen.origin_y + screen.menu_bar_height + 4.0,
                (screen.origin_y + screen.height - height - 8.0).max(40.0),
            );
            (x, y)
        }
    }
}

fn push_ui_state(
    webview: &wry::WebView,
    window: &tao::window::Window,
    status: &SyncState,
    registry: &SkinRegistry,
    config: &AppConfig,
    force_expanded: Option<bool>,
) {
    let active_skin = registry.resolve(&config.skin_id);
    let screen = platform::primary_screen_metrics(Some(window));
    let payload = UiPayload {
        platform: std::env::consts::OS,
        status,
        skin: &active_skin,
        all_skins: registry.list(),
        config,
        screen,
        force_expanded,
    };

    if let Ok(serialized) = serde_json::to_string(&json!(payload)) {
        let script = format!("window.__AGY_RECEIVE__ && window.__AGY_RECEIVE__({serialized});");
        let _ = webview.evaluate_script(&script);
    }
}
