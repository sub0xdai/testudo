//! Tick source abstraction.
//!
//! Two implementations:
//! - `ExchangeSource` — direct exchange WebSocket connection
//! - `TestudoSource` — wraps Testudo's existing gRPC/WS pipe
//!
//! Sources are orchestrated by `SourceOrchestrator`.

// @anchor infra:sheaf:source
// @tags infra

use crate::tick::TickBatch;
use futures::stream::Stream;
use std::pin::Pin;

// ── Venue identifier ──

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VenueId(pub String);

// ── Health ──

#[derive(Debug, Clone)]
pub struct SourceHealth {
    pub venue: VenueId,
    pub state: SourceState,
    pub ticks_per_min: f64,
    pub last_tick_age_ms: Option<u64>,
    pub reconnect_attempt: u32,
    pub reconnect_exhausted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceState {
    Healthy,
    Stale { stale_since: i64 },
    Reconnecting { since: i64, attempt: u32 },
    Dead { since: i64 },
}

// ── Reconnect policy ──

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub multiplier: f64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_backoff_ms: 500,
            max_backoff_ms: 30_000,
            multiplier: 2.0,
        }
    }
}

// ── Subscription ──

#[derive(Debug, Clone)]
pub struct Subscription {
    pub symbol: String,
    pub data_type: TickDataType,
}

#[derive(Debug, Clone)]
pub enum TickDataType {
    Trades,
}

// ── Source priority ──

#[derive(Debug, Clone)]
pub enum TickSourcePriority {
    Direct,
    Merge,
    PreferTestudo,
}

// ── TickSource trait ──

/// The unified tick source interface.
/// Returns a boxed stream of TickBatch.
#[async_trait::async_trait]
pub trait TickSource: Send + Sync {
    fn venue_id(&self) -> &VenueId;

    async fn stream(
        &mut self,
        subscriptions: Vec<Subscription>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<TickBatch, TickSourceError>> + Send + 'static>>,
        TickSourceError,
    >;

    fn health(&self) -> SourceHealth;

    async fn shutdown(&mut self) -> Result<(), TickSourceError>;
}

// ── TickSource error ──

#[derive(Debug, thiserror::Error)]
pub enum TickSourceError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("disconnected unexpectedly")]
    Disconnected,
    #[error("reconnect exhausted after {attempts} attempts")]
    ReconnectExhausted { attempts: u32 },
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid tick: {0}")]
    InvalidTick(String),
    #[error("stale feed: no ticks for {staleness_ms}ms")]
    StaleFeed { staleness_ms: u64 },
    #[error("backpressure: downstream consumer too slow")]
    Backpressure,
    #[error("unknown venue: {0}")]
    UnknownVenue(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// ── ExchangeSource ──

pub struct ExchangeSource {
    pub venue_id: VenueId,
    pub ws_url: String,
    pub reconnect_policy: ReconnectPolicy,
    pub ring_capacity: usize,
}

impl ExchangeSource {
    pub fn new(venue: VenueId, ws_url: String) -> Self {
        Self {
            venue_id: venue,
            ws_url,
            reconnect_policy: ReconnectPolicy::default(),
            ring_capacity: 4096,
        }
    }
}

#[async_trait::async_trait]
impl TickSource for ExchangeSource {
    fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    async fn stream(
        &mut self,
        _subscriptions: Vec<Subscription>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<TickBatch, TickSourceError>> + Send + 'static>>,
        TickSourceError,
    > {
        todo!("ExchangeSource::stream — WebSocket tick ingestion pipeline")
    }

    fn health(&self) -> SourceHealth {
        todo!("ExchangeSource::health")
    }

    async fn shutdown(&mut self) -> Result<(), TickSourceError> {
        Ok(())
    }
}

// ── TestudoSource ──

pub struct TestudoSource {
    pub venue_id: VenueId,
    pub endpoint: String,
    pub auth_token: String,
}

impl TestudoSource {
    pub fn new(venue: VenueId, endpoint: String, auth_token: String) -> Self {
        Self {
            venue_id: venue,
            endpoint,
            auth_token,
        }
    }
}

#[async_trait::async_trait]
impl TickSource for TestudoSource {
    fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    async fn stream(
        &mut self,
        _subscriptions: Vec<Subscription>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<TickBatch, TickSourceError>> + Send + 'static>>,
        TickSourceError,
    > {
        todo!("TestudoSource::stream — gRPC tick relay pipeline")
    }

    fn health(&self) -> SourceHealth {
        todo!("TestudoSource::health")
    }

    async fn shutdown(&mut self) -> Result<(), TickSourceError> {
        Ok(())
    }
}

// ── SourceOrchestrator ──

pub struct SourceOrchestrator {
    pub sources: Vec<Box<dyn TickSource>>,
    pub priority: TickSourcePriority,
}

impl SourceOrchestrator {
    pub fn new(priority: TickSourcePriority) -> Self {
        Self {
            sources: Vec::new(),
            priority,
        }
    }

    pub fn add_source(&mut self, source: Box<dyn TickSource>) {
        self.sources.push(source);
    }

    pub async fn start(
        &mut self,
        _subscriptions: Vec<Subscription>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<TickBatch, TickSourceError>> + Send + 'static>>,
        TickSourceError,
    > {
        todo!("SourceOrchestrator::start — multi-source merge")
    }

    pub fn all_health(&self) -> Vec<SourceHealth> {
        self.sources.iter().map(|s| s.health()).collect()
    }
}
