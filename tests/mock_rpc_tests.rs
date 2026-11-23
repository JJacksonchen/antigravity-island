use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

use antigravity_island::quota::client::QuotaClient;
use antigravity_island::quota::discovery::RpcEndpoint;
use antigravity_island::quota::SyncSource;

#[derive(Clone)]
struct MockServerState {
    expected_token: String,
    mode: MockMode,
    seen_paths: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Copy)]
enum MockMode {
    SummaryWithIdentity,
    PredictionAliasFallback,
    UserStatusFallback,
    CommandModelFallback,
}

async fn spawn_mock_server(mode: MockMode) -> (u16, Arc<Mutex<Vec<String>>>) {
    let seen_paths = Arc::new(Mutex::new(Vec::new()));
    let state = MockServerState {
        expected_token: "test-csrf-123".to_string(),
        mode,
        seen_paths: seen_paths.clone(),
    };

    let app = Router::new()
        .route(
            "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary",
            post(handle_summary),
        )
        .route(
            "/exa.language_server_pb.PredictionService/RetrieveUserQuotaSummary",
            post(handle_prediction_summary),
        )
        .route(
            "/exa.language_server_pb.LanguageServerService/GetUserStatus",
            post(handle_user_status),
        )
        .route(
            "/exa.language_server_pb.LanguageServerService/GetCommandModelConfigs",
            post(handle_command_configs),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (port, seen_paths)
}

fn check_headers(headers: &HeaderMap, expected: &str) -> Result<(), (StatusCode, Json<Value>)> {
    let tok = headers
        .get("x-codeium-csrf-token")
        .and_then(|v| v.to_str().ok());
    let proto = headers
        .get("connect-protocol-version")
        .and_then(|v| v.to_str().ok());
    if tok != Some(expected) || proto != Some("1") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "missing or invalid headers"})),
        ));
    }
    Ok(())
}

async fn handle_summary(
    State(st): State<MockServerState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = check_headers(&headers, &st.expected_token) {
        return e;
    }
    st.seen_paths
        .lock()
        .unwrap()
        .push("RetrieveUserQuotaSummary".to_string());

    match st.mode {
        MockMode::SummaryWithIdentity => (
            StatusCode::OK,
            Json(json!({
                "response": {
                    "groups": [
                        {
                            "displayName": "Gemini Models",
                            "buckets": [
                                {
                                    "bucketId": "gemini-5h",
                                    "displayName": "5h",
                                    "window": "5h",
                                    "remainingFraction": 0.84,
                                    "resetTime": "2026-11-23T18:00:00Z"
                                },
                                {
                                    "bucketId": "gemini-weekly",
                                    "displayName": "Weekly",
                                    "window": "weekly",
                                    "remainingFraction": 0.62,
                                    "resetTime": "2026-11-30T08:00:00Z"
                                }
                            ]
                        },
                        {
                            "displayName": "Claude and GPT models",
                            "buckets": [
                                {
                                    "bucketId": "3p-5h",
                                    "displayName": "5h",
                                    "window": "5h",
                                    "remainingFraction": 0.95
                                }
                            ]
                        }
                    ]
                }
            })),
        ),
        _ => (StatusCode::NOT_FOUND, Json(json!({ "code": "not_found" }))),
    }
}

async fn handle_prediction_summary(
    State(st): State<MockServerState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = check_headers(&headers, &st.expected_token) {
        return e;
    }
    st.seen_paths
        .lock()
        .unwrap()
        .push("PredictionService/RetrieveUserQuotaSummary".to_string());

    match st.mode {
        MockMode::PredictionAliasFallback => (
            StatusCode::OK,
            Json(json!({
                "groups": [{
                    "displayName": "Gemini Models",
                    "buckets": [{
                        "bucketId": "gemini-5h",
                        "displayName": "5h",
                        "remainingFraction": 0.77
                    }]
                }]
            })),
        ),
        _ => (StatusCode::NOT_FOUND, Json(json!({}))),
    }
}

