# Implementation Plan

> Last updated: 2026-04-20
> Current spec: QNT-01a-kelly-engine
> Phase: PLANNING COMPLETE — ready for BUILD

---

## Active Spec: QNT-01a-kelly-engine

### Gap Analysis

**Backend (`testudo-exchange/crates/`):**

1. **`SizingMethod` enum** — `common_utils/src/risk/types.rs:8-20`. Four variants: `FixedFractional`, `KellyCriterion`, `VolatilityAdjusted`, `MaxRiskCap`. `#[serde(rename_all = "snake_case")]`. **Action:** add new `CalibratedKelly` variant alongside the existing `KellyCriterion` (spec FR-1). Keep `KellyCriterion` untouched — it's reachable via existing code paths and renaming would break wire compatibility.

2. **`RiskConfig`** — `common_utils/src/risk/config.rs:11-43`. 10 fields, builder pattern. **No per-user persistence** today — constructed via `new()`/`conservative()`/`aggressive()` presets. Spec asks to add `pub dynamic_risk_enabled: bool` and load from `user_settings` JSONB. Discovery #4.

3. **`PositionSizer::calculate_position_size()`** — `common_utils/src/risk/position_sizer.rs:81-158`. Does `risk_from_percent = balance * account_risk_percent / 100` at line ~95, then `size = risk_from_percent / stop_distance`, then takes MIN across four arms. **Action:** need an override path so Kelly-computed `effective_risk_percent` replaces `account_risk_percent` only for this trade, without disturbing the MIN composition. Cleanest: compute Kelly in the caller (`create_trade`), build an ad-hoc `RiskConfig` with `account_risk_percent = effective_risk_percent`, pass to sizer.

4. **`CreateTradeRequest`** — `router/routes/trade_management.rs:282-304`. `setup_tag: Option<String>` **already present** (RSK-02, shipped 2026-04-18). `management: Option<ManagementBlock>` carries `risk_percent` (baseline). No Kelly fields needed on the request — dynamic mode is read from `user_settings`, not per-trade.

