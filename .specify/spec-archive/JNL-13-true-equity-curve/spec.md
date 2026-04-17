# Specification: True Equity Curve via Balance Snapshots

**Spec ID:** JNL-13-true-equity-curve
**Date:** 2026-04-04
**Status:** Draft
**Class:** Feature / Analytics
**Priority:** P1 — Max drawdown percentage is misleading without account equity as denominator; equity curve shows relative P&L instead of actual account value
**Depends on:** None (first in series)
**Series:** JNL-13 (true equity curve)

---

## Problem Statement

The current equity curve and max drawdown calculations use cumulative P&L as the sole data source. The equity curve plots `SUM(net_pnl)` starting from zero (`journal_timeseries.rs:106-134`), and max drawdown divides peak-to-trough by peak *cumulative P&L* (`journal_stats.rs:481-482`). This produces misleading percentages: a trader with a $1,000 account who gains $10 then loses $9 sees a 90% drawdown — mathematically correct against P&L peak, but the actual account drawdown was 0.9%.

The root cause is that Testudo has no record of account equity. The `journal_trades` table stores individual trade outcomes, and `journal_daily_stats` stores daily aggregations, but neither captures the account balance at any point in time. Without this denominator, percentage-based risk metrics are meaningless.

The fix is a `balance_snapshots` table that captures account equity at trade boundaries, sourced from the existing CCXT sidecar `POST /balance` and Hyperliquid native balance APIs — both already wired through `routes/exchanges.rs:get_exchange_balance`. The equity curve then plots actual account value over time, and drawdown uses peak account equity as the denominator. For users without API connections, a configurable `starting_balance` on the exchange account provides a fallback: `starting_balance + cumulative_pnl`.

---

## User Stories

- **As a trader**, I want the equity curve to show my actual account value over time, so that I can see my real portfolio trajectory — not just a profit/loss offset from zero.
- **As a trader**, I want max drawdown calculated against my account size, so that a $9 loss on a $1,000 account shows as ~0.9% drawdown — not 90%.
- **As a trader without an API connection**, I want to set a starting balance for my journal, so that percentage metrics are still meaningful for manually entered trades.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create `balance_snapshots` table storing `(user_id, exchange_account_id, equity, available, timestamp)` with `Decimal` precision | High | Database |
| FR-2 | Capture a balance snapshot automatically when a trade is closed (journal trade inserted), by calling the existing balance endpoint for that exchange account | High | Router |
| FR-3 | Add optional `starting_balance` column to `exchange_accounts` table for fallback equity calculation | High | Database |
| FR-4 | New endpoint `GET /api/v1/journal/analytics/equity-curve` returns true equity when snapshots exist: `equity` values over time instead of `cumulative_pnl` | High | Router |
| FR-5 | Recompute `max_drawdown_pct` using peak account equity as denominator: `(peak_equity - current_equity) / peak_equity * 100` | High | Router |
| FR-6 | Fallback: when no snapshots exist, use `starting_balance + cumulative_pnl` as equity. When no starting balance set either, fall back to current behavior (cumulative P&L only) with a UI indicator that the percentage is P&L-relative | Medium | Router / Frontend |
| FR-7 | Add "Starting Balance" input field to the Account settings page, stored on the exchange account record | Medium | Frontend |
| FR-8 | HeroEquityCurve baseline shifts from 0 to the starting equity value, showing actual account trajectory | Medium | Frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Migration + snapshot insert on trade close + starting_balance column | Snapshots written to DB on trade close; query returns rows |
| CP-2 | Equity curve endpoint uses snapshots; drawdown uses equity denominator | API returns true equity data; drawdown % matches expected values |
| CP-3 | Frontend renders true equity curve; starting balance settings UI | Visual verification at desk.testudo.vip |

### Database Schema

```sql
-- New table: balance_snapshots
CREATE TABLE balance_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    exchange_account_id UUID NOT NULL REFERENCES exchange_accounts(id),
    equity NUMERIC NOT NULL,          -- total account equity
    available NUMERIC NOT NULL,       -- free/available margin
    snapshot_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_balance_snapshots_user_time
    ON balance_snapshots(user_id, snapshot_at DESC);
CREATE INDEX idx_balance_snapshots_account
    ON balance_snapshots(exchange_account_id, snapshot_at DESC);

-- Add starting_balance to exchange_accounts
ALTER TABLE exchange_accounts
    ADD COLUMN starting_balance NUMERIC;
```

