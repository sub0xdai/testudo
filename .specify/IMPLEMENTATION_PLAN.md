# Implementation Plan

> Last updated: 2026-04-17
> Current spec: RSK-01-unified-risk-hub
> Phase: BUILD

---

## Active Spec: RSK-01-unified-risk-hub

### Gap Analysis

**Account page (`testudo-journal/src/pages/Account.tsx`):**
- Currently renders only `ExchangeCard` grid + `AddExchangeCard` (max-w-4xl, 2-col grid)
- Per-account balances already fetched via `exchangeApi.fetchBalance(acc.id)` after `listAccounts()` resolves; stored in `balances` signal
- No live aggregate metrics, no per-venue position grouping, no correlation widget
- `ActivePositions.tsx` (`components/trades/`) exists with 30s polling — NOT currently imported anywhere (dormant code)

**Layout (`testudo-journal/src/components/Layout.tsx`):**
- Header has `WalletChip` + `ExtensionChip`; nav items: OVERVIEW / JOURNAL / ACCOUNT
- `isStandalonePage()` already carves out `/pair` from the shell — PulseStrip mount point can use the same carve-out pattern
- Theme system: `amoled` | `light`, persisted to `testudo-theme` localStorage
- No row above the header currently — adding one is a structural change

**Backend (`testudo-exchange/crates/router/`):**
- `routes/exchanges.rs::get_exchange_balance` (line 452) — per-account balance, handles HL native + CEX sidecar fork
- `routes/exchanges.rs::get_exchange_positions` (line 781) — per-account positions, same fork
- `repositories/exchange_account.rs::list_by_user` (line 147) — returns active + inactive-agent-wallet rows
- `services/cex_client.rs::fetch_balance` (line 413) and `fetch_positions` — CEX sidecar wrappers
- `AppState` exposes `pool`, `exchange_account_repo`, `cex_client`, `hl_http_client`, `hl_network`, `analytics_pool`
- No `services/risk_snapshot.rs` yet, no `routes/risk.rs` yet
- `routes/trade_management.rs::list_trades` (line 1189) returns active OrderGroups (managed positions, not exchange positions)

**WebSocket (live-update path for FR-7):**
- Router emits to `pg_notify` channel `order.{user_id}` (`main.rs` line 518)
- `ws-stream` crate exists, subscription types include `order` (`ws-stream/src/types.rs:51`)
- `testudo-journal` has ZERO WebSocket client code — `grep WebSocket` only finds `bun.lock`. New addition required for FR-7.

**Reusable patterns:**
- `PageSubHeader.tsx` — page header with HelpTip
- `StatSection.tsx` — label/value rows with dotted leader line (good template for MarginByVenue rows)
- `HelpTip.tsx` + `HELP` from `lib/help-content.ts` — tooltip pattern
- `formatters.ts` — `formatCurrency`, `formatPercent`, `formatNumber`, `pnlColor`, `rColor`
- `createResource` pattern (Overview line 17, ActivePositions line 9)
- 30s polling pattern (ActivePositions line 12: `setInterval(refetch, 30000)`)
- Account page already uses per-account `Promise.allSettled` balance fan-out — pattern to lift server-side

**Routing (`testudo-journal/src/index.tsx`):**
- `/desk/` → Overview
- `/desk/trades` → Trades (Journal)
- `/desk/journal` → Journal
- `/desk/account` → Account ← target page
- `/desk/pair` → Pair (standalone, no Layout)

### Design Decisions (captured before tasking)

1. **Asset-family bucketing lives backend-side.** Single source of truth in `risk_snapshot.rs` — frontend renders only. Hard-coded `coin → bucket` map (`BTC`, `ETH`, `SOL`, etc. → `BTC-beta`, `ETH-beta`, `alt-L1`, `stables`, `other`). Unit test asserts unknown coin falls into `other`. Spec risk #5 mitigation.

2. **Server-side snapshot cache.** 5s TTL `DashMap<Uuid, (RiskSnapshot, Instant)>` keyed by user_id, in `risk_snapshot.rs`. Avoids fan-out cost on burst refetches from WS push debounce. Spec risk #6 mitigation.

