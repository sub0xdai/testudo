# Sheaf Engine

Cellular sheaf topology engine — perception layer between market data and AI trading agents.

## Overview

The sheaf engine ingests multi-venue tick data, computes topological signals (arbitrage, volatility diffusion, regime consistency), and exposes them to any AI agent harness (OpenClaw, Hermes, pi, custom Rust) via gRPC bidirectional streaming.

**Not** a replacement for Testudo's matching engine. A new layer between market data and the AI agent.

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Sheaf Engine │────▶│  AI Agent    │────▶│   Testudo    │
│ (perception) │     │ (orchestr.)  │     │ (execution)  │
└──────────────┘     └──────────────┘     └──────────────┘
```

## Quick Start

```bash
# Build
cargo build --release

# Run with Binance and Hyperliquid, watching ETH and BTC
cargo run --release -- \
  --venue BN,wss://stream.binance.com:9443/ws \
  --venue HL,wss://api.hyperliquid.xyz/ws \
  --symbol ETH_USDT \
  --symbol BTC_USDT
```

## gRPC API

The agent connects via gRPC bidirectional streaming:

```protobuf
service SheafEngine {
  rpc Run(stream ConfigureGraph) returns (stream SignalBatch);
  rpc Snapshot(SnapshotRequest) returns (SnapshotResponse);
  rpc Health(HealthRequest) returns (HealthResponse);
}
```

See `proto/sheaf_engine.proto` for the full schema.

## Module Map

| Module | Purpose |
|--------|---------|
| `tick` | `Tick`, `TickBatch` — Arrow-native columnar types |
| `source` | `TickSource` trait + `ExchangeSource`, `TestudoSource` impls |
| `clock` | `ExchangeClock` — FLOX-style RTT-based clock synchronization |
| `align` | Polars ASOF join wrapper for cross-venue tick alignment |
| `graph` | `SheafGraph` — nodes, edges, auto-discovery, decay |
| `laplacian` | Sheaf Laplacian computation on the topology graph |
| `signals` | Signal extraction — topology observations → `TopologySignal` |
| `health` | `HealthState`, `perception_confidence` scoring |
| `config` | CLI args, config file, environment |
| `error` | Error types |
| `service` | tonic gRPC service implementation |

## Design Document

See `../sheaf-engine-design.md` for the full design with all decisions.

## License

AGPL-3.0
