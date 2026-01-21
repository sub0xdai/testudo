# Testudo: A Risk-First Trading Architecture

**Read time: 8 minutes**

This is a technical deep-dive into how Testudo's architecture enforces risk management at every layer — from position sizing to order lifecycle management.

---

## The Core Insight

Most trading systems bolt risk management on as an afterthought. A max position check here, a leverage limit there.

Testudo inverts this. **Risk validation is the main code path.** Every order — paper or live — flows through the same Decision Loop, which treats risk checks as blocking prerequisites, not optional safeguards.

```
┌─────────────────────────────────────────────────────────────────┐
│                         Decision Loop                           │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────────┐ │
│  │ Order Request│ → │ Risk Service │ → │ Execution (Shadow or │ │
│  │              │   │ (Validation  │   │ Live via Binance)    │ │
│  │              │   │  + Sizing)   │   │                      │ │
│  └──────────────┘   └──────────────┘   └──────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

The Shadow Engine isn't just for paper trading. It's the **simulation layer** that proves your risk rules work before real money moves.

---

## 1. Decision Loop: The Gatekeeper

Every order enters through `DecisionLoop::execute()`. This function orchestrates:

1. Input validation
2. Risk checks via `RiskService::validate()`
3. Position sizing calculation
4. Approval or rejection
5. Routing to Shadow or Live execution

```rust
// crates/router/src/decision_loop.rs

pub fn execute(
    &self,
    input: &DecisionInput,
    account: &AccountState,
    market_data: Option<&MarketData>,
) -> DecisionResult {
    // Convert to OrderRequest for risk service
    let order_request = OrderRequest {
        symbol: input.symbol.clone(),
        side: input.side.into(),
        user_size: input.quantity,
        entry_price: input.entry_price,
        stop_loss_price: input.stop_loss_price,
        take_profit_price: input.take_profit_price,
        leverage: input.leverage.max(1),
    };

    // Run risk validation - this is THE critical path
    let risk_result = self.risk_service.validate(
        &order_request,
        account,
        market_data,
        None,
    );

    // No approval = no execution
    if risk_result.approved {
        DecisionResult::approved(size, method).with_warnings(risk_result.warnings)
    } else {
        DecisionResult::rejected(rejection)
    }
}
```

Key insight: **The same validation runs for both paper and live trading.** There's no "skip risk checks for paper mode" escape hatch.

---

## 2. Risk Service: Blocking Checks + Conservative Sizing

The `RiskService` implements two distinct phases:

### Phase A: Blocking Checks

These return immediate rejection. No sizing calculation, no warnings — just `rejected`.

```rust
// crates/common_utils/src/risk/service.rs

// 1. Stop loss requirement
if self.config.require_stop_loss && order.stop_loss_price.is_none() {
    return RiskCheckResult::rejected(RiskRejection::StopLossRequired);
}

// 2. Leverage limit
if order.leverage > self.config.max_leverage {
    return RiskCheckResult::rejected(RiskRejection::LeverageExceeded { ... });
}

// 3. Max open positions
if account.open_position_count >= max_positions {
    return RiskCheckResult::rejected(RiskRejection::MaxPositionsReached { ... });
}

// 4. Daily drawdown limit
if current_drawdown >= max_drawdown {
    return RiskCheckResult::rejected(RiskRejection::DailyDrawdownExceeded { ... });
}

// 5. Risk/reward ratio
if rr_ratio < min_rr {
    return RiskCheckResult::rejected(RiskRejection::InsufficientRiskReward { ... });
}
```

### Phase B: Conservative Sizing

If all blocking checks pass, sizing kicks in. The core principle: **Conservative Wins**.

```rust
// crates/common_utils/src/risk/position_sizer.rs

// Calculate all possible sizes
let size_from_percent = account_balance * risk_percent / stop_distance;
let size_from_max_risk = max_risk_amount / stop_distance;
let size_from_max_position = max_position_size;

// Take the MINIMUM
let mut min_size = size_from_percent;
let mut limiting_factor = LimitingFactor::AccountRiskPercent;

if size_from_max_risk < min_size {
    min_size = size_from_max_risk;
    limiting_factor = LimitingFactor::MaxRiskAmount;
}

if size_from_max_position < min_size {
    min_size = size_from_max_position;
    limiting_factor = LimitingFactor::MaxPositionSize;
}

// Final margin check
if required_margin > account_balance {
    min_size = account_balance * leverage / entry_price;
    limiting_factor = LimitingFactor::InsufficientBalance;
}
```

The `limiting_factor` tells you exactly which constraint bound the position size. Transparency, not magic.

---

## 3. Shadow Engine: The Simulation Layer

The Shadow Engine (`crates/engine/src/shadow/`) provides a complete trading simulation:

### Components

| Component | Responsibility |
|-----------|----------------|
| `ShadowBalanceManager` | Virtual balances per user/asset, with reserve/release for open orders |
| `ShadowOrderManager` | Order book simulation, fill logic against live prices |
| `ShadowPositionManager` | Position tracking, P&L calculation from mark price |
| `OrderGroupManager` | Links entry orders to SL/TP, handles cascade cancellation |

### Fill Logic

Orders fill when price conditions are met:

```rust
// Buy Limit:  fills if Low Price <= Limit Price
// Sell Limit: fills if High Price >= Limit Price
// Buy Market: fills immediately at Best Ask
// Sell Market: fills immediately at Best Bid
```

The engine processes live market data from Binance and checks all open orders against current bid/ask/high/low.

### Order Groups: The Trade Management Layer

This is where the architecture gets interesting. An `OrderGroup` links an entry order with its associated SL/TP orders:

```rust
// crates/engine/src/shadow/order_group.rs

