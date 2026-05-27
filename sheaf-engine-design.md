# Sheaf Engine Design — Brainstorming Notes

**Date:** 2026-05-25
**Status:** In progress — reconvene 2026-05-26

---

## Concept

Integrate cellular sheaf topology with Testudo as a standalone perception crate.
The sheaf engine ingests multi-venue tick data, computes topological signals
(arbitrage, volatility diffusion, regime consistency), and exposes them to
any AI agent harness (OpenClaw, Hermes, pi, custom Rust). The agent reads
these signals, combines them with journal memory and Lean-verified strategy
proofs, then calls Testudo's API to execute.

**Not** a replacement for Testudo's engine. A new layer between market data
and the AI agent.

---

## Architecture Decisions So Far

### 1. Standalone crate, not inside Testudo

The sheaf engine has radically different compute needs (graph Laplacian,
sparse matrices, tick-aligned SIMD) vs Testudo (order matching, SQL, risk
validation). They share zero infrastructure. The sheaf engine is a perception
crate that the AI agent consumes alongside Testudo's execution API.

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Sheaf Engine │────▶│  AI Agent    │────▶│   Testudo    │
│ (perception) │     │ (orchestr.)  │     │ (execution)  │
└──────────────┘     └──────────────┘     └──────────────┘
```

### 2. Data-agnostic compute layer

The sheaf engine doesn't care where ticks come from. A `TickSource` trait
with two implementations:
- `ExchangeSource` — direct WebSocket connections (standalone deployment)
- `TestudoSource` — wraps Testudo's existing WS pipes (co-deployed)

The engine receives `TickBatch` structs — columnar arrays of (venue, symbol,
price, timestamp, size) laid out for efficient processing.

### 3. Auto-discovered graph topology

The agent tells the engine which venues and symbols to watch. The engine
auto-discovers the graph as ticks flow:

| Edge type | Condition | What failure means |
|-----------|-----------|-------------------|
| Arbitrage | Same symbol, different venues | Price dislocation |
| Triangular | Related pairs at same venue (e.g. BTC/USDT → ETH/BTC → ETH/USDT) | Structural mispricing |
| Correlation | Price co-movement over time | Decoupling — regime shift |

Nodes exist at multiple timeframes (1s, 10s, 1m). The sheaf maps how
information flows up the timeframe ladder.

### 4. gRPC bidirectional stream as primary interface

One connection, all data flows. The agent gets a `Stream<Item = SignalBatch>`.

```
Agent ──ConfigureGraph──▶ Sheaf Engine
      ◀──SignalStream────
```

REST fallback (`GET /snapshot`) for harnesses that don't support gRPC
streaming. Universal compatibility.

### 5. Harness-agnostic output

The sheaf engine works with any harness (OpenClaw, Hermes, pi, Rust) and
any LLM (Claude, GPT, Gemini, local). The output is plain JSON structs.

The harness translates sheaf signals into natural language context injected
into the LLM's prompt. The LLM never touches raw topology data — it reads
structured digests and decides which strategy to apply.

### 6. SIMD scoping

The sheaf Laplacian for realistic graphs (15–30 nodes, 2–3 venues) is micro
linear algebra. Python + NumPy handles this in single-digit microseconds.
SIMD is documented as a future optimization — spec it, but implementation
starts with vectorized NumPy/Mojo hot loops. The real bottlenecks will be
tick ingestion and topology sync, not matrix multiply.

---

## Resolved Design Decisions (2026-05-26)

### A. Output abstraction → Option C: Topology-enriched structural observations

**Decision: Enriched topology, not raw math and not strategy prescription.**

Option A (raw Laplacian spectra) forces the LLM to be a topologist — it won't
reason correctly about eigenvalues. Option B ("execute FUNDING_ARB") makes
the sheaf a strategy engine — but the LLM + Lean proofs already own strategy
selection (see `strat-lean-proofs.md` §5 agent loop). The sheaf is a
**perception** layer, not a strategy layer.

**Chosen format:** Structured topology observations with enough context that
the LLM can weigh them alongside journal memory and regime detection, but the
sheaf never says "trade."

```json
{
  "timestamp": "2026-05-26T14:30:00Z",
  "graph": {
    "nodes": 18,
    "edges": {
      "arbitrage": 6,
      "correlation_pairs": 12,
      "triangular": 3
    }
  },
  "topology_signals": [
    {
      "type": "ARBITRAGE_EDGE",
      "edge": {"from": "ETH:BN", "to": "ETH:HL"},
      "divergence_bps": 23,
      "spread_usd": 2.15,
      "severity": "notable",
      "context": "ETH spot spread 23 bps BN→HL. 30s avg is 4 bps. 5.75σ anomaly."
    },
    {
      "type": "CORRELATION_BREAK",
      "pair": ["BTC_USDT", "ETH_USDT"],
      "rolling_rho_4h": 0.47,
      "baseline_rho_30d": 0.82,
      "decoupling_since": "2026-05-26T10:00:00Z",
      "context": "BTC-ETH correlation dropped from 0.82 to 0.47 over 4h. Possible regime shift in progress."
    },
    {
      "type": "VOLATILITY_DIFFUSION",
      "direction": "upward",
      "source_timeframe": "1m",
      "target_timeframe": "5m",
      "strength": 0.73,
      "context": "1m vol spike diffusing to 5m sheaf stalk. 73% propagation strength suggests genuine vol regime shift, not noise."
    },
    {
      "type": "TRIANGULAR_MISPRICING",
      "venue": "BN",
      "path": ["BTC_USDT", "ETH_BTC", "ETH_USDT"],
      "imbalance_bps": 8,
      "context": "Triangular path BTC→ETH→BTC at Binance shows 8 bps imbalance. Below execution friction (~12 bps) — no trade."
    }
  ],
  "health": {
    "perception_confidence": 0.94,
    "active_nodes": 18,
    "stale_nodes": 0,
    "degraded_edges": 0,
    "broken_edges": 0
  }
}
```

Each signal includes a `context` field — a one-line natural language summary
that the harness injects directly into the LLM's prompt. The LLM reads this,
not raw topology data. The `severity` field tells the agent whether to act
(`critical` → immediate attention), note (`notable` → factor in), or ignore
(`info` → background reading).

**Why not Option B:** Strategy prescription creates a coupling problem.
Testudo strategies evolve (new ones added, parameters tuned). If the sheaf
hardcodes `FUNDING_ARB` as a signal type, every strategy change requires a
sheaf update. The sheaf should outlive individual strategies — it's a market
structure sensor, not a trading system.

**Why not Option A:** The LLM cannot be expected to compute Wasserstein
distances or interpret Laplacian eigenvalues. The existing regime detection
loop (`strat-lean-proofs.md` §1) already computes W₁ — the LLM orchestrates
that computation, it doesn't perform it. The sheaf should emit digestible
observations, not mathematical primitives.

### B. Tick source priority → Prefer-direct, single config flag

**Decision: `ExchangeSource` (direct WebSocket) is the default and preferred
source. `TestudoSource` is a co-deployment convenience. Controlled by one
config flag, not per-edge.**

```rust
enum TickSourcePriority {
    Direct,      // default — use ExchangeSource for everything
    Merge,       // merge both sources, deduplicate by (venue, symbol, ts)
    PreferTestudo // use TestudoSource, fall back to ExchangeSource
}
```

**Rationale:** The sheaf is an independent crate. It should own its data
pipeline. Reasons for preferring direct:

1. **Latency:** Arbitrage edge detection needs the fastest possible tick
   arrival. Routing through Testudo's WS → order book → re-emit adds ~5-15ms.
2. **Failure isolation:** If Testudo is down for maintenance, the sheaf
   continues running. If the sheaf crashes, Testudo keeps matching orders.
3. **Single responsibility:** Testudo validates orders and manages risk.
   The sheaf processes topology. No reason to couple them at the data layer.

`TestudoSource` exists for convenience: when both are deployed on the same
machine or k8s cluster, the operator may want to avoid duplicate exchange
WebSocket connections (rate limits, bandwidth). In that case, set
`source_priority = "PreferTestudo"` or `"Merge"` and the sheaf reads from
Testudo's existing pipes.

**Deduplication in `Merge` mode:** Ticks are matched by `(venue, symbol,
timestamp_ms)`. If both sources deliver the same tick within a 5ms window,
the Direct tick wins (lower latency path). Testudo ticks fill gaps when
Direct misses (e.g., WebSocket reconnect).

### C. Graph mutation → Auto-decay only, no explicit API

**Decision: The engine auto-discovers edges and auto-decays them. No
`POST /graph/edges/remove` — the agent never manages the graph.**

Edges that auto-discover should auto-decay. If the agent had to explicitly
prune stale edges, it would need to understand graph topology — wrong
abstraction. The agent says "watch these venues and symbols," and the engine
handles graph evolution internally.

**Decay rules:**

| Edge type | Decay trigger | Default threshold | Configurable |
|-----------|--------------|-------------------|--------------|
| Arbitrage | No matching tick pair for T seconds | 30s | `arbitrage_edge_staleness_secs` |
| Correlation | Rolling ρ drops below threshold for M consecutive evaluation windows | ρ < 0.3 for 12 windows (48h at 4h candles) | `correlation_decay_threshold`, `correlation_decay_windows` |
| Triangular | Any component leg has no data for T seconds | 30s | `triangular_edge_staleness_secs` |

**Decay lifecycle:**

```
[active] ──staleness thresh exceeded──▶ [degraded] ──2× thresh exceeded──▶ [removed]
    │                                        │
    └──new data arrives──▶ [active] ◀────────┘
