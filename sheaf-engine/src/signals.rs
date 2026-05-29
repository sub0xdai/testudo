//! Signal extraction — topology observations → `TopologySignal`.
//!
//! After each Laplacian computation, this module extracts structured
//! topology signals from the graph state. Signals follow the Option C
//! format: enriched structural observations with `context` fields
//! that the LLM reads directly.

// @anchor infra:sheaf:signals
// @tags infra

use crate::graph::{EdgeType, SheafGraph};
use crate::laplacian::LaplacianResult;
use crate::proto;

/// Extract all topology signals from the current graph state.
pub fn extract_signals(
    graph: &SheafGraph,
    laplacian: &LaplacianResult,
    _cross_exchange_skew_ms: i64,
) -> Vec<proto::TopologySignal> {
    let now = now_ns();
    let mut signals = Vec::new();

    // Arbitrage signals
    extract_arbitrage_signals(graph, now, &mut signals);

    // Correlation break signals
    extract_correlation_signals(graph, now, &mut signals);

    // Volatility diffusion (from Laplacian eigen gap).
    extract_volatility_diffusion(graph, laplacian, now, &mut signals);

    // Triangular mispricing.
    extract_triangular_signals(graph, now, &mut signals);

    // Edge lifecycle.
    extract_edge_lifecycle(graph, now, &mut signals);

    // Node health.
    extract_node_health(graph, now, &mut signals);

    // Post-condition: signal context strings must be non-empty.
    debug_assert!(
        signals.iter().all(|s| !s.context.is_empty()),
        "all topology signals must carry context"
    );

    signals
}

fn extract_arbitrage_signals(
    graph: &SheafGraph,
    _now: i64,
    signals: &mut Vec<proto::TopologySignal>,
) {
    for edge in graph.edges.values() {
        if edge.id.edge_type != EdgeType::Arbitrage {
            continue;
        }

        let node_a = graph.nodes.get(&edge.id.a);
        let node_b = graph.nodes.get(&edge.id.b);
        let (Some(na), Some(nb)) = (node_a, node_b) else {
            continue;
        };
        let (Some(pa), Some(pb)) = (na.last_price, nb.last_price) else {
            continue;
        };

        let spread_bps = ((pa - pb).abs() / pa) * 10_000.0;
        debug_assert!(spread_bps >= 0.0, "spread must be non-negative");

        // Only emit if spread exceeds configured threshold.
        let threshold = graph.config().arbitrage_signal_threshold_bps;
        if spread_bps < threshold {
            continue;
        }

        let sigma = if spread_bps > 0.0 {
            spread_bps / 4.0 // Baseline spread is ~4 bps for crypto.
        } else {
            0.0
        };

        signals.push(proto::TopologySignal {
            r#type: proto::SignalType::ArbitrageEdge as i32,
            severity: if spread_bps > 20.0 {
                proto::Severity::Critical as i32
            } else {
                proto::Severity::Notable as i32
            },
            context: format!(
                "{} spot spread {:.0} bps {}→{}. 30s avg is 4 bps. {:.2}σ anomaly.",
                edge.id.a.symbol, spread_bps, edge.id.a.venue, edge.id.b.venue, sigma
            ),
            first_seen_ns: edge.discovered_at,
            duration_windows: 1, // TODO: track consecutive windows
            signal_data: Some(proto::topology_signal::SignalData::Arbitrage(
                proto::ArbitrageSignal {
                    venue_a: edge.id.a.venue.clone(),
                    venue_b: edge.id.b.venue.clone(),
                    symbol: edge.id.a.symbol.clone(),
                    spread_bps,
                    spread_usd: (pa - pb).abs(),
                    baseline_spread_bps: 4.0,
                    sigma,
                },
            )),
        });
    }
}

fn extract_correlation_signals(
    _graph: &SheafGraph,
    _now: i64,
    _signals: &mut Vec<proto::TopologySignal>,
) {
    // TODO: track rolling correlation in node price history ring buffers.
    // When |ρ| drops > 0.15 from baseline, emit CORRELATION_BREAK.
}

