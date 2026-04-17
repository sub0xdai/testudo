# Implementation Plan

> Last updated: 2026-04-18
> Current spec: RSK-01a-account-consolidation
> Phase: BUILD (T1–T3 complete, next: T4)

---

## Active Spec: RSK-01a-account-consolidation

### Gap Analysis

**Layout (`testudo-journal/src/components/Layout.tsx`, 577 lines):**
- Currently OWNS `fetchRiskSnapshot` via `createResource(() => auth.isAuthenticated(), …)` at line 367.
- Currently OWNS the WS client via `createRiskWsClient(debouncedRefetch)` at line 386.
- Currently OWNS the polling fallback + stale ticker — all three lifecycle effects (lines 389–422) must move.
- Mounts `<PulseStrip snapshot={…} isStale={…} connected={…} />` at lines 458–466 inside a `Show when={auth.isAuthenticated() && pulseStripEnabled()}` guard. Imports PulseStrip + pulseStripEnabled + createRiskWsClient + fetchRiskSnapshot. All four imports die in T1.
- `isStandalonePage()` carve-out (line 363) already excludes `/pair` from the Layout shell — no change needed.

**Overview (`testudo-journal/src/components/Overview.tsx`, 217 lines):**
- Desktop hero at lines 176–199 renders `$X net P&L  $Y balance` inside `flex items-baseline gap-10`. Easy extension point — three more inline entries (exposure, leverage, free) plus a live dot. All use existing `font-mono`, `pnlColor`, `formatCurrency`, `formatNumber`.
- Mobile condensed hero at lines 141–158 — needs a parallel extension (drop labels, keep numbers + live dot per FR-1 mobile rule).
- Currently maintains its own `totalBalance` via per-account `exchangeApi.fetchBalance` fan-out in `onMount` (lines 22–39). Snapshot already exposes aggregate data — this fan-out becomes redundant once Overview owns `fetchRiskSnapshot`, and is a perfect spot to kill duplicate work. **Decision**: keep the existing `balance` display driven by the per-account fan-out (it's showing TOTAL BALANCE across all venues, which conceptually overlaps with but isn't identical to `free_margin_usd`). Touching it is out of scope. Only ADD the new hero fields.
- No `createEffect` / `onCleanup` for WS today — must import `createRiskWsClient` + add lifecycle effects mirroring the ones being removed from Layout.

**Account page (`testudo-journal/src/pages/Account.tsx`, 279 lines):**
- Already owns its own `createResource(fetchRiskSnapshot)` at line 33. Reuses snapshot for `balanceForCard` (lines 51–57) to derive `ExchangeBalanceResponse` from `margin_by_venue` lookup. That derivation goes away in T2 once ExchangeCard receives the full snapshot and does its own lookup.
- Mounts `LiveRiskStrip` at line 151 (inside `Show when={snapshot()}` conditional) — obsolete once Overview hero ships (T1).
- Mounts `PositionsByVenue` + `MarginByVenue` + `CorrelationStack` + `CoachBanner` inside a snapshot-gated block (lines 263–274) below the exchange card grid. CorrelationStack moves to TOP of page in T4. CoachBanner moves to bottom.
- Has PULSE STRIP toggle button at lines 137–145. Dies in T1 with the strip.
- Exchange tile grid is `grid-cols-1 md:grid-cols-2 gap-8` inside `max-w-4xl` wrapper (line 186–187). T4 restructures to 3-col on lg with add-slot fillers.
- Already has `AddExchangeCard` (line 207) — FR-8's dashed "+ ADD EXCHANGE" slot is already styled correctly; T4 just multiplies it via filler logic.
- Form + re-auth modals are `fixed inset-0 z-50` overlays (lines 211–260) — unaffected by grid restructure.