```

- **active:** Edge is tracked and contributes to topology signals.
- **degraded:** Edge is tracked but weighted at 0.3× in signal computation.
  A `CORRELATION_DECAYING` signal is emitted.
- **removed:** Edge is dropped from the active graph. A history entry is
  kept for 24h in case it re-emerges (fast re-activation without cold start).

The `degraded` phase is a buffer — prevents flapping when correlation briefly
dips then recovers. The edge must stay degraded for the full decay window
before removal.

**No explicit add API either:** The agent sends a `ConfigureGraph` gRPC
message with watch targets (venue+symbol pairs and optionally explicit
edge hints). The engine discovers edges from tick co-occurrence. If the
agent wants to force-track an edge (e.g., a known triangular path), it
includes it in `ConfigureGraph.edges` as a hint — the engine will track
it even without tick co-occurrence, but will still auto-decay it.

### D. Failure modes → Health signals at three granularities

**Decision: The sheaf emits health data at node level, edge level, and graph
level. A top-level `perception_confidence` score gates agent decisions.**

The agent already has HaltExecution logic (`AGENT_TRADING.md` §7.5,
`strat-lean-proofs.md` §4.3). The sheaf integrates by providing a new halt
trigger: `perception_confidence < threshold`.

**Health data model (emitted in every `SignalBatch` and via `GET /health`):**

```json
{
  "health": {
    "perception_confidence": 0.94,
    "aggregate": {
      "connected_components": 1,
      "largest_component_size": 18,
      "isolated_nodes": 0,
      "is_connected": true
    },
    "nodes": [
      {
        "id": "ETH:BN",
        "status": "active",
        "last_tick_ts": 1716413400,
        "tick_gap_max_ms": 120,
        "tick_count_1m": 47
      },
      {
        "id": "BTC:HL",
        "status": "stale",
        "last_tick_ts": 1716413200,
        "stale_since": "2026-05-26T14:25:40Z",
        "suspected_cause": "no_tick_data"
      }
    ],
    "edges": [
      {
        "id": "ETH:BN→ETH:HL",
        "type": "arbitrage",
        "status": "active"
      },
      {
        "id": "BTC_USDT↔ETH_USDT",
        "type": "correlation",
        "status": "degraded",
        "degraded_since": "2026-05-26T10:00:00Z",
        "current_rho": 0.47,
        "baseline_rho": 0.82
      }
    ]
  }
}
```

**`perception_confidence` formula:**

```
perception_confidence = w_nodes × node_health + w_edges × edge_health

node_health = active_nodes / total_nodes
edge_health = (active_edges + 0.3 × degraded_edges) / total_edges

w_nodes = 0.4   # data availability is important but recoverable
w_edges = 0.6   # topology quality is the actual product
```

**Agent behavior by confidence level:**

| Confidence | Agent action |
|------------|-------------|
| ≥ 0.80 | Normal operation. Use sheaf signals for regime/structure context. |
| 0.60–0.79 | **Degraded mode.** Sheaf signals are advisory only. Agent falls back to raw OHLCV regime detection (Wasserstein, as currently implemented in `strat-lean-proofs.md` §1). Journal note logged. |
| < 0.60 | **HaltExecution.** Sheaf is unreliable. Agent halts, logs reason, sleeps until next evaluation. Does not attempt raw OHLCV — the sheaf signals are the primary perception layer, and if they're unreliable, trading blind is worse than not trading. |
| 0.00 (sheaf unreachable) | **Hard halt.** The sheaf gRPC stream is dead. Agent halts until stream reconnects. |

**Failure scenarios and sheaf response:**

| Scenario | Sheaf behavior | perception_confidence |
|----------|---------------|----------------------|
| 1 venue down (BN WebSocket dies) | BN nodes → `stale`. Arbitrage edges involving BN → `degraded`. Correlation edges recalculate without BN. | ~0.75 (depends on graph size) |
| All venues up, tick gaps | Affected nodes → `stale` with `tick_gap_max_ms` rising. No edges break if gaps are sub-staleness threshold. | Drops proportionally |
| Graph disconnected (e.g., only ETH data, no BTC) | `connected_components` > 1. Isolated nodes flagged. Edge health drops. | Could drop below 0.60 depending on fragmentation |
| Sheaf process crash | gRPC stream closes. Agent detects stream termination. | 0.00 → hard halt |

**Node status machine:**

```
[active] ──no tick for 15s──▶ [stale] ──no tick for 60s──▶ [down]
    │                            │
    └──tick arrives──▶ [active] ◀┘
```

`stale` nodes still contribute to graph topology but are weighted at 0.3×.
`down` nodes are removed from active topology and their edges are dropped.

### E. SIMD implementation path → Profiling gate, unlikely to fire at target scale

**Decision: Document the trigger, don't implement. At target scale
(15–30 nodes, 2–3 venues, < 1k ticks/sec), vectorized NumPy/Mojo hot loops
handle the Laplacian in single-digit microseconds.**

**Trigger condition:** When `laplacian_compute_ms` exceeds 10% of the tick
window budget for 3 consecutive windows, profile and consider SIMD.

```
tick_window_budget = 100ms  # configurable: how often the sheaf recomputes
laplacian_compute_ms = measured via monotonic clock before/after Laplacian

# Emitted as a metric in the gRPC health stream
if laplacian_compute_ms > tick_window_budget * 0.1:
    emit_metric("laplacian_budget_pressure", laplacian_compute_ms / tick_window_budget)
```

**Realistic scale check:**
- 3 venues × 30 symbols = 90 nodes max (and most are inactive)
- Laplacian is |V|×|V| sparse matrix — 90×90 = 8,100 entries, mostly zeros
- NumPy `scipy.sparse.linalg.eigsh` handles this in ~50μs
- At 10 recomputes/sec (100ms window), that's 0.05% CPU

**Scenarios where SIMD becomes relevant:**

| Trigger | Threshold | Likelihood at target scale |
|---------|-----------|---------------------------|
| Graph nodes > 200 | Unlikely with 2–3 venues | Very low |
| Tick rate > 10,000/sec | Per-venue capacity | Low (BN does ~1k/sec for all symbols) |
| Laplacian recompute > 10ms | Profiling gate | Won't fire at < 100 nodes |

**Implementation note:** When the profiling gate fires, the SIMD path
should target the Laplacian assembly step (constructing L = D - A from
tick-aligned adjacency matrices). This is the only operation that scales
with graph size. The eigenvalue computation itself is handled by LAPACK
and is already optimized.

---

### F. Tick alignment across venues → Adopt three existing patterns, no reinvention

**Decision: Three well-established patterns from market data infrastructure.
No novel alignment logic needed.**

This is a solved problem in HFT and market data pipelines. We adopt:

1. **Two-timestamp model** — every tick carries both `ts` (local arrival) and
   `event_ts` (exchange time). Staleness uses `ts`. Alignment uses `event_ts`.
   They never cross paths.
2. **Exchange clock synchronization** — RTT-based offset estimation per venue
   with EMA smoothing. Normalizes all `event_ts` to a common timebase.
3. **As-of join for alignment** — Polars `join_asof_by()` in Rust, partitioned
   by (venue, symbol), with configurable tolerance. Standard primitive.

---

#### Pattern 1: Two-timestamp model

Every tick entering the sheaf engine carries:

```rust
struct Tick {
    venue: VenueId,
    symbol: SymbolId,
    price: f64,
    size: f64,
    event_ts: i64,  // exchange timestamp — used for CROSS-VENUE ALIGNMENT
    ts: i64,        // local arrival wall clock — used for STALENESS DETECTION
}
```

**Why two timestamps:** A single timestamp conflates two concerns. Using
exchange time for staleness gives false alarms when a venue throttles its
push rate. Using arrival time for alignment produces fake "consensus"
snapshots where ticks that arrived together are treated as simultaneous
when the underlying events were 300ms apart.

**Precedent:** MarketTrace (markettrace.ai) ships this pattern in production
for cross-exchange order book aggregation. The `cross_exchange_skew_ms`
metric they expose (spread between earliest and latest `event_ts` in a
snapshot) becomes a first-class health signal in our sheaf.

```
cross_exchange_skew_ms = max(event_ts) - min(event_ts)  // across all venues in window

// Emitted in every SignalBatch health block:
// "Consensus over a 47ms window" — honest, not hidden
```

#### Pattern 2: Exchange clock synchronization (FLOX-style)

Exchanges use NTP, but NTP accuracy varies under load, after maintenance,
and during NTP steps. A venue's clock can drift 100-300ms from true time.
For arbitrage signals, this matters — a 23 bps spread might be real or might
be a clock artifact.

**Solution:** Per-venue RTT-based offset estimation with EMA smoothing,
as implemented in FLOX (`ExchangeClockSync`, open-source C++ header-only).

```rust
struct ExchangeClock {
    venue: VenueId,
    offset_ns: i64,      // estimated offset from local clock to exchange clock
    confidence_ns: i64,   // 95% confidence interval half-width
    latency_ns: i64,      // estimated one-way latency
    ema_alpha: f64,       // smoothing factor (default 0.1)
}

