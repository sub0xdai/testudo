# Specification: Pre-Submit Kelly Transparency + Unlock UX

**Spec ID:** QNT-01b-kelly-transparency
**Date:** 2026-04-20
**Status:** Draft
**Class:** Feature / Extension UX
**Priority:** P1 — closes the trust gap; plan decision D7 holds that hiding the math invites distrust on first surprise
**Depends on:** QNT-01a (Calibrated Kelly Sizing Engine) — needs `kelly_inputs`, `user_settings`, and the math modules.
**Series:** QNT-01 (Calibrated Kelly — a through c)

---

## Problem Statement

QNT-01a ships the Kelly engine silently: a user who enables Dynamic Risk sees their sized quantity change in the Alt+X modal but has no visibility into *why* it changed or by how much. Per plan decision D7, this is unacceptable — a trader seeing "1.4%" when they typed "1.0%" without explanation will distrust the system on the first surprise and disable it.

A parallel gap: QNT-01a's unlock gate is enforced server-side, but the popup toggle in 01a is just a greyed-out switch. Users with 0, 5, or 25 tagged closes cannot tell the feature apart from a broken one — there is no progress signal, no copy explaining the threshold, no sense of nearness.

This spec closes both gaps with an inline pre-submit preview row in the Alt+X modal and a progress-revealing locked state in the popup. It is pure frontend + one thin backend readiness endpoint; the math and storage are untouched.

---

## User Stories

- **As a user submitting a trade with Dynamic Risk on**, I want to see *Risk: 1.0% → 1.4% (43 trades, 58% win rate, 1.9R avg)* above the confirm button, so that I understand why my size changed before I press Enter.
- **As a user who has not yet reached the 30-trade threshold**, I want to see my progress (e.g. *18 / 30*) on the locked toggle, so that I know the feature exists and how far away I am from it.
- **As a user on an untagged trade**, I want the preview row to tell me *"Tag this setup to unlock calibrated sizing for it"*, so that I learn the shape of the feature without reading docs.
- **As a user on a setup with negative calibrated edge**, I want the preview row to show the rejection reason inline *before* I press Enter, so that I am not surprised by a submission failure.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `GET /api/v1/user/qnt-readiness` returns `{ tagged_closed, unlock_at: 30, unlocked: bool }` | High | `router/routes/journal.rs` |
| FR-2 | Popup toggle displays *"Dynamic Risk unlocks after 30 tagged closes (N/30)"* when `unlocked = false` | High | `extension/src/popup` |
| FR-3 | Popup toggle shows `dynamic_risk_unlocked_at` date in a small caption when unlocked | Low | `extension/src/popup` |
| FR-4 | `POST /api/v1/trades/preview` accepts the same payload as `create_trade` and returns `{ baseline_risk_pct, effective_risk_pct, edge_multiplier, reasoning }` without executing anything | High | `router/routes/trade_management.rs` |
| FR-5 | Alt+X modal renders a single-line preview row above the confirm button when Dynamic Risk is on and the backend returned a `sizing_preview` | High | `extension/src/components/TradeForm.tsx` |
| FR-6 | Preview row formats as: *Risk: {baseline}% → {effective}% ({n_setup} trades, {round(p_eff*100)}% WR, {avg_r_win}R avg)* | High | `extension/src/components/TradeForm.tsx` |
| FR-7 | Preview row on an untagged trade reads: *"Tag this setup to unlock calibrated sizing for it."* (italic, muted) | Medium | `extension/src/components/TradeForm.tsx` |
| FR-8 | Preview row on a negative-edge setup reads: *"Calibration shows negative edge for this setup — size = 0."* in error-red; confirm button is disabled | High | `extension/src/components/TradeForm.tsx` |
| FR-9 | Preview request is debounced by 150 ms on payload change; does NOT block Alt+X modal open | Medium | `extension/src/components/TradeForm.tsx` |
| FR-10 | Preview failures (network, 5xx) render *"Preview unavailable"* in muted text; confirm button remains enabled (falls through to baseline) | Medium | `extension/src/components/TradeForm.tsx` |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `GET /user/qnt-readiness` backend + popup locked-state copy with `N/30` progress. | A user at 12 tagged closes sees *"Dynamic Risk unlocks after 30 tagged closes (12/30)"* and a disabled toggle. A user at 30+ sees an enabled toggle with the unlock date. |
| CP-2 | `POST /trades/preview` backend + Alt+X inline row. Happy path first (tagged, positive edge). | A user submitting on a well-calibrated setup sees *"Risk: 1.0% → 1.4% (43 trades, 58% WR, 1.9R avg)"* above confirm. |
| CP-3 | Edge cases: untagged (FR-7), negative edge (FR-8), preview failure (FR-10). | All three copy variants render correctly; confirm button is disabled only for negative-edge. |

### `sizing_preview` Response Shape

```typescript
// extension/src/schemas.ts addition
export const SizingPreviewSchema = z.object({
  baseline_risk_pct: z.number(),
  effective_risk_pct: z.number(),
  edge_multiplier: z.number(),
  reasoning: z.discriminatedUnion('kind', [
    z.object({
      kind: z.literal('calibrated'),
      n_setup: z.number().int(),
      p_eff: z.number(),
      avg_r_win: z.number(),
      avg_r_loss: z.number(),
    }),
    z.object({ kind: z.literal('untagged') }),
    z.object({ kind: z.literal('negative_edge'), quarter_kelly: z.number() }),
    z.object({ kind: z.literal('fixed_mode') }),  // user has dynamic mode off
  ]),
});
```

The discriminated union lets the frontend render the correct copy variant without re-computing anything.

### Preview Endpoint Contract

