//! Benchmark: sheaf Laplacian computation at various graph sizes.
//!
//! Simulates n venues reporting ETH prices, with all-pairs arbitrage edges
//! (complete graph). This is the worst-case edge density for a given node count.

// @anchor infra:sheaf:laplacian_bench
// @tags infra

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sheaf_engine::graph::{
    Edge, EdgeId, EdgeOrigin, EdgeStatus, EdgeType, GraphConfig,
    NodeId, NodeStatus, SheafGraph, Timeframe,
};
use sheaf_engine::laplacian::compute_laplacian;

fn bench_laplacian(c: &mut Criterion) {
    for &n in &[5, 10, 15, 20, 30] {
        let graph = build_dense_graph(n);
        c.bench_function(&format!("laplacian_{}_nodes", n), |b| {
            b.iter(|| {
                black_box(compute_laplacian(black_box(&graph)));
            });
        });
    }
}

/// Build a complete graph with n nodes, each at a slightly different price.
/// All-pairs arbitrage edges → worst-case edge density O(n²).
fn build_dense_graph(n: usize) -> SheafGraph {
    let watch_targets: Vec<_> = (0..n)
        .map(|i| (format!("V{}", i), "ETH".to_string()))
        .collect();

    let mut graph = SheafGraph::new(watch_targets, vec![Timeframe::T1m], GraphConfig::default());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    // Inject prices (small increments to keep spreads realistic)
    let base_price = 3000.0;
    for i in 0..n {
        let id = NodeId {
            venue: format!("V{}", i),
            symbol: "ETH".into(),
            timeframe: Timeframe::T1m,
        };
        if let Some(node) = graph.nodes.get_mut(&id) {
            node.last_price = Some(base_price + i as f64 * 0.5);
            node.last_tick_ns = Some(now);
            node.status = NodeStatus::Active;
        }
    }

    // All-pairs arbitrage edges
    let node_ids: Vec<NodeId> = graph.nodes.keys().cloned().collect();
    for i in 0..node_ids.len() {
        for j in (i + 1)..node_ids.len() {
            let a = &node_ids[i];
            let b = &node_ids[j];
            let pa = graph.nodes[a].last_price.unwrap();
            let pb = graph.nodes[b].last_price.unwrap();
            let weight = ((pa - pb).abs() / pa) * 10_000.0;

            let eid = EdgeId {
                a: a.clone(),
                b: b.clone(),
                edge_type: EdgeType::Arbitrage,
            };
            graph.edges.insert(
                eid.clone(),
                Edge {
                    id: eid,
                    status: EdgeStatus::Active,
                    weight,
                    discovered_at: now,
                    origin: EdgeOrigin::Discovered,
                },
            );
        }
    }

    graph
}

criterion_group!(benches, bench_laplacian);
criterion_main!(benches);
