# Implementation Plan

> Last updated: 2026-04-20
> Current spec: QNT-01c-kelly-journal-audit
> Phase: PLANNING COMPLETE — ready for BUILD

---

## Active Spec: QNT-01c-kelly-journal-audit

### Gap Analysis

**Backend (`testudo-exchange/crates/router/`):**

1. **`JournalTrade` struct already carries `kelly_inputs`** — `crates/router/src/models/journal.rs:23-54`, field added at `line 51` (`pub kelly_inputs: Option<serde_json::Value>`) by QNT-01a T7. DB column `journal_trades.kelly_inputs JSONB NULL` landed in migration `20260420000100_add_qnt_columns`. Discovery #1.

2. **All read-path SQL already flows `kelly_inputs` end-to-end:**
   - `routes/journal.rs:273` — list endpoint: `SELECT jt.* FROM journal_trades jt WHERE ...` (SELECT *) → sqlx auto-binds into `JournalTrade`, `kelly_inputs` flows unchanged.
   - `routes/journal.rs:362-364` — detail endpoint: `SELECT * FROM journal_trades WHERE id = $1` → same.
   - `services/journal_service.rs:160-196` — idempotency + insert paths use explicit columns that already include `kelly_inputs` (QNT-01a T7).
   - `services/trade_event_writer.rs:366-401` — INSERT explicit-columns path includes `kelly_inputs` at `$25` (QNT-01a T7).

3. **`PaginatedTrades.trades: Vec<TradeWithTags>`** — `TradeWithTags` wraps `JournalTrade` inline (no field filtering), serde flattens all `JournalTrade` fields onto the wire. `kelly_inputs` ships on every trade row of every response today. **Spec FR-2 is already satisfied at the backend level — zero backend code changes required.** Discovery #2.

4. **Verification paved road:** The 655-router-test baseline (post-QNT-01b) should hold unchanged through this spec. No new Rust code means no new tests, no new clippy warnings.

**Journal frontend (`testudo-journal/src/`):**

5. **`JournalTrade` TS type MISSING `kelly_inputs`** — `api/client.ts:192-218`. All fields present except `kelly_inputs`, `setup_tag` present (RSK-02). Direct extension of the interface is trivial. No fetcher signature change needed — `fetchTrades()` / `fetchTradeDetail()` already return `JournalTrade`-shaped rows from the backend.

6. **`TradeRow.tsx` renders inline badges today** — `components/trades/TradeRow.tsx` (97 lines). Structure:
   - Row is `tabindex=0`, fires parent `onClick`
   - Notes-icon pencil (L60-66) and tag badges (L67-92) already compose inline at the right side of the row
   - Existing pattern for "secondary affordance + parent row click still works": badges sit inside the row but DON'T stopPropagation — parent click still fires
   
   For QNT-01c we need a ⚡ badge that (a) signals kelly data is present, and (b) when clicked opens a separate kelly-only modal. That click MUST stopPropagation (otherwise the trade-detail side panel also opens). Discovery #3.

7. **No Modal primitive exists** — `components/trades/TradeDetail.tsx` (500+ lines) is a fixed-position right-slide **side panel**, not a centered modal. Uses `createFocusTrap` from `lib/createFocusTrap.ts` and `CLOSE_ANIMATION_MS` from `lib/tokens.ts`. Reusing these primitives for a new centered modal costs ~60 lines of new component code. Discovery #4.

8. **`help-content.ts`** — `lib/help-content.ts` is a flat `Record<string, string>`. Namespaces by dotted prefix (e.g. `page.*`, `coach.patterns.*`, `chart.*`, `risk.*`). Adding `kelly.*` entries is purely additive, no structural change. `HelpTip` component at `components/HelpTip.tsx` takes `text: string` and returns null when empty — missing keys render as nothing, so the help entries can land in any task without breaking consumers.

