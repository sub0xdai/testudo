# Testudo Hybrid Trading System - AI Engineer Handoff

## Quick Start

```bash
# Backend
cd /home/m0xu/1-projects/testudo/testudo-exchange
cargo run --bin router

# Frontend (separate terminal)
cd /home/m0xu/1-projects/testudo/testudo-web/apps/web
bun run dev
```

**Navigate to**: http://localhost:5173/trade/SOLUSDT

---

## Project Overview

**Testudo** is a perpetual futures trading system that:
- Displays **live market data from Binance Futures API** (539 USDT perp pairs)
- Executes orders through a **Shadow + Live** execution model
- Provides **automated risk management** with "Conservative Wins" policy
- Supports **paper trading** (Shadow mode) before live execution

### Architecture
```
testudo/
├── testudo-exchange/    # Rust backend (see CHANGELOG.md for details)
│   └── crates/
│       ├── common_utils/  # Services, adapters, risk engine
│       ├── engine/        # Order matching + Shadow engine
│       ├── router/        # API gateway
│       └── ...
├── testudo-web/         # React/TypeScript frontend
└── testudo-ops/         # Kubernetes infrastructure
```

---

## Current State (2026-01-24)

**Tests**: 580+ passing across all crates

### Completed Phases

| Phase | Description | Status |
|-------|-------------|--------|
| A | Market Data Pipeline (Binance → API) | Completed |
| B | Shadow Engine (Paper Trading) | Completed |
| C | Risk Engine (Position Sizing) | Completed |
| D | Trade Management (SL/TP, Break-even) | Completed |
| E.1-E.5 | Live Execution Flow | Completed |
| F | Binance Futures Migration | Completed |
| RISK | User Risk Config + Position Calculator | Completed |
| DRAW | Drawable Position Tool | Completed |
| COLUMNAR | Wire-Efficient SoA Data Format | Completed |
| V5 | Native Canvas Position Tool (Hybrid) | Completed |
| GEOM | Time-Anchored Bounded Zones | Completed |
| PAPER | Paper Trading Integration | Completed |
| BALANCE | Paper Balance Reset Feature | Completed |

### Recent Specifications (2026-01-20/21/22/24)

| Spec | Description | Commit |
|------|-------------|--------|
| 001-deprecate-legacy | Deprecated `Engine::create_order()` for Decision Loop | `7b6a1d0` |
| 002-panic-prevention | Hardened production code, reduced unwraps 544→505 | `a363cc4` |
| 003-risk-enforcement | All shadow orders must pass Decision Loop validation | `d99a457` |
| 004-read-compute-write | Lock optimization with Read-Compute-Write pattern | `c102c9d` |
| 005-atomic-cascades | Atomic transaction context for Entry + SL + TP creation | `25192ef` |
| 006-performance-overhaul | Latency reduction, DashMap, range matching | `6f46736` |
| 007-open-positions-layer | Persistent position lines after trade creation | `36a18a9` |
| 007-editable-position-levels | Draggable handles to edit Entry/SL/TP | `b7814bc` |
| **008-unified-exchange-adapter** | **DRY refactoring: lock macro, get_adapter()** | **Latest** |

### Latest: 008-unified-exchange-adapter ✅ COMPLETE

**Problem:** ~100 lines of duplicated code in router crate:
- 11 identical lock poisoning patterns
- 3 methods with identical adapter dispatch logic
- Unused `LiquidityBased` routing strategy

**Solution:** DRY refactoring:

1. **`lock_or_recover!` macro** - Replaced 11 lock patterns with a single macro
2. **`get_adapter()` method** - Centralized adapter dispatch logic
3. **Removed YAGNI** - Deleted unused `LiquidityBased` variant

**Metrics:**
| Before | After |
|--------|-------|
| ExecutionService ~82 LOC | ~12 LOC |
| Lock boilerplate ~80 LOC | ~11 LOC |
| 134 tests passing | 134 tests passing |

**Files:**
- `crates/router/src/exchange/mod.rs`
- `crates/router/src/services/execution_service.rs`