3. **Snapshot endpoint serves both initial fetch and post-event refetch.** No separate `/risk/diff` endpoint. Frontend always re-pulls full snapshot — stateless, simpler, the cache amortizes cost.

4. **Live update strategy.** Build a minimal WS subscriber in `lib/ws.ts` that subscribes to `order.{user_id}` only. Any message → debounced 500ms snapshot refetch. WebSocket disconnect → fall back to 30s polling. `as_of` older than 60s → render `● stale` indicator on PulseStrip.

5. **Pulse Strip placement.** Mount as a sibling of the existing `<header>` in `Layout.tsx` (above it, full-width 1-row strip). Carve-outs: hidden on `/pair`, hidden when `auth.isAuthenticated()` is false, hidden when user pref is off. Default ON per FR-11.

6. **Overview byte-identical preservation.** PulseStrip lives in `Layout`, not in `Overview` — Overview's component tree is unchanged. The "byte-identical" criterion applies to the Overview component output, not the entire viewport.

7. **Frontend fetch strategy on Account page.** Single `createResource(fetchRiskSnapshot)` powers both the existing balance display AND the new strip + widgets. Eliminates the current per-account `Promise.allSettled` fan-out — replaces with one round-trip.

8. **`PositionsByVenue` data source.** Use `snapshot.positions_by_venue` from the new endpoint (which the backend builds from `get_exchange_positions` per account), NOT `fetchActivePositions` (which returns engine-managed OrderGroups). Exchange-side positions are the right source for "what am I exposed to right now" — managed positions only cover trades placed via Testudo.

9. **Effective notional for correlation buckets.** Sum signed notional within each bucket (long = +, short = −). Direction = `long` if all positions same direction, `short` if all same negative, `mixed` otherwise. `effective_notional_usd` = absolute value of net notional within bucket.

10. **CoachBanner shape.** Empty `<div />` (renders null). The slot exists in the layout DOM with reserved padding so RSK-03 can drop content in without re-layout. No conditional rendering — the empty div takes zero vertical space when empty.

### Parallel Track Detection

Three loosely-coupled tracks. Backend (Track A) is fully independent — frontend mocks the response shape until T2 lands. Track B (frontend strip + plumbing) and Track A run in parallel. Track C (account widgets) depends on T6 (real data wired) before starting.

```
Track A (Backend):       T1 → T2 → T3
Track B (Frontend core): T4 → T5 → T6                       (mock until T2 done, then re-point)
Track C (Account body):                  T7 → T8 → T9
Track D (Live + polish):                       T10 → T11
Final:                                                  T12
```

Independent kickoff: T1 (backend types) and T4 (frontend types) can begin same iteration since both define the contract from spec.

---

## Tasks

### Track A: Backend snapshot service