impl ExchangeClock {
    /// Update offset estimate from RTT measurement.
    /// Called on every heartbeat/pong from the venue.
    fn update(&mut self, rtt_ns: i64, server_time_ns: i64) {
        let local_time = monotonic_clock();
        let measured_offset = server_time_ns - (local_time - rtt_ns / 2);
        self.offset_ns = (self.ema_alpha * measured_offset as f64
            + (1.0 - self.ema_alpha) * self.offset_ns as f64) as i64;
    }

    /// Convert exchange timestamp to normalized sheaf time.
    fn to_sheaf_time(&self, event_ts: i64) -> i64 {
        event_ts - self.offset_ns
    }
}
```

**FLOX reference:** `github.com/flox-foundation/flox` — `ExchangeClockSync`
provides `<1ns` amortized read cost after sync, with RTT-based offset
estimation and configurable EMA alpha. We implement the same pattern in Rust.

**Why not skip clock sync:** For most edges (correlation, volatility
diffusion), 100ms clock drift doesn't matter — these operate on 4h windows.
For arbitrage edges, it's critical. A 23 bps spread that's actually 3 bps
after clock correction is a false signal. Clock sync turns "maybe arb" into
"definitely arb" or "definitely not."

#### Pattern 3: As-of join for alignment (Polars ASOF)

Once all ticks have a normalized `sheaf_time` (via clock sync), we align
them using as-of join — the standard primitive for "find the latest value
as of time t" in time-series databases.

**Rust implementation:** Polars provides `DataFrame::join_asof_by()` with:
- **Partition columns:** `(venue, symbol)` — each partition aligned independently
- **Strategy:** `Backward` (latest tick at or before sheaf_time), `Forward`
  (earliest tick at or after), `Nearest` (whichever is closer)
- **Tolerance:** maximum time gap before a match is considered invalid

```rust
use polars::prelude::*;

fn align_ticks(
    ticks: DataFrame,       // all ticks, across venues
    alignment_times: DataFrame, // the times we want snapshots for
    tolerance_ms: i64,
) -> Result<DataFrame> {
    ticks.join_asof_by(
        &alignment_times,
        "ts",                    // left column: alignment times
        "sheaf_time",            // right column: tick timestamps
        ["venue", "symbol"],     // partition by (left)
        ["venue", "symbol"],     // partition by (right)
        AsofStrategy::Backward,  // latest tick at or before alignment time
        Some(AnyValue::Int64(tolerance_ms * 1_000_000)), // tolerance in ns
        false,                   // allow_eq
        false,                   // check_sortedness (data is pre-sorted by source)
    )
}
```

**Why Polars and not a custom implementation:**
- ASOF join is non-trivial to implement correctly (sorted merge with
  inequality predicates, partition parallelism). Polars has this battle-tested.
- Polars uses Apache Arrow columnar layout — same memory representation
  as the `TickBatch` structs we're already designing.
- Polars is a single Rust dependency (`cargo add polars --features asof_join`).
- At our scale (3 venues, 30 symbols, ~1k ticks/sec), Polars DataFrame
  operations are microsecond-fast.

**Precedent:** Polars ASOF join is used in production by Dune Analytics
(DuneSQL), LaminarDB (streaming SQL for market data), and the broader
Rust data engineering ecosystem.

#### Alignment configuration

```rust
struct AlignmentConfig {
    /// Strategy: always Backward for sheaf — we want the latest known
    /// state at each alignment boundary, not a forward-looking value.
    strategy: AsofStrategy,  // Backward

    /// Maximum age of a tick before the node is considered stale.
    /// Default 500ms. Covers worst-case tick intervals on active crypto
    /// venues (100-500ms between ticks per symbol).
    tolerance_ms: u64,  // 500

    /// How often alignment snapshots are produced.
    window_ms: u64,  // 100 (10 snapshots/sec)

    /// Minimum number of venues that must be active for a valid snapshot.
    /// If fewer venues have data, the snapshot is annotated as degraded.
    min_active_venues: usize,  // 1 (don't hard-fail on partial data)
}
```

**What alignment does NOT do:**
- Does not interpolate prices between ticks. If no tick exists within
  tolerance, the node is null for that window.
- Does not forward-fill indefinitely. A node stale for > tolerance_ms
  is excluded from the Laplacian for that window.
- Does not synchronize arrival order. The as-of join is deterministic:
  same input → same output.

#### Integration with the agent

The `cross_exchange_skew_ms` metric flows into the `health` block of every
`SignalBatch`. The agent can use it as an additional gate:

| Skew (ms) | Interpretation | Agent action |
|-----------|---------------|-------------|
| < 50 | All venues' clocks are tightly synced | Normal operation — sheaf signals are reliable |
| 50–200 | Minor skew — within normal NTP drift bounds | Degraded mode — arbitrage signals may have clock artifacts. Correlation signals unaffected. |
| > 200 | Significant skew — possible NTP step or venue clock issue | Flag in journal. Sheaf continues but perception_confidence is docked by ~0.1. |
| > 1000 | Venue clock is broken (maintenance, NTP failure) | Downgrade affected venue to `degraded`. Its nodes get `status: "clock_drift"` in health output. |

---

## TickSource Trait & TickBatch Format (2026-05-26)

### Design principles

1. **Arrow-native columnar format** — `TickBatch` is an Apache Arrow `RecordBatch`.
   Zero-copy into Polars for ASOF join alignment. Same memory layout throughout.
2. **Stream-based, not poll-based** — tick sources implement `Stream<Item =
   Result<TickBatch>>`. Backpressure by consumer pulling. No internal buffering
   beyond the micro-batcher ring buffer.
3. **Normalization at source boundary** — exchange-specific JSON parsing happens
   inside each `TickSource` impl. The sheaf engine never sees raw exchange payloads.
4. **Sources own their lifecycle** — connect, reconnect (exponential backoff),
   health reporting, shutdown. The sheaf engine orchestrates multiple sources
   but each source manages its own connection.

### Precedent crates

| Crate | What it provides | What we adopt |
|-------|-----------------|---------------|
| `fin-stream` (Rust) | `WsManager` with exponential-backoff reconnect, `TickNormalizer` for exchange → canonical form, SPSC ring buffer, `HealthMonitor` for per-feed staleness | Reconnect policy, error taxonomy, health model |
| `arrow` / `arrow-array` (Rust) | `RecordBatch`, columnar arrays, zero-copy IPC | `TickBatch` schema and column layout |
| `barter-data` (Rust) | `StreamBuilder` pattern — compose subscriptions → `MarketStream` | Builder API for configuring sources |
| `nt-market-data` (Rust) | Multi-provider trait abstraction, `Stream<Item = MarketData>` | Trait-based source dispatch |

### Tick struct

Every tick, after normalization, carries two timestamps (decision F):

```rust
use arrow::datatypes::{Schema, Field, DataType};

/// A single normalized tick from any venue.
/// Not used individually in the hot path — ticks are always batched.
#[derive(Debug, Clone)]
struct Tick {
    venue: String,       // "BN", "HL", "BYBIT"
    symbol: String,      // "ETH_USDT", "BTC_USDT"
    price: f64,
    size: f64,
    event_ts: i64,       // exchange timestamp (nanoseconds since epoch)
    ts: i64,             // local arrival timestamp (monotonic clock, nanoseconds)
}
```

### TickBatch format (Arrow RecordBatch)

The hot-path unit of work. A batch of ticks collected over a micro-window
(default 100ms). Stored as an Apache Arrow `RecordBatch` for direct handoff
to Polars.

```rust
/// Columnar batch of ticks. Wraps an Arrow RecordBatch.
/// Schema:
///   venue:   Dictionary<Int32, Utf8>  — dictionary-encoded for small cardinality
///   symbol:  Dictionary<Int32, Utf8>  — dictionary-encoded
///   price:   Float64
///   size:    Float64
///   event_ts: Timestamp(Nanosecond, None)  — exchange time
///   ts:       Timestamp(Nanosecond, None)  — local arrival time
struct TickBatch {
    batch: RecordBatch,
    /// Wall clock when the batch window closed.
    window_close_ts: i64,
    /// Number of raw ticks received in this window (before dedup).
    raw_count: usize,
}

impl TickBatch {
    fn schema() -> Schema {
        Schema::new(vec![
            Field::new("venue", DataType::Dictionary(
                Box::new(DataType::Int32), Box::new(DataType::Utf8)
            ), false),
            Field::new("symbol", DataType::Dictionary(
                Box::new(DataType::Int32), Box::new(DataType::Utf8)
            ), false),
            Field::new("price", DataType::Float64, false),
            Field::new("size", DataType::Float64, false),
            Field::new("event_ts", DataType::Timestamp(
                TimeUnit::Nanosecond, None
            ), false),
            Field::new("ts", DataType::Timestamp(
                TimeUnit::Nanosecond, None
            ), false),
        ])
    }

    /// Convert to Polars DataFrame for ASOF join alignment.
    /// Zero-copy — Polars uses Arrow under the hood.
    fn into_polars(self) -> polars::prelude::DataFrame {
        DataFrame::from(self.batch)
    }

    fn len(&self) -> usize { self.batch.num_rows() }
    fn is_empty(&self) -> bool { self.len() == 0 }
}
```

**Why Dictionary encoding for venue/symbol:** With 3 venues and ~30 symbols,
the cardinality is tiny. Dictionary encoding stores the string once and uses
a compact integer index per row. Saves ~60 bytes per tick and improves
cache locality.

**Why not struct-of-arrays manually:** Arrow `RecordBatch` is the Rust data
engineering standard. Zero-copy into Polars. Zero-copy IPC if we ever split
the sheaf across processes. `RecordBatch` also gives us Parquet serialization
for free if we want to persist ticks for backtesting.

### TickSource trait

```rust
use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;