- Endpoint: `POST /api/v1/trades/preview`
- Auth: `AuthenticatedUser` extractor (same as `create_trade`).
- Body: identical to `CreateTradeRequest` minus side-effect-only fields.
- Behavior: runs the same `CalibrationEngine` + `kelly.rs` pipeline as `create_trade` but returns the `SizingPreview` instead of placing orders. **No database writes, no CCXT calls.**
- Latency budget: < 50 ms p99 (two `COUNT`/`AVG` queries on an indexed column, no I/O elsewhere).

### Paved Roads

- **`AuthenticatedUser`** — same pattern as every other `/api/v1` route.
- **`CalibrationEngine` + `kelly.rs`** — imported from QNT-01a. The preview endpoint is a thin wrapper around the exact same math the execution path uses, guaranteeing preview/execution parity.
- **Zod schemas** — `SizingPreviewSchema` follows the existing discriminated-union pattern in `schemas.ts`.
- **Debounce util** — `extension/src/utils.ts` already exports the 250 ms balance-refresh debounce; reuse the same helper at 150 ms.

### Files

**Backend (Rust)**

- `crates/router/src/routes/journal.rs` — MODIFIED. Add `GET /api/v1/user/qnt-readiness` handler.
- `crates/router/src/routes/trade_management.rs` — MODIFIED. Add `POST /api/v1/trades/preview` handler that reuses the calibration pipeline without side effects.
- `crates/router/src/routes/mod.rs` — MODIFIED. Register the two new routes.

**Extension (TypeScript / Solid)**

- `src/schemas.ts` — MODIFIED. Add `QntReadinessSchema`, `SizingPreviewSchema`.
- `src/background.ts` / `src/background/api.ts` — MODIFIED. Add `getQntReadiness()`, `previewTradeSizing(payload)`. Register in message-dispatch table.
- `src/popup/components/SettingsPanel.tsx` — MODIFIED. Render locked state with `N/30` progress when `!unlocked`; unlocked date caption when `unlocked`.
- `src/components/TradeForm.tsx` — MODIFIED. Wire preview fetch (debounced 150 ms on payload change), render the preview row above the confirm button, disable confirm on negative-edge.
- `src/modal.tsx` — MODIFIED. Add Shadow DOM CSS for the preview row (muted color on calibrated/untagged, error-red on negative-edge, single line, badge-style leading `⚡`).

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `GET /api/v1/user/qnt-readiness` returns the correct shape for users at 0, 15, 30, and 100 tagged closes.
- [ ] Popup toggle with `unlocked = false` shows *"Dynamic Risk unlocks after 30 tagged closes (N/30)"* where N is live; toggle is disabled.
- [ ] Popup toggle with `unlocked = true` shows the unlock date in a small caption; toggle is enabled.
- [ ] Alt+X modal with Dynamic Risk on, tagged setup, calibrated data: renders *"Risk: 1.0% → 1.4% (43 trades, 58% WR, 1.9R avg)"* within 300 ms of modal open.
- [ ] Alt+X modal with Dynamic Risk on, untagged setup: renders *"Tag this setup to unlock calibrated sizing for it."* in muted text; confirm remains enabled (silent fallback still applies — FR-8 in QNT-01a).
- [ ] Alt+X modal with Dynamic Risk on, negative-edge setup: renders *"Calibration shows negative edge for this setup — size = 0."* in error-red; confirm button is disabled.
- [ ] Alt+X modal with Dynamic Risk OFF: preview row is not rendered at all (no API call made).
- [ ] Preview endpoint response is identical (to the cent) to what `create_trade` would produce on the same payload; `effective_risk_pct` parity test in the router integration suite.
- [ ] Network failure on preview fetch: renders *"Preview unavailable"* without breaking the modal; confirm button remains enabled.
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes.
- [ ] `cd testudo-extension && bun run typecheck` passes.

---

## Risks

1. **Preview / execution divergence.** If the preview uses a snapshot of calibration data while execution uses live data, a user could see *"1.4%"* in the preview and get a different effective risk in the confirm. *Mitigation:* single code path — both handlers import `CalibrationEngine::compute(user_id, setup_tag)`. An integration test runs both against the same fixtures and asserts byte-parity on the output tuple.
2. **Latency spikes on large journals.** The two aggregate queries hit all historical tagged trades. For a user at 5000+ closes the query time could drift. *Mitigation:* the existing `idx_journal_trades_user_setup` index supports both count and aggregate in one scan; measure at CP-2; add a 5-min in-process cache only if p99 > 50 ms (defer to a later spec otherwise).
3. **Preview spam at modal open.** Rapid slider drag in the trade form could fire dozens of previews per second. *Mitigation:* FR-9 debounce (150 ms). Drop in-flight preview if a new one supersedes it.
4. **Copy misreads as "advice".** *"Calibration shows negative edge"* could be interpreted as a recommendation rather than a hard rejection. *Mitigation:* pair the copy with a disabled confirm button (FR-8) — the UI makes the rejection structural, not advisory.
5. **Preview confuses new users who haven't yet unlocked.** A user at 5 tagged closes with the toggle locked has no preview. *Mitigation:* correct behavior — preview only exists for users who have opted in. The locked-state copy in the popup is the only surface they see.

---

## Completion Signal

This spec is complete when:
1. All three checkpoints (CP-1 → CP-3) landed on master.
2. All acceptance criteria checked.
3. One end-to-end user-trade observed: Dynamic Risk on, tagged setup, the preview row renders the expected calibration summary before confirm, and the executed trade's `kelly_inputs` matches the preview's numbers.
4. `cargo clippy --all-targets && cargo test` + `bun run typecheck` green.
5. Commit message: `feat(qnt-01b): pre-submit Kelly transparency + unlock UX`.