#### T1: Backend types + route stub — `complete`
**Scope:** CP-2 partial — define the wire contract, wire route plumbing, return empty payload.
**Files:**
- `testudo-exchange/crates/router/src/services/risk_snapshot.rs` — NEW. Define `RiskSnapshot`, `VenuePositions`, `VenueMargin`, `CorrelationBucket` structs (Decimal-typed per Constitution §3). Stub `build_snapshot(user_id, &AppState) -> Result<RiskSnapshot, RiskError>` returning a zeroed snapshot.
- `testudo-exchange/crates/router/src/routes/risk.rs` — NEW. Handler `GET /api/v1/risk/snapshot` reads `AuthenticatedUser`, calls `build_snapshot`, returns JSON.
- `testudo-exchange/crates/router/src/routes/mod.rs` — add `pub mod risk;`.
- `testudo-exchange/crates/router/src/services/mod.rs` — add `pub mod risk_snapshot;`.
- `testudo-exchange/crates/router/src/main.rs` — register `web::scope("/risk").wrap(JwtMiddleware::new(token_service.clone())).route("/snapshot", web::get().to(risk::get_snapshot))` inside `/api/v1`.
- `testudo-exchange/crates/router/src/types/exchanges.rs` (or new `types/risk.rs`) — serializable Decimal-as-string fields per CCXT convention used elsewhere in this codebase.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`
**Acceptance:** `curl /api/v1/risk/snapshot` with valid JWT returns a well-formed but zero-valued JSON envelope.

#### T2: Backend aggregation logic — `complete`
**Scope:** CP-2 — real fan-out, real math, asset-family bucketing, 5s server-side cache.
**Files:**
- `testudo-exchange/crates/router/src/services/risk_snapshot.rs` — implement:
  - `build_snapshot()`: `exchange_account_repo.list_by_user(user_id).await`, then `tokio::join!`-style parallel fan-out per active account to (a) balance fetch (mirrors `routes/exchanges.rs::get_exchange_balance` + `get_hyperliquid_balance` logic — extract a shared helper), (b) positions fetch (mirrors `get_exchange_positions` + HL counterpart).
  - Aggregate: `net_exposure_usd = Σ |position.notional|`; `free_margin_usd = Σ venue.free`; `aggregate_leverage = net_exposure_usd / Σ venue.total` (guard div-by-zero → return 0); `long_pct`, `short_pct`, `net_delta_usd` from signed notionals.
  - `correlation_stack`: hard-coded `BUCKETS: &[(&str, &[&str])]` const map (e.g., `("BTC-beta", &["BTC"])`, `("ETH-beta", &["ETH"])`, `("alt-L1", &["SOL", "AVAX", "NEAR", ...])`, `("stables", &["USDT", "USDC", "DAI"])`); coin not found → `"other"` bucket. Effective notional = abs(signed sum within bucket); direction = long/short/mixed per the rule above.
  - 5s TTL cache: `lazy_static`/`OnceLock<DashMap<Uuid, (RiskSnapshot, Instant)>>`. On `build_snapshot`, check cache; if entry younger than 5s → return clone; else recompute.
  - Refactor: extract `fetch_account_balance(acc, &AppState) -> Vec<ExchangeBalanceEntry>` and `fetch_account_positions(acc, &AppState) -> Vec<ExchangePositionEntry>` from `routes/exchanges.rs` into `services/risk_snapshot.rs` (or a new sibling helper) so the route handlers and the snapshot service share one path.
- `testudo-exchange/crates/router/src/routes/risk.rs` — wire to the now-real service.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`
**Acceptance:** Manual curl with a user holding 2 positions on 2 venues returns plausible non-zero aggregate metrics; second curl within 5s is faster (cache hit logged at debug level).

#### T3: Backend integration tests — `complete`
**Scope:** CP-2 — fixture-driven aggregation tests.
**Implementation note:** The router crate is binary-only (no `lib.rs` / `[[lib]]` target), so a top-level `tests/risk_snapshot_test.rs` integration binary can't `use` internal modules without a crate-wide restructure. Instead: extracted a pure `pub fn aggregate_snapshot(positions, margins, as_of) -> RiskSnapshot` from `build_snapshot` (SRP — separates fetch from aggregate), and added the three fixture-driven cases as inline `#[cfg(test)]` tests in `services/risk_snapshot.rs`. Same coverage, same `cargo test risk_snapshot` command, no binary/lib restructure.
**Files:**
- `testudo-exchange/crates/router/src/services/risk_snapshot.rs` — MODIFIED. Extracted `aggregate_snapshot()` pure function. Added 3 fixture cases (empty, single long single venue, multi-venue mixed direction two families). Existing bucket-fallback unit test (`unknown_coin_falls_into_other_bucket`) already covered spec risk #5.

**Validate:** `cd testudo-exchange && cargo test -p router risk_snapshot`
**Acceptance:** 12/12 tests pass (3 new + 9 prior). `cargo clippy --all-targets` clean on risk_snapshot changes (2 pre-existing warnings in cex_client.rs + evaluator.rs unchanged).

---

### Track B: Frontend strip + plumbing

