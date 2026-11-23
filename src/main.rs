//! CLI entry point for Antigravity Island: Dynamic Island GUI, live probe, skin listing,
//! and Antigravity CLI `statusLine` pipe ingestion.

use std::io::Read;

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};

use antigravity_island::config::{live_statusline_path, AppConfig, DisplayMode};
use antigravity_island::quota::client::QuotaClient;
use antigravity_island::quota::demo;
use antigravity_island::quota::parser::parse_statusline_json;
use antigravity_island::skin::SkinRegistry;
use antigravity_island::ui;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliMode {
    Island,
    NotchHug,
    Widget,
}

impl From<CliMode> for DisplayMode {
    fn from(m: CliMode) -> Self {
        match m {
            CliMode::Island => DisplayMode::Island,
            CliMode::NotchHug => DisplayMode::NotchHug,
            CliMode::Widget => DisplayMode::Widget,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "antigravity-island",
    version,
    about = "Lightweight Rust Dynamic Island & desktop quota widget for Google Antigravity"
)]
struct Cli {
    /// Launch in simulated demo quota mode (no running Antigravity instance required)
    #[arg(long)]
    demo: bool,

    /// Override active skin ID on launch
    #[arg(long)]
    skin: Option<String>,

    /// Override display mode (island | notch-hug | widget)
    #[arg(long, value_enum)]
    mode: Option<CliMode>,

    /// Run a one-shot live quota check and print results to stdout, then exit
    #[arg(long)]
    probe: bool,

    /// Emit raw JSON (used with --probe)
    #[arg(long)]
    json: bool,

    /// List all built-in and custom skins, then exit
    #[arg(long)]
    list_skins: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Read Antigravity CLI `statusLine` JSON from stdin, cache for the Island, and print a compact line
    IngestStatusline,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "antigravity_island=info".into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    if let Some(Commands::IngestStatusline) = cli.command {
        return run_ingest_statusline();
    }

    if cli.list_skins {
        let mut reg = SkinRegistry::load_default();
        let _ = reg.ensure_initialized();
        reg.reload();
        println!("Skins directory: {}", reg.skins_dir().display());
        for s in reg.list() {
            let origin = if s.builtin { "builtin" } else { "custom" };
            println!(
                "- {:<16} [{origin}] {:<18} gauge={:?} — {}",
                s.id, s.name, s.style.gauge, s.description
            );
        }
        return Ok(());
    }

    if cli.probe {
        return run_probe(cli.demo, cli.json);
    }

    let mut config = AppConfig::load();
    if let Some(skin_id) = cli.skin {
        config.skin_id = skin_id;
    }
    if let Some(mode) = cli.mode {
        config.mode = mode.into();
    }

    ui::run_app(config, cli.demo)
}

fn run_probe(demo_mode: bool, as_json: bool) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let snapshot = if demo_mode {
        demo::sample_snapshot(1)
    } else {
        rt.block_on(async { QuotaClient::new().fetch_live_quota().await })?
    };

    if as_json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    let plan = snapshot.account_plan.as_deref().unwrap_or("Unknown Plan");
    println!("Antigravity Quota ({}) — {}", snapshot.source.label(), plan);
    if let Some(credits) = snapshot.ai_credits {
        println!("AI Credits: {:.0}", credits);
    }
    let now = Utc::now();
    for group in &snapshot.groups {
        println!("\n[{}]", group.display_name);
        for bucket in &group.buckets {
            let pct = bucket
                .remaining_percent()
                .map(|p| format!("{p:>3}%"))
                .unwrap_or_else(|| " -- ".to_string());
            let reset = bucket
                .reset_time
                .map(|dt| {
                    let secs = (dt - now).num_seconds().max(0);
                    let h = secs / 3600;
                    let m = (secs % 3600) / 60;
                    format!("resets in {h}h {m:02}m")
                })
                .unwrap_or_else(|| "no reset timestamp".to_string());
            println!(
                "  {:<7} {:<20} {}  ({})",
                bucket.cadence.short_tag(),
                bucket.display_name,
                pct,
                reset
            );
        }
    }
    Ok(())
}

fn run_ingest_statusline() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    if input.trim().is_empty() {
        return Ok(());
    }

    let path = live_statusline_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, input.as_bytes())?;
    std::fs::rename(tmp, &path)?;

    if let Ok(snap) = parse_statusline_json(input.as_bytes()) {
        println!("{}", snap.tray_summary());
    }
    Ok(())
}
