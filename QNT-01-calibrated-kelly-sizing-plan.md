# QNT-01 — Calibrated Kelly Sizing + Bayesian Shrinkage Calibration Engine

**Status:** Design — brainstormed 2026-04-18, not yet promoted to `.specify/specs/`.
**Class:** Risk-engine upgrade. Backend + extension + journal.
**Priority:** P1 — standalone vertical slice; unblocks follow-on QNT-02 / QNT-03.
**Depends on:** RSK-02 (setup_tag at entry) shipped 2026-04-18. Uses `journal_trades.setup_tag` as the per-setup key.
**Follow-ons:** QNT-02 (system-ECE / drift scaler), QNT-03 (regime-conditional calibration).

---

## 1. Thesis

Risk management is not a defensive tool bolted onto trades after the fact. It is the **offensive engine** that scales capital exposure from a mathematically verified distance between one's calibrated edge and the market. Intuition is a liability; conviction is an illusion; sizing is a calculation.

Current Testudo sizing is the **conservative-wins** rule:

```
size = MIN(size_from_risk_percent, max_risk, max_position, margin_capacity)
```

The user picks `risk_percent` (typically 1–2%) manually. This is fine, but it captures nothing about whether the current setup actually has edge. A 45%-win-rate setup and a 65%-win-rate setup are both sized at 1%. That ignores decades of proven math.

This spec introduces **Dynamic Risk Mode** — an opt-in layer that replaces the user's fixed `risk_percent` with a calibration-derived value when sufficient data exists. The user's chosen percentage becomes an **anchor**, not a target; Kelly-derived math modulates it up or down within a bounded band. Users who never flip the switch keep today's behavior unchanged.

---

## 2. Scope

### In scope (MVP)

1. **Kelly sizing layer** — new `SizingMethod::CalibratedKelly` that computes an effective risk percentage from per-setup historical win rate, average R-win, and average R-loss.
2. **Bayesian-shrinkage calibration engine** — reads `journal_trades` synchronously per trade submit, blends per-setup statistics with the user's global prior.
3. **Unlock gate** — users must have closed ≥ 30 tagged trades before the mode becomes flippable.
4. **Transparency surface** — inline pre-submit display in the Alt+X modal; JSONB record of Kelly inputs on every trade.
5. **Popup settings toggle** — single global mode switch in the extension popup.

### Out of scope (deferred)

| Feature | Future spec | Reason |
|---|---|---|
| User confidence capture / true ECE | QNT-02 | Requires Alt+X UX change; adds friction; garbage-in risk. Ship Kelly first. |
| Calibration drift scaler (system-ECE) | QNT-02 | Useful but unproven until we have Kelly data to watch drift against. |
| Regime-conditional calibration (Markov) | QNT-03 | Doubles cognitive load at Alt+X. Evaluate after setup_tag-only Kelly delivers real signal. |
| Combinatorial arbitrage (Markov) | — | Requires prediction-market venue not present in Testudo. |

---

## 3. Architectural Decisions

| # | Decision | Alternative rejected | Why |
|---|---|---|---|
| D1 | **Kelly drives sizing; user's `risk_percent` becomes the anchor** (integration mode B) | Kelly as a cap / floor / advisory-only | Defensive-only Kelly contradicts the alpha's offensive-engine thesis. Must be able to scale UP on strong edge. |
| D2 | **Baseline-scaled math** (B2): `effective_risk% = baseline_risk% × edge_multiplier`, `edge_multiplier ∈ [0.25, 2.0]` | Pure raw Kelly replacement | Raw Kelly numbers for realistic trading setups (e.g. 55% / 2R:1R) come out at 30% of bankroll — insane. Baseline anchoring keeps the output psychologically bounded and within the existing mental model. |
| D3 | **Global toggle in extension popup settings**, silent fallback for uncalibrated setups | Per-trade checkbox / per-setup rule | Per-trade adds friction per-submit. Per-setup is overbuilt — we don't yet know which setups deserve Kelly. Global toggle is the minimal shippable UX. |
| D4 | **Bayesian shrinkage**, `K = 10` pseudocount | Hard threshold / confidence-weighted linear blend | Shrinkage gives no cliff. New setups inherit the user's global prior; setup evidence grows in influence smoothly. Directly aligns with the alpha's Bayesian framing. |
| D5 | **Unlock at 30 tagged-closed trades** (user-level gate) | 20 / 50 / 100 / untagged trades count too | 30 is the minimum sample where per-user global prior becomes non-garbage. Tagged-only gate rewards the discipline RSK-02 exists to encourage. |
| D6 | **Synchronous recompute per trade submit** | pg_queue async / nightly batch | The two aggregate queries run in < 5ms on the existing index. Adding cache infra would be pure overengineering. No staleness-bug surface. |
| D7 | **Inline pre-submit display + JSONB `kelly_inputs` on `journal_trades`** | Silent / post-trade toast / separate sizing table | Hiding the math invites distrust on first surprise. JSONB retrospective data powers later "was Kelly profitable?" audits. Tiny storage cost (~100 B/trade). |

