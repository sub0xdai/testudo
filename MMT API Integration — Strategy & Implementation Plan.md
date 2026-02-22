# MMT API Integration — Strategy & Implementation Plan

## Context

Testudo currently has **zero live market data** — prices come from TradingView DOM scraping or manual entry. The extension has no price feed, no order flow, no depth, no OI. The Rust backend executes trades on Binance Futures but doesn't consume any third-party market intelligence.

**MMT** (Market Monkey Terminal) provides unified crypto market data from 20+ exchanges through one API key: volume delta, heatmaps, candles, open interest, liquidations, order flow, funding rates — REST and WebSocket, all normalized to one schema.

This plan maps out **all viable integration points** across the Testudo stack, then proposes a phased implementation starting with the highest-value, lowest-effort foundations.

---

## Part 1: Integration Opportunities (Ideas)

### A. Extension — Trade Decision Layer

| Opportunity | Data Source | Value | Effort |
|---|---|---|---|
| **1. Flow Context in Modal** | VD + OI | At Alt+X time, show whether flow supports or contradicts the trade direction. A LONG with bearish delta = red flag. | Low |
| **2. Flow Tab (persistent)** | VD + OI + Liquidations | Dedicated popup tab for on-demand market context for any symbol. Check flow without triggering a trade. | Low |
| **3. Liquidation Proximity Warnings** | Liquidation heatmap | When stop/entry is near a dense liquidation cluster, warn the user. Helps avoid stop-hunt zones. | Medium |
| **4. Position Enrichment** | VD + OI (live) | Show live delta/OI next to active positions in the Positions tab. "Is flow still with me?" | Medium |
| **5. Flow Score** | VD + OI + Liq composite | A single 0-100 score combining delta bias, OI trend, and liquidation proximity. Displayed alongside R:R in the modal as a "conviction meter." | Medium |
| **6. Multi-Exchange Context** | Aggregated VD/OI | Show how the same asset trades across Binance, Bybit, OKX, Deribit. Divergences between exchanges = signal. | Low (MMT handles aggregation) |
| **7. Smart Stop Placement** | Liquidation heatmap | Suggest stop adjustments based on liquidation cluster positions — "your stop is in a $50M liquidation zone, consider moving it 0.3% lower." | High |
| **8. Funding Rate Display** | Stats/Funding | Show current funding rate in the modal and Flow tab. Extreme funding = mean reversion signal. | Low |

### B. Rust Backend — Execution Intelligence Layer

| Opportunity | Data Source | Value | Effort |
|---|---|---|---|
| **9. Pre-Trade Flow Check (Decision Loop)** | VD + OI | Before executing a trade, the Decision Loop queries MMT for flow alignment. If delta strongly contradicts trade direction + OI is contracting, reject or flag the trade. This turns MMT into a risk gate. | Medium |
| **10. Dynamic Risk Adjustment** | VD + OI + Liq | PositionSizer already picks MIN(account%, fixed risk, max size, margin). Add a "flow factor" multiplier: reduce size when flow is adverse, allow full size when aligned. | Medium |
| **11. Flow Snapshots at Trade Entry** | VD + OI + Liq | When a trade is executed, snapshot the MMT flow state and store it alongside the trade in PostgreSQL. Enables post-mortem analysis: "Did I take trades against the flow? What was my win rate when flow was aligned vs. adverse?" | Medium |
| **12. Aggregated Price Feed** | Candles/Trades WS | Use MMT's multi-exchange trade stream as a supplementary price source for the Shadow Fill Engine. Currently shadow fills use Binance API only — MMT's aggregated stream gives cross-exchange price context. | High |
| **13. Funding Rate Arbitrage Detection** | Multi-exchange funding | Compare funding rates across exchanges. When Binance funding diverges significantly from Bybit/OKX, flag it as an arb opportunity. Could feed into a future multi-exchange strategy. | High |
| **14. Market Regime Detection** | VD + OI trends | Classify market as "accumulation" (low delta, expanding OI), "distribution" (high delta, contracting OI), "trending" (aligned delta + OI), or "choppy." Auto-adjust risk parameters per regime. | High |
| **15. WebSocket Redundancy** | Trades WS | MMT's aggregated trade stream as a backup to Binance's direct WS. If Binance API goes down, MMT still provides price data from other exchanges. | Medium |

### C. Cross-Cutting / Analytics

