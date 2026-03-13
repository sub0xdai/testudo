# Specification: Performance & Cleanup

**Spec ID:** EXT-28-performance-cleanup
**Date:** 2026-03-14
**Status:** Draft
**Class:** Audit Fix
**Priority:** P1 — performance and bundle hygiene
**Audit Refs:** H-10, H-11, H-12, H-13, M-5, M-6, M-7, M-8, M-12, M-16, L-5, L-6, L-7, L-8, L-10

---

## Overview

Fix 15 performance issues across the extension: unnecessary reflows from layout animations, missing debounce on storage writes, redundant computations, duplicate event listeners, oversized toast stylesheets, unscoped CSS transitions, and dead code. Also remove unused assets and orphaned components.

**Current state:**
- Toggle card expand/collapse animates `max-height` — triggers layout reflow every frame (H-10)
- Every pixel of slider drag fires `browser.storage.local.set()` — 100-pixel drag = 100 writes (H-11)
- Each toast creates a new Shadow DOM with the full 6.2k `MODAL_STYLES` — only ~5 lines needed (H-12)
- `HeaderBar` and `StatusBar` both register independent listeners for `SIDECAR_STATUS_CHANGED` and make duplicate status requests (H-13)
- `transition: all` on all buttons/modal elements watches every animatable property (M-5)
- `backdrop-filter: blur(8px)` on modal forces GPU compositing of TradingView's dense canvas (M-6)
- ArcGauge applies `transition-all` to 21 SVG circles — only `r` and `opacity` change (M-7)
- `riskColor()` called 3x per render, `isLong()` called 4x per card with `parseFloat` each time (M-8)
- `forwardOrderUpdate` calls `browser.tabs.query` on every WebSocket message (M-12)
- `positions()` and `pendingOrders()` computed redundantly in `createEffect` and JSX (M-16)
- 3 unused font files shipped in source (cinzel, space-mono regular/bold) (L-5)
- 16k base64 JPEG embedded inline in CSS (L-6)
- ~12 console.log/warn calls remain in production build (L-7)
- `ExchangeManager.tsx` is complete but never rendered — dead code (L-8)

**Target state:**
- Zero layout-triggering animations — use CSS Grid or transform-based transitions
- Storage writes debounced at 200ms
- Minimal toast styles (~200 bytes vs 6.2k)
- Single sidecar status listener lifted to parent
- All transitions scoped to specific properties
- Computed values memoized
- Tab list cached with invalidation
- Dead code and unused assets removed
- Console calls stripped from production

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace `max-height` transition with CSS Grid `grid-template-rows: 0fr → 1fr` for toggle cards | High | popup.css |
| FR-2 | Debounce `browser.storage.local.set()` in TradeManagement — update signal immediately, persist at 200ms | High | TradeManagement |
| FR-3 | Create minimal `TOAST_STYLES` constant (~200 bytes) with only toast-relevant CSS rules | High | modal.tsx |
| FR-4 | Lift sidecar status to HeaderBar, pass to StatusBar as prop. Remove duplicate listener and request from StatusBar | High | HeaderBar, StatusBar |
| FR-5 | Scope `transition: all` to `background-color, border-color, color` on button base styles | Medium | popup.css |
| FR-6 | Reduce modal backdrop blur from `blur(8px)` to `blur(4px)` or use `background: rgba(0,0,0,0.7)` without blur | Medium | modal.tsx |
| FR-7 | Scope ArcGauge SVG circle transitions to `r, opacity` only | Medium | ArcGauge.tsx |
| FR-8 | Memoize `riskColor()` and `isLong()` with `createMemo` | Medium | TradeManagement, PositionCard |
| FR-9 | Cache `browser.tabs.query` result in background worker. Invalidate on `tabs.onCreated`/`tabs.onRemoved` | Medium | background.ts |
| FR-10 | Replace redundant `positions()` and `pendingOrders()` filter calls with `createMemo` | Medium | ActiveOrders.tsx |
| FR-11 | Delete unused font files: `cinzel-variable.woff2`, `space-mono-regular.woff2`, `space-mono-bold.woff2` | Low | src/fonts/ |
| FR-12 | Extract base64 JPEG from `popup.css` to a separate static asset file | Low | popup.css, build.ts |
| FR-13 | Add `drop: ["console"]` to esbuild production config | Low | build.ts |
| FR-14 | Delete orphaned `ExchangeManager.tsx` | Low | popup/components/ |

---

## Technical Implementation

### 1) CSS Grid Toggle Animation (FR-1)

```css
/* popup.css — replace max-height transition */
/* Before: */
.toggle-content {
  max-height: 0;
  overflow: hidden;
  transition: max-height 300ms ease;
}
.toggle-content.open {
  max-height: 120px;
}

/* After: */
.toggle-content {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 300ms ease;
}
.toggle-content.open {
  grid-template-rows: 1fr;
}
.toggle-content > div {
  overflow: hidden;
}
```

### 2) Debounced Storage (FR-2)

```typescript
// TradeManagement.tsx
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function updateField(field: string, value: number | boolean) {
  setPreset((prev) => ({ ...prev, [field]: value })); // immediate UI update

  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    browser.storage.local.set({ managementPreset: preset() });
  }, 200);
}
```

### 3) Minimal Toast Styles (FR-3)