**Full changelog**: `testudo-exchange/CHANGELOG.md`

---

## API Endpoints

### Market Data
```
GET /api/v1/market-data/ticker?symbol=SOLUSDT
GET /api/v1/market-data/orderbook?symbol=SOLUSDT&limit=20
GET /api/v1/market-data/klines?symbol=SOLUSDT&interval=1h&limit=100
GET /api/v1/market-data/markets   # Returns 539 USDT perps
GET /api/v2/market-data/orderbook # Columnar format (~25% smaller)
```

### Trading
```
POST   /api/v1/trades              # Create trade with SL/TP
GET    /api/v1/trades              # List active trades
PUT    /api/v1/trades/{id}/entry   # Update entry price (pending only)
PUT    /api/v1/trades/{id}/sl      # Update stop loss
PUT    /api/v1/trades/{id}/tp      # Update take profit
DELETE /api/v1/trades/{id}         # Cancel trade group
```

### Paper Trading
```
GET  /api/v1/paper/balances        # Paper balance (lazy init: 10,000 USDT)
POST /api/v1/paper/reset           # Reset balance
```

### Risk Configuration
```
GET /api/v1/risk-config
PUT /api/v1/risk-config
```

---

## Execution Modes

| Mode | Description |
|------|-------------|
| **Shadow** | Paper trading, no real orders. Default mode. |
| **Live** | Real orders sent to Binance Futures. Requires API keys. |

---

## Development Commands

### Backend
```bash
cd testudo-exchange
cargo build --bin router    # Build
cargo run --bin router      # Run API server (port 8080)
cargo test                  # Run tests (580+)
cargo clippy                # Lint
```

### Frontend
```bash
cd testudo-web/apps/web
bun install                 # Install deps
bun run dev                 # Dev server (port 5173)
bun run build               # Production build
```

---

## Key Files

### Backend
```
crates/engine/src/shadow/
├── mod.rs              # ShadowEngine (Read-Compute-Write pattern)
├── orders.rs           # Order management + risk validation
├── balances.rs         # Paper trading balances
├── positions.rs        # Position tracking
└── transaction.rs      # TransactionContext for atomic cascades

crates/router/src/routes/
├── trade_management.rs # /api/v1/trades/* (Decision Loop)
├── paper_balance.rs    # /api/v1/paper/*
└── market_data.rs      # /api/v1 + /api/v2 market-data
```

### Frontend
```
apps/web/src/
├── pages/Trade.tsx                    # Main trading page
├── components/chart/
│   ├── PositionDrawingTool.tsx        # Position tool (hybrid)
│   ├── PositionHandleOverlay.tsx      # Drag handles
│   └── OpenPositionsLayer.tsx         # Persistent position rendering
├── hooks/
│   └── useOpenPositions.ts            # Fetch & manage open trades
├── primitives/PositionZonePrimitive.ts # V5 canvas primitive
└── utils/chart_manager.ts             # Lightweight-charts wrapper (multi-primitive)
```

---

## How to Use Position Tool

1. Click position tool button (crosshair icon) in chart toolbar
2. Click and **hold** on chart at desired entry price
3. **Drag** up or down to set stop loss
4. **Release** mouse - TP auto-calculates based on R:R ratio
5. Adjust levels by dragging handles (entry, SL, TP)
6. Drag right edge to set trade timeout
7. Click Execute button (▶) or press Enter to place order
8. Press Escape or click ✕ to cancel

---

## Specifications

Specs are located in `.specify/specs/`:
```
001-deprecate-legacy/           # Completed
002-panic-prevention/           # Completed
003-risk-enforcement/           # Completed
004-read-compute-write/         # Completed
005-atomic-cascades/            # Completed
006-performance-overhaul/       # Completed
007-open-positions-layer/       # Completed (ad-hoc, not spec'd)
008-unified-exchange-adapter/   # Completed
```

---

## References

- **Changelog**: `testudo-exchange/CHANGELOG.md`
- **PRD**: `.specify/prd.json`
- **Diagrams**: `testudo-web/apps/web/docs/diagrams/`
