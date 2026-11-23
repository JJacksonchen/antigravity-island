//! Quota domain models, cadence classification, compact badge selection, and pace calculations.

pub mod client;
pub mod demo;
pub mod discovery;
pub mod parser;
pub mod sync;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Data origin of a fetched quota snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    AppLocal,
    IdeLocal,
    CliLocal,
    CliPrintUsage,
    CliStatusLine,
    Demo,
}

impl SyncSource {
    /// Human-readable label for UI badges and CLI output.
    pub fn label(&self) -> &'static str {
        match self {
            Self::AppLocal => "Antigravity App",
            Self::IdeLocal => "Antigravity IDE",
            Self::CliLocal => "agy CLI",
            Self::CliPrintUsage => "agy /usage",
            Self::CliStatusLine => "CLI StatusLine",
            Self::Demo => "Demo",
        }
    }
}

/// Normalized quota reset cadence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "label", rename_all = "snake_case")]
pub enum Cadence {
    FiveHour,
    Session,
    Weekly,
    Other(String),
}

impl Cadence {
    /// Short badge label shown alongside progress bars.
    pub fn short_tag(&self) -> &str {
        match self {
            Self::FiveHour => "5h",
            Self::Session => "Session",
            Self::Weekly => "7d",
            Self::Other(s) => s.as_str(),
        }
    }

    /// Sort priority within a group: session / 5h before weekly.
    pub fn sort_rank(&self) -> u8 {
        match self {
            Self::FiveHour => 0,
            Self::Session => 1,
            Self::Weekly => 2,
            Self::Other(_) => 3,
        }
    }

    /// Standard window duration in minutes when known.
    pub fn default_window_minutes(&self) -> Option<u64> {
        match self {
            Self::FiveHour | Self::Session => Some(300),
            Self::Weekly => Some(10_080),
            Self::Other(_) => None,
        }
    }
}

/// A single rate-limit bucket (e.g., Gemini 5-hour or Claude/GPT weekly).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaBucket {
    pub bucket_id: String,
    pub display_name: String,
    pub cadence: Cadence,
    pub window_minutes: Option<u64>,
    pub remaining_fraction: Option<f64>,
    pub reset_time: Option<DateTime<Utc>>,
    pub reset_description: Option<String>,
    pub disabled: bool,
}

impl QuotaBucket {
    /// Rounded integer percentage remaining (0..=100) if measured and enabled.
    pub fn remaining_percent(&self) -> Option<u8> {
        if self.disabled {
            return None;
        }
        self.remaining_fraction
            .map(|f| (f.clamp(0.0, 1.0) * 100.0).round() as u8)
    }

    /// Linear pace delta in percentage points relative to elapsed window time.
    /// Positive means remaining budget exceeds linear schedule; negative means burning faster.
    pub fn pace_delta_pct(&self, now: DateTime<Utc>) -> Option<i16> {
        let actual = self.remaining_fraction? * 100.0;
        let reset = self.reset_time?;
        let window_mins = self
            .window_minutes
            .or_else(|| self.cadence.default_window_minutes())? as f64;
        if window_mins <= 0.0 {
            return None;
        }
        let secs_left = (reset - now).num_seconds() as f64;
        if secs_left <= 0.0 {
            return None;
        }
        let window_secs = window_mins * 60.0;
        let expected = ((secs_left / window_secs).clamp(0.0, 1.0)) * 100.0;
        Some((actual - expected).round() as i16)
    }
}

/// Logical model family group (e.g., Gemini Models, Claude & GPT, Gemini 4 Argon).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaGroup {
    pub id: String,
    pub display_name: String,
    pub short_label: String,
    pub accent_key: String,
    pub description: Option<String>,
    pub buckets: Vec<QuotaBucket>,
}

/// Legacy per-model quota row returned by `GetUserStatus` / `GetCommandModelConfigs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelQuotaRow {
    pub label: String,
    pub model_id: String,
    pub remaining_fraction: Option<f64>,
    pub reset_time: Option<DateTime<Utc>>,
}

/// Compact pill badge extracted for collapsed Dynamic Island & menu bar display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactMetric {
    pub short_label: String,
    pub window_tag: String,
    pub accent_key: String,
    pub percent: u8,
    pub reset_time: Option<DateTime<Utc>>,
}

/// Complete synchronized Antigravity quota snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaSnapshot {
    pub fetched_at: DateTime<Utc>,
    pub source: SyncSource,
    pub account_email: Option<String>,
    pub account_plan: Option<String>,
    pub ai_credits: Option<f64>,
    pub groups: Vec<QuotaGroup>,
    pub model_quotas: Vec<ModelQuotaRow>,
    pub summary_note: Option<String>,
}

impl QuotaSnapshot {
    /// Whether this snapshot carries at least one measured quota value.
    pub fn has_measured_quota(&self) -> bool {
        self.groups.iter().any(|g| {
            g.buckets
                .iter()
                .any(|b| !b.disabled && b.remaining_fraction.is_some())
        }) || self
            .model_quotas
            .iter()
            .any(|m| m.remaining_fraction.is_some())
    }