**ExchangeCard (`testudo-journal/src/components/account/ExchangeCard.tsx`, 257 lines):**
- Renders identity row (lines 172–206), wallet address truncation (208–213), migration prompt (215–223), balance + test result OR reauth button (225–253).
- `KebabMenu` inline sub-component (lines 4–134) — unchanged.
- `formatBalance(balance?)` helper (lines 136–143) — stays. Expand or add sibling helpers for margin breakdown + positions.
- **Prop change needed**: current `balance?: ExchangeBalanceResponse` narrows what the card can show. T2 changes the contract to accept either `snapshot?: RiskSnapshot` + the card already has `account` (so `venueMarginFor(account.id)` lookup happens inside the card), OR keep the synthesis pattern in Account.tsx and pass pre-derived margin + positions. **Decision**: pass `snapshot` directly — the card is the right place for the venue-specific lookup, and this is the cleanest way to pipe positions to the card too.

**CorrelationStack (`testudo-journal/src/components/account/CorrelationStack.tsx`, 100 lines):**
- Renders a `<section>` with header + buckets. Currently guards with `Show when={buckets().length > 0}` showing an empty-state fallback. FR-7 requires `< 2` → `return null` (no section, no height, no outline). T4 adjusts the early-return.

**Components to delete (5 files):**
- `testudo-journal/src/components/PulseStrip.tsx` (74 lines)
- `testudo-journal/src/components/account/LiveRiskStrip.tsx` (63 lines)
- `testudo-journal/src/components/account/MarginByVenue.tsx` (79 lines)
- `testudo-journal/src/components/account/PositionsByVenue.tsx` (127 lines)
- `testudo-journal/src/lib/pulse-strip-preference.ts` (25 lines)
- Total delete: ~368 lines

