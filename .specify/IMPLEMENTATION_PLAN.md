# Implementation Plan

> Last updated: 2026-04-20
> Current spec: QNT-01b-kelly-transparency
> Phase: PLANNING COMPLETE — ready for BUILD

---

## Active Spec: QNT-01b-kelly-transparency

### Gap Analysis

**Backend (`testudo-exchange/crates/router/`):**

1. **`GET /api/v1/user/settings`** — `routes/user_settings.rs:127-147`, shipped in QNT-01a T2. Already returns:
   ```rust
   pub struct UserSettingsResponse {
       pub settings: UserSettings,          // { dynamic_risk_enabled, dynamic_risk_unlocked_at }
       pub unlocked: bool,                  // is_unlocked(count >= 30)
       pub tagged_trade_count: i64,
   }
   ```
   **Spec FR-1** asks for a NEW `/api/v1/user/qnt-readiness` returning `{ tagged_closed, unlock_at: 30, unlocked: bool }`. That is a **strict subset** of what `/user/settings` already exposes. Discovery #1.

2. **Kelly pre-sizing in `create_trade`** — `routes/trade_management.rs:699-841`. QNT-01a T6 integrated this inline. Structure:
   - L709-735: load `user_settings.dynamic_risk_enabled`
   - L741-768: `CalibrationEngine::load_prior()` + `load_setup()`
   - L770-784: `shrink()` + `full_kelly()` + `edge_multiplier()` + `effective_risk_percent()`
   - L786-797: negative-edge 400 rejection (error_code `"negative_edge"`)
   - L817-832: `kelly_inputs_json` construction (13-field blob)
   - L834-840: untagged-fallback branch (silent, `info!` log)
   Spec FR-4 demands that `/trades/preview` and `/trades` produce **byte-identical** sizing output (spec Risk #1). The only way to guarantee this is to extract this block into a single async helper that both handlers call. Discovery #2.

3. **`SizingPreview` type** — grep confirms no existing type. New domain struct needed. Discovery #3.

4. **`AppState.calibration_engine: Arc<CalibrationEngine>`** — `types/app.rs:48`. Already wired (QNT-01a T6).

5. **`CreateTradeRequest`** — `routes/trade_management.rs:294-316`. Carries all fields the preview needs (`setup_tag`, `management.risk_percent`, `entry_price`, `stop_loss_price`, `symbol`, `side`, `quantity`). For preview, only `setup_tag` + `management.risk_percent` are actually consumed — the rest is payload noise that the preview endpoint will ignore. Reuse the type verbatim (avoid a `CreateTradePreviewRequest` DTO duplicate) — simpler contract, identical body shape.

6. **Route registration** — `routes/mod.rs` + `main.rs`. `/user` and `/trades` scopes already exist. Only need `.route("/preview", web::post().to(preview_trade_sizing))` inside `/trades` scope.

**Extension (`testudo-extension/src/`):**

7. **`SettingsPanel.tsx:88-95`** (QNT-01a T3) — current locked-state copy:
   ```jsx
   <Show when={unlocked() || enabled()}
     fallback={<>Unlocks after 30 tagged closes (currently {taggedCount()}).</>}>
     Scales sizing by your calibrated per-setup edge (Quarter-Kelly, ±2× clamp).
   </Show>
   ```
   **Spec FR-2** wants *"Dynamic Risk unlocks after 30 tagged closes (N/30)"* — just a copy tweak. **Spec FR-3** wants the `dynamic_risk_unlocked_at` date rendered in a caption when unlocked. `settings.dynamic_risk_unlocked_at` flows through from the backend already (QNT-01a T2) but SettingsPanel doesn't display it. Discovery #4.

8. **`TradeForm.tsx`** — `components/TradeForm.tsx:1-503`:
   - `createMemo` for sizing math at L55-77
   - two-step confirm state machine at L479-497 (Arm → Confirm)
   - `buildSetup()` + `handleConfirm()` at L118-138 construct the payload
   **No preview fetching logic exists.** FR-5/6/7/8 preview row needs to live above the confirm button.
   
   The confirm button's existing disabled logic is a derived memo; adding a `previewNegativeEdge()` gate is a 2-line addition. Discovery #5.

9. **`modal.tsx:39-206`** — Shadow DOM CSS. Theme variables `--color-signal-red`, `--color-text-dim`, `--color-text-secondary`, `--color-signal-green` all present. Existing `.balance-row` / `.balance-label` / `.balance-value` classes usable. Need new `.kelly-preview-row` class (single line, badge-style `⚡` leading, color variants).

10. **`schemas.ts`** — has `TradePayloadSchema` (L49-72), `UserSettingsResponseSchema` (L218-222 — QNT-01a T3). Need new `SizingPreviewSchema` discriminated union per spec §Response Shape. Runtime message schema L233-295 needs `PREVIEW_TRADE_SIZING` variant.

11. **`background/api.ts:157-200`** — has `apiRequest()`, `executeTrade()`, `listSetupTags()`, `getUserSettings()`, `patchUserSettings()` (QNT-01a). Need `previewTradeSizing(payload)` wrapper.

12. **`background/handlers.ts`** — 28+ message handlers + dispatch table. Need `handlePreviewTradeSizing(msg)`.

13. **Debounce utility — MISSING.** Spec §Paved Roads claims "the 250 ms balance-refresh debounce [is] exported" from `utils.ts`. Grep confirms **no debounce utility exists** in the extension. `calculateRefreshDelay()` in utils.ts is for JWT refresh timing, not debouncing. Discovery #6. Either (a) reach for lodash-debounce (new dep, rejected), (b) write a 10-line debounce helper in `utils.ts`, or (c) inline a `setTimeout` + cleanup pattern inside `TradeForm.tsx`. **Choosing (b)** — it's 10 lines and likely to be reused (FIX series may want it for rate-limit handling).

14. **Production URL defaults rule** (MEMORY `feedback_prod_defaults.md`) — extension verification MUST use `bun run typecheck`, NOT `bun run build`. Spec Acceptance Criteria codifies this. Preserved in T9.

---

### Design Decisions

1. **Skip `/user/qnt-readiness`; reuse `/user/settings`.** Spec FR-1 is redundant — the existing endpoint exposes the full superset. A thin alias endpoint would duplicate logic for zero benefit; worse, it introduces a 2nd source of truth for unlock state that can drift. **Extension calls `/user/settings` for popup unlock state (already does via `getUserSettings()`).** Document as a spec deviation in T1. If future QNT work ever needs a truly slimmer readiness endpoint (e.g., pre-auth smoke check), revisit then.

2. **Shared compute: extract `compute_sizing_preview(user_id, setup_tag, baseline_risk_pct, dynamic_enabled, pool, calibration_engine) -> Result<SizingPreview, Error>`.** Lives in a new `router/src/services/sizing_preview.rs` (or extension of `calibration.rs`). `create_trade` calls it; `preview_trade_sizing` calls it. **Byte-parity guaranteed by construction.** Result is the discriminated union; both handlers interpret it — `create_trade` maps `NegativeEdge` to 400 + `Calibrated.effective_risk_pct` to `RiskConfig` override; preview endpoint returns it verbatim.

3. **`SizingPreview` discriminated union — exact shape per spec:**
   ```rust
   #[derive(Serialize)]
   #[serde(tag = "kind", rename_all = "snake_case")]
   pub enum SizingReasoning {
       Calibrated { n_setup: u32, p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal },
       Untagged,
       NegativeEdge { quarter_kelly: Decimal },
       FixedMode,  // dynamic_risk_enabled == false
   }

   #[derive(Serialize)]
   pub struct SizingPreview {
       pub baseline_risk_pct: Decimal,
       pub effective_risk_pct: Decimal,
       pub edge_multiplier: Decimal,
       pub reasoning: SizingReasoning,
   }
   ```
   For `NegativeEdge`: `effective_risk_pct = 0`, `edge_multiplier = 0`. For `Untagged`/`FixedMode`: `effective_risk_pct = baseline`, `edge_multiplier = 1.0`. The frontend renders copy based solely on `reasoning.kind` — no re-derivation needed.

4. **Preview endpoint has NO side effects.** No DB writes. No CCXT calls. No shadow engine. Only: user_settings read + up to 2 aggregate queries + pure math. Latency budget < 50ms p99 per spec §Preview Endpoint Contract.

5. **Preview endpoint auth = same JWT middleware as `/trades`.** `AuthenticatedUser` extractor. No new surface.

6. **Preview request body shape = `CreateTradeRequest`** (reuse the same struct). Accepts extra fields silently; only consumes `setup_tag` + `management`. Keeps the extension's `executeTrade` and `previewTradeSizing` wire contracts identical — trivial for `TradeForm` to call both with the same payload.

7. **Debounce lives in `utils.ts`, not inside `TradeForm.tsx`.** Module-level function `debounce<T extends (...args: any[]) => void>(fn: T, ms: number): T`. Reusable. Tested inline.

8. **Preview failure is non-fatal — FR-10 "confirm remains enabled (falls through to baseline)".** Preview fetch is purely for UX guidance. The trade execution path is unchanged; if the preview fails for any reason, the user's Alt+X confirm still fires and the backend does its own calibration authoritatively (so any server-side calibration drift surfaces as a backend-side 400, not as a silent sizing divergence).

9. **Preview row does NOT render when dynamic_risk is off.** Spec FR-5 is explicit: "when Dynamic Risk is on and the backend returned a sizing_preview". The extension reads `settings.dynamic_risk_enabled` from the cached user settings (already fetched by popup). If off → skip preview fetch entirely, don't render the row. Avoids unnecessary API traffic for fixed-mode users.

10. **`ArmConfirm` disabled-on-negative-edge is a separate gate from the existing disable logic.** The current two-step confirm button already has multiple disabled states (no stop loss, no target, etc.). Add `previewNegativeEdge()` to the derived disabled memo. On `reasoning.kind === "negative_edge"`, confirm stays disabled even if the user taps through the Arm step.

11. **Copy variants match spec verbatim.** No paraphrasing:
    - Calibrated: `Risk: {baseline}% → {effective}% ({n_setup} trades, {round(p_eff*100)}% WR, {avg_r_win}R avg)`
    - Untagged: `Tag this setup to unlock calibrated sizing for it.`
    - NegativeEdge: `Calibration shows negative edge for this setup — size = 0.`
    - FixedMode: not rendered (falls through to FR-5 "Dynamic Risk on" gate).
    - Failure: `Preview unavailable`
    Frontend pattern-matches on `reasoning.kind`, no i18n layer.

12. **150ms debounce per FR-9** (not the spec's paved-roads 250ms — 150 is the FR, it takes precedence over the paved-roads note).

---

### Vertical Checkpoint Structure (from spec §Technical Implementation)

| CP | Goal | Tasks |
|----|------|-------|
| CP-1 | Locked-state polish on existing `/user/settings` data. Popup shows N/30 progress + unlock date. | T1 |
| CP-2 | Preview endpoint live + happy path inline row. Tagged + positive edge → `"Risk: 1.0% → 1.4% (…)"` | T2, T3, T4, T5 |
| CP-3 | Edge cases: untagged, negative-edge, preview-failure, debounce. | T6, T7 |
| Final | Verification + archival. | T8 |

---

### Parallel Track Detection

```
T1 (SettingsPanel unlock-date + N/30 copy)                    [CP-1, standalone]
    │
T2 (extract compute_sizing_preview + refactor create_trade)   [CP-2]
    │
T3 (POST /trades/preview route)                               [CP-2, depends on T2]
    │
    ├── T4 (extension schemas + message + api + handler)      ─┐ [CP-2, parallel with T5]
    │                                                          │
    └── T5 (TradeForm preview row — calibrated happy path)    ─┤
                                                               │
                                                               ↓
                             T6 (debounce util + edge variants: untagged + negative-edge)  [CP-3]
                                                               │
                                                               ↓
                                               T7 (failure path + "Preview unavailable")  [CP-3]
                                                               │
                                                               ↓
                                                          T8 (verification + commit)
```

T1 independent of everything; can land first. T4 and T5 independent after T3 lands. Single-agent BUILD stays sequential.

---

## Tasks

### T1: SettingsPanel — N/30 progress copy + unlock date caption — `complete`

**Scope:** CP-1. Pure frontend polish. No backend changes.

**Files:**
- `testudo-extension/src/popup/components/SettingsPanel.tsx` — MODIFIED at L88-95:
  - Locked state copy → `"Dynamic Risk unlocks after 30 tagged closes ({taggedCount()}/30)"` per FR-2.
  - Unlocked state adds a small caption line: if `settings.dynamic_risk_unlocked_at` is present, render a muted `text-text-dim text-xs` line `"Unlocked {date}"` (localized, e.g., `"Unlocked 2026-05-04"`). FR-3.
  - Existing toggle state machine (enabled/unlocked gating, optimistic PATCH, 409 error handling) untouched.

**Validate:**
- `cd testudo-extension && bun run typecheck` exit 0.
- Manual visual check deferred to T8.

**Acceptance:**
- User at 0–29 tagged closes sees `"Dynamic Risk unlocks after 30 tagged closes (N/30)"`, toggle disabled.
- User at 30+ tagged closes sees toggle enabled + caption `"Unlocked <date>"` (if `dynamic_risk_unlocked_at` is non-null).
- No new typecheck errors beyond the 18-error baseline.

---

### T2: Extract `compute_sizing_preview()` — reusable calibration pipeline — `complete`

**Scope:** CP-2 load-bearing refactor. Pulls QNT-01a T6's inline Kelly block out of `create_trade` into a shared helper. Byte-for-byte preserves existing sizing semantics (FR-10 regression boundary).

**Files:**
- `testudo-exchange/crates/router/src/services/sizing_preview.rs` — NEW:
  ```rust
  pub struct SizingPreview {
      pub baseline_risk_pct: Decimal,
      pub effective_risk_pct: Decimal,
      pub edge_multiplier: Decimal,
      pub reasoning: SizingReasoning,
      // Internal: the kelly_inputs JSON blob for DB persistence. Only populated on Calibrated.
      pub kelly_inputs: Option<serde_json::Value>,
  }

  #[serde(tag = "kind", rename_all = "snake_case")]
  pub enum SizingReasoning {
      Calibrated { n_setup: u32, p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal },
      Untagged,
      NegativeEdge { quarter_kelly: Decimal },
      FixedMode,
  }

  pub async fn compute_sizing_preview(
      user_id: Uuid,
      setup_tag: Option<&str>,
      baseline_risk_pct: Decimal,
      dynamic_enabled: bool,
      calibration_engine: &CalibrationEngine,
  ) -> Result<SizingPreview, sqlx::Error>;
  ```
  Branch structure inside the fn (mirrors current `create_trade` L709-841 exactly):
  - `!dynamic_enabled` → `FixedMode`, effective = baseline, mult = 1.0.
  - `dynamic_enabled && setup_tag.is_none()` → `Untagged`, effective = baseline, mult = 1.0. Emit existing `info!` log.
  - `dynamic_enabled && setup_tag.is_some()`:
    - Load prior + setup.
    - Shrink → `shrunk`.
    - Compute `full_kelly`.
    - If `full_kelly <= 0` → `NegativeEdge { quarter_kelly }`, effective = 0, mult = 0, kelly_inputs = None.
    - Else: compute `quarter_kelly`, `edge_multiplier`, `effective_risk_percent`. Build `kelly_inputs` JSON. Return `Calibrated { … }`.

- `testudo-exchange/crates/router/src/services/mod.rs` — MODIFIED: `pub mod sizing_preview;`.

- `testudo-exchange/crates/router/src/routes/trade_management.rs` — MODIFIED in `create_trade`:
  - Replace L709-841 with a single call: `let preview = compute_sizing_preview(user_id, setup_tag.as_deref(), baseline, dynamic_enabled, &state.calibration_engine).await?;`
  - Match on `preview.reasoning`:
    - `NegativeEdge { .. }` → return 400 with `{ error: "negative_edge", message: "…" }` (same copy as today).
    - `Calibrated { .. }` → use `preview.effective_risk_pct` for RiskConfig override + `preview.kelly_inputs` for DB.
    - `Untagged` / `FixedMode` → proceed with baseline, `kelly_inputs = None`.
  - All other logic below L841 unchanged.

**Inline tests (in sizing_preview.rs):**
- `fixed_mode_returns_baseline_unchanged` — dynamic_enabled=false → mult=1, effective=baseline, reasoning=FixedMode.
- `untagged_returns_baseline_unchanged` — dynamic_enabled=true, setup_tag=None → mult=1, effective=baseline, reasoning=Untagged.
- `negative_edge_returns_zero` — fixture with p=0.3, b=1.0 (negative full_kelly) → effective=0, mult=0, reasoning=NegativeEdge.
- `calibrated_within_clamp_bounds` — fixture with high edge → effective ∈ [0.25×baseline, 2×baseline].
- (Keep it pure by fixturing `CalibrationEngine` via a test helper OR by extracting the decision logic further into a pure `classify()` fn taking `ShrunkStats` + baseline + dynamic_enabled. Prefer the latter for testability.)

**Validate:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test`.
- **Critical regression check:** all 639 existing router bin tests must still pass (pre-existing AUTH failure `test_me_returns_user_info` aside). Fixed-mode and dynamic-mode trades must size identically to pre-T2 output on matching inputs.

**Acceptance:**
- `compute_sizing_preview` is the single source of Kelly decisions.
- `create_trade` shrinks from ~130 lines to ~15 lines for the Kelly section.
- Test suite baseline preserved.
- No new clippy warnings beyond the 3 pre-existing.

---

### T3: `POST /api/v1/trades/preview` route — `complete`

**Scope:** CP-2 new endpoint. Thin HTTP adapter around `compute_sizing_preview`.

**Files:**
- `testudo-exchange/crates/router/src/routes/trade_management.rs` — MODIFIED:
  - New handler `preview_trade_sizing(user: AuthenticatedUser, req: web::Json<CreateTradeRequest>, state: web::Data<AppState>) -> HttpResponse`.
  - Read `user_settings.dynamic_risk_enabled` (same query as `create_trade`).
  - Call `compute_sizing_preview`.
  - Return 200 with `SizingPreview` (strip `kelly_inputs` from the serialized response — that's internal-only for DB persistence; the preview response has only `baseline_risk_pct`, `effective_risk_pct`, `edge_multiplier`, `reasoning`). Use a `SizingPreviewResponse` DTO that omits `kelly_inputs`, or mark the field `#[serde(skip_serializing)]` on `SizingPreview`.
- `testudo-exchange/crates/router/src/main.rs` (or wherever `/trades` scope is registered) — MODIFIED: `.route("/preview", web::post().to(preview_trade_sizing))`.

**Inline test (in trade_management.rs):**
- `preview_matches_create_trade_byte_parity` — construct a fixture payload; call `compute_sizing_preview` twice through the two code paths (create_trade and preview) and assert identical `(baseline_risk_pct, effective_risk_pct, edge_multiplier, reasoning)` tuples. Satisfies spec Risk #1.

**Validate:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`.

**Acceptance:**
- `POST /api/v1/trades/preview` returns 200 + `SizingPreview` JSON.
- No DB writes (verify by side-effect absence).
- Byte parity with `create_trade` computed values.
- Latency p99 < 50ms on a seeded fixture (best-effort — measured live in T8).

---

### T4: Extension — SizingPreview schema + message + API helper + handler — `complete`

**Scope:** CP-2 extension wire contract.

**Files:**
- `testudo-extension/src/schemas.ts` — MODIFIED:
  ```typescript
  const CalibratedReasoning = z.object({
      kind: z.literal("calibrated"),
      n_setup: z.number().int(),
      p_eff: z.number(),
      avg_r_win: z.number(),
      avg_r_loss: z.number(),
  });
  const UntaggedReasoning = z.object({ kind: z.literal("untagged") });
  const NegativeEdgeReasoning = z.object({
      kind: z.literal("negative_edge"),
      quarter_kelly: z.number(),
  });
  const FixedModeReasoning = z.object({ kind: z.literal("fixed_mode") });

  export const SizingPreviewSchema = z.object({
      baseline_risk_pct: z.number(),
      effective_risk_pct: z.number(),
      edge_multiplier: z.number(),
      reasoning: z.discriminatedUnion("kind", [
          CalibratedReasoning, UntaggedReasoning, NegativeEdgeReasoning, FixedModeReasoning,
      ]),
  });
  export type SizingPreview = z.infer<typeof SizingPreviewSchema>;
  ```
  Add `RuntimeMessageSchema` variant:
  ```typescript
  z.object({
      type: z.literal("PREVIEW_TRADE_SIZING"),
      payload: TradePayloadSchema,  // reuse existing schema — same shape as executeTrade
  }),
  ```

- `testudo-extension/src/background/api.ts` — MODIFIED:
  - `export async function previewTradeSizing(payload: TradePayload): Promise<ApiResult> { … }` — POST `/api/v1/trades/preview`, parse response with `SizingPreviewSchema`. On network error or non-2xx, return `{ ok: false, error: string }` (don't throw — FR-10 preview failure is non-fatal).

- `testudo-extension/src/background/handlers.ts` — MODIFIED:
  - `handlePreviewTradeSizing(msg)` → `previewTradeSizing(msg.payload)`.
  - Register in dispatch table.

**Validate:**
- `cd testudo-extension && bun run typecheck`.
- Pre-existing test baseline preserved (~28 failures from `browser.commands` mock gap — no new failures).

**Acceptance:**
- SizingPreviewSchema round-trips all 4 variants against fixture JSON.
- Message dispatch wired.
- No new typecheck errors.

---

### T5: TradeForm — preview row (calibrated happy path) — `complete`

**Scope:** CP-2 UX surface. Render the preview row for the happy-path `calibrated` variant only. Edge cases land in T6.

**Files:**
- `testudo-extension/src/components/TradeForm.tsx` — MODIFIED:
  - Add signal `const [preview, setPreview] = createSignal<SizingPreview | null>(null);`.
  - Add `createEffect` that fetches preview via `browser.runtime.sendMessage({ type: "PREVIEW_TRADE_SIZING", payload: buildSetup() })` when the relevant payload fields change.
    - For T5, trigger on mount only (no debounce yet — T6 adds debounce).
    - Guard: only fetch if `settings.dynamic_risk_enabled === true` (pull from popup-cached settings OR fetch `getUserSettings()` once on form mount). If off → never render, never fetch.
  - Render preview row above confirm button (new JSX block). For T5, render only when `preview()?.reasoning.kind === "calibrated"`:
    - Copy: ``Risk: {baseline.toFixed(1)}% → {effective.toFixed(1)}% ({n_setup} trades, {Math.round(p_eff * 100)}% WR, {avg_r_win.toFixed(1)}R avg)``
    - Style: muted text (`color: var(--color-text-secondary)`), single line, leading `⚡` badge.

- `testudo-extension/src/modal.tsx` — MODIFIED: add `.kelly-preview-row` CSS block to the Shadow DOM `<style>`:
  ```css
  .kelly-preview-row {
      display: flex; align-items: center; gap: 6px;
      padding: 6px 10px; margin: 8px 0;
      font-size: 12px; color: var(--color-text-secondary);
      border-left: 2px solid var(--color-accent-steel);
  }
  .kelly-preview-row.negative { color: var(--color-signal-red); border-left-color: var(--color-signal-red); }
  .kelly-preview-row.muted { font-style: italic; color: var(--color-text-dim); }
  ```

**Validate:**
- `cd testudo-extension && bun run typecheck`.

**Acceptance:**
- Dynamic-mode user on a tagged setup with positive edge sees the calibrated row within 300ms of modal open.
- Dynamic-mode OFF users see no preview row (no API call made either).
- Non-calibrated variants still return null in this task (edge cases coming in T6).
- Confirm button behavior unchanged (still fires on Enter-Enter).

---

### T6: Debounce utility + untagged + negative-edge variants — `pending`

**Scope:** CP-3. Adds the 150ms debounce plus the remaining copy variants plus the confirm-disable gate for negative-edge.

**Files:**
- `testudo-extension/src/utils.ts` — MODIFIED:
  ```typescript
  export function debounce<T extends (...args: any[]) => void>(fn: T, ms: number): T & { cancel: () => void } {
      let handle: ReturnType<typeof setTimeout> | null = null;
      const wrapped = ((...args: any[]) => {
          if (handle) clearTimeout(handle);
          handle = setTimeout(() => { handle = null; fn(...args); }, ms);
      }) as T & { cancel: () => void };
      wrapped.cancel = () => { if (handle) { clearTimeout(handle); handle = null; } };
      return wrapped;
  }
  ```
- `testudo-extension/src/components/TradeForm.tsx` — MODIFIED:
  - Wrap preview fetch in `debounce(…, 150)`. Trigger debounced fetch on changes to: `entry_price`, `stop_loss_price`, `target_price`, `setup_tag`, `management.risk_percent`. Cancel on unmount via `onCleanup`.
  - Drop in-flight preview if a new one supersedes it (use an async sequence counter: increment on each call, only set preview if the response's counter is still current).
  - Extend preview rendering:
    - `reasoning.kind === "untagged"` → row with copy `"Tag this setup to unlock calibrated sizing for it."`, class `muted` (italic, dim).
    - `reasoning.kind === "negative_edge"` → row with copy `"Calibration shows negative edge for this setup — size = 0."`, class `negative` (red).
    - `reasoning.kind === "fixed_mode"` → don't render (shouldn't happen when dynamic_risk is on, but harmless fallthrough).
  - Confirm button disabled memo gains one more gate: `preview()?.reasoning.kind === "negative_edge"`. Applied to both Arm and Confirm states.

**Validate:**
- `cd testudo-extension && bun run typecheck`.
- Add a unit test for `debounce` in `utils.test.ts` (if the file exists in the project — check at build time; if not, inline the test next to the function using vitest's tree convention).

**Acceptance:**
- Rapid slider drag triggers at most 1 preview per 150ms.
- In-flight preview superseded by a newer request does not clobber the state.
- Untagged setup → italic muted row.
- Negative-edge setup → red row + confirm button disabled even past the Arm step.

---

### T7: Preview failure path + verification polish — `pending`

**Scope:** CP-3. Implement FR-10 "Preview unavailable" + any lingering polish items.

**Files:**
- `testudo-extension/src/components/TradeForm.tsx` — MODIFIED:
  - Add `const [previewError, setPreviewError] = createSignal<boolean>(false);`
  - On failed preview response (non-2xx, network error, schema parse error), set `previewError(true)` and set `preview(null)`.
  - Render a separate row when `previewError() === true`: copy `"Preview unavailable"`, class `muted`, grey.
  - Confirm button remains ENABLED on preview error (FR-10).
  - On next successful preview, clear the error flag.

**Validate:**
- `cd testudo-extension && bun run typecheck`.

**Acceptance:**
- Network disconnect → `"Preview unavailable"` shown, confirm still works.
- Server returns 500 → same.
- Next successful fetch clears the error.

---

### T8: Final verification + spec archival — `pending`

**Scope:** Completion Protocol per constitution.

**Verifications:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test` — green, zero new warnings beyond pre-existing 3.
- `cd testudo-extension && bun run typecheck` — exit 0, pre-existing 18-error baseline preserved. (Do NOT `bun run build` per `feedback_prod_defaults.md`.)
- Integration grep across repo: `compute_sizing_preview | SizingPreview | PREVIEW_TRADE_SIZING | previewTradeSizing | SizingReasoning` wired consistently. Expected: ~8 Rust files, ~5 TS files.
- **Parity assertion** (spec Risk #1): integration test in T3 already covers this. Confirm green.
- **Default-off safety**: dynamic_risk=false user submitting `/trades/preview` returns `{ reasoning: { kind: "fixed_mode" }, effective_risk_pct = baseline }` without any calibration DB queries. Verify via unit test with a spy CalibrationEngine (if testability permits) OR by inspecting SQL logs in T8 manual QA.
- **Latency budget**: best-effort measurement on a seeded fixture user (≥ 100 tagged closes) via `time curl …`. p99 < 50ms target.

**Manual QA (deferred to live session):**
- User at 0–29 tagged closes → popup toggle disabled + `"Dynamic Risk unlocks after 30 tagged closes (N/30)"`.
- User at 30+ → popup toggle enabled + `"Unlocked <date>"` caption.
- Dynamic-mode on + tagged setup + positive edge → inline row `"Risk: 1.0% → 1.4% (43 trades, 58% WR, 1.9R avg)"` within 300ms.
- Dynamic-mode on + untagged setup → `"Tag this setup to unlock calibrated sizing for it."`, confirm still works.
- Dynamic-mode on + negative-edge setup → `"Calibration shows negative edge for this setup — size = 0."` red, confirm disabled.
- Dynamic-mode OFF → no preview row rendered, no API call made.
- Network disconnect during modal → `"Preview unavailable"`, confirm works.
- Executed trade's `journal_trades.kelly_inputs` matches the preview's displayed numbers (end-to-end parity).

**Commit plan (one per task):**
- T1: `feat(qnt-01b): SettingsPanel — N/30 progress + unlock date caption`
- T2: `refactor(qnt-01b): extract compute_sizing_preview for preview/trade parity`
- T3: `feat(qnt-01b): POST /api/v1/trades/preview endpoint`
- T4: `feat(qnt-01b): extension — SizingPreview schema + message + API helper`
- T5: `feat(qnt-01b): TradeForm — calibrated preview row (happy path)`
- T6: `feat(qnt-01b): TradeForm — debounce + untagged + negative-edge variants`
- T7: `feat(qnt-01b): TradeForm — preview-unavailable fallback`
- T8: umbrella (no source): `feat(qnt-01b): pre-submit Kelly transparency + unlock UX`

**Archive:** Move `.specify/specs/QNT-01b-kelly-transparency/` → `.specify/spec-archive/QNT-01b-kelly-transparency/` after T8.

---

## Discoveries

### 2026-04-20 — QNT-01b planning

1. **Spec's `GET /api/v1/user/qnt-readiness` endpoint is redundant.** QNT-01a T2 already shipped `GET /api/v1/user/settings` returning `{ settings, unlocked, tagged_trade_count }` — a strict superset of `/qnt-readiness`'s proposed `{ tagged_closed, unlock_at: 30, unlocked }`. Planning deviates: skip the new endpoint, reuse `/user/settings` in the popup. Documented as a spec deviation in T1. `unlock_at: 30` is a client-side constant if needed (it's already hardcoded as `30` in the server's `is_unlocked(count) -> bool` check).

2. **`compute_sizing_preview()` extraction is non-negotiable for FR-10 byte-parity.** QNT-01a T6 left Kelly pre-sizing inline in `create_trade` (L709-841). Spec Risk #1 demands preview/execution identical output. The only mechanism that enforces this is shared code — both handlers call the same async fn. T2 is therefore a pure refactor: create_trade behavior must be byte-identical pre/post-refactor, verified by the full 639-test router suite.

3. **No `SizingPreview` type exists.** Clean-slate type design. Discriminated union with 4 variants. `kelly_inputs` JSON blob (DB persistence) is internal-only — NOT serialized in the preview endpoint response (use `#[serde(skip_serializing)]` or a wrapper DTO).

4. **SettingsPanel has basic locked copy but no unlock date.** QNT-01a T3 shipped `"Unlocks after 30 tagged closes (currently N)"`. Spec FR-2 asks for a slight rewording (`"(N/30)"` format) and FR-3 adds the `dynamic_risk_unlocked_at` caption. `dynamic_risk_unlocked_at` already flows through from the backend via `UserSettingsSchema` — frontend just needs to render it. Trivial.

5. **TradeForm has no preview logic at all.** Needs a new signal + createEffect + debounced fetch + preview row JSX + CSS. Single-file change (+modal.tsx for CSS), no architectural shifts. Confirm button's disabled memo is already a derived memo; adding `previewNegativeEdge()` to it is a 2-line addition.

6. **Debounce utility MISSING in extension.** Spec §Paved Roads claim about a `250ms balance-refresh debounce` is **incorrect** — grep confirms no such helper. T6 adds a 10-line `debounce()` to `utils.ts`. Spec FR-9 is the authoritative 150ms (not the paved-roads 250ms).

7. **Preview request body = `CreateTradeRequest` (reused).** Same shape as execute. Extra fields the preview doesn't need (quantity, symbol, side) are silently ignored by the handler. Keeps extension's `buildSetup()` reusable for both execute and preview with zero duplication.

8. **In-flight preview supersession via sequence counter.** Spec Risk #3 (preview spam). `let seq = 0;` at module scope inside TradeForm; each fetch increments it; response callback only sets state if the incoming seq matches the current value. Cheaper than cancellation tokens and idiomatic Solid.

9. **Dynamic-off users never hit the preview endpoint.** Spec FR-5 gate: preview only when `settings.dynamic_risk_enabled === true`. Avoids unnecessary /trades/preview calls for ~100% of pre-unlock users. Popup-cached settings (already fetched by SettingsPanel) flow into TradeForm via a message or shared storage read.

10. **Copy strings must match spec verbatim.** All four copy variants are in the spec §FR-6/7/8. Frontend should pattern-match on `reasoning.kind` and emit exact strings — no templating beyond numeric interpolation. Prevents "advice-sounding" drift (spec Risk #4).

---

## Status

BUILD IN PROGRESS

Spec: QNT-01b-kelly-transparency
Progress: T1 + T2 + T3 + T4 + T5 complete; T6 onwards pending.

Next task: T6 — debounce utility + untagged + negative-edge variants.
