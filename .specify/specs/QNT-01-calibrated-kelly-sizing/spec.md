# Specification: Calibrated Kelly Sizing — INDEX

**Spec ID:** QNT-01-calibrated-kelly-sizing
**Date:** 2026-04-18 (brainstormed) / 2026-04-20 (carved into atomic specs)
**Status:** **SUPERSEDED** — split into three atomic specs
**Class:** Series index

---

## Why This Was Split

The original brainstormed plan (`QNT-01-calibrated-kelly-sizing-plan.md`, committed 2026-04-18 as `docs(qnt-01)`) packed three independently shippable deliverables into one 335-line design doc: the calibration/Kelly engine, the pre-submit transparency surface, and the retrospective journal audit. Per project convention ("atomic specs over monolithic plans", ENG-01 precedent), QNT-01 has been carved into three independently specifiable, implementable, and shippable slices.

Each atomic spec is self-contained and does not require reading the others to be implemented.

---

## Atomic Specs

| Spec | Scope | Depends on | Priority |
|------|-------|------------|----------|
| [QNT-01a — Calibrated Kelly Sizing Engine](../QNT-01a-kelly-engine/spec.md) | `user_settings` table + `kelly_inputs` JSONB, `kelly.rs` / `calibration.rs` pure-math modules, wire into `create_trade`, server-side unlock gate (≥30 tagged closes), minimal popup toggle | RSK-02 (shipped) | P1 |
| [QNT-01b — Pre-Submit Kelly Transparency + Unlock UX](../QNT-01b-kelly-transparency/spec.md) | `GET /user/qnt-readiness`, locked-toggle progress copy (`N/30`), pre-submit `sizing_preview` endpoint, inline Alt+X modal row (`1.0% → 1.4%` with reasoning) | QNT-01a | P1 |
| [QNT-01c — Journal Kelly Audit](../QNT-01c-kelly-journal-audit/spec.md) | `JournalTrade.kelly_inputs` type, "⚡ Kelly-sized" badge on trade rows, click-through detail modal | QNT-01a | P2 |

---

## Recommended Sequencing

1. **QNT-01a** first — delivers the core behavior change (dynamic risk sizing). Ships safely because the unlock gate is enforced server-side from day one; users cannot enable the mode before they have the data to support it.
2. **QNT-01b** second — converts the engine's silent behavior into an in-flight trustable UX. Per plan decision D7, hiding the math invites distrust on first surprise; this spec closes that gap.
3. **QNT-01c** third — layers retrospective auditability ("was Kelly profitable for me?") onto trades already carrying `kelly_inputs` from 01a. Pure journal work, no backend touch.

Each spec ships independently via its own `/vox plan` + `/vox build` cycle.

---

## Shared Principles (apply across all three)

- **The user's `risk_percent` is an anchor, not a target.** Kelly math modulates it within `[0.25×, 2.0×]`; the ±2× clamp is load-bearing. Raw Kelly on realistic setups produces 5–15% of bankroll per trade, which is behaviorally unacceptable even if mathematically optimal.
- **Opt-in and silent by default.** Users who never flip the Dynamic Risk toggle keep today's fixed-percentage sizing behavior byte-for-byte.
- **Negative edge blocks the trade.** If Bayesian-shrunk Quarter-Kelly is ≤ 0 for a setup, the submission is rejected with an explicit reason. No rounding up, no minimum floor, no "just this once".
- **Single unlock gate: 30 tagged closes (user-level).** Per decision D5, this is the minimum sample where the user's global prior is non-garbage. Framed as a data-quality threshold, not a reward; no gamification, no auto-enable.
- **JSONB on every dynamic-mode trade.** `journal_trades.kelly_inputs` records the full input tuple (p_eff, avg_r_win, avg_r_loss, n_setup, n_global, baseline, effective, multiplier) at close time. Fixed-mode trades keep `kelly_inputs = NULL`. Tiny storage cost (~100 B/trade); powers 01c's audit and any future drift detection.

---

## Locked Architectural Decisions (carried into each atomic spec)

| # | Decision | Source |
|---|----------|--------|
| D1 | Kelly drives sizing; user's `risk_percent` becomes the anchor (integration mode B) — defensive-only Kelly contradicts the offensive-engine thesis | Plan §3 |
| D2 | Baseline-scaled math: `effective_risk% = baseline_risk% × edge_multiplier`, `edge_multiplier ∈ [0.25, 2.0]` | Plan §3 |
| D3 | Global toggle in extension popup settings; silent fallback to baseline for uncalibrated/untagged setups | Plan §3 |
| D4 | Bayesian shrinkage with `K = 10` pseudocount; no hard cliffs | Plan §3 |
| D5 | Unlock at 30 tagged-closed trades (user-level gate) | Plan §3 |
| D6 | Synchronous recompute per trade submit (< 5 ms on existing index — no cache infra) | Plan §3 |
| D7 | Inline pre-submit display + JSONB `kelly_inputs` on `journal_trades` | Plan §3 |

---

## Deferred / Out of Scope for the QNT-01 Series

| Feature | Future spec | Reason |
|---|---|---|
| User confidence capture / true ECE | QNT-02 | Requires Alt+X UX change; adds friction; garbage-in risk. Ship Kelly first. |
| Calibration drift scaler (system-ECE) | QNT-02 | Useful but unproven until we have Kelly data to watch drift against. |
| Regime-conditional calibration (Markov) | QNT-03 | Doubles cognitive load at Alt+X. Evaluate after setup_tag-only Kelly delivers real signal. |
| Pseudocount `K` user-tunable | — | Statistical parameter, not a preference. Hardcoded `K = 10`. |
| Reference Kelly tunability | — | If consistently off, promote to `config.rs` const. Low risk. |

---

## Original Plan Archive

The brainstormed plan text has been preserved in git history (`docs(qnt-01): calibrated Kelly sizing + Bayesian shrinkage plan (brainstormed)` — commit `d61aab8`). The atomic specs above are the authoritative source going forward; the root-level `QNT-01-calibrated-kelly-sizing-plan.md` is removed by this promotion.
