//! Realistic simulated quota snapshots for `--demo` mode and skin previews.

use chrono::{Duration, Utc};

use super::{Cadence, QuotaBucket, QuotaGroup, QuotaSnapshot, SyncSource};

/// Generates an animated multi-group Antigravity quota snapshot.
pub fn sample_snapshot(tick: u64) -> QuotaSnapshot {
    let now = Utc::now();
    // Gentle oscillation so the user can observe live progress bars, pace tags, and colors.
    let phase = (tick % 24) as f64;
    let gem_5h = (0.86 - phase * 0.008).clamp(0.08, 0.96);
    let gem_7d = (0.64 - phase * 0.003).clamp(0.18, 0.92);
    let claude_5h = (0.94 - phase * 0.011).clamp(0.12, 0.99);
    let claude_7d = (0.72 - phase * 0.004).clamp(0.25, 0.95);
    let argon_7d = (0.58 - phase * 0.005).clamp(0.14, 0.90);

    QuotaSnapshot {
        fetched_at: now,
        source: SyncSource::Demo,
        account_email: Some("demo@antigravity.dev".to_string()),
        account_plan: Some("Google AI Ultra".to_string()),
        ai_credits: Some(128.0),
        groups: vec![
            QuotaGroup {
                id: "gemini-models".to_string(),
                display_name: "Gemini Models".to_string(),
                short_label: "Gemini".to_string(),
                accent_key: "gemini".to_string(),
                description: Some("Gemini Pro & Flash rolling budget".to_string()),
                buckets: vec![
                    QuotaBucket {
                        bucket_id: "gemini-5h".to_string(),
                        display_name: "5-hour limit".to_string(),
                        cadence: Cadence::FiveHour,
                        window_minutes: Some(300),
                        remaining_fraction: Some(gem_5h),
                        reset_time: Some(now + Duration::minutes(134) + Duration::seconds(42)),
                        reset_description: None,
                        disabled: false,
                    },
                    QuotaBucket {
                        bucket_id: "gemini-weekly".to_string(),
                        display_name: "Weekly limit".to_string(),
                        cadence: Cadence::Weekly,
                        window_minutes: Some(10_080),
                        remaining_fraction: Some(gem_7d),
                        reset_time: Some(now + Duration::hours(91) + Duration::minutes(18)),
                        reset_description: None,
                        disabled: false,
                    },
                ],
            },
            QuotaGroup {
                id: "claude-gpt".to_string(),
                display_name: "Claude and GPT models".to_string(),
                short_label: "Claude/GPT".to_string(),
                accent_key: "claude".to_string(),
                description: Some("Shared third-party partner pool".to_string()),
                buckets: vec![
                    QuotaBucket {
                        bucket_id: "3p-5h".to_string(),
                        display_name: "5-hour limit".to_string(),
                        cadence: Cadence::FiveHour,
                        window_minutes: Some(300),
                        remaining_fraction: Some(claude_5h),
                        reset_time: Some(now + Duration::minutes(219) + Duration::seconds(11)),
                        reset_description: None,
                        disabled: false,
                    },
                    QuotaBucket {
                        bucket_id: "3p-weekly".to_string(),
                        display_name: "Weekly limit".to_string(),
                        cadence: Cadence::Weekly,
                        window_minutes: Some(10_080),
                        remaining_fraction: Some(claude_7d),
                        reset_time: Some(now + Duration::hours(118) + Duration::minutes(5)),
                        reset_description: None,
                        disabled: false,
                    },
                ],
            },
            QuotaGroup {
                id: "gemini-4-argon".to_string(),
                display_name: "Gemini 4 Argon".to_string(),
                short_label: "Argon".to_string(),
                accent_key: "argon".to_string(),
                description: Some("High-reasoning Argon budget".to_string()),
                buckets: vec![QuotaBucket {
                    bucket_id: "argon-weekly".to_string(),
                    display_name: "Weekly limit".to_string(),
                    cadence: Cadence::Weekly,
                    window_minutes: Some(10_080),
                    remaining_fraction: Some(argon_7d),
                    reset_time: Some(now + Duration::hours(74) + Duration::minutes(40)),
                    reset_description: None,
                    disabled: false,
                }],
            },
        ],
        model_quotas: Vec::new(),
        summary_note: Some("Simulated live quota stream (--demo)".to_string()),
    }
}
