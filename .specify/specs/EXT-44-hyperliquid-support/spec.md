# Specification: Hyperliquid Platform Support

**Spec ID:** EXT-44-hyperliquid-support
**Date:** 2026-03-29
**Status:** Draft
**Class:** Feature
**Priority:** P1 — Hyperliquid is the primary DEX integration; traders need Alt+X there.
**Depends on:** EXT-43-main-world-bridge
**Series:** EXT-43 through EXT-46 (Multi-platform chart scraping)

---

## Problem Statement

Hyperliquid (`app.hyperliquid.xyz`) is not in the extension's manifest — the content script doesn't inject there. Even if it did, the symbol scraper only has TradingView-specific selectors. Hyperliquid displays pairs as "BTC-USDC" in a `<div>` with styled-components hash classes (unstable, change between builds).

Hyperliquid embeds TradingView's charting library directly (not iframe), so the EXT-43 bridge will handle position tool scraping. This spec adds manifest permissions and symbol extraction.

### DOM Finding (from inspection)

```html
<div class="sc-bjfHbI bFBYgR" style="font-size: 20px; line-height: 30px; display: block;">BTC-USDC</div>
```

- Styled-component classes (`sc-bjfHbI`, `bFBYgR`) are **unstable** — change on every build
- Format: `BTC-USDC` (hyphen-separated, USDC quote)
- Element is a leaf div (no children) with the pair as text content

---

## User Stories

- **As a trader on Hyperliquid**, I want Alt+X to detect "BTCUSDC" as the symbol so I can open the trade modal.
- **As a trader on Hyperliquid**, I want position tools drawn on the chart to be scraped via the bridge.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `*://app.hyperliquid.xyz/*` to `host_permissions` in manifest | High | Extension |
| FR-2 | Add `*://app.hyperliquid.xyz/*` to `content_scripts.matches` in manifest | High | Extension |
| FR-3 | Add `isHyperliquid()` platform detection function in `content.ts` | High | Extension |
| FR-4 | Add Hyperliquid symbol scraper: walk leaf divs for `/^[A-Z0-9]{2,10}-USDC$/` pattern | High | Scraper |
| FR-5 | Convert Hyperliquid format to backend format: `BTC-USDC` → `BTCUSDC` | High | Scraper |
| FR-6 | Fallback: extract symbol from `document.title` | Medium | Scraper |
| FR-7 | Bridge injection enabled on Hyperliquid (via `isChartPlatform()` check) | High | Extension |

---

## Technical Implementation

### manifest.json

```diff
 "host_permissions": [
   ...
+  "*://app.hyperliquid.xyz/*"
 ],
 "content_scripts": [{
   "matches": [
     ...
+    "*://app.hyperliquid.xyz/*"
   ],
```

### content.ts — Platform detection

```typescript
function isHyperliquid(): boolean {
  return location.hostname.includes("hyperliquid");
}
```

### scraper.ts — Symbol extraction

```typescript
function scrapeHyperliquidSymbol(): string | null {
  // Strategy 1: Walk leaf divs for "BTC-USDC" pattern
  // Styled-component classes are unstable — match by text content only
  const divs = document.querySelectorAll("div");
  for (const div of divs) {
    if (div.children.length > 0) continue;
    const text = div.textContent?.trim() || "";
    if (/^[A-Z0-9]{2,10}-USDC$/.test(text)) {
      return text.replace("-", "");  // BTC-USDC → BTCUSDC
    }
  }

  // Strategy 2: document.title
  const match = document.title.match(/([A-Z0-9]{2,10})-USDC/);
  if (match) return match[0].replace("-", "");

  return null;
}
```

Called at the top of `scrapeSymbol()` when `location.hostname.includes("hyperliquid")`.

### Files

- `manifest.json` — add Hyperliquid URLs
- `src/content.ts` — add `isHyperliquid()`, include in `isChartPlatform()`
- `src/scraper.ts` — add `scrapeHyperliquidSymbol()`, call from `scrapeSymbol()`

---

## Acceptance Criteria

- [ ] Content script injects on `app.hyperliquid.xyz`
- [ ] Alt+X on Hyperliquid BTC-USDC page detects symbol "BTCUSDC"
- [ ] Alt+X opens modal with detected symbol in symbol-only mode
- [ ] Bridge injects on Hyperliquid (verify `TESTUDO_BRIDGE_READY` in console)
- [ ] If position tool is drawn, bridge returns entry/stop/target data
- [ ] `bun run build` passes

---

## Risks

1. **Leaf div scan performance** — walking all divs on a complex page could be slow. Mitigation: the regex match short-circuits quickly; Hyperliquid's DOM is relatively lean.
2. **Hyperliquid adds other quote currencies** — the regex is USDC-only. Mitigation: expand pattern if needed (`-USD[CT]?$`).

---

## Completion Signal

This spec is complete when:
1. Alt+X detects the correct symbol on Hyperliquid
2. Modal opens with the pair pre-filled
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
