# Specification: Component Cleanup — Four Anti-Pattern Fixes

**Spec ID:** CLN-03-component-cleanup
**Date:** 2026-05-29
**Status:** Draft
**Class:** Infrastructure / Refactor
**Priority:** P1 — four distinct code quality regressions across four components; each independently makes the codebase harder to maintain
**Depends on:** CLN-01, CLN-02 (no hard dependency, but cleanup order is: client → auth → components)
**Series:** CLN-01 through CLN-03 (Journal Frontend Cleanup) — nuclear review findings on `testudo-journal/`

---

## Problem Statement

A thermo-nuclear code quality review identified four distinct anti-patterns in four components:

1. **`TradeDetail.tsx`** — `createResource` abused as `createEffect` (lines 99–100). Uses Solid's async data-fetching primitive purely as a reactive side-effect watcher with a null-return fetcher. This is a React-ism leaking into Solid. Rule 4: "Prefer direct, boring, maintainable code over hacky or magical code."

2. **`Account.tsx`** — 11-signal antipattern. Each action (test, delete, revoke, import) creates 2 signals (`XId` + shared `error`), and each handler is the same 6-line boilerplate. This is signal soup — rule 2: "Be highly suspicious of new ad-hoc conditionals, scattered special cases." A single `useAsyncAction` or a state machine would collapse 8 signals into 1.

3. **`Overview.tsx`** — duplicate `Object.defineProperty` facades (lines 58–88). Two near-identical blocks wrap batch section results into `CachedResource`-compatible accessors. This is copy-paste that should be a single helper. Rule 6: "copy-pasted logic instead of extracted helpers."

4. **`IdentitySettings.tsx`** — 9 declared-but-never-read signals/functions + 1 unused import in `Account.tsx`. TypeScript reports 10 `TS6133` errors. Dead code from a previous refactor. Rule 3: "Bias toward cleaning the design, not just accepting working code."

All four are mechanical refactors. No behavior changes. Together they remove ~60 lines of dead/boilerplate code and fix 10 TypeScript errors.

---

## User Stories

- **As a journal developer**, I want to read a component's reactive logic without puzzling over misused primitives.
- **As a journal developer**, I want to add a new action to the Account page without copy-pasting 6 lines of boilerplate.
- **As a journal developer**, I want `npx tsc --noEmit` to pass with zero errors.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace `createResource` watchers in `TradeDetail.tsx` with `createEffect`. Lines 99–100. | High | `TradeDetail.tsx` |
| FR-2 | Collapse Account.tsx 11-signal soup into a `useAsyncActions` pattern or state machine. | High | `Account.tsx` |
| FR-3 | Extract `wrapBatchSection<T>()` helper for `Overview.tsx` `defineProperty` facades. | Medium | `Overview.tsx` |
| FR-4 | Remove all dead code from `IdentitySettings.tsx`: `claimError`, `releasing`, `releaseError`, `bioSaving`, `bioError`, `visError`, `handleInputHint`, `startBioEdit`, `saveBio`. | High | `IdentitySettings.tsx` |
| FR-5 | Remove unused `IdentitySettings` import from `Account.tsx`. | High | `Account.tsx` |
| FR-6 | `npx tsc --noEmit` exits 0 with zero errors. | High | `testudo-journal/` |
| FR-7 | `bun run build` exits 0. | High | `testudo-journal/` |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | FR-1 (`createResource` → `createEffect`), FR-4 (dead code removal), FR-5 (unused import) | `npx tsc --noEmit` errors reduced from 10 → 0; `bun run build` passes |
| CP-2 | FR-2 (Account.tsx signal soup → `useAsyncActions`) | Account page still renders; test/delete/revoke/import actions still work |
| CP-3 | FR-3 (Overview.tsx `wrapBatchSection` helper), FR-6, FR-7 (final gates) | `bun run build && npx tsc --noEmit` both exit 0 |

### FR-1: `createResource` → `createEffect`