---

## 4. Mathematics

### 4.1 The Kelly input tuple

For each `(user_id, LOWER(setup_tag))` group, we need three numbers:

- `p_win` = fraction of closed trades where `net_pnl > 0`
- `avg_R_win` = mean `r_multiple` over winning trades
- `avg_R_loss` = mean `|r_multiple|` over losing trades (positive number)

### 4.2 Bayesian shrinkage

We compute both **per-setup** and **global** (across all the user's tagged trades) versions of the tuple, then blend:

```
N_s     = per-setup trade count
N_g     = global (user-level) tagged trade count
K       = 10  (pseudocount — tunable constant)

p_eff       = (N_s × p_s + K × p_g) / (N_s + K)
avg_R_win   = (N_s × R_w_s + K × R_w_g) / (N_s + K)
avg_R_loss  = (N_s × R_l_s + K × R_l_g) / (N_s + K)
```

At `N_s = 0`: effective tuple = global prior (100% shrinkage).
At `N_s = K = 10`: effective tuple = 50/50 blend.
At `N_s → ∞`: effective tuple → per-setup.

Untagged submission (no `setup_tag` in Alt+X): skip per-setup entirely; Kelly reads the global prior only. Or fall back to baseline — see D3 choice on silent fallback.

### 4.3 Quarter-Kelly core

```
b = avg_R_win / avg_R_loss          // reward-to-risk ratio
p = p_eff
q = 1 - p

full_kelly   = (b × p - q) / b      // classic Kelly fraction
quarter_kelly = full_kelly / 4      // canonical safety buffer
```

If `full_kelly ≤ 0`: **block the trade**. Negative edge → zero position. The alpha is non-negotiable here.

### 4.4 Edge multiplier and effective risk

We convert Quarter-Kelly to a multiplier relative to a **minimum-tradeable-edge reference**. The reference represents "just barely tradeable":

```
reference_kelly = Quarter-Kelly(p=0.52, b=1.5)    // ≈ 0.0133, a ~1.3% of-bankroll fraction

edge_multiplier = clamp(quarter_kelly / reference_kelly, 0.25, 2.0)
effective_risk_percent = user_baseline_risk_percent × edge_multiplier
```

Three regimes the user experiences:

| Scenario | Sample | `quarter_kelly` | `edge_multiplier` | 1% baseline becomes |
|---|---|---|---|---|
| Weak edge | p=0.48, R 1.5:1 | −0.02 | 0 → **trade blocked** | blocked |
| Marginal edge | p=0.52, R 1.5:1 | 0.013 | ≈ 1.0 | 1.0% |
| Strong edge | p=0.58, R 2.0:1 | 0.080 | clamped to 2.0 | 2.0% |
| Overwhelming edge | p=0.65, R 2.5:1 | 0.160 | clamped to 2.0 | 2.0% (ceiling) |

The ±2× clamp is load-bearing. Without it, Kelly routinely recommends 5–15% of bankroll per trade, which is behaviorally unacceptable even if mathematically "optimal."

### 4.5 Composition with existing MIN arms

After `effective_risk_percent` is computed, the existing pipeline runs unchanged:

```
size_from_risk = (balance × effective_risk_percent / 100) / stop_distance
size = MIN(size_from_risk, max_risk, max_position, margin_capacity)
```

Kelly slots into the first MIN arm. All existing safety caps still apply.

---

## 5. Unlock Logic

```sql
SELECT COUNT(*) AS tagged_closed
FROM journal_trades
WHERE user_id = $1 AND setup_tag IS NOT NULL;
```

- `tagged_closed < 30` → popup toggle is **locked**; shows `"Dynamic Risk unlocks after 30 tagged closes (N/30)"`.
- `tagged_closed ≥ 30` → toggle is **unlocked but off** by default. User must flip it.
- No auto-enable. No gamification. Frame as a data-quality threshold, not a reward.

Once unlocked and toggled on:
- Every trade submission runs the calibration query.
- For each setup, Bayesian shrinkage blends toward user's global prior.
- First-time users of a new setup inherit their overall performance — no cold-start cliff.

---

## 6. Data Model Changes

### 6.1 New migration — `add_qnt_columns.up.sql`

```sql
-- Per-user preference: is dynamic risk mode on?
-- Store as JSONB extension to an existing user_settings row, OR a new table.
-- Testudo has no dedicated user_settings table today (RiskConfig is cached, not persisted).
-- Create the table here:
CREATE TABLE IF NOT EXISTS user_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Kelly inputs captured at size-decision time.
ALTER TABLE journal_trades
    ADD COLUMN kelly_inputs JSONB NULL;

-- Index for fast "unlock check" (COUNT of tagged trades per user).
-- Existing idx_journal_trades_user_setup (user_id, setup_tag) WHERE setup_tag IS NOT NULL
-- already supports this — no new index needed.
```

The `settings` JSONB on `user_settings` starts with:
```json
{
  "dynamic_risk_enabled": false,
  "dynamic_risk_unlocked_at": null
}
```

Future settings (timezone, notification preferences, etc.) reuse this JSONB without new migrations. This is the right home for per-user preferences Testudo doesn't currently have anywhere.

### 6.2 `kelly_inputs` JSONB shape

Stored only when dynamic mode was active for the trade:

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

When dynamic mode is off, `kelly_inputs` is NULL.

---

## 7. Code Locations

### Backend (Rust)

| File | Change |
|---|---|
| `crates/sqlx_postgres/migrations/…_add_qnt_columns.{up,down}.sql` | NEW. `user_settings` table + `kelly_inputs` column. |
| `crates/common_utils/src/risk/calibration.rs` | NEW. `CalibrationEngine` struct: `load_prior(user_id)`, `load_setup(user_id, setup_tag)`, `shrink(setup, prior, k)`. Pure math; no I/O. |
| `crates/common_utils/src/risk/kelly.rs` | NEW. `quarter_kelly(p, avg_r_win, avg_r_loss)`, `edge_multiplier(quarter_kelly)`, `effective_risk_percent(baseline, multiplier)`. Pure functions. |
| `crates/common_utils/src/risk/sizing.rs` | MODIFIED. Extend `SizingMethod` enum with `CalibratedKelly`. Route through calibration + kelly when user_settings.dynamic_risk_enabled. |
| `crates/common_utils/src/risk/config.rs` | MODIFIED. Add `pub dynamic_risk_enabled: bool` field to `RiskConfig`. Load from `user_settings` JSONB. |
| `crates/router/src/routes/user_settings.rs` | NEW. `GET /api/v1/user/settings`, `PATCH /api/v1/user/settings`. Thin JSONB update. |
| `crates/router/src/routes/trade_management.rs` | MODIFIED. `create_trade` handler: if dynamic mode on, call `CalibrationEngine` → `kelly` → write `effective_risk_percent` into the sizing call. Record `kelly_inputs` on `journal_trades` at close time. |
| `crates/router/src/routes/journal.rs` | MODIFIED. `GET /api/v1/user/qnt-readiness` endpoint: returns `{ tagged_closed, unlock_at: 30, unlocked: bool }` for popup toggle state. |
| `crates/router/src/services/journal_service.rs` | MODIFIED. Thread `kelly_inputs` into `record_trade_close`'s INSERT (mirrors RSK-02's setup_tag path). |

