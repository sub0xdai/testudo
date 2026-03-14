# Spec: EXT-14-concurrency-hardening - Extension Concurrency & Leak Hardening

> Priority: P1 | Depends on: EXT-13 | Status: Draft
> Date: 2026-02-16

---

## Overview

Fix race conditions, memory leaks, and a blocking bug discovered during codebase audit of testudo-extension. The extension has 7 concurrency issues (2 high, 3 medium, 2 low) and 1 outright bug that breaks field-focus UX.

**Current:** Token refresh has no mutual exclusion — concurrent 401s can stampede and log users out. Alt+X hotkey can double-fire during async gaps. Scraper telemetry silently loses records under rapid mutations. TradeForm `autoFilled()` reference throws ReferenceError on every field focus.

**Target:** All async flows are guarded against concurrent access. Service worker restart is resilient. DOM resource creation is bounded. Every field interaction works correctly.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Token refresh guard | Module-scope `Promise` latch (single in-flight refresh) | Simplest mutex for service worker — no external deps, awaitable by multiple callers |
| 401 retry depth | Max 1 retry per request | Prevents infinite recursion while covering the common case (expired token) |
| Alt+X guard | Synchronous `pending` flag set before first await | Closes the async gap without needing a full mutex |
| Scraper observer | Remove dead code (`startWatching`/`stopWatching`) | Never called anywhere — hotkey path is the real flow |
| WS reconnect debounce | Trailing-edge debounce on `connectWebSocket` (300ms) | Collapses rapid `storage.onChanged` firings into single reconnect |
| Toast accumulation | Cap at 3 concurrent toasts, reuse oldest host | Bounds DOM growth during WS update bursts |
| `activeTrades` side effect | Extract `onCountChange` call to `createEffect` | Keeps derived signals pure per Solid.js best practice |

---

## Functional Requirements

| ID | Requirement | Severity | Files | Status |
|----|-------------|----------|-------|--------|
| FR-1 | Fix `autoFilled()` ReferenceError — replace with `autoFilledFields().size > 0` in all 4 onFocus handlers | Bug | `src/components/TradeForm.tsx` | pending |
| FR-2 | Token refresh mutex — only one `refreshAccessToken()` call in flight at a time; concurrent callers await the same Promise | High | `src/background.ts` | pending |
| FR-3 | Retry depth limit — `executeTrade`, `listTrades`, `getBalances` retry 401 at most once (no recursion) | High | `src/background.ts` | pending |
| FR-4 | Alt+X async guard — set synchronous `pending` flag before first await; check it alongside `!isVisible()` | Medium | `src/content.ts` | pending |
| FR-5 | Scraper telemetry atomicity — batch storage writes or use a write queue to prevent lost-update races | Medium | `src/scraper.ts` | pending |
| FR-6 | WebSocket reconnect debounce — `storage.onChanged` for `wsUrl` debounces `connectWebSocket()` calls by 300ms | Medium | `src/background.ts` | pending |
| FR-7 | Move `onCountChange` side effect out of `activeTrades()` derived signal into a `createEffect` | Medium | `src/popup/components/ActiveOrders.tsx` | pending |
| FR-8 | Remove dead `startWatching`/`stopWatching` observer code and module-scope `observer`/`onToolDetected` closures | Low | `src/scraper.ts` | pending |
| FR-9 | Cap concurrent toast hosts at 3; dismiss oldest when exceeded | Low | `src/modal.tsx` | pending |
| FR-10 | All existing tests continue to pass; new tests added for FR-2, FR-3, FR-4 | Testing | `src/background.test.ts`, `src/utils.test.ts` | pending |

---

## Implementation Details

### FR-1: Fix `autoFilled()` ReferenceError

**File:** `src/components/TradeForm.tsx` lines 186, 207, 225, 243

Replace all 4 instances of:
```tsx
onFocus={(e) => autoFilled() && e.currentTarget.select()}
```
with:
```tsx
onFocus={(e) => autoFilledFields().size > 0 && e.currentTarget.select()}
```

Per-field variant (more precise — only select if *that specific field* was auto-filled):
```tsx
// line 186 (symbol field)
onFocus={(e) => autoFilledFields().has("symbol") && e.currentTarget.select()}
// line 207 (entry field)
onFocus={(e) => autoFilledFields().has("entry") && e.currentTarget.select()}
// etc.
```

The per-field variant is preferred — it only selects text when the specific field was auto-filled, giving manual-entry fields normal focus behavior.

### FR-2: Token Refresh Mutex

**File:** `src/background.ts`

Add a module-scope latch:

```ts
let refreshInFlight: Promise<boolean> | null = null;

async function refreshAccessToken(): Promise<boolean> {
  if (refreshInFlight) return refreshInFlight;

  refreshInFlight = doRefresh();
  try {
    return await refreshInFlight;
  } finally {
    refreshInFlight = null;
  }
}

async function doRefresh(): Promise<boolean> {
  // ... existing refresh logic (read token, POST /auth/refresh, store new tokens)
}
```

Multiple concurrent callers awaiting `refreshAccessToken()` all receive the same result. The second 401 handler doesn't fire a duplicate refresh.

### FR-3: Retry Depth Limit

**File:** `src/background.ts`

Replace recursive retry pattern with a single-attempt guard. Apply to `executeTrade`, `listTrades`, `getBalances`:

```ts
async function executeTrade(payload: TradePayload, retried = false): Promise<BackendResponse> {
  // ... existing fetch logic ...
  if (response.status === 401 && tokens && !retried) {
    const refreshed = await refreshAccessToken();
    if (refreshed) return executeTrade(payload, true);  // max 1 retry
  }
  // ...
}
```

Same pattern for `listTrades` and `getBalances`.

