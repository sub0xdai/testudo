//! Sheaf Engine — cellular sheaf topology perception layer.
//!
//! Ingests multi-venue tick data, computes topological signals (arbitrage,
//! volatility diffusion, regime consistency), and exposes them to AI agent
//! harnesses via gRPC bidirectional streaming.
//!
//! ## Architecture
//!
//! ```text
//! TickSource(s) → alignment (Polars ASOF) → SheafGraph auto-discovery
//! → Laplacian → signal extraction → gRPC SignalBatch stream
//! ```
//!
//! ## Crate modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | `tick` | `Tick`, `TickBatch` — Arrow-native columnar types |
//! | `source` | `TickSource` trait + `ExchangeSource`, `TestudoSource` impls |
//! | `clock` | `ExchangeClock` — FLOX-style RTT-based clock synchronization |
//! | `align` | Polars ASOF join wrapper for cross-venue tick alignment |
//! | `graph` | `SheafGraph` — nodes, edges, auto-discovery, decay |
//! | `laplacian` | Sheaf Laplacian computation on the topology graph |
//! | `signals` | Signal extraction — topology observations → `TopologySignal` |
//! | `health` | `HealthState`, `perception_confidence` scoring |
//! | `config` | CLI args, config file, environment |
//! | `error` | Error types |
//! | `service` | tonic gRPC service implementation |
//! | `proto` | Generated protobuf types (from `sheaf_engine.proto`) |
//!
//! ## Quick start
//!
//! ```bash
//! cargo run -- \
//!   --grpc-listen 0.0.0.0:50051 \
//!   --venue BN,wss://stream.binance.com:9443/ws \
//!   --venue HL,wss://api.hyperliquid.xyz/ws \
//!   --symbol ETH_USDT \
//!   --symbol BTC_USDT
//! ```

pub mod tick;
pub mod source;
pub mod clock;
pub mod align;
pub mod graph;
pub mod laplacian;
pub mod signals;
pub mod health;
pub mod config;
pub mod error;
pub mod service;

/// Generated protobuf types and gRPC service definitions.
pub mod proto {
    tonic::include_proto!("sheaf_engine");
}