### Snapshot Capture Flow

When a journal trade is inserted (trade close event), the router already knows the `exchange_account_id`. The capture flow:

1. Trade closes -> `journal_trades` row inserted
2. After successful insert, spawn a background task to fetch balance
3. Call existing `get_exchange_balance()` logic (routes/exchanges.rs:416-495)
4. Insert `balance_snapshots` row with the returned equity

```rust
// In journal trade insertion handler (after successful trade insert)
pub async fn capture_balance_snapshot(
    pool: &PgPool,
    user_id: Uuid,
    exchange_account_id: Uuid,
    cex_client: &CexClient,
    // ... credentials
) -> Result<(), anyhow::Error> {
    let balance = cex_client.fetch_balance(
        &exchange_name, &creds, false, "future"
    ).await?;

    let equity = balance.iter()
        .find(|b| b.asset == "USDT")
        .map(|b| Decimal::from_str(&b.total).unwrap_or(Decimal::ZERO))
        .unwrap_or(Decimal::ZERO);

    sqlx::query(
        "INSERT INTO balance_snapshots \
         (id, user_id, exchange_account_id, equity, available, snapshot_at) \
         VALUES ($1, $2, $3, $4, $5, NOW())"
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(exchange_account_id)
    .bind(equity)
    .bind(available)
    .execute(pool)
    .await?;

    Ok(())
}
```

### Equity Curve Computation (Updated)

```rust
// journal_timeseries.rs — updated equity curve
pub fn compute_equity_curve_from_snapshots(
    snapshots: &[(NaiveDate, Decimal)],  // (date, equity)
) -> Vec<EquityCurvePoint> {
    let mut peak = Decimal::ZERO;
    snapshots.iter().map(|(date, equity)| {
        if *equity > peak { peak = *equity; }
        let drawdown = peak - equity;
        let drawdown_pct = if peak > Decimal::ZERO {
            (drawdown / peak) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        EquityCurvePoint {
            date: *date,
            cumulative_pnl: *equity,  // Now represents true equity
            peak,
            drawdown,
            drawdown_pct,
        }
    }).collect()
}
```

### Drawdown SQL (Updated)

```sql
-- When snapshots exist: use equity values
WITH equity_points AS (
    SELECT
        equity,
        MAX(equity) OVER (
            ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
        ) as peak
    FROM balance_snapshots
    WHERE user_id = $1
        AND ($2::DATE IS NULL OR snapshot_at >= $2)
        AND ($3::DATE IS NULL OR snapshot_at <= $3)
    ORDER BY snapshot_at
)
SELECT
    COALESCE(MAX(peak - equity), 0) as max_drawdown,
    COALESCE(
        MAX(CASE WHEN peak > 0
            THEN (peak - equity) / peak * 100
            ELSE 0
        END), 0
    ) as max_drawdown_pct
FROM equity_points;

-- Fallback when no snapshots: starting_balance + cumulative P&L
WITH cumulative AS (
    SELECT
        SUM(net_pnl) OVER (ORDER BY closed_at) + $6 as equity
    FROM journal_trades
    WHERE user_id = $1 ...
),
...
```

### Fallback Priority

| Priority | Condition | Equity Source |
|----------|-----------|---------------|
| 1 | `balance_snapshots` rows exist for user | Snapshot `equity` values |
| 2 | `starting_balance` set on exchange account | `starting_balance + SUM(net_pnl)` |
| 3 | Neither available | `SUM(net_pnl)` (current behavior, label as "P&L-relative") |

### Frontend Changes

**HeroEquityCurve.tsx**: Change baseline from `0` to the first equity value in the series. When true equity data is available, the chart shows actual account value (e.g., $1,000 -> $1,050 -> $980) rather than P&L offset ($0 -> $50 -> -$20).

