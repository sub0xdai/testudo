# Implementation Plan

> Last updated: 2026-04-18
> Current spec: RSK-02-setup-tag-at-entry
> Phase: PLANNING COMPLETE — ready for BUILD

---

## Active Spec: RSK-02-setup-tag-at-entry

### Gap Analysis

**Extension (`testudo-extension/src/`):**
- `schemas.ts` (259 lines) — `TradePayloadSchema` (lines 49-71) is a nested object inside `RuntimeMessageSchema`'s discriminated union. `break_even_enabled` (line 59) uses `z.boolean().optional().default(true)` — exact template for adding `setup_tag: z.string().trim().max(48).nullable().optional()` after `exchange_account_id` (line 56). No discriminator changes needed.
- `components/TradeForm.tsx` (381 lines) — renders LONG/SHORT toggle, symbol, entry/stop/target, R:R display, management summary, balance block. Double-Enter safety: `confirmStep` signal starts at 0; first Enter → 1 ("Confirm Now"); second Enter calls `props.onConfirm(buildSetup())` (lines 104-113, 115-122). Initial focus goes to first focusable via `requestAnimationFrame` in modal.tsx:258-260. No existing autocomplete/datalist pattern anywhere.
- `components/TradeForm.tsx:104-113` `buildSetup()` returns bare `TradeSetup` (symbol/side/entry/stop/target/timeframe) — needs `setup_tag` added.
- `background/api.ts:266-295` `executeTrade()` POSTs to `/api/v1/trades` with body `{ symbol, side, entry_price, stop_loss_price, take_profit_price, management, exchange_account_id? }`. All numeric fields stringified (string convention across CCXT + router). Adding `setup_tag` = one line inside the body builder.
- `background/handlers.ts` (200 lines) — message dispatch map at lines 173-199. Adding a new `GET_SETUP_TAGS` message type needs: (1) schema variant in `RuntimeMessageSchema`, (2) `listSetupTags()` wrapper in `api.ts`, (3) `handleGetSetupTags` in handlers, (4) dispatch-map entry.
- `types.ts` `TradePayload` (lines 38-54) mirrors the Zod schema. `TradeSetup` lives in `scraper.ts:10-17` — both need `setup_tag?: string | null`.
- `content.ts:283-323` spreads the setup into the EXECUTE_TRADE payload — any new field on `TradeSetup` flows through automatically.
- `modal.tsx:185-262` — Shadow DOM host with focus trap. No changes needed for the field itself. If T4 opts to fetch tags before mounting (to pre-populate suggestions synchronously), add `setupTags?: string[]` prop plumbing; otherwise TradeForm fetches on focus.

**Backend (`testudo-exchange/crates/`):**
- Migration convention: `YYYYMMDDHHMMSS_descriptor.{up,down}.sql`. Latest is `20260410000000_multichain_wallet_address.{up,down}.sql`. Next: `20260418000000_add_setup_tag_to_trades.{up,down}.sql`.
- `routes/trade_management.rs:282-301` `CreateTradeRequest` — plain serde struct. Add `pub setup_tag: Option<String>,` after `idempotency_key` at line 300. Zero other DTO shape changes.
- Persistence pipeline is **3-stage**, which matters for where to stash `setup_tag`:
  1. **Request-time:** `CreateTradeRequest` → `TradeManagerService` places entry order + creates `managed_position` row.
  2. **Lifecycle:** `trade_events` (JSONB payload) append-only log tracks EntryFilled → StopLossFilled / TakeProfitFilled.
  3. **Close-time:** `TradeEventWriter::insert_journal_trade()` (trade_event_writer.rs:278-363) atomically writes `journal_trades` from a `TradeCloseEvent` synthesized off the events + position. Also `JournalService::record_trade_close()` is a direct path for imports.
- `managed_positions` table — doesn't have a natural "metadata" slot for setup_tag; adding a nullable TEXT column is the cleanest spot to hold it between place-trade and close-trade.
- `journal_trades` table (migration `20260318000000_create_journal_tables`) — has `notes TEXT` but no classification/tag field. `ALTER TABLE journal_trades ADD COLUMN setup_tag TEXT NULL` + `CREATE INDEX idx_journal_trades_user_setup ON journal_trades(user_id, setup_tag) WHERE setup_tag IS NOT NULL` covers FR-6.
- Existing tag system already plumbed: `journal_tags (id, user_id, name UNIQUE(user_id, name), color)` + `journal_trade_tags (trade_id, tag_id)` with `ON DELETE CASCADE`. Tag CRUD routes in `routes/journal.rs:821-1095`. FR-7 auto-tag = post-commit upsert into these two tables using lowercased-for-dedup name.
- `AuthenticatedUser` extractor (middleware/auth.rs:265-287) yields `{ user_id: Uuid, wallet_address: String }` post-AUTH-02 — use for all new endpoints.
- Template for GET with `AuthenticatedUser` + query param: `journal.rs:821-848` `list_tags()`. Pattern reuses `fetchWithCredentials` from frontend and Bearer from extension.
- `TradeCloseEvent` struct (journal_service.rs:15-35) — add `pub setup_tag: Option<String>` field. `compute_derived_fields()` is a pure P&L/R-multiple calc — no changes needed.
- INSERT statements in two places — `journal_service.rs:138-150` (direct path) and `trade_event_writer.rs:322-354` (transactional path). Both need `setup_tag` added to column list + binds.
- `parse_trade_close_payload` (trade_event_writer.rs:~442) extracts fields from JSONB — add `setup_tag` extraction.
- Daily-stats upsert (`journal_service.rs:223-297`) is unchanged; `setup_tag` is orthogonal to per-day aggregates.
- `upsert_daily_stats` post-commit idiom is the blueprint for auto-tag creation: non-critical, fire-and-forget, wrapped in try/log.
- Existing analytics endpoints live under `/api/v1/journal/analytics/*` (e.g. `symbol-breakdown` → `fetchSymbolBreakdown`). A new `/api/v1/journal/analytics/setup-breakdown` fits naturally into the same file and query-builder pattern.

