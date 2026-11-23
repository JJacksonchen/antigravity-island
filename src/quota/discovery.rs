//! Local Antigravity process, daemon metadata (`hub_*.json`), and loopback port discovery.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use tokio::process::Command;

use super::SyncSource;

const MAX_DAEMON_JSON_BYTES: u64 = 64 * 1024;

/// Category of local Antigravity process hosting a Connect-RPC server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessKind {
    /// Antigravity 2.0+ / Gemini desktop app language server (rich `RetrieveUserQuotaSummary`).
    App = 0,
    /// `agy` / `antigravity-cli` interactive process.
    Cli = 1,
    /// Antigravity IDE extension language server (`GetUserStatus` session models).
    Ide = 2,
}

impl ProcessKind {
    pub fn sync_source(self) -> SyncSource {
        match self {
            Self::App => SyncSource::AppLocal,
            Self::Cli => SyncSource::CliLocal,
            Self::Ide => SyncSource::IdeLocal,
        }
    }
}

/// Discovered running Antigravity server process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProcess {
    pub pid: u32,
    pub kind: ProcessKind,
    pub command: String,
    pub csrf_token: String,
    pub explicit_https_port: Option<u16>,
    pub explicit_http_port: Option<u16>,
    pub extension_server_port: Option<u16>,
    pub extension_server_csrf_token: Option<String>,
    pub hint_ports: Vec<u16>,
}

/// Parsed connection metadata from Antigravity 2.34+ `~/.gemini/antigravity/daemon/hub_*.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DaemonRecord {
    pub pid: u32,
    #[serde(alias = "httpsPort", alias = "https_port", default)]
    pub https_port: Option<u16>,
    #[serde(alias = "httpPort", alias = "http_port", default)]
    pub http_port: Option<u16>,
    #[serde(alias = "csrfToken", alias = "csrf_token", default)]
    pub csrf_token: Option<String>,
    #[serde(alias = "lsVersion", alias = "ls_version", default)]
    pub ls_version: Option<String>,
}

/// Concrete local RPC connection target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcEndpoint {
    pub scheme: &'static str,
    pub port: u16,
    pub csrf_token: String,
    pub is_extension_server: bool,
}

impl RpcEndpoint {
    pub fn url(&self, path: &str) -> String {
        format!("{}://127.0.0.1:{}{}", self.scheme, self.port, path)
    }
}

/// Discovers running Antigravity processes using both daemon metadata files (`hub_*.json`)
/// and `ps -ax -o pid=,command=` process arguments.
pub async fn discover_processes() -> Result<Vec<DiscoveredProcess>> {
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        Command::new("/bin/ps")
            .args(["-ax", "-o", "pid=,command="])
            .output(),
    )
    .await??;

    if !output.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let alive_pids = parse_alive_pids(&stdout);

    let mut merged: Vec<DiscoveredProcess> = Vec::new();
    let mut seen_pids: HashSet<u32> = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        let gemini_dir = home.join(".gemini");
        for (subdir, kind) in [
            ("antigravity", ProcessKind::App),
            ("antigravity-cli", ProcessKind::Cli),
            ("antigravity-ide", ProcessKind::Ide),
        ] {
            let daemon_dir = gemini_dir.join(subdir).join("daemon");
            for proc in discover_daemon_processes_in_dir(&daemon_dir, kind, &alive_pids) {
                if seen_pids.insert(proc.pid) {
                    merged.push(proc);
                }
            }
        }
    }

    for proc in parse_ps_output(&stdout) {
        if seen_pids.insert(proc.pid) {
            merged.push(proc);
        }
    }

    merged.sort_by_key(|p| (p.kind, p.pid));
    Ok(merged)
}

/// Extracts all currently alive numeric PIDs from `ps -ax -o pid=,command=` stdout.
pub fn parse_alive_pids(stdout: &str) -> HashSet<u32> {
    stdout
        .lines()
        .filter_map(|line| split_pid_and_command(line.trim()))
        .filter_map(|(pid_str, _)| pid_str.parse::<u32>().ok())
        .collect()
}

/// Parses a single `hub_*.json` daemon metadata file.
pub fn parse_daemon_json(bytes: &[u8]) -> Option<DaemonRecord> {
    let record: DaemonRecord = serde_json::from_slice(bytes).ok()?;
    let token = record.csrf_token.as_deref()?.trim();
    if token.is_empty() {
        return None;
    }
    let has_port =
        record.https_port.is_some_and(|p| p > 0) || record.http_port.is_some_and(|p| p > 0);
    if !has_port {
        return None;
    }
    Some(record)
}

