# Specification: Analytics Query API Endpoints

**Spec ID:** JNL-06-analytics-api
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / API
**Priority:** P0 — serves charts and stat panels
**Depends on:** JNL-03-stats-engine, JNL-04-time-series
**Series:** Batch 3 — Backend API (JNL-05, JNL-06)

---

## Problem Statement

The stats engine and time-series service exist but aren't exposed via HTTP. The frontend dashboard needs REST endpoints to fetch pre-computed analytics. These are read-only, user-scoped, and support the same filter parameters across all queries.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Overview endpoint: account overview + performance + risk stats in one call | High | routes/journal.rs |
| FR-2 | Equity curve endpoint | High | routes/journal.rs |
| FR-3 | Daily P&L endpoint | High | routes/journal.rs |
| FR-4 | Symbol breakdown endpoint | High | routes/journal.rs |
| FR-5 | Duration/profitability scatter endpoint | Medium | routes/journal.rs |
| FR-6 | Return distribution endpoint | Medium | routes/journal.rs |
| FR-7 | Time distribution endpoint | Low | routes/journal.rs |
| FR-8 | All endpoints accept common filter query params | High | routes/journal.rs |

---

## Technical Implementation

### Endpoints

```
# Combined stats (single call for dashboard overview)
GET /api/v1/journal/analytics/overview    — account + performance + risk stats

# Time-series (chart data)
GET /api/v1/journal/analytics/equity-curve
GET /api/v1/journal/analytics/daily-pnl
GET /api/v1/journal/analytics/symbol-breakdown
GET /api/v1/journal/analytics/duration-profit
GET /api/v1/journal/analytics/return-distribution
GET /api/v1/journal/analytics/time-distribution
```

### Common Filter Params

All analytics endpoints accept:
```
?exchange=woo
&symbol=BTC_USDT
&date_from=2026-01-01
&date_to=2026-03-17
&tags=revenge-trade,fomo
```

### Response Shapes

**Overview** (combined for single network round-trip):
```json
{
  "account": {
    "total_trades": 234,
    "total_pnl": "1523.45",
    "total_fees": "89.12",
    "net_pnl": "1434.33"
  },
  "performance": {
    "win_rate": "58.5",
    "profit_factor": "1.82",
    "avg_win": "42.30",
    "avg_loss": "-23.15",
    "largest_win": "312.00",
    "largest_loss": "-156.00",
    "expectancy": "15.12",
    "avg_r_multiple": "1.45",
    "trades_per_day": "2.3",
    "avg_duration_secs": 14400,
    "total_duration_days": 102
  },
  "risk": {
    "max_drawdown": "456.00",
    "max_drawdown_pct": "8.7",
    "worst_day": "-156.00",
    "worst_week": "-234.00",
    "worst_month": "-312.00",
    "best_day": "312.00",
    "best_week": "456.00",
    "best_month": "678.00",
    "risk_of_ruin": "0.02",
    "current_streak": 3,
    "best_streak": 8,
    "worst_streak": -5
  }
}
```

**Equity curve:**
```json
{
  "data": [
    { "date": "2026-01-15", "cumulative_pnl": "45.00", "peak": "45.00", "drawdown": "0", "drawdown_pct": "0" },
    { "date": "2026-01-16", "cumulative_pnl": "23.00", "peak": "45.00", "drawdown": "22.00", "drawdown_pct": "4.4" }
  ]
}
```

All numeric values serialized as strings to prevent precision loss in JSON.

### Route Registration

Add to the journal scope in `main.rs`:

```rust
web::scope("/analytics")
    .route("/overview", web::get().to(journal::analytics_overview))
    .route("/equity-curve", web::get().to(journal::equity_curve))
    .route("/daily-pnl", web::get().to(journal::daily_pnl))
    .route("/symbol-breakdown", web::get().to(journal::symbol_breakdown))
    .route("/duration-profit", web::get().to(journal::duration_profit))
    .route("/return-distribution", web::get().to(journal::return_distribution))
    .route("/time-distribution", web::get().to(journal::time_distribution))
```

### Files

- `testudo-exchange/crates/router/src/routes/journal.rs` — add analytics handlers
- `testudo-exchange/crates/router/src/main.rs` — register analytics sub-scope

---

## Acceptance Criteria

- [ ] Overview returns all three stat groups in a single response
- [ ] All numeric fields serialized as strings (no floating point in JSON)
- [ ] Equity curve returns data sorted by date ascending
- [ ] All endpoints accept and correctly apply filter params
- [ ] Empty data returns `{ "data": [] }` or zeroed stats (not errors)
- [ ] Response times < 100ms for 1000 trades
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Completion Signal

This spec is complete when:
1. All 7 analytics endpoints respond correctly
2. Frontend can render all dashboard panels from these endpoints alone
3. All acceptance criteria met
4. Code committed to master