### Extension (TypeScript / Solid)

| File | Change |
|---|---|
| `src/schemas.ts` | Extend `TradePayloadSchema` with `dynamic_risk_enabled: boolean`. Extend `ExecuteTradeResponseSchema` with optional `sizing_preview: { baseline, effective, multiplier, reasoning }` for inline display. |
| `src/background/api.ts` | `getUserSettings()`, `patchUserSettings()`, `getQntReadiness()`. Add to dispatch table. |
| `src/popup/components/SettingsPanel.tsx` (NEW or extend existing) | Toggle: "Dynamic Risk Mode". Locked/unlocked state. Progress display before unlock. |
| `src/components/TradeForm.tsx` | Inline display row when dynamic mode on and backend returned a `sizing_preview`. Shows `Risk: 1.0% → 1.4% (43 trades, 58% win rate, 1.9R avg)` above confirm button. |
| `src/modal.tsx` | Shadow DOM CSS for the new preview row (muted color, single line, badge styling). |

### Journal

| File | Change |
|---|---|
| `src/api/client.ts` | Extend `JournalTrade` with `kelly_inputs: KellyInputs \| null`. Add `fetchKellyAudit(filters)` for the follow-on audit view (optional MVP). |
| `src/components/trades/TradeRow.tsx` (or wherever rows render) | If `kelly_inputs != null`, show a small badge ("⚡ Kelly-sized") — click for detail modal. |