| Opportunity | Data Source | Value | Effort |
|---|---|---|---|
| **16. Trade Journal Enrichment** | Historical VD + OI | When reviewing past trades, overlay what the flow looked like at entry/exit. "I entered LONG here, but delta was bearish for the past 3 hours." | Medium |
| **17. Alert System** | VD + OI WS | Push notifications when significant flow events occur: whale volume spike, OI divergence, massive liquidation event. Extension shows a toast. | High |
| **18. Heatmap Overlay** | Heatmap data | Render a simplified heatmap visualization in the extension or web frontend showing where liquidity is stacked. | High |

---

## MMT API Reference (Research Summary)

### Authentication
```
Header: X-API-Key: YOUR_KEY
Base URL: https://eu-central-1.mmt.gg/api/v1/
```

### Known REST Endpoints

| Endpoint | Description | Weight |
|---|---|---|
| `GET /vd` | Volume delta bars | 1 |
| `GET /heatmap_sd` | Standard heatmap | ~1 |
| `GET /heatmap_hd` | HD heatmap | 10 |
| `GET /markets` | Available markets/symbols | 1 |
| Candles, OI, Liquidations, Stats | Inferred from docs, exact paths behind auth | ~1 each |

### Parameters (common across endpoints)
- `exchange` — `binancef`, `bybitf`, `okxf`, `deribitf`, `hyperliquid`, `bitmexf` (colon-separated for multi)
- `symbol` — `btc/usdt` (unified, lowercase, slash-separated)
- `tf` — timeframe: `1m`, `5m`, `15m`, `1h`, etc.
- `bucket` — trade-size filter for VD (1-11, where 11 = $5M+ trades)

### WebSocket
```json
{
  "type": "subscribe",
  "channel": "trades",
  "exchange": "binancef:bybitf:okxf",
  "symbol": "btc/usd"
}
```
Channels: `trades`, `candles`, `depth`, `heatmaps`, `volume_delta`

### Rate Limits
| Tier | Weight/Min | WS Connections | History |
|---|---|---|---|
| Basic ($99/mo) | 100 | 5 | 90 days |
| Pro ($399/mo) | 750 | 15 | 1 year |

Multi-exchange queries cost +20% per additional exchange.

---

## Part 2: Implementation Plan

### Phased Approach

```
Phase 0: Foundation (MMT client + settings + symbol mapping + mock data)
Phase 1: Modal Integration (flow context at trade confirmation time)
Phase 2: Flow Tab (persistent on-demand market context in popup)
Phase 3: Position Enrichment (live flow next to active positions)
Phase 4: Backend Integration (Decision Loop flow gate + snapshots)
```

This plan covers **Phases 0-3** (extension-side). Phase 4 (backend) is scoped but deferred — it requires API key access and real data to design properly.

---

### Phase 0: Foundation

**Goal:** MMT client module, symbol mapping, types, settings UI, rate limiter. All testable with mock data. Unlocks everything else.

#### 0.1 — Create `src/mmt/types.ts`

TypeScript interfaces for MMT API responses and derived summaries.

```typescript
// Key types:
MmtSymbol           // string, e.g. "btc/usdt"
MmtVolumeDeltaBar   // { t, o, h, l, c, v } — OHLCV-style delta bars
MmtOiBar            // { t, o, h, l, c } — OI candle
MmtMarketContext    // Derived summary: { symbol, volumeDelta: {bias, value, percent}, openInterest: {trend, current, change_percent}, error? }
MmtSettings         // { apiKey, exchange, enabled }
```

**File:** `testudo-extension/src/mmt/types.ts`

#### 0.2 — Create `src/mmt/symbolMap.ts`

Convert TradingView symbols (BTCUSDT, ETHUSDT.P) to MMT format (btc/usdt). Reuses the existing `QUOTE_CURRENCIES` list from `src/utils.ts`.

```typescript
export function toMmtSymbol(tvSymbol: string): string | null
// BTCUSDT   → btc/usdt
// XBTUSD    → btc/usd (override table)
// SOLUSDT.P → sol/usdt (strip .P suffix)
// UNKNOWN   → null
```

**File:** `testudo-extension/src/mmt/symbolMap.ts`
**Test:** `testudo-extension/src/mmt/symbolMap.test.ts`

#### 0.3 — Create `src/mmt/client.ts`

REST client with built-in rate limiter (token bucket, 90 of 100 weight/min). Lives in background worker scope.

```typescript
const MMT_BASE = "https://eu-central-1.mmt.gg/api/v1";

export async function fetchVolumeDelta(symbol, exchange, apiKey, tf?, limit?): Promise<...>
export async function fetchOpenInterest(symbol, exchange, apiKey, tf?, limit?): Promise<...>
```

