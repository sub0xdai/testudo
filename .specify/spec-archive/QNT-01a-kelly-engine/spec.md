# Specification: Calibrated Kelly Sizing Engine

**Spec ID:** QNT-01a-kelly-engine
**Date:** 2026-04-20
**Status:** Draft
**Class:** Feature / Risk Engine
**Priority:** P1 — standalone vertical slice; unblocks QNT-01b (UX) and QNT-01c (audit), plus follow-on QNT-02 / QNT-03
**Depends on:** RSK-02 (setup_tag at entry — shipped 2026-04-18). Reads `journal_trades.setup_tag` as the per-setup key.
**Series:** QNT-01 (Calibrated Kelly — a through c)

---

## Problem Statement

Testudo's current sizing applies the user's fixed `risk_percent` (typically 1–2%) uniformly to every trade:

```
size = MIN(size_from_risk_percent, max_risk, max_position, margin_capacity)
```

This captures nothing about whether the current setup actually has edge. A 45%-win-rate setup and a 65%-win-rate setup are both sized at 1%. That ignores decades of proven Kelly-criterion math and leaves alpha on the table for disciplined users who have built up a tagged-trade history. RSK-02 shipped `setup_tag` at entry in April 2026; the data is now available to act on.

Raw Kelly on realistic trading setups recommends 5–15% of bankroll per trade — behaviorally unacceptable even if mathematically "optimal". The fix is a **baseline-anchored** Quarter-Kelly with Bayesian shrinkage toward the user's global prior, clamped to `[0.25×, 2.0×]` of the user's chosen baseline. Users who never enable the mode keep today's behavior byte-for-byte.

This spec delivers the core engine: schema, pure-math modules, integration into `create_trade`, and the server-side unlock gate (≥30 tagged closes) that prevents activation before enough data exists to calibrate.

---

## User Stories

- **As a disciplined trader** with ≥30 tagged closes, I want my sizing to scale with my measured per-setup edge, so that capital exposure reflects calibration rather than a static percentage.
- **As a new user** with no trading history, I want the feature to be unavailable (not silently broken), so that I cannot activate Kelly math on garbage data.
- **As any user**, I want to submit a trade on a setup with negative calibrated edge and have it explicitly rejected, so that I cannot override the math by habit.
- **As an existing user** who never enables Dynamic Risk, I want my sizing to behave identically to today, so that adoption is strictly opt-in.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | New `SizingMethod::CalibratedKelly` variant routes through calibration + Kelly math when enabled | High | `common_utils/risk` |
| FR-2 | Per-`(user_id, LOWER(setup_tag))` aggregate query returns `(n, p_win, avg_r_win, avg_r_loss)` in < 5 ms using existing `idx_journal_trades_user_setup` | High | `common_utils/risk/calibration.rs` |
| FR-3 | Bayesian shrinkage blends per-setup stats with the user's global prior at `K = 10` pseudocount | High | `common_utils/risk/calibration.rs` |
| FR-4 | Quarter-Kelly → edge multiplier → effective risk percent, clamped to `[0.25, 2.0]` | High | `common_utils/risk/kelly.rs` |
| FR-5 | Negative full-Kelly (`full_kelly ≤ 0`) rejects the trade with reason *"Calibration shows negative edge for this setup — size = 0."* | High | `router/routes/trade_management.rs` |
| FR-6 | Server-side unlock gate: `PATCH /user/settings` rejects enabling Dynamic Risk unless `COUNT(journal_trades WHERE user_id=$1 AND setup_tag IS NOT NULL) ≥ 30` | High | `router/routes/user_settings.rs` |
| FR-7 | `journal_trades.kelly_inputs` JSONB populated at close time for every dynamic-mode trade; `NULL` for fixed-mode trades | High | `router/services/journal_service.rs` |
| FR-8 | Untagged submission with dynamic mode ON falls back silently to baseline `risk_percent` (fixed-mode behavior); logs at `info` level | Medium | `router/decision_loop.rs` |
| FR-9 | Popup exposes a single on/off toggle for Dynamic Risk; disabled state when server reports `unlocked = false` | Medium | `extension/src/popup` |
| FR-10 | Fixed-mode trade sizing is byte-for-byte identical to pre-spec behavior | High | Cross-cutting |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Migration (`user_settings` table + `kelly_inputs` column) + `GET/PATCH /api/v1/user/settings` + minimal popup toggle wired to those endpoints. Unlock gate is enforced server-side. No Kelly math yet. | End-to-end plumbing: a user with ≥30 tagged closes can persist `dynamic_risk_enabled = true`; a user with fewer is rejected with a 409 + reason. |
| CP-2 | Pure-math modules `calibration.rs` + `kelly.rs`. Unit-tested against fixture trades. No integration with the trade path yet. | Given `(user_id, setup_tag)`, the modules return correct `p_eff`, `edge_multiplier`, `effective_risk_percent`. Anti-gaming test: small positive `p_win` on weak R:R → `full_kelly ≤ 0`. |
| CP-3 | Wire into `create_trade` handler. Dynamic mode on → calibration query → Kelly → `effective_risk_percent` flows through the existing `MIN` pipeline. `kelly_inputs` JSONB written at trade close in `record_trade_close`. | End-to-end sizing: a trade submitted with dynamic mode on gets Kelly-sized; its `journal_trades` row records the inputs; fixed-mode trades remain unchanged. |

