# Implementation Plan

> Last updated: 2026-04-20
> Current spec: HIST-03-import-dedup
> Phase: BUILD in progress — T1 complete

---

## Active Spec: HIST-03-import-dedup

### Gap Analysis

**Backend (`testudo-exchange/crates/router/`):**

1. **Partial unique index already exists (HIST-01)** — `crates/sqlx_postgres/migrations/20260326000000_add_import_fields.up.sql:8-10`:
   ```sql
   CREATE UNIQUE INDEX idx_unique_import_fill
       ON journal_trades(user_id, exchange, exchange_fill_id)
       WHERE exchange_fill_id IS NOT NULL;
   ```
   No schema change needed. `ON CONFLICT` target predicate must match this exactly, including the `WHERE` qualifier. Discovery #1.

2. **Three INSERT-into-journal_trades sites today, only TWO are relevant for this spec:**
   - `JournalService::record_trade_close()` at `services/journal_service.rs:184-224` — **used by all three import callers**: `import_worker.rs:292` (HL batch), `import_worker.rs:391` (CEX pnl fills), `import_worker.rs:458` (CEX reconstructed). Also used by `ws_fills.rs:520` (LIVE HL 30s poll). This is the `ON CONFLICT + RecordOutcome` target per FR-1/FR-2.
   - `TradeEventWriter::insert_journal_trade()` at `services/trade_event_writer.rs:324-410` — the CON-01 transaction-atomic path for LIVE CEX trade closes (triggered by `FillDetector::emit_trade_closed` via `trade_event_tx` channel). Uses its own SELECT-then-INSERT idempotency keyed on `trade_group_id`. Live trades land here with `exchange_fill_id = None` → partial index never applies → no duplication risk. But spec §Files lists it as MODIFIED "if it writes closes" — it does. Defensive ON CONFLICT mirrored here keeps the partial-index contract uniform. Discovery #2.
   - (There is no third direct writer — `fill_detector.rs:emit_trade_closed` emits a channel event; it does NOT write SQL. Spec §Files lists `fill_detector.rs` as "MODIFIED. Minimal: update to the new return type" — this is incorrect relative to ground truth; fill_detector doesn't call `record_trade_close`. Document as spec deviation in T1.)

3. **Current idempotency mechanism is error-based, not `ON CONFLICT`:**
   - `import_worker.rs:295-304, 394-401, 461-468` — catches `sqlx::Error`, string-matches `idx_unique_import_fill` or `duplicate key` on the message, and swallows as "skipped". Works WHEN the partial index actually matches; silently inserts a duplicate WHEN the key differs. This is exactly the failure mode the spec is observing.
   - `record_trade_close` uses `fetch_one` on INSERT — with `ON CONFLICT DO NOTHING`, conflict returns no row; must switch to `fetch_optional`. Discovery #3.

4. **`exchange_fill_id` fallback is the highest-likelihood root cause (spec hypothesis #2):**
   - `import_worker.rs:386` (CEX pnl fills): `exchange_fill_id: Some(fill.id.parse::<i64>().unwrap_or(fill.timestamp as i64))`
   - `import_worker.rs:533` (CEX reconstructed trades via `reconstruct_positions`): same pattern on `last_fill_id: fill.id.parse::<i64>().unwrap_or(fill.timestamp as i64)`, stored into `ReconstructedTrade.last_fill_id: i64`, then stamped onto `exchange_fill_id` at line 453.
   - HL path is safe: `hl_fill_journal.rs:93` uses `Some(fill.tid as i64)` where `tid` is a stable numeric `u64`. No fallback.
   - Concern: if Bybit v5 trade-history returns a UUID-shaped `execId` OR varies between endpoint versions (some fills carry parseable IDs, others fall back to timestamp), the same fill could land with two different `exchange_fill_id` values across runs, defeating the partial index. FR-5 removes the fallback. Discovery #4.

5. **`exchange` column casing is mostly safe but not enforced at the journal-write boundary:**
   - `common_utils/src/models/exchange_account.rs:125` normalizes `exchange_name` to lowercase on account save.
   - `import_worker.rs:170-173` matches `payload.exchange_name.as_str()` against `"hyperliquid"` — case-sensitive match that would route a mis-cased "Hyperliquid" into the CEX fallback branch. Since account saves enforce lowercase, this is safe today. But downstream stamping at INSERT time (`import_worker.rs:370`: `exchange: payload.exchange_name.clone()`) passes through unmodified. FR-4 canonicalizes at the journal-write boundary to belt-and-braces the index contract. Discovery #5.
   - Live path stamps via `emit_trade_closed` at `fill_detector.rs:635`: `group.exchange_name.as_deref().unwrap_or("unknown")`. `OrderGroup.exchange_name` is set via `ConfigureGroup` in `trade_management.rs::create_trade` from a route-level lookup; current lookup path also normalizes. Safe, but same belt-and-braces argument.

6. **`record_trade_close` has a pre-INSERT idempotency check by `trade_group_id`** (`journal_service.rs:158-178`) — live trades (with `trade_group_id = Some(...)`) will hit this SELECT and short-circuit. Imports always have `trade_group_id = None`, so they bypass this entirely. ON CONFLICT is the import path's ONLY dedup guard. Discovery #6.

7. **No existing `canonical_exchange_name()` helper** — `.to_lowercase()` is inlined at 3 callsites in `sqlx_postgres/` and `common_utils/`. Per FR-4 + spec §Paved Roads, add a single free function (trivial 1-line) in `common_utils::models::exchange_account` OR next to the exchange-name logic — no competing helpers. Discovery #7.

8. **`TradeCloseEvent` struct** (`journal_service.rs:16-39`) carries `source: Option<String>` and `exchange_fill_id: Option<i64>`. No struct change needed — only the INSERT SQL and caller handling shift. Discovery #8.

9. **Tests:** Existing `journal_service.rs` test module covers pure `compute_derived_fields` + `upsert_auto_tag` — all in-module `#[cfg(test)]`, no DB. Router's crate has **no `tests/` directory** (it's a binary-only crate per AGENTS.md 2026-04-17 note) — integration tests for the DB-bound idempotency path must live inside the module via `#[cfg(test)] mod tests` with a pool fixture, OR via an ignored test similar to the existing `sqlx_postgres` one-offs. Easiest path: submodule `services/tests/journal_service_idempotency.rs` referenced by `services/mod.rs` under `#[cfg(test)]`. Discovery #9.

10. **`bun run build` vs `bun run typecheck`**: Per memory `feedback_prod_defaults.md`, NEVER run `bun run build` in `testudo-extension/` during verification — use `typecheck`. Spec acceptance criterion "Extension bun run typecheck passes" matches this rule. No wire-shape changes expected from this spec (it's a pure backend refactor), but verify anyway. Discovery #10.

---

### Design Decisions

1. **CP-0 "diagnostic" cannot run against the user's production DB from Vox.** Vox has no DB credentials and should not read `.env`. Translate CP-0 into **static-analysis evidence**: enumerate the code paths that could produce the four hypotheses, identify which one the fix addresses, and document in the T1 commit message. The user runs the `SELECT COUNT(*) GROUP BY` query manually post-deploy to confirm. Concretely: hypotheses #1 (missing ON CONFLICT) + #2 (timestamp fallback) are both structurally present in the current code and both are fixed by this spec; #3 (exchange casing drift) is defended by FR-4 as belt-and-braces; #4 is contradicted by the presence of idempotency checks so not the issue.

2. **ON CONFLICT target predicate must match the partial index verbatim.** Syntax: `ON CONFLICT (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL DO NOTHING`. The `WHERE` qualifier is load-bearing — without it, Postgres cannot resolve which partial index to target and raises `ERROR: there is no unique or exclusion constraint matching the ON CONFLICT specification`. Build-time test in T7 asserts a fresh INSERT into a duplicate row resolves without error.

3. **`RecordOutcome::{Inserted(JournalTrade), SkippedDuplicate}` replaces `JournalTrade` as the return type.** Use `fetch_optional` → `Some(row)` maps to `Inserted`, `None` maps to `SkippedDuplicate`. Existing test in `journal_service.rs` tests pure helper functions, not `record_trade_close` itself — so return-type change only affects call sites: all 3 in `import_worker.rs` + 1 in `ws_fills.rs`. Update mechanical match on return type.

4. **Live path callers (`ws_fills.rs`) are updated to handle `RecordOutcome` but ALWAYS see `Inserted`** — HL live polls stamp `exchange_fill_id = Some(tid)`, and TIDs don't collide. If the LIVE path ever DID hit a partial-index conflict (it won't in practice), counting as "Inserted" vs "SkippedDuplicate" is a logging distinction only — the trade itself is not lost either way because the earlier run already journaled it.

5. **TradeEventWriter's atomic INSERT gets defensive ON CONFLICT too** (T3). Mirror the pattern — `ON CONFLICT (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL DO NOTHING`. Live trades carry `exchange_fill_id = None` → WHERE predicate excludes them → ON CONFLICT becomes a structural no-op. Zero behavior change in practice, but prevents any future CEX live-path that stamps fill IDs from regressing. Uses `execute()` (not `fetch_one`/`fetch_optional`) since TradeEventWriter doesn't consume the RETURNING row; no fetch_* change needed here.

6. **`canonical_exchange_name(name: &str) -> String`** lands in `common_utils/src/models/exchange_account.rs` right below the existing `.to_lowercase()` call at line 125. One-line helper: `name.trim().to_lowercase()`. Import and apply at `record_trade_close` (on the `event.exchange` bind) and at `TradeEventWriter::insert_journal_trade` (on the `close_event.exchange` bind). Idempotent — imports that were already lowercase become no-ops.

7. **Remove `.unwrap_or(fill.timestamp as i64)` in CEX import paths (FR-5).** Change to `fill.id.parse::<i64>().ok()`. If parse fails: log WARN with full fill payload (`fill.id` string, `fill.symbol`, `fill.timestamp`), skip (`continue`). Increment a new `unparseable_fill_ids` counter under `errors`. Two call sites: `import_worker.rs:386` (pnl fills) and `import_worker.rs:533` (reconstruction path via `last_fill_id`). This removes the fallback entirely — preferable to a synthetic key that can drift between runs.

8. **`trades_skipped_duplicate` counter added to `ImportResult`** (FR-3). Current `ImportResult`:
   ```rust
   pub struct ImportResult { pub trades_imported: u64, pub trades_skipped: u64, pub errors: u64 }
   ```
   The existing `trades_skipped` field was used for "non-closing fills" AND "duplicate via error-string-match". Split them: keep `trades_skipped` for "fills we skipped for structural reasons (non-closing, spot, zero-qty)", add `trades_skipped_duplicate: u64` for partial-index hits. Only `Inserted` bumps `trades_imported`; only `SkippedDuplicate` bumps `trades_skipped_duplicate`. WebSocket notification payload gets the new field too (for the extension to surface accurate counts).

9. **Integration test** (FR-6 / CP-4) uses a real Postgres pool via the existing `sqlx::test` attribute pattern or an ignored manual test. Pattern: acquire a pool, insert a minimal user, construct a synthetic `TradeCloseEvent { exchange_fill_id: Some(42), ... }`, call `record_trade_close` twice, assert:
   - First call returns `Inserted(trade)` with a fresh ID.
   - Second call returns `SkippedDuplicate`.
   - `COUNT(*) FROM journal_trades WHERE exchange_fill_id = 42` = 1.
   
   The test lives inline in `services/journal_service.rs` as a `#[sqlx::test]` module, OR as a `#[cfg(test)] #[ignore]` harness if `#[sqlx::test]` isn't already wired in this crate. Check during T7 — if `sqlx::test` isn't used anywhere in router, add it as a dev-dep feature and gate with `#[ignore]` fallback.

10. **No schema migration.** HIST-01's partial index is reused verbatim.

11. **Post-deploy cleanup SQL lives in the T8 commit-message body, not as a migration.** Spec Risk #4 calls this out — the spec prevents FUTURE duplicates, doesn't clean existing rows. Exact SQL shipped in commit body:
    ```sql
    DELETE FROM journal_trades a
      USING journal_trades b
      WHERE a.id > b.id
        AND a.user_id = b.user_id
        AND a.exchange = b.exchange
        AND a.exchange_fill_id = b.exchange_fill_id
        AND a.exchange_fill_id IS NOT NULL;
    ```
    User runs manually post-deploy against their prod DB.

12. **Spec-file deviations documented in T1 commit:**
    - CP-0 is static-analysis evidence, not a live DB query (Vox lacks credentials).
    - `fill_detector.rs` is NOT modified — it doesn't call `record_trade_close` directly; it emits via channel. Spec §Files is inaccurate.
    - Live `TradeEventWriter` path gets defensive ON CONFLICT but no `RecordOutcome` plumbing (it doesn't consume the return — uses `execute()`).

---

### Vertical Checkpoint Structure (from spec §Technical Implementation)

| CP | Goal | Tasks |
|----|------|-------|
| CP-0 | Static-analysis diagnostic, document hypotheses fired. | T1 |
| CP-1 | ON CONFLICT DO NOTHING + RecordOutcome enum (JournalService). Live callers match the new type. | T2, T3 |
| CP-2 | Importer counters + canonical_exchange_name() applied at all journal-write INSERT sites. | T4, T5 |
| CP-3 | Remove timestamp fallback on `exchange_fill_id`; log+skip unparseable IDs. | T6 |
| CP-4 | Integration test for idempotency + partial-index contract. | T7 |
| Final | Verification + archival + post-deploy cleanup SQL in commit body. | T8 |

---

### Parallel Track Detection

```
T1 (CP-0 static-analysis diagnostic — documentation only)
    │
T2 (CP-1a: RecordOutcome + ON CONFLICT in JournalService::record_trade_close)
    │
T3 (CP-1b: ON CONFLICT mirror in TradeEventWriter::insert_journal_trade [defensive])
    │
T4 (CP-2a: canonical_exchange_name() helper + apply at INSERT sites)
    │
T5 (CP-2b: ImportResult.trades_skipped_duplicate + consume RecordOutcome in all 4 callers)
    │
T6 (CP-3: remove timestamp fallback in import_worker CEX paths)
    │
T7 (CP-4: integration test)
    │
T8 (verify + archive)
```

Strictly sequential — each task either adds or depends on the preceding type/function signature. Single-agent BUILD.

---

## Tasks

### T1: CP-0 static-analysis diagnostic + spec-deviation documentation — `complete`

**Scope:** No code changes. Produce a short markdown report (embedded in the commit message of T2, not a standalone file) that documents:

1. Which of the spec's four hypotheses are structurally present in the code:
   - **#1 (missing ON CONFLICT):** YES — `journal_service.rs:184-224` has bare INSERT RETURNING. Dedup relies on post-INSERT error-string match in `import_worker.rs:295-304, 394-401, 461-468`.
   - **#2 (timestamp fallback on `exchange_fill_id`):** YES — `import_worker.rs:386, 533` both fall back to `fill.timestamp as i64`.
   - **#3 (exchange casing drift):** Low risk but NOT defended at INSERT boundary. Account-save path normalizes; journal-write path does not.
   - **#4 (record_trade_close shared live/import):** Partially incorrect — live path uses `TradeEventWriter::insert_journal_trade` (separate INSERT), NOT `record_trade_close`. Only `ws_fills.rs` HL live poll shares. Live CEX path has its own dedup via `trade_group_id` SELECT.

2. Spec §Files deviations (document in T1 commit body):
   - `fill_detector.rs` NOT modified (doesn't call `record_trade_close`).
   - `TradeEventWriter` gets defensive ON CONFLICT only; no `RecordOutcome` (it uses `execute`, not `fetch_*`).
   - `cex_history.rs` NOT modified (pure HTTP fetcher; no journal writes).

3. One-shot cleanup SQL for existing duplicates (to be included in T8 commit body):
   ```sql
   DELETE FROM journal_trades a
     USING journal_trades b
     WHERE a.id > b.id
       AND a.user_id = b.user_id
       AND a.exchange = b.exchange
       AND a.exchange_fill_id = b.exchange_fill_id
       AND a.exchange_fill_id IS NOT NULL;
   ```

**Files:** None modified. T1 exists as a planning artifact; its conclusions are folded into T2's commit message.

**Validate:** `cargo check -p router` — sanity-only, baseline.

**Acceptance:**
- T2 commit body references CP-0 findings.
- Spec deviations logged in Discoveries (this plan).

**Verified findings (2026-04-20):**
- **Hypothesis #1 present:** `journal_service.rs:184-224` bare `INSERT ... RETURNING` via `fetch_one`. No `ON CONFLICT`. Dedup relies on error-string match in callers.
- **Hypothesis #2 present:** `import_worker.rs:386` (pnl fills) + `import_worker.rs:533` (reconstruction) both `.unwrap_or(fill.timestamp as i64)` on unparseable `fill.id`.
- **Hypothesis #3 undefended at journal boundary:** accounts lowercase on save, but `import_worker.rs:370` passes `payload.exchange_name.clone()` through verbatim to INSERT.
- **Hypothesis #4 partially wrong:** live CEX path uses `TradeEventWriter::insert_journal_trade` with its own `trade_group_id` SELECT — decoupled from imports. Only HL live poll (`ws_fills.rs:520`) shares `record_trade_close` with imports.
- **5th error-string site found:** `ws_fills.rs:572-573` also string-matches `idx_unique_import_fill|duplicate key`. T5 must update this site too — spec plan originally listed only 4 callers.
- **Partial index is HIST-01 verbatim:** `sqlx_postgres/migrations/20260326000000_add_import_fields.up.sql:8-10` — `ON CONFLICT (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL DO NOTHING` is the required target predicate.

---

### T2: `RecordOutcome` enum + `ON CONFLICT DO NOTHING` in `JournalService::record_trade_close` — `pending`

**Scope:** CP-1a FR-1 + FR-2. The core fix. Every import INSERT becomes structurally idempotent.

**Files:**
- `testudo-exchange/crates/router/src/services/journal_service.rs` — MODIFIED:
  - Add public enum above `JournalService`:
    ```rust
    /// Result of attempting to persist a trade close. `Inserted` for a fresh write,
    /// `SkippedDuplicate` when the partial unique index (HIST-01) catches a re-import.
    #[derive(Debug)]
    pub enum RecordOutcome {
        Inserted(JournalTrade),
        SkippedDuplicate,
    }
    ```
  - Change `record_trade_close` return type from `Result<JournalTrade, sqlx::Error>` to `Result<RecordOutcome, sqlx::Error>`.
  - Existing `trade_group_id` idempotency check (L158-178) stays — on hit, return `Ok(RecordOutcome::Inserted(trade))` (preserves current behavior where "already recorded" == "effectively inserted for our purposes"). Alternative: return `SkippedDuplicate` here too. Chose `Inserted(trade)` because the caller may still want the row back (live path) and live doesn't use this counter anyway — semantic of "this trade is in the journal" is preserved.
  - Change INSERT clause:
    ```sql
    INSERT INTO journal_trades (...)
    VALUES (...)
    ON CONFLICT (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL
    DO NOTHING
    RETURNING id, user_id, exchange, ...
    ```
    (full RETURNING column list preserved).
  - Change `.fetch_one(&self.pool)` to `.fetch_optional(&self.pool)`. Match:
    ```rust
    let trade: JournalTrade = match sqlx::query_as::<_, JournalTrade>(...).fetch_optional(&self.pool).await? {
        Some(t) => t,
        None => {
            tracing::debug!(
                user_id = %event.user_id,
                exchange = %event.exchange,
                exchange_fill_id = ?event.exchange_fill_id,
                "Journal: duplicate import skipped by partial unique index"
            );
            return Ok(RecordOutcome::SkippedDuplicate);
        }
    };
    ```
  - Post-INSERT side effects (draft-notes merge L226-250, daily stats L253-259, auto-tag L263-271, info log L273-278) only run on the `Some(t)` branch.
  - Final return: `Ok(RecordOutcome::Inserted(trade))`.

**Validate:**
- `cd testudo-exchange && cargo check --all-targets` — will fail on caller sites (import_worker, ws_fills). Those are fixed in T5. Temporarily acceptable in T2 if T5 lands in the same session; otherwise inline-fix the 4 call sites (match `Ok(_)` → `result.trades_imported += 1` for now, ignore `SkippedDuplicate` as if it were the error-string path). **Preferred:** land T2 and T5 together as a single atomic commit to avoid an intermediate broken build. If splitting is required, T2's call-site changes are minimal string-replace fixes.

**Acceptance:**
- `RecordOutcome` enum exported from `services::journal_service`.
- `record_trade_close` returns `Result<RecordOutcome, sqlx::Error>`.
- SQL `ON CONFLICT` target predicate matches partial index verbatim (including `WHERE exchange_fill_id IS NOT NULL`).
- Live idempotency path (trade_group_id match) preserved.

---

### T3: Defensive `ON CONFLICT` mirror in `TradeEventWriter::insert_journal_trade` — `pending`

**Scope:** CP-1b. Belt-and-braces. Live path already dedupes via `trade_group_id` SELECT; this ensures the partial-index contract is uniform across all journal writers.

**Files:**
- `testudo-exchange/crates/router/src/services/trade_event_writer.rs` — MODIFIED:
  - Change INSERT at L366-374:
    ```sql
    INSERT INTO journal_trades (...)
    VALUES (...)
    ON CONFLICT (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL
    DO NOTHING
    ```
  - Keep `.execute(&mut **tx)` (no RETURNING consumption). Live trades have `exchange_fill_id = None`, so the WHERE predicate excludes them → ON CONFLICT is unreachable in practice. Zero behavioral change.
  - No struct or return-type changes.

**Validate:**
- `cd testudo-exchange && cargo check --all-targets` — no caller changes needed.

**Acceptance:**
- SQL ON CONFLICT target matches HIST-01 partial index.
- Existing trade_group_id idempotency SELECT (L335-348) retained.
- Live trade round-trip tests (existing suite) pass unchanged.

---

### T4: `canonical_exchange_name()` helper + apply at INSERT sites — `pending`

**Scope:** CP-2a FR-4. One-line helper, two application sites.

**Files:**
- `testudo-exchange/crates/common_utils/src/models/exchange_account.rs` — MODIFIED:
  - Add public free function near the existing `.to_lowercase()` call at L125:
    ```rust
    /// Normalize an exchange name to its canonical form for storage/index keys.
    /// HIST-03: single source of truth; prevents casing drift from defeating the
    /// partial unique index `idx_unique_import_fill(user_id, exchange, exchange_fill_id)`.
    pub fn canonical_exchange_name(name: &str) -> String {
        name.trim().to_lowercase()
    }
    ```

- `testudo-exchange/crates/router/src/services/journal_service.rs` — MODIFIED:
  - Import `use common_utils::models::exchange_account::canonical_exchange_name;`
  - In `record_trade_close`, before the INSERT: `let exchange_canon = canonical_exchange_name(&event.exchange);` and bind `&exchange_canon` instead of `&event.exchange`.

- `testudo-exchange/crates/router/src/services/trade_event_writer.rs` — MODIFIED:
  - Same import + same bind swap on `close_event.exchange`.

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test -p common_utils exchange_account` — existing exchange_account tests pass; new helper is trivial, doesn't need its own test (covered by `test_should_normalize_exchange_names` pattern implicitly — but add a 1-case unit test for paranoia:
  ```rust
  #[test]
  fn canonical_exchange_name_normalizes() {
      assert_eq!(canonical_exchange_name("Bybit"), "bybit");
      assert_eq!(canonical_exchange_name("  HYPERLIQUID  "), "hyperliquid");
      assert_eq!(canonical_exchange_name("woo"), "woo");
  }
  ```

**Acceptance:**
- Helper callable from router crate.
- Both journal INSERT sites stamp canonical form.
- Unit test passes.
- Clippy clean.

---

### T5: `ImportResult.trades_skipped_duplicate` + consume `RecordOutcome` in all callers — `pending`

**Scope:** CP-2b FR-3. Wire the new return type through the 4 call sites; split counters.

**Files:**
- `testudo-exchange/crates/router/src/services/import_worker.rs` — MODIFIED:
  - Add field to `ImportResult` (L44-48):
    ```rust
    pub struct ImportResult {
        pub trades_imported: u64,
        pub trades_skipped: u64,                    // non-closing / spot / zero-qty
        pub trades_skipped_duplicate: u64,          // HIST-03: partial index collision
        pub errors: u64,
    }
    ```
  - `process_hl_fill` (L280-306) — replace error-string match with `RecordOutcome` match:
    ```rust
    match self.journal.record_trade_close(event).await {
        Ok(RecordOutcome::Inserted(_)) => Ok(true),
        Ok(RecordOutcome::SkippedDuplicate) => Ok(false), // caller maps Ok(false) to trades_skipped_duplicate
        Err(e) => Err(ImportError::Database(e.to_string())),
    }
    ```
    Note: `Ok(false)` currently maps to `trades_skipped` at the caller. Change the caller (L246-258) to split on a new `Ok(ProcessOutcome::Duplicate)` vs `Ok(ProcessOutcome::StructuralSkip)`. Cleanest approach: promote `process_hl_fill`'s return type from `Result<bool, _>` to `Result<ProcessOutcome, _>` where:
    ```rust
    enum ProcessOutcome { Imported, Duplicate, StructuralSkip }
    ```
    Update `process_hl_fill` to return `ProcessOutcome::StructuralSkip` when `build_trade_close_event` returns `None` (non-closing/spot/zero-qty), `ProcessOutcome::Imported` on `Inserted`, `ProcessOutcome::Duplicate` on `SkippedDuplicate`. Caller then matches and bumps the right counter.
  - `import_cex` pnl-fill loop (L351-403) — same pattern: `Ok(RecordOutcome::Inserted(_)) => result.trades_imported += 1;`, `Ok(RecordOutcome::SkippedDuplicate) => result.trades_skipped_duplicate += 1;`, error branch removes the duplicate-by-string-match fallback (no longer needed — conflict is silent now).
  - `record_reconstructed_trade` (L429-470) — same pattern: return `ProcessOutcome` enum or propagate outcome directly. Caller in L408-423 splits counters accordingly.
  - `notify_user` (L143-167) — add `trades_skipped_duplicate` to the payload:
    ```json
    {"trades_imported": ..., "trades_skipped": ..., "trades_skipped_duplicate": ...}
    ```

- `testudo-exchange/crates/router/src/services/hyperliquid/ws_fills.rs` — MODIFIED:
  - L520-528: match on `RecordOutcome`:
    ```rust
    match journal.record_trade_close(event).await {
        Ok(RecordOutcome::Inserted(trade)) => {
            journal_writes += 1;
            tracing::info!(trade_id = %trade.id, ..., "REL-02: HL closing fill written to journal");
            // reconcile_group(...) continues as today
        }
        Ok(RecordOutcome::SkippedDuplicate) => {
            tracing::debug!(tid = fill.tid, "REL-02: HL fill already journaled, skipping");
            // Skip reconcile_group — the earlier run already handled it.
        }
        Err(e) => { /* existing error path */ }
    }
    ```

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test -p router` — all callers compile; test counter arithmetic if easy (add a small unit test constructing `ImportResult` defaults).

**Acceptance:**
- `ImportResult` has 4 counter fields; all increment on distinct events.
- All 4 call sites handle `RecordOutcome` exhaustively.
- WebSocket notification carries `trades_skipped_duplicate`.
- No "duplicate key" error-string string-matching left in the codebase (grep: `grep -rn "idx_unique_import_fill\|duplicate key" crates/router/src/services/`).

---

### T6: Remove timestamp fallback on `exchange_fill_id` in CEX import paths — `pending`

**Scope:** CP-3 FR-5. Structural fix for spec hypothesis #2.

**Files:**
- `testudo-exchange/crates/router/src/services/import_worker.rs` — MODIFIED:
  - `import_cex` pnl-fill loop, around L383-389 (constructing `TradeCloseEvent`):
    ```rust
    let Some(exchange_fill_id) = fill.id.parse::<i64>().ok() else {
        tracing::warn!(
            fill_id = %fill.id,
            symbol = %fill.symbol,
            timestamp = fill.timestamp,
            exchange = %payload.exchange_name,
            "HIST-03: unparseable fill ID — skipping fill to avoid synthetic timestamp-based key"
        );
        result.errors += 1;
        continue;
    };
    // ...
    exchange_fill_id: Some(exchange_fill_id),
    ```
  - `reconstruct_positions` (L489-583) — at the two `last_fill_id: fill.id.parse::<i64>().unwrap_or(fill.timestamp as i64)` sites (L533 emitting completed trade + fallback). Need to decide: skip the entire completed trade if the closing fill's ID is unparseable? Yes — an unparseable ID means we cannot reliably dedup this trade. Log WARN + skip (don't push to `completed`). Thread the Option<i64> through `ReconstructedTrade` OR filter at emit. Simpler: change `last_fill_id: i64` to `last_fill_id: Option<i64>`, skip when None at the caller (`record_reconstructed_trade`).
  - Alternative: keep `ReconstructedTrade.last_fill_id: i64`, but at the emit site (L524-534), use `if let Some(id) = fill.id.parse::<i64>().ok() { completed.push(ReconstructedTrade { ..., last_fill_id: id }); } else { warn!; }`. Cleaner — trade is either fully built or skipped at the boundary.

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test -p router`.

**Acceptance:**
- Zero callsites of `fill.timestamp as i64` as fallback for `exchange_fill_id` (grep confirms).
- Unparseable fill IDs produce a WARN log + skip (`result.errors += 1`); the fill does not enter `journal_trades`.
- Bybit's historical "all IDs numeric" case: behavior unchanged (all fills import as before).

---

### T7: Integration test — idempotent re-import — `pending`

**Scope:** CP-4 FR-6. Structural guarantee of idempotency.

**Files:**
- `testudo-exchange/crates/router/src/services/journal_service.rs` — MODIFIED (inline test module):
  - Add a `#[cfg(test)] mod idempotency_tests { ... }` module OR extend existing `#[cfg(test)] mod tests`. Test functions use `#[sqlx::test]` if the attribute is available in this crate (check Cargo.toml for `sqlx = { features = ["macros", ...] }` + a test helper); otherwise fall back to `#[tokio::test] #[ignore]` with a manual pool creation from `DATABASE_URL` env var, matching how other ignored integration tests work in this crate (if any).
  - Test cases:
    1. **`fresh_insert_returns_inserted`**: Create user row, construct `TradeCloseEvent { exchange_fill_id: Some(42), exchange: "bybit", ... }`, call `record_trade_close`, assert `Ok(RecordOutcome::Inserted(trade))` with `trade.exchange_fill_id == Some(42)`.
    2. **`second_insert_returns_skipped_duplicate`**: Same setup as #1, call twice. Second call returns `Ok(RecordOutcome::SkippedDuplicate)`. Assert `SELECT COUNT(*) FROM journal_trades WHERE user_id = ? AND exchange = 'bybit' AND exchange_fill_id = 42` == 1.
    3. **`null_exchange_fill_id_not_affected_by_partial_index`**: Construct event with `exchange_fill_id: None` (live trade shape), call twice with **different** `trade_group_id` each time (else the trade_group_id SELECT short-circuit catches it first). Both return `Inserted`. Partial index's WHERE clause excludes these rows.
    4. **`canonical_exchange_applied`**: Construct event with `exchange: "Bybit"` (mixed case). After insert, query `SELECT exchange FROM journal_trades` — assert it's `"bybit"` (lowercased by canonical_exchange_name). Confirms T4 landed.

**Validate:**
- `cd testudo-exchange && cargo test -p router journal_service` — all 4 idempotency tests pass. (Non-ignored if `sqlx::test` wiring is present; otherwise gated behind `--ignored`.)
- If `sqlx::test` is NOT wired: provide a `.env.example`-level DATABASE_URL comment in the test file so the user can `cargo test -- --ignored` post-deploy. Do NOT read a real `.env`.

**Acceptance:**
- 4 test cases green.
- Integration test physically verifies the partial-index predicate works.
- FR-6 acceptance criterion (second run reports `trades_imported = 0, trades_skipped_duplicate = N`) structurally guaranteed by the test.

---

### T8: Final verification + spec archival — `pending`

**Scope:** Completion Protocol.

**Verifications:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test` — all tests pass, 3 pre-existing clippy warnings stable (actor.rs, cex_client.rs, evaluator.rs). New tests from T4 + T7 added to baseline.
- `cd testudo-extension && bun run typecheck` — no wire-shape changes expected; baseline 18 pre-existing errors (per QNT-01b T8 discoveries) should be unchanged. If the WS `import_complete` payload gains `trades_skipped_duplicate` and the extension consumes it, add the field to the schema — grep `import_complete` in `testudo-extension/src/` to check.
- Integration grep: `grep -rn "idx_unique_import_fill\|duplicate key\|timestamp as i64" crates/router/src/services/` returns ZERO hits in code (may still appear in test fixtures/comments; target is non-empty error-string-matching usage).
- Spec deviations logged: CP-0 is static analysis (not live DB), `fill_detector.rs` unmodified, `cex_history.rs` unmodified.

**Manual QA (deferred to live session):**
- User runs `POST /api/v1/imports/cex/bybit` twice on the same time window.
  - First run: `trades_imported = N`, `trades_skipped_duplicate = 0`.
  - Second run: `trades_imported = 0`, `trades_skipped_duplicate = N`.
  - `COUNT(*) FROM journal_trades WHERE source = 'import_ccxt'` unchanged between runs.
- Live Bybit round-trip: submit via Alt+X → fill → TP/SL close → journal row populates with `source = 'testudo'`, `exchange_fill_id = NULL`. No regression.
- Post-deploy cleanup SQL from T1 commit body is run once against prod DB to purge pre-existing duplicates.

**Commit plan:**
- T2 (bundled with T3 to avoid intermediate broken build since T2 changes record_trade_close's signature and T3 adds defensive ON CONFLICT): `fix(hist-03): idempotent CEX history import — ON CONFLICT DO NOTHING + exchange-name canonicalization` — body references CP-0 findings + post-deploy cleanup SQL.
  - Alternative if splitting: T2 alone is a breaking-build commit unless the 4 caller sites also land. Prefer T2+T3+T5 in one commit.
- T4: `refactor(hist-03): canonical_exchange_name helper + apply at journal INSERT sites`
- T5: (folded into T2 commit if splitting is needed)
- T6: `fix(hist-03): remove timestamp fallback on exchange_fill_id — log+skip unparseable fill IDs`
- T7: `test(hist-03): integration test for idempotent re-import`
- T8: umbrella archival (may be no-op if per-task commits cover everything): `chore(hist-03): archive spec`

**Recommended final commit strategy:** Land T2+T3+T5 as a single atomic commit titled `fix(hist-03): idempotent CEX history import — ON CONFLICT DO NOTHING`, then T4/T6/T7/T8 as small follow-ups. Reduces review friction and keeps the build green.

**Archive:** Move `.specify/specs/HIST-03-import-dedup/` → `.specify/spec-archive/HIST-03-import-dedup/` after T8.

---

## Discoveries

### 2026-04-20 — HIST-03 planning

1. **HIST-01 partial unique index exists as documented.** `sqlx_postgres/migrations/20260326000000_add_import_fields.up.sql:8-10`. ON CONFLICT target predicate must match its `WHERE exchange_fill_id IS NOT NULL` qualifier verbatim.

2. **Journal INSERT sites are TWO, not one.** `JournalService::record_trade_close` handles all 3 importer callers + HL live poll. `TradeEventWriter::insert_journal_trade` handles CEX live fill atomic writes. Spec §Files incorrectly lists `fill_detector.rs` as modified — that file emits a channel event, doesn't write SQL. Document as spec deviation in T1.

3. **Current dedup mechanism is error-string matching**, not ON CONFLICT. `import_worker.rs:295-304, 394-401, 461-468` all `contains("idx_unique_import_fill") || contains("duplicate key")`. Brittle; depends on error message format stability across sqlx versions.

4. **`exchange_fill_id` timestamp fallback is spec hypothesis #2 — confirmed present.** `import_worker.rs:386, 533` both `parse().unwrap_or(fill.timestamp as i64)`. This is the most likely root cause: if Bybit returns a non-numeric `execId`/`tradeId` (UUID-ish hash string), the fallback synthesizes a key from timestamp. If two import runs hit the same fill with different endpoint versions or different ID fields, the synthesized key differs → partial index misses → duplicate row created. FR-5 removes this fallback.

5. **Exchange casing drift (spec hypothesis #3) is low-risk but not defended.** `common_utils/src/models/exchange_account.rs:125` lowercases on account save. Journal-write boundary (`import_worker.rs:370`, `fill_detector.rs:635`) passes through unmodified. FR-4's `canonical_exchange_name()` helper + apply at INSERT sites closes this gap belt-and-braces.

6. **Hypothesis #4 is partially incorrect.** Live CEX trades use `TradeEventWriter::insert_journal_trade` (separate INSERT path), NOT `record_trade_close`. Only HL live polls (`ws_fills.rs:520`) share `record_trade_close` with imports. Spec Risk #2 ("Live path coupling") overstates the blast radius — live CEX path is decoupled.

7. **`record_trade_close` has a pre-INSERT idempotency SELECT by `trade_group_id`** (L158-178). Live trades hit this and short-circuit. Imports always have `trade_group_id = None`, bypassing it. The partial unique index is imports' only dedup guard — hence the sensitivity to hypotheses #2 and #3.

8. **No existing `canonical_exchange_name()` helper.** `.to_lowercase()` inlined at 3 sites in `sqlx_postgres/` and `common_utils/`. FR-4 adds the single source of truth.

9. **Router crate is binary-only** — no `src/lib.rs`, no top-level `tests/` integration dir (per AGENTS.md 2026-04-17 RSK-01 T3 discovery). Integration tests for DB-touching code must live inline as `#[cfg(test)] mod tests` using pool fixtures. T7 follows this pattern with either `#[sqlx::test]` (preferred) or `#[tokio::test] #[ignore]` (fallback).

10. **CP-0 cannot be a live DB query from Vox.** No DB credentials, must not read `.env`. Translated into static-analysis diagnostic: hypotheses #1 and #2 are the code-level causes; the fix addresses both. User runs the spec's `GROUP BY HAVING COUNT > 1` query manually post-deploy to confirm no new duplicates form.

11. **Post-deploy cleanup SQL ships in T8 commit body, not as migration.** Spec Risk #4. One-shot `DELETE USING` statement purges existing duplicates by keeping the lowest `id`. User runs manually.

12. **WebSocket import_complete payload may need extension update.** If `testudo-extension/src/` consumes the import-complete WS event and the spec requires `trades_skipped_duplicate` surfaced to the UI, the extension's WS schema needs the new field. Grep during T8 to check — if not consumed, no extension change needed.

13. **`TradeEventWriter::insert_journal_trade` gets defensive-only ON CONFLICT** (T3). No RecordOutcome plumbing — it uses `execute()`, doesn't consume RETURNING. Live trades structurally can't hit the partial index (exchange_fill_id is always NULL). Belt-and-braces in case future code stamps fill IDs on the live path.

---

## Status

PLANNING COMPLETE

Spec: HIST-03-import-dedup
Total Tasks: 8 (T1, T2, T3, T4, T5, T6, T7, T8)
Ready for BUILD mode.

Next task: T1 — CP-0 static-analysis diagnostic (documentation folded into T2 commit body)
