# Specification: Trade Event Ingestion Pipeline

**Spec ID:** JNL-02-ingestion
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Data Layer
**Priority:** P0 — data must flow before analytics work
**Depends on:** JNL-01-schema
**Series:** Batch 1 — Data Foundation (JNL-01, JNL-02)

---

## Problem Statement

The journal schema exists but has no data flowing into it. When trades close (via fill detection, manual close, or stop/TP trigger), the journal_trades table must receive a normalized record regardless of which exchange adapter originated the trade. The ingestion pipeline must be idempotent — re-processing a fill must not create duplicate journal entries.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Write a `JournalService` that accepts closed trade data and persists to `journal_trades` | High | router/services |
| FR-2 | Hook into existing trade close flow (fill_detector, trade_manager) to trigger journal write | High | router/services |
| FR-3 | Compute derived fields on ingestion: `realized_pnl_pct`, `net_pnl`, `r_multiple`, `duration_secs` | High | journal service |
| FR-4 | Update `journal_daily_stats` on each trade close (upsert daily row) | High | journal service |
| FR-5 | Idempotent writes — `trade_group_id` uniqueness prevents duplicate journal entries | High | journal service |
| FR-6 | Support all exchange sources: CCXT sidecar (WOO/Binance/Bybit), Hyperliquid SDK, Shadow engine, future DEX | High | journal service |

---

## Technical Implementation

### JournalService

```rust
// crates/router/src/services/journal_service.rs

pub struct JournalService {
    pool: PgPool,
}

/// Input from any trade close event — exchange-agnostic
pub struct TradeCloseEvent {
    pub user_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: String,           // "LONG" | "SHORT"
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub leverage: i32,
    pub fees: Decimal,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub trade_group_id: Option<Uuid>,
    pub exchange_order_ids: Vec<String>,
}

impl JournalService {
    pub async fn record_trade_close(&self, event: TradeCloseEvent) -> Result<JournalTrade, Error>;
    async fn compute_derived_fields(&self, event: &TradeCloseEvent) -> DerivedFields;
    async fn upsert_daily_stats(&self, trade: &JournalTrade) -> Result<(), Error>;
}
```

### Derived Field Computation

```rust
struct DerivedFields {
    realized_pnl: Decimal,
    realized_pnl_pct: Decimal,
    net_pnl: Decimal,
    r_multiple: Option<Decimal>,
    duration_secs: i32,
}

fn compute(event: &TradeCloseEvent) -> DerivedFields {
    let pnl = match event.side.as_str() {
        "LONG" => (event.exit_price - event.entry_price) * event.quantity,
        "SHORT" => (event.entry_price - event.exit_price) * event.quantity,
        _ => Decimal::ZERO,
    };
    let margin = (event.entry_price * event.quantity) / Decimal::from(event.leverage);
    let pnl_pct = if margin > Decimal::ZERO { (pnl / margin) * Decimal::from(100) } else { Decimal::ZERO };
    let net = pnl - event.fees;
    let r_mult = event.risk_amount.filter(|r| *r > Decimal::ZERO).map(|r| net / r);
    let duration = (event.closed_at - event.opened_at).num_seconds() as i32;

    DerivedFields { realized_pnl: pnl, realized_pnl_pct: pnl_pct, net_pnl: net, r_multiple: r_mult, duration_secs: duration }
}
```

### Hook Points

The journal write triggers from two existing flows:

1. **Fill detector** (`services/fill_detector.rs`) — when a closing fill completes a trade group
2. **Trade manager** (`services/trade_manager/service.rs`) — when `close_trade` is called explicitly

Both already know the trade group, entry/exit prices, and fees. Add a `JournalService` call at the point where the trade status transitions to `Closed`.

### Daily Stats Upsert

On each trade close, upsert the daily stats row:

```sql
INSERT INTO journal_daily_stats (user_id, stat_date, exchange, trade_count, win_count, loss_count, gross_profit, gross_loss, net_pnl, fees, cumulative_pnl, peak_cumulative_pnl, drawdown, drawdown_pct)
VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
ON CONFLICT (user_id, stat_date, exchange)
DO UPDATE SET
    trade_count = journal_daily_stats.trade_count + 1,
    win_count = journal_daily_stats.win_count + EXCLUDED.win_count,
    loss_count = journal_daily_stats.loss_count + EXCLUDED.loss_count,
    gross_profit = journal_daily_stats.gross_profit + EXCLUDED.gross_profit,
    gross_loss = journal_daily_stats.gross_loss + EXCLUDED.gross_loss,
    net_pnl = journal_daily_stats.net_pnl + EXCLUDED.net_pnl,
    fees = journal_daily_stats.fees + EXCLUDED.fees,
    cumulative_pnl = $10,
    peak_cumulative_pnl = GREATEST(journal_daily_stats.peak_cumulative_pnl, $10),
    drawdown = $12,
    drawdown_pct = $13;
```

Cumulative fields require a running total query after the upsert to stay consistent.

### Files

- `testudo-exchange/crates/router/src/services/journal_service.rs` — new
- `testudo-exchange/crates/router/src/services/mod.rs` — add `pub mod journal_service;`
- `testudo-exchange/crates/router/src/services/fill_detector.rs` — add journal hook
- `testudo-exchange/crates/router/src/services/trade_manager/service.rs` — add journal hook

---

## Acceptance Criteria

- [ ] `JournalService::record_trade_close()` persists to `journal_trades`
- [ ] Derived fields (P&L, R-multiple, duration) computed correctly
- [ ] Daily stats upserted on each trade close
- [ ] Idempotent: calling with same `trade_group_id` twice does not create duplicates
- [ ] Works for all exchange types (exchange field is just a string)
- [ ] Unit tests for derived field computation
- [ ] Integration test: close a trade → verify journal row exists
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Completion Signal

This spec is complete when:
1. Closing a trade automatically creates a journal record
2. Daily stats update on each close
3. All acceptance criteria met
4. Code committed to master