Rate limiter: module-level `weightUsed` counter, reset every 60s. `canAfford(cost)` check before each call. Returns `null` when rate-limited (callers treat as "data unavailable").

**File:** `testudo-extension/src/mmt/client.ts`

#### 0.4 — Create `src/mmt/summarize.ts`

Pure functions that convert raw MMT responses into `MmtMarketContext`. Testable in isolation, no network, no browser APIs.

```typescript
export function summarizeVolumeDelta(resp): { bias: "bullish"|"bearish"|"neutral", value, percent }
export function summarizeOi(resp): { trend: "expanding"|"contracting"|"flat", current, change_percent }
export function buildMarketContext(symbol, vdResp, oiResp): MmtMarketContext
```

Logic: Last 2-3 bars net delta. >20% of volume = bullish/bearish. OI change >1% = expanding/contracting.

**File:** `testudo-extension/src/mmt/summarize.ts`
**Test:** `testudo-extension/src/mmt/summarize.test.ts`

#### 0.5 — Add MMT host permission

Add `*://*.mmt.gg/*` to `host_permissions` in manifest.json. Without this, service worker fetch to MMT is blocked by the browser.

**File:** `testudo-extension/manifest.json`

#### 0.6 — Wire into background.ts

Add 3 new message types to the message router:

```
MMT_MARKET_CONTEXT  { symbol: string } → MmtMarketContext
MMT_GET_SETTINGS    {} → MmtSettings
MMT_SAVE_SETTINGS   { apiKey, exchange } → { success: true }
```

Add `getMmtSettings()` and `fetchMarketContext(symbol)` functions. Pattern follows existing `getSettings()` and `getBalances()`.

**File:** `testudo-extension/src/background.ts`

#### 0.7 — Settings UI for API key

Add "MMT API" section to SettingsView with:
- API key input (`type="password"`, same onChange+save pattern as backendUrl)
- Exchange selector dropdown (binancef, bybitf, okxf, multi options)

**File:** `testudo-extension/src/popup/components/SettingsView.tsx`

---

### Phase 1: Modal Integration

**Goal:** When the user presses Alt+X, the trade confirmation modal fetches and displays flow context for the symbol.

#### 1.1 — Fetch MMT data in content.ts

In the Alt+X handler (content.ts), after fetching balance, also fetch market context:

```typescript
let marketCtx = null;
try {
  marketCtx = await browser.runtime.sendMessage({
    type: "MMT_MARKET_CONTEXT",
    symbol: setup?.symbol || symbolHint || "",
  });
} catch { /* non-blocking */ }
```

Pass `marketCtx` to `showModal()` as a new parameter.

**Files:** `testudo-extension/src/content.ts`, `testudo-extension/src/modal.tsx`

#### 1.2 — Display flow context in TradeForm

Add a compact "Market Flow" row between the Balance Summary and Footer sections of TradeForm. Renders as:

```
┌─────────────────────────────────────────────────┐
│  DELTA  BULLISH +2.3%  │  OI  EXPANDING +1.1%  │
└─────────────────────────────────────────────────┘
```

Color-coded: green/bullish, red/bearish, amber/neutral. Absent entirely when no API key configured or data unavailable.

