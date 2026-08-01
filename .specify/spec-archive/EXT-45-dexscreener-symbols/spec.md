# Specification: DexScreener Symbol Extraction

**Spec ID:** EXT-45-dexscreener-symbols
**Date:** 2026-03-29
**Status:** Complete
**Class:** Feature
**Priority:** P2 — DexScreener already in manifest but symbol extraction doesn't work there.
**Depends on:** EXT-43-main-world-bridge
**Series:** EXT-43 through EXT-46 (Multi-platform chart scraping)

---

## Problem Statement

DexScreener (`dexscreener.com`) is already in the extension's manifest (`host_permissions` + `content_scripts.matches`), but Alt+X fails there because:

1. Symbol selectors (`SYMBOL_SELECTORS` in `scraper.ts`) only target TradingView.com-specific elements (`#header-toolbar-symbol-search`, etc.)
2. On non-TV sites, only Strategy 2 (Chart API) is attempted — which is dead code (see EXT-43)
3. No symbol fallback is attempted on non-TV sites

DexScreener uses Chakra UI with hashed CSS classes (unstable). It loads TradingView's charting library directly via `<script src="/tv/v27.001/charting_library.js">`. The EXT-43 bridge handles position tool scraping. This spec adds symbol extraction.

### DOM Findings (from inspection)

- **URL format**: `dexscreener.com/{chain}/{token-address}` — token address, not symbol
- **Toolbar**: Shows `USD / SOL` as the pair
- **Chart legend**: `pippin/SOL (Market Cap) on Raydium · 15` — TradingView legend format
- **Page title**: Contains pair info (e.g., "pippin (PIPPIN) | dexscreener")
- **Framework**: Chakra UI (`body.chakra-ui-dark`) — styled-components hashes are unstable

---

## User Stories

- **As a trader on DexScreener**, I want Alt+X to detect the trading pair so I can open the trade modal.
- **As a trader**, I want the symbol normalized to backend format (e.g., "PIPPINSOL" or extracted base token).

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `isDexScreener()` platform detection function | High | Extension |
| FR-2 | Extract symbol from TradingView legend line (charting lib embedded directly) | High | Scraper |
| FR-3 | Fallback: extract from `document.title` (often contains token symbol in parens) | Medium | Scraper |
| FR-4 | Fallback: scan for `XXX / YYY` pattern in leaf elements (toolbar pair display) | Medium | Scraper |
| FR-5 | Bridge injection enabled on DexScreener | High | Extension |

---

## Technical Implementation

### content.ts — Platform detection

```typescript
function isDexScreener(): boolean {
  return location.hostname.includes("dexscreener.com");
}
```

### scraper.ts — Symbol extraction

```typescript
function scrapeDexScreenerSymbol(): string | null {
  // Strategy 1: TradingView legend (charting lib is embedded, not iframe)
  // Legend shows "TOKEN/QUOTE (description) on DEX · timeframe"
  for (const sel of [
    '[data-name="legend-source-item"] [class*="title"]',
    '[class*="legendTitle"]',
  ]) {
    const el = document.querySelector(sel);
    if (el?.textContent) {
      const match = el.textContent.trim().match(/^([A-Za-z0-9]+)\/([A-Za-z]+)/);
      if (match) return (match[1] + match[2]).toUpperCase();
    }
  }

  // Strategy 2: document.title — "tokenName (SYMBOL) Price | DEX Screener"
  const parenMatch = document.title.match(/\(([A-Z0-9]{2,10})\)/);
  if (parenMatch) return parenMatch[1] + "USD";

  // Strategy 3: slash-separated pair in title — "TOKEN / SOL"
  const slashMatch = document.title.match(/([A-Za-z0-9]+)\s*\/\s*([A-Za-z]+)/);
  if (slashMatch) return (slashMatch[1] + slashMatch[2]).toUpperCase();

  // Strategy 4: scan leaf spans/divs for "XXX / YYY" pattern
  const elements = document.querySelectorAll("span, div, a");
  for (const el of elements) {
    if (el.children.length > 0) continue;
    const text = el.textContent?.trim() || "";
    if (/^[A-Z]{2,10}\s*\/\s*[A-Z]{2,10}$/.test(text)) {
      return text.replace(/\s*\/\s*/, "");
    }
  }

  return null;
}
```

### Files

- `src/content.ts` — add `isDexScreener()`, include in `isChartPlatform()`
- `src/scraper.ts` — add `scrapeDexScreenerSymbol()`, call from `scrapeSymbol()`

---

## Acceptance Criteria

- [ ] Alt+X on DexScreener token page detects the trading pair
- [ ] Symbol extracted correctly from at least one strategy (legend, title, or DOM scan)
- [ ] Modal opens with detected symbol pre-filled
- [ ] Bridge injects on DexScreener (verify in console)
- [ ] If position tool drawn, bridge returns entry/stop/target
- [ ] `bun run build` passes

---

## Risks

1. **DexScreener DOM varies by chain/token** — different token pages may structure data differently. Mitigation: 4 fallback strategies, plus bridge symbol extraction as final fallback.
2. **CSP may block bridge injection** — DexScreener may have strict Content Security Policy. Mitigation: fall back to DOM-only scraping if bridge injection fails.

---

## Completion Signal

This spec is complete when:
1. Alt+X detects the correct symbol on at least 3 different DexScreener token pages
2. All acceptance criteria met
3. `bun run build` passes
4. Code committed to master
