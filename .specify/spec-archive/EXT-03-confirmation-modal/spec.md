# EXT-03: Confirmation Modal & Hotkey

> Priority: P0 | Depends on: EXT-02 | Status: COMPLETE

## Overview
**Current:** Scraper extracts trade data but no way to trigger or confirm execution.
**Target:** `Alt+X` hotkey triggers confirmation modal overlaying TradingView chart. Modal displays trade details and R:R ratio. `Enter` confirms, `Escape` dismisses.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Alt+X hotkey triggers scraper + modal from content script | DONE |
| FR-2 | Modal overlay — Z-index > 9999, centered, non-blocking | DONE |
| FR-3 | Trade summary — side, symbol, entry, stop, target | DONE |
| FR-4 | R:R calculation — (Target-Entry)/(Entry-Stop) | DONE |
| FR-5 | Keyboard control — Enter to execute, Escape to dismiss | DONE |
| FR-6 | Error state — "No position tool detected" when scraper returns null | DONE |
| FR-7 | Configurable hotkey (stored in chrome.storage) | DEFERRED (low priority) |

## Key Files

| File | Purpose |
|------|---------|
| `testudo-extension/src/modal.ts` | Shadow DOM modal + toast notifications |
| `testudo-extension/src/content.ts` | Alt+X listener, wires scraper → modal → execute |

## Architecture
- Modal injected as **Shadow DOM** to avoid TradingView CSS conflicts
- Dark theme matching TradingView aesthetic
- R:R color coding: green (>=2), amber (>=1), red (<1)
- Toast notification system for execution feedback (EXT-04)
- `executeTrade()` stub ready for EXT-04 REST dispatch

## Verification
```bash
cd testudo-extension && bun run typecheck && bun run build
# Manual: load in Chrome, open TradingView, Alt+X shows modal or error
```

<promise>DONE</promise>
