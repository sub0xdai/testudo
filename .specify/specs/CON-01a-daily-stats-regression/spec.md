# Specification: Fix CON-01 Regressions in TradeEventWriter

**Spec ID:** CON-01a-daily-stats-regression
**Date:** 2026-03-31
**Status:** Draft
**Class:** Core / Data Pipeline
**Priority:** P1 — Daily P&L chart broken for ALL exchanges, exchange name wrong for CON-01 path
**Depends on:** None (independent fix, but related to REL-02 series)
**Series:** REL-02 through CON-01a (HL data pipeline fix + CON-01 regression cleanup)

---

## Problem Statement

CON-01 (`c7d1e44`, Mar 30) rewired journal writes from a direct `JournalService::record_trade_close()` call to an atomic co-write in `TradeEventWriter::insert_journal_trade()`. This fixed the fire-and-forget crash risk but introduced three regressions:

1. **Daily stats not upserted** — `TradeEventWriter::insert_journal_trade()` (line 268) has a TODO comment but never calls `upsert_daily_stats()`. Every trade closed via the CON-01 path (CCXT/WOO) since Mar 30 is missing from `journal_daily_stats`. The Daily P&L chart reads from this table and shows no data.

2. **JNL-20 draft notes not merged** — `JournalService::record_trade_close()` (lines 167-191) deletes the draft from `journal_trade_drafts` and sets `notes` on the journal row. The TradeEventWriter path skips this entirely. Draft notes for active trades are orphaned on close.

3. **Exchange hardcoded to "cex"** — `fill_detector.rs:571` sets `"exchange": "cex"` in the TradeClosed payload. For HL trades going through FillDetector (entry fills), this writes the wrong exchange name. For CCXT/WOO this is also wrong — it should be the actual exchange name (e.g. "woo", "binance").

These regressions affect the CCXT/WOO path. The HL path is separately broken (REL-02). Fixing these ensures that once REL-02 bypasses FillDetector for HL closes, the remaining CON-01 path works correctly for all other exchanges.

---

## User Stories

- **As a trader**, I want the Daily P&L chart to show all my closed trades, so that I can track daily performance.
- **As a trader**, I want notes I write on active trades to survive when the trade closes, so that my journal context isn't lost.
- **As a trader**, I want the correct exchange name in my journal, so that I can filter trades by exchange accurately.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After `insert_journal_trade()` succeeds, call `upsert_daily_stats()` within the same flush cycle (not necessarily same transaction — daily stats are recomputable) | High | TradeEventWriter |
| FR-2 | After journal insert, merge draft notes from `journal_trade_drafts` if `trade_group_id` is present (same logic as `JournalService::record_trade_close()` lines 167-191) | Medium | TradeEventWriter |
| FR-3 | `emit_trade_closed()` in FillDetector resolves the actual exchange name from the OrderGroup's `exchange_account_id` (or default to the account's exchange name), not hardcoded `"cex"` | High | fill_detector.rs |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add `upsert_daily_stats()` call after `insert_journal_trade()` | Daily P&L chart populates for new trades |
| CP-2 | Add draft notes merge after journal insert | Notes survive trade close |
| CP-3 | Resolve exchange name from OrderGroup in `emit_trade_closed()` | Journal shows correct exchange name |

### FR-1: Daily Stats Upsert

In `trade_event_writer.rs`, after the transaction commits (line 179), iterate TradeClosed events and upsert daily stats:

```rust
tx.commit().await?;

// Post-commit: upsert daily stats for TradeClosed events (non-critical, fire-and-forget)
for event in batch {
    if event.event_type == TradeEventType::TradeClosed {
        if let Some(close_event) = parse_trade_close_payload(event.user_id, event.group_id.unwrap_or_default(), &event.payload) {
            let derived = compute_derived_fields(&close_event);
            if let Err(e) = self.upsert_daily_stats(&close_event, &derived).await {
                tracing::warn!(group_id = ?event.group_id, "daily stats upsert failed: {e}");
            }
        }
    }
}
```

The `upsert_daily_stats()` logic can be extracted from `JournalService` into a shared function (or TradeEventWriter gets its own copy using the same SQL). Since daily stats are recomputable from `journal_trades`, this is fire-and-forget.

### FR-2: Draft Notes Merge

Same pattern — after commit, for each TradeClosed event with a `group_id`:

```rust
if let Some(group_id) = event.group_id {
    if let Ok(Some(Some(notes))) = sqlx::query_scalar::<_, Option<String>>(
        "DELETE FROM journal_trade_drafts WHERE trade_group_id = $1 RETURNING notes"
    ).bind(group_id).fetch_optional(&self.pool).await {
        if !notes.is_empty() {
            let _ = sqlx::query(
                "UPDATE journal_trades SET notes = $1 WHERE trade_group_id = $2 AND (notes IS NULL OR notes = '')"
            ).bind(&notes).bind(group_id).execute(&self.pool).await;
        }
    }
}
```

### FR-3: Exchange Name Resolution — Full Implementation

Add `exchange_name: Option<String>` field to `OrderGroup`. Set it at creation time when the exchange account is known. This avoids async DB lookups in the hot fill-detection path.

