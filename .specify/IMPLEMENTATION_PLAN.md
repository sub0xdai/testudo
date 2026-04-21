# Implementation Plan

> Last updated: 2026-04-21
> Current spec: QNT-01c-kelly-journal-audit
> Phase: BUILD complete — all 3 tasks landed, spec archived

---

## Active Spec: QNT-01c-kelly-journal-audit

### Tasks

| Task | Description | Status |
|------|-------------|--------|
| T1 | `KellyInputs` TS type + `JournalTrade.kelly_inputs` field | complete |
| T2 | Backend passthrough verified + ⚡ Kelly badge on trade row | complete |
| T3 | Kelly detail modal + narrative summary + HELP entries | complete |

---

## Status

BUILD COMPLETE — spec archived to `.specify/spec-archive/QNT-01c-kelly-journal-audit/`

Spec: QNT-01c-kelly-journal-audit
Total Tasks: 3 (T1, T2, T3)
Complete: T1, T2, T3

### Final Verification (T-final, 2026-04-21)

- `cargo clippy --all-targets`: clean. 3 pre-existing warnings (actor.rs:1858, cex_client.rs:653, evaluator.rs:188) unchanged — QNT-01c is pure journal frontend, no backend changes.
- `cargo test`: 655 passing / 1 pre-existing fail (`routes::auth::tests::test_me_returns_user_info`, AUTH-02 regression documented since QNT-01a T2) / 13 ignored. Zero QNT-01c-introduced regressions.
- `bun run build` (testudo-journal): exit 0, 16.95s. No new bundle size regressions.
- QNT-01c introduced no Rust changes — backend `kelly_inputs` passthrough was already present via QNT-01a T7's `SELECT jt.*` / `SELECT *` queries.

### Manual QA (deferred to live session)

- Observe one Kelly-sized trade row in journal showing ⚡ badge; one fixed-mode row showing no badge.
- Click badge → modal opens showing all 12 fields (`baseline_risk_pct`, `effective_risk_pct`, `edge_multiplier`, `p_eff`, `avg_r_win`, `avg_r_loss`, `quarter_kelly`, `n_setup`, `n_global`, `pseudocount_k`, `p_setup_raw`, `p_global_raw`, `computed_at`).
- Narrative summary selects the correct variant across up-sized / up-clamped / down-sized / down-clamped / baseline cases.
- HELP tooltips on `kelly.badge`, `kelly.edge_multiplier`, `kelly.n_setup`, `kelly.n_global`, `kelly.p_eff` return non-empty copy.
