# EXT-02: TradingView DOM Scraper

> Priority: P0 | Depends on: EXT-01 | Status: COMPLETE

## Overview
**Current:** Content script injects but does nothing.
**Target:** Content script identifies active Long/Short drawing tool, scrapes Entry/Stop/Target prices, and extracts ticker symbol and timeframe from chart header.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Identify active drawing tool (Long/Short Position) | DONE |
| FR-2 | Scrape price levels (Entry, Stop, Target) from floating DOM | DONE |
| FR-3 | Extract ticker & timeframe from chart header | DONE |
| FR-4 | Normalize values (strip commas, currency symbols) | DONE |
| FR-5 | Determine side (LONG/SHORT from price relationship) | DONE |
| FR-6 | Selector resilience with multiple fallback strategies | DONE |

## Key Files

| File | Purpose |
|------|---------|
| `testudo-extension/src/scraper.ts` | 3-strategy scraper with fallbacks |
| `testudo-extension/src/content.ts` | Content script with SCRAPE message handler |

## Architecture
The scraper uses 3 strategies in priority order:
1. **Properties Panel** — looks for labeled inputs in `#overlap-manager-root`
2. **Chart Overlay** — scans for `[data-name="long-position"]` etc.
3. **Price Scan** — fallback scanning all floating panels for position keywords

Symbol extraction uses 4 selector strategies. Timeframe uses `data-value` attributes.

## Risks
- TradingView DOM is unstable — selectors may break on updates
- Multiple fallback strategies mitigate single-selector failure

## Verification
```bash
cd testudo-extension && bun run typecheck && bun run build
# Manual: load in Chrome, open TradingView, draw Long Position, check console
```

<promise>DONE</promise>