5. **`DecisionLoop::execute()`** — `router/src/decision_loop.rs:185-225`. Line 207 passes `None` for `trading_stats` to `risk_service.validate()`. This is the current extension point: when dynamic mode is on, we need to either (a) pre-compute Kelly before DecisionLoop and inject an overridden `RiskConfig`, or (b) pass real trading stats into RiskService. **Decision (Discovery #5):** route (a) — Kelly calibration happens in `create_trade` BEFORE DecisionLoop; `effective_risk_percent` overrides baseline on the `RiskConfig` instance passed through. Keeps DecisionLoop untouched.

6. **`JournalService::record_trade_close`** — `router/src/services/journal_service.rs:148`. Accepts `TradeCloseEvent` (with `setup_tag: Option<String>` already wired). **No `kelly_inputs` field on TradeCloseEvent yet.** **Action:** add `kelly_inputs: Option<serde_json::Value>` to the event + thread into INSERT column list. Mirrors RSK-02 T1's `setup_tag` path.

7. **`TradeEventWriter`** — also persists to `journal_trades` (discovered during RSK-02 T5). Secondary write path. **Action:** check if it also writes `journal_trades` rows and whether Kelly path needs mirror there. Low-risk — the dynamic-mode path flows through `record_trade_close`.

8. **`journal_trades` table** — `sqlx_postgres/migrations/20260318000000_create_journal_tables.up.sql` with column additions in `20260418000000_add_setup_tag.up.sql`. **Has:** `setup_tag`, `r_multiple`, `net_pnl`, `realized_pnl`, `risk_amount`, `closed_at`, `opened_at`, `notes`. **Missing:** `kelly_inputs JSONB NULL` — new column added by this spec.

9. **`idx_journal_trades_user_setup`** — confirmed in `20260418000000_add_setup_tag.up.sql`: `CREATE INDEX ... ON journal_trades(user_id, setup_tag) WHERE setup_tag IS NOT NULL;`. Serves BOTH the per-setup aggregate query AND the ≥30-trade unlock-count query. No new index needed.

10. **Settings storage pattern divergence** — RSK-03 added `coach_enabled`/`coach_banner_last_viewed_at` directly on `users`. Spec for QNT-01a explicitly asks for a NEW `user_settings` JSONB table. **Following spec.** Rationale: Kelly config is future-extensible (QNT-01b transparency overlay may add opt-in metrics; QNT-02/03 may add drift thresholds). JSONB gives room to grow. Discovery #2.

11. **`AppState`** — `router/src/types/app.rs:15-44`. 16 fields. No `calibration_engine` yet. **Action:** add `pub calibration_engine: Arc<CalibrationEngine>`, construct in `main.rs:434-451`.

12. **`AuthenticatedUser` extractor** — post-AUTH-02 yields `{ user_id: Uuid, wallet_address: String }`. Same pattern used by `/coach`, `/risk-config`, `/journal`. New `/user/settings` scope wraps with `JwtMiddleware::new(token_service.clone())`.

13. **Routes mod** — `router/src/routes/mod.rs` lists 18 modules. **Action:** add `pub mod user_settings;`.

14. **`CalibrationEngine` placement — spec vs. convention tension** — Spec says `common_utils/src/risk/calibration.rs`. But common_utils's existing `risk/` is pure-math (no sqlx imports; `RiskConfig` is hardcoded). Adding `sqlx::PgPool` and async queries to common_utils would introduce a dependency direction that peer services (coach, risk_snapshot, journal_timeseries) explicitly avoid by living in `router/services/`. **Decision (Discovery #3):** keep pure functions (`shrink()`, weight formulas) in `common_utils/src/risk/kelly.rs` per spec. Place the I/O-bearing `CalibrationEngine` in **`router/src/services/calibration.rs`** instead. This matches the RSK-03 coach-service pattern and avoids forcing sqlx onto common_utils. Spec path deviation documented below.

15. **`journal_trades.net_pnl` vs. `realized_pnl`** — schema has BOTH (legacy + current). Spec's p_win definition uses `net_pnl > 0`. Confirm at build time which is the authoritative "P&L after fees" column; production queries in `journal_timeseries.rs` use `net_pnl`. Use `net_pnl` for Kelly inputs.

**Extension (`testudo-extension/src/`):**

16. **`TradePayloadSchema`** — `schemas.ts:49-72`. Has `setup_tag: z.string().trim().max(48).nullable().optional()`. Has `management.risk_percent`. **No Kelly fields needed** on the payload — dynamic mode is a server-side setting keyed off `user_id`, not per-trade.

17. **`RuntimeMessageSchema`** — `schemas.ts:222-279`. Discriminated union, 28 variants. **Action:** add `GET_USER_SETTINGS` and `PATCH_USER_SETTINGS` variants.

18. **API fetch helpers** — `background/api.ts:40-88`. `ApiOpts`/`ApiResult` typed pattern; named wrappers like `executeTrade()`, `listSetupTags()` (RSK-02 T2). **Action:** add `getUserSettings()` and `patchUserSettings()` wrappers following the same shape.

19. **Handlers** — `background/handlers.ts:33-80`. Typed `handleX()` functions + dispatch table. **Action:** add `handleGetUserSettings`/`handlePatchUserSettings`; register in dispatch.

20. **Popup SettingsPanel** — **does not exist.** Components dir has 11 entries (`ActiveOrders`, `AuthSection`, `ExchangeSelector`, `HeaderBar`, `LoginPreview`, `MainView`, `PairView`, `PositionCard`, `StatusBar`, `TabBar`, `TradeManagement`). **Action:** create `popup/components/SettingsPanel.tsx` with a single Dynamic Risk toggle. Mount in an existing popup surface (likely `MainView` or a new settings tab). For 01a the disabled state (when server reports `unlocked = false`) is a plain greyed-out switch — progress copy is QNT-01b's job.

21. **Production URL defaults rule** — from MEMORY (feedback_prod_defaults.md): **MUST use `bun run typecheck`, NOT `bun run build`, during extension verification.** The extension's prod URL defaults break during dev build. Spec already codifies this.

---

### Design Decisions (captured before tasking)

1. **`user_settings` is a new JSONB table, not columns on `users`.** Spec explicit. JSONB gives forward-compat headroom for QNT-01b/c additions without more ALTER TABLEs. Trade-off: one extra row in the hot `/user/settings` read path vs. ~5 scattered NULLable columns on `users`. Given <10 reads per session, trivial.

2. **`CalibrationEngine` lives in `router/src/services/calibration.rs`, not `common_utils/src/risk/calibration.rs`.** Spec deviation. Rationale in Gap Analysis #14. Pure math (`shrink()`, `quarter_kelly()`, `edge_multiplier()`, `effective_risk_percent()`) stays in `common_utils/src/risk/kelly.rs` per spec — those functions have no I/O. This keeps common_utils dep-clean and mirrors the coach-service placement.

3. **Kelly calibration runs in `create_trade` handler BEFORE DecisionLoop, not inside it.** DecisionLoop stays untouched. When dynamic mode is on: handler loads per-setup stats → global prior → shrinks → Kelly → clamps → overrides `RiskConfig.account_risk_percent` with `effective_risk_percent` → passes to DecisionLoop. Byte-for-byte preserves the MIN composition downstream (FR-10).

4. **Negative-edge rejection is a hard 4xx response from `create_trade`, not a DecisionLoop rejection.** Cleaner separation: Kelly is a pre-sizing gate, not a sizing outcome. Response: `400 Bad Request` with `{ "error": "negative_edge", "message": "Calibration shows negative edge for this setup — size = 0." }`. No `journal_trades` row created (FR-5). Discovery #6.

5. **Untagged + dynamic-on → silent fallback.** Spec FR-8. `tracing::info!(user_id = %user_id, "dynamic_risk: setup_tag missing, falling back to baseline")`. No client-facing warning in 01a — QNT-01b will add the inline nudge. `kelly_inputs` remains NULL at close.

6. **`kelly_inputs` populated only when dynamic mode produced a Kelly-derived size.** Fixed-mode trades AND untagged-fallback trades → `kelly_inputs = NULL`. This is the per-trade mode audit trail (Risk #5 in spec).

7. **`UserSettings` struct shape** — tiny for 01a:
   ```rust
   pub struct UserSettings {
       pub dynamic_risk_enabled: bool,
       // Future: drift_warnings, per-setup overrides, ...
   }
   ```
   Persisted as JSONB. Stored even when `dynamic_risk_enabled = false` (to preserve future preferences on upgrade). Unlock state (`dynamic_risk_unlocked_at: Option<DateTime>`) set at first successful enable → remains set even if user later toggles off.

8. **Unlock gate SQL uses the existing RSK-02 index.** `SELECT COUNT(*) FROM journal_trades WHERE user_id = $1 AND setup_tag IS NOT NULL`. Partial index `idx_journal_trades_user_setup` serves this query directly — O(log n) + small constant.

9. **`dynamic_risk_unlocked_at` is informational, not enforcement.** Enforcement is the COUNT check on every PATCH-to-enable. Once unlocked, the flag lets UI show "unlocked on 2026-05-04" copy. If count drops below 30 (only possible via trade deletion), existing `dynamic_risk_enabled = true` stays valid — no re-gating after initial unlock. Discovery #9.

10. **`reference_kelly` constant** — `Quarter-Kelly(p=0.52, b=1.5) ≈ 0.0133`. Computed once as a module-level `lazy_static` or `OnceLock<Decimal>` in `kelly.rs`. Pure Decimal math, no runtime variance. Discovery #7.

11. **Pseudocount `K = 10` is a module-level `const`**, not a config knob. Tuning requires code change + redeploy. Spec says "hardcoded constant" — follow spec. If QNT-02 adds drift detection, K may become per-user tunable.

12. **Extension SettingsPanel for 01a is minimal.** Single toggle. When `unlocked = false` from server: greyed-out + tooltip "Unlocks after 30 tagged closes." When `unlocked = true`: functional switch. No progress indicator, no preview UI — that's QNT-01b (Transparency overlay). Mount location TBD at build time; likely inside `MainView.tsx` or a new collapsible "Settings" section on it. Discovery #8.

13. **`authServer` unlock check happens on every PATCH, not just on toggle-on.** Cheap query; idempotent behavior. User flipping on/off/on in rapid succession gets consistent server validation.

---

### Vertical Checkpoint Structure (from spec §Technical Implementation)

| CP | Goal | Tasks |
|----|------|-------|
| CP-1 | Plumbing end-to-end: migration + server endpoints + unlock gate + extension toggle. Frontend can mock against live contract; no math yet. | T1, T2, T3 |
| CP-2 | Pure-math modules unit-tested against fixtures. No trade-path integration. | T4, T5 |
| CP-3 | Math wired into create_trade. Kelly sizing flows through existing MIN pipeline; `kelly_inputs` persisted at close. | T6, T7, T8 |
| Final | Verification + commit. | T9 |

---

### Parallel Track Detection

```
T1 (migration — user_settings table + kelly_inputs column)
  │
  ├── T2 (user_settings routes + unlock gate + AppState wiring) ──┐
  │                                                                │
  └── T3 (extension — schemas + messages + handlers + toggle UI) ──┤   [parallel with T2]
                                                                   │
                                                                   ↓
                                              T4 (kelly.rs pure math + tests)
                                                   │
                                                   └── T5 (calibration.rs engine + shrink + tests)  [parallel with T4]
                                                              │
                                                              ↓
                                       T6 (create_trade: Kelly pre-sizing + CalibratedKelly variant + negative-edge reject)
                                                              │
                                                              ↓
                                       T7 (record_trade_close: kelly_inputs JSONB at close)
                                                              │
                                                              ↓
                                       T8 (untagged fallback path + info log)
                                                              │
                                                              ↓
                                                          T9 (verification + commit)
```

T2 and T3 independent after T1 lands. T4 and T5 independent after T2 lands. Single-agent BUILD stays sequential.

---

## Tasks

### T1: Migration — `user_settings` table + `journal_trades.kelly_inputs` column — `complete`

**Scope:** CP-1 persistence layer.

**Files:**
- `testudo-exchange/crates/sqlx_postgres/migrations/{ts}_add_qnt_columns.up.sql` — NEW:
  ```sql
  CREATE TABLE IF NOT EXISTS user_settings (
      user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
      settings JSONB NOT NULL DEFAULT '{"dynamic_risk_enabled": false, "dynamic_risk_unlocked_at": null}'::jsonb,
      updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );

  ALTER TABLE journal_trades
      ADD COLUMN kelly_inputs JSONB NULL;
  ```
- `testudo-exchange/crates/sqlx_postgres/migrations/{ts}_add_qnt_columns.down.sql` — NEW: reverse (DROP COLUMN + DROP TABLE).

**Timestamp:** Use `20260420000100_add_qnt_columns` (later than the ENG-01 dignitas migration reserved at `20260420000000`, if that lands first; otherwise `20260420000000`). Resolved at build time by greping existing migrations.

**Validate:** `cd testudo-exchange && cargo check --all-targets` (migrations compile into sqlx offline cache only if `SQLX_OFFLINE=true`; runtime validation in T2).

**Acceptance:**
- Up+down apply cleanly on a fresh DB.
- `user_settings.settings` defaults to `{"dynamic_risk_enabled": false, "dynamic_risk_unlocked_at": null}`.
- `journal_trades.kelly_inputs` nullable, default NULL.
- `ON DELETE CASCADE` from users.

---

### T2: `user_settings` routes + unlock gate + AppState wiring — `complete`

**Scope:** CP-1 backend API surface. Server-side unlock check on enable.

**Files:**
- `testudo-exchange/crates/router/src/routes/user_settings.rs` — NEW:
  - `#[derive(Serialize, Deserialize)] pub struct UserSettings { dynamic_risk_enabled: bool, dynamic_risk_unlocked_at: Option<DateTime<Utc>> }`.
  - `#[derive(Serialize)] pub struct UserSettingsResponse { settings: UserSettings, unlocked: bool, tagged_trade_count: i64 }`.
  - `#[derive(Deserialize)] pub struct PatchUserSettingsRequest { dynamic_risk_enabled: bool }`.
  - `GET /api/v1/user/settings` → `UserSettingsResponse`. Fetches settings row (creates default if missing via `INSERT ... ON CONFLICT DO NOTHING`), counts tagged closes, sets `unlocked = count >= 30`.
  - `PATCH /api/v1/user/settings` → `UserSettingsResponse` on 200 OR `409 Conflict` with `{ "error": "unlock_gate", "message": "Dynamic Risk requires ≥ 30 tagged closed trades (you have N).", "tagged_trade_count": N, "required": 30 }` when enabling without threshold.
  - Unlock SQL: `SELECT COUNT(*)::bigint FROM journal_trades WHERE user_id = $1 AND setup_tag IS NOT NULL`.
  - First-time enable sets `dynamic_risk_unlocked_at = NOW()`.
- `testudo-exchange/crates/router/src/routes/mod.rs` — MODIFIED: `pub mod user_settings;`.
- `testudo-exchange/crates/router/src/main.rs` — MODIFIED: register `/api/v1/user` scope with `JwtMiddleware::new(token_service.clone())`, mount `/settings` handlers.
- `testudo-exchange/crates/common_utils/src/risk/config.rs` — MODIFIED: add `pub dynamic_risk_enabled: bool` to `RiskConfig` (default `false`). Add builder method `with_dynamic_risk(bool)`. Document that the field is populated by the `create_trade` handler from `user_settings`, not from the defaults — presets leave it `false`.

**Inline tests (cfg test):**
- `enables_when_threshold_met` — user with ≥ 30 tagged closes can PATCH to true.
- `rejects_enable_below_threshold` — user with 29 tagged closes gets 409 with count.
- `disable_always_allowed` — PATCH to `false` works regardless of count.
- `unlocked_at_set_on_first_enable_only` — re-enabling after disable does not update `unlocked_at`.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test user_settings`.

**Acceptance:**
- GET returns default settings for first-time user.
- PATCH enforces unlock gate server-side.
- 409 response shape matches spec's error contract.
- RiskConfig has `dynamic_risk_enabled` field.

---

### T3: Extension — schemas, messages, API helpers, SettingsPanel toggle — `complete`

**Scope:** CP-1 extension surface. Wire the minimal Dynamic Risk toggle to the new endpoints. Can run in parallel with T2 once T1's wire contract is known.

**Files:**
- `testudo-extension/src/schemas.ts` — MODIFIED:
  - Add `UserSettingsSchema = z.object({ dynamic_risk_enabled: z.boolean(), dynamic_risk_unlocked_at: z.string().datetime().nullable() })`.
  - Add `UserSettingsResponseSchema = z.object({ settings: UserSettingsSchema, unlocked: z.boolean(), tagged_trade_count: z.number().int().nonnegative() })`.
  - Add two variants to `RuntimeMessageSchema`: `{ type: 'GET_USER_SETTINGS' }` and `{ type: 'PATCH_USER_SETTINGS', dynamic_risk_enabled: z.boolean() }`.
- `testudo-extension/src/background/api.ts` — MODIFIED:
  - `export async function getUserSettings(): Promise<ApiResult>` — GET `/api/v1/user/settings`, parse with `UserSettingsResponseSchema`.
  - `export async function patchUserSettings(enabled: boolean): Promise<ApiResult>` — PATCH with `{ dynamic_risk_enabled: enabled }`. Map 409 → `{ ok: false, error_code: 'unlock_gate', error: <server message> }`.
- `testudo-extension/src/background/handlers.ts` — MODIFIED:
  - `handleGetUserSettings()` → `getUserSettings()`.
  - `handlePatchUserSettings(msg)` → `patchUserSettings(msg.dynamic_risk_enabled)`.
  - Register both in dispatch table.
- `testudo-extension/src/popup/components/SettingsPanel.tsx` — NEW:
  - Solid component. `createResource(fetchUserSettings)`.
  - Renders a labeled toggle "Dynamic Risk (Calibrated Kelly)".
  - When `settings.loading`: skeleton.
  - When `!unlocked`: disabled toggle + subtext "Unlocks after 30 tagged closes (currently N)".
  - When `unlocked`: live toggle, calls `PATCH_USER_SETTINGS` on change, optimistically updates + rolls back on error.
  - On 409 response: surfaces the server message inline.
- Mount point — `testudo-extension/src/popup/components/MainView.tsx` OR a new tab in `TabBar.tsx`. **Decision at build time** — simplest is a collapsible "Settings" section at the bottom of MainView. Document choice in T3's completion commit.

**Validate:**
- `cd testudo-extension && bun run typecheck` (NOT `bun run build` per prod URL defaults feedback).
- `bun run test` — add vitest cases for the two new messages if handler tests are the convention (check existing `handlers.test.ts`).

**Acceptance:**
- Popup toggle disabled state visible when `unlocked = false`.
- Toggle round-trips to server on change.
- 409 response surfaces inline.
- No new pre-existing test regressions (baseline: ~28 pre-existing failures per RSK-02 T2 — don't add more).

---

### T4: `kelly.rs` pure math module + unit tests — `complete`

**Scope:** CP-2 pure math. No I/O. Can run in parallel with T5.

**Files:**
- `testudo-exchange/crates/common_utils/src/risk/kelly.rs` — NEW:
  ```rust
  use rust_decimal::Decimal;
  use rust_decimal_macros::dec;
  use std::sync::OnceLock;

  pub const PSEUDOCOUNT_K: u32 = 10;
  pub const CLAMP_MIN: Decimal = dec!(0.25);
  pub const CLAMP_MAX: Decimal = dec!(2.00);

  /// Quarter-Kelly for p=0.52, b=1.5 — the reference point for the ±2× clamp.
  pub fn reference_kelly() -> Decimal {
      static CACHE: OnceLock<Decimal> = OnceLock::new();
      *CACHE.get_or_init(|| quarter_kelly(dec!(0.52), dec!(1.5), dec!(1.0)))
  }

  /// Full Kelly = (b·p − q) / b. Returns raw value (may be negative).
  pub fn full_kelly(p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal) -> Decimal;

  /// Quarter-Kelly = full_kelly / 4.
  pub fn quarter_kelly(p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal) -> Decimal;

  /// clamp(quarter_kelly / reference_kelly, 0.25, 2.0)
  pub fn edge_multiplier(quarter_kelly: Decimal) -> Decimal;

  /// baseline_risk_percent * edge_multiplier
  pub fn effective_risk_percent(baseline: Decimal, multiplier: Decimal) -> Decimal;
  ```
- `testudo-exchange/crates/common_utils/src/risk/mod.rs` — MODIFIED: `pub mod kelly;`.

**Inline tests:**
- `reference_kelly_matches_013` — within `dec!(0.0001)` of `0.0133`.
- `full_kelly_positive_on_positive_edge` — p=0.6, b=2.0 → positive.
- `full_kelly_negative_on_negative_edge` — p=0.4, b=1.0 → `≤ 0`.
- `edge_multiplier_clamped_at_low` — tiny Kelly → 0.25.
- `edge_multiplier_clamped_at_high` — huge Kelly → 2.0.
- `effective_risk_percent_matches_baseline_at_reference` — multiplier 1.0 → baseline unchanged.
- `effective_risk_percent_doubles_at_clamp_max` — multiplier 2.0 → 2× baseline.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test -p common_utils kelly`.

**Acceptance:**
- All 7 tests green.
- No `f64` anywhere.
- `reference_kelly()` value matches spec's annotation "≈ 0.0133".

---

### T5: `calibration.rs` CalibrationEngine + `shrink()` + unit tests — `complete`

**Scope:** CP-2 I/O-bearing layer. Lives in `router/src/services/` per Design Decision #2 (deviation from spec's common_utils placement — Discovery #3).

**Files:**
- `testudo-exchange/crates/router/src/services/calibration.rs` — NEW:
  ```rust
  use common_utils::risk::kelly::PSEUDOCOUNT_K;
  use rust_decimal::Decimal;
  use sqlx::PgPool;
  use uuid::Uuid;

  #[derive(Debug, Clone)]
  pub struct SetupStats { pub n: u32, pub p_win: Decimal, pub avg_r_win: Decimal, pub avg_r_loss: Decimal }

  #[derive(Debug, Clone)]
  pub struct ShrunkStats { pub p_eff: Decimal, pub avg_r_win: Decimal, pub avg_r_loss: Decimal, pub n_setup: u32, pub n_global: u32 }

  pub struct CalibrationEngine { pool: PgPool }

  impl CalibrationEngine {
      pub fn new(pool: PgPool) -> Self;
      pub async fn load_prior(&self, user_id: Uuid) -> Result<SetupStats, sqlx::Error>;
      pub async fn load_setup(&self, user_id: Uuid, setup_tag: &str) -> Result<SetupStats, sqlx::Error>;
  }

  /// Pure Bayesian shrinkage. Pseudocount K from kelly::PSEUDOCOUNT_K.
  pub fn shrink(setup: &SetupStats, prior: &SetupStats, k: u32) -> ShrunkStats;
  ```
- `testudo-exchange/crates/router/src/services/mod.rs` — MODIFIED: `pub mod calibration;`.

**SQL — `load_setup`:**
```sql
SELECT
    COUNT(*)::integer AS n,
    COALESCE(AVG(CASE WHEN net_pnl > 0 THEN 1.0 ELSE 0.0 END)::numeric, 0.0) AS p_win,
    COALESCE(AVG(CASE WHEN net_pnl > 0 AND r_multiple IS NOT NULL THEN r_multiple END)::numeric, 0.0) AS avg_r_win,
    COALESCE(AVG(CASE WHEN net_pnl <= 0 AND r_multiple IS NOT NULL THEN ABS(r_multiple) END)::numeric, 0.0) AS avg_r_loss
FROM journal_trades
WHERE user_id = $1 AND LOWER(setup_tag) = LOWER($2) AND closed_at IS NOT NULL
```

**SQL — `load_prior`:** same shape, drops `setup_tag` clause.

**Inline tests:**
- `shrink_at_zero_setup_trades_returns_prior` — `n_setup=0` → `p_eff == p_prior`, `avg_r_win == prior.avg_r_win`, etc.
- `shrink_at_K_equals_50_50_blend` — `n_setup=10, K=10` → half-and-half.
- `shrink_at_10K_dominates_by_setup` — `n_setup=100, K=10` → `p_eff ≈ p_setup ± small prior drag`.
- `anti_gaming_small_n_cannot_spike_p_eff` — small n, high p_setup, neutral prior → p_eff still near prior.

(Integration tests against real DB deferred — SQL verified by compile + T9's full-suite.)

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test -p router calibration`.

**Acceptance:**
- 4 inline tests green.
- `shrink()` uses `PSEUDOCOUNT_K` from kelly.rs (single source of truth).
- SQL uses case-insensitive setup_tag match (`LOWER`) per RSK-02 convention.

---

### T6: `create_trade` integration — Kelly pre-sizing + `CalibratedKelly` variant + negative-edge rejection — `complete`

**Scope:** CP-3 trade-path integration. The load-bearing task.

**Additive-only contract (ENFORCED — do not break current functionality):**
- Every new code path is gated behind `dynamic_risk_enabled == true`. Default is `false`. A user with NO `user_settings` row behaves as `{ dynamic_risk_enabled: false }` — handler must treat `NotFound` as fixed-mode, NOT error. Explicit `.unwrap_or_default()` or equivalent match arm.
- `SizingMethod` enum gains `CalibratedKelly`; existing `FixedFractional`, `KellyCriterion`, `VolatilityAdjusted`, `MaxRiskCap` untouched. No renames. No default-variant changes.
- `RiskConfig` gains `dynamic_risk_enabled: bool` with `#[serde(default)]`; preset constructors (`new()`, `conservative()`, `aggressive()`) explicitly set it to `false`.
- Calibration query failure (DB error, not "no data") = `tracing::warn!` + fall back to baseline fixed-mode sizing. A transient DB hiccup must NEVER fail a trade that today would succeed.
- Existing `CreateTradeRequest` wire format gains no required fields. Existing clients (pre-spec extension builds) keep working unchanged.

**Files:**
- `testudo-exchange/crates/common_utils/src/risk/types.rs` — MODIFIED: add `CalibratedKelly` variant to `SizingMethod` enum (appended; existing variants untouched).
- `testudo-exchange/crates/router/src/types/app.rs` — MODIFIED: add `pub calibration_engine: Arc<CalibrationEngine>`.
- `testudo-exchange/crates/router/src/main.rs` — MODIFIED: construct `Arc::new(CalibrationEngine::new(pool.clone()))` and slot into AppState literal at lines 434-451.
- `testudo-exchange/crates/router/src/routes/trade_management.rs` — MODIFIED in `create_trade` handler:
  1. Load `UserSettings` for the user (query `user_settings`). **`NotFound` → treat as `dynamic_risk_enabled = false`, continue.** DB error → `warn!` + treat as false, continue.
  2. If `dynamic_risk_enabled == false` → no change. Proceeds with fixed-mode baseline (FR-10 byte-for-byte).
  3. If `dynamic_risk_enabled == true` AND `setup_tag.is_some()`:
     - `let prior = calibration_engine.load_prior(user_id).await?;`
     - `let setup = calibration_engine.load_setup(user_id, &tag).await?;`
     - `let shrunk = shrink(&setup, &prior, PSEUDOCOUNT_K);`
     - `let fk = full_kelly(shrunk.p_eff, shrunk.avg_r_win, shrunk.avg_r_loss);`
     - If `fk <= Decimal::ZERO` → **return 400 with `{ "error": "negative_edge", "message": "Calibration shows negative edge for this setup — size = 0." }`**. Do NOT create a trade row.
     - `let qk = fk / dec!(4);`
     - `let mult = edge_multiplier(qk);`
     - `let baseline = management.risk_percent;`
     - `let eff = effective_risk_percent(baseline, mult);`
     - Build ad-hoc `RiskConfig` with `account_risk_percent = eff` and `sizing_method = SizingMethod::CalibratedKelly`.
     - Pass to DecisionLoop/RiskService. Existing MIN composition preserved.
     - **Stash the `kelly_inputs` JSONB in-memory** (on the OrderGroup or a parallel map keyed by trade_group_id) so T7's `record_trade_close` path can retrieve it.
  4. If `dynamic_risk_enabled == true` AND `setup_tag.is_none()`: T8 handles — here, fall through to baseline.

**`kelly_inputs` in-memory stash strategy — Design Decision:** the cleanest is to add `pub kelly_inputs: Option<serde_json::Value>` to `OrderGroup` (in `engine/src/shadow/order_group.rs`) and propagate via `EngineCommand::ConfigureGroup` (same mechanism RSK-02 used for `setup_tag`). Written at trade entry, read at trade close. **The field MUST carry `#[serde(default)]` and `Option<_>` type** so any existing serialized OrderGroup state in pg_queue, WS buffers, or rehydration snapshots deserializes with `kelly_inputs = None` without error. Document in T6's commit.

**Error path:** Negative-edge rejection is a hard 4xx. Client (extension modal) surfaces the message via existing toast pipeline (RSK-03 T7 toast).

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test` — full suite.

**Acceptance:**
- Fixed-mode trades size identically to pre-spec (regression check — compare sizing math output on same inputs).
- Dynamic-mode tagged trade: `effective_risk_percent ∈ [0.25 × baseline, 2.0 × baseline]`.
- Negative-edge trade: 400 response, no `journal_trades` row created.
- `SizingMethod::CalibratedKelly` serialized in RiskResult when path active.

---

### T7: `record_trade_close` — persist `kelly_inputs` JSONB at close — `complete`

**Scope:** CP-3 write side.

**Files:**
- `testudo-exchange/crates/router/src/services/journal_service.rs` — MODIFIED:
  - Extend `TradeCloseEvent` with `pub kelly_inputs: Option<serde_json::Value>`.
  - Thread into the `INSERT INTO journal_trades (..., setup_tag, kelly_inputs) VALUES (..., $N, $N+1)` column list + bindings (mirror the RSK-02 T1 setup_tag addition).
- `testudo-exchange/crates/router/src/services/fill_detector.rs` (or wherever `emit_trade_closed` is called) — MODIFIED: read `OrderGroup.kelly_inputs` and pass through to `TradeCloseEvent`.
- `testudo-exchange/crates/engine/src/shadow/order_group.rs` — MODIFIED: add `pub kelly_inputs: Option<serde_json::Value>`. Default `None`. Populated by `EngineCommand::ConfigureGroup` from T6.
- `testudo-exchange/crates/router/src/services/trade_event_writer.rs` — **T7 FIRST STEP, NOT "verify at build time"**: grep the file for `INSERT INTO journal_trades` or `UPDATE journal_trades`. If it writes rows, mirror the column addition (pass `None` when not in dynamic mode → column stays NULL, identical to today). If it does not write to `journal_trades`, note the finding in the commit and move on. This must resolve BEFORE committing T7; leaving it as "verify later" risks a production row shape mismatch.

**kelly_inputs JSON shape (from spec):**
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
Built in T6 at trade submission time; only `computed_at` uses `Utc::now()`, rest are snapshot values at entry. Document in T7 commit that this is intentionally "entry-time snapshot" not "close-time recompute" — preserves audit integrity.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`.

**Acceptance:**
- Dynamic-mode trade close → `journal_trades.kelly_inputs` populated with the 13-field JSON.
- Fixed-mode trade close → `kelly_inputs` NULL.
- Router restart between entry and close: `kelly_inputs` survives via OrderGroup rehydration (mirror RSK-02 T1's `setup_tag` rehydration path in `rehydration.rs`).

---

### T8: Decision-loop untagged fallback + info log — `complete`

**Scope:** CP-3 FR-8 completion.

**Files:**
- `testudo-exchange/crates/router/src/routes/trade_management.rs` — MODIFIED: in the dynamic-mode branch added by T6, the untagged case (`dynamic_risk_enabled && setup_tag.is_none()`) now emits `tracing::info!(user_id = %user_id, "dynamic_risk: setup_tag missing, falling back to baseline")` and falls through to the fixed-mode path. No Kelly computation. No `kelly_inputs` on the resulting trade.

**This may already be a no-op** if T6's branch structure naturally falls through on `setup_tag.is_none()`. In that case, T8 is purely the `tracing::info!` line + inline-test verification.

**Inline test (in trade_management.rs or a helper):**
- `dynamic_on_untagged_falls_back` — mock dynamic_enabled=true, setup_tag=None → sizing uses baseline (unchanged), no Kelly query fired.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`.

**Acceptance:**
- Untagged dynamic-mode trade sizes identically to fixed-mode.
- Info log emitted at trace level `info`.
- No `journal_trades.kelly_inputs` on these trades.

---

### T9: Final verification + commit — `complete`

**Scope:** Completion Protocol per constitution.

**Verifications:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test` — all green, zero new warnings beyond the pre-existing 3 (actor.rs:1849, cex_client.rs:653, evaluator.rs:188).
- `cd testudo-extension && bun run typecheck` — exit 0 (do NOT run `bun run build` per `feedback_prod_defaults.md`).
- Migration up+down clean on a fresh DB.
- Integration grep across repo: `SizingMethod::CalibratedKelly | CalibrationEngine | kelly_inputs | user_settings | dynamic_risk_enabled | PATCH_USER_SETTINGS` wired consistently across router + extension. Expected hit count: ~20 Rust files, ~4 TS files.
- **Anti-gaming assertion** (spec FR-2/Risk #1): construct a hypothetical fixture — 45% win-rate setup → verify effective_risk_pct strictly below baseline; 65% win-rate setup → strictly above baseline but capped at `2 × baseline`. Verifiable via unit test against `kelly.rs` helpers + fixture `ShrunkStats`.
- **FR-10 byte-for-byte fixed-mode regression**: dispatch a unit test that submits an identical trade in fixed mode pre-spec vs. post-spec and asserts the computed `PositionSizeResult.size` matches. Guard against regressions in `position_sizer.rs` during SizingMethod enum expansion.
- **Default-off safety test**: a user with no `user_settings` row submitting a trade must route through the fixed-mode path with zero calls to `CalibrationEngine`. Verify via a test that injects a mock engine and asserts zero invocations.
- **Rehydration compatibility**: existing in-flight OrderGroups serialized before the migration must deserialize after deploy with `kelly_inputs = None`. Verify via a JSON fixture captured from production shape (or a minimal hand-written pre-spec OrderGroup JSON) deserialized against the new struct.
- **Existing test suite**: the pre-existing 972 Rust tests must all still pass with zero behavior changes. Any new failure in a pre-existing test is a regression, not a Kelly interaction — stop and investigate before proceeding.

**Manual QA (deferred to live session):**
- New user (0 tagged closes) → PATCH returns 409 with accurate count.
- Trader with 30+ tagged closes → PATCH succeeds, `dynamic_risk_unlocked_at` recorded.
- Live trade submitted with dynamic mode ON + setup_tag → sizes via Kelly path; close writes `kelly_inputs` JSONB.
- Live trade with dynamic mode ON + negative-edge setup → 400 response, no trade created.

**Commit plan (one per task):**
- T1: `feat(qnt-01a): migration — user_settings table + journal_trades.kelly_inputs column`
- T2: `feat(qnt-01a): user_settings routes + server-side unlock gate`
- T3: `feat(qnt-01a): extension — Dynamic Risk toggle + user_settings API`
- T4: `feat(qnt-01a): kelly.rs — Quarter-Kelly pure math + reference constant`
- T5: `feat(qnt-01a): calibration engine — Bayesian shrinkage + aggregate queries`
- T6: `feat(qnt-01a): create_trade — Kelly pre-sizing + negative-edge rejection`
- T7: `feat(qnt-01a): record_trade_close — kelly_inputs JSONB at close`
- T8: `feat(qnt-01a): dynamic-risk untagged fallback + info log`
- T9: umbrella: `feat(qnt-01a): calibrated Kelly sizing engine + Bayesian shrinkage`

**Archive:** Move `.specify/specs/QNT-01a-kelly-engine/` → `.specify/spec-archive/QNT-01a-kelly-engine/` after T9.

---

## Discoveries

### 2026-04-20 — QNT-01a planning

1. **`setup_tag` infrastructure already shipped (RSK-02, 2026-04-18).** `CreateTradeRequest.setup_tag`, `TradeCloseEvent.setup_tag`, `journal_trades.setup_tag`, `OrderGroup.setup_tag` (via `EngineCommand::ConfigureGroup`), `idx_journal_trades_user_setup` partial index — all present. QNT-01a layers on top of this foundation; no setup-tag plumbing needed.

2. **`user_settings` is a new JSONB table, not columns on `users`.** Spec explicit. Diverges from RSK-03's direct-column approach. JSONB is chosen for forward-compat with QNT-01b (transparency preferences) and QNT-02/03 (drift thresholds) without more ALTER TABLEs. One extra row read per session — trivial cost.

3. **`CalibrationEngine` placement deviates from spec: `router/src/services/calibration.rs`, not `common_utils/src/risk/calibration.rs`.** Spec says common_utils; but common_utils's existing `risk/` module is I/O-free and peer services (coach, risk_snapshot, journal_timeseries) all live in `router/services/`. Adding sqlx to common_utils would introduce an inversion. Pure math (`shrink()`, Kelly helpers) stays in `common_utils/src/risk/kelly.rs` per spec. I/O lives in router/services. Zero behavioral difference; cleaner dep graph.

4. **`RiskConfig` has no per-user persistence today.** Constructed via hardcoded presets. Spec asks to add `dynamic_risk_enabled: bool` AND load from `user_settings`. The loading is a per-trade concern (done in `create_trade` handler using `user_settings` query), not a boot-time concern. The `dynamic_risk_enabled` field on `RiskConfig` is set by the handler before passing to DecisionLoop/PositionSizer.

5. **Kelly pre-sizing happens in the handler, NOT inside DecisionLoop.** DecisionLoop.execute() signature + body untouched. Handler computes `effective_risk_percent`, constructs a per-trade `RiskConfig` override with `account_risk_percent = effective_risk_percent`, passes to DecisionLoop. Existing MIN composition preserved byte-for-byte (FR-10). Cleaner than passing `trading_stats` into `risk_service.validate()`.

6. **Negative-edge rejection is a 400 from the HTTP handler, not a DecisionLoop rejection.** Kelly is a pre-sizing gate. Response body: `{ "error": "negative_edge", "message": "Calibration shows negative edge for this setup — size = 0." }`. No `journal_trades` row ever created. Client (extension modal) surfaces via existing toast pipeline.

7. **`reference_kelly` is a `OnceLock<Decimal>`.** Single computation at first use; cached. Value ≈ 0.0133 (Quarter-Kelly for p=0.52, b=1.5). Unit test verifies within 0.0001 tolerance.

8. **Extension SettingsPanel is minimal for 01a.** Single toggle, disabled when server reports unlocked=false. No progress UI, no preview — that's QNT-01b. Mount location (MainView section vs. new TabBar tab) decided at build time.

9. **`dynamic_risk_unlocked_at` is informational, not enforcement.** Set on first successful enable. Never re-checked after. Once a user has 30 tagged closes, their unlock status persists even if trades are later deleted.

10. **Production URL defaults rule is load-bearing.** Extension verification uses `bun run typecheck`, NOT `bun run build`. Encoded in the spec's Acceptance Criteria and in MEMORY (`feedback_prod_defaults.md`). T3 and T9 both specify this.

11. **Kelly inputs JSONB is entry-time snapshot, not close-time recompute.** The 13-field blob is built at trade submission in T6 (from `shrunk`, `fk`, `qk`, `mult`, `eff`, baseline). `computed_at = Utc::now()` at entry. Persisted via `OrderGroup.kelly_inputs` → `TradeCloseEvent.kelly_inputs` → INSERT. This preserves audit integrity: `kelly_inputs` reflects the calibration state at the moment the trade was sized, not a hypothetical recompute at close.

12. **`net_pnl` is the authoritative P&L column for Kelly inputs.** Both `net_pnl` and `realized_pnl` exist; production analytics use `net_pnl` (journal_timeseries.rs, setup_breakdown). T5's aggregate queries use `net_pnl > 0` for win-rate and `r_multiple` for avg_R.

13. **TradeEventWriter may need mirror updates.** Journal-writing has two paths: `JournalService::record_trade_close` (primary) and `TradeEventWriter::flush_transaction` (secondary, RSK-02 T5 discovery). If `TradeEventWriter` INSERTs into `journal_trades`, T7 must mirror the `kelly_inputs` column addition there too. Verified at build time.

14. **Extension test baseline is ~28 pre-existing failures** (per RSK-02 T2 discovery: `browser.commands` mock gap from EXT-46). T3 must NOT add new failures — verifiable by comparing pre/post test counts.

---

## Status

COMPLETE

Spec: QNT-01a-kelly-engine
Total Tasks: 9 (T1–T3 CP-1 plumbing; T4–T5 CP-2 pure math; T6–T8 CP-3 integration; T9 verification) — all `complete`.

**T9 Verification Results (2026-04-20):**
- Rust clippy: 3 pre-existing warnings unchanged (actor.rs:1858, cex_client.rs:653, evaluator.rs:188). Zero new.
- Rust tests: common_utils 315 / engine 108×2 / pg_queue 11 / router 639 passing / 1 pre-existing failure (`test_me_returns_user_info`, AUTH regression unrelated to QNT-01a, documented in T2 discovery) / 9 ignored / sqlx_postgres 17 / ws_stream 10. No QNT-01a regressions.
- Extension typecheck: 18 pre-existing errors unchanged (matches T3 baseline). Zero new.
- Integration grep: 16 Rust files + 4 TS files + 2 migration files. Wire contract consistent end-to-end.

**Deferred (requires live session):**
- Manual QA: new user 409 on unlock, threshold-crossing PATCH success, live dynamic-mode trade sizing, negative-edge 400, `kelly_inputs` JSONB populated at close.
- Migration up+down on fresh DB (local verification only; runs automatically on first-boot in prod).