**Why not a DB lookup at emit time:** `emit_trade_closed()` is sync (`fn`, not `async fn`). Making it async would ripple through FillDetector's control flow. Storing the name on the group is cleaner.

**OrderGroup field addition:**

```rust
// engine/src/shadow/order_group.rs
pub struct OrderGroup {
    // ... existing fields ...
    pub exchange_name: Option<String>,  // NEW: "woo", "binance", "bybit", "hyperliquid"
}
```

**Population points — every path that creates or rehydrates an OrderGroup:**

| Path | File | How exchange_name is set |
|------|------|-------------------------|
| Trade placement | `routes/trade_management.rs` | From the `exchange_name` on the selected exchange account (already resolved for routing) |
| Rehydration | `services/rehydration.rs` | From `exchange_accounts` join when loading groups from PostgreSQL |
| Shadow engine creation | `engine/src/shadow/order_group.rs` | Default `None` — shadow-only groups don't have an exchange |

**Supported exchange names** (from `types/exchange_names.rs`):

| Constant | Value | Notes |
|----------|-------|-------|
| `exchanges::HYPERLIQUID` | `"hyperliquid"` | Native SDK path (REL-02 handles journal separately) |
| `exchanges::BINANCE` | `"binance"` | CCXT sidecar — futures testnet verified |
| `exchanges::WOO` | `"woo"` | CCXT sidecar — production verified |
| `exchanges::BYBIT` | `"bybit"` | CCXT sidecar — new, same CCXT flow as WOO |
| `exchanges::OKX` | `"okx"` | CCXT sidecar — future support |

**Edge cases per exchange:**

| Exchange | Variation | Handling |
|----------|-----------|---------|
| Binance | Futures vs Spot accounts have same exchange_name | OK — `"binance"` for both. Journal doesn't distinguish sub-account type. |
| Binance | Testnet vs Mainnet | Same exchange_name `"binance"`. Network distinction is in the credentials, not the journal. |
| Bybit | Unified Trading Account (UTA) vs Standard | Same exchange_name `"bybit"`. CCXT abstracts this. |
| Bybit | Inverse perpetuals use different symbol format | CCXT normalises to standard format. Symbol stored on OrderGroup is already normalised. |
| WOO | WOO X vs WOO (legacy) | Same exchange_name `"woo"`. Both use same API endpoints via CCXT. |
| Hyperliquid | Entry fills go through FillDetector, closes go through REL-02 | Entry fills now emit correct `"hyperliquid"` name via group.exchange_name. Closing fills use REL-02 which hardcodes `"hyperliquid"`. Consistent. |

**In `emit_trade_closed()`:**

```rust
// Before
"exchange": "cex",

// After
"exchange": group.exchange_name.as_deref().unwrap_or("unknown"),
```

Using `"unknown"` as fallback instead of `"cex"` — makes it obvious if a code path fails to set the exchange name, rather than silently miscategorizing.

### Paved Roads

- `journal_service.rs:213-283` — Existing `upsert_daily_stats()` SQL (reuse verbatim)
- `journal_service.rs:167-191` — Existing draft notes merge pattern (reuse verbatim)
- `trade_event_writer.rs:163-177` — Existing batch loop structure to hook into
- `types/exchange_names.rs` — Single source of truth for exchange name constants
- `routes/trade_management.rs:606` — Already resolves exchange_account_id from request

### Files

- `crates/engine/src/shadow/order_group.rs` — Add `exchange_name: Option<String>` field + update constructors
- `crates/router/src/routes/trade_management.rs` — Set `exchange_name` on OrderGroup at creation
- `crates/router/src/services/rehydration.rs` — Populate `exchange_name` from DB during group reload
- `crates/router/src/services/trade_event_writer.rs` — Add daily stats upsert + draft notes merge after commit
- `crates/router/src/services/fill_detector.rs` — Use `group.exchange_name` in `emit_trade_closed()` payload

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `journal_daily_stats` row created/updated for every trade closed via CON-01 path
- [ ] Daily P&L chart shows data for trades closed after this fix
- [ ] Draft notes from `journal_trade_drafts` appear on the closed trade's journal entry
- [ ] Exchange name resolves to actual exchange (`"woo"`, `"binance"`, `"bybit"`, `"hyperliquid"`) — never `"cex"`
- [ ] Binance Futures trades show `"binance"` in journal
- [ ] Bybit trades show `"bybit"` in journal
- [ ] WOO trades show `"woo"` in journal
- [ ] Rehydrated groups retain correct exchange_name after service restart
- [ ] Existing CON-01 atomic transaction (trade_events + managed_positions + journal_trades) is not broken
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Daily stats upsert after commit** — If the process crashes between commit and upsert, daily stats are stale. Mitigation: daily stats are recomputable from `journal_trades` at any time. A periodic recompute job or manual `POST /journal/recompute-stats` endpoint covers this edge case.

2. **Draft notes merge race** — If the user is editing a draft note at the exact moment the trade closes, the DELETE + UPDATE could lose the in-flight edit. Mitigation: extremely narrow window (< 1ms), and the draft is still in the journal entry's notes field if the update succeeds.

---

## Completion Signal

This spec is complete when:
1. Daily P&L chart shows data for CON-01 path trades
2. Draft notes merge on trade close
3. All acceptance criteria pass
4. Code committed to master
