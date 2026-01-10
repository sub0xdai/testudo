# Next Engineer Prompt: Complete Phase D - Trade Management

## Your Task

Implement **Phase D: Trade Management** for the Testudo Hybrid Trading System. This adds SL/TP linking, break-even automation, and multi-target exits to the existing Shadow Engine.

---

## Context

Read these files first:
- `hybrid_trading.json` - PRD with requirements
- `HYBRID_TRADING_SYSTEM_PLAN.md` - Full implementation plan
- `HANDOFF.md` - Technical reference

### What's Already Built

| Phase | Status | What It Does |
|-------|--------|--------------|
| A | ✅ Done | Live Binance data via `/api/v1/market-data/*` |
| B | ✅ Done | Shadow Engine: paper trading with virtual balances |
| C | ✅ Done | Risk Engine: position sizing, validation |
| **D** | 🔜 **YOUR TASK** | Trade management: SL/TP, break-even, multi-target |

---

## Phase D Requirements

### 1. Order Groups (Link Entry + SL + TP)

**File**: `crates/common_utils/src/models/order_group.rs`

```rust
pub struct OrderGroup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub entry_order_id: Uuid,
    pub stop_loss_order_id: Option<Uuid>,
    pub take_profit_order_ids: Vec<Uuid>,  // Multiple TPs for scaling out
    pub status: OrderGroupStatus,
    pub break_even_enabled: bool,
    pub break_even_trigger_percent: Option<Decimal>,  // Move SL to entry at X% profit
}
```

### 2. SL/TP Auto-Creation on Entry Fill

**File**: `crates/engine/src/shadow/sl_tp.rs`

When an entry order fills, automatically create linked SL and TP orders:

```rust
// In ShadowEngine, after entry fills:
if order.stop_loss_price.is_some() || order.take_profit_price.is_some() {
    self.create_linked_orders(&filled_order).await;
}
```

**Test to write first (TDD)**:
```rust
#[tokio::test]
async fn test_sl_tp_created_on_entry_fill() {
    let engine = ShadowEngine::new();
    let user_id = Uuid::new_v4();
    engine.init_user(user_id).await;

    let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000))
        .with_stop_loss(dec!(49000))
        .with_take_profit(dec!(52000));

    engine.place_order(user_id, order).await.unwrap();

    // Simulate fill
    engine.process_price_update("BTC_USDC", dec!(49900), dec!(50000), dec!(50100), dec!(49900)).await;

    let orders = engine.get_open_orders(user_id).await;
    assert!(orders.iter().any(|o| o.order_type == ShadowOrderType::StopLoss));
    assert!(orders.iter().any(|o| o.order_type == ShadowOrderType::TakeProfit));
}
```

### 3. Break-Even Automation

**File**: `crates/engine/src/shadow/breakeven.rs`

Move SL to entry price when position reaches X% profit:

```rust
pub struct BreakEvenConfig {
    pub trigger_percent: Decimal,  // e.g., 1.0 = move SL to entry at 1% profit
    pub offset: Option<Decimal>,   // Optional: move SL slightly above entry
}

// Check on each price update
fn check_break_even(&self, position: &ShadowPosition, mark_price: Decimal) -> bool {
    let profit_percent = position.unrealized_pnl_percent();
    profit_percent >= self.config.trigger_percent
}
```

**Test**:
```rust
#[tokio::test]
async fn test_break_even_moves_stop_loss() {
    // Entry at 50000, SL at 49000, break-even at 1% profit
    // When price hits 50500 (1% up), SL should move to 50000
}
```

### 4. Multi-Target Exits

**File**: `crates/engine/src/shadow/multi_target.rs`

Scale out of positions at multiple take-profit levels:

```rust
pub struct MultiTargetConfig {
    pub targets: Vec<ExitTarget>,
}

pub struct ExitTarget {
    pub price: Decimal,
    pub percent_to_close: Decimal,  // e.g., 50 = close 50% at this level
}

// Example: Exit 50% at T1, 25% at T2, let 25% run
let config = MultiTargetConfig {
    targets: vec![
        ExitTarget { price: dec!(52000), percent_to_close: dec!(50) },
        ExitTarget { price: dec!(55000), percent_to_close: dec!(25) },
    ],
};
```

### 5. Trade Management API

**File**: `crates/router/src/routes/trade_management.rs`

```
POST   /api/v1/trades                  # Create trade with SL/TP
GET    /api/v1/trades                  # List active trade groups
GET    /api/v1/trades/{id}             # Get trade group details
PUT    /api/v1/trades/{id}/sl          # Update stop loss
PUT    /api/v1/trades/{id}/tp          # Update/add take profit
PUT    /api/v1/trades/{id}/breakeven   # Enable break-even
DELETE /api/v1/trades/{id}             # Cancel entire trade group
```

---

## Files to Create/Modify

### New Files
```
crates/common_utils/src/models/order_group.rs    # OrderGroup struct
crates/engine/src/shadow/sl_tp.rs                # SL/TP linking logic
crates/engine/src/shadow/breakeven.rs            # Break-even automation
crates/engine/src/shadow/multi_target.rs         # Multi-target exits
crates/router/src/routes/trade_management.rs     # API routes
```

### Modify
```
crates/engine/src/shadow/mod.rs                  # Add new modules, update ShadowEngine
crates/engine/src/shadow/orders.rs               # Link orders to groups
crates/router/src/routes/mod.rs                  # Add trade_management
crates/router/src/main.rs                        # Wire up new routes
```

---

## TDD Workflow

1. **Write failing test** in the target module
2. **Run**: `cargo test shadow` or `cargo test trade`
3. **Implement** minimal code to pass
4. **Refactor** and add next test

---

## Verification

After completing Phase D:

```bash
cargo test                    # All tests pass (expect ~380+)
cargo test shadow             # Shadow engine tests
cargo test order_group        # Order group tests
cargo clippy                  # No warnings
```

---

## Key Existing Code to Reference

### ShadowOrder (already has SL/TP fields)
```rust
// crates/engine/src/shadow/orders.rs
pub struct ShadowOrder {
    // ... existing fields ...
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_price: Option<Decimal>,
    pub parent_order_id: Option<Uuid>,
}
```

### ShadowEngine methods to extend
```rust
// crates/engine/src/shadow/mod.rs
impl ShadowEngine {
    pub async fn place_order(...) -> Result<ShadowOrder, ShadowEngineError>
    pub async fn process_price_update(...) -> Vec<ShadowOrder>
    // Add: create_order_group, update_stop_loss, enable_break_even, etc.
}
```

---

## Success Criteria

- [ ] Entry orders with SL/TP automatically create linked orders on fill
- [ ] Cancelling entry cancels all linked SL/TP orders
- [ ] Break-even moves SL to entry when profit threshold reached
- [ ] Multi-target exits close partial positions at each level
- [ ] API routes work for CRUD on trade groups
- [ ] All new code has unit tests
- [ ] `cargo test` passes with 0 failures