**Journal (`testudo-journal/src/`):**
- `api/client.ts` (635 lines) — existing analytics fetcher idiom at lines 143-169 returns `{ data: Item[] }`. `fetchWithCredentials` helper auto-refreshes on 401. `JournalTrade` interface (lines 173-198) needs `setup_tag: string | null` added. Tag fetchers already exist (`fetchTags`, `createTag`, `addTradeTags`, etc. at lines 200+).
- `components/charts/SymbolBreakdown.tsx` (81 lines) — perfect template. Uses `createResource(filters, fetchSymbolBreakdown)`, `createMemo` for ECharts option, `ChartContainer` wrapper with loading/empty/error. Horizontal bar, `barWidth: '40%'`, `inverse: true` for y-axis category, `palette[i % palette.length]` for color cycling. Sort order is backend-driven (no client-side toggle).
- `components/charts/ExpectancyBySymbol.tsx` (84 lines) — shows the client-side transform pattern for derived metrics: `parseFloat(s.total_pnl) / s.trade_count` → expectancy. Same pattern we'll apply to setup aggregates. Has a sort: `.sort((a, b) => b.expectancy - a.expectancy)`.
- `components/ChartSelector.tsx` (95 lines) — `ChartOption` union at line 20, `CHART_OPTIONS` list at line 24, `<Show when={selected() === 'x'}><XBreakdown /></Show>` pattern lines 74-91. Adding `'setup'` = 4 edits (union, option, lazy import, Show branch).
- `components/Overview.tsx` (383 lines post-RSK-01a) — mounts two `<ChartSelector>` instances in a grid (lines 374-377); no changes needed beyond adding the new option upstream in ChartSelector.
- `lib/formatters.ts` (85 lines) — existing `formatCurrency`, `formatPercent`, `pnlColor`, `rColor` helpers cover all display needs for the new chart's columns. No new helpers required.
- `components/journal/TagSelector.tsx` (67 lines) — dropdown pattern with `role="listbox"`, filter-by-prefix, click-to-add, auto-close. The TradeForm autocomplete should mimic this UX grammar (not copy the component verbatim — the extension is isolated Shadow DOM; just mirror the keyboard/focus idiom).

### Design Decisions (captured before tasking)

1. **setup_tag stashed on `managed_positions` + carried into TradeClose payload.** The cleanest place to hold setup_tag between place-trade and close-trade is the `managed_positions` table — it's the durable per-position record that already carries entry/exit metadata. At close, `TradeEventWriter` reads it via JOIN (or reads it off the synthesized `TradeCloseEvent` — whichever is simpler given the existing call graph). Avoids bloating every trade_event payload with a rarely-changing field, and avoids a separate drafts table.

2. **Two separate backend endpoints: autocomplete vs analytics.** They serve different shapes.
   - `GET /api/v1/journal/setup-tags?limit=20` → distinct tags ordered by `MAX(closed_at) DESC, COUNT(*) DESC`, returns `[{ name, last_used, uses }]`. Cheap. Used by TradeForm on focus.
   - `GET /api/v1/journal/analytics/setup-breakdown` → per-setup aggregates (`trade_count`, `win_rate`, `avg_r_multiple`, `total_pnl`, `expectancy`) with filter-context support. Fits existing `/analytics/*` pattern.

3. **Spec's `/api/user/setup-tags` path renamed to `/api/v1/journal/setup-tags`.** Codebase convention is `/api/v1/journal/*` for journal-domain data, and there's no existing `user.rs` routes module. Creating one just for this endpoint is over-structuring. Route lives in `journal.rs` alongside `list_tags`. Same decision for the analytics endpoint (`/api/v1/journal/analytics/setup-breakdown`).

4. **Auto-tag (FR-7) is post-commit and best-effort.** Mirrors `upsert_daily_stats` and draft-notes merge: after `journal_trades` INSERT commits, upsert `journal_tags (user_id, LOWER(setup_tag))` with `ON CONFLICT DO NOTHING`, then upsert `journal_trade_tags (trade_id, tag_id)` with `ON CONFLICT DO NOTHING`. Failures log but don't abort — the trade is already persisted correctly. Tag display color defaults to null (TagBadge falls back to palette by index).

5. **Normalization: trim + max 48 + preserve casing for display + lowercase for dedup.** The Zod `.trim().max(48)` at the schema layer handles the wire-format constraint. Backend receives the trimmed/bounded string and stores it verbatim in `journal_trades.setup_tag`. Dedup is via `journal_tags.name` which we lowercase before INSERT so `"Breakout"`, `"breakout"`, `"BREAKOUT"` collapse to one tag row. The original casing is preserved on the trade record itself; the chart groups by `LOWER(setup_tag)` too.

6. **Empty setup_tag on the wire = null in storage.** The extension sends `setup_tag: null` (or omits the field) when the user leaves the input empty. The Zod schema accepts `z.string().trim().max(48).nullable().optional()` — handles undefined, null, and empty-string equally (empty becomes null after trim → convert to null in `buildSetup`). Backend's `Option<String>` handles all three JSON shapes.

7. **Autocomplete fetched on focus, cached 5min, debounced 300ms on subsequent opens.** FR-2 + risk #4. First modal open per session: fetch `GET /api/v1/journal/setup-tags?limit=20`, cache in a module-scoped `{ tags: string[], fetchedAt: number }`. Subsequent opens within 5min: use cache. Filter client-side by prefix. Max 10 suggestions in dropdown. Graceful fallback: empty list → no dropdown, field still works as free-text input.

8. **Double-Enter safety preserved unchanged.** The Setup field is an ordinary `<input>` in the normal tab order. Tab advances from target → setup → Cancel/Confirm buttons. Enter in the Setup field accepts the highlighted suggestion if dropdown is open, else defers to the form-wide handler that owns confirm-step logic. No new Enter trapping.

9. **Chart handles `(untagged)` bucket explicitly.** Acceptance criterion: "Empty / null setup_tag trades are grouped under `(untagged)` in the Setup Breakdown." Backend aggregates with `COALESCE(setup_tag, '(untagged)')`. Chart renders the bucket the same as any other — no special styling needed.

10. **No new dependencies anywhere.** Spec explicitly says "Dependencies Added: None." Zod, ECharts, alloy, sqlx — all existing versions handle everything.

