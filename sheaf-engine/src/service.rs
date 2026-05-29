//! gRPC service implementation for the SheafEngine.
//!
//! Implements the `SheafEngine` proto service with bidirectional streaming.
//! Agent sends `ConfigureGraph` requests, engine streams `SignalBatch` responses.

// @anchor infra:sheaf:service
// @tags infra

use crate::align::AlignmentConfig;
use crate::config::RuntimeConfig;
use crate::graph::{GraphConfig, SheafGraph};
use crate::health::compute_health;
use crate::laplacian::compute_laplacian;
use crate::proto::sheaf_engine_server::{SheafEngine, SheafEngineServer};
use crate::proto::{
    ConfigureGraph, HealthRequest, HealthResponse, SignalBatch,
    SnapshotRequest, SnapshotResponse,
};
use crate::signals::extract_signals;
use crate::source::SourceOrchestrator;
use futures::stream::StreamExt;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

/// The sheaf engine gRPC service.
pub struct SheafEngineService {
    pub config: RuntimeConfig,
    /// Orchestrator manages tick sources. Created once at startup.
    pub orchestrator: Option<SourceOrchestrator>,
}

impl SheafEngineService {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            orchestrator: None,
        }
    }

    /// Create the tonic server builder.
    pub fn into_server(self) -> SheafEngineServer<Self> {
        SheafEngineServer::new(self)
    }
}

#[tonic::async_trait]
impl SheafEngine for SheafEngineService {
    /// Bidirectional streaming: agent streams config, engine streams signals.
    type RunStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<SignalBatch, Status>> + Send>>;

    async fn run(
        &self,
        request: Request<Streaming<ConfigureGraph>>,
    ) -> Result<Response<Self::RunStream>, Status> {
        let mut inbound = request.into_inner();

        // Read the first ConfigureGraph message.
        let config_msg = inbound
            .next()
            .await
            .ok_or_else(|| Status::invalid_argument("no ConfigureGraph message received"))??;

        tracing::info!(
            "Received ConfigureGraph: mode={:?}, watch_targets={}",
            config_msg.mode,
            config_msg.watch.len()
        );

        // Build watch targets from the config message.
        let watch_targets: Vec<(String, String)> = config_msg
            .watch
            .iter()
            .map(|w| (w.venue.clone(), w.symbol.clone()))
            .collect();

        // Initialize graph inside a thread-safe container.
        // Arc<RwLock<>> allows shared read access from the compute loop
        // and future snapshot/health endpoints without data races.
        let graph_config = GraphConfig::default();
        let timeframes = self.config.timeframes.clone();
        let graph = Arc::new(RwLock::new(SheafGraph::new(
            watch_targets,
            timeframes.clone(),
            graph_config,
        )));
        let graph_for_spawn = Arc::clone(&graph);

        let _alignment_config = AlignmentConfig {
            tolerance_ms: self.config.alignment_tolerance_ms,
            window_ms: self.config.alignment_window_ms,
            min_active_venues: 1,
        };

        // Channel for signal batches.
        let (tx, rx) = mpsc::channel::<Result<SignalBatch, Status>>(64);

        // Spawn the sheaf compute loop.
        // In production, this would consume from the SourceOrchestrator's merged stream.
        // For the scaffold, it ticks on a timer with empty batches.
        let window_ms = self.config.alignment_window_ms;
        let mut seq: u64 = 0;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(window_ms)).await;

                // Scope the read lock to minimum duration: compute
                // everything while holding the lock, then release before
                // the async send. This keeps RwLockReadGuard from
                // crossing an .await boundary.
                let (laplacian, signals, health, graph_state, edge_count) = {
                    let g = graph_for_spawn
                        .read()
                        .expect("RwLock must not be poisoned");

                    let laplacian = compute_laplacian(&g);
                    let signals = extract_signals(&g, &laplacian, 0);
                    let health = compute_health(&g, 0, vec![]);

                    let graph_state = crate::proto::GraphState {
                        version: g.version,
                        node_count: g.nodes.len() as u32,
                        edge_count: g.edges.len() as u32,
                        arbitrage_edges: g.edge_count_by_type(
                            crate::graph::EdgeType::Arbitrage,
                        ) as u32,
                        correlation_edges: g.edge_count_by_type(
                            crate::graph::EdgeType::Correlation,
                        ) as u32,
                        triangular_edges: g.edge_count_by_type(
                            crate::graph::EdgeType::Triangular,
                        ) as u32,
                        connected_components: 0, // TODO: compute connected components from graph
                        isolated_nodes: g.down_node_count() as u32,
                        is_connected: g.down_node_count() == 0,
                        nodes: vec![],
                        edges: vec![],
                    };
                    let edge_count = g.edges.len() as i64;

                    (laplacian, signals, health, graph_state, edge_count)
                    // `g` (RwLockReadGuard) dropped here — lock released.
                };

                let batch = SignalBatch {
                    timestamp_ns: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as i64,
                    seq,
                    graph: Some(graph_state),
                    signals,
                    health: Some(health),
                    metrics: Some(crate::proto::Metrics {
                        alignment_ns: 0,
                        graph_discovery_ns: 0,
                        laplacian_ns: laplacian.compute_ns,
                        signal_extraction_ns: 0,
                        total_window_ns: 0,
                        ticks_per_second: 0,
                        batches_per_second: 0,
                        total_ticks_ingested: 0,
                        total_edges_created: edge_count,
                        config_applies: 1,
                    }),
                };

                seq += 1;

                if tx.send(Ok(batch)).await.is_err() {
                    // Receiver dropped — agent disconnected.
                    break;
                }
            }
        });

        let outbound = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(outbound)))
    }

    /// REST fallback: snapshot of current topology state.
    async fn snapshot(
        &self,
        _request: Request<SnapshotRequest>,
    ) -> Result<Response<SnapshotResponse>, Status> {
        // TODO: return full graph state with node/edge details.
        Err(Status::unimplemented("snapshot not yet implemented"))
    }

    /// Health check.
    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse {
            health: Some(crate::proto::HealthState {
                perception_confidence: 1.0,
                active_nodes: 0,
                stale_nodes: 0,
                down_nodes: 0,
                active_edges: 0,
                degraded_edges: 0,
                broken_edges: 0,
                cross_exchange_skew_ms: 0,
                venues: vec![],
                active_alerts: vec![],
            }),
        }))
    }
}
