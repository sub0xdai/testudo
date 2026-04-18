# Specification: Setup Tagging at Entry Time — Optional Field in Alt+X Modal

**Spec ID:** RSK-02-setup-tag-at-entry
**Date:** 2026-04-17
**Status:** Draft
**Class:** Feature / Extension + Backend + Frontend
**Priority:** P1 — small-surface feature, unblocks RSK-03's `SetupFatigue` pattern and upgrades Overview's chart grid with a Setup Breakdown view
**Depends on:** None (tag system already exists in journal for post-hoc tagging)
**Series:** RSK-01 through RSK-04

---

## Problem Statement

Testudo captures every trade through the Alt+X modal but not the **intent** behind it. The user is staring at a chart, sees a pattern, hits Alt+X, sizes the trade, and fills — and the "why" (breakout, mean reversion, liquidity sweep, news fade, etc.) is lost at the moment it was most obvious.

The journal already has a full tag system (`TagManager`, `TagSelector`, `TagBadge`) for **post-hoc** tagging. But retrospective tagging is a chore most users skip, and when they do it they're reconstructing intent from memory. Legacy journals (Edgewonk, Tradervue, TradeZella) all suffer the same gap.

Capturing the tag **at entry time — optionally** — turns post-hoc analytics into live intent data without friction. This is a non-obvious UX differentiator: Edgewonk etc. force users to open a form after the fact; Testudo already has a modal at the exact moment of clarity and can capture one string for free. Downstream, this tag powers a new Setup Breakdown chart on Overview and feeds the SetupFatigue pattern in RSK-03. The field stays **optional** — zero friction for users who don't want it, full analytical payoff for those who do.

---

## User Stories

- **As a trader hitting Alt+X**, I want to optionally name the setup I'm taking (e.g., "BTC 4H breakout"), so my journal has the intent captured at the moment I was clearest about it — without being forced to fill anything.
- **As a returning user**, I want to see my previous setup names auto-suggested so typos don't fragment my data ("breakout" vs "break-out" vs "bo").
- **As an Overview user**, I want a Setup Breakdown chart alongside Symbol Breakdown so I can see which setups have real edge.
- **As a user who wants frictionless trading**, I want the tag field to be skippable with no warnings or delays — just Enter to confirm.
- **As the RSK-03 coach**, I want access to per-setup baselines so I can detect setup fatigue (trailing R-multiple declining vs all-time average).

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Alt+X modal (`TradeForm.tsx`) includes an **optional** `Setup` text field; empty submission ships the trade with `setup_tag: null` | High | extension |
| FR-2 | Setup field has **auto-complete** from the user's prior distinct setup tags, ranked by recency + frequency | High | extension + backend |
| FR-3 | Setup tag is **normalized** on submit: trimmed, lowercased for dedup comparison, max 48 chars, display preserves user's casing | High | extension |
| FR-4 | Setup tag flows through existing trade pipeline: extension → router → trade record → journal | High | router, db |
| FR-5 | Zod schema (`schemas.ts`) extended with optional `setup_tag: string \| null` field | High | extension |
| FR-6 | Backend trade schema + migration adds nullable `setup_tag` column | High | sqlx_postgres |
| FR-7 | Journal's existing `Tag` system gets **auto-populated** from `setup_tag` on trade ingest — no manual re-tagging required | Medium | db-processor |
| FR-8 | New `SetupBreakdown.tsx` chart component in `testudo-journal/src/components/charts/`, added to the Overview `ChartSelector` options | High | journal/frontend |
| FR-9 | Setup Breakdown shows per-setup: trade count, win rate, avg R-multiple, expectancy; sortable by any column | High | journal/frontend |
| FR-10 | **Zero friction:** Enter on the Setup field (empty or filled) moves to next field or confirms; no validation blocks empty; double-Enter safety (existing pattern) still works | High | extension |
| FR-11 | New backend endpoint `GET /api/user/setup-tags?limit=20` returns distinct prior tags for auto-complete | Medium | router |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Schema change + migration + router passthrough; extension submits `setup_tag` optionally | End-to-end pipe works; empty tag is a first-class value |
| CP-2 | `TradeForm.tsx` gains the optional field + autocomplete dropdown | UX: field is skippable, typing suggests prior tags, Enter commits |
| CP-3 | Journal ingest auto-tags trades from `setup_tag` using existing tag system | Tag surfaces in existing journal views without code changes |
| CP-4 | `SetupBreakdown.tsx` chart + wired into Overview's `ChartSelector` | Analytical payoff visible in Overview |

### Key Changes

```typescript
// testudo-extension/src/schemas.ts — extend TradeRequestSchema
export const TradeRequestSchema = z.object({
  // ... existing fields ...
  setup_tag: z.string().trim().max(48).nullable().optional(),
})
```

```typescript
// testudo-extension/src/components/TradeForm.tsx — add field
// Placement: below pair/size, above the confirm button.
// Behavior:
//   - <input type="text" placeholder="Setup (optional)" />
//   - onFocus: fetch suggestions (debounced 300ms)
//   - onChange: filter suggestions by prefix
//   - onKeyDown: Tab/Enter accepts highlighted suggestion OR commits current value
//   - Empty string → submit as null
```