/// Scans a daemon directory for live `*.json` endpoint descriptors, filtering out stale PIDs.
pub fn discover_daemon_processes_in_dir(
    dir: &Path,
    kind: ProcessKind,
    alive_pids: &HashSet<u32>,
) -> Vec<DiscoveredProcess> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut items: Vec<(SystemTime, DiscoveredProcess)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_DAEMON_JSON_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Some(rec) = parse_daemon_json(&bytes) else {
            continue;
        };
        if !alive_pids.contains(&rec.pid) {
            continue;
        }

        let explicit_https_port = rec.https_port.filter(|&p| p > 0);
        let explicit_http_port = rec.http_port.filter(|&p| p > 0);
        let csrf_token = rec.csrf_token.unwrap_or_default().trim().to_string();
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        items.push((
            mtime,
            DiscoveredProcess {
                pid: rec.pid,
                kind,
                command: path.display().to_string(),
                csrf_token,
                explicit_https_port,
                explicit_http_port,
                extension_server_port: None,
                extension_server_csrf_token: None,
                hint_ports: Vec::new(),
            },
        ));
    }

    // Prefer newest daemon descriptor first when multiple live files exist.
    items.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.pid.cmp(&b.1.pid)));
    items.into_iter().map(|(_, p)| p).collect()
}

/// Discovers TCP listening ports owned by a specific PID via `lsof`.
pub async fn discover_listening_ports(pid: u32) -> Result<Vec<u16>> {
    let pid_str = pid.to_string();
    let lsof_bin = if std::path::Path::new("/usr/sbin/lsof").exists() {
        "/usr/sbin/lsof"
    } else {
        "lsof"
    };
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        Command::new(lsof_bin)
            .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-a", "-p", &pid_str])
            .output(),
    )
    .await??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_lsof_ports(&stdout))
}

/// Parses `ps -ax -o pid=,command=` output into prioritized Antigravity server candidates.
pub fn parse_ps_output(stdout: &str) -> Vec<DiscoveredProcess> {
    let mut found = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((pid_str, cmd)) = split_pid_and_command(trimmed) else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let Some(kind) = classify_process_kind(cmd) else {
            continue;
        };

        let csrf_token = extract_flag(cmd, "--csrf_token")
            .or_else(|| extract_flag(cmd, "--csrf-token"))
            .unwrap_or_default();

        // Legacy Desktop App and IDE argv matches require `--csrf_token`.
        if matches!(kind, ProcessKind::App | ProcessKind::Ide) && csrf_token.is_empty() {
            continue;
        }

        let extension_server_port = extract_port(cmd, "--extension_server_port")
            .or_else(|| extract_port(cmd, "--extension-server-port"));
        let extension_server_csrf_token = extract_flag(cmd, "--extension_server_csrf_token")
            .or_else(|| extract_flag(cmd, "--extension-server-csrf-token"));

        let mut hint_ports = Vec::new();
        for flag in [
            "--https_server_port",
            "--https-server-port",
            "--hub-port",
            "--hub_port",
            "--port",
        ] {
            if let Some(p) = extract_port(cmd, flag) {
                if p > 0 && !hint_ports.contains(&p) {
                    hint_ports.push(p);
                }
            }
        }

        found.push(DiscoveredProcess {
            pid,
            kind,
            command: cmd.to_string(),
            csrf_token,
            explicit_https_port: None,
            explicit_http_port: None,
            extension_server_port,
            extension_server_csrf_token,
            hint_ports,
        });
    }

    found.sort_by_key(|p| (p.kind, p.pid));
    found
}

/// Parses TCP listening ports from `lsof -nP -iTCP -sTCP:LISTEN` stdout.
pub fn parse_lsof_ports(stdout: &str) -> Vec<u16> {
    static PORT_RE: OnceLock<Regex> = OnceLock::new();
    let re = PORT_RE.get_or_init(|| Regex::new(r":(\d+)\s*\(LISTEN\)").expect("valid lsof regex"));

    let mut ports = Vec::new();
    let mut seen = HashSet::new();
    for line in stdout.lines() {
        for cap in re.captures_iter(line) {
            if let Ok(port) = cap[1].parse::<u16>() {
                if seen.insert(port) {
                    ports.push(port);
                }
            }
        }
    }
    ports
}