### Mathematics (verbatim from brainstormed plan §4)

For each `(user_id, LOWER(setup_tag))` group we need three numbers:

- `p_win` = fraction of closed trades where `net_pnl > 0`
- `avg_R_win` = mean `r_multiple` over winning trades
- `avg_R_loss` = mean `|r_multiple|` over losing trades (positive number)

**Bayesian shrinkage** (blend per-setup with global prior):

```
N_s = per-setup trade count
N_g = global (user-level) tagged trade count
K   = 10  (pseudocount — hardcoded constant)

p_eff       = (N_s × p_s + K × p_g) / (N_s + K)
avg_R_win   = (N_s × R_w_s + K × R_w_g) / (N_s + K)
avg_R_loss  = (N_s × R_l_s + K × R_l_g) / (N_s + K)
```

At `N_s = 0`: effective tuple = global prior (100% shrinkage). At `N_s = K = 10`: 50/50 blend. At `N_s → ∞`: per-setup dominates.

**Quarter-Kelly core:**

```
b = avg_R_win / avg_R_loss       // reward-to-risk ratio
p = p_eff
q = 1 - p

full_kelly    = (b × p - q) / b
quarter_kelly = full_kelly / 4
```

If `full_kelly ≤ 0`: **block the trade** (FR-5). Negative edge → zero position, non-negotiable.

**Edge multiplier and effective risk:**

```
reference_kelly = Quarter-Kelly(p=0.52, b=1.5)   // ≈ 0.0133, a ~1.3% fraction

edge_multiplier        = clamp(quarter_kelly / reference_kelly, 0.25, 2.0)
effective_risk_percent = baseline_risk_percent × edge_multiplier
```

The ±2× clamp is load-bearing. A user with baseline=1% maxes at 2% effective — identical to picking baseline=2% in fixed mode.

**Composition with existing MIN arms** (unchanged downstream):

```
size_from_risk = (balance × effective_risk_percent / 100) / stop_distance
size = MIN(size_from_risk, max_risk, max_position, margin_capacity)
```

### Key Types

```rust
// common_utils/src/risk/calibration.rs
pub struct SetupStats {
    pub n: u32,
    pub p_win: Decimal,
    pub avg_r_win: Decimal,
    pub avg_r_loss: Decimal,
}

pub struct ShrunkStats {
    pub p_eff: Decimal,
    pub avg_r_win: Decimal,
    pub avg_r_loss: Decimal,
    pub n_setup: u32,
    pub n_global: u32,
}

impl CalibrationEngine {
    pub async fn load_prior(&self, user_id: Uuid) -> Result<SetupStats, Error>;
    pub async fn load_setup(&self, user_id: Uuid, setup_tag: &str) -> Result<SetupStats, Error>;
    pub fn shrink(setup: SetupStats, prior: SetupStats, k: u32) -> ShrunkStats;
}

// common_utils/src/risk/kelly.rs — pure functions
pub fn quarter_kelly(p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal) -> Decimal;
pub fn edge_multiplier(quarter_kelly: Decimal) -> Decimal;  // clamp [0.25, 2.0]
pub fn effective_risk_percent(baseline: Decimal, multiplier: Decimal) -> Decimal;
```

