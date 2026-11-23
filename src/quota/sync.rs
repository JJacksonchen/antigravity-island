//! Adaptive real-time quota synchronization engine with filesystem activity watching,
//! reset-zero scheduling, and threshold alert notifications.

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use super::client::QuotaClient;
use super::demo;
use super::{QuotaSnapshot, SyncState};
use crate::config::live_statusline_path;
use crate::platform;

/// Commands sent from the UI/tray to the background sync coordinator.
#[derive(Debug, Clone)]
pub enum SyncCommand {
    RefreshNow,
    SetPollInterval(u64),
    SetDemoMode(bool),
    SetNotifications(bool),
    SetThresholds { warn: u8, danger: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertLevel {
    Normal,
    Warn,
    Danger,
}

/// Spawns the background Tokio runtime driving real-time quota synchronization.
/// Calls `on_update(SyncState)` on every state change.
pub fn spawn_sync_engine<F>(
    initial_poll_secs: u64,
    initial_demo: bool,
    initial_notifications: bool,
    warn_threshold: u8,
    danger_threshold: u8,
    on_update: F,
) -> mpsc::UnboundedSender<SyncCommand>
where
    F: Fn(SyncState) + Send + Sync + 'static,
{
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<SyncCommand>();

    std::thread::Builder::new()
        .name("agy-quota-sync".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio sync runtime");

            rt.block_on(async move {
                let client = QuotaClient::new();
                let mut poll_secs = initial_poll_secs.clamp(5, 300);
                let mut demo_mode = initial_demo;
                let mut notifications = initial_notifications;
                let mut warn = warn_threshold;
                let mut danger = danger_threshold;
                let mut demo_tick: u64 = 0;

                let mut state = SyncState::initial(demo_mode);
                let mut prev_alert = AlertLevel::Normal;

                // Passive filesystem activity channel: triggers a debounced sync shortly after
                // Antigravity writes conversation/state metadata or CLI statusLine JSON updates.
                let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<()>();
                let _watcher = start_antigravity_activity_watcher(fs_tx);

                let mut pending_fs_refresh: Option<tokio::time::Instant> = None;
                let mut next_reset_refresh: Option<tokio::time::Instant> = None;

                // Trigger initial fetch immediately.
                perform_sync(
                    &client,
                    &mut state,
                    demo_mode,
                    &mut demo_tick,
                    notifications,
                    warn,
                    danger,
                    &mut prev_alert,
                    &mut next_reset_refresh,
                    &on_update,
                )
                .await;

                let mut interval = tokio::time::interval(Duration::from_secs(poll_secs));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                interval.tick().await; // consume immediate first tick

                loop {
                    let fs_sleep = async {
                        if let Some(deadline) = pending_fs_refresh {
                            tokio::time::sleep_until(deadline).await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    };
                    let reset_sleep = async {
                        if let Some(deadline) = next_reset_refresh {
                            tokio::time::sleep_until(deadline).await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    };

                    tokio::select! {
                        _ = interval.tick() => {
                            perform_sync(
                                &client,
                                &mut state,
                                demo_mode,
                                &mut demo_tick,
                                notifications,
                                warn,
                                danger,
                                &mut prev_alert,
                                &mut next_reset_refresh,
                                &on_update,
                            ).await;
                        }
                        _ = fs_sleep => {
                            pending_fs_refresh = None;
                            perform_sync(
                                &client,
                                &mut state,
                                demo_mode,
                                &mut demo_tick,
                                notifications,
                                warn,
                                danger,
                                &mut prev_alert,
                                &mut next_reset_refresh,
                                &on_update,
                            ).await;
                        }
                        _ = reset_sleep => {
                            next_reset_refresh = None;
                            perform_sync(
                                &client,
                                &mut state,
                                demo_mode,
                                &mut demo_tick,
                                notifications,
                                warn,
                                danger,
                                &mut prev_alert,
                                &mut next_reset_refresh,
                                &on_update,
                            ).await;
                        }
                        Some(()) = fs_rx.recv() => {
                            // Debounce rapid bursts of agent writes into a single prompt refresh 2.2s later.
                            pending_fs_refresh = Some(tokio::time::Instant::now() + Duration::from_millis(2200));
                        }
                        maybe_cmd = cmd_rx.recv() => {
                            let Some(cmd) = maybe_cmd else { break };
                            match cmd {
                                SyncCommand::RefreshNow => {
                                    client.invalidate_cache().await;
                                    perform_sync(
                                        &client,
                                        &mut state,
                                        demo_mode,
                                        &mut demo_tick,
                                        notifications,
                                        warn,
                                        danger,
                                        &mut prev_alert,
                                        &mut next_reset_refresh,
                                        &on_update,
                                    ).await;
                                }
                                SyncCommand::SetPollInterval(secs) => {
                                    poll_secs = secs.clamp(5, 300);
                                    interval = tokio::time::interval(Duration::from_secs(poll_secs));
                                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                                    interval.tick().await;
                                }
                                SyncCommand::SetDemoMode(enabled) => {
                                    demo_mode = enabled;
                                    state.demo_mode = enabled;
                                    perform_sync(
                                        &client,
                                        &mut state,
                                        demo_mode,
                                        &mut demo_tick,
                                        notifications,
                                        warn,
                                        danger,
                                        &mut prev_alert,
                                        &mut next_reset_refresh,
                                        &on_update,
                                    ).await;
                                }
                                SyncCommand::SetNotifications(enabled) => {
                                    notifications = enabled;
                                }
                                SyncCommand::SetThresholds { warn: w, danger: d } => {
                                    warn = w;
                                    danger = d;
                                }
                            }
                        }
                    }
                }
            });
        })
        .expect("spawn quota sync thread");

    cmd_tx
}

#[allow(clippy::too_many_arguments)]
async fn perform_sync<F>(
    client: &QuotaClient,
    state: &mut SyncState,
    demo_mode: bool,
    demo_tick: &mut u64,
    notifications: bool,
    warn_threshold: u8,
    danger_threshold: u8,
    prev_alert: &mut AlertLevel,
    next_reset_refresh: &mut Option<tokio::time::Instant>,
    on_update: &F,
) where
    F: Fn(SyncState),
{
    state.syncing = true;
    state.demo_mode = demo_mode;
    on_update(state.clone());

    let result: anyhow::Result<QuotaSnapshot> = if demo_mode {
        *demo_tick = demo_tick.wrapping_add(1);
        Ok(demo::sample_snapshot(*demo_tick))
    } else {
        client.fetch_live_quota().await
    };

    state.syncing = false;
    match result {
        Ok(snapshot) => {
            let now = Utc::now();
            *next_reset_refresh = snapshot.soonest_reset(now).map(|reset| {
                let secs = (reset - now).num_seconds().clamp(2, 21_600) as u64 + 2;
                tokio::time::Instant::now() + Duration::from_secs(secs)
            });

            if let Some(min_pct) = snapshot.min_remaining_percent() {
                let next_alert = if min_pct <= danger_threshold {
                    AlertLevel::Danger
                } else if min_pct <= warn_threshold {
                    AlertLevel::Warn
                } else {
                    AlertLevel::Normal
                };

                if notifications && !demo_mode {
                    maybe_emit_alert(*prev_alert, next_alert, min_pct);
                }
                *prev_alert = next_alert;
            }

            state.connected = true;
            state.last_error = None;
            state.snapshot = Some(snapshot);
        }
        Err(err) => {
            state.connected = false;
            state.last_error = Some(err.to_string());
        }
    }

    on_update(state.clone());
}

fn maybe_emit_alert(prev: AlertLevel, next: AlertLevel, min_pct: u8) {
    match (prev, next) {
        (AlertLevel::Normal, AlertLevel::Warn) => {
            platform::send_notification(
                "Antigravity Quota Warning",
                &format!("Remaining quota dropped to {min_pct}%."),
            );
        }
        (AlertLevel::Normal | AlertLevel::Warn, AlertLevel::Danger) => {
            platform::send_notification(
                "Antigravity Quota Critical",
                &format!("Only {min_pct}% quota remaining before reset."),
            );
        }
        (AlertLevel::Danger | AlertLevel::Warn, AlertLevel::Normal) if min_pct >= 95 => {
            platform::send_notification(
                "Antigravity Quota Refreshed",
                &format!("Quota window reset ({min_pct}% available)."),
            );
        }
        _ => {}
    }
}

fn start_antigravity_activity_watcher(tx: mpsc::UnboundedSender<()>) -> Option<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if !event.kind.is_access() {
                let _ = tx.send(());
            }
        }
    })
    .ok()?;

    let mut watched_any = false;
    if let Some(home) = dirs::home_dir() {
        let agy_dir = home.join(".gemini").join("antigravity");
        for sub in ["conversations", "presence", "daemon"] {
            let p: PathBuf = agy_dir.join(sub);
            if p.is_dir() && watcher.watch(&p, RecursiveMode::NonRecursive).is_ok() {
                watched_any = true;
            }
        }
        let state_pbtxt = agy_dir.join("antigravity_state.pbtxt");
        if state_pbtxt.is_file()
            && watcher
                .watch(&state_pbtxt, RecursiveMode::NonRecursive)
                .is_ok()
        {
            watched_any = true;
        }
    }

    let statusline = live_statusline_path();
    if let Some(parent) = statusline.parent() {
        let _ = std::fs::create_dir_all(parent);
        if watcher.watch(parent, RecursiveMode::NonRecursive).is_ok() {
            watched_any = true;
        }
    }

    if watched_any {
        Some(watcher)
    } else {
        None
    }
}