/// Builds ordered local Connect-RPC candidate endpoints for a process and its listening ports.
pub fn build_candidate_endpoints(
    proc: &DiscoveredProcess,
    listening_ports: &[u16],
) -> Vec<RpcEndpoint> {
    let mut endpoints = Vec::new();
    let mut seen: HashSet<(&'static str, u16, String)> = HashSet::new();

    if let Some(port) = proc.explicit_https_port.filter(|&p| p > 0) {
        let key = ("https", port, proc.csrf_token.clone());
        if seen.insert(key) {
            endpoints.push(RpcEndpoint {
                scheme: "https",
                port,
                csrf_token: proc.csrf_token.clone(),
                is_extension_server: false,
            });
        }
    }
    if let Some(port) = proc.explicit_http_port.filter(|&p| p > 0) {
        let key = ("http", port, proc.csrf_token.clone());
        if seen.insert(key) {
            endpoints.push(RpcEndpoint {
                scheme: "http",
                port,
                csrf_token: proc.csrf_token.clone(),
                is_extension_server: false,
            });
        }
    }

    let mut ports = Vec::new();
    for &p in &proc.hint_ports {
        if p > 0 && !ports.contains(&p) {
            ports.push(p);
        }
    }
    for &p in listening_ports {
        if p > 0 && !ports.contains(&p) {
            ports.push(p);
        }
    }

    for port in ports {
        if Some(port) == proc.extension_server_port {
            continue;
        }
        // Try HTTPS first (standard for language_server), then HTTP fallback.
        for scheme in ["https", "http"] {
            let key = (scheme, port, proc.csrf_token.clone());
            if seen.insert(key) {
                endpoints.push(RpcEndpoint {
                    scheme,
                    port,
                    csrf_token: proc.csrf_token.clone(),
                    is_extension_server: false,
                });
            }
        }
    }

    if let Some(ext_port) = proc.extension_server_port {
        if let Some(ext_tok) = &proc.extension_server_csrf_token {
            let key = ("http", ext_port, ext_tok.clone());
            if seen.insert(key) {
                endpoints.push(RpcEndpoint {
                    scheme: "http",
                    port: ext_port,
                    csrf_token: ext_tok.clone(),
                    is_extension_server: true,
                });
            }
        }
        let key = ("http", ext_port, proc.csrf_token.clone());
        if seen.insert(key) {
            endpoints.push(RpcEndpoint {
                scheme: "http",
                port: ext_port,
                csrf_token: proc.csrf_token.clone(),
                is_extension_server: true,
            });
        }
    }

    endpoints
}

/// Locates an installed `agy` CLI executable if present on the machine.
pub fn find_agy_binary() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("ANTIGRAVITY_CLI_PATH") {
        let p = PathBuf::from(explicit.trim());
        if p.is_file() {
            return Some(p);
        }
        return None;
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let cand = dir.join("agy");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }

    let mut well_known = Vec::new();
    if let Some(home) = dirs::home_dir() {
        well_known.push(home.join(".local/bin/agy"));
        well_known.push(home.join("Applications/Gemini.app/Contents/MacOS/agy"));
    }
    well_known.push(PathBuf::from("/opt/homebrew/bin/agy"));
    well_known.push(PathBuf::from("/usr/local/bin/agy"));
    well_known.push(PathBuf::from("/Applications/Gemini.app/Contents/MacOS/agy"));

    well_known.into_iter().find(|p| p.is_file())
}

fn split_pid_and_command(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let pid = parts.next()?.trim();
    let rest = parts.next()?.trim();
    if pid.is_empty() || rest.is_empty() {
        None
    } else {
        Some((pid, rest))
    }
}

/// Classifies a process command line into App, IDE, or CLI language server.
pub fn classify_process_kind(command: &str) -> Option<ProcessKind> {
    let lower = command.to_ascii_lowercase();
    if is_language_server_cmd(&lower) && is_antigravity_marker(&lower) {
        if is_antigravity_ide_marker(&lower) {
            return Some(ProcessKind::Ide);
        }
        return Some(ProcessKind::App);
    }
    if is_antigravity_cli_cmd(&lower) {
        return Some(ProcessKind::Cli);
    }
    None
}

fn is_language_server_cmd(lower: &str) -> bool {
    static LS_RE: OnceLock<Regex> = OnceLock::new();
    let re = LS_RE.get_or_init(|| {
        Regex::new(r"(^|[/\\])language(?:_|-)server(?:[_-][a-z0-9]+)*(?:\.exe)?(\s|$)")
            .expect("valid language_server regex")
    });
    re.is_match(lower)
}

fn is_antigravity_marker(lower: &str) -> bool {
    (lower.contains("--app_data_dir") && lower.contains("antigravity"))
        || lower.contains("antigravity.app/")
        || lower.contains("antigravity.app\\")
        || lower.contains("/gemini.app/")
        || lower.contains("\\gemini.app\\")
        || lower.contains("antigravity ide.app/")
        || lower.contains("antigravity ide.app\\")
        || lower.contains("/antigravity/")
        || lower.contains("\\antigravity\\")
}

fn is_antigravity_ide_marker(lower: &str) -> bool {
    [
        "antigravity ide.app/",
        "antigravity ide.app\\",
        "--app_data_dir antigravity-ide",
        "--app_data_dir=antigravity-ide",
        "/extensions/antigravity/bin/language_server",
        "\\extensions\\antigravity\\bin\\language_server",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

fn is_antigravity_cli_cmd(lower: &str) -> bool {
    static CLI_RE: OnceLock<Regex> = OnceLock::new();
    let re = CLI_RE.get_or_init(|| {
        Regex::new(r"(^|[/\\])(?:antigravity-cli|antigravity_cli|agy)(?:\s|$)")
            .expect("valid cli regex")
    });
    re.is_match(lower)
}

fn extract_flag(command: &str, flag: &str) -> Option<String> {
    let escaped = regex::escape(flag);
    let pattern = format!(r#"(?:^|\s){escaped}(?:=|\s+)("[^"]+"|'[^']+'|\S+)"#);
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(command)?;
    let raw = caps.get(1)?.as_str().trim();
    let unquoted = raw.trim_matches(|c| c == '"' || c == '\'').trim();
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

fn extract_port(command: &str, flag: &str) -> Option<u16> {
    extract_flag(command, flag)?.parse::<u16>().ok()
}