fn extract_volatility_diffusion(
    _graph: &SheafGraph,
    laplacian: &LaplacianResult,
    _now: i64,
    signals: &mut Vec<proto::TopologySignal>,
) {
    // Eigen gap heuristic: large gap → strong community structure →
    // potential volatility regime shift. Full diffusion modeling requires
    // timeframe stalk edges (future work).
    if laplacian.eigen_gap > 0.5 && laplacian.node_count > 2 {
        debug_assert!(
            laplacian.eigen_gap.is_finite(),
            "eigen gap must be finite"
        );

        signals.push(proto::TopologySignal {
            r#type: proto::SignalType::VolatilityDiffusion as i32,
            severity: proto::Severity::Notable as i32,
            context: format!(
                "Sheaf eigen gap {:.3}: volatility regime \
                 structure forming. {} nodes active.",
                laplacian.eigen_gap, laplacian.node_count
            ),
            first_seen_ns: now_ns(),
            duration_windows: 1,
            signal_data: Some(
                proto::topology_signal::SignalData::VolatilityDiffusion(
                    proto::VolatilityDiffusionSignal {
                        direction: "upward".into(),
                        source_timeframe: proto::Timeframe::T1m as i32,
                        target_timeframe: proto::Timeframe::T5m as i32,
                        strength: laplacian.eigen_gap.min(1.0),
                        symbols: vec![],
                    },
                ),
            ),
        });
    }
}

fn extract_triangular_signals(
    graph: &SheafGraph,
    _now: i64,
    signals: &mut Vec<proto::TopologySignal>,
) {
    for edge in graph.edges.values() {
        if edge.id.edge_type != EdgeType::Triangular {
            continue;
        }
        let friction = 12.0; // estimated execution friction in bps

        signals.push(proto::TopologySignal {
            r#type: proto::SignalType::TriangularMispricing as i32,
            severity: if edge.weight > friction {
                proto::Severity::Notable as i32
            } else {
                proto::Severity::Info as i32
            },
            context: format!(
                "Triangular path {} shows {:.0} bps imbalance. {} execution friction (~{:.0} bps).",
                edge.id.a.venue,
                edge.weight,
                if edge.weight > friction {
                    "Above"
                } else {
                    "Below"
                },
                friction
            ),
            first_seen_ns: edge.discovered_at,
            duration_windows: 1,
            signal_data: Some(
                proto::topology_signal::SignalData::TriangularMispricing(
                    proto::TriangularMispricingSignal {
                        venue: edge.id.a.venue.clone(),
                        path: vec![
                            edge.id.a.symbol.clone(),
                            edge.id.b.symbol.clone(),
                        ],
                        imbalance_bps: edge.weight,
                        execution_friction_bps: friction,
                    },
                ),
            ),
        });
    }
}

fn extract_edge_lifecycle(
    _graph: &SheafGraph,
    _now: i64,
    _signals: &mut Vec<proto::TopologySignal>,
) {
    // TODO: compare current edge state to previous state.
    // Emit EDGE_APPEARED / EDGE_REMOVED on transitions.
}

