# Cross-Cutting Operational Rules

> **Scope:** this file holds ONLY cross-cutting rules that apply to every spec
> — tech-stack conventions, shared anti-patterns, protocol decisions that
> transcend individual specs.
>
> **Per-spec discoveries live in `.specify/specs/<spec>/LEARNINGS.md`**
> (append-only by vox build iterations). Vox reads both files on every
> iteration, but writes only to the per-spec file.
>
> Rule of thumb: if a learning would apply to an unrelated future spec,
> lift it here by hand. If it's specific to one spec's architecture, leave
> it in the per-spec file. Git history and `.specify/spec-archive/` preserve
> per-spec postmortems.

---

## Codex Axioms

- KISS, DRY, SoC, SOLID, TDD (Red-Green-Refactor), Five Whys
- `rust_decimal::Decimal` for all financial math — never `f64`
- `Result<T, E>` everywhere; no `unwrap()` in production code

## Git

- Commit messages: `type: description` (feat, fix, refactor, docs, test, chore)
- No `Co-Authored-By` trailers in this repo
- Always stage specific files; never `git add -A` or `git add .`
- Don't commit broken intermediate states — if a task's changes break callers,
  bundle dependent tasks into one atomic commit (plan-sanctioned bundling).

---

## Financial Math & Wire Format

- `Decimal` serializes as a JSON **string** by default (`rust_decimal` 1.3x+).
  All wire types use `string` on the TS side. `Decimal::ONE` serializes as
  `"1"`, not `"1.0"` — watch this in assertions.
- `dec!(…)` macro works in `const` position — use it for threshold constants.
- Division-by-zero guards are mandatory in any helper that divides
  user-provided Decimals (losses, quantities, baselines).
- `Uuid` → `string` on the wire. `DateTime<Utc>` → RFC3339 ISO string.

## Decimal conventions with frontend

- When values feed math or formatters on the frontend, accept both
  `z.string()` and `z.number()` via a coercing union and `.transform(Number)`.
  Pure-display decimals can stay as `string`.

---

## Rust Backend Patterns

- `BTreeMap` for orderbook, `DashMap` for lock-free per-user balance access.
- `OnceLock<DashMap<…>>` > `lazy_static!` for singleton caches. `OnceLock`
  also handles "compute once" values (e.g. reference constants).
- `Arc<dyn Trait>` already carries `Send + Sync` when the trait is
  `Trait: Send + Sync`. Don't spell it twice.
- Prefer `pub(crate) fn` free helpers over methods on services when the
  helper is shared across unrelated services. Matches the established
  `compute_derived_fields` / `canonical_exchange_name` / `shrink` pattern.
- Pure / async split: extract the pure decision logic from any DB-bound
  service method so it is unit-testable without a Postgres fixture
  (e.g. `aggregate_snapshot` ↔ `build_snapshot`, `compose_digest` ↔
  `build_digest`, `compute_sizing_preview` → handlers).
- Router is binary-only (no `src/lib.rs`) — top-level `tests/` directories
  cannot `use router::…`. Keep inline `#[cfg(test)] mod tests` per module.
- `sqlx::query_as::<_, (T,)>` tuple destructuring is idiomatic for
  single-column reads; don't mint a 1-field `FromRow` struct.
- `ON CONFLICT DO UPDATE SET col = EXCLUDED.col RETURNING …` is the
  upsert-and-always-return-id idiom. `DO NOTHING` skips RETURNING.
- `#[serde(default)]` on newly added struct fields — defends against
  legacy serialized blobs (pg_queue buffers, WS reconnects, rehydration).
- Errors `Box<dyn Error + Send + Sync>` at service boundaries is an
  acceptable stub for new modules — narrow to a typed enum when the shape
  stabilizes. The router crate has `thiserror`, not `anyhow`.
- `dev-dependencies` are invisible to prod modules. If a crate is used by
  a non-test call site, it must be in `[dependencies]`.

## Actor / Engine Conventions

- `OrderGroup` (in-memory actor state) is the source of truth for
  TradeClosed payloads — `fill_detector::emit_trade_closed` reads from
  `&OrderGroup`, not `managed_positions`. Any new field that must reach
  the journal/analytics path needs to land on `OrderGroup` via
  `EngineCommand::ConfigureGroup`.
- `ConfigureGroup` is the single canonical write site for optional group
  metadata. Additions follow the same shape
  (`if value.is_some() { group.field = value; }`).
- `managed_positions` is the secondary store used for rehydration after
  router restart. New `OrderGroup` fields usually need a parallel column
  there (and the rehydration path) or be explicitly declared MVP-only
  non-durable.

## Database & Persistence

- Canonical exchange name: always apply `common_utils::models::canonical_exchange_name`
  (`name.trim().to_lowercase()`) at every INSERT site that writes the
  exchange key into the journal / idempotency path. Two call sites today:
  `JournalService::record_trade_close` and
  `TradeEventWriter::insert_journal_trade`.
- Partial unique index: `idx_unique_import_fill` on
  `(user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL`.
  `ON CONFLICT` must repeat the `WHERE` predicate verbatim — Postgres
  cannot resolve partial-index conflicts otherwise. Live-trade inserts
  (exchange_fill_id = None) are excluded by design.
- Never use a synthetic timestamp as a stable idempotency key. If an
  exchange returns unparseable IDs, skip the row and bump a counter —
  do not paper over with `timestamp as i64` or similar.
- Idempotent INSERTs use structural outcomes, not error-string matching
  (`RecordOutcome::Inserted | SkippedDuplicate` over
  `err.contains("duplicate key")`). The `Box<JournalTrade>` enum variant
  avoids `large_enum_variant` clippy.
