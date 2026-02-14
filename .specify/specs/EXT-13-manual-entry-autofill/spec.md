# EXT-13: Architecture Inversion — Manual Entry with Auto-Fill

> Priority: P0 | Depends on: EXT-03 (modal), EXT-02 (scraper) | Status: Draft
> Created: 2026-02-14

## Overview

**Problem:** The extension has an existential dependency on TradingView's undocumented internals. All 6 scraper strategies access either `window.TradingViewApi` (an internal leak, not a public API) or DOM nodes that will vanish when TradingView completes its canvas migration. TradingView has no public API for data access and no plans to expose one for overlay tools. The extension is inert on any site that isn't `tradingview.com`.

**Solution:** Invert the architecture. Make the confirmation modal an **editable trade entry form** rather than a read-only display of scraped data. When the scraper succeeds, fields are auto-filled. When it fails (or the user is on a non-TradingView site), fields are empty and editable. The user can always override auto-filled values.

**Impact:**
- TradingView scraping becomes a convenience (auto-fill), not a requirement
- Extension works on DexScreener, GMX, Bybit charts, or any site — manual entry is always available
- Scraper breakage degrades to "user types 4 fields" instead of "extension is dead"
- Opens the door for popup-based trade entry without needing a charting page open at all

## User Stories

- [ ] As a trader on TradingView, I want the modal to auto-fill from my position tool so my workflow is unchanged.
- [ ] As a trader on TradingView, I want to edit auto-filled values before confirming so I can adjust entry/stop/target without redrawing.
- [ ] As a trader on DexScreener, I want to press Alt+X and manually enter a trade setup so I'm not locked to TradingView.
- [ ] As a trader, I want to enter a trade from the popup without any charting page open so I can act on ideas from Discord/Twitter/alerts.
- [ ] As a trader, I want the symbol to auto-detect from the page when possible, even if the position tool isn't drawn.

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | **Editable Modal Fields**: Entry, Stop, Target, Symbol, Side become editable input fields in the confirmation modal. When scraper returns data, fields are pre-filled. When scraper returns null, fields are empty. | High |
| FR-2 | **Field Validation**: Modal validates all fields before enabling confirm. Entry/Stop/Target must be positive numbers. Symbol must be non-empty. Side must be LONG or SHORT. R:R recalculates live as fields change. | High |
| FR-3 | **Side Toggle**: LONG/SHORT selector in the modal. Auto-inferred from prices when auto-filled (entry > stop = LONG). Manually toggleable. | High |
| FR-4 | **Symbol Auto-Detection**: When scraper fails to find a position tool, still attempt `scrapeSymbol()` from the page header. Pre-fill symbol field even when price fields are empty. | Medium |
| FR-5 | **Graceful Scraper Failure**: When `scrapeTradeSetup()` returns null, show the editable form with empty fields instead of the "No position tool detected" error. Remove the error-only fallback. | High |
| FR-6 | **Multi-Platform Content Script**: Expand manifest `content_scripts.matches` and `host_permissions` to include DexScreener, GMX, and other charting platforms. Alt+X hotkey works on all matched sites. Scraper attempts Strategy 0 on any site (may work if site embeds TradingView library). | Medium |
| FR-7 | **Popup Quick Trade**: Add a "Quick Trade" view/tab in the popup that provides the same manual entry form (symbol, side, entry, stop, target). Sends the same `EXECUTE_TRADE` message to background. Does not require any charting page to be open. | Medium |
| FR-8 | **Keyboard Flow Preserved**: Tab navigates between fields. Enter confirms (with double-Enter for live mode). Escape dismisses. Auto-filled fields are selected on focus for quick overwrite. | High |
| FR-9 | **Balance/Sizing Still Works**: BalanceSummary component recalculates live as entry/stop/target fields change, not just on initial render. Position size, margin, risk in USDT update reactively. | High |
| FR-10 | **Auto-Fill Indicator**: When fields are auto-filled from scraper, show a subtle visual indicator (e.g., small icon or "auto" badge) so the user knows data was scraped vs. manually entered. Clicking the indicator clears the field. | Low |
| FR-11 | **Strategy 0 Health Detection**: Add runtime detection for when `window.TradingViewApi` disappears or changes shape (missing `activeChart`, missing `getAllShapes`, etc.). Log the failure mode and scraper strategy that was attempted. On failure, fall back to manual entry silently — no error toast, just empty fields. | High |
| FR-12 | **Scraper Telemetry**: Track which scraper strategy succeeded (or that all failed) per invocation. Store last N results in `browser.storage.local` under a `scraperHealth` key. Surface in popup settings or account tab so the user knows if auto-fill is degrading. | Medium |
| FR-13 | **Cross-Platform Strategy 0 Probing**: On non-TradingView sites, attempt Strategy 0 (`window.TradingViewApi` or `window.ChartApiInstance`) as a speculative probe — many charting platforms embed TradingView's charting library and may expose the same widget API. If it works, auto-fill proceeds normally. If it fails, manual entry. No DOM strategies attempted on non-TV sites. | Medium |