### Parallel Track Detection

```
T1 (backend migration + DTO + persistence) ────┐
                                                ├──→ T3 (autocomplete endpoint) ──→ T4 (TradeForm field + autocomplete)
                                                ├──→ T5 (auto-tag post-commit)
                                                └──→ T6 (analytics endpoint) ──→ T7 (journal API client types + fetchers) ──→ T8 (SetupBreakdown chart) ──→ T9 (ChartSelector wire)
T2 (extension schema + types + payload passthrough) ──(depends on T1 shape only)──→ T4
                                                                                    │
                                                                                    └──→ T10 (verification + commit)
```

**Realistic parallelism:** T3 + T5 + T6 can run in parallel after T1 lands; they touch different files. T2 can land independently once the T1 wire-shape is agreed. T4 needs both T2 + T3. T7+T8+T9 are sequential. Because Vox BUILD mode is single-threaded per task, I'll keep the plan sequential but note the opportunity in Discoveries in case a fast-follow pass wants to parallelize.

---

## Tasks

### T1: Backend migration + DTO passthrough + persistence — `complete`

**Scope:** CP-1 backend. `setup_tag` threads from `CreateTradeRequest` → `managed_positions` → `TradeCloseEvent` payload → `journal_trades.setup_tag` column. Normalization (trim, max 48) enforced at the request layer. Nullable throughout.

**Files:**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260418000000_add_setup_tag_to_trades.up.sql` — NEW:
  ```sql
  ALTER TABLE journal_trades ADD COLUMN setup_tag TEXT NULL;
  CREATE INDEX idx_journal_trades_user_setup
    ON journal_trades(user_id, setup_tag) WHERE setup_tag IS NOT NULL;

  ALTER TABLE managed_positions ADD COLUMN setup_tag TEXT NULL;
  ```
- `testudo-exchange/crates/sqlx_postgres/migrations/20260418000000_add_setup_tag_to_trades.down.sql` — NEW:
  ```sql
  ALTER TABLE managed_positions DROP COLUMN setup_tag;
  DROP INDEX IF EXISTS idx_journal_trades_user_setup;
  ALTER TABLE journal_trades DROP COLUMN setup_tag;
  ```
- `testudo-exchange/crates/router/src/routes/trade_management.rs` — MODIFIED:
  - Add `pub setup_tag: Option<String>,` to `CreateTradeRequest` at line 300 (after `idempotency_key`).
  - In `create_trade` handler, normalize: `let setup_tag = body.setup_tag.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.len() <= 48);` — reject input > 48 chars upstream? No — truncate or ignore per extension-level validation. Reject with 400 if > 48 to be strict: `if body.setup_tag.as_ref().map_or(false, |s| s.len() > 48) { return HttpResponse::BadRequest()... }`.
  - Pass `setup_tag` into the `TradeManagerService` call (whatever registers the position).
- `testudo-exchange/crates/router/src/services/trade_manager/service.rs` — MODIFIED:
  - Accept `setup_tag: Option<String>` in the trade placement fn signature.
  - INSERT `setup_tag` into `managed_positions` row on position creation.
- `testudo-exchange/crates/router/src/services/journal_service.rs` — MODIFIED:
  - Add `pub setup_tag: Option<String>` to `TradeCloseEvent` struct (line 15-35).
  - Update INSERT statement in `record_trade_close` (lines 138-150) to include `setup_tag` column + bind. Column index: 24 (after `exchange_fill_id`).
- `testudo-exchange/crates/router/src/services/trade_event_writer.rs` — MODIFIED:
  - Update `parse_trade_close_payload` (~line 442) to extract `setup_tag` from JSONB payload.
  - Alternatively (cleaner): modify `insert_journal_trade` (lines 278-363) to JOIN `managed_positions` on group_id to read `setup_tag` and include it in the INSERT.
  - INSERT statement (lines 322-354) adds `setup_tag` column + bind.
- `testudo-exchange/crates/router/src/models/journal.rs` — MODIFIED:
  - Add `pub setup_tag: Option<String>` to `JournalTrade` struct (wherever it's defined).

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets`
- `cd testudo-exchange && cargo test`

**Acceptance (subset of CP-1):**
- Migration applies cleanly up and down.
- `CreateTradeRequest` accepts `setup_tag` as optional field.
- Trades placed with `setup_tag` land in `managed_positions.setup_tag`.
- On trade close, `journal_trades.setup_tag` is populated from the position.
- `setup_tag = null` path (missing or empty) works end-to-end without warnings.
- All existing tests still pass; add at least 2 new tests: (a) CreateTradeRequest deserializes with and without `setup_tag`; (b) `journal_trades.setup_tag` is populated on TradeClose when the position had one.

---

### T2: Extension schema + types + payload passthrough — `complete`

**Scope:** CP-1 extension half. Zod schema + TypeScript types extended. Extension POSTs `setup_tag` if TradeSetup carries one. No UI yet — TradeForm comes in T4.

**Files:**
- `testudo-extension/src/schemas.ts` — MODIFIED:
  - In `TradePayloadSchema` (lines 49-71), after `exchange_account_id` (line 56), insert:
    ```typescript
    setup_tag: z.string().trim().max(48).nullable().optional(),
    ```
- `testudo-extension/src/types.ts` — MODIFIED:
  - `TradePayload` (lines 38-54) — add `setup_tag?: string | null;` after `exchange_account_id`.
- `testudo-extension/src/scraper.ts` — MODIFIED:
  - `TradeSetup` (lines 10-17) — add `setup_tag?: string | null;` after `timeframe`.
- `testudo-extension/src/background/api.ts` — MODIFIED:
  - In `executeTrade()` (lines 266-295), body builder (lines 272-280), after `exchange_account_id` handling, add:
    ```typescript
    if (payload.setup_tag && payload.setup_tag.trim().length > 0) {
      body.setup_tag = payload.setup_tag.trim();
    }
    ```
    (Explicit omission for empty / whitespace-only strings — backend sees absent field and treats as null.)