- `journal_trades`: decimal columns always nullable-only when meaningfully
  optional (e.g. `r_multiple` depends on SL presence); `JSONB` columns get
  a DDL-level `DEFAULT '...'::jsonb` so `INSERT ON CONFLICT DO NOTHING`
  paths don't need app-side initializers.
- Two pools on `AppState`: `pool` (writes, auth, trades) and
  `analytics_pool` (read-optimized, every `/analytics/*` endpoint). Wrong
  pool is an easy and silent bug.
- `ON DELETE CASCADE` on any per-user auxiliary table's user_id FK —
  privacy posture is "user controls what leaves the server".

---

## Extension / Frontend Patterns

- Runtime validation: Zod schemas in `src/schemas.ts` are the single
  source of truth for runtime types. TS types derive via
  `z.infer<typeof Schema>`.
- **Content scripts**: avoid bundling heavy deps. Do NOT `import { z }` in
  `scraper.ts` / `content.ts` — it pulls full Zod + locale files. Use
  plain TS runtime checks for the 3-5 simple validators that run there.
- Content scripts on MV3 can drop `webextension-polyfill` and use
  `const browser = (globalThis as any).browser ?? (globalThis as any).chrome;`
  — native Promise-based APIs work on both Chrome and Firefox. Background
  worker keeps the polyfill.
- Token storage: `chrome.storage.session` (JWTs) + `chrome.storage.local`
  (settings, prefs). Session storage auto-clears on browser close.
- Extension uses `/auth/extension-refresh` (JSON body); web/journal use
  `/auth/refresh` (HttpOnly cookie). Never cross the wires.
- Module-scoped caches (TTL'd `let cache: {…} | null = null`) survive
  modal remounts (Shadow DOM teardown) but not content-script reload.
  Good fit for autocomplete / user-settings flags (5-min TTL).
- SolidJS: `<For>` over `.map()` for idiomatic signal reactivity;
  `createResource` with a reactive source function gates fetches on auth;
  `createEffect` is idempotent — use it for lifecycle logic with safe
  repeat semantics.
- Shadow DOM isolation: modal needs its own `[data-theme]` attribute on
  the host element — it cannot inherit from the outer document.
- Verification command: `bun run typecheck` in `testudo-extension/`
  (never `bun run build` on the extension during verification — extension
  defaults must stay prod-URL; see extension rules).

---

## Trading / Exchange

- Order placement: entry = limit (non-reduce-only); SL = stop-market with
  stopPrice (reduce-only); TP = limit (NOT reduce-only — WOO rejects
  reduce-only before position exists).
- `clientOrderId`: `testudo:{group_id}:{role}` where role = entry | sl | tp.
- All CCXT sidecar numerics transmitted as strings; all `SidecarOrderResponse`
  fields like `status`, `filled`, `remaining` are `Option<String>` (WOO
  returns null).
- Exchange status normalization: `normalize_status()` in `exchange_api.rs`
  maps SDK variants to CCXT strings (`"closed"` = filled, `"open"` = resting).
  Never `format!("{:?}", ExchangeDataStatus)` — Debug leaks internal shape.
- HL trigger orders need explicit `trigger_limit_px()` (10% slippage band);
  SDK default is a broken `"0"`.
- HL fills: match closed positions on `closedPnl != "0"` / `dir` semantics,
  not on order ID — WaitingForTrigger OIDs do not match final fill OIDs.
- Position sizing: conservative wins —
  `MIN(account%, fixed risk, max size, margin capacity)`.

---

## Shared Anti-Patterns

- Don't cancel `Active` order groups in cleanup — only `Pending` ghosts.
- Don't cache a `balance?` field pre-synthesized on the page and thread
  it through component props — let cards look up venue-specific data from
  the snapshot directly (single source of truth).
- Don't use `format!("{:?}", …)` for any value that crosses a wire
  contract or a CCXT-style normalization gate.
- Don't invent "test-only" struct fields or methods in production code —
  use the `Default` derive or a builder.
- Don't `amend` commits when pre-commit hooks fail; create a new commit
  with the fix.

---

## Test / Verification Workflow

- TDD: Red → Pause (map the connections) → Green → Refactor.
- Background `run_in_background: true` Bash tasks must use absolute paths —
  the background runner CWD differs from the session CWD. `cd <relative>
  && cmd` silently fails with `no such file or directory`.
- `cargo test | tail -N` truncates per-crate summaries. For full counts
  omit the tail pipe or re-read the raw output file. Exit code is still
  accurate.
- Manual QA deferrals are normal for autonomous runs — anything that
  needs a live exchange / real signed wallet session is documented in the
  spec's plan Status section as "deferred to live session", not shimmed.
- Integration tests against live Postgres use `#[tokio::test] #[ignore]`
  + env-var `DATABASE_URL` (workspace `sqlx` dep lacks the `macros`
  feature) — `#[sqlx::test]` would require a cross-cutting dep bump.
- Cleanup order in DB integration tests matches FK direction: children
  before parents. Use `let _ = ...` for idempotent cleanups that tolerate
  "row not found".

---

## Opus 4.7 Ergonomics

- Large `create_trade`-scale handlers (~400 lines, multi-subsystem) are
  a context hazard. Prefer extracting pure helpers (`sizing_preview`,
  `prepare_report`) and keeping the HTTP handler thin.
- `AGENTS.md` blowups are the visible symptom of per-spec context
  pollution; keep entries here timeless, archive dated journals to
  per-spec LEARNINGS.md.