Current code (`TradeDetail.tsx:99-100`):

```typescript
createResource(() => detail(), () => { syncNotes(); return null })
createResource(() => draftData(), () => { syncDraftNotes(); return null })
```

Replace with:

```typescript
createEffect(() => { detail(); syncNotes() })
createEffect(() => { draftData(); syncDraftNotes() })
```

The `detail()` and `draftData()` calls inside `createEffect` register reactive dependencies. When the resource resolves, the effect re-runs and calls `syncNotes()`. The behavior is identical — `syncNotes` already guards against `notesDirty()`.

### FR-2: Account.tsx Signal Soup → `useAsyncActions`

Current state (11 signals):

```typescript
const [testResults, setTestResults] = createSignal<Record<string, TestConnectionResult>>({})
const [testingId, setTestingId] = createSignal<string | null>(null)
const [deletingId, setDeletingId] = createSignal<string | null>(null)
const [revokingId, setRevokingId] = createSignal<string | null>(null)
const [importingId, setImportingId] = createSignal<string | null>(null)
const [showForm, setShowForm] = createSignal(false)
const [formInitialExchange, setFormInitialExchange] = createSignal('')
const [reauthAccountId, setReauthAccountId] = createSignal<string | null>(null)
const [setupComplete, setSetupComplete] = createSignal(false)
const [error, setError] = createSignal('')
```

Proposed: extract a `useAsyncActions` helper and a `useModal` helper.

**`useAsyncAction`** — generic wrapper for `setPending(true)` → try/catch → `setPending(false)`:

```typescript
// src/lib/useAsyncAction.ts
import { createSignal } from 'solid-js'

export function useAsyncAction() {
  const [pending, setPending] = createSignal<string | null>(null)
  const [error, setError] = createSignal('')

  async function run(id: string, action: () => Promise<void>, onError?: string) {
    setPending(id)
    setError('')
    try {
      await action()
    } catch {
      setError(onError ?? 'Action failed')
    } finally {
      setPending(null)
    }
  }

  return { pending, error, setError, run }
}
```

Usage in `Account.tsx`:

```typescript
const action = useAsyncAction()

async function handleTest(id: string) {
  await action.run(id, async () => {
    const result = await exchangeApi.testConnection(id)
    // testResults still needs its own signal since it accumulates data
    setTestResults(prev => ({ ...prev, [id]: result }))
  }, 'Connection failed')
}

async function handleDelete(id: string) {
  await action.run(id, async () => {
    await exchangeApi.deleteAccount(id)
    refetchAccounts()
  }, 'Failed to delete account')
}
```

This eliminates `testingId`, `deletingId`, `revokingId`, `importingId` signals plus the shared `error` signal — 5 signals collapsed into 1 hook call. The `action.pending()` signal replaces all individual ID trackers.

**`useModal`** — generic open/close:

```typescript
function useModal() {
  const [isOpen, setIsOpen] = createSignal(false)
  const [data, setData] = createSignal<string>('') // modal-specific data
  return { isOpen, data, open: (d?: string) => { setData(d ?? ''); setIsOpen(true) }, close: () => setIsOpen(false) }
}
```

Replaces `showForm` + `formInitialExchange` (2 signals → 1 hook). The `reauthAccountId` can also use the same pattern or remain as a standalone signal since it's conceptually different (it's an overlay, not a modal).

Net reduction: 11 signals → ~5 signals + 2 hooks. The component becomes scannable.

### FR-3: `wrapBatchSection` Helper

Current code in `Overview.tsx` (duplicated pattern):

```typescript
const statsAccessor = (() => batch.sections.overview() as OverviewResponse | undefined) as {
  (): OverviewResponse | undefined
  readonly loading: boolean
  readonly error: unknown
  refetch: () => void
}
Object.defineProperty(statsAccessor, 'loading', {
  get: () => batch.sections.overview.loading, enumerable: true,
})
Object.defineProperty(statsAccessor, 'error', {
  get: () => batch.sections.overview.error, enumerable: true,
})
Object.defineProperty(statsAccessor, 'refetch', {
  value: () => batch.refetch(), enumerable: true, writable: false,
})
```