- `testudo-extension/src/content.ts` — NO CHANGE NEEDED. The `...setup` spread at line 289 auto-forwards any new TradeSetup field.
- `testudo-extension/src/modal.tsx` — NO CHANGE NEEDED. `initialSetup` prop flows through unchanged.

**Validate:**
- `cd testudo-extension && bun run build` (both Chrome + Firefox targets).
- Spot-check: artifact sizes unchanged aside from ~50-100 bytes for the new schema entry.

**Acceptance (subset of CP-1):**
- `bun run build` passes for both targets.
- Zod schema accepts `{setup_tag: "foo"}`, `{setup_tag: null}`, `{setup_tag: undefined}`, `{}` (omitted).
- Zod rejects `{setup_tag: "x".repeat(49)}` (49 chars > max 48).
- `executeTrade` POST body includes `setup_tag` string when provided, omits when empty/null.
- Existing background tests all still pass (70 passing, 7 pre-existing failures unchanged).

---

### T3: Backend autocomplete endpoint `GET /api/v1/journal/setup-tags` — `complete`

**Scope:** FR-11. Return up to `limit` distinct tags the user has previously used, ordered by recency then frequency. Reused by TradeForm autocomplete in T4.

**Files:**
- `testudo-exchange/crates/router/src/routes/journal.rs` — MODIFIED:
  - Add handler `list_setup_tags(req, app_state, user, query) -> Result<HttpResponse>`:
    ```rust
    #[derive(Debug, Deserialize)]
    pub struct ListSetupTagsQuery {
        pub limit: Option<i64>,
    }

    #[derive(Debug, Serialize, sqlx::FromRow)]
    pub struct SetupTagEntry {
        pub name: String,
        pub last_used: DateTime<Utc>,
        pub uses: i64,
    }

    pub async fn list_setup_tags(
        app_state: web::Data<AppState>,
        user: AuthenticatedUser,
        query: web::Query<ListSetupTagsQuery>,
    ) -> Result<HttpResponse> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let tags: Vec<SetupTagEntry> = sqlx::query_as(
            "SELECT setup_tag AS name, MAX(closed_at) AS last_used, COUNT(*) AS uses \
             FROM journal_trades \
             WHERE user_id = $1 AND setup_tag IS NOT NULL AND setup_tag <> '' \
             GROUP BY setup_tag \
             ORDER BY last_used DESC, uses DESC \
             LIMIT $2"
        ).bind(user.user_id).bind(limit).fetch_all(&app_state.pool).await
        .map_err(|e| { tracing::error!("list_setup_tags failed: {e}"); ErrorInternalServerError("db") })?;
        Ok(HttpResponse::Ok().json(tags))
    }
    ```
