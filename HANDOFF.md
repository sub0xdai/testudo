# Testudo Hybrid Trading System - AI Engineer Handoff

## Quick Start

```bash
cd /home/m0xu/1-projects/testudo/testudo-exchange
cargo test
```

**Expected**: 341 tests passing, 0 failed

---

## Project Overview

**Testudo** is a hybrid trading system that:
- Displays **live market data from Binance**
- Executes orders through a **Shadow + External** model
- Provides **automated risk management** with "Conservative Wins" policy
- Supports **paper trading** for practice before live execution

### Architecture
```
testudo/
├── testudo-exchange/    # Rust backend (THIS IS WHERE YOU WORK)
│   └── crates/
│       ├── common_utils/  # Services, adapters, risk engine
│       ├── engine/        # Order matching + Shadow engine
│       ├── router/        # API gateway
│       └── ...
├── testudo-web/         # React/TypeScript frontend
└── testudo-ops/         # Kubernetes infrastructure
```

---

## Current State (2026-01-10)

### Completed Phases

| Phase | Description | Status |
|-------|-------------|--------|
| **A** | Market Data Pipeline (Binance → API) | ✅ Complete |
| **B** | Shadow Engine (Paper Trading) | ✅ Complete |
| **C** | Risk Engine (Position Sizing) | ✅ Complete |
| **D** | Trade Management (SL/TP, Break-even) | 🔜 **NEXT** |
| **E** | Live Execution (Binance Orders) | 📋 Planned |

### Key Files Created

```
crates/common_utils/src/
├── services/
│   ├── binance_data.rs    # Live Binance data fetching
│   └── cache.rs           # Redis caching
├── risk/
│   ├── config.rs          # RiskConfig (account %, max risk, etc.)
│   ├── position_sizer.rs  # "Conservative Wins" sizing
│   └── validator.rs       # Pre-trade validation

crates/engine/src/
└── shadow/
    ├── mod.rs             # ShadowEngine orchestrator
    ├── balances.rs        # Virtual balance management
    ├── orders.rs          # Order simulation + fill logic
    └── positions.rs       # Position tracking + P&L

crates/router/src/routes/
└── market_data.rs         # /api/v1/market-data/* endpoints
```

---

## Next Task: Phase D - Trade Management

**Goal**: Implement SL/TP linking, break-even automation, and multi-target exits

### Tasks

| # | Task | File | Description |
|---|------|------|-------------|
| 19 | Order Groups Model | `common_utils/src/models/order_group.rs` | Link entry with SL/TP orders |
| 20 | SL/TP Linking | `engine/src/shadow/sl_tp.rs` | Create SL/TP when entry fills |
| 21 | Break-even Automation | `engine/src/shadow/breakeven.rs` | Move SL to entry at X% profit |
| 22 | Multi-target Exit | `engine/src/shadow/multi_target.rs` | Exit 50% at T1, 25% at T2, etc. |
| 23 | Trade Management API | `router/src/routes/trade_management.rs` | CRUD for trade groups |

### TDD Starting Point

```rust
// RED: Write this test first in engine/src/shadow/sl_tp.rs
#[tokio::test]
async fn test_sl_tp_created_on_entry_fill() {
    let engine = ShadowEngine::new();
    let user_id = Uuid::new_v4();
    engine.init_user(user_id).await;

    // Place entry with SL/TP
    let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000))
        .with_stop_loss(dec!(49000))
        .with_take_profit(dec!(52000));

    let placed = engine.place_order(user_id, order).await.unwrap();

    // Simulate price hitting entry
    engine.process_price_update("BTC_USDC", dec!(49900), dec!(50000), dec!(50100), dec!(49900)).await;

    // Entry should be filled
    let orders = engine.get_open_orders(user_id).await;

    // SL and TP orders should now exist
    assert!(orders.iter().any(|o| o.order_type == ShadowOrderType::StopLoss));
    assert!(orders.iter().any(|o| o.order_type == ShadowOrderType::TakeProfit));
}
```

---

## API Endpoints

### Existing (Phase A)
```
GET /api/v1/market-data/ticker?symbol=BTC_USDC
GET /api/v1/market-data/orderbook?symbol=BTC_USDC&limit=20
GET /api/v1/market-data/klines?symbol=BTC_USDC&interval=1h&limit=100
GET /api/v1/market-data/markets
```

### To Create (Phase D)
```
POST /api/v1/trades              # Create trade with SL/TP
GET  /api/v1/trades              # List active trades
GET  /api/v1/trades/{id}         # Get trade details
PUT  /api/v1/trades/{id}/sl      # Update stop loss
PUT  /api/v1/trades/{id}/tp      # Update take profit
PUT  /api/v1/trades/{id}/breakeven  # Enable break-even
DELETE /api/v1/trades/{id}       # Cancel trade group
```

---

## Risk Engine Summary

### Position Sizing ("Conservative Wins")
```rust
let config = RiskConfig::new()
    .with_account_risk_percent(dec!(2))    // 2% per trade
    .with_max_risk_amount(dec!(100))       // Max $100 loss
    .with_max_position_size(dec!(0.1));    // Max 0.1 BTC

let sizer = PositionSizer::new(config);
let result = sizer.calculate_position_size(
    account_balance,  // $10,000
    entry_price,      // $50,000
    stop_loss_price,  // $49,000
);
// Result: size = 0.1 BTC (limited by max_position_size)
```

### Validation
```rust
let validator = RiskValidator::new(config);
let result = validator.validate(&order, &account_state);

if !result.is_valid {
    // Handle violations: StopLossRequired, PositionSizeExceeded, etc.
}
```

---

## Shadow Engine Usage

```rust
// Initialize
let engine = ShadowEngine::new();
engine.init_user(user_id).await;

// Place order (reserves funds)
let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
engine.place_order(user_id, order).await?;

// Process price update (checks for fills)
let filled = engine.process_price_update(
    "BTC_USDC",
    bid, ask, high, low
).await;

// Get positions and P&L
let positions = engine.get_positions(user_id).await;
let pnl = engine.get_unrealized_pnl(user_id).await;
```

---

## Fill Logic (from PRD)

| Order Type | Side | Fill Condition |
|------------|------|----------------|
| Limit | Buy | `Low <= Limit Price` |
| Limit | Sell | `High >= Limit Price` |
| Market | Buy | Immediate at `Ask` |
| Market | Sell | Immediate at `Bid` |
| Stop Loss | Buy | `High >= Stop Price` |
| Stop Loss | Sell | `Low <= Stop Price` |

---

## Development Guidelines

### TDD Cycle
1. **RED**: Write failing test
2. **GREEN**: Minimal code to pass
3. **REFACTOR**: Clean up

### Build Commands
```bash
cargo check              # Quick compilation check
cargo test               # Run all tests
cargo test shadow        # Run shadow engine tests
cargo test risk          # Run risk engine tests
cargo clippy             # Linting
```

### Code Style
- Use `rust_decimal::Decimal` for all financial values
- `thiserror` for custom error types
- Comprehensive doc comments
- Unit tests for all public methods

---

## Key References

- **PRD**: `hybrid_trading.json` - System requirements
- **Plan**: `HYBRID_TRADING_SYSTEM_PLAN.md` - Implementation roadmap
- **Progress**: `.ralph/progress.md` - Completed work log
- **Phase 2 Tasks**: `phase2-tasks.md` - External connectivity tasks

---

## Test Counts

| Module | Tests |
|--------|-------|
| common_utils (adapters, risk, services) | 237 |
| engine (shadow) | 25 |
| router | 56 |
| sqlx_postgres | 17 |
| **Total** | **341** |