### FR-4: Alt+X Async Guard

**File:** `src/content.ts`

```ts
let altXPending = false;

document.addEventListener("keydown", async (e: KeyboardEvent) => {
  if (e.altKey && e.key.toLowerCase() === "x" && !isVisible() && !altXPending) {
    altXPending = true;  // synchronous — closes the gap
    e.preventDefault();
    e.stopPropagation();
    try {
      // ... existing async scrape + modal logic ...
    } finally {
      altXPending = false;
    }
  }
}, true);
```

### FR-5: Scraper Telemetry Atomicity

**File:** `src/scraper.ts`

Replace fire-and-forget read-modify-write with a serialized write queue:

```ts
let telemetryQueue: Promise<void> = Promise.resolve();

function recordScraperResult(strategyUsed: number | null): void {
  const record: ScraperHealthRecord = {
    timestamp: Date.now(),
    strategyUsed,
    success: strategyUsed !== null,
  };

  telemetryQueue = telemetryQueue.then(async () => {
    const stored = await browser.storage.local.get(["scraperHealth"]);
    const history = (stored.scraperHealth as ScraperHealthRecord[]) || [];
    history.push(record);
    const trimmed = history.slice(-SCRAPER_HEALTH_MAX);
    await browser.storage.local.set({ scraperHealth: trimmed });
  }).catch(() => {});
}
```

Each write waits for the previous one to complete, preventing lost updates.

### FR-6: WebSocket Reconnect Debounce

**File:** `src/background.ts`

```ts
let wsDebounceTimer: ReturnType<typeof setTimeout> | null = null;

function debouncedConnectWebSocket(): void {
  if (wsDebounceTimer) clearTimeout(wsDebounceTimer);
  wsDebounceTimer = setTimeout(() => {
    wsDebounceTimer = null;
    connectWebSocket();
  }, 300);
}

// Replace storage listener:
browser.storage.onChanged.addListener((changes) => {
  if (changes.wsUrl) {
    debouncedConnectWebSocket();
  }
});
```

### FR-7: Pure Derived Signal

**File:** `src/popup/components/ActiveOrders.tsx`

Extract the side effect:

```tsx
import { createSignal, createEffect, onMount, onCleanup, For, Show } from "solid-js";

// Replace the existing activeTrades with a pure computation:
const activeTrades = () => trades().filter((t) => t.status !== "Completed");

// Separate effect for the side-effect:
createEffect(() => {
  props.onCountChange?.(activeTrades().length);
});
```

### FR-8: Remove Dead Observer Code

**File:** `src/scraper.ts`

Delete lines 533-566 (`observer`, `onToolDetected`, `startWatching`, `stopWatching`). These are never called from anywhere in the codebase.

### FR-9: Toast Cap

**File:** `src/modal.tsx`

```ts
const activeToasts: HTMLElement[] = [];
const MAX_TOASTS = 3;

export function showToast(message: string, type: ToastStyle = "success"): void {
  // Evict oldest if at cap
  while (activeToasts.length >= MAX_TOASTS) {
    const oldest = activeToasts.shift();
    oldest?.remove();
  }

  const host = document.createElement("div");
  // ... existing shadow DOM creation ...

  activeToasts.push(host);

  setTimeout(() => {
    toast.classList.remove("visible");
    setTimeout(() => {
      host.remove();
      const idx = activeToasts.indexOf(host);
      if (idx !== -1) activeToasts.splice(idx, 1);
    }, 300);
  }, 2000);
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/components/TradeForm.tsx` | FR-1: Fix 4x `autoFilled()` -> per-field `autoFilledFields().has()` |
| `src/background.ts` | FR-2: Refresh mutex, FR-3: retry depth limit, FR-6: WS debounce |
| `src/content.ts` | FR-4: Alt+X async guard flag |
| `src/scraper.ts` | FR-5: Telemetry write queue, FR-8: Remove dead observer code |
| `src/popup/components/ActiveOrders.tsx` | FR-7: Extract side effect to `createEffect` |
| `src/modal.tsx` | FR-9: Toast cap at 3 |
| `src/background.test.ts` | FR-10: Tests for refresh mutex, retry limit |
| `src/utils.test.ts` | FR-10: Additional utility tests if needed |

---

## Acceptance Criteria

1. `bun run test` — all existing tests pass, no regressions
2. `bun run typecheck` — zero type errors
3. `autoFilled()` ReferenceError is gone — fields can be focused without throwing
4. Concurrent 401 responses share a single refresh call (verified by test)
5. `executeTrade` retries at most once on 401 (verified by test mocking 2 consecutive 401s)
6. Alt+X pressed twice rapidly during async gap produces only one modal
7. `grep -n "autoFilled()" src/` returns zero matches
8. `grep -n "startWatching\|stopWatching" src/scraper.ts` returns zero matches
9. No recursive `return executeTrade(payload)` without depth guard — `grep -n "return executeTrade\|return listTrades\|return getBalances" src/background.ts` all include `retried` parameter

---

## Completion Signal

All race conditions and memory leaks from the Feb 16 audit are resolved. `bun run test` and `bun run typecheck` pass clean. Manual verification: Alt+X on TradingView produces one modal, field focus works, rapid WS updates don't accumulate unbounded DOM nodes.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Refresh mutex holds during network timeout | `doRefresh` has existing try/catch + 30s fetch timeout; callers get the same rejection |
| Dead code removal breaks imports | `startWatching`/`stopWatching` are not imported anywhere (verified by grep) |
| Toast cap hides important notifications | 3 is generous — order fills are rare enough; oldest is evicted, not suppressed entirely |
| `altXPending` flag not reset on unhandled rejection | `finally` block guarantees reset even on throw |