---

## 8. Vertical Checkpoints

| CP | Scope | Validates |
|---|---|---|
| **CP-1** | `user_settings` table + `kelly_inputs` column + backend settings endpoint + popup toggle (read-only, no Kelly math yet) | End-to-end plumbing: user can see their own settings, toggle is wired to the DB. |
| **CP-2** | `CalibrationEngine` + `kelly.rs` pure-math modules. Unit-tested with fixture trades. No integration yet. | The math works in isolation. Given `(user_id, setup_tag)`, returns correct `p_eff`, `edge_multiplier`, `effective_risk_percent`. |
| **CP-3** | Wire into `create_trade` handler. Dynamic mode on → compute Kelly → use `effective_risk_percent`. Write `kelly_inputs` JSONB at close time. | End-to-end sizing: a trade submitted with dynamic mode on gets Kelly-sized; its `journal_trades` row records the inputs. |
| **CP-4** | Unlock gate: `qnt-readiness` endpoint + locked-toggle UI in popup + pre-30 copy. | New user sees locked toggle with progress. At 30 tagged closes, toggle unlocks. Existing-user with >30 already unlocked. |
| **CP-5** | Inline modal preview — backend returns `sizing_preview` on trade submission's pre-check path; TradeForm renders it above confirm. | UX transparency: user sees 1.0% → 1.4% with reasoning before confirming. |
| **CP-6** | Journal trade-row badge + detail modal showing `kelly_inputs`. | Retrospective auditability: user can inspect Kelly's decision on any historical trade. |

Each CP is independently shippable and testable.

---

## 9. Acceptance Criteria

- [ ] New user with 0 tagged closes sees locked toggle in popup: *"Dynamic Risk unlocks after 30 tagged closes (0/30)"*.
- [ ] User at 30 tagged closes sees unlocked toggle; flipping it ON persists to `user_settings`.
- [ ] Trade submitted with dynamic mode OFF sizes exactly as it does today (zero behavior change for fixed-mode users).
- [ ] Trade submitted with dynamic mode ON and a setup_tag having ≥10 prior closes: `effective_risk_percent` differs from baseline by at most a 2× factor in either direction.
- [ ] Trade submitted with dynamic mode ON and a setup with NO prior closes: `effective_risk_percent` reads from global prior; Kelly inputs recorded.
- [ ] Trade submitted with dynamic mode ON and negative full-Kelly (`p_eff < q_eff / b`): **trade is rejected** with rejection reason *"Calibration shows negative edge for this setup — size = 0."*
- [ ] Inline preview in modal shows: *"Risk: 1.0% → X.X%"* with one-line reasoning (trade count, win rate, avg R).
- [ ] `journal_trades.kelly_inputs` JSONB is populated for every dynamic-mode trade at close time; NULL for fixed-mode trades.
- [ ] Historical trade row in journal shows "⚡ Kelly-sized" badge for trades with non-null `kelly_inputs`.
- [ ] Backend `cargo clippy --all-targets && cargo test` passes with new unit tests for `kelly.rs` and `calibration.rs`.
- [ ] Extension `bun run build` passes (Chrome + Firefox).
- [ ] Journal `bun run build` passes.

---

## 10. Risks

