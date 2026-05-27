//! gRPC service implementation for the SheafEngine.
//!
//! Implements the `SheafEngine` proto service with bidirectional streaming.
//! Agent sends `ConfigureGraph` requests, engine streams `SignalBatch` responses.

use crate::align::AlignmentConfig;
use crate::config::RuntimeConfig;
use crate::graph::{GraphConfig, SheafGraph};
use crate::health::compute_health;
use crate::laplacian::compute_laplacian;
use crate::proto::sheaf_engine_server::{SheafEngine, SheafEngineServer};
use crate::proto::{
    ConfigureGraph, HealthRequest, HealthResponse, SignalBatch, SnapshotRequest, SnapshotResponse,
};
use crate::signals::extract_signals;
use crate::source::SourceOrchestrator;
use futures::stream::StreamExt;
use std::pin::Pin;
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

        // Initialize graph and alignment.
        let graph_config = GraphConfig::default();
        let timeframes = self.config.timeframes.clone();
        let graph = SheafGraph::new(watch_targets, timeframes.clone(), graph_config);

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

                // In production: pull next TickBatch from orchestrator, align, graph.ingest().
                // For scaffold: produce a minimal heartbeat batch.
                let laplacian = compute_laplacian(&graph);
                let signals = extract_signals(&graph, &laplacian, 0);
                let health = compute_health(&graph, 0, vec![]);

                let batch = SignalBatch {
                    timestamp_ns: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as i64,
                    seq,
                    graph: Some(crate::proto::GraphState {
                        version: graph.version,
                        node_count: graph.nodes.len() as u32,
                        edge_count: graph.edges.len() as u32,
                        arbitrage_edges: graph.edge_count_by_type(
                            crate::graph::EdgeType::Arbitrage,
                        ) as u32,
                        correlation_edges: graph.edge_count_by_type(
                            crate::graph::EdgeType::Correlation,
                        ) as u32,
                        triangular_edges: graph.edge_count_by_type(
                            crate::graph::EdgeType::Triangular,
                        ) as u32,
                        connected_components: 1,
                        isolated_nodes: graph.down_node_count() as u32,
                        is_connected: graph.down_node_count() == 0,
                        nodes: vec![],
                        edges: vec![],
                    }),
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
                        total_edges_created: graph.edges.len() as i64,
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