9. **No Zod on the journal fetch boundary** — Journal uses plain TS interfaces + `fetch(...).json()` without runtime validation (contrast with `testudo-extension/src/schemas.ts` which is Zod-heavy). Spec Risk #1 calls for a minimal Zod schema on the parse boundary to surface shape drift gracefully. Two options: (a) pull zod into the journal (new dep, ~40kb), (b) write a ~30-line plain-TS runtime type guard (same pattern used by `testudo-extension/src/scraper.ts` post-bundle-optimization). **Choosing (b)** — keeps the journal bundle free of zod, mirrors the scraper.ts precedent, sufficient for a single type. Discovery #5.

10. **Existing side-panel pattern opens on row click via `selectedTradeId` signal** — `pages/Trades.tsx`. Row click → parent sets `selectedTradeId` → `<TradeDetail>` renders. The kelly modal is an **independent overlay**, not a replacement for TradeDetail. If a row has `kelly_inputs != null` and the user clicks the ⚡ badge, only the kelly modal opens (not the side panel). If the user clicks anywhere ELSE on the row, the side panel opens as today (no kelly display inside — kelly is exclusively on the modal surface). Discovery #6.

---

### Design Decisions

1. **Skip backend changes entirely.** Spec §Files lists `services/journal_service.rs` as MODIFIED, but grep + read confirms `kelly_inputs` already ships on every trade-list and trade-detail response (Discovery #2). Document as spec deviation in T1. Verification still hits `cargo clippy --all-targets && cargo test` to preserve the 655-test router baseline untouched.

2. **Build a real centered modal (not reuse the side panel).** Spec FR-4 is explicit ("opens a detail modal"). The journal's established side-panel pattern is right for "here's everything about a trade"; a kelly audit is a focused, single-purpose overlay. A new `KellyInputsModal` component (centered overlay, backdrop, ESC-close, focus trap) costs ~60 lines — cheaper than retrofitting the side panel to carry a second viewing mode.

3. **Runtime validation via plain-TS type guard, not Zod.** `safeParseKellyInputs(raw: unknown): KellyInputs | null` lives inline in `api/client.ts`. Validates the 13 fields the spec pins (mode, baseline_risk_pct, effective_risk_pct, edge_multiplier, p_eff, avg_r_win, avg_r_loss, quarter_kelly, n_setup, n_global, pseudocount_k, p_setup_raw, p_global_raw, computed_at). Parse failure → returns null → UI renders "calibration data unavailable" fallback (spec Risk #1). Zero dep footprint.

4. **Badge click MUST stopPropagation.** Otherwise parent row's click handler fires and opens the side panel underneath the kelly modal. One-line guard (`e.stopPropagation()` in the badge's onClick).

5. **Narrative summary line: pure function `buildKellyNarrative(kelly: KellyInputs): string`.** Five-case selector per spec §Narrative Summary Logic. Lives in `lib/` (new file `lib/kelly-narrative.ts`) so it can be unit-tested without mounting the modal. Input is `KellyInputs`; output is a single string. Renders as the modal's top-of-fold headline.

6. **Modal field table layout mirrors existing data grids.** TradeDetail's P&L grid pattern (`grid grid-cols-2 gap-x-4 gap-y-2`) is the aesthetic reference. No new primitives.

7. **No backend migration, no backend route changes, no backend tests.** This is a pure journal frontend spec. The spec's §Files backend section is incorrect relative to ground truth — document the deviation explicitly.

8. **`kelly.*` HELP namespace lands in T5 (final CP-3 task).** T2-T4 can reference `HELP['kelly.badge']` etc. safely — empty string return is a no-op. Separating the help content into its own task keeps T2-T4 focused on component wiring + modal structure, and T5 bundles all the copy in one diff for easy review.

9. **Snapshot-test the narrative function in T4** per spec acceptance criterion "snapshot-tested with fixture `KellyInputs` values." Plain vitest `toBe(...)` assertions against five fixtures (up, up-clamped, baseline, down, down-clamped). No SolidJS mounting needed — pure TS function, simplest possible test.

10. **Badge copy exactly `⚡ Kelly-sized`** per spec FR-3. No paraphrase. Small pill, same size as the existing tag badges (`.badge` pattern in TradeRow.tsx).

---

### Vertical Checkpoint Structure (from spec §Technical Implementation)

| CP | Goal | Tasks |
|----|------|-------|
| CP-1 | Frontend type extension + row badge; backend confirmed already passing kelly_inputs through. | T1, T2 |
| CP-2 | KellyInputsModal raw field dump + Zod-like parse guard. | T3, T4a |
| CP-3 | Narrative summary line + HELP entries for `kelly.*`. | T4b, T5 |
| Final | Verification + archival. | T6 |

**Rationale for the split**: The spec's CP-2 is "modal opens + raw fields"; I split it so the parse guard (risk #1 mitigation) lands alongside the modal (T3 = modal shell, T4a = parse guard). Then T4b = narrative, T5 = help. Keeps tasks atomic and easy to revert if any one of them regresses.

---

### Parallel Track Detection

```
T1 (JournalTrade TS type + KellyInputs type)              [CP-1, foundational]
    │
T2 (TradeRow ⚡ badge — opens nothing yet, visual only)   [CP-1, depends on T1]
    │
T3 (KellyInputsModal component + wire badge click)        [CP-2]
    │
T4a (safeParseKellyInputs type guard + fallback UI)       [CP-2]
    │
T4b (buildKellyNarrative + snapshot tests)                [CP-3]
    │
T5 (HELP kelly.* entries + HelpTips in modal)             [CP-3]
    │
T6 (final verification + spec archival)
```

All tasks sequential — this spec is shallow enough that parallelization won't save meaningful wall time. Single-agent BUILD.

---

## Tasks

### T1: JournalTrade TS type + KellyInputs type — `pending`

**Scope:** CP-1. Pure type-level extension of `api/client.ts`. No runtime behavior change. No visual change yet.

**Files:**
- `testudo-journal/src/api/client.ts` — MODIFIED:
  - Add exported type:
    ```typescript
    export type KellyInputs = {
      mode: 'calibrated_kelly'
      baseline_risk_pct: number
      effective_risk_pct: number
      edge_multiplier: number
      p_eff: number
      avg_r_win: number
      avg_r_loss: number
      quarter_kelly: number
      n_setup: number
      n_global: number
      pseudocount_k: number
      p_setup_raw: number
      p_global_raw: number
      computed_at: string  // ISO 8601
    }
    ```
  - Extend `JournalTrade` interface at `line 192` with:
    ```typescript
    kelly_inputs: KellyInputs | null
    ```
    Placed near the existing `setup_tag: string | null` field for grouping.

**Validate:**
- `cd testudo-journal && bun run build` exit 0. (No `typecheck` script exists in the journal; build IS the type gate per the journal's `package.json`.)

**Acceptance:**
- `KellyInputs` type exported and matches the QNT-01a T7 JSONB blob shape byte-for-byte.
- `JournalTrade.kelly_inputs: KellyInputs | null` compiles without touching any consumer (unused field in T1).
- No new build errors.

---

### T2: TradeRow ⚡ Kelly-sized badge — `pending`

**Scope:** CP-1. Visual indicator on the row when `kelly_inputs != null`. Click handler is a placeholder that logs; T3 wires the modal.

**Files:**
- `testudo-journal/src/components/trades/TradeRow.tsx` — MODIFIED:
  - Inside the badges-container region (near the notes icon + tag badges, L60-92), conditionally render:
    ```tsx
    <Show when={props.trade.kelly_inputs != null}>
      <button
        type="button"
        class="kelly-badge"
        title="Kelly-sized"
        onClick={(e) => {
          e.stopPropagation()
          // T3 will wire the modal open here
          console.debug('Kelly badge clicked', props.trade.id)
        }}
      >
        ⚡ Kelly-sized
      </button>
    </Show>
    ```
  - Style via Tailwind utility classes (match tag-badge grammar — small, pill-shaped, muted bg): `inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded border border-container-border/50 text-text-dim hover:text-text-primary`. Exact class list subject to alignment with existing `.tag-badge` visual weight during build-verify.

**Validate:**
- `cd testudo-journal && bun run build` exit 0.

**Acceptance:**
- Rows with `trade.kelly_inputs != null` render ⚡ Kelly-sized badge.
- Rows with `trade.kelly_inputs == null` render no badge.
- Clicking the badge does NOT trigger the row-click (side panel does not open).
- Badge has `title="Kelly-sized"` so hover is self-documenting pre-T5.

---

### T3: KellyInputsModal component shell + wire badge click — `pending`

**Scope:** CP-2. New centered-overlay modal. Raw field dump of all 13 KellyInputs fields in a two-column grid. No narrative yet (T4b), no help tooltips yet (T5).

**Files:**
- `testudo-journal/src/components/trades/KellyInputsModal.tsx` — NEW:
  - Props: `{ kelly: KellyInputs; onClose: () => void }`
  - Structure: fixed inset-0 backdrop (`bg-black/60 z-40 animate-fade-in`) + centered panel (`fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 max-w-md w-full animate-fade-in`).
  - Use `createFocusTrap` (`lib/createFocusTrap.ts`) on the panel ref for keyboard focus containment.
  - ESC key handler closes via `onClose`.
  - Click-outside (backdrop click) closes.
  - Close animation: set local `closing` signal, `setTimeout(props.onClose, CLOSE_ANIMATION_MS)`.
  - Body: raw two-column grid with the 13 fields formatted per type (percentages for `baseline_risk_pct`/`effective_risk_pct`/`p_eff`/`p_setup_raw`/`p_global_raw`, R-values for `avg_r_win`/`avg_r_loss`, multiplier for `edge_multiplier`, integer counts for `n_setup`/`n_global`/`pseudocount_k`, decimal for `quarter_kelly`, formatted datetime for `computed_at`).
  - Header copy: `KELLY AUDIT` (small, mono, all-caps) + close X button.

- `testudo-journal/src/components/trades/TradeRow.tsx` — MODIFIED:
  - Replace the `console.debug` placeholder with: `props.onKellyClick?.(props.trade.kelly_inputs!)` (new optional prop on TradeRowProps).
  - Add prop `onKellyClick?: (kelly: KellyInputs) => void` to the interface.

- `testudo-journal/src/pages/Trades.tsx` (or wherever TradeRow is consumed — confirm at build time) — MODIFIED:
  - Add local signal `const [kellyModalInputs, setKellyModalInputs] = createSignal<KellyInputs | null>(null)`.
  - Pass `onKellyClick={setKellyModalInputs}` to each `<TradeRow>`.
  - Conditionally render `<KellyInputsModal kelly={kellyModalInputs()!} onClose={() => setKellyModalInputs(null)} />` when signal is non-null.

**Validate:**
- `cd testudo-journal && bun run build` exit 0.
- Manual test deferred to T6: open a fixture trade with `kelly_inputs != null`, click ⚡ badge, confirm modal opens, ESC closes, click-backdrop closes, click-row-elsewhere opens side panel independently.

**Acceptance:**
- Clicking ⚡ badge on a Kelly-sized row opens the modal.
- Modal shows all 13 fields in a readable grid.
- ESC, backdrop-click, close-X all dismiss the modal.
- Side panel and modal are independent overlays (clicking a row while modal is open does NOT open side panel — modal's stopPropagation wins on the badge; row body still opens side panel as before when clicked outside the badge).
- Focus trap active while modal is open.

---

### T4a: `safeParseKellyInputs` runtime type guard + fallback UI — `pending`

**Scope:** CP-2 defensive layer. Guard against QNT-01a JSONB shape drift (spec Risk #1).

**Files:**
- `testudo-journal/src/api/client.ts` — MODIFIED:
  - Add exported function:
    ```typescript
    export function safeParseKellyInputs(raw: unknown): KellyInputs | null {
      if (raw == null || typeof raw !== 'object') return null
      const r = raw as Record<string, unknown>
      const numField = (k: string) => typeof r[k] === 'number' ? (r[k] as number) : null
      const strField = (k: string) => typeof r[k] === 'string' ? (r[k] as string) : null
      // Validate all 13 fields; return null if any required field is missing/wrong type
      // ...
    }
    ```
  - Field-by-field checks: `mode === 'calibrated_kelly'`, all numeric fields present and finite, `computed_at` parseable as ISO date.
  - ~40 lines total.

- `testudo-journal/src/components/trades/TradeRow.tsx` — MODIFIED:
  - Badge click site now does `const parsed = safeParseKellyInputs(props.trade.kelly_inputs); if (parsed) props.onKellyClick?.(parsed)`. If parse fails, no modal opens.
  - Alternative: always pass through, validate inside the modal. Chose badge-site validation because if kelly_inputs is malformed, the badge itself shouldn't appear either — guard at the same gate.
  - Gate the `<Show when={...}>` for the badge on a helper memo `kellyValid = createMemo(() => safeParseKellyInputs(props.trade.kelly_inputs))`. Badge renders iff `kellyValid() != null`.

- `testudo-journal/src/components/trades/KellyInputsModal.tsx` — No change needed (already receives a validated `KellyInputs`).

**Validate:**
- `cd testudo-journal && bun run build` exit 0.
- Inline unit test for `safeParseKellyInputs` at `api/client.test.ts` (NEW file, or co-located if test convention exists) OR next to the narrative test in T4b — consolidate both test files if simpler. Cases: valid full blob, missing field returns null, wrong mode returns null, null input returns null, non-object input returns null.

**Acceptance:**
- Malformed `kelly_inputs` in a trade row produces NO badge (graceful degradation).
- Valid `kelly_inputs` produces a badge that opens a modal with correctly-typed fields.
- Parse-failure case covered by unit tests.

---

### T4b: Narrative summary line — `buildKellyNarrative` + snapshot tests — `pending`

**Scope:** CP-3 FR-6. Pure function + five snapshot assertions. Rendered at top of modal.

**Files:**
- `testudo-journal/src/lib/kelly-narrative.ts` — NEW:
  ```typescript
  import type { KellyInputs } from '../api/client'

  export function buildKellyNarrative(kelly: KellyInputs): string {
    const m = kelly.edge_multiplier
    const mFmt = m.toFixed(1) + 'x'
    if (m > 1.05 && m < 2.0) {
      return `Sized up ${mFmt} because this setup's ${kelly.n_setup}-trade history beats your ${kelly.n_global}-trade baseline.`
    }
    if (m >= 2.0) {
      return `Sized up 2.0x (ceiling hit) — this setup's edge is strong enough that the clamp engaged.`
    }
    if (m < 0.95 && m > 0.25) {
      return `Sized down ${mFmt} because this setup's ${kelly.n_setup}-trade history trails your baseline.`
    }
    if (m <= 0.25) {
      return `Sized down 0.25x (floor hit) — calibration is weak for this setup.`
    }
    return `Sized at baseline — calibration is neutral for this setup.`
  }
  ```

- `testudo-journal/src/lib/kelly-narrative.test.ts` — NEW:
  - 5 snapshot tests covering each of the 5 branches.
  - Plus 2 boundary tests: `edge_multiplier = 1.05` (neutral side, should read "baseline"), `edge_multiplier = 2.0` (clamp-hit copy).

- `testudo-journal/src/components/trades/KellyInputsModal.tsx` — MODIFIED:
  - Import `buildKellyNarrative`.
  - Render narrative at top of modal body, above the raw field table. Styled as a prominent-but-calm headline: `text-sm text-text-primary leading-relaxed mb-4`.

**Validate:**
- `cd testudo-journal && bun run build` exit 0.
- `cd testudo-journal && bun run test kelly-narrative` — all 7 tests pass (5 branch + 2 boundary).

**Acceptance:**
- Modal shows the narrative headline above the raw grid.
- All 5 copy variants read exactly as spec §Narrative Summary Logic.
- Boundary cases (`m=1.05` and `m=2.0`) hit the correct branch per spec acceptance criterion.

---

### T5: HELP `kelly.*` entries + HelpTips in modal — `pending`

**Scope:** CP-3 FR-7 final polish. Pure additive content + `<HelpTip>` mounts.

**Files:**
- `testudo-journal/src/lib/help-content.ts` — MODIFIED:
  - Add entries (copy subject to in-task review, here's the skeleton):
    ```typescript
    'kelly.badge': 'Shown on trades sized by calibrated Kelly. Click to see the inputs that produced the sizing decision.',
    'kelly.edge_multiplier': 'Ratio of your effective risk to your baseline risk for this setup. 1.0x means Kelly agreed with your baseline; >1 means sized up, <1 means sized down. Clamped to [0.25x, 2.0x].',
    'kelly.n_setup': 'How many past trades of this setup fed the calibration. More trades = more confidence in the per-setup stats.',
    'kelly.n_global': 'How many past trades across all setups fed the baseline prior. The prior anchors sizing when per-setup data is thin.',
    'kelly.p_eff': 'Effective win-rate used by Kelly. A shrinkage blend of this setup\'s raw win rate and your global win rate; tilts toward global when n_setup is low.',
    'kelly.quarter_kelly': 'Kelly\'s own fraction, divided by 4. Quarter-Kelly is the conservative default that trades off growth rate against drawdown depth.',
    'kelly.computed_at': 'When the calibration was computed for this trade. Calibration inputs are frozen at trade entry, not re-derived retrospectively.',
    ```
  - ~7-9 entries.

- `testudo-journal/src/components/trades/KellyInputsModal.tsx` — MODIFIED:
  - Import `HelpTip` + `HELP`.
  - Attach `<HelpTip text={HELP['kelly.edge_multiplier']} />` next to the edge-multiplier field label, similarly for `n_setup`, `n_global`, `p_eff`, `quarter_kelly`, `computed_at`. 

- `testudo-journal/src/components/trades/TradeRow.tsx` — MODIFIED:
  - Badge `title` attribute → `HELP['kelly.badge']` (falls back to `'Kelly-sized'` if empty — but T5 ensures it's populated).

**Validate:**
- `cd testudo-journal && bun run build` exit 0.

**Acceptance:**
- All `kelly.*` HELP keys defined in the spec FR-7 acceptance list (`kelly.badge`, `kelly.edge_multiplier`, `kelly.n_setup`, `kelly.n_global`, `kelly.p_eff`) return non-empty strings.
- HelpTips render next to at least the 5 fields named in the acceptance criterion.
- Badge's hover `title` shows the kelly.badge copy.

---

### T6: Final verification + spec archival — `pending`

**Scope:** Completion Protocol per constitution.

**Verifications:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test` — must be byte-identical to the pre-T1 baseline. 655 router bin passing / 1 pre-existing failure / 9 ignored. Zero new clippy warnings beyond the 3 pre-existing (actor.rs, cex_client.rs, evaluator.rs). Proves spec deviation "no backend changes" held in practice.
- `cd testudo-journal && bun run build` — exit 0.
- `cd testudo-journal && bun run test` — narrative + parse-guard tests green. Existing test suite unchanged.
- Integration grep: `kelly_inputs | KellyInputs | KellyInputsModal | buildKellyNarrative | safeParseKellyInputs` — expected ~6 TS files (api/client.ts, lib/kelly-narrative.ts, lib/help-content.ts, components/trades/TradeRow.tsx, components/trades/KellyInputsModal.tsx, pages/Trades.tsx or equivalent).
- No Rust files added/changed — confirm via `git diff --stat testudo-exchange/` is empty (or contains only spec-archive move).

**Manual QA (deferred to live session):**
- Two real historical trades observed in the journal: one Kelly-sized (badge + modal works), one not (no badge). Spec §Completion Signal #3.
- Fire all five narrative variants by surfacing trades with differing `edge_multiplier` values (seeded from live data if available, or construct fixtures on staging).
- HELP tooltip hover on badge + each modal field returns copy.
- Filter by `setup_tag` in the journal and confirm Kelly+fixed rows interleave naturally (spec Acceptance).

**Commit plan (one per task):**
- T1: `feat(qnt-01c): journal TS types for KellyInputs`
- T2: `feat(qnt-01c): TradeRow ⚡ Kelly-sized badge`
- T3: `feat(qnt-01c): KellyInputsModal shell + wire badge click`
- T4a: `feat(qnt-01c): safeParseKellyInputs guard + graceful fallback`
- T4b: `feat(qnt-01c): narrative summary line + snapshot tests`
- T5: `feat(qnt-01c): HELP kelly.* entries + modal tooltips`
- T6: umbrella (no source): `feat(qnt-01c): journal Kelly audit — row badge + detail modal`

**Archive:** Move `.specify/specs/QNT-01c-kelly-journal-audit/` → `.specify/spec-archive/QNT-01c-kelly-journal-audit/` after T6.

---

## Discoveries

### 2026-04-20 — QNT-01c planning

1. **`JournalTrade` struct already has `kelly_inputs`** — `models/journal.rs:51` added by QNT-01a T7. Migration `20260420000100_add_qnt_columns` shipped the DB column. No backend struct changes needed.

2. **Backend read paths already flow `kelly_inputs` end-to-end.** `routes/journal.rs:273` uses `SELECT jt.*`; `routes/journal.rs:362` uses `SELECT *`. `JournalTrade` auto-derives `FromRow` + `Serialize`, so `kelly_inputs: Option<serde_json::Value>` ships on the wire in every trade list and detail response today. **Spec's FR-2 is already satisfied.** The spec's §Files "Backend (Rust) — one touch only" is incorrect relative to ground truth. Documenting as T1 deviation.

3. **TradeRow has existing badge grammar** — `components/trades/TradeRow.tsx:67-92`. Tag badges and notes icon already compose inline without stopPropagation. The ⚡ badge needs explicit `e.stopPropagation()` because it's the first badge whose click does something other than open the side panel.

4. **No centered-Modal primitive exists in the journal.** `TradeDetail.tsx` is a right-slide side panel. Reusing its `createFocusTrap` + `CLOSE_ANIMATION_MS` primitives for a new centered modal costs ~60 lines — cheaper than dual-purposing the side panel.

5. **No Zod in the journal.** Plain-TS type guard (`safeParseKellyInputs`) mirrors the `testudo-extension/src/scraper.ts` post-bundle-optimization pattern. Keeps bundle size unchanged.

6. **Click-surface independence is load-bearing.** Kelly modal must NOT replace the side panel — both are valid overlays. The badge's click stopPropagation is the one-line mechanism that preserves this. Missing that guard would break the existing row-opens-side-panel UX.

7. **HelpTip gracefully handles missing keys** — returns null when text is empty. T2-T4 can reference `HELP['kelly.*']` keys safely before T5 populates them; no build break, no visual break (hover tooltip just doesn't appear). This is why T5 lands last without blocking T2-T4.

8. **`bun run test` availability** — Journal has vitest (`bun run test` works); `kelly-narrative.test.ts` + optional `safeParseKellyInputs` test go through vitest. Not adding any new testing infrastructure — reusing what exists.

9. **Spec FR-6 narrative copy is descriptive, not prescriptive** — reviewed explicitly in T5 prior to commit to ensure "sized up 1.4×" reads as report, not endorsement. Past-tense framing + multiplier-as-fact keeps spec Risk #3 in check.

10. **`kelly_inputs` not persisted in `managed_positions`** — QNT-01a T7 noted that a router restart between trade entry and close drops kelly_inputs because `managed_positions` has no column for it. That means some recent trades may carry `kelly_inputs = null` even when the user DID have dynamic risk enabled. The journal correctly shows no badge for those trades — behavior is "true to what was persisted," which is the right default. No mitigation needed for QNT-01c; a future QNT spec could add the column + rehydration path if needed.

---

## Status

PLANNING COMPLETE

Spec: QNT-01c-kelly-journal-audit
Total Tasks: 7 (T1, T2, T3, T4a, T4b, T5, T6)
Ready for BUILD mode.

Next task: T1 — JournalTrade TS type + KellyInputs type
