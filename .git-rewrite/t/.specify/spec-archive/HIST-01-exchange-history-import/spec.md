# Specification: Import Exchange Trade History

**Spec ID:** HIST-01-exchange-history-import
**Date:** 2026-03-26
**Status:** Draft
**Class:** Feature / Backend + Frontend
**Priority:** P0 — Without trade data, the Desk dashboard is empty for new users. This is the retention cliff identified in ARCH-01 D4. History import moves value delivery to onboarding step 3.
**Depends on:** DESK-01-unified-dashboard
**Series:** HIST-01 (Phase 1: Hyperliquid) then HIST-02 (Phase 2: CCXT — future spec)

---

## Problem Statement

A wallet-connected user who adds exchange API keys sees an empty dashboard. The only way to populate `journal_trades` today is by placing trades through the Testudo extension. This creates a four-step funnel where users must: connect wallet → add API keys → install extension → pair extension → place first trade before seeing any value. Every step is a drop-off point.

Exchanges already expose trade history via API. Hyperliquid's `userFillsByTime` endpoint returns fills with `closedPnl` natively — no position reconstruction required. The Rust backend already has the InfoProvider SDK integration (`ws_fills.rs`), the `JournalService::record_trade_close()` pipeline, and the `pg_queue` infrastructure for async jobs.

This spec adds a background import job that fetches 90 days of perp trade history from connected exchanges, normalizes closing fills into `TradeCloseEvent` structs, and routes them through the existing journal pipeline. Phase 1 covers Hyperliquid only (native SDK). Phase 2 (separate spec) extends to CEX exchanges via CCXT sidecar.

---

## User Stories

- **As a new user**, I want my past trades to appear on the dashboard when I connect an exchange, so that the Desk is immediately useful.
- **As a trader**, I want my imported trades to look identical to extension-placed trades, so that my analytics are unified regardless of how the trade was placed.
- **As a user with multiple exchanges**, I want each exchange's history imported independently, so that I can add exchanges at my own pace.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `source TEXT NOT NULL DEFAULT 'testudo'` column to `journal_trades`. Values: `testudo`, `import_hl`, `import_ccxt`. | High | Migration |
| FR-2 | Add `exchange_fill_id BIGINT` column to `journal_trades` (nullable). Stores HL `tid` for dedup. | High | Migration |
| FR-3 | Add partial unique index `idx_unique_import_fill ON journal_trades(user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL`. | High | Migration |
| FR-4 | Add `queue_imports` table to pg_queue schema with same structure as existing queue tables (`id BIGSERIAL`, `payload JSONB`, `status TEXT DEFAULT 'pending'`, `created_at`, `processed_at`). Add `TradeImports` variant to `QueueName`. | High | pg_queue |
| FR-5 | Add `POST /api/v1/trades/import` endpoint. Accepts `{ exchange_name: String }`. Validates user has active credentials for that exchange. Enqueues import job via pg_queue. Returns `{ job_id, status: "queued" }`. | High | Router |
| FR-6 | Auto-trigger: when exchange credentials are saved (POST to `/exchanges`), automatically enqueue an import job for that exchange. | High | Router |
| FR-7 | Import worker fetches fills from Hyperliquid via `InfoProvider::user_fills_by_time()`. Paginates in 2000-fill chunks across a 90-day window. Filters to perp fills only (excludes coins prefixed with `@`). Filters to closing fills only (`closedPnl != "0.0"`). | High | Worker |
| FR-8 | For each closing fill, construct a `TradeCloseEvent` and call `journal_service.record_trade_close()`. Map: `px` → `exit_price`, derive `entry_price` from `closedPnl/sz` adjusted for side, `closedPnl` → pre-fee P&L, `fee` → fees, `time` → `closed_at`, `tid` → `exchange_fill_id`, leverage defaults to 1. | High | Worker |
| FR-9 | Deduplication: if `(user_id, exchange, exchange_fill_id)` already exists, skip the fill. Handle via `ON CONFLICT DO NOTHING` or pre-check. | High | Worker |
| FR-10 | On job completion, send WebSocket notification to user via existing `ws-stream` infrastructure: `{ type: "import_complete", exchange: "hyperliquid", trades_imported: N }`. | Medium | ws-stream |
| FR-11 | On job failure, mark job as failed in pg_queue. User can retry via `POST /trades/import`. | Medium | Worker |
| FR-12 | Add `GET /api/v1/trades/import/status` endpoint. Returns list of import jobs for the user with status (`queued`, `processing`, `completed`, `failed`) and `trades_imported` count. | Medium | Router |

---

## Technical Implementation

### Phase 1: Hyperliquid Import

#### Import Job Payload

```rust
#[derive(Serialize, Deserialize)]
pub struct ImportJobPayload {
    pub user_id: Uuid,
    pub account_id: Uuid,       // exchange_accounts.id — load credentials at runtime
    pub exchange_name: String,   // "hyperliquid"
    pub start_time: i64,         // 90 days ago, unix ms
    pub end_time: i64,           // now, unix ms
}
```

Credentials are NOT stored in the job payload — the worker loads them from `exchange_account_repo.load_credentials(account_id, user_id)` at execution time.

#### HL Fill → TradeCloseEvent Mapping

