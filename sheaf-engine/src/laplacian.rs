//! Sheaf Laplacian computation.
//!
//! For realistic graphs (15-30 nodes, 2-3 venues), the Laplacian is a
//! sparse |V|×|V| matrix. At this scale, a dense linear algebra approach
//! with LAPACK (via `nalgebra` or `faer`) handles it in single-digit
//! microseconds. SIMD is documented as future work (see decision E).
//!
//! The Laplacian L = D - A where:
//! - D is the degree matrix (diagonal, D[i][i] = sum of weights of edges incident to node i)
//! - A is the weighted adjacency matrix (A[i][j] = weight of edge between nodes i and j)

use crate::graph::{EdgeType, SheafGraph};

/// The result of a sheaf Laplacian computation.
#[derive(Debug, Clone)]
pub struct LaplacianResult {
    /// Number of nodes in the graph.
    pub node_count: usize,
    /// Eigenvalues of the Laplacian (sorted ascending).
    /// λ₀ = 0 indicates a connected component.
    pub eigenvalues: Vec<f64>,
    /// Eigengap: λ₁ - λ₀. Larger gap = more community structure.
    pub eigengap: f64,
    /// Algebraic connectivity: λ₁ (second smallest eigenvalue).
    /// Larger = better connected graph. 0 = disconnected.
    pub algebraic_connectivity: f64,
    /// Spectral radius: λ_max.
    pub spectral_radius: f64,
    /// Compute time in nanoseconds.
    pub compute_ns: i64,
}

/// Compute the sheaf Laplacian for the current graph state.
///
/// At target scale (15-30 nodes), this is a dense eigenvalue problem
/// solved with LAPACK's DSYEV. Returns the full eigenspectrum.
pub fn compute_laplacian(graph: &SheafGraph) -> LaplacianResult {
    let t0 = now_ns();

    // Build node index mapping: assign each active node a dense index.
    let active_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|(_, n)| n.last_price.is_some())
        .collect();

    let n = active_nodes.len();
    if n == 0 {
        return LaplacianResult {
            node_count: 0,
            eigenvalues: vec![],
            eigengap: 0.0,
            algebraic_connectivity: 0.0,
            spectral_radius: 0.0,
            compute_ns: now_ns() - t0,
        };
    }

    let node_index: std::collections::HashMap<_, usize> = active_nodes
        .iter()
        .enumerate()
        .map(|(i, (id, _))| ((*id).clone(), i))
        .collect();

    // Build dense Laplacian matrix L = D - A.
    let mut l = vec![vec![0.0f64; n]; n];

    // Fill adjacency (off-diagonal) from edges.
    for edge in graph.edges.values() {
        let i = node_index.get(&edge.id.a);
        let j = node_index.get(&edge.id.b);
        if let (Some(&i), Some(&j)) = (i, j) {
            let w = edge_weight(edge.id.edge_type, edge.weight);
            l[i][j] -= w;
            l[j][i] -= w;
        }
    }

    // Fill degree (diagonal): D[i][i] = -sum of off-diagonal row i.
    for i in 0..n {
        let row_sum: f64 = l[i].iter().sum::<f64>() - l[i][i];
        l[i][i] = -row_sum;
    }

    // Compute eigenvalues via nalgebra's dense symmetric eigen-decomposition.
    // For target scale (15-30 nodes), this is single-digit µs.
    let eigenvalues: Vec<f64> = dense_symmetric_eigenvalues(&l, n);

    let compute_ns = now_ns() - t0;

    let algebraic_connectivity = if n > 1 { eigenvalues[1] } else { 0.0 };
    let eigengap = if n > 1 {
        eigenvalues[1] - eigenvalues[0]
    } else {
        0.0
    };
    let spectral_radius = eigenvalues.last().copied().unwrap_or(0.0);

    LaplacianResult {
        node_count: n,
        eigenvalues,
        eigengap,
        algebraic_connectivity,
        spectral_radius,
        compute_ns,
    }
}

/// Map edge type + weight to Laplacian edge weight.
///
/// Different edge types contribute to the Laplacian with different
/// semantics. Arbitrage edges represent price dislocation (negative
/// correlation). Correlation edges represent co-movement.
fn edge_weight(edge_type: EdgeType, weight: f64) -> f64 {
    match edge_type {
        EdgeType::Arbitrage => weight.max(0.0), // spread bps → non-negative
        EdgeType::Correlation => weight.abs(),   // |ρ| → always non-negative
        EdgeType::Triangular => weight.max(0.0),
    }
}

/// Compute eigenvalues of a dense symmetric matrix via nalgebra.
///
/// Uses nalgebra's built-in `SymmetricEigen` which is pure Rust — no LAPACK required.
/// For our target scale (15-30 nodes), performance is single-digit microseconds.
fn dense_symmetric_eigenvalues(l: &[Vec<f64>], n: usize) -> Vec<f64> {
    use nalgebra::DMatrix;

    // Flatten row-major Vec<Vec<f64>> into a contiguous slice.
    let flat: Vec<f64> = l.iter().flat_map(|row| row.iter().copied()).collect();
    let m = DMatrix::from_row_slice(n, n, &flat);

    let eig = m.symmetric_eigen();
    let mut ev: Vec<f64> = eig.eigenvalues.as_slice().to_vec();
    // nalgebra does not guarantee sorted eigenvalues — sort ascending.
    ev.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ev
}

fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, EdgeId, EdgeOrigin, EdgeStatus, EdgeType, NodeId, NodeStatus, SheafGraph, Timeframe};

    /// 2-node graph connected by one edge: eigenvalues should be [0, 2w].
    #[test]
    fn test_two_node_laplacian() {
        let lap = laplacian_for_prices(&[100.0, 102.0]);
        assert_eq!(lap.node_count, 2);
        // λ₀ ≈ 0 (connected graph)
        assert!(lap.eigenvalues[0].abs() < 1e-10, "λ₀ should be 0, got {}", lap.eigenvalues[0]);
        // λ₁ > 0 (exact value depends on node ordering due to spread denominator)
        assert!(lap.eigenvalues[1] > 0.0, "λ₁ should be positive, got {}", lap.eigenvalues[1]);
        assert_eq!(lap.algebraic_connectivity, lap.eigenvalues[1]);
        assert!((lap.eigengap - lap.eigenvalues[1]).abs() < 1e-10);
        assert!((lap.spectral_radius - lap.eigenvalues[1]).abs() < 1e-10);
    }

    /// 3-node complete graph (triangle) with equal edges.
    #[test]
    fn test_three_node_triangle() {
        let lap = laplacian_for_prices(&[100.0, 100.5, 101.0]);
        assert_eq!(lap.node_count, 3);
        // λ₀ ≈ 0
        assert!(lap.eigenvalues[0].abs() < 1e-10, "λ₀ should be 0, got {}", lap.eigenvalues[0]);
        // With 3 nodes, eigenvalues should be [0, λ₁, λ₂] where λ₁, λ₂ > 0
        assert!(lap.eigenvalues[1] > 0.0, "λ₁ should be > 0");
        assert!(lap.eigenvalues[2] > 0.0, "λ₂ should be > 0");
        assert!(lap.eigengap > 0.0, "connected graph should have positive eigengap");
        assert!(lap.algebraic_connectivity > 0.0);
    }

    /// Graph with 2 nodes, 1 edge: verify λ₀=0, λ₁=2w (theoretical).
    #[test]
    fn test_two_node_laplacian_precise() {
        let lap = laplacian_for_prices(&[100.0, 200.0]);
        assert_eq!(lap.node_count, 2);
        // spread = (100/100)*10000 = 10000 bps
        // L = [[10000, -10000], [-10000, 10000]] → ev: [0, 20000]
        assert!(lap.eigenvalues[0].abs() < 1e-8, "λ₀ should be 0, got {}", lap.eigenvalues[0]);
        assert!((lap.eigenvalues[1] - 20000.0).abs() < 1e-6, "λ₁ should be 20000, got {}", lap.eigenvalues[1]);
    }

    /// Empty graph returns sensible defaults.
    #[test]
    fn test_empty_graph() {
        let lap = laplacian_for_prices(&[]);
        assert_eq!(lap.node_count, 0);
        assert!(lap.eigenvalues.is_empty());
        assert_eq!(lap.eigengap, 0.0);
        assert_eq!(lap.algebraic_connectivity, 0.0);
        assert_eq!(lap.spectral_radius, 0.0);
    }

    /// Single node: 1×1 matrix → eigenvalue is trivially 0.
    #[test]
    fn test_single_node() {
        let lap = laplacian_for_prices(&[100.0]);
        assert_eq!(lap.node_count, 1);
        assert_eq!(lap.eigenvalues.len(), 1);
        assert!(lap.eigenvalues[0].abs() < 1e-10);
        assert_eq!(lap.algebraic_connectivity, 0.0);
        assert_eq!(lap.eigengap, 0.0);
    }

    /// Verify eigenvalue ordering: λ₀ ≤ λ₁ ≤ ... ≤ λ_{n-1}.
    #[test]
    fn test_eigenvalues_are_sorted() {
        let lap = laplacian_for_prices(&[100.0, 101.0, 99.5, 102.0]);
        for i in 1..lap.eigenvalues.len() {
            assert!(
                lap.eigenvalues[i - 1] <= lap.eigenvalues[i] + 1e-10,
                "eigenvalues should be sorted: {:?}",
                lap.eigenvalues
            );
        }
    }

    // ── helpers ──

    /// Build a graph with n venues at different prices for the same symbol,
    /// then compute the Laplacian.
    fn laplacian_for_prices(prices: &[f64]) -> LaplacianResult {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;

        // Create watch targets with unique venues
        let watch_targets: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(i, _)| (format!("V{}", i), "ETH".to_string()))
            .collect();

        let mut graph = SheafGraph::new(
            watch_targets,
            vec![Timeframe::T1m],
            Default::default(),
        );

        // Inject prices into the pre-created nodes
        for (i, &price) in prices.iter().enumerate() {
            let id = NodeId {
                venue: format!("V{}", i),
                symbol: "ETH".into(),
                timeframe: Timeframe::T1m,
            };
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.last_price = Some(price);
                node.last_tick_ns = Some(now);
                node.status = NodeStatus::Active;
            }
        }

        // Create arbitrage edges between all pairs (deterministic order)
        let mut node_ids: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        node_ids.sort_by(|a, b| a.venue.cmp(&b.venue).then_with(|| a.symbol.cmp(&b.symbol)));
        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let a = &node_ids[i];
                let b = &node_ids[j];
                let pa = graph.nodes[a].last_price.unwrap();
                let pb = graph.nodes[b].last_price.unwrap();
                let weight = ((pa - pb).abs() / pa) * 10_000.0; // spread in bps

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

        compute_laplacian(&graph)
    }
}