/// Identifies a venue in the sheaf graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VenueId(String);

/// Health status of a tick source.
#[derive(Debug, Clone)]
struct SourceHealth {
    venue: VenueId,
    state: SourceState,
    /// Number of ticks emitted in the last 60 seconds.
    ticks_per_min: f64,
    /// Age of the most recent tick (wall clock time).
    /// None if no ticks received yet.
    last_tick_age_ms: Option<u64>,
    /// Current reconnect attempt (0 = never disconnected).
    reconnect_attempt: u32,
    /// Whether the source has exhausted all reconnect attempts.
    reconnect_exhausted: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum SourceState {
    /// Connected and emitting ticks.
    Healthy,
    /// Connected but no ticks received within staleness threshold.
    Stale { stale_since: i64 },
    /// Disconnected, attempting reconnect.
    Reconnecting { since: i64, attempt: u32 },
    /// All reconnect attempts exhausted. Requires operator intervention.
    Dead { since: i64 },
}

/// Reconnect policy — standard exponential backoff.
/// Defaults: 10 attempts, 500ms initial, 30s max, 2× multiplier.
#[derive(Debug, Clone)]
struct ReconnectPolicy {
    max_attempts: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    multiplier: f64,
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

/// Which symbols to subscribe to.
#[derive(Debug, Clone)]
struct Subscription {
    symbol: String,
    /// Whether to track trades, order book depth, or both.
    /// For sheaf: trades only (price + size for adjacency edges).
    /// Order book is future work for spread-based signals.
    data_type: TickDataType,
}

#[derive(Debug, Clone)]
enum TickDataType {
    Trades,
    // OrderBookDelta,  // future
}

/// The unified tick source interface.
#[async_trait]
trait TickSource: Send + Sync {
    /// Human-readable identifier for logging and metrics.
    fn venue_id(&self) -> &VenueId;

    /// Open the tick stream.
    /// Returns a Stream of TickBatch instances. Each batch represents
    /// ticks collected over a micro-window (default 100ms).
    ///
    /// The stream is infinite — it only terminates on fatal error
    /// or when `shutdown()` is called.
    async fn stream(
        &mut self,
        subscriptions: Vec<Subscription>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TickBatch, TickSourceError>> + Send>>, TickSourceError>;

    /// Current health snapshot. Non-blocking.
    fn health(&self) -> SourceHealth;

    /// Graceful shutdown. Closes WebSocket connections, flushes buffers.
    async fn shutdown(&mut self) -> Result<(), TickSourceError>;
}

/// Errors that tick sources can emit.
#[derive(Debug, thiserror::Error)]
enum TickSourceError {
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
```

### Implementations

#### 1. ExchangeSource — direct WebSocket

```rust
struct ExchangeSource {
    venue_id: VenueId,
    ws_url: String,
    reconnect_policy: ReconnectPolicy,
    /// FLOX-style clock sync for this venue (decision F).
    clock: ExchangeClock,
    /// Ring buffer between WebSocket reader and micro-batcher.
    /// Lock-free SPSC, zero allocation in the hot path.
    ring_capacity: usize,  // default: 4096
}

impl ExchangeSource {
    /// Create for a known venue.
    fn new(venue: VenueId, ws_url: String) -> Self { ... }

    /// Internal: WebSocket read loop → normalize → push to ring buffer.
    async fn ws_reader_loop(&self, ws: WebSocket, ring_tx: Sender<Tick>) { ... }

    /// Internal: ring buffer → drain every N ms → build Arrow RecordBatch.
    async fn micro_batcher(&self, ring_rx: Receiver<Tick>, window_ms: u64) -> TickBatch { ... }
}

#[async_trait]
impl TickSource for ExchangeSource { ... }
```

#### 2. TestudoSource — wraps Testudo's pipe

```rust
struct TestudoSource {
    venue_id: VenueId,
    /// gRPC or WebSocket endpoint on Testudo.
    endpoint: String,
    /// JWT token for authenticated access.
    auth_token: String,
    clock: ExchangeClock,
}

impl TestudoSource {
    fn new(venue: VenueId, testudo_endpoint: String, auth_token: String) -> Self { ... }
}

#[async_trait]
impl TickSource for TestudoSource { ... }
```

**TestudoSource difference:** Testudo already validates ticks (order matching,
risk engine). TestudoSource reads from Testudo's existing WS/gRPC pipe rather
than opening a duplicate exchange WebSocket. The output is the same Arrow
`TickBatch` — the sheaf engine doesn't care about the source.

### Source orchestration

```rust
/// Manages multiple tick sources, resolving priority conflicts (decision B).
struct SourceOrchestrator {
    sources: Vec<Box<dyn TickSource>>,
    priority: TickSourcePriority,
    subscriptions: HashMap<VenueId, Vec<Subscription>>,
}

impl SourceOrchestrator {
    /// Start all sources and merge streams according to priority.
    async fn start(&mut self) -> Result<Pin<Box<dyn Stream<Item = Result<TickBatch>>>> {
        match self.priority {
            TickSourcePriority::Direct => {
                // Use ExchangeSource for everything. TestudoSource ignored.
                self.merge_sources(|s| s.is::<ExchangeSource>())
            }
            TickSourcePriority::PreferTestudo => {
                // Use TestudoSource where available, ExchangeSource as fallback.
                self.merge_with_fallback(|s| s.is::<TestudoSource>(), |s| s.is::<ExchangeSource>())
            }
            TickSourcePriority::Merge => {
                // Merge both, deduplicate by (venue, symbol, ts).
                // Direct ticks win on collision (lower latency).
                self.merge_and_dedup()
            }
        }
    }