## Technical Notes

### Files to Modify

| File | Change |
|------|--------|
| `src/modal.tsx` | Rewrite `ConfirmationModal` — replace read-only display rows with editable input fields. Add Solid.js signals for each field. Add validation logic. R:R and BalanceSummary become reactive to field changes. |
| `src/content.ts` | Change `showModal(setup, ...)` call — pass `setup` even when null (no longer an error state). Export `scrapeSymbol()` for standalone symbol detection. |
| `src/scraper.ts` | Export `scrapeSymbol()` as a public function (currently private). Add `getChartApiHealth()` function that reports the availability and shape of `window.TradingViewApi` (exists, has `activeChart`, has `getAllShapes`, etc.). Add strategy result tracking — each invocation records which strategy succeeded or that all failed. |
| `src/types.ts` | Add `QuickTradePayload` interface if popup quick trade needs different typing (likely reuses `TradePayload`). |
| `manifest.json` | Expand `content_scripts.matches` and `host_permissions` for DexScreener, GMX, etc. |
| `src/popup/components/MainView.tsx` | Add "Quick Trade" as a new tab option. |
| `src/popup/components/TabBar.tsx` | Add "Quick Trade" tab ID and icon. |

### Files to Create

| File | Purpose |
|------|---------|
| `src/popup/components/QuickTrade.tsx` | Standalone trade entry form in popup — symbol, side, entry/stop/target inputs, management preset summary, confirm button. Sends `EXECUTE_TRADE` to background. |
| `src/components/TradeForm.tsx` | **Shared** editable trade form component used by both the modal and QuickTrade. Accepts optional `initialSetup` prop for auto-fill. Contains all validation, signals, and field rendering. |

### Architecture

#### Current Flow (scraping required)
```
Alt+X → scrapeTradeSetup() → setup | null
  ├── setup found  → showModal(setup)     → read-only display → Enter → execute
  └── setup null   → showModal(null)      → "No position tool detected" → dead end
```

#### New Flow (scraping optional)
```
Alt+X → scrapeTradeSetup() → setup | null
  ├── setup found  → showModal(setup)     → editable form (pre-filled) → Enter → execute
  └── setup null   → scrapeSymbol()       → editable form (symbol only or empty) → Enter → execute

Popup Quick Trade tab
  └── editable form (always empty) → Confirm → execute
```

#### Shared TradeForm Component
```
TradeForm(props: { initialSetup?: TradeSetup; management: ManagementPreset; balance?: BalanceResponse[]; isLiveMode: boolean; onConfirm: (setup: TradeSetup) => void; onDismiss?: () => void })
  ├── [signal] symbol
  ├── [signal] side
  ├── [signal] entry
  ├── [signal] stop
  ├── [signal] target
  ├── [derived] rr (reactive)
  ├── [derived] isValid (reactive)
  ├── BalanceSummary (reactive to field changes)
  └── ManagementSummary (static from preset)
```

### Modal Input Styling

Inputs must work inside the Shadow DOM. No Tailwind available — use inline styles matching the existing `MODAL_STYLES` pattern:

```css
.field-input {
  width: 100%;
  background: rgba(255,255,255,0.05);
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 8px;
  padding: 8px 12px;
  font-size: 14px;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  color: #fff;
  outline: none;
  transition: border-color 0.15s;
}
.field-input:focus { border-color: rgba(59,130,246,0.5); }
.field-input.invalid { border-color: rgba(239,68,68,0.5); }
.field-input.auto-filled { border-color: rgba(52,211,153,0.3); }
```

### Multi-Platform Host Permissions

```json
{
  "host_permissions": [
    "*://*.tradingview.com/*",
    "*://*.dexscreener.com/*",
    "*://*.gmx.io/*",
    "*://*.bybit.com/*",
    "http://localhost/*",
    "http://127.0.0.1/*"
  ],
  "content_scripts": [
    {
      "matches": [
        "*://*.tradingview.com/*",
        "*://*.dexscreener.com/*",
        "*://*.gmx.io/*",
        "*://*.bybit.com/*"
      ],
      "js": ["content.js"],
      "run_at": "document_idle"
    }
  ]
}
```

