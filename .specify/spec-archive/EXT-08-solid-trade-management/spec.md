# EXT-08: Solid.js Rewrite + Trade Management Configuration

> Priority: P0 | Depends on: EXT-07 | Status: complete

## Overview
**Current:** Extension popup and modal are vanilla TypeScript with manual DOM manipulation. Trade payload sends a fixed `quantity` calculated client-side from hardcoded 100 USDT risk. No trade management rules — orders are fire-and-forget.
**Target:** Rewrite all extension UI in Solid.js + Tailwind CSS. Add trade management configuration (risk %, break-even, trailing stop, partial TP) to popup settings. Include `management` block in trade payload. Backend calculates position size from risk % and real exchange balance.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Rewrite popup in Solid.js with Tailwind CSS dark theme | complete |
| FR-2 | Rewrite confirmation modal in Solid.js with Tailwind CSS (Shadow DOM retained) | complete |
| FR-3 | Add trade management settings section to popup: risk %, break-even trigger %, trailing stop toggle + distance %, partial TP toggle + close % | complete |
| FR-4 | Persist management config to chrome.storage as a named preset object (single preset for v1, structured for future array of presets) | complete |
| FR-5 | Display active management rules summary in confirmation modal before trade submission | complete |
| FR-6 | Update trade payload: remove `quantity`, add `management` block with `risk_percent`, `break_even_at`, `trailing_stop`, `partial_tp` | complete |
| FR-7 | Update build system: add solid-js, vite-plugin-solid or babel-preset-solid to esbuild, add Tailwind CSS compilation | complete |
| FR-8 | Background worker remains vanilla TypeScript (no framework — no UI) | complete |
| FR-9 | Migrate existing Vitest unit tests to Solid testing equivalents | complete |
| FR-10 | Migrate existing Playwright E2E tests to work with new Solid UI | complete |
| FR-11 | WebSocket order update toasts rendered in Solid with event-type styling (green/profit, red/stop, blue/amendment) | complete |

## Data Model

### Management Config (stored in chrome.storage)
```typescript
interface ManagementPreset {
  name: string;                  // "default" for v1
  risk_percent: number;          // e.g. 1.0
  break_even_at: number;         // % of distance to target, e.g. 50
  trailing_stop: {
    enabled: boolean;
    distance_percent: number;    // % of entry-to-target distance, e.g. 25
  };
  partial_tp: {
    enabled: boolean;
    close_percent: number;       // % of position to close at target, e.g. 50
  };
}
```

### Updated Trade Payload
```typescript
interface TradePayload {
  symbol: string;                // "BTC_USDT"
  side: "LONG" | "SHORT";
  entry: number;
  stop: number;
  target: number;
  timeframe: string;
  management: {
    risk_percent: number;
    break_even_at: number;
    trailing_stop: { enabled: boolean; distance_percent: number };
    partial_tp: { enabled: boolean; close_percent: number };
  };
}
```

### WebSocket Event Types (display only — backend defines these in EXT-09)
```
order.filled        → green toast
order.amended       → blue toast
order.trailing      → blue toast
order.partial_close → green toast
order.stopped       → red toast
order.tp_hit        → green toast
```

## Key Files

| File | Purpose |
|------|---------|
| `testudo-extension/src/popup/App.tsx` | Solid.js popup root component |
| `testudo-extension/src/popup/components/Settings.tsx` | Connection settings (backend URL, WS URL) |
| `testudo-extension/src/popup/components/TradeManagement.tsx` | Management config inputs |
| `testudo-extension/src/popup/components/AuthSection.tsx` | Login/logout UI |
| `testudo-extension/src/popup/components/StatusBar.tsx` | WebSocket status indicator |
| `testudo-extension/src/popup/popup.html` | Popup HTML entry point (updated for Solid mount) |
| `testudo-extension/src/popup/index.tsx` | Solid render entry point |
| `testudo-extension/src/modal.tsx` | Solid.js confirmation modal (replaces modal.ts) |
| `testudo-extension/src/content.ts` | Content script (mounts Solid modal into Shadow DOM) |
| `testudo-extension/src/background.ts` | Unchanged — vanilla TS service worker |
| `testudo-extension/src/types.ts` | Updated with ManagementPreset, new TradePayload |
| `testudo-extension/src/scraper.ts` | Unchanged |
| `testudo-extension/tailwind.config.ts` | Tailwind dark theme config |
| `testudo-extension/build.ts` | Updated with Solid + Tailwind build pipeline |

## Architecture

### Build Pipeline
```
src/background.ts    → esbuild (ESM, no framework)     → dist/chrome/background.js
src/content.ts       → esbuild + solid + tailwind (IIFE) → dist/chrome/content.js
src/popup/index.tsx  → esbuild + solid + tailwind (IIFE) → dist/chrome/popup/popup.js
```

### Popup Component Tree
```
App
├── StatusBar (WS indicator, connection dot)
├── Settings (backend URL, WS URL, execution mode)
├── TradeManagement (risk %, BE, trailing, partial TP)
└── AuthSection (login form or logged-in display)
```

### Modal Component Tree
```
ConfirmationModal (Shadow DOM host)
├── Header (symbol, side badge, timeframe)
├── PriceTable (entry, stop, target)
├── RiskReward (R:R ratio with color)
├── ManagementSummary (active rules list)
└── Actions (Enter to confirm, Escape to dismiss)
```

### Design Reference
- Dark theme: `#1a1a2e` background (existing), clean typography
- Todoist extension aesthetic: generous spacing, subtle borders, no visual clutter
- Solid.js signals for reactive state (WS status, auth state, settings)
- Tailwind utility classes, no custom CSS except where Tailwind falls short

## Verification
```bash
cd testudo-extension && bun run typecheck && bun run build
# dist/chrome/ and dist/firefox/ produced with Solid.js bundles
bun run test
# All unit tests pass (Solid testing library)
bun run test:e2e
# All E2E tests pass with new Solid UI
```

## Acceptance Criteria
- [x] Popup renders in Solid.js with Tailwind dark theme
- [x] Modal renders in Solid.js inside Shadow DOM
- [x] Trade management settings persist to chrome.storage
- [x] Management config stored as named preset object `{ name: "default", ... }`
- [x] Trade payload includes `management` block, no `quantity` field
- [x] All existing functionality preserved (Alt+X, scraping, auth, WS status)
- [x] Toast notifications styled per event type
- [x] Bundle size < 100KB total (popup + content + background)
- [x] All unit tests migrated and passing
- [x] All E2E tests migrated and passing
- [x] `bun run typecheck && bun run build` clean

## Completion Signal

### Implementation Checklist
- [x] All functional requirements implemented
- [x] All acceptance criteria verified
- [x] Code follows project constitution standards
- [x] No new linting warnings introduced

### Testing Requirements
- [x] `bun run test` passes (unit tests)
- [x] `bun run test:e2e` passes (E2E tests)
- [x] Manual verification: popup settings save and reload correctly
- [x] Manual verification: Alt+X modal shows management rules

### Done Signal
<promise>DONE</promise>
Output only when ALL criteria pass.
