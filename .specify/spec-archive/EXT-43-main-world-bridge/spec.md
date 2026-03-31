# Specification: Main-World Bridge for TradingView Chart API

**Spec ID:** EXT-43-main-world-bridge
**Date:** 2026-03-29
**Status:** Complete
**Class:** Feature / Infrastructure
**Priority:** P1 — Chart API strategy is dead code due to Chrome isolated world; fixes position tool scraping on all platforms.
**Depends on:** None (first in series)
**Series:** EXT-43 through EXT-46 (Multi-platform chart scraping)

---

## Problem Statement

The extension's `findPositionToolByChartApi()` in `src/scraper.ts` accesses `window.TradingViewApi` — but Chrome content scripts run in an **isolated world** with a separate `window` object. The page's `window.TradingViewApi` is invisible to the content script. Strategy 2 silently returns `null` every time, even on TradingView.com.

The extension only works because DOM-based Strategies 0/1 (dialog inspection via `data-name` attributes) succeed on TradingView when the properties dialog is open.

DexScreener and Hyperliquid both embed TradingView's charting library directly (not iframe — DexScreener via `<script src="/tv/v27.001/charting_library.js">`, Hyperliquid via similar embed). A main-world bridge would enable Chart API access on ALL three platforms.

---

## User Stories

- **As a trader**, I want Alt+X to detect my position tool without opening the properties dialog, so that I can execute trades faster.
- **As a trader using DexScreener/Hyperliquid**, I want position tools on those platforms to work with Testudo, so that I'm not locked to TradingView.com.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create `page-bridge.ts` that runs in MAIN world and accesses TradingView chart API globals | High | Extension |
| FR-2 | Bridge probes multiple widget variable names: `TradingViewApi`, `ChartApiInstance`, `tvWidget` | High | Extension |
| FR-3 | Bridge extracts position tool data (entry, stop, target, side) via `getAllShapes()` + `getShapeById()` | High | Extension |
| FR-4 | Bridge extracts current symbol via `chart.symbol()` | Medium | Extension |
| FR-5 | Content script injects bridge via `<script>` tag, communicates via `postMessage` | High | Extension |
| FR-6 | Bridge request/response uses unique message type and request IDs to avoid collisions | High | Extension |
| FR-7 | Bridge request times out after 500ms (no hang if bridge fails to inject) | Medium | Extension |
| FR-8 | `page-bridge.js` added to `web_accessible_resources` in manifest | High | Extension |
| FR-9 | `build.ts` bundles `page-bridge.ts` as separate IIFE output | High | Build |

---

## Technical Implementation

### New File: `src/page-bridge.ts`

Runs in the page's JavaScript context (MAIN world). Injected by content script via `<script>` tag.

```typescript
// Probe all known TradingView widget global names
function findChartWidget(): any | null {
  const w = window as any;
  const widget = w.TradingViewApi || w.ChartApiInstance || w.tvWidget;
  if (!widget || typeof widget.activeChart !== "function") return null;
  return widget;
}
```

Responds to `TESTUDO_BRIDGE_REQUEST` messages with position tool data or symbol.

Tick size calculation and position tool extraction logic moved from `src/scraper.ts` (the existing `getTickSize`, `findPositionToolByChartApi` functions).

### Content Script Bridge Injection

```typescript
function injectBridge(): void {
  if (document.getElementById("testudo-bridge")) return;
  const script = document.createElement("script");
  script.id = "testudo-bridge";
  script.src = browser.runtime.getURL("page-bridge.js");
  (document.head || document.documentElement).appendChild(script);
}
```

### Communication Protocol

```
Content Script → Bridge:  { type: "TESTUDO_BRIDGE_REQUEST", action: "probe"|"getPositionTool"|"getSymbol", id: number }
Bridge → Content Script:  { type: "TESTUDO_BRIDGE_RESPONSE", id: number, data: any }
Bridge ready signal:      { type: "TESTUDO_BRIDGE_READY" }
```

### Files

- `src/page-bridge.ts` — **NEW**: main-world bridge script
- `src/content.ts` — inject bridge, add `bridgeRequest()` helper
- `manifest.json` — add `page-bridge.js` to `web_accessible_resources`
- `build.ts` — add `page-bridge.ts` to IIFE build entries

---

## Acceptance Criteria

- [ ] `page-bridge.js` exists in `dist/chrome/` and `dist/firefox/` after build
- [ ] Bridge script is injected on TradingView pages
- [ ] `bridgeRequest("probe")` returns `{ widgetFound: true }` on TradingView.com
- [ ] Position tool data returned via bridge matches what Strategy 0/1 returns (same prices)
- [ ] Bridge timeout (500ms) prevents hangs on sites where injection fails
- [ ] No console errors from bridge injection
- [ ] `bun run build` passes

---

## Risks

1. **Widget variable name varies by platform** — DexScreener/Hyperliquid may use a custom name. Mitigation: probe multiple names; add more as discovered.
2. **Content Security Policy blocks script injection** — some sites have strict CSP. Mitigation: fall back to DOM strategies if injection fails.

---

## Completion Signal

This spec is complete when:
1. Bridge script is built, injected, and communicating on TradingView.com
2. Position tool data is successfully extracted via bridge (verify in console)
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