- `testudo-exchange/crates/router/src/main.rs` — MODIFIED:
  - Register route at the `journal` scope: `.route("/setup-tags", web::get().to(journal::list_setup_tags))`.
  - Placement: next to `/tags` route registration (~line 1003, adjust for actual location).

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test`

**Acceptance:**
- `GET /api/v1/journal/setup-tags?limit=20` with valid JWT returns JSON array ordered by `last_used DESC, uses DESC`.
- Empty history → `[]` (not 404).
- Unauthenticated request → 401.
- Limit clamped to [1, 100]; default 20.
- Cargo tests pass; add a test for the handler that asserts the SQL + clamp behavior (can use `sqlx::test` or a mock).

---

### T4: TradeForm.tsx Setup field + autocomplete — `complete`

**Scope:** CP-2. Optional Setup text input below price fields. Autocomplete dropdown sourced from the T3 endpoint via a new `GET_SETUP_TAGS` message type. Double-Enter safety preserved.

**Files:**
- `testudo-extension/src/schemas.ts` — MODIFIED:
  - Add to `RuntimeMessageSchema` union:
    ```typescript
    z.object({ type: z.literal("GET_SETUP_TAGS"), limit: z.number().int().min(1).max(100).optional() }),
    ```
  - Add response schema:
    ```typescript
    export const SetupTagEntrySchema = z.object({
      name: z.string(),
      last_used: z.string(),
      uses: z.number().int().nonnegative(),
    });
    export const SetupTagsResponseSchema = z.array(SetupTagEntrySchema);
    ```
- `testudo-extension/src/background/api.ts` — MODIFIED:
  - Add `listSetupTags(limit?: number)`:
    ```typescript
    export async function listSetupTags(limit = 20): Promise<{ success: boolean; data?: string[]; error?: string }> {
      const result = await apiRequest(`/api/v1/journal/setup-tags?limit=${limit}`, { auth: "hard" });
      if (!result.ok) return { success: false, error: result.error };
      const parsed = SetupTagsResponseSchema.safeParse(result.raw);
      if (!parsed.success) return { success: false, error: "invalid response shape" };
      return { success: true, data: parsed.data.map(t => t.name) };
    }
    ```
- `testudo-extension/src/background/handlers.ts` — MODIFIED:
  - Add handler:
    ```typescript
    function handleGetSetupTags(msg: ParsedMessage): Promise<unknown> {
      return listSetupTags((msg as MsgOf<"GET_SETUP_TAGS">).limit);
    }
    ```
  - Add `GET_SETUP_TAGS: handleGetSetupTags,` to dispatch map.
- `testudo-extension/src/components/TradeForm.tsx` — MODIFIED:
  - Add signals: `const [setupTag, setSetupTag] = createSignal(props.initialSetup?.setup_tag ?? "");`
  - Add signals: `const [suggestions, setSuggestions] = createSignal<string[]>([]);`
  - Add signals: `const [showSuggestions, setShowSuggestions] = createSignal(false);`
  - Add signals: `const [highlightIdx, setHighlightIdx] = createSignal(0);`
  - Module-scoped cache: `let tagCache: { tags: string[]; fetchedAt: number } | null = null; const CACHE_TTL_MS = 5 * 60 * 1000;`
  - Fetch helper (inside component, debounced 300ms on first focus):
    ```typescript
    async function loadTags() {
      const now = Date.now();
      if (tagCache && now - tagCache.fetchedAt < CACHE_TTL_MS) {
        setSuggestions(tagCache.tags);
        return;
      }
      try {
        const res = await browser.runtime.sendMessage({ type: "GET_SETUP_TAGS", limit: 20 });
        if (res?.success && Array.isArray(res.data)) {
          tagCache = { tags: res.data, fetchedAt: now };
          setSuggestions(res.data);
        }
      } catch { /* silent fallback, field still works as free-text */ }
    }
    ```
  - Filtered suggestions derived memo: `const filtered = createMemo(() => suggestions().filter(t => t.toLowerCase().startsWith(setupTag().toLowerCase())).slice(0, 10));`
  - Render an `<input type="text" placeholder="Setup (optional)">` block below the target field (around current line 277-286 region where R:R sits). Layout: label + input + dropdown as an absolutely-positioned `<ul>` below.
  - `onFocus` → `loadTags()` + `setShowSuggestions(true)`.
  - `onBlur` → `setTimeout(() => setShowSuggestions(false), 150)` (delay so click-to-select fires first).
  - `onInput` → update `setupTag`, `setHighlightIdx(0)`, `setShowSuggestions(true)`.
  - `onKeyDown`:
    - ArrowDown → `setHighlightIdx(i => Math.min(i + 1, filtered().length - 1))`, preventDefault.
    - ArrowUp → `setHighlightIdx(i => Math.max(i - 1, 0))`, preventDefault.
    - Enter with `showSuggestions() && filtered().length > 0` → accept highlighted, `setShowSuggestions(false)`, preventDefault. (Does NOT advance confirm step.)
    - Enter with no suggestions visible → fall through to form-wide Enter handler (double-Enter safety).
    - Tab → accept highlighted if dropdown open, else normal tab.
    - Escape → `setShowSuggestions(false)`, do NOT dismiss modal (preventDefault + stopPropagation on the input).
  - Update `buildSetup()` at line 104-113 to include `setup_tag: setupTag().trim() || null`.
  - Dropdown rendering: `<Show when={showSuggestions() && filtered().length > 0}>` → `<ul class="suggestions">` → `<For each={filtered()}>` each as `<li classList={{ highlighted: i() === highlightIdx() }} onMouseDown={...}>`.
  - Styling: reuse existing Shadow DOM `field-input` + add `.suggestions` list rule in the inline CSS at `modal.tsx` MODAL_STYLES (border, bg-container-bg, absolute, max-h, overflow-y-auto).
- `testudo-extension/src/modal.tsx` — MODIFIED:
  - Add `.suggestions` + `.suggestion-item` + `.suggestion-item.highlighted` CSS rules in MODAL_STYLES template string.

**Validate:**
- `cd testudo-extension && bun run build`
- Manual: open modal on TradingView, tab to setup field, type → dropdown appears; arrow-down/up navigates; Enter accepts; Tab accepts; click-outside closes.
- Manual: empty setup_tag → Enter fires double-Enter safety unchanged.

**Acceptance (CP-2):**
- `bun run build` passes.
- TradeForm renders optional Setup field below target.
- First Enter on empty Setup → "Confirm Now" (double-Enter preserved).
- Autocomplete dropdown appears on focus + type, max 10 suggestions, prefix-matched case-insensitively.
- Keyboard nav (arrow up/down, Tab, Enter, Escape) behaves per design decision 7+8.
- Cache works: second modal open within 5min doesn't re-fetch.
- Graceful fallback: network failure or empty history → no dropdown, field still usable as free-text.

---

### T5: Auto-tag creation on trade close (FR-7) — `complete`

**Scope:** CP-3. After `journal_trades` INSERT commits, upsert `journal_tags (user_id, LOWER(setup_tag))` and `journal_trade_tags (trade_id, tag_id)`. Fire-and-forget; failures log but don't block.

**Files:**
- `testudo-exchange/crates/router/src/services/journal_service.rs` — MODIFIED:
  - After the daily-stats upsert in `record_trade_close` (~line 220), add:
    ```rust
    if let Some(tag) = trade.setup_tag.as_ref() {
        if let Err(e) = self.upsert_auto_tag(trade.user_id, trade.id, tag).await {
            tracing::warn!("auto-tag upsert failed for trade {}: {e}", trade.id);
        }
    }
    ```
  - New method:
    ```rust
    async fn upsert_auto_tag(&self, user_id: Uuid, trade_id: Uuid, raw_tag: &str) -> Result<(), sqlx::Error> {
        let name_lc = raw_tag.trim().to_lowercase();
        if name_lc.is_empty() { return Ok(()); }
        let tag_id: Uuid = sqlx::query_scalar(
            "INSERT INTO journal_tags (user_id, name, color) VALUES ($1, $2, NULL) \
             ON CONFLICT (user_id, name) DO UPDATE SET name = EXCLUDED.name \
             RETURNING id"
        ).bind(user_id).bind(&name_lc).fetch_one(&self.pool).await?;
        sqlx::query(
            "INSERT INTO journal_trade_tags (trade_id, tag_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING"
        ).bind(trade_id).bind(tag_id).execute(&self.pool).await?;
        Ok(())
    }
    ```
  - Note: the `ON CONFLICT DO UPDATE SET name = EXCLUDED.name` is a no-op that forces RETURNING to fire (postgres doesn't RETURN on pure `DO NOTHING` conflict). Alternative: separate SELECT after `DO NOTHING`. Pick whichever is idiomatic for existing queries in this file.
- `testudo-exchange/crates/router/src/services/trade_event_writer.rs` — MODIFIED:
  - After the transaction commits in `flush_transaction` (~line 194-236), and after `upsert_daily_stats`, add equivalent auto-tag call. Read `setup_tag` from the newly-inserted `journal_trades` row (or carry it forward from the `TradeCloseEvent` like other fields).
  - Option: extract the auto-tag helper into a free function or shared module so both paths DRY it.

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- Integration: place a trade with `setup_tag: "Breakout"`, close it, query `journal_tags` → row exists with `name = "breakout"`. Query `journal_trade_tags` → row links the trade.
- Second trade with `setup_tag: "breakout"` (lowercase) → only ONE row in `journal_tags`; both trades linked.
- Third trade with `setup_tag: null` → no new tag row, no trade_tag link.

**Acceptance (CP-3):**
- All tests pass.
- Manual: closing a trade with setup_tag results in a `journal_tags` row (lowercased) and a `journal_trade_tags` link.
- Manual: case variants (`"Breakout"`, `"breakout"`, `"BREAKOUT"`) all collapse to a single tag row.
- Failures log at `warn` level but don't affect trade persistence (trade still inserted if tag upsert fails).
- Trade shows in journal views with the tag visible via the existing tag rendering.

---

### T6: Backend analytics endpoint `/analytics/setup-breakdown` — `pending`

**Scope:** Per-setup aggregated stats for the chart. Mirrors the existing `/analytics/symbol-breakdown` pattern.

**Files:**
- `testudo-exchange/crates/router/src/routes/journal.rs` (or the analytics submodule if separated) — MODIFIED:
  - New struct:
    ```rust
    #[derive(Debug, Serialize, sqlx::FromRow)]
    pub struct SetupBreakdownItem {
        pub setup_tag: String,           // "(untagged)" for NULL
        pub trade_count: i64,
        pub total_pnl: Decimal,
        pub win_rate: Decimal,           // 0..100
        pub avg_r_multiple: Option<Decimal>,
        pub expectancy: Decimal,         // total_pnl / trade_count
    }
    ```
  - New handler with same filter-builder pattern as `list_trades` (supports `exchange`, `symbol`, `date_from`, `date_to`):
    ```rust
    pub async fn setup_breakdown(...) -> Result<HttpResponse> {
        // Build WHERE clause from filters
        let items: Vec<SetupBreakdownItem> = sqlx::query_as(
            "SELECT \
               COALESCE(setup_tag, '(untagged)') AS setup_tag, \
               COUNT(*) AS trade_count, \
               SUM(net_pnl) AS total_pnl, \
               (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC / NULLIF(COUNT(*), 0) * 100 AS win_rate, \
               AVG(r_multiple) AS avg_r_multiple, \
               SUM(net_pnl) / NULLIF(COUNT(*), 0) AS expectancy \
             FROM journal_trades \
             WHERE user_id = $1 AND closed_at BETWEEN $2 AND $3 \
             GROUP BY COALESCE(setup_tag, '(untagged)') \
             ORDER BY expectancy DESC"
        )...
        Ok(HttpResponse::Ok().json(json!({ "data": items })))
    }
    ```
- `testudo-exchange/crates/router/src/main.rs` — MODIFIED:
  - Register `.route("/analytics/setup-breakdown", web::get().to(journal::setup_breakdown))` next to other `/analytics/*` routes.

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- Unit/integration test: seed 5 trades with mixed setup_tags (2 "breakout", 2 "reversion", 1 null) → endpoint returns 3 buckets; `(untagged)` has trade_count=1; win_rates + expectancies correct.

**Acceptance:**
- `GET /api/v1/journal/analytics/setup-breakdown?date_from=...&date_to=...` returns `{ data: [...] }` with per-setup aggregates.
- Response shape matches `SetupBreakdownItem` schema.
- NULL setup_tag bucketed as `(untagged)`.
- Filter support (exchange/symbol/date) mirrors other analytics endpoints.

---

### T7: Journal API client types + fetchers — `pending`

**Scope:** Wire the frontend to the two new backend endpoints. Also extend `JournalTrade` type.

**Files:**
- `testudo-journal/src/api/client.ts` — MODIFIED:
  - `JournalTrade` interface (lines 173-198) → add `setup_tag: string | null` after `notes`.
  - Add new interface after `SymbolBreakdownItem`:
    ```typescript
    export interface SetupBreakdownItem {
      setup_tag: string
      trade_count: number
      total_pnl: string
      win_rate: string
      avg_r_multiple: string | null
      expectancy: string
    }
    ```
  - Add fetcher:
    ```typescript
    export async function fetchSetupBreakdown(filters: StatsFilter): Promise<{ data: SetupBreakdownItem[] }> {
      return fetchApi<{ data: SetupBreakdownItem[] }>('setup-breakdown', filters)
    }
    ```
  - Add `fetchUserSetupTags()` (for parity with extension cache; journal may not use it right now but the endpoint is the single source):
    ```typescript
    export interface SetupTagEntry { name: string; last_used: string; uses: number }
    export async function fetchUserSetupTags(limit = 20): Promise<SetupTagEntry[]> {
      const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/setup-tags?limit=${limit}`)
      if (!res.ok) throw new Error(`API error: ${res.status}`)
      return res.json()
    }
    ```

**Validate:**
- `cd testudo-journal && bun run build`

**Acceptance:**
- `bun run build` passes with no type errors.
- `JournalTrade` interface includes `setup_tag: string | null`.
- `fetchSetupBreakdown` signature matches sibling analytics fetchers.
- `fetchUserSetupTags` returns `SetupTagEntry[]`.

---

### T8: `SetupBreakdown.tsx` chart component — `pending`

**Scope:** CP-4. Mirrors `SymbolBreakdown.tsx` but groups by setup_tag and displays all four metrics (trade_count, win_rate, avg_r_multiple, expectancy). Sortable by any column (acceptance criterion + FR-9).

**Files:**
- `testudo-journal/src/components/charts/SetupBreakdown.tsx` — NEW:
  - Signature: `export function SetupBreakdown(): JSX.Element`.
  - Imports: `useFilters`, `fetchSetupBreakdown`, `ChartContainer`, ECharts + `EChart`, `getTagPalette`, `getTextTertiary`, `formatCurrency`, `formatPercent`, `formatNumber`, `pnlColor`.
  - `const [data, { refetch }] = createResource(filters, fetchSetupBreakdown);`
  - `const [sortKey, setSortKey] = createSignal<'trade_count' | 'win_rate' | 'avg_r_multiple' | 'expectancy'>('expectancy');`
  - `const [sortDir, setSortDir] = createSignal<'asc' | 'desc'>('desc');`
  - `const sorted = createMemo(() => { … client-side sort by sortKey+sortDir … })`.
  - Render: the spec says "chart" but the acceptance criterion says "sortable trade count, win rate, avg R-multiple, expectancy" — sort-by-column implies a table UI, not a bar chart. Decision: render as a **mixed view** — horizontal bars keyed to the selected sort column (visual signal) + a small stats table below with sort-click headers. This matches `ExpectancyBySymbol.tsx`'s grammar (bars with numeric tooltip) while satisfying FR-9's sortability requirement. Keep under ~150 lines.
  - EChart option: horizontal bars, y-axis = setup names (with `(untagged)` if present), x-axis = value-of-selected-column, color by `palette[i % palette.length]`.
  - Below the chart: a 5-column table (Setup, Trades, Win%, Avg R, Expectancy) with `onClick` on header `<th>` to toggle sortKey/sortDir. Mobile: stack columns or hide non-critical (Avg R) via `sm:hidden`.
  - Wrap in `<ChartContainer title="SETUP BREAKDOWN" loading={data.loading} empty={!data()?.data?.length} onRetry={refetch} hasActiveFilters={hasActiveFilters()} onClearFilters={() => setFilters({})}>`.
- `testudo-journal/src/lib/help-content.ts` — MODIFIED (optional):
  - Add `'chart.setup': 'Per-setup trade count, win rate, average R-multiple, and expectancy. Untagged trades grouped separately.'` for HelpTip wiring.

**Validate:**
- `cd testudo-journal && bun run build`
- Manual (after T9 mounts it): select "Setup Breakdown" from the ChartSelector → chart renders with at least 2 setups when trades have tags. Click column header → sort order inverts.

**Acceptance (partial CP-4):**
- `bun run build` passes.
- Chart renders with bars + sortable table.
- `(untagged)` bucket displays correctly.
- Empty state ("No trades with setup tags yet" or the existing `ChartContainer` fallback) renders when `data.length === 0`.
- Sorting by any column works; default sort = expectancy DESC.
- Metrics formatted: trade_count as integer, win_rate as `"X.X%"`, avg_r_multiple as `"X.XR"`, expectancy as `"+$X.XX"` with pnl color.

---

### T9: ChartSelector + help-content wiring — `pending`

**Scope:** CP-4 mount. Four-edit pass on ChartSelector.tsx to surface the new chart option.

**Files:**
- `testudo-journal/src/components/ChartSelector.tsx` — MODIFIED:
  - Line 20 — add `| 'setup'` to the `ChartOption` union.
  - Line 24 — add `{ value: 'setup', label: 'Setup Breakdown' },` to `CHART_OPTIONS` (placement: after `'expectancy'` to keep the "per-group" charts grouped).
  - Top-of-file lazy imports — add `const SetupBreakdown = lazy(() => import('./charts/SetupBreakdown').then(m => ({ default: m.SetupBreakdown })))`.
  - Lines 74-91 render switch — add `<Show when={selected() === 'setup'}><SetupBreakdown /></Show>`.
- `testudo-journal/src/components/Overview.tsx` — NO CHANGE. The ChartSelector instances at lines 374-377 auto-pick up the new option.

**Validate:**
- `cd testudo-journal && bun run build`
- Manual: open journal → Overview → click a ChartSelector dropdown → "Setup Breakdown" is present → select it → chart loads (possibly empty if no tagged trades exist).

**Acceptance (CP-4):**
- `bun run build` passes.
- Both ChartSelector instances on Overview expose "Setup Breakdown" in the dropdown.
- HelpTip text appears when `selected() === 'setup'` (if T8 added it).
- Switching to/from Setup Breakdown works cleanly; `createResource` refires on filter changes.

---

### T10: Final verification + commit — `pending`

**Scope:** Full-suite verification per the Completion Protocol. Commit under the spec's mandated message.

**Verifications:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test` — passes, 0 new warnings beyond the 3 pre-existing ones documented in AGENTS.md.
- `cd testudo-extension && bun run build` — passes for both Chrome + Firefox targets.
- `cd testudo-journal && bun run build` — passes (tsc + vite, exit 0).
- Migration up + down cleanly from a fresh DB state.
- `grep -r "setup_tag" testudo-exchange/crates/` shows consistent usage across migration, DTO, service, journal INSERT, endpoint handler.
- `grep -r "setup_tag" testudo-extension/src/` shows schema + types + form + executeTrade all wired.
- `grep -r "setup_tag\|SetupBreakdown\|fetchSetupBreakdown" testudo-journal/src/` shows API client, chart component, and ChartSelector integration.
- **Manual QA (required by spec — acceptance criterion: "tag 5 trades with distinct setups, 2 untagged, confirm all appear correctly in Setup Breakdown"):**
  - Trade 1: setup_tag "breakout" → fills + closes
  - Trade 2: setup_tag "mean reversion" → fills + closes
  - Trade 3: setup_tag "liquidity sweep" → fills + closes
  - Trade 4: setup_tag "news fade" → fills + closes
  - Trade 5: setup_tag "Breakout" (case variant) → fills + closes
  - Trades 6, 7: no setup_tag → fill + close
  - Open Setup Breakdown chart → 5 buckets (breakout should have trade_count=2 due to case-insensitive dedup via auto-tag, but the chart groups by `journal_trades.setup_tag` which preserves original casing — so "breakout" and "Breakout" may appear as separate buckets unless the chart SQL uses `LOWER(setup_tag)` too).
  - **Decision point for T6/T10:** Should `setup_breakdown` SQL `GROUP BY LOWER(setup_tag)` for case-insensitive aggregation? Per design decision 5, YES — dedup is via lowercase. Update T6 SQL accordingly: `GROUP BY COALESCE(LOWER(setup_tag), '(untagged)')`. Will surface this during T6 build. (Captured in Discoveries.)
- **Deferred / nice-to-have (not blocking):**
  - Live user trading through the full Alt+X → Overview loop (requires real exchange state).
  - Autocomplete dropdown visual QA on TradingView, Hyperliquid, DexScreener shadow-DOM embeds.

**Commit format per spec Completion Signal #5:**
- T1: `feat(rsk-02): CP-1 backend — migration + DTO + persistence for setup_tag`
- T2: `feat(rsk-02): CP-1 extension — schema + types + payload passthrough`
- T3: `feat(rsk-02): autocomplete endpoint GET /api/v1/journal/setup-tags`
- T4: `feat(rsk-02): CP-2 — TradeForm Setup field + autocomplete`
- T5: `feat(rsk-02): CP-3 — auto-tag on trade close`
- T6: `feat(rsk-02): CP-4 backend — analytics/setup-breakdown endpoint`
- T7: `feat(rsk-02): CP-4 client — journal types + fetchSetupBreakdown`
- T8: `feat(rsk-02): CP-4 — SetupBreakdown chart component`
- T9: `feat(rsk-02): CP-4 — wire SetupBreakdown into ChartSelector`
- T10: Final squash commit or single umbrella commit: `feat(rsk-02): optional setup tag at Alt+X entry + Setup Breakdown chart`

**Archive step (per completion protocol):** After T10 closes clean, move `.specify/specs/RSK-02-setup-tag-at-entry/` to `.specify/spec-archive/`.

---

## Discoveries

### 2026-04-18 — RSK-02 planning

- **Spec's "trades" table = codebase's `journal_trades`.** The spec code snippet says `ALTER TABLE trades ADD COLUMN setup_tag TEXT NULL;` but no `trades` table exists — it's `journal_trades` (the close-time record) + `managed_positions` (the live record) + `trade_events` (the append-only log). setup_tag needs to live on both `managed_positions` (so it's durable during the position lifecycle) and `journal_trades` (so analytics can aggregate). Migration adds to both.

- **Spec's endpoint path `/api/user/setup-tags` doesn't match codebase convention.** All routes are `/api/v1/*`. There's no `user.rs` routes file, and this is journal-domain data (depends on `journal_trades`). Moved to `/api/v1/journal/setup-tags`. The analytics endpoint for the chart is separately `/api/v1/journal/analytics/setup-breakdown`, consistent with the existing `/analytics/symbol-breakdown` + 5 siblings.

- **Two backend endpoints, not one.** FR-11's `/setup-tags` endpoint is just distinct recent tags for autocomplete — cheap, called frequently. The chart needs per-setup aggregated stats (`trade_count`, `win_rate`, `avg_r_multiple`, `expectancy`) — more expensive, filter-aware. Conflating them would either (a) make autocomplete slow by returning full aggregates, or (b) make the chart do client-side aggregation of raw trades (wrong — the journal already has backend aggregation for every other breakdown).

- **Case-insensitive dedup is enforced on `journal_tags.name`, NOT on `journal_trades.setup_tag`.** This is subtle. The user may type `"Breakout"` on one trade and `"breakout"` on another — both are persisted as-typed on the `journal_trades.setup_tag` column (FR-3 "display preserves user's casing"). But FR-3 also says "lowercased for dedup comparison," and the existing `journal_tags (user_id, name) UNIQUE` constraint naturally enforces this when auto-tag upserts `LOWER(setup_tag)`. The Setup Breakdown chart must decide: group by raw `setup_tag` (two buckets: "Breakout", "breakout") or by `LOWER(setup_tag)` (one bucket: "breakout"). **Decision**: group by `LOWER(setup_tag)` for the chart to match the tag-system dedup semantics. Surface the lowercased name on the chart (matches what the user sees in the tag badge on the trade). T6's SQL uses `GROUP BY COALESCE(LOWER(setup_tag), '(untagged)')`.

- **Extension autocomplete cache is module-scoped (not SolidJS signal).** TradeForm is remounted every time the modal opens (Shadow DOM is torn down on dismiss). A signal would reset each open. A module-level `let tagCache: { tags: string[]; fetchedAt: number } | null = null;` persists across modal open/close within the same extension-process lifetime. 5min TTL matches spec risk #4.

- **Auto-tag creation is post-commit, not transactional.** Mirrors the draft-notes merge and daily-stats upsert patterns. Rationale: tag creation is auxiliary — the trade record itself is the source of truth. A tag upsert failure should never block trade persistence. Fire-and-forget with `tracing::warn!` on failure.

- **T5 touches both write paths (direct + transactional).** `JournalService::record_trade_close` is used by import workers. `TradeEventWriter::flush_transaction` is used by the live lifecycle path. Both must auto-upsert tags. Extracting a shared `upsert_auto_tag(user_id, trade_id, setup_tag)` helper into a shared module (or a shared fn on `JournalService`) avoids duplication — the spec's DRY axiom. Alternative: only put it on `JournalService` and have `TradeEventWriter` call `journal_service.upsert_auto_tag(...)` post-commit.

- **Double-Enter safety is NOT intercepted by the Setup field.** The confirm-step logic lives at form scope (not per-input). When the Setup field has no suggestion dropdown open, Enter bubbles up to the form-wide handler that increments confirm step. When the dropdown is open with a highlighted suggestion, Enter accepts the suggestion (preventDefault) and does NOT increment confirm step — user must press Enter again (with dropdown closed) to enter confirm-step 1, then Enter once more to confirm. Three Enters worst case. Acceptable — the user can always skip the field entirely with zero Enters.

- **No RSK-01/RSK-01a file paths should be affected.** This spec is orthogonal to the risk snapshot / account consolidation work. `RiskSnapshot` types are untouched. LiveRiskStrip/PulseStrip are already deleted. Overview's hero is already consolidated. Only the ChartSelector option list + one new chart file are added inside the journal.

- **Parallel opportunity NOT taken in this plan:** T3 (autocomplete endpoint), T5 (auto-tag), T6 (analytics endpoint) are independent once T1 lands. A future parallel-tracks pass could dispatch three agents into worktrees after T1's column migration and DTO shape are frozen. For now, sequential execution keeps the plan debuggable and avoids merge conflicts across worktrees. Flagged if pace becomes a concern.

- **No dependency additions.** Zod, alloy, sqlx, ECharts, Solid — all existing versions cover everything. Spec explicitly says "Dependencies Added: None" and the gap analysis confirmed this.

---

## Status

PLANNING COMPLETE

Spec: RSK-02-setup-tag-at-entry
Total Tasks: 10 (T1–T10)
Ready for BUILD mode.

Next task: T1 — Backend migration + DTO passthrough + persistence (adds `setup_tag` column to `journal_trades` + `managed_positions`, threads through `CreateTradeRequest` → `TradeCloseEvent` → INSERT)

---