**EquityPoint type update**:
```typescript
interface EquityPoint {
    date: string
    equity: string           // NEW: true account equity (when available)
    cumulative_pnl: string   // kept for backward compat
    peak: string
    drawdown: string
    drawdown_pct: string
    is_true_equity: boolean  // NEW: flag indicating data source
}
```

**Account settings**: Add "Starting Balance" input to the exchange account configuration, stored via `PATCH /api/v1/exchanges/accounts/{id}`.

### Paved Roads

- Balance fetching already wired: `routes/exchanges.rs:get_exchange_balance` handles both Hyperliquid and CEX sidecar
- CEX client balance method exists: `cex_client.rs:fetch_balance()`
- Journal trade insertion happens in `routes/journal.rs` — snapshot capture hooks in after the insert
- `journal_daily_stats` already has `cumulative_pnl` and `peak_cumulative_pnl` columns — pattern to follow
- All financial math uses `rust_decimal::Decimal` — maintain this for snapshots

### Files

- `crates/sqlx_postgres/migrations/YYYYMMDD_balance_snapshots.up.sql` — new migration
- `crates/router/src/services/balance_snapshot.rs` — new service: capture + query
- `crates/router/src/services/journal_stats.rs` — update `fetch_drawdown_sql()` to use equity denominator
- `crates/router/src/services/journal_timeseries.rs` — update `equity_curve()` to prefer snapshots
- `crates/router/src/routes/journal.rs` — hook snapshot capture on trade insert; update equity curve response
- `crates/router/src/routes/exchanges.rs` — add `starting_balance` to account PATCH handler
- `crates/router/src/main.rs` — register any new routes
- `testudo-journal/src/api/client.ts` — update `EquityPoint` type, add starting balance API
- `testudo-journal/src/components/HeroEquityCurve.tsx` — dynamic baseline
- `testudo-journal/src/components/Overview.tsx` — pass equity source flag
- `testudo-journal/src/components/Account.tsx` — starting balance input [CLARIFY: where is the account/settings page?]

### Dependencies Added

None — all required crates (`sqlx`, `rust_decimal`, `uuid`, `chrono`) are already in use.

---

## Acceptance Criteria

- [ ] `balance_snapshots` table created with proper indexes (FR-1)
- [ ] Balance snapshot captured automatically on trade close (FR-2)
- [ ] Snapshot capture failure does not block trade insertion (FR-2, error path)
- [ ] `starting_balance` column added to `exchange_accounts` (FR-3)
- [ ] Equity curve endpoint returns true equity when snapshots exist (FR-4)
- [ ] Max drawdown uses peak equity denominator when snapshots exist (FR-5)
- [ ] Fallback to `starting_balance + cumulative_pnl` when no snapshots (FR-6)
- [ ] Fallback to cumulative P&L when neither snapshots nor starting balance exist (FR-6)
- [ ] Starting balance configurable from frontend (FR-7)
- [ ] HeroEquityCurve renders actual account value, not P&L from zero (FR-8)
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Balance fetch latency on trade close** — Exchange API calls add 200-500ms to trade close path. Mitigation: spawn snapshot capture as a background task (`tokio::spawn`); trade insertion completes immediately regardless of snapshot success.
2. **Stale snapshots during periods of no trading** — Account equity changes from open positions, funding fees, and manual transfers even when no trades close. Mitigation: Phase 2 could add periodic polling (e.g., hourly cron), but this spec focuses on trade-boundary snapshots which are sufficient for drawdown accuracy.
3. **Multi-exchange aggregation** — User may trade on multiple exchanges; equity curve currently filters by exchange. Mitigation: snapshots are per-account; equity curve aggregation follows the same filter pattern as existing journal queries. Cross-exchange totaling is out of scope.

---

## Completion Signal

This spec is complete when:
1. Balance snapshots are captured on every trade close event
2. Equity curve displays true account equity when snapshot data is available
3. Max drawdown percentage uses account equity as denominator
4. Starting balance fallback is functional for manual journal users
5. All acceptance criteria met
6. `cargo clippy --all-targets && cargo test` passes
7. `cd testudo-journal && bun run build` passes
8. Code committed to master