```rust
// testudo-exchange/crates/sqlx_postgres/migrations/NNNN_setup_tag.sql
ALTER TABLE trades ADD COLUMN setup_tag TEXT NULL;
CREATE INDEX idx_trades_user_setup ON trades(user_id, setup_tag) WHERE setup_tag IS NOT NULL;
```

```rust
// testudo-exchange/crates/router/src/routes/user.rs
// GET /api/user/setup-tags?limit=20
// SELECT DISTINCT setup_tag, MAX(created_at) AS last_used, COUNT(*) AS uses
// FROM trades
// WHERE user_id = $1 AND setup_tag IS NOT NULL
// GROUP BY setup_tag
// ORDER BY last_used DESC, uses DESC
// LIMIT $2
```

### Paved Roads

- **Existing tag infrastructure** — `TagManager`, `TagSelector`, `TagBadge` in `testudo-journal/src/components/journal/` is the data home. `setup_tag` on the trade is the *source*; the tag system is the *consumer*.
- **Zod schema pattern** — existing `RuntimeMessageSchema` discriminated union in `schemas.ts` is the extension runtime contract.
- **Double-Enter safety** in `TradeForm.tsx` — preserved unchanged; optional field does not intercept the confirm path.
- **`ChartSelector`** — adding a new chart is a one-line addition to the `defaultChart` enum and a new case in its render switch.
- **`SymbolBreakdown.tsx`** — aesthetic and data-shape template for `SetupBreakdown.tsx`; copy/adapt.

### Files

**New:**
- `testudo-journal/src/components/charts/SetupBreakdown.tsx`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_setup_tag.sql`
- `testudo-exchange/crates/router/src/routes/user.rs` (new route, or extend existing)

**Modified:**
- `testudo-extension/src/schemas.ts` — add `setup_tag` to trade schemas
- `testudo-extension/src/components/TradeForm.tsx` — add optional field + autocomplete
- `testudo-extension/src/background.ts` — pass `setup_tag` through trade submission
- `testudo-extension/src/types.ts` — type mirrors Zod schema
- `testudo-exchange/crates/router/src/services/trade_manager/service.rs` — persist `setup_tag`
- `testudo-exchange/crates/db-processor/src/ingest.rs` (or equivalent) — auto-create tag from `setup_tag` on trade ingest
- `testudo-journal/src/api/client.ts` — add `fetchUserSetupTags`
- `testudo-journal/src/components/ChartSelector.tsx` — add `'setup'` option
- `testudo-journal/src/components/Overview.tsx` — optionally add second `SetupBreakdown` to the chart grid (by default users pick it via selector)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Alt+X modal renders the Setup field as optional; Enter on empty value ships the trade with `setup_tag: null`
- [ ] Autocomplete dropdown appears when typing in the Setup field, ranked by recency + frequency, max 10 suggestions
- [ ] Double-Enter safety still gates live confirmation
- [ ] Backend persists `setup_tag` in the `trades` table
- [ ] Journal's tag system auto-creates and applies a tag matching `setup_tag` on trade ingest (no manual re-tagging)
- [ ] `SetupBreakdown` chart renders trade count, win rate, avg R-multiple, and expectancy per setup; sortable
- [ ] Empty / null `setup_tag` trades are grouped under `(untagged)` in the Setup Breakdown
- [ ] Extension `bun run build` succeeds with no type errors
- [ ] Backend `cargo clippy --all-targets && cargo test` passes
- [ ] Journal `bun run build` succeeds
- [ ] Manual QA: tag 5 trades with distinct setups, 2 untagged, confirm all appear correctly in Setup Breakdown

---

## Risks

1. **Typo fragmentation** — users type "breakout", "break-out", "bo", "BO" and the data fragments. *Mitigation:* autocomplete on prior tags + normalize to lowercase for grouping while preserving display casing. Optional future add: fuzzy-match suggestions (`breakout` suggests existing `break-out` etc.).
2. **Scope creep into full tag editor** — temptation to add multi-tag selection, categories, colors. *Mitigation:* MVP is a single-string field. Multi-tag is explicit non-goal here; the journal's existing tag system handles it post-hoc.
3. **Extension Zod schema drift** — if backend accepts `setup_tag` but extension forgets to send, silent data loss. *Mitigation:* schemas mirrored in `schemas.ts`; typecheck gate in extension build.
4. **Autocomplete latency on slow connections** — fetching tags on focus is a network round-trip. *Mitigation:* debounce 300ms, cache suggestions for 5min per extension session, graceful empty-list fallback.

---

## Completion Signal

This spec is complete when:
1. All FR-1 through FR-11 implemented
2. All acceptance criteria checked off
3. One real user trade submitted with a setup tag and observed end-to-end (extension → Overview's Setup Breakdown)
4. Verification commands pass: `cargo clippy --all-targets && cargo test`; `bun run build` in both `testudo-extension` and `testudo-journal`
5. Code committed: `feat(rsk-02): optional setup tag at Alt+X entry + Setup Breakdown chart`
6. RSK-03's `SetupFatigue` pattern detector now has per-setup baselines available (unblocks RSK-03 CP-1)
