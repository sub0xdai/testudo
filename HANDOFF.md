# Testudo Hybrid Trading System - AI Engineer Handoff

## Quick Start

```bash
cd /home/m0xu/1-projects/testudo/testudo-exchange
cargo test
```

**Expected**: 398 tests passing, 0 failed

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
| **D** | Trade Management (SL/TP, Break-even) | ✅ Complete |
| **E** | Live Execution (Binance Orders) | 🔜 **NEXT** |

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
├── lib.rs                 # Engine library exports
└── shadow/
    ├── mod.rs             # ShadowEngine orchestrator + trade management
    ├── balances.rs        # Virtual balance management
    ├── orders.rs          # Order simulation + fill logic
    ├── positions.rs       # Position tracking + P&L
    └── order_group.rs     # OrderGroup, SL/TP linking, break-even

crates/router/src/routes/
├── market_data.rs         # /api/v1/market-data/* endpoints
└── trade_management.rs    # /api/v1/trades/* endpoints
```

---

## Phase D Complete - Trade Management ✅

**Completed**: 2026-01-10

| Feature | Description |
|---------|-------------|
| **Order Groups** | Link entry orders with SL/TP via `OrderGroup` struct |
| **Auto SL/TP** | SL/TP orders created when entry fills (not when placed) |
| **Break-even** | Move SL to entry when position hits X% profit |
| **Multi-target** | Scale out at multiple TP levels (50% T1, 25% T2, etc.) |
| **Sibling Cancel** | SL fill cancels TPs, TP fill cancels SL |

## API Endpoints

### Market Data (Phase A)
```
GET /api/v1/market-data/ticker?symbol=BTC_USDC
GET /api/v1/market-data/orderbook?symbol=BTC_USDC&limit=20
GET /api/v1/market-data/klines?symbol=BTC_USDC&interval=1h&limit=100
GET /api/v1/market-data/markets
```

### Trade Management (Phase D) ✅
```
POST   /api/v1/trades              # Create trade with SL/TP
GET    /api/v1/trades              # List active trades
GET    /api/v1/trades/{id}         # Get trade details
PUT    /api/v1/trades/{id}/sl      # Update stop loss
PUT    /api/v1/trades/{id}/tp      # Update take profit
PUT    /api/v1/trades/{id}/breakeven  # Enable break-even
DELETE /api/v1/trades/{id}         # Cancel trade group
```

---

## Next Task: Phase E - Live Execution

**Goal**: Connect to Binance for live order execution with shadow verification

### Tasks

| # | Task | Description |
|---|------|-------------|
| 25 | API Key Connection | Secure storage and validation of Binance API keys |
| 26 | Decision Loop | Shadow → External execution flow |
| 27 | Order Execution | Place real orders on Binance |
| 28 | Position Sync | Keep shadow positions in sync with exchange |
| 29 | Risk Validation | Pre-trade checks before live execution |

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