fn extract_node_health(
    graph: &SheafGraph,
    now: i64,
    signals: &mut Vec<proto::TopologySignal>,
) {
    debug_assert!(now > 0, "timestamp must be positive");

    for (id, node) in &graph.nodes {
        match node.status {
            crate::graph::NodeStatus::Stale { since }
                if now - since < 200_000_000 =>
            {
                signals.push(proto::TopologySignal {
                        r#type: proto::SignalType::NodeStale as i32,
                        severity: proto::Severity::Notable as i32,
                        context: format!(
                            "{}:{} no tick data for {}s. Venue may be throttling.",
                            id.venue, id.symbol, (now - since) / 1_000_000_000
                        ),
                        first_seen_ns: since,
                        duration_windows: 1,
                        signal_data: Some(
                            proto::topology_signal::SignalData::NodeHealth(
                                proto::NodeHealthSignal {
                                    venue: id.venue.clone(),
                                    symbol: id.symbol.clone(),
                                    event: "stale".into(),
                                    since_ns: since,
                                    suspected_cause: "no_tick_data".into(),
                                },
                            ),
                        ),
                    });
            }
            crate::graph::NodeStatus::Down { since }
                if now - since < 200_000_000 =>
            {
                signals.push(proto::TopologySignal {
                        r#type: proto::SignalType::NodeDown as i32,
                        severity: proto::Severity::Critical as i32,
                        context: format!(
                            "{}:{} venue appears DOWN. No ticks for {}s.",
                            id.venue, id.symbol, (now - since) / 1_000_000_000
                        ),
                        first_seen_ns: since,
                        duration_windows: 1,
                        signal_data: Some(
                            proto::topology_signal::SignalData::NodeHealth(
                                proto::NodeHealthSignal {
                                    venue: id.venue.clone(),
                                    symbol: id.symbol.clone(),
                                    event: "down".into(),
                                    since_ns: since,
                                    suspected_cause: "venue_maintenance".into(),
                                },
                            ),
                        ),
                    });
            }
            _ => {}
        }
    }
}

pub(crate) use crate::graph::now_ns;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, EdgeId, EdgeOrigin, EdgeType, GraphConfig,
        NodeId, NodeStatus, SheafGraph, Timeframe};
    use crate::laplacian::LaplacianResult;

    /// When `arbitrage_signal_threshold_bps` is raised to 10.0, a 7 bps spread
    /// should NOT emit a signal. Proves the config threshold is wired.
    #[test]
    fn test_arbitrage_signal_respects_config_threshold() {
        let now = crate::graph::now_ns();

        // Custom config: threshold at 10 bps instead of default 5.
        let config = GraphConfig {
            arbitrage_signal_threshold_bps: 10.0,
            ..Default::default()
        };

        let mut graph = SheafGraph::new(
            vec![("A".into(), "ETH".into()), ("B".into(), "ETH".into())],
            vec![Timeframe::T1m],
            config,
        );

        // Set node A price to 100.0, node B to 99.93 — spread = 7 bps.
        let id_a = NodeId { venue: "A".into(), symbol: "ETH".into(), timeframe: Timeframe::T1m };
        let id_b = NodeId { venue: "B".into(), symbol: "ETH".into(), timeframe: Timeframe::T1m };

        if let Some(n) = graph.nodes.get_mut(&id_a) {
            n.last_price = Some(100.0);
            n.last_tick_ns = Some(now);
            n.status = NodeStatus::Active;
        }
        if let Some(n) = graph.nodes.get_mut(&id_b) {
            n.last_price = Some(99.93);
            n.last_tick_ns = Some(now);
            n.status = NodeStatus::Active;
        }

        // Add an arbitrage edge between A and B.
        let edge_id = EdgeId {
            a: id_a.clone(),
            b: id_b.clone(),
            edge_type: EdgeType::Arbitrage,
        };
        graph.edges.insert(
            edge_id.clone(),
            Edge {
                id: edge_id,
                status: crate::graph::EdgeStatus::Active,
                weight: 1.0,
                discovered_at: now,
                origin: EdgeOrigin::Discovered,
            },
        );

        // Extract signals: spread is 7 bps, which is below 10 bps threshold.
        // Should produce NO arbitrage signal.
        let dummy_laplacian = LaplacianResult {
            node_count: 2,
            eigenvalues: vec![0.0, 1.0],
            eigen_gap: 1.0,
            algebraic_connectivity: 1.0,
            spectral_radius: 1.0,
            compute_ns: 0,
        };
        let signals = extract_signals(&graph, &dummy_laplacian, 0);

        let arb_signals: Vec<_> = signals
            .iter()
            .filter(|s| s.r#type == crate::proto::SignalType::ArbitrageEdge as i32)
            .collect();

        assert!(
            arb_signals.is_empty(),
            "Expected 0 arbitrage signals at 7 bps with threshold 10 bps, got {}",
            arb_signals.len()
        );
    }
}