Extract:

```typescript
function wrapBatchSection<T>(
  section: CachedResource<unknown>,
  refetchAll: () => void,
): CachedResource<T> {
  const accessor = (() => section() as T | undefined) as CachedResource<T>
  Object.defineProperty(accessor, 'loading', {
    get: () => section.loading, enumerable: true,
  })
  Object.defineProperty(accessor, 'error', {
    get: () => section.error, enumerable: true,
  })
  Object.defineProperty(accessor, 'isStale', {
    get: () => section.isStale, enumerable: true,
  })
  Object.defineProperty(accessor, 'refetch', {
    value: refetchAll, enumerable: true, writable: false,
  })
  return accessor
}
```

Usage:

```typescript
const stats = wrapBatchSection<OverviewResponse>(batch.sections.overview, batch.refetch)
const equity = wrapBatchSection<{ data: EquityPoint[] }>(batch.sections.equity_curve, batch.refetch)
```

Two 12-line blocks collapse into 2 one-liner calls. The helper lives in `Overview.tsx` since it's the only consumer (for now). If other pages use `useCachedBatch`, it can be promoted to `cache.ts`.

### FR-4: Dead Code Removal

In `IdentitySettings.tsx`, remove:
- Signals: `claimError`, `releasing`, `releaseError`, `bioSaving`, `bioError`, `visError`
- Functions: `handleInputHint`, `startBioEdit`, `saveBio`

These were created in a prior feature pass and never wired into JSX. Their removal has zero behavioral impact — they're never read.

### FR-5: Unused Import

In `Account.tsx`, remove:
```typescript
import { IdentitySettings } from '../components/account/IdentitySettings'
```

### Files

- `src/components/trades/TradeDetail.tsx` — FR-1: `createResource` → `createEffect`
- `src/pages/Account.tsx` — FR-2: signal soup → `useAsyncAction` + `useModal`; FR-5: remove unused import
- `src/components/Overview.tsx` — FR-3: extract `wrapBatchSection`
- `src/components/account/IdentitySettings.tsx` — FR-4: dead code removal
- `src/lib/useAsyncAction.ts` — new (optional; could also live inline in Account.tsx for now)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `TradeDetail.tsx` uses `createEffect` instead of `createResource` for note syncing
- [ ] `Account.tsx` signal count reduced from 11 to ≤ 6
- [ ] `Overview.tsx` uses `wrapBatchSection` helper instead of duplicate `defineProperty` blocks
- [ ] `IdentitySettings.tsx` has zero declared-but-never-read variables
- [ ] `Account.tsx` has zero unused imports
- [ ] `npx tsc --noEmit` exits 0 with zero errors
- [ ] `bun run build` exits 0

---

## Risks

1. **`useAsyncAction` timing** — switching from individual `XId` signals to a shared `pending` signal means only one action can be "in flight" at a time. Mitigation: this is already the UX expectation (you don't test and delete simultaneously). If concurrent actions are needed later, the hook can be extended with a `Map<string, boolean>` internally.
2. **`wrapBatchSection` reactivity** — the `Object.defineProperty` pattern is sensitive to how Solid tracks getters. Mitigation: the extracted helper uses the same pattern as the inlined code. No behavioral change.
3. **Dead code removal** — removing signals could cascade-remove event handlers wired to them. Mitigation: these signals are *never read in JSX or used as event handler deps*. Verified by `TS6133` — TypeScript guarantees they're unused.

---

## Completion Signal

This spec is complete when:
1. All four anti-patterns fixed
2. `npx tsc --noEmit` exits 0 with zero errors
3. `bun run build` passes
4. Code committed to master with message `refactor: component cleanup — fix createResource abuse, signal soup, defineProperty duplication, dead code`
