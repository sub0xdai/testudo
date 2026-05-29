# CLN-03-component-cleanup — Implementation Plan

## Current State Summary

Four components have code quality anti-patterns identified in the nuclear review:

1. **TradeDetail.tsx (551 lines):** `createResource` abused as `createEffect` at lines 99-100 — fetches detail/draft data, then calls `syncNotes()`/`syncDraftNotes()` in the fetcher, returns `null`. This is a React-ism leaking into Solid.

2. **Account.tsx (249 lines):** 10 signals + 3 createResources = 13 reactive declarations. Four action handlers (`handleTest`, `handleDelete`, `handleRevoke`, `handleImport`) share identical 6-line boilerplate (`setXId(id)` → try/catch → `setXId(null)` in finally). `error` signal is set but never displayed.

3. **Overview.tsx (428 lines):** Lines 58-88 have two near-identical 12-line `Object.defineProperty` blocks wrapping `batch.sections.overview` and `batch.sections.equity_curve` into `CachedResource`-compatible accessors.

4. **IdentitySettings.tsx (291 lines):** 9 TS6133 errors (declared-but-never-read). Analysis shows: `claimError`, `releasing`, `releaseError`, `visError` have setters wired to JSX but getters never consumed → destructure with `[, setter]`. `bioSaving`, `bioError`, `handleInputHint`, `startBioEdit`, `saveBio` + cascading `bioEdit` are wholly dead → remove. FR-5 (unused `IdentitySettings` import in Account.tsx) already fixed in CLN-01 CP-4.

## Checkpoints

### CP-1: Fix createResource abuse + dead code + tsc zero errors ✅
- Completed 2026-05-29 by /skill:vox build. 0 TS errors. IdentitySettings.tsx: 291→256 lines. TradeDetail.tsx: createResource→createEffect.
- **Touches**: `src/components/trades/TradeDetail.tsx`, `src/components/account/IdentitySettings.tsx`
- **Tasks**:
  1. `TradeDetail.tsx`: Replace `createResource(() => detail(), () => { syncNotes(); return null })` with `createEffect(() => { detail(); syncNotes() })` (line 99)
  2. `TradeDetail.tsx`: Same for `createResource(() => draftData(), () => { syncDraftNotes(); return null })` (line 100)
  3. `IdentitySettings.tsx`: Remove dead functions `handleInputHint`, `startBioEdit`, `saveBio`; remove dead signals `bioEdit`, `bioSaving`, `bioError`
  4. `IdentitySettings.tsx`: Destructure unused getters: `claimError` → `[, setClaimError]`, `releasing` → `[, setReleasing]`, `releaseError` → `[, setReleaseError]`, `visError` → `[, setVisError]`
- **Verification**: `npx tsc --noEmit 2>&1 | grep -c TS6133` prints `0`
- **Verification**: `bun run build` exits 0
- **Commit message**: `fix: replace createResource abuse with createEffect, remove IdentitySettings dead code`

### CP-2: Collapse Account.tsx signal soup into useAsyncAction
- **Touches**: `src/pages/Account.tsx`, `src/lib/useAsyncAction.ts` (new)
- **Tasks**:
  1. Create `src/lib/useAsyncAction.ts` with `useAsyncAction()` hook: `{ pending, error, setError, run }`
  2. `Account.tsx`: Replace `testingId`, `deletingId`, `revokingId`, `importingId`, `error` signals (5 → 1 hook call)
  3. `Account.tsx`: Refactor `handleTest`, `handleDelete`, `handleRevoke`, `handleImport` to use `action.run(id, ...)`
  4. `Account.tsx`: Replace `Show`/`Suspense` guards that check individual `XId()` with `action.pending()` check
- **Verification**: `bun run build` exits 0; Account page renders without errors
- **Verification**: `grep -c createSignal src/pages/Account.tsx` prints ≤ 7 (was 11)
- **Commit message**: `refactor: collapse Account.tsx signal soup into useAsyncAction hook`

### CP-3: Extract wrapBatchSection helper + final gates
- **Touches**: `src/components/Overview.tsx`
- **Tasks**:
  1. Extract `wrapBatchSection<T>(section, refetchAll)` function in Overview.tsx
  2. Replace two 12-line `defineProperty` blocks with `wrapBatchSection` calls
- **Verification**: `bun run build` exits 0
- **Verification**: `npx tsc --noEmit` exits 0 with zero errors
- **Commit message**: `refactor: extract wrapBatchSection helper from Overview.tsx defineProperty duplication`

---

## Risks

1. **`useAsyncAction` concurrent actions** — a single `pending` signal means only one action at a time. Mitigation: this matches current UX (you don't test and delete simultaneously). The hook can be extended later with a `Map<string, boolean>`.
2. **`wrapBatchSection` reactivity** — `Object.defineProperty` getters must re-read each time. Mitigation: extracted helper uses identical pattern to inlined code. Zero behavioral change.
3. **`bioEdit` cascade removal** — removing `bioEdit` means the bio edit form JSX (if any) would break. Mitigation: no JSX references `bioEdit`, `startBioEdit`, or `saveBio` — confirmed by TS6133 silence on `bioEdit`.

## Notes

- FR-5 (remove unused `IdentitySettings` import from `Account.tsx`) is **already complete** from CLN-01 CP-4. Skip.
- FR-6/FR-7 (tsc zero errors + build) are verification gates, not tasks.