### `kelly_inputs` JSONB Shape (stored only for dynamic-mode trades)

```json
{
  "mode": "calibrated_kelly",
  "baseline_risk_pct": 1.0,
  "effective_risk_pct": 1.4,
  "edge_multiplier": 1.4,

  "p_eff": 0.582,
  "avg_r_win": 1.92,
  "avg_r_loss": 0.95,
  "quarter_kelly": 0.019,

  "n_setup": 43,
  "n_global": 312,
  "pseudocount_k": 10,

  "p_setup_raw": 0.61,
  "p_global_raw": 0.54,

  "computed_at": "2026-04-18T14:22:31Z"
}
```

### Migration

`crates/sqlx_postgres/migrations/{ts}_add_qnt_columns.up.sql`:

```sql
CREATE TABLE IF NOT EXISTS user_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE journal_trades
    ADD COLUMN kelly_inputs JSONB NULL;
```

`settings` JSONB initial shape for new rows:

```json
{ "dynamic_risk_enabled": false, "dynamic_risk_unlocked_at": null }
```

No new index: `idx_journal_trades_user_setup (user_id, setup_tag) WHERE setup_tag IS NOT NULL` (shipped in RSK-02) already serves both the aggregate query and the unlock-count query.

### Paved Roads

- **`AuthenticatedUser` extractor** — same pattern as `/coach`, `/risk`, `/journal` scopes. `routes/user_settings.rs` mounts identically.
- **`AppState` wiring** — add `pub calibration_engine: Arc<CalibrationEngine>` alongside the existing 15 fields; constructed at `router/src/main.rs:442`.
- **`TransactionContext`** — `record_trade_close` already uses it for atomic SL/TP cascade; `kelly_inputs` write joins the same transaction.
- **Decimal math everywhere** — `rust_decimal::Decimal`, no `f64` (per `.claude/rules/rust-backend.md`).

### Files

**Backend (Rust)**

- `crates/sqlx_postgres/migrations/{ts}_add_qnt_columns.{up,down}.sql` — NEW
- `crates/common_utils/src/risk/calibration.rs` — NEW. `CalibrationEngine` + `shrink()`. Pure math; two aggregate queries for I/O.
- `crates/common_utils/src/risk/kelly.rs` — NEW. Three pure functions.
- `crates/common_utils/src/risk/sizing.rs` — MODIFIED. Extend `SizingMethod` enum with `CalibratedKelly`.
- `crates/common_utils/src/risk/config.rs` — MODIFIED. Add `pub dynamic_risk_enabled: bool` to `RiskConfig`; load from `user_settings` JSONB.
- `crates/router/src/routes/user_settings.rs` — NEW. `GET /api/v1/user/settings`, `PATCH /api/v1/user/settings` with server-side unlock check.
- `crates/router/src/routes/mod.rs` — MODIFIED. `pub mod user_settings;`
- `crates/router/src/routes/trade_management.rs` — MODIFIED. `create_trade` calls calibration → Kelly when dynamic mode on. Negative-edge rejection path.
- `crates/router/src/services/journal_service.rs` — MODIFIED. Thread `kelly_inputs: Option<serde_json::Value>` into `record_trade_close`'s INSERT (mirrors RSK-02's `setup_tag` path).
- `crates/router/src/decision_loop.rs` — MODIFIED. Untagged + dynamic-on → silent fallback to baseline.
- `crates/router/src/main.rs` — MODIFIED. Construct `CalibrationEngine`, inject into `AppState`.

**Extension (TypeScript / Solid)**

- `src/schemas.ts` — Extend `TradePayloadSchema` with optional `dynamic_risk_enabled` passthrough; add `UserSettingsSchema`.
- `src/background.ts` / `src/background/api.ts` — `getUserSettings()`, `patchUserSettings()`. Add to message-dispatch table.
- `src/popup/components/SettingsPanel.tsx` (NEW or extend existing popup surface) — single on/off toggle. Disabled when server reports `unlocked = false`. Progress copy is QNT-01b's job; for 01a the disabled state can be a plain greyed-out switch.

### Dependencies Added

None. `rust_decimal`, `serde_json`, `sqlx`, `actix-web` all in tree.