    /// Collect health from all sources.
    fn all_health(&self) -> Vec<SourceHealth> { ... }
}
```

### Data flow diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     SourceOrchestrator                       │
│                                                             │
│  ┌──────────────────┐    ┌──────────────────┐              │
│  │  ExchangeSource  │    │  TestudoSource   │              │
│  │  (Binance WS)    │    │  (wraps HL pipe) │              │
│  │                  │    │                  │              │
│  │  ws_reader_loop  │    │  grpc_stream     │              │
│  │   → normalize    │    │   → map to Arrow │              │
│  │   → ring buffer  │    │   → TickBatch    │              │
│  │   → micro-batch  │    │                  │              │
│  │   → TickBatch    │    │                  │              │
│  └────────┬─────────┘    └────────┬─────────┘              │
│           │                       │                         │
│           └───────────┬───────────┘                         │
│                       │                                     │
│            MergeStream (priority-aware)                     │
│                       │                                     │
└───────────────────────┼─────────────────────────────────────┘
                        │
                        ▼ Stream<Item = TickBatch>
                        │
┌───────────────────────┼─────────────────────────────────────┐
│               Sheaf Engine (next design step)                │
│                                                             │
│  1. Clock sync (ExchangeClock::to_sheaf_time)              │
│  2. ASOF join alignment (Polars)                            │
│  3. Auto-discover graph topology                            │
│  4. Compute sheaf Laplacian                                 │
│  5. Extract topology signals                                │
│  6. Emit SignalBatch (gRPC stream + gRPC health)            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                        │
                        ▼
                  AI Agent (OpenClaw / Hermes / pi)
```

### Error handling in the stream

`TickSourceError` variants map to sheaf behavior:

| Error | Sheaf action |
|-------|-------------|
| `ConnectionFailed`, `Disconnected` | Venue node → `stale`. Reconnect in background. Sheaf continues with remaining venues. |
| `ReconnectExhausted` | Venue node → `down`. All edges involving venue → `broken`. Operator alert emitted. |
| `ParseError`, `InvalidTick` | Drop the tick. Increment `dropped_ticks` metric. No sheaf impact. |
| `StaleFeed` | Venue node → `stale`. `perception_confidence` drops proportionally. |
| `Backpressure` | Drop oldest batch in ring buffer. Increment `backpressure_events`. Alert if sustained. |

---

## Graph Configuration Protocol (2026-05-26)

### Design constraints

1. **Agent initiates, engine maintains.** The agent sends a single
   `ConfigureGraph` message. The engine auto-discovers edges from then on.
   No ongoing graph management RPCs.
2. **Incremental updates.** The agent can add/remove watch targets without
   restarting the engine. A `mode` field disambiguates full replacement
   (`FULL`) from incremental patches (`PATCH`).
3. **Edge hints, not edges.** The agent may suggest edges to track (e.g.,
   known triangular paths), but the engine owns edge discovery and decay.
4. **Multi-timeframe nodes.** The sheaf maps information flow from fast
   timeframes (1s) to slow (1h, 4h). Nodes exist at each timeframe.
   Configuration specifies which rungs of the ladder are active.
5. **Proto3 + tonic.** Wire format is protobuf, Rust codegen via `tonic`
   + `prost`. Bidirectional streaming: agent streams `ConfigureGraph`
   requests, engine streams `SignalBatch` responses.

### Proto service definition

```protobuf
// sheaf_engine.proto
syntax = "proto3";
package sheaf_engine;

// ── Service ──
service SheafEngine {
  // Bidirectional stream:
  //   Agent → Engine: ConfigureGraph (once, then optional patches)
  //   Engine → Agent: SignalBatch (continuous, per sheaf window)
  rpc Run(stream ConfigureGraph) returns (stream SignalBatch);

  // REST fallback — snapshot of current topology state.
  rpc Snapshot(SnapshotRequest) returns (SnapshotResponse);

  // Health check (standard gRPC health service also available).
  rpc Health(HealthRequest) returns (HealthResponse);
}

// ── Configuration ──
message ConfigureGraph {
  // Full replacement or incremental patch.
  ConfigMode mode = 1;

  // Venues and symbols to watch.
  repeated WatchTarget watch = 2;

  // Optional: edges the agent wants to force-track.
  // Engine will still auto-decay these if they go stale.
  repeated EdgeHint edge_hints = 3;

  // Which timeframes to build nodes at.
  repeated Timeframe timeframes = 4;

  // Optional parameter overrides. Omitted keys use defaults.
  map<string, string> config_overrides = 5;
}

enum ConfigMode {
  FULL = 0;   // Replace entire watch list and graph state.
  PATCH = 1;   // Add/remove targets incrementally.
}

message WatchTarget {
  string venue = 1;    // "BN", "HL", "BYBIT"
  string symbol = 2;   // "ETH_USDT", "BTC_USDT"
  WatchAction action = 3;  // only meaningful in PATCH mode
}

enum WatchAction {
  ADD = 0;
  REMOVE = 1;
}

message EdgeHint {
  // Pair of nodes the agent wants tracked.
  NodeRef a = 1;
  NodeRef b = 2;
  EdgeType edge_type = 3;
}

message NodeRef {
  string venue = 1;
  string symbol = 2;
  Timeframe timeframe = 3;  // which timeframe layer this node lives at
}

enum EdgeType {
  ARBITRAGE = 0;     // same symbol, different venues
  CORRELATION = 1;   // price co-movement over time
  TRIANGULAR = 2;    // related pairs at same venue
}

enum Timeframe {
  T1S = 0;    // 1 second
  T10S = 1;   // 10 seconds
  T1M = 2;    // 1 minute
  T5M = 3;    // 5 minutes
  T1H = 4;    // 1 hour
  T4H = 5;    // 4 hours
}

// ── Read-only endpoints ──
message SnapshotRequest {}

message SnapshotResponse {
  GraphState graph = 1;
  HealthState health = 2;
}

message HealthRequest {}

message HealthResponse {
  HealthState health = 1;
}

// (SignalBatch, GraphState, HealthState defined in step 5)
```

**Why bidirectional streaming and not separate RPCs:** The agent opens one
connection, sends config, and receives signals for the lifetime of the
session. No polling, no reconnect for config changes — the agent streams
a `PATCH` message down the same channel. This matches the design decision
"gRPC bidirectional stream as primary interface."

### Rust API (what tonic generates + our wrapper)

```rust
// Generated from sheaf_engine.proto
use sheaf_engine_proto::sheaf_engine_client::SheafEngineClient;
use sheaf_engine_proto::{ConfigureGraph, SignalBatch, ConfigMode, WatchTarget};

// Our ergonomic wrapper
struct SheafClient {
    inner: SheafEngineClient<Channel>,
}

impl SheafClient {
    async fn connect(addr: String) -> Result<Self> { ... }

    /// Start the sheaf engine with initial configuration.
    /// Returns a stream of SignalBatch that runs until the connection drops.
    async fn run(
        &mut self,
        watch: Vec<WatchTarget>,
        edge_hints: Vec<EdgeHint>,
        timeframes: Vec<Timeframe>,
    ) -> Result<impl Stream<Item = Result<SignalBatch, Status>>> {
        let config = ConfigureGraph {
            mode: ConfigMode::Full.into(),
            watch,
            edge_hints,
            timeframes,
            config_overrides: HashMap::new(),
        };

        // Open bidirectional stream, send config, return signal stream
        let (tx, rx) = self.inner.run().await?;
        tx.send(config).await?;
        Ok(rx)
    }

    /// Add or remove watch targets without restarting.
    async fn patch(&mut self, additions: Vec<WatchTarget>, removals: Vec<WatchTarget>) -> Result<()> { ... }

    /// REST fallback snapshot (uses unary gRPC, not REST).
    async fn snapshot(&mut self) -> Result<SnapshotResponse> { ... }

    async fn health(&mut self) -> Result<HealthResponse> { ... }
}
```

### Internal graph data model

```rust
/// The sheaf graph is a time-varying directed multigraph.
/// Nodes exist at multiple timeframes. Edges connect nodes.
struct SheafGraph {
    /// All nodes, keyed by (venue, symbol, timeframe).
    nodes: HashMap<NodeId, Node>,
    /// All edges, keyed by (source_node, target_node, edge_type).
    edges: HashMap<EdgeId, Edge>,
    /// Watch list — which (venue, symbol) pairs the agent asked for.
    watch_targets: HashSet<(VenueId, SymbolId)>,
    /// Which timeframes are active.
    active_timeframes: Vec<Timeframe>,
    /// Graph-level metadata.
    meta: GraphMeta,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct NodeId {
    venue: String,
    symbol: String,
    timeframe: Timeframe,
}

#[derive(Debug, Clone)]
struct Node {
    id: NodeId,
    status: NodeStatus,
    last_tick: Option<Tick>,
    /// Ring buffer of recent ticks for rolling aggregations.
    tick_window: TickWindow,
    /// When this node was first observed.
    created_at: i64,
}

#[derive(Debug, Clone)]
enum NodeStatus {
    /// Receiving ticks within staleness threshold.
    Active,
    /// No tick for > 15s. Weighted at 0.3×.
    Stale { since: i64 },
    /// No tick for > 60s. Removed from active topology.
    Down { since: i64 },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EdgeId {
    a: NodeId,
    b: NodeId,
    edge_type: EdgeType,
}

#[derive(Debug, Clone)]
struct Edge {
    id: EdgeId,
    status: EdgeStatus,
    /// Edge weight (interpretation depends on edge_type):
    ///   Arbitrage:   spread in basis points
    ///   Correlation: |Pearson ρ|, range [0, 1]
    ///   Triangular:  imbalance in basis points
    weight: f64,
    /// When the edge was first discovered.
    discovered_at: i64,
    /// Rolling window of weight history for decay detection.
    weight_history: RingBuffer<f64>,
    /// How this edge was created.
    origin: EdgeOrigin,
}

#[derive(Debug, Clone)]
enum EdgeStatus {
    /// Actively contributing to topology signals. Weight at 1.0×.
    Active,
    /// Staleness or decay threshold exceeded. Weight at 0.3×.
    /// A CORRELATION_DECAYING signal is emitted.
    Degraded { since: i64 },
    /// Fully removed from active topology. History kept for 24h.
    Removed { since: i64 },
}

#[derive(Debug, Clone)]
enum EdgeOrigin {
    /// Engine discovered from tick co-occurrence.
    Discovered,
    /// Agent provided as an edge hint.
    Hinted,
}

#[derive(Debug, Clone, Default)]
struct GraphMeta {
    /// Monotonic version counter. Incremented on every structural change.
    version: u64,
    /// Number of ConfigureGraph messages received.
    config_count: u32,
    /// Total ticks ingested since launch.
    total_ticks: u64,
    /// Total edges ever created (including removed).
    total_edges_created: u64,
}
```

### Auto-discovery rules

```rust
impl SheafGraph {
    /// Called after every aligned TickBatch.
    /// Discovers new edges and updates existing edge weights.
    fn discover(&mut self, batch: &TickBatch) -> Vec<EdgeId> {
        let mut new_edges = Vec::new();

        // 1. Ensure nodes exist for every (venue, symbol, timeframe) in the batch.
        self.ensure_nodes(batch);

        // 2. Arbitrage edges: same symbol, different venues.
        for timeframe in &self.active_timeframes {
            let by_symbol: HashMap<&str, Vec<&NodeId>> = self.active_nodes_by_symbol(timeframe);
            for (symbol, node_ids) in &by_symbol {
                if node_ids.len() < 2 { continue; }
                for i in 0..node_ids.len() {
                    for j in (i+1)..node_ids.len() {
                        let a = node_ids[i];
                        let b = node_ids[j];
                        if a.venue == b.venue { continue; }  // different venues only

                        let edge_id = EdgeId { a: a.clone(), b: b.clone(), edge_type: EdgeType::Arbitrage };
                        let spread_bps = self.compute_spread(a, b);
                        self.upsert_edge(edge_id, spread_bps, &mut new_edges);
                    }
                }
            }
        }

        // 3. Correlation edges: any two symbols with sufficient history.
        //    Evaluated at the 4h timeframe only (configurable).
        if self.active_timeframes.contains(&Timeframe::T4H) {
            let nodes: Vec<&NodeId> = self.active_nodes(Timeframe::T4H);
            for i in 0..nodes.len() {
                for j in (i+1)..nodes.len() {
                    let a = nodes[i];
                    let b = nodes[j];
                    if a.symbol == b.symbol { continue; }  // different symbols only

                    if let Some(rho) = self.compute_correlation(a, b, 30) {  // 30 candles
                        if rho.abs() > self.config.correlation_threshold {
                            let edge_id = EdgeId { a: a.clone(), b: b.clone(), edge_type: EdgeType::Correlation };
                            self.upsert_edge(edge_id, rho.abs(), &mut new_edges);
                        }
                    }
                }
            }
        }

        // 4. Triangular edges: three pairs forming a cycle at same venue.
        for venue in self.active_venues() {
            self.discover_triangles(venue, &mut new_edges);
        }

        new_edges
    }

    /// Upsert an edge: create if new, update weight if existing.
    /// New edges start as Active. Existing edges that were Degraded
    /// are re-promoted to Active if weight recovers.
    fn upsert_edge(&mut self, id: EdgeId, weight: f64, new_edges: &mut Vec<EdgeId>) {
        match self.edges.get_mut(&id) {
            Some(edge) => {
                edge.weight = weight;
                edge.weight_history.push(weight);
                if matches!(edge.status, EdgeStatus::Degraded { .. }) {
                    edge.status = EdgeStatus::Active;
                }
            }
            None => {
                let edge = Edge {
                    id: id.clone(),
                    status: EdgeStatus::Active,
                    weight,
                    discovered_at: now_ns(),
                    weight_history: RingBuffer::new(64),
                    origin: EdgeOrigin::Discovered,
                };
                edge.weight_history.push(weight);
                self.edges.insert(id, edge);
                new_edges.push(id);
            }
        }
    }

    /// Discover triangular cycles at a venue.
    /// A triangle exists when venue has: BTC_USDT, ETH_BTC, ETH_USDT.
    fn discover_triangles(&self, venue: &str, new_edges: &mut Vec<EdgeId>) {
        // Build symbol adjacency: which pairs exist at this venue?
        let pairs: HashSet<(&str, &str)> = self.nodes.keys()
            .filter(|n| n.venue == venue && n.timeframe == Timeframe::T1S)
            .map(|n| {
                let parts: Vec<&str> = n.symbol.split('_').collect();
                (parts[0], parts[1])
            })
            .collect();

        // For each base currency, find all quote currencies it pairs with
        // Then check if there's a third pair closing the triangle
        // Example: BTC→USDT, ETH→BTC, ETH→USDT forms a triangle
        for (base, intermediate) in &pairs {
            for (intermediate2, quote) in &pairs {
                if intermediate != intermediate2 { continue; }
                if pairs.contains(&(base, quote)) {
                    // Triangle found: base→intermediate→quote→base
                    // Create edges for each leg if they don't exist
                    // ... (edge creation logic)
                }
            }
        }
    }
```

### Timeframe ladder

```rust
/// How timeframes relate to each other in the sheaf.
///
/// Ticks arrive at the 1s layer. Aggregation flows upward:
///
///   T1S  ◄── raw ticks (every venue, every symbol)
///    │
///    ▼ aggregate (mean price, sum volume)
///   T10S
///    │
///    ▼ aggregate
///   T1M
///    │
///    ▼ aggregate
///   T5M  ─── used for volatility diffusion signals
///    │
///    ▼ aggregate
///   T1H  ─── used for regime persistence signals
///    │
///    ▼ aggregate
///   T4H  ─── used for correlation edges and regime classification
///
/// Information flow between timeframes is a sheaf stalk.
/// Volatility diffusion is measured as the propagation strength
/// from fast to slow layers.

struct TimeframeLadder {
    rungs: Vec<Timeframe>,
    /// For each rung, the aggregation window from the rung below.
    /// E.g., T10S aggregates 10 × T1S candles.
    aggregation_factor: HashMap<Timeframe, usize>,
}

impl Default for TimeframeLadder {
    fn default() -> Self {
        Self {
            rungs: vec![
                Timeframe::T1S,
                Timeframe::T10S,
                Timeframe::T1M,
                Timeframe::T5M,
                Timeframe::T1H,
                Timeframe::T4H,
            ],
            aggregation_factor: HashMap::from([
                (Timeframe::T10S, 10),   // 10 × 1s = 10s
                (Timeframe::T1M, 6),     // 6 × 10s = 1m
                (Timeframe::T5M, 5),     // 5 × 1m = 5m
                (Timeframe::T1H, 12),    // 12 × 5m = 1h
                (Timeframe::T4H, 4),     // 4 × 1h = 4h
            ]),
        }
    }
}

/// Between-timeframe edges (sheaf stalk edges).
/// Created automatically — the agent doesn't configure these.
struct TimeframeEdge {
    /// Node at the faster timeframe.
    parent: NodeId,
    /// Node at the slower timeframe (aggregated from parent's layer).
    child: NodeId,
    /// How strongly information propagates upward.
    /// 1.0 = perfect propagation (every tick at fast layer affects slow layer).
    /// 0.0 = no propagation (fast layer is noise).
    propagation_strength: f64,
}
```

**Why the ladder matters for signals:** A volatility spike at 1s that propagates
strongly to 1m is a genuine regime shift. A spike that stays at 1s and
dissipates at 10s is noise. The sheaf stalks quantify this propagation.
This is what the `VOLATILITY_DIFFUSION` signal type measures (Option C).

### Edge hint resolution

When the agent provides edge hints in `ConfigureGraph`:

```rust
fn resolve_edge_hints(&mut self, hints: &[EdgeHint]) {
    for hint in hints {
        let node_a = self.ensure_node(&hint.a.venue, &hint.a.symbol, hint.a.timeframe);
        let node_b = self.ensure_node(&hint.b.venue, &hint.b.symbol, hint.b.timeframe);

        let edge_id = EdgeId {
            a: node_a,
            b: node_b,
            edge_type: hint.edge_type,
        };

        // Create edge immediately, even without tick co-occurrence.
        // Mark as Hinted origin — still subject to auto-decay.
        if !self.edges.contains_key(&edge_id) {
            let edge = Edge {
                id: edge_id,
                status: EdgeStatus::Active,
                weight: 0.0,  // will be updated on first tick co-occurrence
                discovered_at: now_ns(),
                weight_history: RingBuffer::new(64),
                origin: EdgeOrigin::Hinted,
            };
            self.edges.insert(edge_id, edge);
        }
    }
}
```

**Hinted edges still auto-decay.** If the agent says "track BTC↔ETH correlation"
but the rolling ρ drops below threshold for 12 windows, the edge degrades and
removes. Hints are suggestions, not contracts.

### Configurable defaults (config_overrides)

```rust
struct SheafConfig {
    // ── Edge discovery ──
    /// Minimum absolute Pearson ρ to create a correlation edge.
    correlation_threshold: f64,  // default: 0.3
    /// Number of 4h candles required before correlation is computed.
    correlation_min_history: usize,  // default: 30 (5 days at 4h)
    /// Minimum spread in bps to emit an ARBITRAGE_EDGE signal.
    /// Edges are still tracked below this threshold; only signal emission is gated.
    arbitrage_signal_threshold_bps: f64,  // default: 5.0

    // ── Edge decay ──
    arbitrage_staleness_secs: u64,  // default: 30
    correlation_decay_threshold: f64,  // default: 0.3
    correlation_decay_windows: usize,  // default: 12
    triangular_staleness_secs: u64,  // default: 30

    // ── Node staleness ──
    node_stale_secs: u64,  // default: 15
    node_down_secs: u64,  // default: 60

    // ── Timeframes ──
    /// Default timeframe ladder. Omit higher rungs to reduce compute.
    /// Minimal viable set for sheaf: [T1S, T1M, T4H].
    active_timeframes: Vec<Timeframe>,  // default: all

    // ── Alignment (from decision F) ──
    alignment_tolerance_ms: u64,  // default: 500
    alignment_window_ms: u64,  // default: 100
}

impl Default for SheafConfig {
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
            active_timeframes: vec![T1S, T10S, T1M, T5M, T1H, T4H],
            alignment_tolerance_ms: 500,
            alignment_window_ms: 100,
        }
    }
}
```

### Configuration workflow

```
Agent                              Sheaf Engine
  │                                      │
  │  ── Run(ConfigureGraph{FULL}) ──▶   │  Initialize graph, subscribe to
  │                                      │  venues, start tick ingestion.
  │                                      │  Auto-discover edges as ticks flow.
  │                                      │
  │  ◀── SignalBatch ──────────────────  │  (continuous stream)
  │  ◀── SignalBatch ──────────────────  │
  │  ◀── SignalBatch ──────────────────  │
  │                                      │
  │  ── Run(ConfigureGraph{PATCH,       │  Add new symbol, remove old.
  │       watch: [+ETH_USDT, -BTC_USDT]})│  Graph updates incrementally.
  │                                      │
  │  ◀── SignalBatch ──────────────────  │  (stream continues uninterrupted)
  │  ◀── SignalBatch ──────────────────  │
  │                                      │
  │  ── close stream ──────────────────▶ │  Agent disconnects.
  │                                      │  Engine drains, shuts down.
```

### Minimal viable configuration

For the first working version, the agent only needs:

```protobuf
ConfigureGraph {
  mode: FULL
  watch: [
    { venue: "BN", symbol: "ETH_USDT" },
    { venue: "HL", symbol: "ETH_USDT" },
    { venue: "BN", symbol: "BTC_USDT" },
    { venue: "HL", symbol: "BTC_USDT" },
  ]
  // edge_hints: empty — engine discovers everything
  // timeframes: default — all rungs active  
  // config_overrides: empty — use all defaults
}
```

This is enough for the engine to discover:
- 2 arbitrage edges (ETH BN↔HL, BTC BN↔HL)
- 1 correlation edge (BTC↔ETH, after 30 candles of history)
- 0 triangular edges (no ETH_BTC pair configured)
- Full timeframe ladder (volatility diffusion signals)

With 4 watch targets, the graph has 4 nodes × 6 timeframes = 24 total nodes
and a handful of edges. The sheaf Laplacian is 24×24, solves in < 50μs.

---

## SignalBatch Output Schema (2026-05-26)

### Design principles

1. **One schema, two transports.** The same `SignalBatch` is emitted on the
   gRPC bidirectional stream (~10/sec, 100ms window) and available as a
   REST snapshot (`GET /snapshot`). Same struct, same fields.
2. **All signals, every batch.** The engine doesn't filter by signal type.
   If the arbitrage edge exists with a spread value, it's in the batch —
   regardless of whether the spread crossed a threshold. The `severity`
   field tells the agent whether to care.
3. **Health is always present.** `perception_confidence` is in every batch.
   The agent never needs a separate health check to know if signals are
   reliable.
4. **The `context` field is the LLM interface.** Every signal includes a
   one-line natural language summary. The harness injects this directly
   into the LLM's prompt. The LLM never sees raw topology data.

### Proto definition (continuing sheaf_engine.proto)

```protobuf
// ── Signal stream output ──

message SignalBatch {
  // When this batch was computed (sheaf time, not wall clock).
  int64 timestamp_ns = 1;

  // Incrementing sequence number for this engine session.
  uint64 seq = 2;

  // Current state of the topology graph.
  GraphState graph = 3;

  // All topology signals active at this moment.
  // Empty list means no signals (healthy, quiet market).
  repeated TopologySignal signals = 4;

  // Health and reliability metadata.
  HealthState health = 5;

  // Compute metrics.
  Metrics metrics = 6;
}

// ── Graph State ──

message GraphState {
  uint64 version = 1;    // increments on every structural change
  uint32 node_count = 2;
  uint32 edge_count = 3;

  // Edge counts by type.
  uint32 arbitrage_edges = 4;
  uint32 correlation_edges = 5;
  uint32 triangular_edges = 6;

  // Connectivity.
  uint32 connected_components = 7;
  uint32 isolated_nodes = 8;
  bool is_connected = 9;

  // Optional: full graph adjacency for debugging/visualization.
  // Only populated in REST snapshot, not streaming (too verbose).
  repeated NodeSnapshot nodes = 10;
  repeated EdgeSnapshot edges = 11;
}

message NodeSnapshot {
  string venue = 1;
  string symbol = 2;
  Timeframe timeframe = 3;
  NodeStatus status = 4;
  int64 last_tick_ns = 5;
  double price = 6;  // latest price at this timeframe
}

message EdgeSnapshot {
  NodeRef a = 1;
  NodeRef b = 2;
  EdgeType edge_type = 3;
  EdgeStatus status = 4;
  double weight = 5;
}

// ── Topology Signals (Option C: enriched structural observations) ──

message TopologySignal {
  // Signal type discriminator.
  SignalType type = 1;

  // Severity: how urgently the agent should act.
  Severity severity = 2;

  // Natural language context injected into LLM prompt.
  // Example: "ETH spot spread 23 bps BN→HL. 30s avg is 4 bps. 5.75σ anomaly."
  string context = 3;

  // When this signal first appeared.
  int64 first_seen_ns = 4;

  // How many consecutive windows this signal has been active.
  uint32 duration_windows = 5;

  // Type-specific data (oneof).
  oneof signal_data {
    ArbitrageSignal arbitrage = 10;
    CorrelationSignal correlation = 11;
    VolatilityDiffusionSignal volatility_diffusion = 12;
    TriangularMispricingSignal triangular_mispricing = 13;
    RegimeTransitionSignal regime_transition = 14;
    EdgeLifecycleSignal edge_lifecycle = 15;
    NodeHealthSignal node_health = 16;
  }
}

enum SignalType {
  ARBITRAGE_EDGE = 0;
  CORRELATION_BREAK = 1;
  VOLATILITY_DIFFUSION = 2;
  TRIANGULAR_MISPRICING = 3;
  REGIME_TRANSITION = 4;
  EDGE_APPEARED = 5;
  EDGE_REMOVED = 6;
  NODE_STALE = 7;
  NODE_DOWN = 8;
}

enum Severity {
  INFO = 0;       // background reading, no action needed
  NOTABLE = 1;    // factor into decision, but don't force action
  CRITICAL = 2;   // immediate attention required
}

// ── Signal data types ──

message ArbitrageSignal {
  string venue_a = 1;
  string venue_b = 2;
  string symbol = 3;
  // Spread in basis points.
  // Positive = venue_a cheaper than venue_b (buy A, sell B).
  double spread_bps = 4;
  double spread_usd = 5;
  // Baseline spread over the lookback window.
  double baseline_spread_bps = 6;
  // How many standard deviations above baseline.
  double sigma = 7;
}

message CorrelationSignal {
  string symbol_a = 1;
  string symbol_b = 2;
  // Current rolling Pearson ρ.
  double current_rho = 4;
  // Baseline ρ over the comparison window.
  double baseline_rho = 5;
  // When the decoupling was first detected.
  int64 decoupling_since_ns = 6;
  // Direction: 'breaking' (ρ dropping) or 'strengthening' (ρ rising).
  string direction = 7;
}

message VolatilityDiffusionSignal {
  // Direction of vol propagation: 'upward' (fast→slow) or 'downward' (slow→fast).
  string direction = 1;
  Timeframe source_timeframe = 2;
  Timeframe target_timeframe = 3;
  // Propagation strength [0, 1].
  // > 0.7 = genuine regime shift. < 0.3 = noise.
  double strength = 4;
  // Which symbol(s) are experiencing the diffusion.
  repeated string symbols = 5;
}

message TriangularMispricingSignal {
  string venue = 1;
  // The three pairs forming the triangle.
  repeated string path = 2;  // e.g. ["BTC_USDT", "ETH_BTC", "ETH_USDT"]
  // Imbalance in basis points.
  double imbalance_bps = 3;
  // Execution friction in bps (fees + slippage estimate).
  // If imbalance > friction, arbitrage is executable.
  double execution_friction_bps = 4;
}

message RegimeTransitionSignal {
  // Previously detected regime.
  string from_regime = 1;
  // Newly detected regime.
  string to_regime = 2;
  // W₁ distance change.
  double wasserstein_delta = 3;
  // Which symbol this applies to, or 'market' for aggregate.
  string symbol = 4;
}

message EdgeLifecycleSignal {
  EdgeType edge_type = 1;
  NodeRef a = 2;
  NodeRef b = 3;
  // 'appeared' or 'removed'
  string event = 4;
  string reason = 5;  // e.g. "stale_30s", "correlation_decayed", "agent_hinted"
}

message NodeHealthSignal {
  string venue = 1;
  string symbol = 2;
  // 'stale' or 'down'
  string event = 3;
  int64 since_ns = 4;
  string suspected_cause = 5;  // "no_tick_data", "clock_drift", "venue_maintenance"
}

// ── Health State ──

message HealthState {
  // Overall perception confidence [0, 1].
  // Gates agent decision-making (see decision D).
  double perception_confidence = 1;

  // Node-level health.
  uint32 active_nodes = 2;
  uint32 stale_nodes = 3;
  uint32 down_nodes = 4;

  // Edge-level health.
  uint32 active_edges = 5;
  uint32 degraded_edges = 6;
  uint32 broken_edges = 7;

  // Cross-exchange clock skew in milliseconds (see decision F).
  int64 cross_exchange_skew_ms = 8;

  // Per-venue health for granular diagnostics.
  repeated VenueHealth venues = 9;

  // Empty when healthy.
  repeated string active_alerts = 10;
}

message VenueHealth {
  string venue = 1;
  string status = 2;  // "healthy", "stale", "reconnecting", "dead"
  int32 ticks_per_min = 3;
  int64 last_tick_age_ms = 4;
  int32 reconnect_attempt = 5;
  int64 estimated_clock_offset_us = 6;  // from ExchangeClock sync
}

// ── Metrics ──

message Metrics {
  // Compute timings (nanoseconds).
  int64 alignment_ns = 1;
  int64 graph_discovery_ns = 2;
  int64 laplacian_ns = 3;
  int64 signal_extraction_ns = 4;
  int64 total_window_ns = 5;

  // Data rates.
  int64 ticks_per_second = 6;
  int64 batches_per_second = 7;

  // Graph stats.
  int64 total_ticks_ingested = 8;
  int64 total_edges_created = 9;
  uint32 config_applies = 10;
}
```

### Rust types (conceptual — derived from proto)

These are what tonic/prost generates. Shown here for clarity when
reasoning about the agent integration.

```rust
/// The core output. Received 10×/sec on gRPC stream,
/// also returned by Snapshot RPC.
struct SignalBatch {
    timestamp_ns: i64,
    seq: u64,
    graph: GraphState,
    signals: Vec<TopologySignal>,
    health: HealthState,
    metrics: Metrics,
}

struct TopologySignal {
    signal_type: SignalType,
    severity: Severity,
    context: String,          // ← LLM injects this directly
    first_seen_ns: i64,
    duration_windows: u32,
    signal_data: SignalData,  // oneof
}

enum SignalData {
    Arbitrage(ArbitrageSignal),
    Correlation(CorrelationSignal),
    VolatilityDiffusion(VolatilityDiffusionSignal),
    TriangularMispricing(TriangularMispricingSignal),
    RegimeTransition(RegimeTransitionSignal),
    EdgeLifecycle(EdgeLifecycleSignal),
    NodeHealth(NodeHealthSignal),
}
```

### Signal emission rules

Not every signal type fires on every window. The engine applies gates:

| Signal type | Emitted when | Suppressed when |
|-------------|-------------|-----------------|
| `ARBITRAGE_EDGE` | Spread > 5 bps above baseline | Spread ≤ 5 bps (tracked internally, not emitted) |
| `CORRELATION_BREAK` | \|ρ\| dropped > 0.15 from baseline, or crossed threshold | ρ stable (±0.05 from last emission) |
| `VOLATILITY_DIFFUSION` | Propagation strength > 0.5 | First time with < 3 candles of history |
| `TRIANGULAR_MISPRICING` | Imbalance > execution_friction + buffer (2 bps) | Imbalance ≤ friction |
| `REGIME_TRANSITION` | W₁ distance to nearest centroid changed by > 0.001 | Stable regime (W₁ within 0.001 of last classification) |
| `EDGE_APPEARED` | First occurrence of any edge type | N/A — always emitted on discovery |
| `EDGE_REMOVED` | Edge status transitions from Degraded → Removed | N/A — always emitted on removal |
| `NODE_STALE` | Node has no tick for > 15s | Already stale (don't repeat) |
| `NODE_DOWN` | Node stale for > 60s | Already down (don't repeat) |

### Complete example: SignalBatch at 14:30:00 UTC

```json
{
  "timestamp_ns": 1716413400000000000,
  "seq": 8472,
  "graph": {
    "version": 14,
    "node_count": 18,
    "edge_count": 9,
    "arbitrage_edges": 4,
    "correlation_edges": 3,
    "triangular_edges": 2,
    "connected_components": 1,
    "isolated_nodes": 0,
    "is_connected": true
  },
  "signals": [
    {
      "type": "ARBITRAGE_EDGE",
      "severity": "NOTABLE",
      "context": "ETH spot spread 23 bps BN→HL. 30s avg is 4 bps. 5.75σ anomaly.",
      "first_seen_ns": 1716413394000000000,
      "duration_windows": 6,
      "arbitrage": {
        "venue_a": "BN",
        "venue_b": "HL",
        "symbol": "ETH_USDT",
        "spread_bps": 23.0,
        "spread_usd": 2.15,
        "baseline_spread_bps": 4.0,
        "sigma": 5.75
      }
    },
    {
      "type": "CORRELATION_BREAK",
      "severity": "NOTABLE",
      "context": "BTC-ETH correlation dropped from 0.82 to 0.47 over 4h. Possible regime shift.",
      "first_seen_ns": 1716413200000000000,
      "duration_windows": 12,
      "correlation": {
        "symbol_a": "BTC_USDT",
        "symbol_b": "ETH_USDT",
        "current_rho": 0.47,
        "baseline_rho": 0.82,
        "decoupling_since_ns": 1716411600000000000,
        "direction": "breaking"
      }
    },
    {
      "type": "VOLATILITY_DIFFUSION",
      "severity": "NOTABLE",
      "context": "1m vol spike diffusing to 5m sheaf stalk. 73% propagation strength suggests genuine vol regime shift, not noise.",
      "first_seen_ns": 1716413360000000000,
      "duration_windows": 10,
      "volatility_diffusion": {
        "direction": "upward",
        "source_timeframe": "T1M",
        "target_timeframe": "T5M",
        "strength": 0.73,
        "symbols": ["ETH_USDT", "BTC_USDT"]
      }
    },
    {
      "type": "TRIANGULAR_MISPRICING",
      "severity": "INFO",
      "context": "Triangular path BTC→ETH→BTC at Binance shows 8 bps imbalance. Below execution friction (~12 bps) — no trade.",
      "first_seen_ns": 1716413390000000000,
      "duration_windows": 3,
      "triangular_mispricing": {
        "venue": "BN",
        "path": ["BTC_USDT", "ETH_BTC", "ETH_USDT"],
        "imbalance_bps": 8.0,
        "execution_friction_bps": 12.0
      }
    }
  ],
  "health": {
    "perception_confidence": 0.94,
    "active_nodes": 18,
    "stale_nodes": 0,
    "down_nodes": 0,
    "active_edges": 8,
    "degraded_edges": 1,
    "broken_edges": 0,
    "cross_exchange_skew_ms": 47,
    "venues": [
      {"venue": "BN", "status": "healthy", "ticks_per_min": 2340, "last_tick_age_ms": 42, "reconnect_attempt": 0, "estimated_clock_offset_us": -120},
      {"venue": "HL", "status": "healthy", "ticks_per_min": 1780, "last_tick_age_ms": 89, "reconnect_attempt": 0, "estimated_clock_offset_us": 310}
    ],
    "active_alerts": []
  },
  "metrics": {
    "alignment_ns": 180000,
    "graph_discovery_ns": 45000,
    "laplacian_ns": 52000,
    "signal_extraction_ns": 31000,
    "total_window_ns": 320000,
    "ticks_per_second": 680,
    "batches_per_second": 10,
    "total_ticks_ingested": 5760480,
    "total_edges_created": 14,
    "config_applies": 1
  }
}
```

### How the agent consumes SignalBatch

```python
# Pseudocode — agent integration layer (Python, Node, or Rust).
# The harness translates SignalBatch into LLM context.

async def consume_signal_batch(batch: SignalBatch):
    # 1. Gate on health
    if batch.health.perception_confidence < 0.60:
        return HaltExecution("sheaf perception degraded")
    if batch.health.cross_exchange_skew_ms > 200:
        log_warning(f"High clock skew: {batch.health.cross_exchange_skew_ms}ms")

    # 2. Extract LLM context from signals
    critical_signals = [s for s in batch.signals if s.severity == Severity.CRITICAL]
    notable_signals = [s for s in batch.signals if s.severity == Severity.NOTABLE]
    info_signals = [s for s in batch.signals if s.severity == Severity.INFO]

    # 3. Build prompt context
    #    The context field is already natural language — inject directly.
    prompt_context = []

    if critical_signals:
        prompt_context.append("## 🚨 Critical Sheaf Signals")
        for s in critical_signals:
            prompt_context.append(f"- {s.context} [active for {s.duration_windows} windows]")

    if notable_signals:
        prompt_context.append("## ⚡ Notable Sheaf Signals")
        for s in notable_signals:
            prompt_context.append(f"- {s.context}")

    if info_signals:
        prompt_context.append("## 📊 Background")
        for s in info_signals:
            prompt_context.append(f"- {s.context}")

    prompt_context.append(f"\nSheaf perception confidence: {batch.health.perception_confidence:.0%}")
    prompt_context.append(f"Graph: {batch.graph.node_count} nodes, {batch.graph.edge_count} edges, connected: {batch.graph.is_connected}")

    # 4. Inject into LLM context alongside journal memory and OHLCV
    llm_context = f"""
    ## Market Structure (Sheaf Engine)
    {chr(10).join(prompt_context)}

    ## Performance Memory
    {journal_summary}

    ## Regime Detection
    Current regime: {regime}

    Based on the above, decide: trade or wait.
    """

    decision = llm.decide(llm_context)

    if decision.trade:
        # Execute via POST /api/v1/signals (existing Testudo API)
        result = testudo.place_signal(decision.to_signal_input())
        # Tag and journal as usual (AGENT_TRADING.md §5-6)
```

### Signal deduplication

The agent may receive the same signal across multiple consecutive windows.
The `first_seen_ns` and `duration_windows` fields let the agent decide:

- **New signal** (`duration_windows == 1`): Unconditionally inject into LLM context.
- **Recurring but stable** (`1 < duration_windows < 10`): Include if severity ≥ NOTABLE.
- **Chronic** (`duration_windows > 10`): Downgrade to INFO. It's now background
  state, not a new event. The agent has already factored it in.

```python
def should_include(signal: TopologySignal) -> bool:
    if signal.duration_windows == 1:
        return True  # always show new signals
    if signal.duration_windows < 10:
        return signal.severity != Severity.INFO  # drop chronic info-level
    # Chronic: only critical signals still worth flagging
    return signal.severity == Severity.CRITICAL
```

This prevents the LLM context from being flooded with "ETH spread still at
23 bps" on every single window for hours. The agent already knows.

### Transport difference: streaming vs REST snapshot

| Field | gRPC stream (10/sec) | REST snapshot |
|-------|---------------------|---------------|
| `graph.nodes`, `graph.edges` | **Omitted** (too verbose at 10/sec) | **Full** (for debugging/visualization) |
| `signals` | Full, with dedup logic | Full, all active signals |
| `health` | Full | Full |
| `metrics` | Full | Full |

In proto3, this is handled by the server simply not populating `nodes`/`edges`
on the streaming path. The `snapshot` RPC handler populates them.

---

## Next Steps

1. ~~Resolve output abstraction level (A vs B above)~~ ✅ Option C chosen
2. ~~Resolve tick alignment across venues (Open Question F above)~~ ✅ Two-timestamp model + Polars ASOF + FLOX clock sync
3. ~~Define the `TickSource` trait and `TickBatch` format~~ ✅ Arrow RecordBatch, stream-based trait, two implementations
4. ~~Define the graph configuration protocol (initial + auto-discovery)~~ ✅ Proto3 + tonic, auto-discovery + edge hints, timeframe ladder
5. ~~Define the `SignalBatch` output schema (incorporating chosen Option C format)~~ ✅ Proto3, 9 signal types, severity-gated, LLM-ready context fields
6. ~~Scaffold the crate structure~~ ✅ `sheaf-engine/` — 2,373 lines Rust, compiles with `cargo check`
7. Spec as `.specify/specs/SHEAF-01-topology-engine/spec.md`