| HL Fill Field | TradeCloseEvent Field | Derivation |
|---------------|----------------------|------------|
| `coin` | `symbol` | Map to `{COIN}_USDT` format |
| `dir` | `side` | Contains "Long" → `"LONG"`, contains "Short" → `"SHORT"` |
| `px` | `exit_price` | Direct (Decimal parse) |
| — | `entry_price` | Long: `exit - (closedPnl / sz)`, Short: `exit + (closedPnl / sz)` |
| `sz` | `quantity` | Direct (Decimal parse) |
| `closedPnl` | (pre-fee P&L) | Used for entry_price derivation; `record_trade_close` recomputes |
| `fee` | `fees` | Direct (Decimal parse) |
| `time` | `closed_at` | Unix ms → `DateTime<Utc>` |
| `time` | `opened_at` | Same as `closed_at` (we don't know actual open time) |
| `tid` | `exchange_fill_id` | Stored for dedup |
| — | `leverage` | Default `1` |
| — | `trade_group_id` | `None` |
| — | `stop_price` | `None` |
| — | `target_price` | `None` |
| — | `risk_amount` | `None` |

#### Pagination Strategy

HL returns max 2000 fills per request. For active traders with >2000 fills in 90 days:

```rust
let mut cursor = start_time;
loop {
    let fills = info.user_fills_by_time(address, cursor, Some(end_time), None).await?;
    if fills.is_empty() { break; }

    // Process closing fills
    for fill in fills.iter().filter(|f| f.closed_pnl != "0.0" && !f.coin.starts_with("@")) {
        // map to TradeCloseEvent, call record_trade_close
    }

    // Advance cursor past last fill
    cursor = fills.last().unwrap().time + 1;
    if fills.len() < 2000 { break; }
}
```

#### Worker Loop

Runs as a Tokio task alongside existing queue workers. Polls `queue_imports` via `pop()`, processes one job at a time.

```rust
async fn import_worker_loop(
    queue: QueueRepository,
    exchange_repo: ExchangeAccountRepository,
    journal_service: JournalService,
) {
    loop {
        match queue.pop::<ImportJobPayload>(QueueName::TradeImports).await {
            Ok(Some(job)) => {
                match process_import(job.payload, &exchange_repo, &journal_service).await {
                    Ok(count) => {
                        queue.complete(QueueName::TradeImports, job.id).await.ok();
                        // Send WS notification with count
                    }
                    Err(e) => {
                        tracing::error!("import job {} failed: {e}", job.id);
                        queue.fail(QueueName::TradeImports, job.id).await.ok();
                    }
                }
            }
            Ok(None) => {
                // No jobs — wait for NOTIFY
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                tracing::error!("import queue pop error: {e}");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
```

### Phase 2: CCXT (Future — HIST-02)

Phase 2 adds:
- `POST /trades` endpoint to CCXT sidecar calling `exchange.fetchMyTrades()`
- `fetch_trades()` method on `CexClient`
- `import_ccxt` source variant
- Same pg_queue pattern, different fill mapping

Not in scope for this spec.

### Files

**New files:**
- `crates/router/src/services/import_worker.rs` — Import worker loop + HL fill processor
- `crates/router/src/routes/imports.rs` — `/trades/import` and `/trades/import/status` endpoints
- `crates/sqlx_postgres/migrations/YYYYMMDD_add_import_fields.up.sql` — Schema migration
- `crates/sqlx_postgres/migrations/YYYYMMDD_queue_imports.up.sql` — Import queue table

**Modified files:**
- `crates/pg_queue/src/lib.rs` — Add `TradeImports` to `QueueName` enum
- `crates/pg_queue/src/queue.rs` — Handle new queue table
- `crates/router/src/routes/mod.rs` — Register import routes
- `crates/router/src/routes/exchanges.rs` — Auto-trigger import on credential save
- `crates/router/src/main.rs` — Spawn import worker task
- `crates/router/src/services/journal_service.rs` — Add `source` and `exchange_fill_id` to insert query
- `crates/router/src/models/journal.rs` — Add fields to `JournalTrade` model

### Dependencies Added

None — Hyperliquid SDK (`hyperliquid_sdk_rs`) and pg_queue are already dependencies.

---

## Acceptance Criteria

- [ ] Migration adds `source` and `exchange_fill_id` columns to `journal_trades`
- [ ] Unique partial index prevents duplicate imports
- [ ] `POST /trades/import` enqueues a job and returns job ID
- [ ] Import worker fetches HL fills, filters to closing perp fills, inserts via `record_trade_close`
- [ ] Re-importing the same exchange is idempotent (no duplicate rows)
- [ ] Auto-import triggers when exchange credentials are saved
- [ ] WebSocket notification sent on import completion
- [ ] `GET /trades/import/status` returns job history with counts
- [ ] Imported trades appear identically in dashboard analytics
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **HL API rate limiting** — Paginating 90 days of fills for active traders may hit rate limits. Mitigation: Add backoff between pagination requests (100ms delay between calls).
2. **Entry price derivation precision** — `entry = exit - (closedPnl / sz)` may have rounding differences vs actual entry. Mitigation: Acceptable for analytics — the P&L itself is exact from `closedPnl`.
3. **Large import volumes** — A very active trader could have thousands of fills. Mitigation: Batch inserts, progress tracking via WS, timeout protection on worker.
4. **Opened_at unknown** — HL closing fills don't include when the position was opened. Mitigation: Set `opened_at = closed_at`. Duration will show 0s for imported trades. Acceptable tradeoff — the alternative (correlating opens with closes) is the position reconstruction we explicitly avoided.

---

## Completion Signal

This spec is complete when:
1. A user can connect a Hyperliquid account and see their past 90 days of trades on the Desk dashboard automatically
2. Re-importing produces no duplicates
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
