# Specification: Journal Database Schema

**Spec ID:** JNL-01-schema
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Data Layer
**Priority:** P0 — foundation for all journal features
**Depends on:** None
**Series:** Batch 1 — Data Foundation (JNL-01, JNL-02)

---

## Problem Statement

Testudo executes and manages trades but has no persistent journal layer. Trade data exists transiently in the trade management system but is not stored in a form suitable for analytics, reflection, or historical review. We need a dedicated schema to capture the full trade lifecycle, journal entries, and tagging metadata.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create `journal_trades` table for closed trade records | High | PostgreSQL |
| FR-2 | Create `journal_entries` table for markdown notes linked to trades or time periods | High | PostgreSQL |
| FR-3 | Create `journal_tags` table and `journal_trade_tags` junction table | High | PostgreSQL |
| FR-4 | Create `journal_daily_stats` table for pre-computed daily aggregates | Medium | PostgreSQL |
| FR-5 | Write SQLx migrations for all tables | High | sqlx_postgres |
| FR-6 | Create Rust struct definitions with SQLx `FromRow` derives | High | router crate |

---

## Technical Implementation

### journal_trades

Stores one row per completed trade (entry → exit). Exchange-agnostic — normalized from any adapter.

```sql
CREATE TABLE journal_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    exchange TEXT NOT NULL,              -- "woo", "binance", "hyperliquid", "shadow"
    symbol TEXT NOT NULL,                -- normalized: "BTC_USDT"
    side TEXT NOT NULL,                  -- "LONG" or "SHORT"
    entry_price NUMERIC NOT NULL,
    exit_price NUMERIC NOT NULL,
    quantity NUMERIC NOT NULL,
    leverage INTEGER NOT NULL DEFAULT 1,

    -- P&L
    realized_pnl NUMERIC NOT NULL,
    realized_pnl_pct NUMERIC NOT NULL,  -- % return on margin
    fees NUMERIC NOT NULL DEFAULT 0,
    net_pnl NUMERIC NOT NULL,           -- realized_pnl - fees

    -- Risk metrics
    stop_price NUMERIC,
    target_price NUMERIC,
    risk_amount NUMERIC,                -- $ risked
    r_multiple NUMERIC,                 -- net_pnl / risk_amount

    -- Timing
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ NOT NULL,
    duration_secs INTEGER NOT NULL,

    -- Linkage
    trade_group_id UUID,                -- links to trade management system
    exchange_order_ids TEXT[],          -- array of exchange-side order IDs

    -- Metadata
    notes TEXT,                         -- quick inline note
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journal_trades_user_time ON journal_trades(user_id, closed_at DESC);
CREATE INDEX idx_journal_trades_user_symbol ON journal_trades(user_id, symbol);
CREATE INDEX idx_journal_trades_user_exchange ON journal_trades(user_id, exchange);
```

### journal_entries

Markdown journal entries linked to a specific trade, a date, or free-standing.

```sql
CREATE TABLE journal_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    trade_id UUID REFERENCES journal_trades(id) ON DELETE SET NULL,
    entry_date DATE,                    -- for daily/weekly reflections
    title TEXT NOT NULL,
    body TEXT NOT NULL,                  -- markdown content
    entry_type TEXT NOT NULL DEFAULT 'note',  -- "note", "pre-trade", "post-trade", "daily", "weekly"
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journal_entries_user ON journal_entries(user_id, created_at DESC);
CREATE INDEX idx_journal_entries_trade ON journal_entries(trade_id);
```

### journal_tags

```sql
CREATE TABLE journal_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    color TEXT,                          -- hex color for UI
    UNIQUE(user_id, name)
);

CREATE TABLE journal_trade_tags (
    trade_id UUID NOT NULL REFERENCES journal_trades(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES journal_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (trade_id, tag_id)
);
```

### journal_daily_stats

Pre-computed daily aggregates for fast equity curve and stats queries.

```sql
CREATE TABLE journal_daily_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    stat_date DATE NOT NULL,
    exchange TEXT,                       -- NULL = all exchanges combined

    trade_count INTEGER NOT NULL DEFAULT 0,
    win_count INTEGER NOT NULL DEFAULT 0,
    loss_count INTEGER NOT NULL DEFAULT 0,
    gross_profit NUMERIC NOT NULL DEFAULT 0,
    gross_loss NUMERIC NOT NULL DEFAULT 0,
    net_pnl NUMERIC NOT NULL DEFAULT 0,
    fees NUMERIC NOT NULL DEFAULT 0,

    -- Running totals (for equity curve)
    cumulative_pnl NUMERIC NOT NULL DEFAULT 0,
    peak_cumulative_pnl NUMERIC NOT NULL DEFAULT 0,
    drawdown NUMERIC NOT NULL DEFAULT 0,
    drawdown_pct NUMERIC NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, stat_date, exchange)
);

CREATE INDEX idx_journal_daily_user ON journal_daily_stats(user_id, stat_date);
```

### Rust Structs

Place in `crates/router/src/models/journal.rs`:

```rust
use rust_decimal::Decimal;
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalTrade {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub leverage: i32,
    pub realized_pnl: Decimal,
    pub realized_pnl_pct: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub duration_secs: i32,
    pub trade_group_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub trade_id: Option<Uuid>,
    pub entry_date: Option<NaiveDate>,
    pub title: String,
    pub body: String,
    pub entry_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalTag {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalDailyStat {
    pub id: Uuid,
    pub user_id: Uuid,
    pub stat_date: NaiveDate,
    pub exchange: Option<String>,
    pub trade_count: i32,
    pub win_count: i32,
    pub loss_count: i32,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub net_pnl: Decimal,
    pub fees: Decimal,
    pub cumulative_pnl: Decimal,
    pub peak_cumulative_pnl: Decimal,
    pub drawdown: Decimal,
    pub drawdown_pct: Decimal,
}
```

### Files

- `testudo-exchange/crates/sqlx_postgres/migrations/YYYYMMDD_create_journal_tables.sql`
- `testudo-exchange/crates/router/src/models/journal.rs`
- `testudo-exchange/crates/router/src/models/mod.rs` — add `pub mod journal;`

---

## Acceptance Criteria

- [ ] All 4 tables created via SQLx migration
- [ ] Migration runs cleanly on fresh and existing databases
- [ ] Rust structs compile with `FromRow`, `Serialize`, `Deserialize`
- [ ] Indexes exist for all user-scoped queries
- [ ] `NUMERIC` type used for all financial values (never `FLOAT`)
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Completion Signal

This spec is complete when:
1. Migration applied to development database
2. Rust structs defined and accessible from router crate
3. All acceptance criteria met
4. Code committed to master