| # | Risk | Mitigation |
|---|---|---|
| 1 | **Sample size is still small at N=30.** Global prior gets real; per-setup priors are noisy. Kelly may size aggressively on a lucky recent streak. | Bayesian shrinkage shifts weight to global prior when per-setup sample is small (K=10). Clamp to ±2× baseline caps sheer magnitude. User still gets inline preview + double-Enter veto. |
| 2 | **Survivorship bias in `journal_trades`.** Only closed trades count; cancelled / abandoned trades don't feed the prior. | Accept as MVP limitation. Document in plan. Re-evaluate if follow-on QNT-02 drift detector shows calibration walking away from reality. |
| 3 | **User closes a bad run and Kelly immediately sizes smaller on next trade, compounding drawdown psychology.** | This is a *feature*, not a bug, per the alpha. Drawdown-scaling is exactly what the shrinkage is for. But document it clearly: dynamic mode will size down when recent performance degrades. |
| 4 | **Untagged trade submits with dynamic mode on — no setup-level signal available.** | Silent fallback to baseline risk_percent (fixed mode behavior). Log at `info` level. Consider nudging user to tag via inline copy: *"Tag this setup to unlock dynamic sizing for it."* (follow-on polish, not blocking.) |
| 5 | **User enables dynamic mode, hates it, disables it mid-streak — half their trades in the window have `kelly_inputs` and half don't, muddying analysis.** | Accept. The JSONB is per-trade truth: if `kelly_inputs IS NULL`, trade was fixed-mode. Journal audit view groups by mode. |
| 6 | **Quarter-Kelly is still mathematically aggressive for trading.** Some practitioners recommend eighth-Kelly or tenth-Kelly. | Our ±2× clamp caps the practical output. Quarter-Kelly + 2× clamp means max output is `2 × baseline`. User who sets baseline=1% maxes at 2% — identical to picking baseline=2% in fixed mode. The clamp *is* the extra safety buffer. |
| 7 | **No detection of "setup concept drift"** (e.g. "breakout" used to work; market regime changed; it now doesn't). The shrinkage uses all history equally. | Deferred to QNT-02 (drift scaler). For MVP, a user seeing obvious drift can turn dynamic mode off and re-evaluate. |

---

## 11. Open Questions (not blocking, resolve during build)

1. **Pseudocount K tunability.** MVP is hard-coded K=10. Should it be user-tunable (advanced setting)? My lean: no — it's a statistical parameter, not a preference. Document and move on.
2. **Reference Kelly value.** MVP uses `Quarter-Kelly(p=0.52, b=1.5) ≈ 0.013`. If this proves consistently off, make it a constant in `config.rs` rather than inline. Low risk.
3. **Should untagged dynamic-mode trades use global-prior Kelly or silently fall back to fixed?** MVP chooses silent fallback (Risk #4). Reconsider if users complain.
4. **Inline preview latency.** `POST /api/v1/trades/preview` is a new endpoint? Or computed client-side from cached priors pushed to extension? MVP: new preview endpoint, separate from submit. Optimizes later.

---

## 12. Follow-on Specs

### QNT-02 — System ECE / Calibration Drift Scaler
Measure how far a setup's recent rolling performance (last 20 closes) has drifted from its full-history prior. Scale `edge_multiplier` down when drift is high (the calibration is no longer trustworthy). No user-facing UX change. ~1 week.

### QNT-03 — Regime-Conditional Calibration
Add `market_regime: 'trend' | 'range' | 'volatile'` optional field in Alt+X modal (mirrors setup_tag UX). Kelly conditions on `(setup, regime)` when both are present. Powers a Markov transition matrix of regime changes. ~2 weeks, needs UX design.

---

## 13. Completion Signal

This design is complete when:
1. All CP-1 through CP-6 delivered.
2. All acceptance criteria checked.
3. Two real user-trades observed end-to-end: one with dynamic mode OFF (baseline behavior preserved), one with dynamic mode ON and a well-calibrated setup (`effective_risk_percent` ≠ baseline, `kelly_inputs` populated in `journal_trades`).
4. Commit message: `feat(qnt-01): calibrated Kelly sizing + Bayesian shrinkage calibration engine`.
5. Design doc promoted from `/QNT-01-calibrated-kelly-sizing-plan.md` (this file) into `.specify/specs/QNT-01-calibrated-kelly-sizing/spec.md`.

---

*Brainstormed 2026-04-18 via superpowers:brainstorming. Decisions locked through Q9. Ready to promote to atomic spec.*