async fn handle_user_status(
    State(st): State<MockServerState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = check_headers(&headers, &st.expected_token) {
        return e;
    }
    st.seen_paths
        .lock()
        .unwrap()
        .push("GetUserStatus".to_string());

    match st.mode {
        MockMode::SummaryWithIdentity | MockMode::PredictionAliasFallback => (
            StatusCode::OK,
            Json(json!({
                "userStatus": {
                    "email": "tester@example.com",
                    "userTier": { "name": "Google AI Ultra" }
                }
            })),
        ),
        MockMode::UserStatusFallback => (
            StatusCode::OK,
            Json(json!({
                "userStatus": {
                    "email": "ide@example.com",
                    "planStatus": { "planInfo": { "planDisplayName": "Google AI Pro" } },
                    "cascadeModelConfigData": {
                        "clientModelConfigs": [{
                            "label": "Gemini 4 Pro",
                            "modelOrAlias": { "model": "gemini-4-pro" },
                            "quotaInfo": { "remainingFraction": 0.51 }
                        }]
                    }
                }
            })),
        ),
        MockMode::CommandModelFallback => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({}))),
    }
}

async fn handle_command_configs(
    State(st): State<MockServerState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = check_headers(&headers, &st.expected_token) {
        return e;
    }
    st.seen_paths
        .lock()
        .unwrap()
        .push("GetCommandModelConfigs".to_string());

    (
        StatusCode::OK,
        Json(json!({
            "clientModelConfigs": [{
                "label": "Claude 4.5 Opus",
                "modelOrAlias": { "model": "claude-4-5-opus" },
                "quotaInfo": { "remainingFraction": 0.39 }
            }]
        })),
    )
}

#[tokio::test]
async fn mock_rpc_primary_summary_with_identity_enrichment() {
    let (port, seen) = spawn_mock_server(MockMode::SummaryWithIdentity).await;
    let client = QuotaClient::new();
    let ep = RpcEndpoint {
        scheme: "http",
        port,
        csrf_token: "test-csrf-123".to_string(),
        is_extension_server: false,
    };

    let snap = client
        .fetch_from_endpoint(&ep, SyncSource::AppLocal)
        .await
        .unwrap();

    assert_eq!(snap.account_email.as_deref(), Some("tester@example.com"));
    assert_eq!(snap.account_plan.as_deref(), Some("Google AI Ultra"));
    assert_eq!(snap.groups.len(), 2);
    assert_eq!(snap.groups[0].buckets[0].remaining_percent(), Some(84));

    let calls = seen.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            "RetrieveUserQuotaSummary".to_string(),
            "GetUserStatus".to_string()
        ]
    );
}

#[tokio::test]
async fn mock_rpc_prediction_alias_fallback() {
    let (port, _) = spawn_mock_server(MockMode::PredictionAliasFallback).await;
    let client = QuotaClient::new();
    let ep = RpcEndpoint {
        scheme: "http",
        port,
        csrf_token: "test-csrf-123".to_string(),
        is_extension_server: false,
    };

    let snap = client
        .fetch_from_endpoint(&ep, SyncSource::AppLocal)
        .await
        .unwrap();
    assert_eq!(snap.groups[0].buckets[0].remaining_percent(), Some(77));
    assert_eq!(snap.account_plan.as_deref(), Some("Google AI Ultra"));
}

#[tokio::test]
async fn mock_rpc_legacy_user_status_and_command_model_fallbacks() {
    let (port1, _) = spawn_mock_server(MockMode::UserStatusFallback).await;
    let client = QuotaClient::new();
    let ep1 = RpcEndpoint {
        scheme: "http",
        port: port1,
        csrf_token: "test-csrf-123".to_string(),
        is_extension_server: false,
    };
    let snap1 = client
        .fetch_from_endpoint(&ep1, SyncSource::IdeLocal)
        .await
        .unwrap();
    assert_eq!(snap1.account_plan.as_deref(), Some("Google AI Pro"));
    assert_eq!(snap1.groups[0].buckets[0].remaining_percent(), Some(51));

    let (port2, _) = spawn_mock_server(MockMode::CommandModelFallback).await;
    let ep2 = RpcEndpoint {
        scheme: "http",
        port: port2,
        csrf_token: "test-csrf-123".to_string(),
        is_extension_server: false,
    };
    let snap2 = client
        .fetch_from_endpoint(&ep2, SyncSource::IdeLocal)
        .await
        .unwrap();
    assert_eq!(snap2.groups[0].short_label, "Claude/GPT");
    assert_eq!(snap2.groups[0].buckets[0].remaining_percent(), Some(39));
}

#[tokio::test]
async fn mock_rpc_rejects_wrong_csrf_token() {
    let (port, _) = spawn_mock_server(MockMode::SummaryWithIdentity).await;
    let client = QuotaClient::new();
    let ep = RpcEndpoint {
        scheme: "http",
        port,
        csrf_token: "wrong-token".to_string(),
        is_extension_server: false,
    };

    let err = client
        .fetch_from_endpoint(&ep, SyncSource::AppLocal)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("401"));
}