```typescript
// modal.tsx — new constant
const TOAST_STYLES = `
  .toast {
    position: fixed; bottom: 16px; right: 16px;
    padding: 12px 20px; border-radius: 8px;
    font-family: 'DM Sans', system-ui, sans-serif;
    font-size: 14px; color: #fff;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    animation: toast-in 300ms ease forwards;
    z-index: 10001;
  }
  .toast-success { background: var(--color-signal-green, #22c55e); }
  .toast-error { background: var(--color-signal-red, #ef4444); }
  .toast-info { background: var(--color-accent-steel, #94a3b8); }
  @keyframes toast-in {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
`;
```

### 4) Lift Sidecar Status (FR-4)

```tsx
// HeaderBar.tsx — owns the listener, passes status down
const [sidecarStatus, setSidecarStatus] = createSignal("unknown");

onMount(() => {
  browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }).then(setSidecarStatus);
  const handler = (msg: any) => {
    if (msg.type === "SIDECAR_STATUS_CHANGED") setSidecarStatus(msg.status);
  };
  browser.runtime.onMessage.addListener(handler);
  onCleanup(() => browser.runtime.onMessage.removeListener(handler));
});

// Render:
<StatusBar status={sidecarStatus()} />

// StatusBar.tsx — becomes a pure presentational component
// Remove: onMount listener, onMessage listener, status signal
interface StatusBarProps { status: string; }
```

### 5) Scope Transitions (FR-5, FR-7)

```css
/* popup.css — button base */
button { transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease; }
```

```tsx
// ArcGauge.tsx — SVG circles
// Before: class="transition-all duration-700 ease-out"
// After:  style={{ transition: "r 700ms ease-out, opacity 700ms ease-out" }}
```

### 6) Memoize Computed Values (FR-8, FR-10)

```tsx
// TradeManagement.tsx
const riskColorMemo = createMemo(() => riskColor(preset().riskPercent));

// PositionCard.tsx
const isLongMemo = createMemo(() => parseFloat(trade.quantity) > 0);

// ActiveOrders.tsx
const positions = createMemo(() => trades().filter(t => t.status === "active"));
const pendingOrders = createMemo(() => trades().filter(t => t.status === "pending"));
```

### 7) Cache Tabs Query (FR-9)

```typescript
// background.ts
let cachedTabs: browser.Tabs.Tab[] | null = null;

browser.tabs.onCreated.addListener(() => { cachedTabs = null; });
browser.tabs.onRemoved.addListener(() => { cachedTabs = null; });

async function getContentTabs(): Promise<browser.Tabs.Tab[]> {
  if (!cachedTabs) {
    cachedTabs = await browser.tabs.query({});
  }
  return cachedTabs;
}
```

### 8) Production Console Drop (FR-13)

```typescript
// build.ts — add to esbuild options
{
  drop: isProduction ? ["console"] : [],
}
```

---

## Affected Files

| File | Changes |
|------|---------|
| `src/popup/popup.css` | Grid toggle animation, scoped button transitions, extract base64 JPEG |
| `src/popup/components/TradeManagement.tsx` | Debounced storage, memoized riskColor |
| `src/modal.tsx` | Minimal TOAST_STYLES, reduced backdrop blur |
| `src/popup/components/HeaderBar.tsx` | Own sidecar status, pass as prop |
| `src/popup/components/StatusBar.tsx` | Convert to presentational component |
| `src/popup/components/ArcGauge.tsx` | Scoped SVG transitions |
| `src/popup/components/PositionCard.tsx` | Memoized isLong |
| `src/popup/components/ActiveOrders.tsx` | Memoized positions/pendingOrders |
| `src/background.ts` | Cached tabs.query |
| `build.ts` | Console drop, asset copying |
| `src/fonts/cinzel-variable.woff2` | DELETE |
| `src/fonts/space-mono-regular.woff2` | DELETE |
| `src/fonts/space-mono-bold.woff2` | DELETE |
| `src/popup/components/ExchangeManager.tsx` | DELETE |

---

## Verification

```bash
cd testudo-extension && bun run build
```

- [ ] Build succeeds with no errors
- [ ] No `max-height` transitions remain — `grep 'max-height' src/popup/popup.css` returns no transition rules
- [ ] Storage write debounce in place — `grep 'setTimeout.*storage' src/popup/components/TradeManagement.tsx` returns match
- [ ] `TOAST_STYLES` exists and is ≤500 bytes — check modal.tsx
- [ ] StatusBar has no `onMount` or `onMessage` listener — pure presentational
- [ ] No `transition: all` or `transition-all` in popup.css or component classes — `grep -r 'transition.*all\|transition-all' src/` returns nothing
- [ ] `createMemo` used for riskColor, isLong, positions, pendingOrders — `grep -c 'createMemo' src/popup/components/` ≥ 4
- [ ] `drop: ["console"]` in build.ts production config
- [ ] Deleted files no longer exist: `ExchangeManager.tsx`, 3 font files
- [ ] Manual: toggle card expand/collapse is smooth (no jank)
- [ ] Manual: dragging sliders feels responsive (no storage write lag)

---

*Consolidates audit issues H-10, H-11, H-12, H-13, M-5, M-6, M-7, M-8, M-12, M-16, L-5, L-6, L-7, L-8, L-10.*