pub struct OrderGroup {
    pub id: Uuid,
    pub entry_order_id: Uuid,
    pub entry_price: Option<Decimal>,       // Set on fill
    pub stop_loss_order_id: Option<Uuid>,   // Created on entry fill
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_order_ids: Vec<Uuid>,   // Created on entry fill
    pub take_profit_targets: Vec<TakeProfitTarget>,
    pub status: OrderGroupStatus,
    pub break_even_config: Option<BreakEvenConfig>,
}
```

The lifecycle:

1. **Entry placed** → `OrderGroup` created with `status: Pending`
2. **Entry fills** → SL/TP orders auto-created, `status: Active`
3. **SL fills** → All TP orders cancelled, `status: StoppedOut`
4. **TP fills** → SL order cancelled, `status: TookProfit`
5. **Entry cancelled (before fill)** → Group cancelled, no orphan orders

This is **atomic trade management**. You can't have a position without a stop loss if your config requires one.

---

## 4. Position Tracking with Mark Price

Positions track unrealized P&L using mark price (mid-price), not bid/ask:

```rust
// crates/engine/src/shadow/positions.rs

fn calculate_unrealized_pnl(&self) -> Decimal {
    match self.side {
        PositionSide::Long => (self.mark_price - self.entry_price) * self.size,
        PositionSide::Short => (self.entry_price - self.mark_price) * self.size,
    }
}
```

Why mark price? Bid/ask spread flickers constantly. Using mark price prevents P&L noise from triggering false break-even or trailing stop conditions.

Position averaging works correctly:

```rust
// Adding to a position
let total_cost = (position.entry_price * position.size) + (fill_price * order.quantity);
let new_size = position.size + order.quantity;
position.entry_price = total_cost / new_size;  // Weighted average
```

---

## 5. Break-Even Automation

Order groups support automatic stop-loss movement:

```rust
pub struct BreakEvenConfig {
    pub trigger_percent: Decimal,  // e.g., 1.0 = move SL at 1% profit
    pub offset: Option<Decimal>,   // e.g., 10 = SL at entry + $10
    pub triggered: bool,           // One-shot, won't trigger twice
}
```

When `check_break_even()` is called with live prices:

```rust
pub fn should_trigger_break_even(&self, current_price: Decimal) -> bool {
    if config.triggered { return false; }

    let profit_percent = ((current_price - entry_price) / entry_price) * 100;
    profit_percent >= config.trigger_percent
}
```

If triggered, the SL order's stop price is updated to `entry_price + offset`. The position is now risk-free (or better).

---

## 6. Multi-Target Exits

Scale out at multiple price levels:

```rust
pub struct TakeProfitTarget {
    pub price: Decimal,
    pub percent_to_close: Decimal,  // e.g., 50 = close 50% at this level
    pub order_id: Option<Uuid>,
    pub filled: bool,
}
```

Example configuration:
- TP1: Close 50% at $52,000
- TP2: Close 30% at $55,000
- TP3: Close 20% at $60,000

When entry fills, separate TP orders are created for each target with the correct quantities. The SL order covers the full position until targets are hit.

---

## 7. Why This Architecture?

### Paper trading validates production code

The Shadow Engine runs the **exact same risk validation** as live execution. Bugs in risk logic surface in paper trading before they cost real money.

### Risk is structural, not optional

You can't bypass `RiskService::validate()`. It's not a flag you disable. The blocking checks run on every order, period.

### Trade management is atomic

`OrderGroup` ensures you can't have orphan SL/TP orders. Entry cancellation cascades. SL fill cancels TPs. The state machine is correct by construction.

### Position sizing is transparent

`LimitingFactor` tells you exactly which constraint bound your size. No guessing why your order was smaller than expected.

---

## 8. The Configuration Surface

All of this is controlled by `RiskConfig`:

```rust
RiskConfig::new()
    .with_account_risk_percent(dec!(2))      // 2% per trade
    .with_max_risk_amount(dec!(100))         // Max $100 loss per trade
    .with_max_position_size(dec!(0.1))       // Max 0.1 BTC
    .with_max_leverage(5)                    // 5x max
    .with_daily_max_drawdown(dec!(5))        // Stop at 5% daily loss
    .with_max_open_positions(3)              // Max 3 concurrent
    .with_require_stop_loss(true)            // Mandatory SL
    .with_min_risk_reward(dec!(1.5))         // Min 1.5:1 R:R
```

Presets exist for common profiles:
- `RiskConfig::conservative()` — 1% risk, 3% daily max, 2:1 R:R
- `RiskConfig::aggressive()` — 5% risk, 10% daily max, no SL required

---

## Summary

| Layer | File | Responsibility |
|-------|------|----------------|
| Decision Loop | `decision_loop.rs` | Orchestration, routing |
| Risk Service | `risk/service.rs` | Validation + sizing |
| Position Sizer | `risk/position_sizer.rs` | "Conservative Wins" sizing |
| Shadow Engine | `shadow/mod.rs` | Paper trading simulation |
| Order Groups | `shadow/order_group.rs` | SL/TP linking, cascade cancel |
| Positions | `shadow/positions.rs` | P&L tracking |

The architecture enforces a simple invariant: **no order executes without passing risk validation**. Paper trading and live trading share this code path. The Shadow Engine proves your risk rules work before real money is at stake.

---

*Testudo: "The best risk management is the kind you can't override."*