---

## Acceptance Criteria

- [ ] New user with 0 tagged closes: `PATCH /api/v1/user/settings {"dynamic_risk_enabled": true}` returns 409 Conflict with reason referencing the 30-trade threshold.
- [ ] User at ≥ 30 tagged closes: `PATCH` succeeds; `GET` reflects `dynamic_risk_enabled = true` and records `dynamic_risk_unlocked_at`.
- [ ] Trade submitted with dynamic mode OFF sizes exactly as it did pre-spec (FR-10: byte-for-byte).
- [ ] Trade submitted with dynamic mode ON and a setup_tag having ≥ 10 prior closes: `effective_risk_percent` differs from baseline by at most `2×` in either direction (i.e. `effective ∈ [0.25 × baseline, 2.0 × baseline]`).
- [ ] Trade submitted with dynamic mode ON and a setup with NO prior closes (`N_s = 0`): `effective_risk_percent` derived from 100% global prior; `kelly_inputs.n_setup == 0`.
- [ ] Trade submitted with dynamic mode ON and negative full-Kelly (`p_eff < q_eff / b`): **rejected** with reason *"Calibration shows negative edge for this setup — size = 0."* `journal_trades` row is NOT created for rejected trades.
- [ ] Trade submitted with dynamic mode ON but no `setup_tag`: silent fallback to baseline sizing; `kelly_inputs` remains NULL at close; `info`-level log emitted.
- [ ] `journal_trades.kelly_inputs` JSONB is populated at close time for every dynamic-mode trade; NULL for every fixed-mode trade.
- [ ] Unit tests cover: shrinkage at `N_s ∈ {0, K, 10×K}`, negative-edge rejection, clamp at both bounds, reference-Kelly constant.
- [ ] Backend verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes.
- [ ] Extension verification: `cd testudo-extension && bun run typecheck` passes (do NOT run `bun run build` per project rule on prod URL defaults).

---

## Risks

1. **Sample size is still small at N = 30.** Per-setup priors are noisy; Kelly may size aggressively on a lucky recent streak. *Mitigation:* Bayesian shrinkage shifts weight to the global prior when per-setup sample is small (`K = 10`). ±2× clamp caps magnitude. User still keeps the existing double-Enter veto at Alt+X.
2. **Survivorship bias in `journal_trades`.** Only closed trades feed the prior; cancelled / abandoned trades don't. *Mitigation:* accept as MVP limitation. Re-evaluate if follow-on QNT-02 drift detector shows calibration walking away from reality.
3. **Drawdown psychology.** After a bad run, Kelly sizes smaller on the next trade, compounding drawdown feel. *Mitigation:* this is the intended behavior per plan thesis. Document clearly in the popup tooltip (QNT-01b scope). The math is non-negotiable.
4. **Untagged trade with dynamic mode on — no setup signal.** *Mitigation:* FR-8 silent fallback. QNT-01b will add an inline nudge to tag; not blocking for 01a.
5. **User toggles mid-streak, muddying audit.** *Mitigation:* `kelly_inputs IS NULL` vs `NOT NULL` is the per-trade truth; QNT-01c can group by mode at analysis time.
6. **Quarter-Kelly is mathematically aggressive for trading.** Some practitioners recommend eighth-Kelly. *Mitigation:* the ±2× clamp is the load-bearing safety buffer; combined with Quarter-Kelly the practical max is `2 × baseline`, identical to a user picking `baseline = 2 × X` in fixed mode.
7. **Setup concept drift** (regime change making a previously-profitable setup unprofitable). *Mitigation:* deferred to QNT-02. A user seeing obvious drift can toggle Dynamic Risk off mid-session.

---

## Completion Signal

This spec is complete when:
1. All three checkpoints (CP-1 → CP-3) landed on master.
2. All acceptance criteria checked.
3. Two real user-trades observed end-to-end: one with dynamic mode OFF (baseline behavior preserved), one with dynamic mode ON and a well-calibrated setup (`effective_risk_percent ≠ baseline`, `kelly_inputs` populated in `journal_trades`).
4. `cargo clippy --all-targets && cargo test` + `bun run typecheck` green.
5. Commit message: `feat(qnt-01a): calibrated Kelly sizing engine + Bayesian shrinkage`.
