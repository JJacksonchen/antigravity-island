//! Desktop window host, WKWebView/WebView2 lifecycle, bidirectional IPC, and event loop controller.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
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
    TrayEvent(TrayIconEvent),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum IpcCommand {
    Ready,
    Resize { width: f64, height: f64 },
    Drag,
    Hide,
    ResetPosition,
    Refresh,
    SetSkin { id: String },
    SaveSkin { skin: Box<SkinManifest> },
    SetMode { mode: DisplayMode },
    SetPillDensity { density: PillDensity },
    SetPinned { pinned: bool },
    SetPollInterval { secs: u64 },
    SetLanguage { lang: Language },
    SetNotifications { enabled: bool },
    SetAlwaysShowMenubarText { enabled: bool },
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

    let tray_click_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |ev| {
        let _ = tray_click_proxy.send_event(AppEvent::TrayEvent(ev));
    }));

    let mut sync_state = SyncState::initial(demo_mode);
    let mut tray = TrayController::new(&config, registry.list(), &sync_state)?;

    let screen = platform::primary_screen_metrics(None);
    let init_w = match config.mode {
        DisplayMode::NotchHug => 380.0,
        _ => 380.0,
    };
    let init_h = 68.0;
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

    let actual_screen = platform::active_screen_metrics(&window);
    let actual_pos = compute_window_position(&config, actual_screen, init_w, init_h);
    let mut last_programmatic_move = Instant::now();
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
    let mut pending_pos_save: Option<Instant> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = match pending_pos_save {
            Some(deadline) => ControlFlow::WaitUntil(deadline),
            None => ControlFlow::Wait,
        };

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
                AppEvent::TrayEvent(ev) => {
                    // On Windows & Linux, left-clicking the tray ring icon toggles Show/Hide directly.
                    #[cfg(not(target_os = "macos"))]
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        config.visible = !config.visible;
                        window.set_visible(config.visible);
                        let _ = config.save();
                        tray.sync_state(&config, registry.list(), &sync_state);
                    }
                    #[cfg(target_os = "macos")]
                    let _ = (ev, MouseButton::Left, MouseButtonState::Up);
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
                            TrayAction::ResetPosition => {
                                pending_pos_save = None;
                                config.widget_x = None;
                                config.widget_y = None;
                                if !config.visible {
                                    config.visible = true;
                                    window.set_visible(true);
                                }
                                let _ = config.save();
                                let screen = platform::primary_screen_metrics(Some(&window));
                                let (x, y) = compute_window_position(
                                    &config,
                                    screen,
                                    current_size.0,
                                    current_size.1,
                                );
                                last_programmatic_move = Instant::now();
                                window.set_outer_position(LogicalPosition::new(x, y));
                                tray.sync_state(&config, registry.list(), &sync_state);
                            }
                            TrayAction::RefreshNow => {
                                let _ = sync_tx.send(SyncCommand::RefreshNow);
                            }
                            TrayAction::SetMode(mode) => {
                                apply_mode_change(
                                    &mut config,
                                    mode,
                                    &window,
                                    current_size,
                                    &mut last_programmatic_move,
                                );
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
                            TrayAction::ToggleMenuBarText => {
                                config.always_show_menubar_text = !config.always_show_menubar_text;
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
                                if pending_pos_save.take().is_some() {
                                    let _ = config.save();
                                }
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
                            let w = width.clamp(120.0, 820.0);
                            let h = height.clamp(48.0, 820.0);
                            current_size = (w, h);
                            last_programmatic_move = Instant::now();
                            window.set_inner_size(LogicalSize::new(w, h));
                            let screen = platform::active_screen_metrics(&window);
                            let (x, y) = compute_window_position(&config, screen, w, h);
                            window.set_outer_position(LogicalPosition::new(x, y));
                        }
                        IpcCommand::Drag => {
                            // Free dragging works in both Island and Widget modes on macOS & Windows.
                            if config.mode != DisplayMode::NotchHug {
                                let _ = window.drag_window();
                            }
                        }
                        IpcCommand::Hide => {
                            pending_pos_save = None;
                            config.visible = false;
                            window.set_visible(false);
                            let _ = config.save();
                            tray.sync_state(&config, registry.list(), &sync_state);
                        }
                        IpcCommand::ResetPosition => {
                            pending_pos_save = None;
                            config.widget_x = None;
                            config.widget_y = None;
                            let _ = config.save();
                            let screen = platform::primary_screen_metrics(Some(&window));
                            let (x, y) = compute_window_position(
                                &config,
                                screen,
                                current_size.0,
                                current_size.1,
                            );
                            last_programmatic_move = Instant::now();
                            window.set_outer_position(LogicalPosition::new(x, y));
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
                            apply_mode_change(
                                &mut config,
                                mode,
                                &window,
                                current_size,
                                &mut last_programmatic_move,
                            );
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
                        IpcCommand::SetAlwaysShowMenubarText { enabled } => {
                            config.always_show_menubar_text = enabled;
                            let _ = config.save();
                            tray.sync_state(&config, registry.list(), &sync_state);
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
                // Persist user-dragged position (storing horizontal center so expand/collapse grows symmetrically).
                // Debounced so 60fps dragging never thrashes disk I/O while always saving the final resting spot.
                if config.mode != DisplayMode::NotchHug
                    && last_programmatic_move.elapsed() > Duration::from_millis(240)
                {
                    let scale = window.scale_factor();
                    let logical = pos.to_logical::<f64>(scale);
                    config.widget_x = Some(logical.x + current_size.0 / 2.0);
                    config.widget_y = Some(logical.y);
                    pending_pos_save = Some(Instant::now() + Duration::from_millis(350));
                }
            }
            Event::MainEventsCleared => {
                if let Some(deadline) = pending_pos_save {
                    if Instant::now() >= deadline {
                        let _ = config.save();
                        pending_pos_save = None;
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if pending_pos_save.take().is_some() {
                    let _ = config.save();
                }
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
    last_programmatic_move: &mut Instant,
) {
    #[cfg(not(target_os = "macos"))]
    let mode = if mode == DisplayMode::NotchHug {
        DisplayMode::Island
    } else {
        mode
    };

    config.mode = mode;
    if mode == DisplayMode::NotchHug {
        config.widget_x = None;
        config.widget_y = None;
    }
    platform::update_window_mode(window, mode);
    let screen = platform::active_screen_metrics(window);
    let (x, y) = compute_window_position(config, screen, current_size.0, current_size.1);
    *last_programmatic_move = Instant::now();
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
        DisplayMode::NotchHug => {
            let x = screen.origin_x + ((screen.width - width) / 2.0).round();
            let y = screen.origin_y + 2.0;
            (x, y)
        }
        DisplayMode::Island | DisplayMode::Widget => {
            let min_y = screen.origin_y + screen.menu_bar_height + 2.0;
            let max_x = (screen.origin_x + screen.width - width - 6.0).max(screen.origin_x + 6.0);
            let max_y = (screen.origin_y + screen.height - height - 8.0).max(min_y);

            if let (Some(cx), Some(saved_y)) = (config.widget_x, config.widget_y) {
                // Saved `widget_x` is horizontal center so expand/collapse expands symmetrically!
                let x = (cx - width / 2.0)
                    .round()
                    .clamp(screen.origin_x + 6.0, max_x);
                let y = saved_y.round().clamp(min_y, max_y);
                return (x, y);
            }

            if config.mode == DisplayMode::Widget {
                let x = (screen.origin_x + screen.width - width - 22.0)
                    .clamp(screen.origin_x + 6.0, max_x);
                let y = (min_y + 12.0).clamp(min_y, max_y);
                (x, y)
            } else {
                // Default Dynamic Island: cleanly centered directly BELOW the macOS menu bar
                // (or right near top edge on Windows where `menu_bar_height == 0.0`).
                let x = screen.origin_x + ((screen.width - width) / 2.0).round();
                (x, min_y)
            }
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
    let screen = platform::active_screen_metrics(window);
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
