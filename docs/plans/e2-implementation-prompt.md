# E.2 Decision Loop Implementation Prompt

Copy and paste everything below the line to start a new session:

---

## Task: Implement E.2 Decision Loop

You are implementing the Decision Loop for the Testudo Hybrid Trading System. The design is complete and approved.

### Context Files
- **PRD**: `@hybrid_trading.json` - E.2 acceptance criteria
- **Design**: `@docs/plans/2026-01-10-e2-decision-loop-design.md` - Full technical design

### What to Build

**Position Sizing Module** (`common_utils/src/risk/sizing.rs`):
- `fixed_fractional(balance, risk_percent, entry, stop)` → Decimal
- `kelly_criterion(win_rate, avg_win, avg_loss)` → Decimal
- `volatility_adjusted(base_size, target_vol, current_atr)` → Decimal
- `calculate_position_size(config, market_data)` → uses all methods, returns minimum (conservative wins)

**Risk Types** (`common_utils/src/risk/types.rs`):
- `RiskCheckResult { approved, calculated_size, sizing_method_used, rejection_reason, warnings }`
- `RiskRejection` enum: InsufficientBalance, DailyDrawdownExceeded, MaxPositionsReached, StopLossRequired
- `RiskWarning` enum: SizeOverrideExceedsCalculated, HighVolatilityDetected, ApproachingDrawdownLimit
- `SizingMethod` enum: FixedFractional, KellyCriterion, VolatilityAdjusted, MaxRiskCap

**Risk Service** (`common_utils/src/risk/service.rs`):
- `RiskService::validate(order, config, balances, positions)` → RiskCheckResult
- Checks: stop-loss required, balance sufficient, drawdown limit, max positions

**Decision Loop** (`router/src/decision_loop.rs`):
- Orchestrates: validate input → create shadow order → risk check → approve/reject
- Wire into `router/src/routes/order.rs` execute_order function

### TDD Required

Follow Red-Green-Refactor. Write failing test first, then implement. Key tests:
1. `sizing_fixed_fractional` - $10k account, 2% risk, entry 50k, stop 49k → 0.2 BTC
2. `sizing_conservative_wins` - Multiple methods, smallest wins
3. `reject_insufficient_balance` - Return clear error
4. `reject_drawdown_exceeded` - Daily loss limit blocks trade
5. `approve_with_warnings` - Passes but warns on high volatility

### Using Ralph Plugin

Add this task to `.ralph/prd.json` then run the script:

```json
{
  "id": "E.2",
  "title": "Implement Decision Loop",
  "description": "Risk validation and position sizing before order execution",
  "status": "pending",
  "files": [
    "crates/common_utils/src/risk/sizing.rs",
    "crates/common_utils/src/risk/types.rs",
    "crates/common_utils/src/risk/service.rs",
    "crates/router/src/decision_loop.rs"
  ]
}
```

Then run:
```bash
./scripts/ralph.sh --max-iterations 3 --completion-promise "E.2 COMPLETE"
```

### Success Criteria

- [ ] All position sizing methods implemented with tests
- [ ] RiskService validates orders against user config
- [ ] Decision Loop wired into execute_order route
- [ ] Rejected orders return detailed reason
- [ ] Approved orders include calculated_size and any warnings
- [ ] All tests pass: `cargo test -p common_utils risk`
- [ ] Mark E.2 as "complete" in hybrid_trading.json

### Do NOT

- Do not use Redis pub/sub (in-process for now)
- Do not skip TDD - write failing test first
- Do not over-engineer - KISS principle applies
