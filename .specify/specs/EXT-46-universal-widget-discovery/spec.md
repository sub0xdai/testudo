# Specification: Universal TradingView Widget Discovery

**Spec ID:** EXT-46-universal-widget-discovery
**Date:** 2026-04-14
**Status:** Draft
**Class:** Feature / Extension
**Priority:** P1 — Extension only works on tradingview.com; all other embedded TradingView sites (Bybit, GMX, DexScreener) fail silently
**Depends on:** None
**Series:** EXT-46 (standalone)

---

## Problem Statement

The Testudo extension's DOM scraper and chart API bridge only work on tradingview.com. On every other site that embeds the TradingView charting library (Bybit, GMX, DexScreener, Hyperliquid), pressing Alt+X fails silently — no position tool data is scraped despite the drawing tool being visible on the chart.

Root cause: `findChartWidget()` in `page-bridge.ts` only checks 3 hardcoded global variable names: `TradingViewApi`, `ChartApiInstance`, `tvWidget`. On tradingview.com, the widget is exposed as `window.TradingViewApi`. On embedded sites, the developer stores the widget instance under their own variable name — or in a closure with no global reference at all. The lookup always returns `null`.

The TradingView Charting Library API is identical whether on tradingview.com or embedded. `activeChart()`, `getAllShapes()`, `getShapeById()`, `getProperties()` all work the same way (confirmed via TradingView Charting Library v29 docs). The `long_position` and `short_position` drawing tools are supported line tools in the charting library. The only problem is **finding the widget instance on the page**.

---

## User Stories

- **As a trader using Bybit's chart**, I want to draw a Long/Short Position tool and press Alt+X, so that Testudo scrapes my entry/stop/target and opens the trade confirmation modal — the same as on TradingView.
- **As a trader using any TradingView-powered charting platform**, I want the extension to automatically find the chart widget regardless of which site I'm on, so that I don't have to switch to tradingview.com to use Testudo.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `findChartWidget()` discovers TradingView widget instances stored under any global variable name via `Object.getOwnPropertyNames(window)` scan | High | page-bridge |
| FR-2 | A constructor hook intercepts `TradingView.widget()` calls at `document_start` to capture widget instances stored in closures (Chrome only) | High | widget-hook |
| FR-3 | Constructor hook uses `Object.defineProperty` on `window.TradingView` to intercept namespace creation, then patches `.widget` constructor | High | widget-hook |
| FR-4 | Captured widget stored on `window.__TESTUDO_TV_WIDGET__` for bridge consumption | High | widget-hook |
| FR-5 | Firefox build strips `"world": "MAIN"` content script entries (not supported at `strict_min_version: "112.0"`) — Firefox users get window scan only | Medium | build |
| FR-6 | Existing TradingView.com functionality is unaffected (regression-safe) | High | page-bridge |
| FR-7 | Non-chart pages incur zero overhead — hook and scan are no-ops when `TradingView` is undefined | Medium | widget-hook |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Window property scan in `findChartWidget()` | Alt+X on Bybit finds widget via scan → scrapes position tool |
| CP-2 | Constructor hook (`widget-hook.ts`) + manifest + build | Alt+X on sites with closure-stored widgets (Chrome) |
| CP-3 | Firefox build handling | `bun run build` produces correct Firefox manifest without MAIN world entries |

### Architecture

Two-layer discovery, ordered by reliability:

```
Alt+X pressed → page-bridge.ts findChartWidget()
  │
  ├─ Tier 1: window.__TESTUDO_TV_WIDGET__ (set by constructor hook)
  │   └─ Catches closure-stored widgets (Chrome only)
  │
  ├─ Tier 2: Known globals (TradingViewApi, ChartApiInstance, tvWidget)
  │   └─ Fast path for TradingView.com
  │
  └─ Tier 3: Object.getOwnPropertyNames(window) scan
      └─ Finds any object with activeChart() method (all browsers)
```

### Widget Hook (`widget-hook.ts`)

Runs in MAIN world at `document_start` — before any page JS executes.

