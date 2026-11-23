//! Async Connect-RPC client for local Antigravity quota retrieval with intelligent endpoint caching
//! and safe CLI fallbacks.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Result};
use reqwest::Client;
use serde_json::json;
use tokio::process::Command;
use tokio::sync::Mutex;

use super::discovery::{
    build_candidate_endpoints, discover_listening_ports, discover_processes, find_agy_binary,
    DiscoveredProcess, ProcessKind, RpcEndpoint,
};
use super::parser::{
    parse_cli_usage_report, parse_command_model_configs_response, parse_quota_summary_response,
    parse_statusline_json, parse_user_status_response,
};
use super::{QuotaSnapshot, SyncSource};
use crate::config::live_statusline_path;

const UNLEASH_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetUnleashData";
const QUOTA_SUMMARY_PATH: &str =
    "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";
const PREDICTION_QUOTA_SUMMARY_PATH: &str =
    "/exa.language_server_pb.PredictionService/RetrieveUserQuotaSummary";
const GET_USER_STATUS_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetUserStatus";
const COMMAND_MODEL_CONFIGS_PATH: &str =
    "/exa.language_server_pb.LanguageServerService/GetCommandModelConfigs";

#[derive(Debug, Clone)]
struct CachedEndpoint {
    #[allow(dead_code)]
    pid: u32,
    kind: ProcessKind,
    endpoint: RpcEndpoint,
}

/// Loopback Connect-RPC client for querying live Antigravity quota state.
#[derive(Clone)]
pub struct QuotaClient {
    http: Client,
    cached: Arc<Mutex<Option<CachedEndpoint>>>,
}

impl Default for QuotaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaClient {
    /// Builds a loopback-only HTTP/HTTPS client bypassing system proxies (important when local
    /// HTTP proxies like Clash/Surge are active) and accepting Antigravity's localhost self-signed cert.
    pub fn new() -> Self {
        let http = Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .connect_timeout(Duration::from_millis(900))
            .timeout(Duration::from_millis(2500))
            .build()
            .expect("reqwest client build should succeed");

        Self {
            http,
            cached: Arc::new(Mutex::new(None)),
        }
    }

    /// Invalidates any cached working endpoint so the next fetch performs fresh discovery.
    pub async fn invalidate_cache(&self) {
        let mut guard = self.cached.lock().await;
        *guard = None;
    }

