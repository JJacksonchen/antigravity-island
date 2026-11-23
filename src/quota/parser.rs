//! Resilient parsers for every Antigravity local RPC, CLI report, and statusLine JSON payload.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use super::{Cadence, ModelQuotaRow, QuotaBucket, QuotaGroup, QuotaSnapshot, SyncSource};

/// Identity and optional legacy model quotas parsed from `GetUserStatus`.
#[derive(Debug, Clone)]
pub struct UserStatusParsed {
    pub account_email: Option<String>,
    pub account_plan: Option<String>,
    pub ai_credits: Option<f64>,
    pub groups: Vec<QuotaGroup>,
    pub model_quotas: Vec<ModelQuotaRow>,
}

/// Parses `LanguageServerService/RetrieveUserQuotaSummary` JSON responses.
/// Supports `response.groups`, `summary.groups`, and top-level `groups`, with camelCase or snake_case.
pub fn parse_quota_summary_response(bytes: &[u8], source: SyncSource) -> Result<QuotaSnapshot> {
    let root: Value = serde_json::from_slice(bytes)?;
    check_rpc_code(&root)?;

    let payload = root
        .get("response")
        .or_else(|| root.get("summary"))
        .unwrap_or(&root);

    parse_quota_summary_payload(payload, source)
}

/// Parses `agy -p /usage --output-format json` non-interactive report.
pub fn parse_cli_usage_report(bytes: &[u8]) -> Result<QuotaSnapshot> {
    let root: Value = serde_json::from_slice(bytes)?;
    let status = root
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !status.eq_ignore_ascii_case("SUCCESS") {
        bail!("CLI usage report status is not SUCCESS: {status}");
    }
    let command = root
        .get("command")
        .ok_or_else(|| anyhow!("Missing command object in CLI usage report"))?;
    let name = command
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if name != "usage" {
        bail!("Unexpected CLI report command name: {name}");
    }
    let data = command
        .get("data")
        .ok_or_else(|| anyhow!("Missing command.data in CLI usage report"))?;

    let snapshot = parse_quota_summary_payload(data, SyncSource::CliPrintUsage)?;
    if !snapshot.has_measured_quota() {
        bail!("CLI usage report contained no measured quota buckets");
    }
    Ok(snapshot)
}

/// Parses `LanguageServerService/GetUserStatus` JSON responses.
pub fn parse_user_status_response(bytes: &[u8], source: SyncSource) -> Result<UserStatusParsed> {
    let root: Value = serde_json::from_slice(bytes)?;
    check_rpc_code(&root)?;

    let user_status = root
        .get("userStatus")
        .or_else(|| root.get("user_status"))
        .ok_or_else(|| anyhow!("Missing userStatus field"))?;

    let account_email = non_empty_str(user_status.get("email"));

    // Prefer `userTier.name` over `planStatus.planInfo` (planInfo can report "Pro" for Ultra users).
    let tier_name = user_status
        .get("userTier")
        .or_else(|| user_status.get("user_tier"))
        .and_then(|t| {
            non_empty_str(t.get("name"))
                .or_else(|| non_empty_str(t.get("displayName")))
                .or_else(|| non_empty_str(t.get("display_name")))
        });

    let plan_info_name = user_status
        .get("planStatus")
        .or_else(|| user_status.get("plan_status"))
        .and_then(|ps| ps.get("planInfo").or_else(|| ps.get("plan_info")))
        .and_then(|pi| {
            non_empty_str(pi.get("planDisplayName"))
                .or_else(|| non_empty_str(pi.get("plan_display_name")))
                .or_else(|| non_empty_str(pi.get("displayName")))
                .or_else(|| non_empty_str(pi.get("display_name")))
                .or_else(|| non_empty_str(pi.get("planName")))
                .or_else(|| non_empty_str(pi.get("plan_name")))
                .or_else(|| non_empty_str(pi.get("productName")))
                .or_else(|| non_empty_str(pi.get("planShortName")))
        });

    let account_plan = tier_name.or(plan_info_name);

    let ai_credits = user_status
        .get("planStatus")
        .or_else(|| user_status.get("plan_status"))
        .and_then(|ps| {
            extract_number(ps.get("availablePromptCredits"))
                .or_else(|| extract_number(ps.get("availableFlowCredits")))
                .or_else(|| extract_number(ps.get("remainingCredits")))
        });

    let configs = user_status
        .get("cascadeModelConfigData")
        .or_else(|| user_status.get("cascade_model_config_data"))
        .and_then(|c| {
            c.get("clientModelConfigs")
                .or_else(|| c.get("client_model_configs"))
        })
        .and_then(Value::as_array)
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);

    let model_quotas = parse_model_configs(configs);
    let groups = group_legacy_model_quotas(&model_quotas, source);

    Ok(UserStatusParsed {
        account_email,
        account_plan,
        ai_credits,
        groups,
        model_quotas,
    })
}

