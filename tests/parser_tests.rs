use antigravity_island::quota::parser::{
    parse_cli_usage_report, parse_command_model_configs_response, parse_quota_summary_response,
    parse_statusline_json, parse_user_status_response,
};
use antigravity_island::quota::{Cadence, SyncSource};
use chrono::{Duration, Utc};

#[test]
fn parses_nested_response_groups_and_sorts_five_hour_before_weekly() {
    let payload = r#"{
        "response": {
            "description": "Google AI Ultra",
            "groups": [
                {
                    "displayName": "Gemini Models",
                    "buckets": [
                        {
                            "bucketId": "gemini-weekly",
                            "displayName": "Weekly Limit",
                            "window": "weekly",
                            "remainingFraction": 0.55,
                            "resetTime": "2026-11-30T08:45:39Z"
                        },
                        {
                            "bucketId": "gemini-5h",
                            "displayName": "5-Hour Limit",
                            "window": "5h",
                            "remainingFraction": 0.86,
                            "resetTime": "2026-11-23T15:39:34Z"
                        }
                    ]
                },
                {
                    "displayName": "Claude and GPT models",
                    "buckets": [
                        {
                            "bucketId": "3p-5h",
                            "displayName": "Session",
                            "remaining": { "remainingFraction": 0.73 },
                            "resetTime": "2026-11-23T14:52:10Z"
                        },
                        {
                            "bucketId": "3p-weekly",
                            "displayName": "Weekly",
                            "remaining": { "case": "remainingFraction", "value": 0.64 },
                            "resetTime": "2026-11-29T00:39:54Z"
                        }
                    ]
                },
                {
                    "displayName": "Gemini 4 Argon",
                    "buckets": [
                        {
                            "bucketId": "argon-weekly",
                            "displayName": "Weekly",
                            "remaining_fraction": "0.91"
                        }
                    ]
                }
            ]
        }
    }"#;

    let snap = parse_quota_summary_response(payload.as_bytes(), SyncSource::AppLocal).unwrap();
    assert_eq!(snap.groups.len(), 3);
    assert_eq!(snap.groups[0].accent_key, "gemini");
    assert_eq!(snap.groups[0].buckets[0].cadence, Cadence::FiveHour);
    assert_eq!(snap.groups[0].buckets[0].remaining_percent(), Some(86));
    assert_eq!(snap.groups[0].buckets[1].cadence, Cadence::Weekly);
    assert_eq!(snap.groups[0].buckets[1].remaining_percent(), Some(55));

    assert_eq!(snap.groups[1].accent_key, "claude");
    assert_eq!(snap.groups[1].buckets[0].remaining_percent(), Some(73));
    assert_eq!(snap.groups[1].buckets[1].remaining_percent(), Some(64));

    assert_eq!(snap.groups[2].accent_key, "argon");
    assert_eq!(snap.groups[2].buckets[0].remaining_percent(), Some(91));

    assert_eq!(snap.min_remaining_percent(), Some(55));
    assert_eq!(snap.tray_summary(), "G 86%/55% · C 73%/64%");
    let tip = snap.tray_tooltip(Utc::now());
    assert!(tip.contains("Gemini: 5h 86%"));
    assert!(tip.contains("7d 55%"));
    assert!(tip.contains("Claude/GPT: 5h 73%"));
}

#[test]
fn parses_top_level_groups_and_cadence_aliases() {
    for alias in ["session", "5h", "5-hour", "five hour", "five-hour"] {
        let json = format!(
            r#"{{
                "groups": [
                    {{
                        "displayName": "Gemini Models",
                        "buckets": [
                            {{
                                "bucketId": "gemini-{alias}",
                                "displayName": "{alias}",
                                "remaining": {{ "remainingFraction": 0.75 }}
                            }}
                        ]
                    }}
                ]
            }}"#
        );
        let snap = parse_quota_summary_response(json.as_bytes(), SyncSource::CliLocal).unwrap();
        let b = &snap.groups[0].buckets[0];
        assert_eq!(b.remaining_percent(), Some(75));
        assert!(matches!(b.cadence, Cadence::FiveHour | Cadence::Session));
    }
}

#[test]
fn rejects_nonzero_rpc_error_code() {
    let err_json = br#"{"code": 16, "message": "unauthenticated"}"#;
    let err = parse_quota_summary_response(err_json, SyncSource::AppLocal).unwrap_err();
    assert!(err.to_string().contains("16"));
}

