# E.2 Decision Loop Design

**Epic**: E (Live Execution)
**Story**: E.2 - Implement Decision Loop that validates shadow orders before live execution
**Status**: Design approved, ready for implementation

## Acceptance Criteria (from hybrid_trading.json)

```json
"decision_flow": {
  "step_1": "Shadow order created and validated",
  "step_2": "Risk Engine approves (position size, drawdown check)",
  "step_3": "If live mode enabled, forward to Binance executor",
  "step_4": "Binance response updates shadow state"
},
"rejection_handling": "Failed risk check returns detailed reason, order stays in shadow only"
```

## Design Decision

**In-process Risk Service** (not Redis pub/sub) for Phase 2:
- Lower latency (no Redis roundtrip)
- Simpler deployment (no separate Engine process)
- Can migrate to Redis pub/sub later when scaling requires it

## Architecture

```
POST /api/v1/order
    ↓
┌─────────────────────────────────────────┐
│           Decision Loop                  │
├─────────────────────────────────────────┤
│ 1. Validate input & build StandardOrder │
│ 2. Create Shadow Order (persist)        │
│ 3. RiskService.validate(order, config)  │
│    ├─ Check: balance >= required        │
│    ├─ Check: daily drawdown < limit     │
│    ├─ Check: position count < max       │
│    └─ Check: stop-loss if required      │
│ 4. If rejected → return error + reason  │
│ 5. If approved:                         │
│    ├─ Shadow mode → return shadow order │
│    └─ Live mode → forward to Binance    │
│ 6. Update shadow state with result      │
└─────────────────────────────────────────┘
```

## Position Sizing

**Core Formula:**
```
Position Size = (Account Balance × Risk %) ÷ |Entry - Stop|
```

**Multi-Method Approach (Conservative Wins):**

| Method | Formula | Use Case |
|--------|---------|----------|
| Fixed Fractional | `balance × risk% ÷ \|entry - stop\|` | Base calculation |
| Kelly Criterion | `(win_rate × avg_win - loss_rate × avg_loss) / avg_win` | Optimal sizing from historical stats |
| Volatility-Adjusted | `base_size × (target_volatility / current_ATR)` | Scale down in high volatility |
| Max Risk Cap | `min(calculated, max_risk_amount / \|entry - stop\|)` | Hard dollar limit |

**Example:**
```
Account: $10,000
Risk: 2% ($200)
Entry: 50,000, Stop: 49,000 (1000 point risk)
ATR: 1500 (high volatility)

Fixed Fractional: $200 / $1000 = 0.2 BTC
Kelly (60% win, 2:1 R:R): 0.4 × balance = aggressive
Volatility-Adjusted: 0.2 × (1000/1500) = 0.133 BTC
Max Cap ($100 risk): 0.1 BTC

Final: min(0.2, 0.4, 0.133, 0.1) = 0.1 BTC ← Conservative wins
```

## Risk Validation Checks

| Check | Calculation | Fail Reason |
|-------|-------------|-------------|
| Stop-loss present | `order.stop_loss.is_some()` | "Stop-loss required" |
| Position size | `balance × risk% ÷ \|entry - stop\|` | Auto-calculated, not a rejection |
| Max position size | `calculated_size <= config.max_position_size` | "Exceeds max position size" |
| Balance check | `entry × size <= available_balance` | "Insufficient balance" |
| Daily drawdown | `today_pnl >= -config.daily_max_drawdown%` | "Daily drawdown limit reached" |
| Open positions | `open_count < config.max_open_positions` | "Max open positions reached" |

**Latency target:** < 50ms for risk validation (in-memory checks only)

## Types

```rust
pub struct RiskCheckResult {
    pub approved: bool,
    pub calculated_size: Decimal,
    pub sizing_method_used: SizingMethod,
    pub rejection_reason: Option<RiskRejection>,
    pub warnings: Vec<RiskWarning>,
}

pub enum SizingMethod {
    FixedFractional,
    KellyCriterion,
    VolatilityAdjusted,
    MaxRiskCap,
}

pub enum RiskRejection {
    InsufficientBalance { required: Decimal, available: Decimal },
    DailyDrawdownExceeded { current: Decimal, limit: Decimal },
    MaxPositionsReached { current: u32, limit: u32 },
    StopLossRequired,
    MarketClosed,
}

pub enum RiskWarning {
    SizeOverrideExceedsCalculated { requested: Decimal, calculated: Decimal },
    HighVolatilityDetected { current_atr: Decimal },
    ApproachingDrawdownLimit { remaining_percent: Decimal },
}
```

## HTTP Responses

| Scenario | Status | Body |
|----------|--------|------|
| Approved | 201 | Shadow order + calculated size |
| Rejected | 400 | `{ "code": "risk_rejected", "reason": "...", "details": {...} }` |
| Warning + Approved | 201 | Order + warnings array |

## Files to Create/Modify

```
common_utils/src/risk/
├── config.rs          # (existing) RiskConfig
├── mod.rs             # (update) exports
├── service.rs         # NEW: RiskService - validation logic
├── sizing.rs          # NEW: Position sizing calculations
└── types.rs           # NEW: RiskCheckResult, RiskRejection, etc.

router/src/
├── routes/order.rs    # (update) integrate DecisionLoop
└── decision_loop.rs   # NEW: Orchestrates the flow
```

## Testing Strategy

| Test | Description |
|------|-------------|
| `sizing_fixed_fractional` | Basic position size from risk % |
| `sizing_kelly_criterion` | Kelly optimal sizing |
| `sizing_volatility_adjusted` | ATR-scaled sizing |
| `sizing_conservative_wins` | Min of all methods selected |
| `reject_insufficient_balance` | Balance check fails |
| `reject_drawdown_exceeded` | Daily loss limit hit |
| `reject_max_positions` | Too many open trades |
| `approve_with_warnings` | Passes but flags concerns |
| `full_decision_loop` | End-to-end order flow |

## Dependencies

- Shadow balance/positions from Redis (existing)
- ATR from market data service (existing)
- Trade history for Kelly stats (new - defaults to conservative estimates initially)

## TDD Implementation Order

1. RED: Write failing test for `PositionSizer::fixed_fractional`
2. GREEN: Implement fixed fractional sizing
3. RED: Write failing test for `PositionSizer::kelly_criterion`
4. GREEN: Implement Kelly sizing
5. RED: Write failing test for `PositionSizer::volatility_adjusted`
6. GREEN: Implement volatility-adjusted sizing
7. RED: Write failing test for `RiskService::validate`
8. GREEN: Implement risk validation
9. RED: Write failing test for `DecisionLoop::process_order`
10. GREEN: Wire into router
11. REFACTOR: Clean up and optimize
