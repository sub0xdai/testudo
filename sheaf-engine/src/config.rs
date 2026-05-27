//! Configuration — CLI args, config file, environment variables.
//!
//! The sheaf engine is configured via CLI flags (primary) with
//! environment variable fallback. No config file in v0.1.

use clap::Parser;

/// Sheaf Engine — cellular sheaf topology perception layer.
///
/// Ingests multi-venue tick data and computes topological signals
/// for AI agent harnesses via gRPC bidirectional streaming.
#[derive(Parser, Debug, Clone)]
#[command(name = "sheaf-engine", version, about)]
pub struct Cli {
    /// gRPC listen address.
    #[arg(long, env = "SHEAF_GRPC_LISTEN", default_value = "0.0.0.0:50051")]
    pub grpc_listen: String,

    /// Venues to connect to. Format: "NAME,URL".
    /// Repeat for each venue.
    /// Example: --venue BN,wss://stream.binance.com:9443/ws
    #[arg(long = "venue", value_parser = parse_venue)]
    pub venues: Vec<VenueEntry>,

    /// Symbols to watch. Repeat for each symbol.
    #[arg(long = "symbol")]
    pub symbols: Vec<String>,

    /// Tick source priority: direct, merge, or testudo.
    #[arg(long, env = "SHEAF_SOURCE_PRIORITY", default_value = "direct")]
    pub source_priority: String,

    /// Alignment tolerance in milliseconds.
    #[arg(long, env = "SHEAF_ALIGNMENT_TOLERANCE_MS", default_value = "500")]
    pub alignment_tolerance_ms: u64,

    /// Alignment window in milliseconds (how often snapshots are produced).
    #[arg(long, env = "SHEAF_ALIGNMENT_WINDOW_MS", default_value = "100")]
    pub alignment_window_ms: u64,

    /// Active timeframes (comma-separated: t1s,t10s,t1m,t5m,t1h,t4h).
    #[arg(long, env = "SHEAF_TIMEFRAMES", default_value = "t1s,t10s,t1m,t5m,t1h,t4h")]
    pub timeframes: String,
}

/// A single venue entry from the CLI.
#[derive(Debug, Clone)]
pub struct VenueEntry {
    pub name: String,
    pub url: String,
}

fn parse_venue(s: &str) -> Result<VenueEntry, String> {
    let parts: Vec<&str> = s.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err(format!(
            "invalid venue format '{}': expected NAME,URL (e.g. BN,wss://stream.binance.com:9443/ws)",
            s
        ));
    }
    Ok(VenueEntry {
        name: parts[0].to_string(),
        url: parts[1].to_string(),
    })
}

/// Runtime configuration derived from CLI + defaults.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub grpc_listen: String,
    pub venues: Vec<VenueEntry>,
    pub symbols: Vec<String>,
    pub source_priority: crate::source::TickSourcePriority,
    pub alignment_tolerance_ms: u64,
    pub alignment_window_ms: u64,
    pub timeframes: Vec<crate::graph::Timeframe>,
    pub graph_config: crate::graph::GraphConfig,
}

impl RuntimeConfig {
    /// Build runtime config from CLI args.
    pub fn from_cli(cli: &Cli) -> Self {
        let source_priority = match cli.source_priority.as_str() {
            "direct" => crate::source::TickSourcePriority::Direct,
            "merge" => crate::source::TickSourcePriority::Merge,
            "testudo" => crate::source::TickSourcePriority::PreferTestudo,
            other => {
                tracing::warn!(
                    "unknown source priority '{}', falling back to direct",
                    other
                );
                crate::source::TickSourcePriority::Direct
            }
        };

        let timeframes = parse_timeframes(&cli.timeframes);

        Self {
            grpc_listen: cli.grpc_listen.clone(),
            venues: cli.venues.clone(),
            symbols: cli.symbols.clone(),
            source_priority,
            alignment_tolerance_ms: cli.alignment_tolerance_ms,
            alignment_window_ms: cli.alignment_window_ms,
            timeframes,
            graph_config: crate::graph::GraphConfig::default(),
        }
    }
}

pub fn parse_timeframes(s: &str) -> Vec<crate::graph::Timeframe> {
    use crate::graph::Timeframe;

    s.split(',')
        .filter_map(|t| match t.trim() {
            "t1s" => Some(Timeframe::T1s),
            "t10s" => Some(Timeframe::T10s),
            "t1m" => Some(Timeframe::T1m),
            "t5m" => Some(Timeframe::T5m),
            "t1h" => Some(Timeframe::T1h),
            "t4h" => Some(Timeframe::T4h),
            other => {
                tracing::warn!("unknown timeframe '{}', skipping", other);
                None
            }
        })
        .collect()
}
