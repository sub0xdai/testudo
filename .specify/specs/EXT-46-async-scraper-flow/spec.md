# Specification: Multi-Platform Alt+X Flow

**Spec ID:** EXT-46-async-scraper-flow
**Date:** 2026-03-29
**Status:** Complete
**Class:** Feature / Integration
**Priority:** P1 — Ties EXT-43/44/45 together into a working Alt+X flow on all platforms.
**Depends on:** EXT-43-main-world-bridge, EXT-44-hyperliquid-support, EXT-45-dexscreener-symbols
**Series:** EXT-43 through EXT-46 (Multi-platform chart scraping)

---

## Problem Statement

The current Alt+X flow in `content.ts` has two issues:

1. **Symbol-only fallback is TradingView-only** — the `if (!setup && isTradingView())` guard at line 50 means DexScreener and Hyperliquid users get nothing if the full scrape fails.

2. **Strategy routing is broken for non-TV sites** — on non-TV sites, only Strategy 2 is attempted (which silently fails in isolated world). With the bridge (EXT-43), the scrape flow needs to: try bridge first → fall back to DOM strategies → fall back to symbol-only mode.

---

## Safety Approach

**No existing code is removed.** The bridge is added as a new path BEFORE existing strategies. Existing TradingView flow is 100% unchanged. Strategy 2 (`findPositionToolByChartApi`) stays in the array — it's harmless dead code that returns null. Dead code cleanup is a separate future task after the bridge is proven in production.

Key guard change: symbol-only fallback guard changes from `isTradingView()` to `isChartPlatform()` (TV + DexScreener + Hyperliquid only). This prevents spurious modal opens on random websites.

---

## User Stories

- **As a trader on any supported platform**, I want Alt+X to always at least detect the symbol, even if position tool data can't be scraped.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Alt+X tries bridge first (async, 500ms timeout), then existing DOM strategies, then symbol-only | High | Extension |
| FR-2 | Symbol-only fallback guard changes from `isTradingView()` to `isChartPlatform()` | High | Extension |
| FR-3 | `isChartPlatform()` = `isTradingView() \|\| isDexScreener() \|\| isHyperliquid()` | High | Extension |
| FR-4 | Bridge injection triggered on page load for chart platforms (not on Alt+X) | High | Extension |
| FR-5 | Existing `scrapeTradeSetup()` call path unchanged — no signature or behavior changes | High | Scraper |
| FR-6 | No dead code removed — Strategy 2 stays, `getChartApiHealth()` stays, all tests pass as-is | High | Scraper |

---

## Technical Implementation

### content.ts — Revised Alt+X flow (lines 44-55)

```typescript
try {
  let setup: TradeSetup | null = null;

  // 1. NEW: Bridge — Chart API in main world (all chart platforms)
  if (isChartPlatform()) {
    const bridgeResult = await bridgeRequest("probe");
    if (bridgeResult?.positionTool) {
      const symbol = bridgeResult.symbol || scrapeSymbol();
      if (symbol) {
        setup = {
          symbol,
          side: bridgeResult.positionTool.side,
          entry: bridgeResult.positionTool.entry,
          stop: bridgeResult.positionTool.stop,
          target: bridgeResult.positionTool.target,
          timeframe: isTradingView() ? scrapeTimeframe() : "chart",
        };
      }
    }
  }

  // 2. EXISTING: DOM strategies (unchanged — only on TradingView)
  if (!setup && isTradingView()) {
    setup = scrapeTradeSetup();  // all 6 strategies including dead Strategy 2
  }

  // 3. CHANGED GUARD: Symbol-only fallback (was isTradingView(), now isChartPlatform())
  if (!setup && isChartPlatform()) {
    const symbol = scrapeSymbol();
    if (symbol) {
      setup = { symbol, side: "LONG", entry: 0, stop: 0, target: 0, timeframe: "manual" };
    }
  }
```

### What stays exactly the same

- `scrapeTradeSetup()` signature and behavior — unchanged
- Strategy array (0-5) — unchanged, including dead Strategy 2
- `getChartApiHealth()` — unchanged
- `ChartApiHealth` type — unchanged
- All test assertions — unchanged
- Message listener `SCRAPE` handler (line 165-168) — unchanged
- Re-exports (line 180) — unchanged

### Files

- `src/content.ts` — add bridge-first path, change symbol fallback guard, add `isChartPlatform()`, inject bridge on load
- `src/scraper.ts` — add platform-specific branches in `scrapeSymbol()` (from EXT-44, EXT-45)

---

## Acceptance Criteria

- [ ] Alt+X on TradingView: bridge tried first, existing DOM fallback works, symbol-only fallback works
- [ ] Alt+X on Hyperliquid: bridge tried, symbol detected, modal opens
- [ ] Alt+X on DexScreener: bridge tried, symbol detected, modal opens
- [ ] **No regression**: TradingView with position tool dialog open → still scrapes correctly via DOM strategies
- [ ] **No regression**: TradingView without position tool → still gets symbol-only mode
- [ ] Existing tests pass without modification
- [ ] `bun run build` passes

---

## Risks

1. **Bridge timeout adds latency** — worst case 500ms if bridge hasn't loaded. Mitigation: bridge injected on page load, should be ready by Alt+X time.
2. **CSP blocks bridge on some platforms** — bridge injection may fail silently. Mitigation: existing DOM strategies still fire as fallback.

---

## Completion Signal

This spec is complete when:
1. Alt+X works on all three platforms (TradingView, Hyperliquid, DexScreener)
2. No regression on TradingView
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
