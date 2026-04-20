# Specification: Journal Kelly Audit

**Spec ID:** QNT-01c-kelly-journal-audit
**Date:** 2026-04-20
**Status:** Draft
**Class:** Feature / Journal UI
**Priority:** P2 — retrospective analysis; ships after calibration data has accumulated
**Depends on:** QNT-01a (Calibrated Kelly Sizing Engine) — reads `journal_trades.kelly_inputs` JSONB written at close time.
**Series:** QNT-01 (Calibrated Kelly — a through c)

---

## Problem Statement

Once QNT-01a is live, every dynamic-mode trade carries a `kelly_inputs` JSONB blob capturing the exact calibration inputs at decision time: `p_eff`, `avg_r_win`, `avg_r_loss`, `n_setup`, `n_global`, `edge_multiplier`, `baseline_risk_pct`, `effective_risk_pct`. That data is currently invisible in the journal UI — users can see a trade's P&L but not whether Kelly sized it differently than fixed mode would have, or *why*.

This spec exposes the data retrospectively: a small "⚡ Kelly-sized" badge on trade rows with non-null `kelly_inputs`, and a click-through detail modal showing the full input tuple. It enables the natural audit question *"was Kelly profitable for me on this setup?"* without requiring a new aggregation endpoint; the JSONB blobs are already grouped by `setup_tag` via the existing journal filters.

This spec is pure journal frontend plus a type extension — no backend changes beyond surfacing a field that already exists.

---

## User Stories

- **As a user reviewing past trades**, I want to see at a glance which trades were Kelly-sized vs fixed-sized, so that I can mentally separate the two regimes.
- **As a user auditing my calibration**, I want to open any Kelly-sized trade and see the exact inputs (sample size, win rate, avg R, multiplier) that produced the sizing decision, so that I can judge whether the math was acting on fresh or stale data.
- **As a user filtering by setup**, I want to compare Kelly-sized vs fixed-sized trades for the same setup, so that I can tell whether Dynamic Risk is pulling its weight for that setup.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `JournalTrade` type in `testudo-journal` carries `kelly_inputs: KellyInputs \| null` | High | `testudo-journal/src/api/client.ts` |
| FR-2 | Journal API client returns `kelly_inputs` on all trade-fetch endpoints (passthrough from backend JSONB column) | High | `testudo-exchange/crates/router/src/services/journal_service.rs` |
| FR-3 | Trade row renders a `⚡ Kelly-sized` badge when `kelly_inputs != null`; no badge for fixed-mode trades | High | `testudo-journal/src/components/trades/TradeRow.tsx` |
| FR-4 | Clicking the badge (or a dedicated button on the row) opens a detail modal showing the full `kelly_inputs` tuple, formatted for humans | High | `testudo-journal/src/components/trades/KellyInputsModal.tsx` |
| FR-5 | Detail modal shows: baseline → effective risk with multiplier, sample sizes (`n_setup`, `n_global`), blended stats (`p_eff`, `avg_r_win`, `avg_r_loss`), the raw per-setup and global priors for comparison, and `computed_at` timestamp | High | `testudo-journal/src/components/trades/KellyInputsModal.tsx` |
| FR-6 | Detail modal shows a one-line summary: *"Sized up 1.4× because this setup's 43-trade history beats your 312-trade baseline by ~8 points of win rate."* (or equivalent for down-sized / neutral cases) | Medium | `testudo-journal/src/components/trades/KellyInputsModal.tsx` |
| FR-7 | HELP tooltip namespace `kelly.*` explains the badge, each field in the modal, and links to the transparency page (QNT-01b or future docs) | Low | `testudo-journal/src/lib/help-content.ts` |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `JournalTrade.kelly_inputs` type + backend passthrough + row badge. No modal yet. | A Kelly-sized trade row shows the ⚡ badge; a fixed-mode row does not. Clicking does nothing yet. |
| CP-2 | Detail modal (raw field dump). | Clicking the badge opens a modal that shows every field from `kelly_inputs`, formatted but not yet narrated. |
| CP-3 | Narrative summary line (FR-6) + HELP entries (FR-7). | The modal's one-line summary reads naturally for up-sized, down-sized, and clamped cases. |

### `KellyInputs` Type (shared)

Mirror of the JSONB shape written by QNT-01a:

```typescript
// testudo-journal/src/api/client.ts
export type KellyInputs = {
  mode: 'calibrated_kelly';
  baseline_risk_pct: number;
  effective_risk_pct: number;
  edge_multiplier: number;

  p_eff: number;
  avg_r_win: number;
  avg_r_loss: number;
  quarter_kelly: number;

  n_setup: number;
  n_global: number;
  pseudocount_k: number;

  p_setup_raw: number;
  p_global_raw: number;

  computed_at: string;  // ISO 8601
};

export type JournalTrade = {
  // ... existing fields ...
  kelly_inputs: KellyInputs | null;
};
```

Matching Rust serialization on the backend side is a pure `serde_json::Value` passthrough — the router does not need to deserialize or validate the shape; QNT-01a wrote it, QNT-01c reads it.

### Narrative Summary Logic (FR-6)

Three variants, selected by comparing `edge_multiplier` against bounds:

| Condition | Copy |
|-----------|------|
| `edge_multiplier > 1.05` and `edge_multiplier < 2.0` | *"Sized up {Xx} because this setup's {n_setup}-trade history beats your {n_global}-trade baseline."* |
| `edge_multiplier >= 2.0` | *"Sized up 2.0× (ceiling hit) — this setup's edge is strong enough that the clamp engaged."* |
| `edge_multiplier < 0.95` and `edge_multiplier > 0.25` | *"Sized down {Xx} because this setup's {n_setup}-trade history trails your baseline."* |
| `edge_multiplier <= 0.25` | *"Sized down 0.25× (floor hit) — calibration is weak for this setup."* |
| otherwise | *"Sized at baseline — calibration is neutral for this setup."* |

Keep copy terse. One line, no exclamation marks, no advice.

### Paved Roads

- **Existing `JournalTrade` type** — already carries `setup_tag`, `risk_amount`, `r_multiple`, `notes`. Adding `kelly_inputs` follows the same nullable-column pattern.
- **Trade row component** — existing UI already renders small inline badges (e.g. `setup_tag`). ⚡ badge reuses the same badge primitive.
- **Modal primitive** — the journal already mounts modals for trade-detail views; reuse the existing shell, only the body is new.
- **HELP tooltip** — `help-content.ts` namespaces are additive; `kelly.*` entries land alongside `dignitas.*` (from ENG-01a) and `coach.*` (from RSK-03).

### Files

**Backend (Rust)** — one touch only

- `crates/router/src/services/journal_service.rs` — MODIFIED. Include `kelly_inputs` in the `SELECT` for trade-fetch queries and in the `JournalTrade` struct that serializes to JSON. QNT-01a already writes the column; this is a pure surfacing change.

**Journal (TypeScript / Solid)**

- `src/api/client.ts` — MODIFIED. Add `KellyInputs` type; extend `JournalTrade` with `kelly_inputs: KellyInputs | null`.
- `src/components/trades/TradeRow.tsx` (or equivalent — confirm exact path at build time) — MODIFIED. Render ⚡ badge when `kelly_inputs != null`; click opens `KellyInputsModal`.
- `src/components/trades/KellyInputsModal.tsx` — NEW. Raw field table + narrative summary.
- `src/lib/help-content.ts` — MODIFIED. Add `kelly.*` namespace.

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Backend `GET /api/v1/journal/trades` response includes `kelly_inputs` field on every row (non-null for Kelly-sized trades, null for fixed-mode trades).
- [ ] Journal trade row renders the `⚡ Kelly-sized` badge for trades with `kelly_inputs != null`.
- [ ] Journal trade row renders no badge for trades with `kelly_inputs == null`.
- [ ] Clicking the badge opens a modal showing baseline, effective, multiplier, `n_setup`, `n_global`, `p_eff`, `avg_r_win`, `avg_r_loss`, `p_setup_raw`, `p_global_raw`, `quarter_kelly`, `computed_at`.
- [ ] Modal narrative line selects correctly across all five cases (up / up-clamped / baseline / down / down-clamped) — snapshot-tested with fixture `KellyInputs` values.
- [ ] HELP tooltips on `kelly.badge`, `kelly.edge_multiplier`, `kelly.n_setup`, `kelly.n_global`, `kelly.p_eff` return non-empty copy.
- [ ] Filtering the journal by `setup_tag` correctly shows Kelly and fixed-mode trades interleaved; badge placement is per-row, not per-filter.
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes (backend surface change only).
- [ ] `cd testudo-journal && bun run build` passes.

---

## Risks

1. **Shape drift between backend and journal.** If QNT-01a ever changes the `kelly_inputs` JSONB shape, the journal TypeScript type silently breaks. *Mitigation:* add a minimal Zod `KellyInputsSchema` on the journal fetch boundary (same pattern as `testudo-extension/src/schemas.ts`); surface parse failures as *"calibration data unavailable"* rather than crashing the row.
2. **Historical trades predate Kelly.** Every trade before QNT-01a rolls out has `kelly_inputs = NULL`. The UI correctly shows no badge for those. *No mitigation needed* — behavior is correct by construction.
3. **Narrative line sounds like advice.** A user might read *"sized up"* as the system endorsing a trade rather than reporting its own math. *Mitigation:* copy is strictly descriptive, past-tense, and references the multiplier as a fact, not a recommendation. Reviewed in CP-3.
4. **Badge visual noise at high trade counts.** A filtered view of 500 trades could show 500 ⚡ badges. *Mitigation:* the badge is small and single-char; if it still noises at scale, promote to a column header filter ("Kelly / Fixed / All") in a follow-up.
5. **Modal info density confuses casual users.** Full Bayesian tuple is intimidating. *Mitigation:* narrative summary line (FR-6) is the top-of-modal headline; raw fields are below a fold for users who want them.

---

## Completion Signal

This spec is complete when:
1. All three checkpoints (CP-1 → CP-3) landed on master.
2. All acceptance criteria checked.
3. Two real historical trades observed in the journal: one with `kelly_inputs != null` showing the ⚡ badge and a working detail modal, one with `kelly_inputs == null` showing no badge.
4. `cargo clippy --all-targets && cargo test` + `bun run build` green.
5. Commit message: `feat(qnt-01c): journal Kelly audit — row badge + detail modal`.