    /// Quality score used to choose the richest snapshot across multiple running processes.
    pub fn quality_score(&self) -> usize {
        let mut score = 0usize;
        let active_buckets: usize = self
            .groups
            .iter()
            .flat_map(|g| &g.buckets)
            .filter(|b| !b.disabled && b.remaining_fraction.is_some())
            .count();
        if active_buckets > 0 {
            score += 1000 + self.groups.len() * 20 + active_buckets * 30;
        }
        let known_models = self
            .model_quotas
            .iter()
            .filter(|m| m.remaining_fraction.is_some())
            .count();
        score += known_models * 10;
        if self.account_email.is_some() {
            score += 3;
        }
        if self.account_plan.is_some() {
            score += 2;
        }
        if self.source == SyncSource::AppLocal {
            score += 5;
        }
        score
    }

    /// Lowest remaining percentage across all active buckets (drives alert colors).
    pub fn min_remaining_percent(&self) -> Option<u8> {
        let bucket_min = self
            .groups
            .iter()
            .flat_map(|g| &g.buckets)
            .filter_map(|b| b.remaining_percent())
            .min();
        if bucket_min.is_some() {
            return bucket_min;
        }
        self.model_quotas
            .iter()
            .filter_map(|m| {
                m.remaining_fraction
                    .map(|f| (f.clamp(0.0, 1.0) * 100.0).round() as u8)
            })
            .min()
    }

    /// Soonest future reset timestamp across all buckets (for automatic reset-zero refresh).
    pub fn soonest_reset(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.groups
            .iter()
            .flat_map(|g| &g.buckets)
            .filter_map(|b| b.reset_time)
            .chain(self.model_quotas.iter().filter_map(|m| m.reset_time))
            .filter(|dt| *dt > now)
            .min()
    }

    /// Selects up to 3 high-signal compact badges for the collapsed Dynamic Island pill.
    /// For each group, picks the most constrained active bucket (or 5h when tied).
    pub fn compact_highlights(&self) -> Vec<CompactMetric> {
        let mut metrics = Vec::new();
        for group in &self.groups {
            let best = group
                .buckets
                .iter()
                .filter_map(|b| {
                    let pct = b.remaining_percent()?;
                    Some((pct, b.cadence.sort_rank(), b))
                })
                .min_by_key(|(pct, rank, _)| (*pct, *rank));

            if let Some((percent, _, bucket)) = best {
                metrics.push(CompactMetric {
                    short_label: group.short_label.clone(),
                    window_tag: bucket.cadence.short_tag().to_string(),
                    accent_key: group.accent_key.clone(),
                    percent,
                    reset_time: bucket.reset_time,
                });
            }
        }
        metrics.truncate(3);
        metrics
    }

    /// Concise string for macOS menu bar tray label (e.g., "G 76%·67% | C 100%").
    pub fn tray_summary(&self) -> String {
        let mut parts = Vec::new();
        for group in self.groups.iter().take(2) {
            let code = match group.accent_key.as_str() {
                "gemini" => "G",
                "claude" => "C",
                "argon" => "A",
                _ => "M",
            };
            let five_h = group
                .buckets
                .iter()
                .find(|b| matches!(b.cadence, Cadence::FiveHour | Cadence::Session))
                .and_then(|b| b.remaining_percent());
            let weekly = group
                .buckets
                .iter()
                .find(|b| matches!(b.cadence, Cadence::Weekly))
                .and_then(|b| b.remaining_percent());

            match (five_h, weekly) {
                (Some(s), Some(w)) if s != w => parts.push(format!("{code} {s}%/{w}%")),
                (Some(s), _) => parts.push(format!("{code} {s}%")),
                (None, Some(w)) => parts.push(format!("{code} {w}%")),
                (None, None) => {}
            }
        }
        if parts.is_empty() {
            return "AGY --".to_string();
        }
        parts.join(" · ")
    }

    /// Rich multi-line tooltip for Windows/Linux/macOS system tray hover.
    pub fn tray_tooltip(&self, now: DateTime<Utc>) -> String {
        let plan = self.account_plan.as_deref().unwrap_or("Antigravity");
        let mut lines = vec![format!("Antigravity Island — {plan}")];
        for group in &self.groups {
            let mut bucket_strs = Vec::new();
            for b in &group.buckets {
                if let Some(pct) = b.remaining_percent() {
                    let reset_part = b
                        .reset_time
                        .map(|dt| {
                            let secs = (dt - now).num_seconds().max(0);
                            if secs >= 86_400 {
                                format!(" ({}d {}h)", secs / 86_400, (secs % 86_400) / 3600)
                            } else if secs >= 3600 {
                                format!(" ({}h {:02}m)", secs / 3600, (secs % 3600) / 60)
                            } else {
                                format!(" ({}m)", secs / 60)
                            }
                        })
                        .unwrap_or_default();
                    bucket_strs.push(format!("{} {pct}%{reset_part}", b.cadence.short_tag()));
                }
            }
            if !bucket_strs.is_empty() {
                lines.push(format!(
                    "{}: {}",
                    group.short_label,
                    bucket_strs.join(" · ")
                ));
            }
        }
        if let Some(cr) = self.ai_credits {
            lines.push(format!("AI Credits: {:.0}", cr));
        }
        lines.join("\n")
    }
}

/// Live synchronization status shared between the background engine, tray, and WebView UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub connected: bool,
    pub syncing: bool,
    pub demo_mode: bool,
    pub last_error: Option<String>,
    pub snapshot: Option<QuotaSnapshot>,
}

impl SyncState {
    pub fn initial(demo_mode: bool) -> Self {
        Self {
            connected: false,
            syncing: true,
            demo_mode,
            last_error: None,
            snapshot: None,
        }
    }
}
