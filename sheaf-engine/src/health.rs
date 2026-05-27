//! Health state computation and perception confidence scoring.
//!
//! `perception_confidence` gates the agent's decision-making:
//! - ≥ 0.80: normal operation
//! - 0.60–0.79: degraded — fall back to raw OHLCV
//! - < 0.60: HaltExecution
//! - 0.00: sheaf unreachable — hard halt

use crate::graph::SheafGraph;
use crate::proto;

/// Compute the current health state from graph and alignment data.
pub fn compute_health(
    graph: &SheafGraph,
    cross_exchange_skew_ms: i64,
    venue_healths: Vec<proto::VenueHealth>,
) -> proto::HealthState {
    let active_nodes = graph.active_node_count() as u32;
    let stale_nodes = graph.stale_node_count() as u32;
    let down_nodes = graph.down_node_count() as u32;
    let total_nodes = graph.nodes.len() as u32;

    let active_edges = graph.active_edge_count() as u32;
    let degraded_edges = graph.degraded_edge_count() as u32;
    let broken_edges = graph
        .edges
        .values()
        .filter(|e| matches!(e.status, crate::graph::EdgeStatus::Removed { .. }))
        .count() as u32;
    let total_edges = graph.edges.len() as u32;

    let confidence = perception_confidence(
        active_nodes, total_nodes,
        active_edges, degraded_edges, total_edges,
    );

    proto::HealthState {
        perception_confidence: confidence,
        active_nodes,
        stale_nodes,
        down_nodes,
        active_edges,
        degraded_edges,
        broken_edges,
        cross_exchange_skew_ms,
        venues: venue_healths,
        active_alerts: vec![], // TODO: detect alert conditions
    }
}

/// Compute perception confidence score.
///
/// Formula (from decision D):
/// ```text
/// node_health = active_nodes / total_nodes
/// edge_health = (active_edges + 0.3 × degraded_edges) / total_edges
/// perception_confidence = 0.4 × node_health + 0.6 × edge_health
/// ```
pub fn perception_confidence(
    active_nodes: u32,
    total_nodes: u32,
    active_edges: u32,
    degraded_edges: u32,
    total_edges: u32,
) -> f64 {
    debug_assert!(active_nodes <= total_nodes,
        "active_nodes ({active_nodes}) must not exceed total_nodes ({total_nodes})");
    debug_assert!(active_edges + degraded_edges <= total_edges || total_edges == 0,
        "active + degraded edges must not exceed total edges");

    if total_nodes == 0 {
        return 0.0;
    }

    let node_health = active_nodes as f64 / total_nodes as f64;

    let edge_health = if total_edges == 0 {
        1.0 // No edges yet = not degraded.
    } else {
        (active_edges as f64 + 0.3 * degraded_edges as f64) / total_edges as f64
    };

    let confidence = 0.4 * node_health + 0.6 * edge_health;

    // Post-condition: confidence must be in [0, 1].
    debug_assert!(
        (0.0..=1.0).contains(&confidence),
        "confidence ({confidence}) must be in [0, 1]"
    );

    confidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_health() {
        let c = perception_confidence(10, 10, 5, 0, 5);
        assert!((c - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_all_stale() {
        let c = perception_confidence(0, 10, 0, 0, 5);
        assert!((c - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_half_degraded() {
        // 5/10 nodes active, 2/4 edges active + 2 degraded
        let c = perception_confidence(5, 10, 2, 2, 4);
        let expected = 0.4 * 0.5 + 0.6 * ((2.0 + 0.3 * 2.0) / 4.0);
        assert!((c - expected).abs() < 0.001);
    }
}