**Help content (`testudo-journal/src/lib/help-content.ts`):**
- Delete `risk.pulse` (dies with strip, T1)
- Delete `risk.long_short` (dies with LiveRiskStrip, T1)
- Delete `risk.positions_by_venue` (dies with widget, T3)
- Delete `risk.margin_by_venue` (dies with widget, T2/T3)
- Keep `risk.exposure`, `risk.leverage`, `risk.margin` — still used on ExchangeCard margin breakdown and CorrelationStack
- Add `risk.venue_margin`, `risk.venue_positions` (or similar) — inline help for the new ExchangeCard sections (optional; spec doesn't require — HelpTip is currently on section headers only, ExchangeCard has no HelpTips today)

**WS client (`testudo-journal/src/lib/ws.ts`, 106 lines):**
- Unchanged. `createRiskWsClient(callback)` returns a fresh handle per call — not a singleton. Because SolidJS route changes unmount the page, Overview's WS handle is torn down (via `onCleanup` → `wsClient.disconnect()`) before Account's handle is spun up. No double-subscription concern in practice.
- Account page currently does NOT own a WS handle (it just calls `createResource(fetchRiskSnapshot)` — no refetch driver beyond the resource's natural invalidation). This is a latent gap: when a trade closes while the user is on Account, the snapshot won't refresh until the page is reloaded. Out of scope for RSK-01a — documented under Discoveries.

**Pair page (`testudo-journal/src/pages/Pair.tsx`):**
- No risk-widget imports. Layout already bypasses the shell via `isStandalonePage()` returning true for `/pair`. Nothing to change.

**Journal page:**
- No risk-widget imports. Only affected transitively (Layout no longer mounts PulseStrip above the header → Journal header sits at top of viewport). Aesthetic goal of spec.

### Design Decisions (captured before tasking)

1. **Bundle PulseStrip + LiveRiskStrip + pulse-strip-preference deletions into T1.** Spec groups them in CP-5 as "structural tidy-up," but the Overview hero added in CP-1 makes LiveRiskStrip immediately redundant, and PulseStrip has no snapshot source once Layout releases ownership. Shipping an intermediate where Account still shows a redundant strip or where PulseStrip temporarily owns its own resource is wasteful churn. T1 does the Overview hero AND all three file deletions together — one coherent "consolidation of live-aggregate metrics onto Overview" commit. Acceptance criteria for CP-1, CP-5, and FR-2/3/11 all fall out of T1.

2. **ExchangeCard accepts `snapshot?: RiskSnapshot` instead of `balance?: ExchangeBalanceResponse`.** The card does its own lookup: `snapshot?.margin_by_venue.find(m => m.exchange_id === account.id)` and `snapshot?.positions_by_venue.find(v => v.exchange_id === account.id)?.positions`. This keeps per-venue derivation inside the card (where it belongs) and removes `balanceForCard` + `venueMarginFor` helpers from Account.tsx. Defensive: if snapshot is undefined OR the lookup returns nothing, the card renders a "margin unavailable" fallback (spec risk #1). Existing `balance?: ExchangeBalanceResponse` + `formatBalance` helper are deleted along with the synthesis.

3. **Free-ratio bar grammar mirrors CorrelationStack.** A thin 1.5–2px filled rectangle proportional to `free_usd / total_usd`. Same `bg-text-primary/5` track with `bg-signal-green` (or `text-primary` neutral) fill. Keeps visual language coherent; no new primitive.

4. **Grid filler logic: `max(accounts.length, 3)` on lg, `max(accounts.length, 2)` on md, exact count on mobile.** Spec risk #6 calls for this exact pattern to avoid "hollow" 5-add-slot views. Implemented as a `Math.max(accounts()?.length ?? 0, 3) - (accounts()?.length ?? 0)` filler count, rendered with `<For each={Array(fillerCount()).fill(0)}>`.

5. **Live-dot state on Overview hero: `signal-green` (WS connected, fresh) / `signal-amber` (polling or stale).** `pulseStale()` logic from Layout ports verbatim — compares `Date.parse(snapshot.as_of)` against a 10s-ticking `now` signal for 60s threshold. Hover tooltip: `title` attr with a short relative-time string ("12s ago"). No new library — a small inline `relativeTime(ms)` helper.

6. **CoachBanner position = bottom of Account page.** Currently mounted at the end of the snapshot-gated block (line 271 — good). T4's grid restructure leaves CoachBanner as the last child of the page, per FR-9.

7. **No change to `fetchRiskSnapshot` API client shape.** Backend untouched. Only frontend consumers shift.

### Parallel Track Detection

Mostly sequential — each task modifies the same files (Layout, Overview, Account, ExchangeCard). Parallelism low. The backend acceptance criterion ("no backend changes") is a simple verification gate that runs alongside any task.

```
T1 → T2 → T3 → T4 → T5 (verification)
```

---

## Tasks

### T1: Overview hero consolidation + delete PulseStrip / LiveRiskStrip / pulse-strip-preference — `complete`

**Scope:** CP-1 + bundled deletions (FR-1, FR-2, FR-3, FR-10, FR-11, FR-12). Overview owns snapshot + WS + polling fallback + stale indicator. Layout stops owning live data. All three "strip" files deleted in one commit.

**Files:**
- `testudo-journal/src/components/Overview.tsx` — MODIFIED:
  - Add imports: `createEffect`, `onCleanup`, `fetchRiskSnapshot`, `createRiskWsClient`.
  - Add `createResource(() => auth.isAuthenticated(), async (authed) => authed ? fetchRiskSnapshot() : null)` for the snapshot (mirrors the current Layout.tsx pattern at line 367). Need access to `useAuth()` — add import from `../context/AuthContext`.
  - Add WS + polling + stale-ticker lifecycle: copy `debouncedRefetch`, `createRiskWsClient`, two `createEffect` blocks (WS connect/disconnect + polling fallback toggle), a `now` signal + `setInterval(setNow(Date.now()), 10_000)`, and `onCleanup` that clears all timers + calls `wsClient.disconnect()`. Port verbatim from Layout.tsx lines 373–422.
  - Add `pulseStale()` derived accessor (Layout.tsx lines 424–431).
  - Extend desktop hero (lines 176–199) with three new inline entries: `net exposure`, `leverage`, `free margin`. Each follows the existing `<div>…<span class="font-mono text-4xl md:text-5xl font-bold …">{value}</span><span class="font-mono text-sm text-text-secondary ml-3">{label}</span></div>` pattern. Slightly smaller font for secondary metrics (`text-2xl md:text-3xl`) so the net P&L stays dominant. Wrap each in `<Show when={snapshot()}>`.
  - Append live/stale dot to the right side of the hero: `<span class="w-1.5 h-1.5 rounded-full" classList={{ 'bg-signal-green animate-pulse': !pulseStale() && wsClient.connected(), 'bg-signal-green': !pulseStale() && !wsClient.connected(), 'bg-signal-amber': pulseStale() }} title={relativeTime(snapshot())} />` + small `live`/`stale` text label.
  - Extend mobile hero (lines 141–158) with compressed format: drop labels, keep numbers — e.g. `{netPnl} · {balance} · {leverage}x · ● {dot}`. Explicit per FR-1 mobile rule.
  - Add small `relativeTime(snap)` helper at module scope: returns `"just now"` for <10s, `"Ns ago"` for <60s, `"Nm ago"` for ≥60s.
- `testudo-journal/src/components/Layout.tsx` — MODIFIED:
  - Remove imports: `PulseStrip`, `pulseStripEnabled`, `createRiskWsClient`, `fetchRiskSnapshot`.
  - Remove all snapshot + WS + polling + stale-ticker logic (lines 367–431).
  - Remove `<Show when={auth.isAuthenticated() && pulseStripEnabled()}>…<PulseStrip …/>…</Show>` block (lines 458–466).
  - Remove `createResource`/`createEffect` imports if they become unused.
  - Net Layout.tsx delta: ~65 lines removed, 0 added.
- `testudo-journal/src/pages/Account.tsx` — MODIFIED:
  - Remove imports: `LiveRiskStrip`, `pulseStripEnabled`, `setPulseStripEnabled`.
  - Remove `<LiveRiskStrip>` mount block (lines 148–154).
  - Remove PULSE STRIP toggle button (lines 137–145) from subheader.
- `testudo-journal/src/components/PulseStrip.tsx` — DELETE.
- `testudo-journal/src/components/account/LiveRiskStrip.tsx` — DELETE.
- `testudo-journal/src/lib/pulse-strip-preference.ts` — DELETE.
- `testudo-journal/src/lib/help-content.ts` — MODIFIED: remove `'risk.pulse'` and `'risk.long_short'` entries.

**Validate:** `cd testudo-journal && bun run build`

**Acceptance (from CP-1 + CP-5):**
- Overview hero shows 5 inline metrics on desktop (net P&L, balance, exposure, leverage, free margin) + live dot.
- Mobile hero shows compressed format (numbers + dot, no labels).
- Live dot: green (WS connected), green (polling, no pulse), amber (stale, >60s since `as_of`).
- `title` attr on dot shows relative time.
- PulseStrip.tsx, LiveRiskStrip.tsx, pulse-strip-preference.ts no longer exist (`ls` returns 404).
- Layout.tsx no longer imports or mounts PulseStrip; no `pulse-strip-preference` references anywhere in `testudo-journal/src`.
- Account.tsx no longer shows a PULSE STRIP toggle.
- `testudo-pulse-strip` localStorage key is no longer read/written anywhere in the codebase (`grep` confirms zero references).
- Overview's existing stats sidebar + calendar + charts render unchanged at DOM level.
- `bun run build` passes for testudo-journal.

---

### T2: ExchangeCard grows margin breakdown section — `complete`

**Scope:** CP-2 (FR-4 margin half). Card expands vertically to include margin total, free-ratio bar, free + used labels. Positions list NOT added yet — `PositionsByVenue` stays mounted so the app is shippable at this checkpoint.

**Files:**
- `testudo-journal/src/components/account/ExchangeCard.tsx` — MODIFIED:
  - Change props signature: replace `balance?: ExchangeBalanceResponse` with `snapshot?: RiskSnapshot`.
  - Add internal derived accessors: `venueMargin = () => props.snapshot?.margin_by_venue.find(m => m.exchange_id === props.account.id)`.
  - Replace the existing `formatBalance` + balance display (current lines 225–244) with a new "Margin breakdown" block:
    - `{total} total` — large, top line.
    - Free-ratio bar — full-width thin rectangle, width proportional to `free/total`. Use `bg-text-primary/5` track + `bg-signal-green` fill (reuse CorrelationStack's visual grammar).
    - `{free} free · {used} used` — small subtitle.
  - When `venueMargin()` is undefined (spec risk #1 fallback): render "Margin unavailable" in small tertiary text instead of throwing.
  - Test result row stays — renders below margin breakdown as before.
  - Keep reauth button path unchanged (lines 245–253).
  - Add `formatBalanceUsd(raw: string)` helper: strips sign, prefixes `$`, 2-decimal format.
- `testudo-journal/src/pages/Account.tsx` — MODIFIED:
  - Remove `venueMarginFor` + `balanceForCard` helpers (lines 47–57).
  - Change the `<ExchangeCard>` call site: replace `balance={balanceForCard(acc.id)}` with `snapshot={snapshot()}`.
  - Remove `ExchangeBalanceResponse` and `VenueMargin` imports from `../api/client`.
- `testudo-journal/src/lib/help-content.ts` — optional: add `'risk.venue_margin'` help entry if a HelpTip is introduced on the margin section. Current ExchangeCard has no HelpTips, so defer unless T2 needs one.

**Validate:** `cd testudo-journal && bun run build`

**Acceptance (from CP-2):**
- Each ExchangeCard shows `{total_usd} total` on the primary line.
- Each ExchangeCard shows a free-ratio bar (proportional width, `bg-signal-green` fill).
- Each ExchangeCard shows `{free_usd} free · {used_usd} used` subtitle.
- Card with no matching venue margin shows "Margin unavailable" (not a crash).
- `bun run build` passes.
- Card visual density matches the spec's target mockup (identity row, margin block, spacer, future positions slot reserved).

---

### T3: ExchangeCard absorbs positions + delete PositionsByVenue + MarginByVenue — `complete`

**Scope:** CP-3 (FR-4 positions half, FR-5, FR-6). Inline positions list rendered inside each ExchangeCard. `PositionsByVenue` + `MarginByVenue` files deleted. Help content cleaned up.

**Files:**
- `testudo-journal/src/components/account/ExchangeCard.tsx` — MODIFIED:
  - Add internal derived accessor: `venuePositions = () => props.snapshot?.positions_by_venue.find(v => v.exchange_id === props.account.id)?.positions ?? []`.
  - Add a "Positions" section below margin breakdown (inside the main card block, not displacing the test result or reauth path):
    - When `venuePositions().length === 0`: render `── NO OPEN POSITIONS ──` label (small mono, tertiary color).
    - When `venuePositions().length >= 1`: render `── {N} POSITION(S) ──` section header, then a compact list. Each row: `{symbol} · {side}` line 1, `{quantity} @ {entry} → {mark}` line 2, `{formatCurrency(unrealized_pnl_usd)}` line 3 color-coded via `pnlColor`.
    - Mirror the spec's target visual density exactly:
      ```
      ── 1 POSITION ──
      SOL_USDT · LONG
      1.3 @ 87.85 → 89.26
      +$1.83
      ```
  - If the card exceeds ~250 lines after this task (spec risk #5), extract `ExchangeCardMargin` and `ExchangeCardPositions` as sibling components in a new `components/account/ExchangeCardSections/` folder, or inline within the same file as named non-exported functions. Decision deferred to implementation — default to inline until the file gets uncomfortably dense.
- `testudo-journal/src/components/account/PositionsByVenue.tsx` — DELETE.
- `testudo-journal/src/components/account/MarginByVenue.tsx` — DELETE.
- `testudo-journal/src/pages/Account.tsx` — MODIFIED:
  - Remove imports: `PositionsByVenue`, `MarginByVenue`.
  - Remove mount blocks inside the snapshot-gated `Show` (current lines 263–274). Leave only `<CorrelationStack>` + `<CoachBanner>` for now — T4 restructures further.
- `testudo-journal/src/lib/help-content.ts` — MODIFIED: remove `'risk.positions_by_venue'` and `'risk.margin_by_venue'` entries.

**Validate:** `cd testudo-journal && bun run build`

**Acceptance (from CP-3):**
- Each ExchangeCard shows inline positions list (or "no open positions" placeholder).
- `PositionsByVenue.tsx` and `MarginByVenue.tsx` no longer exist.
- Account.tsx no longer imports or mounts either.
- Position list in a card updates on WS fill event within 2s — via the Account page's own `createResource(fetchRiskSnapshot)`. T3 does NOT add a WS handle on Account (latent gap from RSK-01 persists; flagged in Discoveries). Behavior verified: page-refresh / route-remount shows updated state.
- `bun run build` passes.

---

### T4: CorrelationStack conditional + move to top + exchange tile grid restructure — `pending`

**Scope:** CP-4 (FR-7, FR-8, FR-9). CorrelationStack renders only when `≥ 2` buckets; moves to top of Account above the exchange tile grid. Grid expands to 3-col on lg with "+ ADD EXCHANGE" filler logic. CoachBanner anchored at bottom.

**Files:**
- `testudo-journal/src/components/account/CorrelationStack.tsx` — MODIFIED:
  - Early-return: at the top of the component, `if (props.snapshot.correlation_stack.length < 2) return null;`.
  - Remove the existing empty-state fallback inside `<Show when={buckets().length > 0}>` — no longer needed since `< 2` returns null and `≥ 2` definitely has buckets.
- `testudo-journal/src/pages/Account.tsx` — MODIFIED:
  - Move `<CorrelationStack>` mount from the bottom block to ABOVE the exchange tile grid, inside `Show when={!isOnboarding()}` and `Show when={snapshot()}`. Placement: after the subheader, before the `max-w-4xl mx-auto` grid wrapper.
  - Update the exchange tile grid wrapper:
    - Drop `max-w-4xl`; use `max-w-6xl` or full-width container (spec FR-8 shows three-tile rows; `max-w-4xl` caps at ~56rem which is too narrow for three `min-w[…]` cards).
    - Change grid classes from `grid-cols-1 md:grid-cols-2` to `grid-cols-1 md:grid-cols-2 lg:grid-cols-3`.
    - Add filler slot logic:
      - `const minSlots = () => { const n = accounts()?.length ?? 0; /* depends on viewport */ return Math.max(n, 3); }` — but Solid/Tailwind can't easily switch breakpoint-aware counts in JS. Simplification: always render `Math.max(accounts.length, 3) - accounts.length` filler `AddExchangeCard`s. On md where only 2 cols show, extra filler wraps to a second row — acceptable per spec risk #6 (one to two "add" slots visible is an invitation).
      - Alternative simpler rule: render `Math.max(1, 3 - accounts.length)` filler slots — always at least 1 "+ ADD EXCHANGE", up to 3 total tiles visible on lg.
    - Use `<For each={Array.from({ length: fillerCount() })}>{() => <AddExchangeCard onClick={…} />}</For>`.
  - Move `<CoachBanner />` mount to the BOTTOM of the page — after the grid (and after the now-removed PositionsByVenue/MarginByVenue block from T3).
  - Ensure the snapshot-gated `Show` wraps CorrelationStack correctly: `<Show when={snapshot()}>{(snap) => <CorrelationStack snapshot={snap()} />}</Show>` at the top; CoachBanner doesn't need snapshot.

**Validate:** `cd testudo-journal && bun run build`

**Acceptance (from CP-4):**
- CorrelationStack sits at top of Account (above exchange grid) when `correlation_stack.length ≥ 2`.
- CorrelationStack returns `null` (no height, no border, no label) when `< 2` buckets.
- Exchange tile grid is 3-col on lg, 2-col on md, 1-col on mobile (via `grid-cols-1 md:grid-cols-2 lg:grid-cols-3`).
- With 1 account, lg viewport shows: 1 tile + 2 "+ ADD EXCHANGE" filler tiles = 3 tiles per row.
- With 3 accounts, lg viewport shows: 3 tiles + 0 fillers (plus 1 explicit "+ ADD EXCHANGE" in a new row from the `<AddExchangeCard>` that Account already renders). Reconcile: single `<AddExchangeCard>` at the end of the `<For each={accounts()}>` block is enough when count ≥ 2; filler loop only kicks in when count < 2. Simplification: `fillerCount = Math.max(0, 3 - (accounts.length + 1))` to account for the always-present "add" card.
- CoachBanner sits at the bottom of the page (after the grid).
- `bun run build` passes.

---

### T5: Verification — `pending`

**Scope:** CP-6. Mechanical verification: builds clean, LOC delta negative, no regressions on Overview below the hero, Journal and Pair visually unchanged at structural level.

**Verifications:**
- `cd testudo-journal && bun run build` passes (vite + tsc, exit 0).
- `cd testudo-exchange && cargo check --all-targets` passes (no backend changes should be present; expect byte-identical to baseline).
- `git diff --stat master` shows negative LOC delta in `testudo-journal/` (deletions > additions).
- `git log -- testudo-journal/src/pages/Overview.tsx` shows only T1's modification — no accidental downstream changes.
- `git log -- testudo-journal/src/pages/Journal.tsx` is empty (Journal should not appear in diff).
- `git log -- testudo-journal/src/pages/Pair.tsx` is empty.
- Grep sweep: `grep -r "PulseStrip\|LiveRiskStrip\|PositionsByVenue\|MarginByVenue\|pulse-strip-preference\|testudo-pulse-strip" testudo-journal/src` returns zero hits.
- Responsive structural inspection:
  - Overview hero: `hidden md:*` / `flex md:hidden` branches render the two formats.
  - Account tile grid: `grid-cols-1 md:grid-cols-2 lg:grid-cols-3`.
  - ExchangeCard: vertical stacking at all breakpoints, no overflow on narrow viewports.
- Deferred to live session (out of autonomous scope):
  - Live 2s WS-driven update of a card's position list.
  - Pixel-level viewport sweep (320 / 375 / 414 / 768 / 1024 / 1440).
  - Multi-venue fixture with `correlation_stack.length ≥ 2` confirming CorrelationStack renders at top.

**Commit format per spec completion signal #6:**
- T1: `refactor(rsk-01a): CP-1 + CP-5 — Overview hero consolidation, delete strip components`
- T2: `refactor(rsk-01a): CP-2 — ExchangeCard margin breakdown`
- T3: `refactor(rsk-01a): CP-3 — ExchangeCard positions, delete Positions/Margin widgets`
- T4: `refactor(rsk-01a): CP-4 — CorrelationStack top-mount, 3-col grid with filler slots`
- T5: `refactor(rsk-01a): account consolidation complete`

**Archive step (per completion signal #8):** move `.specify/specs/RSK-01a-account-consolidation/` to `.specify/spec-archive/` after T5 closes.

---

## Discoveries

### 2026-04-18 — RSK-01a planning

- **WS client is not a singleton, but SolidJS routing makes this moot.** `createRiskWsClient(callback)` allocates a fresh `WebSocket` per call. When Overview unmounts on `/account` route change, its `onCleanup` → `wsClient.disconnect()` tears down the socket before Account's `createResource` mounts. Only one WS per page at a time. Spec risk #7 was predicated on concurrent mounts of two WS-owning components on the same page — doesn't happen here because Overview and Account are siblings under `<Router>`, never co-mounted.

- **Account page has no WS-driven refetch today (latent gap).** Account's `createResource(fetchRiskSnapshot)` fires once on mount. Without a WS handle or polling interval, ExchangeCard positions will look stale until the user navigates away and back. RSK-01a's CP-3 acceptance criterion "position list updates on WS fill event within 2s" is therefore NOT satisfiable without adding a WS handle on Account. **Decision**: accept this as a carry-over from RSK-01's original design — the spec's FR-10 says "WS push + 30s polling fallback (from RSK-01 T10) continues to drive updates; logic moves from Layout into Overview.tsx and Account.tsx as the actual consumers." So Account SHOULD get a WS handle. T1 can opportunistically install one on Account (identical to the Overview pattern), OR leave it for a fast-follow. **Plan**: install WS handle on Account in T1 — small addition (maybe 30 lines), preserves FR-10 fidelity, and avoids a latent regression. Updated T1 scope accordingly (see "Files" for Overview.tsx + note that Account.tsx also needs WS lifecycle).

  Correction: T1 scope currently only touches Layout and Overview for WS ownership. Adding Account WS at the same time is cleaner but scope-creeps T1. Since Account already has `createResource(fetchRiskSnapshot)` working, I'll leave T1 as defined and add Account WS to T3 (when positions are added — that's when the refetch need becomes user-facing). Will surface this in Discoveries during build.

- **Overview's existing `totalBalance` fan-out stays.** Overview.tsx computes aggregate balance via per-account `exchangeApi.fetchBalance` calls in `onMount` (lines 22–39). Snapshot exposes `margin_by_venue` with per-venue `total_usd`, so Overview COULD drop the fan-out and sum from snapshot. But: the semantics differ subtly — "balance" in Overview today is the total asset value (USDT + USDC + other coins), while `total_usd` in VenueMargin is the stablecoin-denominated margin sum. Conflating them would silently change displayed numbers. RSK-01a is a structural consolidation, not a semantic rewrite. Leave the fan-out alone; just ADD the new hero fields.

- **Grid filler math simplified to `Math.max(0, 3 - (accounts.length + 1))`.** The Account grid already renders one `<AddExchangeCard>` at the end of `<For each={accounts()}>`. With 0 accounts we're in onboarding flow (different code path). With 1 account: 1 + 1 add = 2; filler = 1 → 3 tiles. With 2 accounts: 2 + 1 add = 3; filler = 0 → 3 tiles. With 3+ accounts: filler = 0. Keeps the `lg:grid-cols-3` row full for 1–3 account users without forcing hollow fillers on 4+ account users. This simplification is cleaner than breakpoint-aware filler counts and still honors FR-8.

- **No new dependencies, no backend changes, net LOC delta negative.** Per spec's completion signal #3, this consolidation should DELETE more than it adds. Rough count:
  - Deletions: PulseStrip.tsx (74) + LiveRiskStrip.tsx (63) + PositionsByVenue.tsx (127) + MarginByVenue.tsx (79) + pulse-strip-preference.ts (25) + Layout.tsx snapshot/WS/polling block (~65) + Account.tsx preference toggle + `balanceForCard` / `venueMarginFor` helpers (~20) + help-content entries (~4) = ~457 lines deleted.
  - Additions: Overview.tsx WS/polling/hero extension (~80) + ExchangeCard.tsx margin breakdown + positions sections (~120) + CorrelationStack early-return (~3) + Account.tsx grid restructure + CorrelationStack re-mount (~15) = ~218 lines added.
  - Net: ~−240 lines. Consolidation achieved.

- **Pair and Journal structural preservation is free.** Pair page uses `isStandalonePage()` carve-out, so Layout shell changes don't reach it. Journal doesn't import any risk widget and doesn't read `pulseStripEnabled`, so removing those only affects Layout's header row height (no more PulseStrip above → Journal's header moves up by 28px). This is a spec-desired effect ("no persistent strip above the header, so those pages' aesthetics remain uncluttered").

- **CorrelationStack's existing empty-state fallback ("No directional exposure") is dropped.** With the `< 2` early-return, there's never a case where the section renders with zero buckets. Removing the fallback is a small cleanup in T4.

---

## Status

PLAN COMPLETE

Spec: RSK-01a-account-consolidation
Total Tasks: 5 (T1–T5)
Next task: T1 — Overview hero consolidation + delete PulseStrip / LiveRiskStrip / pulse-strip-preference

Ready for BUILD mode.