#### T4: Frontend types + API client — `complete`
**Scope:** CP-1 prep — define wire contract on the JS side; safe to do before T2 lands since both work from the spec's type table.
**Files:**
- `testudo-journal/src/api/client.ts` — add `RiskSnapshot`, `VenuePositions`, `VenueMargin`, `CorrelationBucket` interfaces matching backend shape (numerics as strings per repo convention). Add `fetchRiskSnapshot(): Promise<RiskSnapshot>` using existing `fetchWithCredentials` helper.

**Validate:** `cd testudo-journal && bun run build`
**Acceptance:** TS compile clean; callable from a component with full autocomplete.

#### T5: LiveRiskStrip + PulseStrip components (mock data) — `complete`
**Scope:** CP-1 — render aesthetic-fidelity strip + pulse with stub data, verifying brutalist design language match before wiring real data.
**Files:**
- `testudo-journal/src/components/account/LiveRiskStrip.tsx` — NEW. 4-metric horizontal strip (NET EXPOSURE / LEVERAGE / FREE MARGIN / LONG/SHORT). Reuses `font-mono`, `border-container-border`, `bg-container-bg`, `pnlColor`, `formatCurrency`, `formatPercent`. Accepts `snapshot: RiskSnapshot` prop. Mobile (`md:` breakpoint): stacks to 2x2 grid.
- `testudo-journal/src/components/PulseStrip.tsx` — NEW. Single-line strip ≤32px tall: compact `$X · Yx · $Z free` with `● stale` indicator slot (initially not shown). Click handler navigates to `/account` via `useNavigate` from `@solidjs/router`. Accepts `snapshot: RiskSnapshot | null` prop. Mobile: compresses to `$X / Yx`.
- Both components stay pure presentational — no fetch, no resource — so they're easy to mock in T5 and re-point in T6.

**Validate:** `cd testudo-journal && bun run build`
**Acceptance:** Mounted in a temporary scratch route or rendered with hard-coded mock prop, both components match the existing brutalist aesthetic (no glass, no rounded, mono-font, signal palette only).