#[test]
fn parses_user_status_prefers_user_tier_over_plan_info() {
    let json = br#"{
        "code": 0,
        "userStatus": {
            "email": "dev@example.com",
            "userTier": { "name": "Google AI Ultra" },
            "planStatus": {
                "availablePromptCredits": 250,
                "planInfo": { "planName": "Pro", "planDisplayName": "Google AI Pro" }
            },
            "cascadeModelConfigData": {
                "clientModelConfigs": [
                    {
                        "label": "Gemini 4 Pro",
                        "modelOrAlias": { "model": "gemini-4-pro" },
                        "quotaInfo": {
                            "remainingFraction": 0.8,
                            "resetTime": "2026-11-23T18:00:00Z"
                        }
                    },
                    {
                        "label": "Claude 4.5 Sonnet",
                        "modelOrAlias": { "model": "claude-4-5-sonnet" },
                        "quotaInfo": {
                            "remainingFraction": 0.45,
                            "resetTime": 1795450000
                        }
                    }
                ]
            }
        }
    }"#;

    let parsed = parse_user_status_response(json, SyncSource::IdeLocal).unwrap();
    assert_eq!(parsed.account_email.as_deref(), Some("dev@example.com"));
    assert_eq!(parsed.account_plan.as_deref(), Some("Google AI Ultra"));
    assert_eq!(parsed.ai_credits, Some(250.0));
    assert_eq!(parsed.model_quotas.len(), 2);
    assert_eq!(parsed.groups.len(), 2);
}

#[test]
fn parses_command_model_configs_fallback() {
    let json = br#"{
        "clientModelConfigs": [
            {
                "label": "Gemini 3.8 Flash",
                "modelOrAlias": { "model": "gemini-3-8-flash" },
                "quotaInfo": { "remainingFraction": 0.92 }
            }
        ]
    }"#;

    let snap = parse_command_model_configs_response(json, SyncSource::IdeLocal).unwrap();
    assert!(snap.has_measured_quota());
    assert_eq!(snap.groups[0].buckets[0].remaining_percent(), Some(92));
}

#[test]
fn parses_agy_cli_usage_report_and_statusline_json() {
    let cli_report = br#"{
        "status": "SUCCESS",
        "command": {
            "name": "usage",
            "data": {
                "groups": [
                    {
                        "displayName": "Gemini Models",
                        "buckets": [
                            {
                                "bucketId": "gemini-5h",
                                "displayName": "5h",
                                "remainingFraction": 0.88
                            }
                        ]
                    }
                ]
            }
        }
    }"#;
    let snap = parse_cli_usage_report(cli_report).unwrap();
    assert_eq!(snap.source, SyncSource::CliPrintUsage);
    assert_eq!(snap.groups[0].buckets[0].remaining_percent(), Some(88));

    let statusline = br#"{
        "plan_tier": "Pro",
        "email": "cli@example.com",
        "quota": {
            "gemini-weekly": {
                "remaining_fraction": 0.9378,
                "reset_time": "2026-11-30T07:50:32Z",
                "reset_in_seconds": 560580
            },
            "3p-5h": {
                "remaining_fraction": 0.62,
                "reset_in_seconds": 9000
            }
        }
    }"#;
    let sl = parse_statusline_json(statusline).unwrap();
    assert_eq!(sl.source, SyncSource::CliStatusLine);
    assert_eq!(sl.account_plan.as_deref(), Some("Pro"));
    assert_eq!(sl.groups.len(), 2);
}

#[test]
fn calculates_pace_delta_accurately() {
    let now = Utc::now();
    let payload = serde_json::json!({
        "groups": [{
            "displayName": "Gemini Models",
            "buckets": [{
                "bucketId": "gemini-5h",
                "displayName": "5h",
                "window": "5h",
                "remainingFraction": 0.80,
                "resetTime": (now + Duration::minutes(150)).to_rfc3339()
            }]
        }]
    });
    let snap =
        parse_quota_summary_response(payload.to_string().as_bytes(), SyncSource::AppLocal).unwrap();
    // 150m left out of 300m -> expected remaining is 50%; actual is 80% -> pace delta ~ +30%
    let delta = snap.groups[0].buckets[0].pace_delta_pct(now).unwrap();
    assert!((29..=31).contains(&delta), "unexpected pace delta: {delta}");
}