    /// Fetches the best live quota snapshot using fast endpoint cache, process discovery,
    /// fresh CLI statusLine cache, or `agy -p /usage` fallback.
    pub async fn fetch_live_quota(&self) -> Result<QuotaSnapshot> {
        // 1. Fast path: try the previously verified working localhost endpoint.
        let maybe_cached = { self.cached.lock().await.clone() };
        if let Some(cached) = maybe_cached {
            if let Ok(snap) = self
                .fetch_from_endpoint(&cached.endpoint, cached.kind.sync_source())
                .await
            {
                return Ok(snap);
            }
            self.invalidate_cache().await;
        }

        // 2. Full process + port discovery across running Antigravity App / CLI / IDE servers.
        let processes = discover_processes().await.unwrap_or_default();
        let mut best_snapshot: Option<QuotaSnapshot> = None;
        let mut best_cached: Option<CachedEndpoint> = None;
        let mut last_err: Option<anyhow::Error> = None;

        for proc in processes.into_iter().take(4) {
            match self.probe_process(&proc).await {
                Ok((snap, ep)) => {
                    let is_better = best_snapshot
                        .as_ref()
                        .is_none_or(|cur| snap.quality_score() > cur.quality_score());
                    if is_better {
                        best_snapshot = Some(snap);
                        best_cached = Some(CachedEndpoint {
                            pid: proc.pid,
                            kind: proc.kind,
                            endpoint: ep,
                        });
                    }
                    // If we already got a rich AppLocal quota summary, no need to probe lower-priority IDE servers.
                    if best_snapshot.as_ref().is_some_and(|s| {
                        s.source == SyncSource::AppLocal && s.quality_score() >= 1000
                    }) {
                        break;
                    }
                }
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }

        if let (Some(snap), Some(cached)) = (best_snapshot, best_cached) {
            let mut guard = self.cached.lock().await;
            *guard = Some(cached);
            return Ok(snap);
        }

        // 3. Fresh CLI statusLine JSON ingested by `antigravity-island ingest-statusline`.
        if let Ok(snap) = read_fresh_statusline_snapshot(Duration::from_secs(120)).await {
            return Ok(snap);
        }

        // 4. Optional `agy -p /usage --output-format json` fallback if `agy` is installed.
        if let Some(agy_bin) = find_agy_binary() {
            if let Ok(snap) = fetch_via_agy_print_usage(&agy_bin).await {
                return Ok(snap);
            }
        }

        Err(last_err.unwrap_or_else(|| {
            anyhow!("Antigravity is not running (no active language_server or agy CLI found)")
        }))
    }

    async fn probe_process(
        &self,
        proc: &DiscoveredProcess,
    ) -> Result<(QuotaSnapshot, RpcEndpoint)> {
        let source = proc.kind.sync_source();

        // Fast path for daemon metadata files (`hub_*.json`) with explicit HTTPS/HTTP ports:
        // skip `lsof` completely unless explicit ports fail.
        if proc.explicit_https_port.is_some() || proc.explicit_http_port.is_some() {
            let direct = build_candidate_endpoints(proc, &[]);
            for ep in &direct {
                if let Ok(snap) = self.fetch_from_endpoint(ep, source).await {
                    return Ok((snap, ep.clone()));
                }
            }
        }

        let listening = discover_listening_ports(proc.pid).await.unwrap_or_default();
        let candidates = build_candidate_endpoints(proc, &listening);
        if candidates.is_empty() {
            bail!("No candidate ports found for PID {}", proc.pid);
        }

        let working = self.resolve_working_endpoint(&candidates).await;

        let mut ordered = Vec::with_capacity(candidates.len());
        if let Some(ep) = working {
            ordered.push(ep);
        }
        for c in candidates {
            if !ordered.contains(&c) {
                ordered.push(c);
            }
        }

        let mut last_err = None;
        for ep in ordered {
            match self.fetch_from_endpoint(&ep, source).await {
                Ok(snap) => return Ok((snap, ep)),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Failed to query PID {}", proc.pid)))
    }

    async fn resolve_working_endpoint(&self, candidates: &[RpcEndpoint]) -> Option<RpcEndpoint> {
        let body = unleash_request_body();
        for ep in candidates {
            if let Ok(resp) = self.post_raw(ep, UNLEASH_PATH, &body).await {
                let status = resp.status().as_u16();
                if status > 0 && status < 500 {
                    return Some(ep.clone());
                }
            }
        }
        None
    }

    /// Queries a single known endpoint using Antigravity's Connect-RPC fallback hierarchy:
    /// 1) `RetrieveUserQuotaSummary` (+ `PredictionService` alias) with `GetUserStatus` identity enrichment
    /// 2) `GetUserStatus` legacy model quotas
    /// 3) `GetCommandModelConfigs` legacy model quotas
    pub async fn fetch_from_endpoint(
        &self,
        ep: &RpcEndpoint,
        source: SyncSource,
    ) -> Result<QuotaSnapshot> {
        let summary_body = json!({ "forceRefresh": true });
        for path in [QUOTA_SUMMARY_PATH, PREDICTION_QUOTA_SUMMARY_PATH] {
            if let Ok(bytes) = self.post_json_ok(ep, path, &summary_body).await {
                if let Ok(mut snap) = parse_quota_summary_response(&bytes, source) {
                    if snap.has_measured_quota() {
                        // Best-effort identity & plan tier enrichment (non-fatal).
                        if let Ok(status_bytes) = self
                            .post_json_ok(ep, GET_USER_STATUS_PATH, &default_metadata_body())
                            .await
                        {
                            if let Ok(identity) = parse_user_status_response(&status_bytes, source)
                            {
                                snap.account_email = identity.account_email;
                                snap.account_plan = identity.account_plan;
                                snap.ai_credits = identity.ai_credits;
                            }
                        }
                        return Ok(snap);
                    }
                }
            }
        }

        // Fallback 1: GetUserStatus
        if let Ok(status_bytes) = self
            .post_json_ok(ep, GET_USER_STATUS_PATH, &default_metadata_body())
            .await
        {
            if let Ok(parsed) = parse_user_status_response(&status_bytes, source) {
                let snap = QuotaSnapshot {
                    fetched_at: chrono::Utc::now(),
                    source,
                    account_email: parsed.account_email,
                    account_plan: parsed.account_plan,
                    ai_credits: parsed.ai_credits,
                    groups: parsed.groups,
                    model_quotas: parsed.model_quotas,
                    summary_note: None,
                };
                if snap.has_measured_quota() {
                    return Ok(snap);
                }
            }
        }

        // Fallback 2: GetCommandModelConfigs
        let cmd_bytes = self
            .post_json_ok(ep, COMMAND_MODEL_CONFIGS_PATH, &default_metadata_body())
            .await?;
        let snap = parse_command_model_configs_response(&cmd_bytes, source)?;
        if !snap.has_measured_quota() {
            bail!("Quota response contained no measured quota buckets");
        }
        Ok(snap)
    }

    async fn post_json_ok(
        &self,
        ep: &RpcEndpoint,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<Vec<u8>> {
        let resp = self.post_raw(ep, path, body).await?;
        let status = resp.status();
        let bytes = resp.bytes().await?.to_vec();
        if !status.is_success() {
            let preview = String::from_utf8_lossy(&bytes);
            bail!(
                "HTTP {} on {}: {}",
                status.as_u16(),
                path,
                preview.chars().take(160).collect::<String>()
            );
        }
        Ok(bytes)
    }

    async fn post_raw(
        &self,
        ep: &RpcEndpoint,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let url = ep.url(path);
        // Strict security invariant: never transmit local CSRF tokens anywhere except loopback.
        let parsed = reqwest::Url::parse(&url)?;
        if parsed.host_str() != Some("127.0.0.1") {
            bail!("Refusing non-loopback RPC request to {}", url);
        }

        let mut req = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("Connect-Protocol-Version", "1")
            .json(body);

        if !ep.csrf_token.is_empty() {
            req = req.header("X-Codeium-Csrf-Token", &ep.csrf_token);
        }

        Ok(req.send().await?)
    }
}

fn default_metadata_body() -> serde_json::Value {
    json!({
        "metadata": {
            "ideName": "antigravity",
            "extensionName": "antigravity",
            "ideVersion": "unknown",
            "locale": "en"
        }
    })
}

fn unleash_request_body() -> serde_json::Value {
    json!({
        "context": {
            "properties": {
                "devMode": "false",
                "extensionVersion": "unknown",
                "hasAnthropicModelAccess": "true",
                "ide": "antigravity",
                "ideVersion": "unknown",
                "installationId": "antigravity-island",
                "language": "UNSPECIFIED",
                "os": std::env::consts::OS,
                "requestedModelId": "MODEL_UNSPECIFIED"
            }
        }
    })
}

async fn read_fresh_statusline_snapshot(max_age: Duration) -> Result<QuotaSnapshot> {
    let path = live_statusline_path();
    let meta = tokio::fs::metadata(&path).await?;
    let modified = meta.modified()?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::MAX);
    if age > max_age {
        bail!(
            "Cached CLI statusLine JSON is stale ({}s old)",
            age.as_secs()
        );
    }
    let bytes = tokio::fs::read(&path).await?;
    parse_statusline_json(&bytes)
}

async fn fetch_via_agy_print_usage(agy_bin: &std::path::Path) -> Result<QuotaSnapshot> {
    let tmp_dir =
        std::env::temp_dir().join(format!("antigravity-island-probe-{}", std::process::id()));
    let _ = tokio::fs::create_dir_all(&tmp_dir).await;

    let output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(agy_bin)
            .args(["-p", "/usage", "--output-format", "json"])
            .current_dir(&tmp_dir)
            .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true")
            .env("BROWSER", "/usr/bin/false")
            .output(),
    )
    .await??;

    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;

    if !output.status.success() {
        bail!("agy -p /usage exited with {}", output.status);
    }
    parse_cli_usage_report(&output.stdout)
}