```typescript
(function () {
  let _tv: any = (window as any).TradingView;

  function patchConstructor(tv: any): void {
    if (!tv?.widget || tv.__testudo_patched__) return;
    const Orig = tv.widget;
    tv.widget = function (this: any, ...args: any[]) {
      const instance = new Orig(...args);
      (window as any).__TESTUDO_TV_WIDGET__ = instance;
      return instance;
    };
    tv.widget.prototype = Orig.prototype;
    tv.__testudo_patched__ = true;
  }

  // If already defined, patch now
  if (_tv) patchConstructor(_tv);

  // Intercept future assignment
  Object.defineProperty(window, 'TradingView', {
    get() { return _tv; },
    set(val) {
      _tv = val;
      if (val && typeof val === 'object') {
        // Watch for .widget being assigned later (incremental pattern)
        let _widget = val.widget;
        if (typeof _widget === 'function') {
          patchConstructor(val);
        } else {
          Object.defineProperty(val, 'widget', {
            get() { return _widget; },
            set(w) { _widget = w; if (typeof w === 'function') patchConstructor(val); },
            configurable: true, enumerable: true,
          });
        }
      }
    },
    configurable: true,
    enumerable: true,
  });
})();
```

### Updated `findChartWidget()` in `page-bridge.ts`

```typescript
function findChartWidget(): any | null {
  const w = window as any;

  // Tier 1: Captured by constructor hook
  if (w.__TESTUDO_TV_WIDGET__?.activeChart) return w.__TESTUDO_TV_WIDGET__;

  // Tier 2: Known global names
  for (const name of ['TradingViewApi', 'ChartApiInstance', 'tvWidget']) {
    if (w[name]?.activeChart) return w[name];
  }

  // Tier 3: Window property scan
  for (const key of Object.getOwnPropertyNames(w)) {
    try {
      const val = w[key];
      if (val && typeof val === 'object' && !Array.isArray(val)
          && typeof val.activeChart === 'function') {
        return val;
      }
    } catch { /* cross-origin or getter errors */ }
  }

  return null;
}
```

### Paved Roads

- **Page bridge injection pattern**: `content.ts:47-53` injects `page-bridge.js` via `<script>` tag for MAIN world access. Same pattern available for widget-hook if needed.
- **IIFE build entries**: `build.ts:34-36` `BRIDGE_ENTRIES` array — widget-hook uses the same build configuration.
- **Manifest content scripts**: `manifest.json:22-34` already lists all target sites. New entry follows same structure.
- **Bridge message protocol**: `page-bridge.ts:154-176` request/response via `window.postMessage` — no changes needed.

### Files

- `testudo-extension/src/widget-hook.ts` — **New.** MAIN world constructor hook.
- `testudo-extension/src/page-bridge.ts` — **Modify.** Replace `findChartWidget()` with 3-tier discovery.
- `testudo-extension/manifest.json` — **Modify.** Add `document_start` + `MAIN` world content script entry.
- `testudo-extension/build.ts` — **Modify.** Add `widget-hook.ts` to `BRIDGE_ENTRIES`. Strip MAIN world entries for Firefox in `writeManifest()`.

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Alt+X on Bybit chart with position tool drawn → trade confirmation modal opens with correct entry/stop/target
- [ ] Alt+X on TradingView.com → still works (regression check)
- [ ] Alt+X on non-chart page → no errors, no modal
- [ ] `bun run build` succeeds for both Chrome and Firefox targets
- [ ] Firefox manifest does NOT contain `"world": "MAIN"` content script entries
- [ ] Chrome manifest DOES contain the widget-hook entry with `"run_at": "document_start"` and `"world": "MAIN"`
- [ ] Console shows `[Testudo]` log indicating which discovery tier matched

---

## Risks

1. **`Object.defineProperty` conflicts** — Other extensions or the site itself might also define property traps on `window.TradingView`. Mitigation: use `configurable: true` and `__testudo_patched__` guard.
2. **Window scan false positives** — A non-TradingView object might have an `activeChart()` method. Mitigation: low probability; the method name is specific to TradingView's API. Could add secondary check for `getAllShapes` if needed.
3. **iframe-embedded charts** — If a site loads TradingView inside a cross-origin iframe, neither hook nor scan reaches it. Mitigation: punt for now; most CEX sites embed the library directly. Can add iframe URL patterns to manifest later.

---

## Completion Signal

This spec is complete when:
1. Widget discovery works on at least Bybit and one other embedded TradingView site
2. All acceptance criteria met
3. `bun run build` passes for both Chrome and Firefox
4. Code committed to master
