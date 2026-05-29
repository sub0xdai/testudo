//! Sheaf graph — nodes, edges, auto-discovery, and decay.
//!
//! The graph is auto-discovered from tick co-occurrence. Nodes are
//! created for every (venue, symbol, timeframe) in the watch list.
//! Edges are discovered as ticks flow:
//!
//! - Arbitrage edges: same symbol, different venues
//! - Correlation edges: any two symbols with sufficient history
//! - Triangular edges: three pairs forming a cycle at same venue
//!
//! Edges auto-decay when stale (per decision C).

// @anchor infra:sheaf:graph
// @tags infra

use crate::align::AlignedSnapshot;
use std::collections::{HashMap, HashSet};

// ── Node ──

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct NodeId {
    pub venue: String,
    pub symbol: String,
    pub timeframe: Timeframe,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Timeframe {
    T1s,
    T10s,
    T1m,
    T5m,
    T1h,
    T4h,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub status: NodeStatus,
    pub last_tick_ns: Option<i64>,
    pub last_price: Option<f64>,
    pub created_at: i64,
    /// Number of ticks received in the current window.
    pub tick_count_window: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Active,
    Stale { since: i64 },
    Down { since: i64 },
}

// ── Edge ──

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EdgeId {
    pub a: NodeId,
    pub b: NodeId,
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum EdgeType {
    Arbitrage,
    Correlation,
    Triangular,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub id: EdgeId,
    pub status: EdgeStatus,
    /// Edge weight (interpretation depends on edge_type):
    /// - Arbitrage: spread in basis points
    /// - Correlation: |Pearson ρ|, range [0, 1]
    /// - Triangular: imbalance in basis points
    pub weight: f64,
    pub discovered_at: i64,
    /// Origin of this edge.
    pub origin: EdgeOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeStatus {
    Active,
    Degraded { since: i64 },
    Removed { since: i64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeOrigin {
    Discovered,
    Hinted,
}

// ── Graph ──

/// The sheaf topology graph — discovered and maintained from tick data.
pub struct SheafGraph {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: HashMap<EdgeId, Edge>,

    /// Active watch targets from ConfigureGraph.
    watch_targets: HashSet<(String, String)>,

    /// Active timeframes from configuration.
    active_timeframes: Vec<Timeframe>,

    /// Monotonic version counter. Incremented on structural changes.
    pub version: u64,

    /// Configurable thresholds.
    config: GraphConfig,
}

#[derive(Debug, Clone)]
pub struct GraphConfig {
    /// Minimum absolute Pearson ρ to create a correlation edge.
    pub correlation_threshold: f64, // default: 0.3
    /// Number of 4h candles before correlation is computed.
    pub correlation_min_history: usize, // default: 30
    /// Minimum spread in bps to emit an arbitrage signal.
    pub arbitrage_signal_threshold_bps: f64, // default: 5.0
    /// Staleness thresholds.
    pub arbitrage_staleness_secs: u64, // default: 30
    pub correlation_decay_threshold: f64, // default: 0.3
    pub correlation_decay_windows: usize, // default: 12
    pub triangular_staleness_secs: u64, // default: 30
    pub node_stale_secs: u64, // default: 15
    pub node_down_secs: u64, // default: 60
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            correlation_threshold: 0.3,
            correlation_min_history: 30,
            arbitrage_signal_threshold_bps: 5.0,
            arbitrage_staleness_secs: 30,
            correlation_decay_threshold: 0.3,
            correlation_decay_windows: 12,
            triangular_staleness_secs: 30,
            node_stale_secs: 15,
            node_down_secs: 60,
        }
    }
}

impl SheafGraph {
    /// Create an empty graph with the given config and watch targets.
    pub fn new(
        watch_targets: Vec<(String, String)>,
        timeframes: Vec<Timeframe>,
        config: GraphConfig,
    ) -> Self {
        debug_assert!(
            !watch_targets.is_empty(),
            "must have at least one watch target"
        );
        debug_assert!(
            !timeframes.is_empty(),
            "must have at least one timeframe"
        );

        let watch_set: HashSet<_> = watch_targets.into_iter().collect();

        let mut graph = Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            watch_targets: watch_set,
            active_timeframes: timeframes,
            version: 0,
            config,
        };

        // Pre-create nodes for every watch target × timeframe.
        graph.ensure_all_nodes();

        graph
    }

    /// Ensure nodes exist for all watch targets at all active timeframes.
    fn ensure_all_nodes(&mut self) {
        let now = now_ns();
        let targets: Vec<_> = self.watch_targets.iter().cloned().collect();
        for (venue, symbol) in targets {
            for &timeframe in &self.active_timeframes {
                let id = NodeId {
                    venue: venue.clone(),
                    symbol: symbol.clone(),
                    timeframe,
                };
                self.nodes.entry(id).or_insert_with(|| Node {
                    id: NodeId {
                        venue: venue.clone(),
                        symbol: symbol.clone(),
                        timeframe,
                    },
                    status: NodeStatus::Stale { since: now },
                    last_tick_ns: None,
                    last_price: None,
                    created_at: now,
                    tick_count_window: 0,
                });
            }
        }
    }

    /// Ingest an aligned snapshot: update node state, discover edges.
    pub fn ingest(&mut self, snapshot: &AlignedSnapshot) -> Vec<EdgeId> {
        let now = now_ns();
        debug_assert!(now > 0, "timestamp must be positive");

        let mut new_edges = Vec::new();

        // 1. Update node states from the snapshot.
        self.update_nodes_from_snapshot(snapshot, now);

        // 2. Apply decay to existing nodes and edges.
        self.apply_decay(now);

        // 3. Discover new edges.
        self.discover_arbitrage_edges(&mut new_edges);
        // TODO: discover correlation edges (needs price history ring buffers)
        // TODO: discover triangular edges (needs symbol adjacency graph)
        // TODO: discover timeframe stalk edges (volatility diffusion)

        if !new_edges.is_empty() {
            self.version += 1;
        }

        new_edges
    }

    /// Update node last_tick and status from snapshot data.
    fn update_nodes_from_snapshot(&mut self, snapshot: &AlignedSnapshot, _now: i64) {
        let df = &snapshot.df;
        if df.is_empty() {
            return;
        }

        // Extract columns from the DataFrame
        let venues = df.column("venue").and_then(|c| c.str()).ok();
        let symbols = df.column("symbol").and_then(|c| c.str()).ok();
        let prices = df.column("price").and_then(|c| c.f64()).ok();
        let event_tss = df.column("event_ts").and_then(|c| c.i64()).ok();

        if let (Some(venues), Some(symbols), Some(prices), Some(event_tss)) =
            (venues, symbols, prices, event_tss)
        {
            for row in 0..df.height() {
                let venue = venues.get(row).unwrap_or("");
                let symbol = symbols.get(row).unwrap_or("");
                let price = prices.get(row).unwrap_or(0.0);
                let event_ts = event_tss.get(row).unwrap_or(0);

                // Update all timeframes for this (venue, symbol)
                for timeframe in &self.active_timeframes {
                    let id = NodeId {
                        venue: venue.to_string(),
                        symbol: symbol.to_string(),
                        timeframe: *timeframe,
                    };
                    if let Some(node) = self.nodes.get_mut(&id) {
                        node.last_tick_ns = Some(event_ts);
                        node.last_price = Some(price);
                        node.tick_count_window += 1;
                        node.status = NodeStatus::Active;
                    }
                }
            }
        }
    }

    /// Apply node and edge decay.
    fn apply_decay(&mut self, now: i64) {
        debug_assert!(now > 0, "decay timestamp must be positive");
        debug_assert!(
            self.config.node_down_secs > self.config.node_stale_secs,
            "node_down_secs ({}) must exceed node_stale_secs ({})",
            self.config.node_down_secs,
            self.config.node_stale_secs
        );

        let node_stale_ns = self.config.node_stale_secs as i64 * 1_000_000_000;
        let node_down_ns = self.config.node_down_secs as i64 * 1_000_000_000;

        // Node decay
        for node in self.nodes.values_mut() {
            if let Some(last_tick) = node.last_tick_ns {
                let age = now - last_tick;
                if age > node_down_ns {
                    node.status = NodeStatus::Down { since: last_tick + node_down_ns };
                } else if age > node_stale_ns {
                    node.status = NodeStatus::Stale {
                        since: last_tick + node_stale_ns,
                    };
                }
            }
        }

        // Edge decay: if either endpoint node is stale/down, edge degrades.
        // Full decay logic (correlation ρ tracking, window counting) is TODO.
        let stale_node_ids: HashSet<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, n)| !matches!(n.status, NodeStatus::Active))
            .map(|(id, _)| id.clone())
            .collect();

        for edge in self.edges.values_mut() {
            if (stale_node_ids.contains(&edge.id.a) || stale_node_ids.contains(&edge.id.b))
                && matches!(edge.status, EdgeStatus::Active)
            {
                edge.status = EdgeStatus::Degraded { since: now };
            }
        }
    }

    /// Discover arbitrage edges: same symbol, different venues.
    fn discover_arbitrage_edges(&mut self, new_edges: &mut Vec<EdgeId>) {
        let now = now_ns();

        // Group active nodes by (symbol, timeframe)
        let mut by_symbol_tf: HashMap<(String, Timeframe), Vec<&NodeId>> = HashMap::new();
        for (id, node) in &self.nodes {
            if node.status == NodeStatus::Active && node.last_price.is_some() {
                by_symbol_tf
                    .entry((id.symbol.clone(), id.timeframe))
                    .or_default()
                    .push(id);
            }
        }

        for ((_symbol, _timeframe), node_ids) in &by_symbol_tf {
            if node_ids.len() < 2 {
                continue;
            }
            for i in 0..node_ids.len() {
                for j in (i + 1)..node_ids.len() {
                    let a = node_ids[i];
                    let b = node_ids[j];
                    if a.venue == b.venue {
                        continue; // different venues only
                    }

                    // Compute spread from node prices.
                    let node_a = self.nodes.get(a)
                        .expect("node must exist: id came from active nodes");
                    let node_b = self.nodes.get(b)
                        .expect("node must exist: id came from active nodes");
                    if let (Some(pa), Some(pb)) = (node_a.last_price, node_b.last_price) {
                        let spread_bps = ((pa - pb).abs() / pa) * 10_000.0;

                        let edge_id = EdgeId {
                            a: a.clone(),
                            b: b.clone(),
                            edge_type: EdgeType::Arbitrage,
                        };

                        self.edges
                            .entry(edge_id.clone())
                            .and_modify(|e| {
                                e.weight = spread_bps;
                                if matches!(e.status, EdgeStatus::Degraded { .. }) {
                                    e.status = EdgeStatus::Active;
                                }
                            })
                            .or_insert_with(|| {
                                let edge = Edge {
                                    id: edge_id.clone(),
                                    status: EdgeStatus::Active,
                                    weight: spread_bps,
                                    discovered_at: now,
                                    origin: EdgeOrigin::Discovered,
                                };
                                new_edges.push(edge_id);
                                edge
                            });
                    }
                }
            }
        }
    }

    // ── Graph queries ──

    pub fn active_node_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Active)
            .count()
    }

    pub fn stale_node_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| matches!(n.status, NodeStatus::Stale { .. }))
            .count()
    }

    pub fn down_node_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| matches!(n.status, NodeStatus::Down { .. }))
            .count()
    }

    pub fn active_edge_count(&self) -> usize {
        self.edges
            .values()
            .filter(|e| e.status == EdgeStatus::Active)
            .count()
    }

    pub fn degraded_edge_count(&self) -> usize {
        self.edges
            .values()
            .filter(|e| matches!(e.status, EdgeStatus::Degraded { .. }))
            .count()
    }

    pub fn edge_count_by_type(&self, edge_type: EdgeType) -> usize {
        self.edges
            .values()
            .filter(|e| e.id.edge_type == edge_type)
            .count()
    }

    /// Access the graph configuration.
    pub(crate) fn config(&self) -> &GraphConfig {
        &self.config
    }
}

/// Current wall-clock time in nanoseconds since Unix epoch.
///
/// Falls back to 0 if the system clock is before 1970 (should never happen).
pub(crate) fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}