Add CSS classes to `MODAL_STYLES` in modal.tsx (Shadow DOM, can't use popup Tailwind).

**Files:** `testudo-extension/src/components/TradeForm.tsx`, `testudo-extension/src/modal.tsx`

---

### Phase 2: Flow Tab

**Goal:** Persistent on-demand market context in the popup, accessible without triggering a trade.

#### 2.1 — Add "Flow" tab

Update `TabId` type and tabs array in TabBar.tsx:

```typescript
export type TabId = "trade" | "quick" | "positions" | "flow" | "account";
// Add: { id: "flow", label: "Flow", testId: "tab-flow" }
```

**File:** `testudo-extension/src/popup/components/TabBar.tsx`

#### 2.2 — Create FlowTab component

New Solid.js component with:
- Symbol input + refresh button
- Volume delta display card (bias, value, percent)
- Open interest display card (trend, current, change)
- "No API key" message when not configured (links to Settings)
- Last-updated timestamp

Fetches via `browser.runtime.sendMessage({ type: "MMT_MARKET_CONTEXT", symbol })`.

**Files:** `testudo-extension/src/popup/components/FlowTab.tsx`, `testudo-extension/src/popup/components/MainView.tsx`

---

### Phase 3: Position Enrichment

**Goal:** Show live flow data next to active positions in the Positions tab.

#### 3.1 — Enrich PositionCard with flow data

For each active position, fetch `MMT_MARKET_CONTEXT` for that position's symbol. Display a mini flow indicator (colored dot + "Bullish"/"Bearish") on each PositionCard.

Optimization: batch-fetch unique symbols only (if 3 positions are all BTCUSDT, fetch once).

**File:** `testudo-extension/src/popup/components/ActiveOrders.tsx`

---

### Phase 4: Backend Integration (Scoped, Deferred)

These are the highest-value backend opportunities, to be implemented once the extension integration is proven and you have an API key.

#### 4.1 — Decision Loop Flow Gate

Add an optional MMT check to `decision_loop.rs`. Before executing, if MMT is configured, fetch VD+OI for the symbol. If flow strongly contradicts trade direction (e.g., bearish delta > 30% on a LONG with contracting OI), either reject the trade or reduce position size.

This would be a new `FlowCheck` step in the Decision Loop pipeline, gated behind a feature flag.

**File:** `testudo-exchange/crates/router/src/decision_loop.rs`

#### 4.2 — Flow Snapshots in PostgreSQL

On trade execution, snapshot the MMT market context and store it as JSONB alongside the trade record. Enables future analytics: "win rate when flow-aligned vs. flow-opposed."

**Files:** `testudo-exchange/crates/sqlx_postgres/` (migration + model), `testudo-exchange/crates/router/` (capture at execution)

---

## Verification

### Phase 0 verification
- `bun test` — symbolMap.test.ts and summarize.test.ts pass
- `bun run build` — extension builds without errors for both Chrome and Firefox
- Load extension in Chrome → Settings → enter a test API key → "Saved" indicator shows
- Inspect background service worker console → no errors related to MMT imports

### Phase 1 verification
- On TradingView, press Alt+X → modal shows "Market Flow" row (or gracefully absent if no key)
- With mock/real API key: row shows delta bias + OI trend with correct color coding
- Without API key: row does not render at all (no empty space, no error)

### Phase 2 verification
- Open popup → "Flow" tab visible in tab bar
- Enter symbol, click refresh → flow data loads and displays
- Without API key → "Configure MMT API key in Settings" message with link

### Phase 3 verification
- With active positions → each PositionCard shows mini flow indicator
- Multiple positions with same symbol → only one MMT fetch per unique symbol

---

## File Change Summary

| File | Action | Phase |
|---|---|---|
| `src/mmt/types.ts` | Create | 0 |
| `src/mmt/symbolMap.ts` | Create | 0 |
| `src/mmt/symbolMap.test.ts` | Create | 0 |
| `src/mmt/client.ts` | Create | 0 |
| `src/mmt/summarize.ts` | Create | 0 |
| `src/mmt/summarize.test.ts` | Create | 0 |
| `manifest.json` | Modify — add host_permissions | 0 |
| `src/background.ts` | Modify — add 3 message handlers | 0 |
| `src/popup/components/SettingsView.tsx` | Modify — add MMT key/exchange fields | 0 |
| `src/content.ts` | Modify — fetch MMT context before modal | 1 |
| `src/modal.tsx` | Modify — pass context to TradeForm, add CSS | 1 |
| `src/components/TradeForm.tsx` | Modify — render flow context row | 1 |
| `src/popup/components/TabBar.tsx` | Modify — add Flow tab | 2 |
| `src/popup/components/FlowTab.tsx` | Create | 2 |
| `src/popup/components/MainView.tsx` | Modify — mount FlowTab | 2 |
| `src/popup/components/ActiveOrders.tsx` | Modify — add flow indicators | 3 |

All changes are within `testudo-extension/`. Backend changes (Phase 4) are deferred.

---

## Key Design Decisions

**REST on-demand, not WebSocket streaming (for now).** The popup lives for seconds to minutes. A persistent MMT WS would reconnect on every popup open, wasting a connection slot. REST fetch at modal-open and tab-refresh is cheaper and simpler. WS can be added in a future phase for the "alert system" idea.

**Rate limiter in client.ts, not background.ts.** The MMT client owns its rate budget (SRP). Background.ts stays a pure message router.

**Graceful degradation.** No API key = no MMT UI rendered. No errors, no empty states, just absent. The extension works exactly as before.

**`src/mmt/` subdirectory.** MMT is a self-contained integration with 4+ files. A subdirectory keeps it isolated and easy to delete or replace. esbuild resolves by path, no config change needed.

**Mock data first.** Since there's no API key yet, all pure logic (symbolMap, summarize) is TDD'd with mock data. The client.ts fetch calls can be tested manually once a key is available.