/// Parses `LanguageServerService/GetCommandModelConfigs` JSON responses.
pub fn parse_command_model_configs_response(
    bytes: &[u8],
    source: SyncSource,
) -> Result<QuotaSnapshot> {
    let root: Value = serde_json::from_slice(bytes)?;
    check_rpc_code(&root)?;

    let configs = root
        .get("clientModelConfigs")
        .or_else(|| root.get("client_model_configs"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Missing clientModelConfigs"))?;

    let model_quotas = parse_model_configs(configs);
    let groups = group_legacy_model_quotas(&model_quotas, source);
    if model_quotas.is_empty() && groups.is_empty() {
        bail!("No model quotas in GetCommandModelConfigs response");
    }

    Ok(QuotaSnapshot {
        fetched_at: Utc::now(),
        source,
        account_email: None,
        account_plan: None,
        ai_credits: None,
        groups,
        model_quotas,
        summary_note: Some("Legacy model quota fallback".to_string()),
    })
}

/// Parses official Antigravity CLI `statusLine` JSON payload.
pub fn parse_statusline_json(bytes: &[u8]) -> Result<QuotaSnapshot> {
    let root: Value = serde_json::from_slice(bytes)?;
    let quota_obj = root
        .get("quota")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Missing quota map in statusLine JSON"))?;

    let account_email = non_empty_str(root.get("email"));
    let account_plan = non_empty_str(root.get("plan_tier"));

    let mut gemini_buckets = Vec::new();
    let mut claude_buckets = Vec::new();
    let mut argon_buckets = Vec::new();
    let mut other_buckets = Vec::new();

    let now = Utc::now();

    for (bucket_id, val) in quota_obj {
        let disabled = val
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let remaining_fraction = extract_fraction(val.get("remaining_fraction"))
            .or_else(|| extract_fraction(val.get("remainingFraction")));
        let reset_time = parse_date_value(val.get("reset_time"))
            .or_else(|| parse_date_value(val.get("resetTime")))
            .or_else(|| {
                val.get("reset_in_seconds")
                    .and_then(Value::as_i64)
                    .map(|secs| now + chrono::Duration::seconds(secs.max(0)))
            });

        let cadence = classify_cadence(None, bucket_id, bucket_id);
        let window_minutes = cadence.default_window_minutes();

        let bucket = QuotaBucket {
            bucket_id: bucket_id.clone(),
            display_name: prettify_bucket_id(bucket_id, &cadence),
            cadence,
            window_minutes,
            remaining_fraction,
            reset_time,
            reset_description: None,
            disabled,
        };

        let lower = bucket_id.to_ascii_lowercase();
        if lower.contains("argon") {
            argon_buckets.push(bucket);
        } else if lower.contains("claude") || lower.contains("3p") || lower.contains("gpt") {
            claude_buckets.push(bucket);
        } else if lower.contains("gemini") {
            gemini_buckets.push(bucket);
        } else {
            other_buckets.push(bucket);
        }
    }

    let mut groups = Vec::new();
    push_group_if_nonempty(
        &mut groups,
        "gemini",
        "Gemini Models",
        "Gemini",
        "gemini",
        gemini_buckets,
    );
    push_group_if_nonempty(
        &mut groups,
        "claude-gpt",
        "Claude and GPT models",
        "Claude/GPT",
        "claude",
        claude_buckets,
    );
    push_group_if_nonempty(
        &mut groups,
        "argon",
        "Gemini 4 Argon",
        "Argon",
        "argon",
        argon_buckets,
    );
    push_group_if_nonempty(
        &mut groups,
        "other",
        "Other Models",
        "Other",
        "other",
        other_buckets,
    );

    if groups.is_empty() {
        bail!("No quota buckets in statusLine JSON");
    }

    Ok(QuotaSnapshot {
        fetched_at: now,
        source: SyncSource::CliStatusLine,
        account_email,
        account_plan,
        ai_credits: None,
        groups,
        model_quotas: Vec::new(),
        summary_note: None,
    })
}

fn parse_quota_summary_payload(payload: &Value, source: SyncSource) -> Result<QuotaSnapshot> {
    let raw_groups = payload
        .get("groups")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Missing quota groups"))?;

    let summary_note = non_empty_str(payload.get("description"));
    let mut groups = Vec::new();

    for (idx, group_val) in raw_groups.iter().enumerate() {
        let display_name = non_empty_str(group_val.get("displayName"))
            .or_else(|| non_empty_str(group_val.get("display_name")))
            .or_else(|| non_empty_str(group_val.get("name")))
            .unwrap_or_else(|| format!("Group {}", idx + 1));

        let description = non_empty_str(group_val.get("description"));
        let raw_buckets = group_val
            .get("buckets")
            .and_then(Value::as_array)
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        let mut buckets = Vec::new();
        for b_val in raw_buckets {
            let bucket_id = non_empty_str(b_val.get("bucketId"))
                .or_else(|| non_empty_str(b_val.get("bucket_id")))
                .or_else(|| non_empty_str(b_val.get("id")));
            let b_display = non_empty_str(b_val.get("displayName"))
                .or_else(|| non_empty_str(b_val.get("display_name")))
                .or_else(|| non_empty_str(b_val.get("name")));

            let resolved_id = match (bucket_id, b_display.as_ref()) {
                (Some(id), _) => id,
                (None, Some(name)) => name.clone(),
                (None, None) => continue,
            };

            let window_raw = non_empty_str(b_val.get("window"));
            let resolved_display = b_display.unwrap_or_else(|| resolved_id.clone());
            let cadence = classify_cadence(window_raw.as_deref(), &resolved_id, &resolved_display);
            let window_minutes = cadence.default_window_minutes();

            let remaining_fraction = extract_fraction(b_val.get("remainingFraction"))
                .or_else(|| extract_fraction(b_val.get("remaining_fraction")))
                .or_else(|| {
                    b_val.get("remaining").and_then(|rem| {
                        extract_fraction(rem.get("remainingFraction"))
                            .or_else(|| extract_fraction(rem.get("remaining_fraction")))
                            .or_else(|| {
                                let case = rem.get("case").and_then(Value::as_str);
                                if case == Some("remainingFraction")
                                    || case == Some("remaining_fraction")
                                {
                                    extract_fraction(rem.get("value"))
                                } else {
                                    None
                                }
                            })
                    })
                });

            let reset_time = parse_date_value(b_val.get("resetTime"))
                .or_else(|| parse_date_value(b_val.get("reset_time")));
            let reset_description = non_empty_str(b_val.get("description"));
            let disabled = b_val
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            buckets.push(QuotaBucket {
                bucket_id: resolved_id,
                display_name: resolved_display,
                cadence,
                window_minutes,
                remaining_fraction,
                reset_time,
                reset_description,
                disabled,
            });
        }

        if buckets.is_empty() {
            continue;
        }

        buckets.sort_by_key(|b| b.cadence.sort_rank());
        let (short_label, accent_key) = classify_group_identity(&display_name);
        let group_id = slugify(&display_name);

        groups.push(QuotaGroup {
            id: group_id,
            display_name,
            short_label,
            accent_key,
            description,
            buckets,
        });
    }

    if groups.is_empty() {
        bail!("Quota summary payload contained no non-empty groups");
    }

    Ok(QuotaSnapshot {
        fetched_at: Utc::now(),
        source,
        account_email: None,
        account_plan: None,
        ai_credits: None,
        groups,
        model_quotas: Vec::new(),
        summary_note,
    })
}

fn parse_model_configs(configs: &[Value]) -> Vec<ModelQuotaRow> {
    let mut rows = Vec::new();
    for cfg in configs {
        let Some(quota_info) = cfg.get("quotaInfo").or_else(|| cfg.get("quota_info")) else {
            continue;
        };
        let label = non_empty_str(cfg.get("label")).unwrap_or_else(|| "Model".to_string());
        let model_id = cfg
            .get("modelOrAlias")
            .or_else(|| cfg.get("model_or_alias"))
            .and_then(|m| non_empty_str(m.get("model")))
            .unwrap_or_else(|| label.clone());

        let remaining_fraction = extract_fraction(quota_info.get("remainingFraction"))
            .or_else(|| extract_fraction(quota_info.get("remaining_fraction")));
        let reset_time = parse_date_value(quota_info.get("resetTime"))
            .or_else(|| parse_date_value(quota_info.get("reset_time")));

        rows.push(ModelQuotaRow {
            label,
            model_id,
            remaining_fraction,
            reset_time,
        });
    }
    rows
}

/// Converts flat per-model quotas from legacy `GetUserStatus` / `GetCommandModelConfigs`
/// into logical `QuotaGroup` rows so the UI displays a unified model card experience.
fn group_legacy_model_quotas(rows: &[ModelQuotaRow], _source: SyncSource) -> Vec<QuotaGroup> {
    if rows.is_empty() {
        return Vec::new();
    }

    let mut gemini_buckets = Vec::new();
    let mut claude_buckets = Vec::new();
    let mut argon_buckets = Vec::new();
    let mut other_buckets = Vec::new();

    for row in rows {
        let lower = row.label.to_ascii_lowercase();
        let model_lower = row.model_id.to_ascii_lowercase();
        let bucket = QuotaBucket {
            bucket_id: row.model_id.clone(),
            display_name: row.label.clone(),
            cadence: Cadence::Session,
            window_minutes: Some(300),
            remaining_fraction: row.remaining_fraction,
            reset_time: row.reset_time,
            reset_description: None,
            disabled: false,
        };

        if lower.contains("argon") || model_lower.contains("argon") {
            argon_buckets.push(bucket);
        } else if lower.contains("claude")
            || lower.contains("gpt")
            || model_lower.contains("claude")
            || model_lower.contains("gpt")
        {
            claude_buckets.push(bucket);
        } else if lower.contains("gemini") || model_lower.contains("gemini") {
            gemini_buckets.push(bucket);
        } else {
            other_buckets.push(bucket);
        }
    }

    let mut groups = Vec::new();
    push_group_if_nonempty(
        &mut groups,
        "gemini",
        "Gemini Models",
        "Gemini",
        "gemini",
        gemini_buckets,
    );
    push_group_if_nonempty(
        &mut groups,
        "claude-gpt",
        "Claude and GPT models",
        "Claude/GPT",
        "claude",
        claude_buckets,
    );
    push_group_if_nonempty(
        &mut groups,
        "argon",
        "Gemini 4 Argon",
        "Argon",
        "argon",
        argon_buckets,
    );
    push_group_if_nonempty(
        &mut groups,
        "other",
        "Other Models",
        "Other",
        "other",
        other_buckets,
    );
    groups
}

fn push_group_if_nonempty(
    out: &mut Vec<QuotaGroup>,
    id: &str,
    display_name: &str,
    short_label: &str,
    accent_key: &str,
    mut buckets: Vec<QuotaBucket>,
) {
    if buckets.is_empty() {
        return;
    }
    buckets.sort_by_key(|b| b.cadence.sort_rank());
    out.push(QuotaGroup {
        id: id.to_string(),
        display_name: display_name.to_string(),
        short_label: short_label.to_string(),
        accent_key: accent_key.to_string(),
        description: None,
        buckets,
    });
}

fn check_rpc_code(root: &Value) -> Result<()> {
    let Some(code) = root.get("code") else {
        return Ok(());
    };
    if let Some(n) = code.as_i64() {
        if n == 0 {
            return Ok(());
        }
        bail!("RPC error code {n}");
    }
    if let Some(s) = code.as_str() {
        let norm = s.trim();
        if norm.is_empty() || norm == "0" || norm.eq_ignore_ascii_case("ok") {
            return Ok(());
        }
        bail!("RPC error code {norm}");
    }
    Ok(())
}

fn classify_cadence(window: Option<&str>, bucket_id: &str, display_name: &str) -> Cadence {
    let haystacks = [
        window.unwrap_or_default().to_ascii_lowercase(),
        bucket_id.to_ascii_lowercase(),
        display_name.to_ascii_lowercase(),
    ];

    for h in &haystacks {
        if h.contains("5h")
            || h.contains("5-hour")
            || h.contains("five-hour")
            || h.contains("five hour")
        {
            return Cadence::FiveHour;
        }
        if h.contains("weekly") || h.contains("7d") || h.contains("week") {
            return Cadence::Weekly;
        }
        if h.contains("session") {
            return Cadence::Session;
        }
    }

    if let Some(w) = window.filter(|s| !s.is_empty()) {
        return Cadence::Other(w.to_string());
    }
    Cadence::Other("Limit".to_string())
}

fn classify_group_identity(display_name: &str) -> (String, String) {
    let lower = display_name.to_ascii_lowercase();
    if lower.contains("argon") {
        ("Argon".to_string(), "argon".to_string())
    } else if lower.contains("claude") || lower.contains("gpt") || lower.contains("3p") {
        ("Claude/GPT".to_string(), "claude".to_string())
    } else if lower.contains("gemini") {
        ("Gemini".to_string(), "gemini".to_string())
    } else {
        let short = display_name
            .split_whitespace()
            .next()
            .unwrap_or("Quota")
            .to_string();
        (short, "other".to_string())
    }
}

fn prettify_bucket_id(bucket_id: &str, cadence: &Cadence) -> String {
    match cadence {
        Cadence::FiveHour => "5-hour limit".to_string(),
        Cadence::Session => "Session limit".to_string(),
        Cadence::Weekly => "Weekly limit".to_string(),
        Cadence::Other(_) => bucket_id.to_string(),
    }
}

fn non_empty_str(val: Option<&Value>) -> Option<String> {
    val.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_number(val: Option<&Value>) -> Option<f64> {
    let v = val?;
    if let Some(n) = v.as_f64() {
        return n.is_finite().then_some(n);
    }
    if let Some(s) = v.as_str() {
        if let Ok(n) = s.trim().parse::<f64>() {
            return n.is_finite().then_some(n);
        }
    }
    None
}

fn extract_fraction(val: Option<&Value>) -> Option<f64> {
    extract_number(val).map(|n| n.clamp(0.0, 1.0))
}

fn parse_date_value(val: Option<&Value>) -> Option<DateTime<Utc>> {
    let v = val?;
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
            return Some(dt.with_timezone(&Utc));
        }
        if let Ok(secs) = trimmed.parse::<f64>() {
            return timestamp_from_number(secs);
        }
        return None;
    }
    if let Some(secs) = v.as_f64() {
        return timestamp_from_number(secs);
    }
    None
}

fn timestamp_from_number(n: f64) -> Option<DateTime<Utc>> {
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    // Heuristic: treat values > 1e12 as milliseconds, otherwise seconds.
    let secs = if n > 1e12 { n / 1000.0 } else { n };
    let whole = secs.trunc() as i64;
    let nanos = ((secs.fract()) * 1e9).round() as u32;
    Utc.timestamp_opt(whole, nanos).single()
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "group".to_string()
    } else {
        out
    }
}