#### T6: Wire components to real endpoint + Layout mount + Account page composition — `complete`
**Scope:** CP-1 + CP-2 close — replace mocks with `createResource(fetchRiskSnapshot)`, mount strip in Layout, mount LiveRiskStrip on Account.
**Files:**
- `testudo-journal/src/components/Layout.tsx` — add `<PulseStrip snapshot={snapshot()} />` as a sibling above the existing `<header>`. Mount inside the existing `Show when={!isStandalonePage()}` carve-out. Drive from a `createResource(fetchRiskSnapshot)` in Layout (separate from Account's resource — Layout is the long-lived owner). Skip the resource entirely when `!auth.isAuthenticated()`.
- `testudo-journal/src/pages/Account.tsx` — add `<LiveRiskStrip snapshot={snapshot()} />` immediately after the page subheader, above the exchange card grid. Add a Layout-style `createResource(fetchRiskSnapshot)` (or read from a shared signal — see decision #7). Remove the existing per-account `fetchBalances()` + `balances` signal — redundant with snapshot's `margin_by_venue`. Pass per-venue `total` from snapshot down to `ExchangeCard` via a new `balance` prop derivation (snapshot.margin_by_venue.find(...)).
- `testudo-journal/src/lib/help-content.ts` — add `'risk.exposure'`, `'risk.leverage'`, `'risk.margin'`, `'risk.long_short'`, `'risk.pulse'` entries.

**Note on shared state:** Layout and Account both want the snapshot. Two acceptable options — pick the simpler:
- Option A: each owns its own `createResource` (extra round trip but isolated). Reasonable since cache is server-side (T2).
- Option B: lift to a context (`RiskSnapshotContext`) the way `filterContext` is structured. More plumbing but one fetch.
- **Decision: Option A initially.** The 5s server cache (T2) makes the duplicate fetch effectively free. Revisit only if measurable.

**Validate:** `cd testudo-journal && bun run build`
**Acceptance:** Account page shows real aggregate values; PulseStrip visible on every authenticated page (Overview, Journal, Account); both update on page refresh.

---

### Track C: Account body widgets

#### T7: PositionsByVenue widget — `complete`
**Scope:** CP-3 — grouped active positions block, source = `snapshot.positions_by_venue`.
**Files:**
- `testudo-journal/src/components/account/PositionsByVenue.tsx` — NEW. Renders section header with pulse dot + total-position badge; one per-venue subsection with venue header (exchange_name + position count) and table columns: SYMBOL / SIDE / ENTRY / MARK / SIZE / UNREALIZED. Venues with zero positions are filtered out before rendering (keeps the view dense when one venue is idle). Empty-state fallback when total positions = 0 ("No open positions across N venue(s)"). `overflow-x-auto` around the table for narrow viewports. Reuses `bg-container-bg`, `border-container-border`, `formatPrice`, `formatNumber`, `formatCurrency`, `pnlColor`.
- `testudo-journal/src/pages/Account.tsx` — mount `<PositionsByVenue snapshot={snap()} />` inside a `<Show when={snapshot()}>` block **after** the `max-w-4xl` card grid container (full-width at `px-8 pb-10` — matching LiveRiskStrip's pattern rather than the narrower card grid).
- `testudo-journal/src/lib/help-content.ts` — added `'risk.positions_by_venue'`.

**Validate:** `cd testudo-journal && bun run build` ✅
**Acceptance:** Component ready to render venues from `snapshot.positions_by_venue`; empty-state path renders without layout collapse. Live rendering with 2+ open positions across 2 venues is exercised in T12 manual verification.

#### T8: MarginByVenue + CorrelationStack widgets — `complete`
**Scope:** CP-4 + CP-5 — 2-col grid below positions; mobile 1-col.
**Files:**
- `testudo-journal/src/components/account/MarginByVenue.tsx` — NEW. Sorted descending by `free_usd`. Each row: venue name + free on a dotted leader line, with used/total as small subtitle. Header shows total free margin as aggregate hint.
- `testudo-journal/src/components/account/CorrelationStack.tsx` — NEW. Per-bucket row with horizontal bar (width proportional to max bucket's effective notional, min 2% for visibility), direction-coded color (signal-green/red/amber). Contributing symbols listed both as `title` attr (hover) and as visible inline row (`·` separated) — MVP, no statistical correlation.
- `testudo-journal/src/pages/Account.tsx` — composed inside the existing `Show when={snapshot()}` block below `<PositionsByVenue>`, wrapped in `grid grid-cols-1 lg:grid-cols-2 gap-8`. Outer container bumped to `flex flex-col gap-8` so the strip, positions, and grid share one rhythm.
- `testudo-journal/src/lib/help-content.ts` — added `'risk.margin_by_venue'`, `'risk.correlation'`.

**Validate:** `cd testudo-journal && bun run build` ✅
**Acceptance:** Both widgets render side-by-side at `lg` breakpoint, stack at `md` and below; correlation bars scaled to max bucket for visual comparability; help tips present on both section headers. Visual verification with populated data exercised in T12.

#### T9: CoachBanner placeholder slot — `complete`
**Scope:** CP-7 — empty slot reserved for RSK-03.
**Files:**
- `testudo-journal/src/components/account/CoachBanner.tsx` — NEW. Returns `null`. Single-line comment reserves the slot for RSK-03.
- `testudo-journal/src/pages/Account.tsx` — mounted `<CoachBanner />` after the 2-col grid inside the snapshot-gated block.

**Validate:** `cd testudo-journal && bun run build` ✅
**Acceptance:** Build passes; component renders `null` (no DOM artifact); RSK-03 can drop content by editing `CoachBanner.tsx`, leaving `Account.tsx` untouched.

---

### Track D: Live updates + UX polish

#### T10: WebSocket live push + polling fallback + stale indicator — `complete`
**Scope:** CP-6 — sub-2s update on position change.
**Files:**
- `testudo-journal/src/lib/ws.ts` — NEW. Minimal WS client. Connects to `VITE_WS_URL` (default `ws://localhost:4000`), subscribes to `order.{user_id}` using the `WsMessage { method: "SUBSCRIBE", params: ["order.{user_id}"], id: 1 }` shape (matches `ws-stream/src/types.rs:10`). On frames where `stream` starts with `order.`, calls the injected `onRiskEvent` callback. Exponential reconnect backoff 1s → 30s (cap). Exposes `connected: Accessor<boolean>` via `createSignal`. Idempotent `connect(userId)` / `disconnect()` — safe to call repeatedly inside a `createEffect`.
- `testudo-journal/src/components/Layout.tsx` — captures `{ refetch }` from the existing `createResource(fetchRiskSnapshot)`. Debounced refetch (500ms) fires on every WS risk event. Two `createEffect`s: (a) reconciles WS connect/disconnect with `auth.isAuthenticated()` + `auth.user()?.id`; (b) toggles a 30s `setInterval(refetchPulse, 30_000)` polling path when `!wsClient.connected()`. A `now` signal ticks every 10s so `pulseStale()` stays reactive between fetches. All timers (debounce / poll / stale tick) cleaned up in `onCleanup`; the WS client is disconnected there too.
- `testudo-journal/src/components/PulseStrip.tsx` — new optional `connected?: boolean` prop (defaults true for back-compat). Dot states: stale → amber static, connected → green pulsing, polling → green static (no pulse). Same `Show` / `classList` structure — zero snapshot-consumer API changes.
- `testudo-journal/src/api/client.ts` — unchanged (`fetchRiskSnapshot` from T4 reused).
- `VITE_WS_URL` — referenced via `import.meta.env` with an inline default; documented in `ws.ts` header comment.

**Validate:** `cd testudo-journal && bun run build` ✅ (17.92s, no TS or Vite errors)
**Acceptance:** Ready for live QA in T12. With WS connected, a place-order event triggers a single debounced refetch within 0.5s of the frame arrival; WS drops → the 30s polling path takes over; `as_of` older than 60s flips the strip to `● stale` (amber).

#### T11: Pulse Strip preference toggle — `complete`
**Scope:** CP-8 — FR-11 user control.
**Files:**
- `testudo-journal/src/lib/pulse-strip-preference.ts` — NEW. Module-scoped `createSignal` initialized from `localStorage.getItem('testudo-pulse-strip')` (default `'on'`). Exports reader `pulseStripEnabled` (Accessor) and writer `setPulseStripEnabled(next)` which also persists to localStorage. Try/catch around localStorage keeps it safe in private mode.
- `testudo-journal/src/components/Layout.tsx` — import `pulseStripEnabled`; guard `<PulseStrip>` render with `auth.isAuthenticated() && pulseStripEnabled()`.
- `testudo-journal/src/pages/Account.tsx` — compact toggle button in the page subheader's right side: `PULSE STRIP: ON/OFF`. Uses the shared signal — Layout re-renders on toggle, no reload.

**Validate:** `cd testudo-journal && bun run build` ✅
**Acceptance:** Toggling on Account page hides/shows PulseStrip across navigation; preference persists across browser refresh (localStorage key `testudo-pulse-strip`).

---

### Final

#### T12: Mobile + Overview-unchanged + cross-surface verification — `pending`
**Scope:** Acceptance criteria audit + responsive QA.
**Checks:**
- Open Account page at viewport widths 320px, 375px, 768px, 1024px, 1440px — confirm no widget clip, 2-col grid collapses to 1-col at `md`, PulseStrip compresses to `$X / Yx` on mobile.
- Visual diff (or DOM snapshot) of Overview page (`/desk/`) before T6 and after T11 → confirm the `<main>` subtree is byte-identical (PulseStrip lives in Layout outside `<main>`).
- Manual end-to-end: open 2 positions on 2 different exchanges (Hyperliquid + a CEX), observe both reflected in `LiveRiskStrip`, `PositionsByVenue`, `MarginByVenue`, `CorrelationStack`, and `PulseStrip` within 2s of fill.
- Empty-state pass: log out and back in with a fresh user (no exchanges connected) → confirm `0 exp · 0.0x · $0 free` in PulseStrip, no errors, AddExchangeCard onboarding flow still works.
- All builds pass:
  ```bash
  cd testudo-exchange && cargo clippy --all-targets && cargo test
  cd testudo-journal && bun run build
  ```

**Acceptance:** All acceptance criteria from spec checked off; commit with conventional message `feat(rsk-01): unified risk hub on Account page + pulse strip`.

---

## Discoveries

### 2026-04-17 — RSK-01 planning

- **`ActivePositions.tsx` is dormant.** The component exists in `components/trades/` with `fetchActivePositions` + 30s polling, but `Trades.tsx` does not import it (only `TradeTable`). FR-2 ("replacing isolation on Trades page") is technically incorrect — there is no current isolation; the spec's intent is "introduce live position display on Account, grouped by venue." T7 builds this fresh against `snapshot.positions_by_venue` rather than reusing the dormant component, which queries the wrong source (engine-managed OrderGroups, not exchange-side positions).

- **Two position concepts exist; spec wants the second.** `list_trades` (`routes/trade_management.rs:1189`) returns engine-managed `OrderGroup`s — only trades placed via Testudo. `get_exchange_positions` (`routes/exchanges.rs:781`) returns the exchange's actual position state. For "what am I exposed to right now," exchange-side positions are correct (covers manual trades placed elsewhere too). T2 aggregates from `get_exchange_positions`-equivalent path.

- **Per-account fan-out logic is duplicated between Overview.tsx and Account.tsx.** Both call `accounts.map(acc => exchangeApi.fetchBalance(acc.id))`. Lifting this server-side via `RiskSnapshot` eliminates the duplication and the round-trip-per-account cost. T6 removes the Account-page version; Overview's version stays for now (out of spec scope) but a future cleanup could re-point it at `fetchRiskSnapshot`.

- **Journal has no WebSocket client today.** `grep WebSocket` in `testudo-journal/src` returns zero hits. Backend WS plumbing (`ws-stream`) is fully built and emits `order.{user_id}` events; the journal just never subscribed. T10 builds a fresh minimal client. Extension already has a WS client (`testudo-extension/src/background/websocket.ts`) — patterns there can inform but won't be reused (different runtime, different module system).

- **5s server cache is load-bearing for FR-7.** Without it, debounced WS-triggered refetches plus the 30s polling fallback could fan-out to 5+ exchanges every refetch. With the cache, second-and-subsequent fetches within 5s are O(1). The 5s window is short enough that "live" feel survives; long enough that a typical fill event triggers exactly one fan-out.

- **Asset-family map deliberately backend-side.** Spec mentioned a frontend `config/asset_families.ts`. Choosing backend instead because (a) the bucket assignment is part of the snapshot payload (`bucket: string` field), so frontend just renders strings; (b) one source of truth simplifies the unknown-coin fallback contract; (c) future bucket changes don't require shipping new frontend bundles to all surfaces (extension would also benefit from this if it ever consumes the snapshot).

- **`POSITION_PRECISION` for notional sums.** All `*_usd` fields use `Decimal` → string-encoded JSON. Frontend uses `parseFloat` only for display (already standard in the codebase per `formatCurrency`). No precision regression vs current per-account balance flow.

- **No new dependencies required.** All backend math via existing `rust_decimal`. Frontend has no new libraries — `solid-js` resources, native `WebSocket` constructor, existing formatters. Aligns with spec's "Dependencies Added: None expected."

---

## Status

PLANNING COMPLETE

Spec: RSK-01-unified-risk-hub
Total Tasks: 12 (T1–T12)
Tracks: A (backend, T1–T3) ∥ B (frontend core, T4–T6) ∥ C (account widgets, T7–T9) ∥ D (live + polish, T10–T11) → T12 (verification)
Ready for BUILD mode.

Next task: T12 — Verification. Run cross-viewport QA (320/375/768/1024/1440px), confirm Overview `<main>` is unchanged (PulseStrip lives outside it), exercise the empty state with a fresh user, and run `cd testudo-exchange && cargo clippy --all-targets && cargo test` + `cd testudo-journal && bun run build`.