### What Doesn't Change

- `background.ts`: Zero changes. `EXECUTE_TRADE` message format unchanged.
- `scraper.ts`: All 6 strategies remain. Only change is exporting `scrapeSymbol()`.
- Trade execution payload: Same `TradePayload` shape — symbol, side, entry, stop, target, management.
- Management presets: Unchanged. Still loaded from `browser.storage.local`.
- Auth flow: Unchanged. JWT/paper mode works the same.
- Double-Enter live safety: Unchanged. Applied in TradeForm.

### Risk Assessment

- **Low risk**: The core change is making existing read-only fields editable. No new backend API calls, no new data formats, no auth changes.
- **Scraper backward-compatible**: If Strategy 0 works, the user experience is identical — fields auto-fill, user hits Enter. The only visible difference is that fields have cursor focus affordance.
- **Testing**: Existing E2E tests verify the Alt+X → Enter flow. They need updating to handle input fields instead of static text, but the flow is the same.

---

## Acceptance Criteria

- [ ] Alt+X on TradingView with position tool drawn: modal opens with all fields pre-filled, R:R calculated, Enter executes
- [ ] Alt+X on TradingView without position tool drawn: modal opens with empty editable fields (or symbol pre-filled from header), no error message
- [ ] User can edit any auto-filled field and R:R / BalanceSummary recalculate live
- [ ] Alt+X on DexScreener: modal opens with empty fields, user enters trade manually, Enter executes
- [ ] Popup "Quick Trade" tab: form with symbol/side/entry/stop/target, confirm sends trade to backend
- [ ] Confirm button / Enter key disabled until all fields are valid (positive numbers, non-empty symbol, side selected)
- [ ] Double-Enter safety for live mode still works
- [ ] Tab key navigates between input fields in logical order (symbol → side → entry → stop → target)
- [ ] Escape dismisses modal from any field focus state
- [ ] `bun run build` succeeds for Chrome and Firefox
- [ ] `bun run test` passes (updated tests for new input-based flow)
- [ ] Existing scraper strategies are unmodified (no regression)

---

## Completion Signal

### Implementation Checklist
- [ ] `scrapeSymbol()` exported from `scraper.ts`
- [ ] `getChartApiHealth()` added to `scraper.ts` — reports `window.TradingViewApi` availability and shape
- [ ] Strategy result tracking added — records which strategy succeeded per invocation
- [ ] `scraperHealth` storage key — last N scraper results persisted to `browser.storage.local`
- [ ] `TradeForm` shared component created with reactive signals
- [ ] `ConfirmationModal` rewritten to use `TradeForm` with editable inputs
- [ ] Modal CSS updated with `.field-input` styles in Shadow DOM
- [ ] `content.ts` updated — null scrape result shows editable form, not error
- [ ] `content.ts` updated — attempts `scrapeSymbol()` fallback when full scrape fails
- [ ] `content.ts` updated — on non-TV sites, only attempt Strategy 0 probe (no DOM strategies)
- [ ] `manifest.json` expanded with multi-platform host permissions
- [ ] `QuickTrade.tsx` popup component created
- [ ] `MainView.tsx` updated with Quick Trade tab
- [ ] `TabBar.tsx` updated with Quick Trade tab ID
- [ ] Validation logic: all fields required, positive numbers, non-empty symbol
- [ ] R:R recalculates reactively on field change
- [ ] BalanceSummary recalculates reactively on field change
- [ ] Keyboard navigation: Tab between fields, Enter confirm, Escape dismiss
- [ ] Scraper health visible in popup (settings or account tab)

### Testing Requirements
- [ ] `bun run build` exits 0
- [ ] `bun run test` exits 0
- [ ] Manual: Alt+X on TradingView with position tool → auto-filled editable form
- [ ] Manual: Alt+X on TradingView without position tool → empty editable form
- [ ] Manual: Edit auto-filled values → R:R updates, balance recalculates
- [ ] Manual: Alt+X on non-TradingView site → empty form, manual entry works
- [ ] Manual: Popup Quick Trade → enter trade → executes via backend
- [ ] Manual: Live mode double-Enter safety preserved
- [ ] Manual: Invalid fields prevent confirmation
- [ ] Manual: Scraper health indicator visible in popup after several Alt+X invocations
- [ ] Manual: On non-TV site (e.g. DexScreener), Strategy 0 probe attempted, DOM strategies skipped

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

*Template version: 1.0*
