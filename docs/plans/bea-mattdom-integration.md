# Plan: MattDOM (Bleeding Edge Alpha) Integration - Exploration

**Status:** Plan only - no code changes
**Date:** 2026-08-06
**Goal:** Explore what integration with MattDOM is possible via its public API. Data consumption is a means, integration is the end. Product shape, branding, and native form (web-first, desktop later) are undecided.

---

## 1. What MattDOM is (from public site)

Bleeding Edge Alpha (`bleedingedgealpha.net`) is a market-intelligence platform:
- **MattDOM** - authenticated web workspace: depth-of-market, tape, heatmaps
- **Premium** ($8/mo, $75/yr, $200 lifetime, Stripe): Big Positions overlay (OI changes sorted with tape), Liquidation Clusters overlay, Lessons, unrestricted API access, early feature access
- **/board** - secondary surface (SPA, not inspectable without login)
- **API** - the only public integration surface: REST v1 (Bearer token) + Socket.IO WS + tokenless history
- Auth: email/password accounts, API tokens created in a web token manager, stored server-side as hashes

**MattDOM has NO execution layer.** No trading/order/position endpoints exist in the documented API. Testudo brings execution, risk, and journal. The integration direction is data-out of MattDOM, execution-out of Testudo.

---

## 2. Feasibility matrix (public API only)

| Integration shape | What it means | Feasibility | Friction |
|---|---|---|---|
| **A. Data layer in Testudo** | MattDOM tick/dom/tape/heatmap/history consumed by extension, journal, coach, agents | **HIGH** - fully documented | BEA token needed for REST/WS (history is tokenless) |
| **B. Native combined web app** | Web app where MattDOM data is the workspace (DOM/tape/heatmap via their WS) and Testudo executes (signals/trades API) | **HIGH** - both sides fully documented, no SPA reverse-engineering | New app surface; auth model (BEA token vs Testudo JWT vs both) |
| **C. Extension on MattDOM** | Testudo extension gains bleedingedgealpha.net content-script target, scrapes symbol/price like TradingView | **MEDIUM** - MattDOM SPA is authenticated + uncooperative; no drawing tools known; fragile scraping of third-party SPA | Needs a MattDOM account to inspect; may break on their updates |
| **D. Data INTO MattDOM** | Testudo signals/positions rendered inside MattDOM | **LOW** - no inbound endpoints, no webhooks in public API | Not possible today |

**Recommendation:** explore A and B. A is the smallest feasible slice and the foundation; B is the highest-value "native" surface. C only if an account reveals a chart-like surface worth targeting. D is off the table with public API only.

---

## 3. Unknowns to resolve before committing (all resolvable with one live token)

1. **Full OpenAPI spec** - the docs page hides the endpoint reference until a Bearer token is pasted ("Load OpenAPI to see endpoints"). A live token reveals the complete endpoint list - may include endpoints beyond `/health` and `/market/tick` (e.g. symbols list, funding, positions of big holders).
2. **dom/tape/heatmap payload schemas** - docs only detail `tick` and history candle fields. These three WS events are untyped until captured live.
3. **Rate limits** - default-tier specifics unknown; affects whether clients hit BEA directly or proxy through Testudo backend.
4. **Supported symbol universe** - docs show `BTC-USD`, `ETH-USD`, `SOL-USD`; full list (perps? spot? all venues?) unknown until the API is queried.
5. **MattDOM SPA structure** - only matters for shape C; needs an account.

**Prerequisite step: capture the live API surface.** With one BEA token:
- Paste into the docs page → dump the OpenAPI spec
- `curl /api/v1/health` and `/api/v1/market/tick?symbol=BTC-USD`
- Connect to Socket.IO `/api`, `subscribe_symbols`, record one of each `tick|dom|tape|heatmap`
- `curl /api/history/BTC-USD` for the candle shape
- Save everything into `docs/plans/bea-api-capture/` as ground truth

---

## 4. Exploration path (small slices, decide as we learn)

### Step 1 - API capture + feasibility proof (no product commitment)
- Capture OpenAPI, WS payloads, history shape (Section 3)
- Verify Testudo's existing WS infra (PG NOTIFY pub/sub) can relay `market.{symbol}` channels unchanged - yes, ws-stream is channel-agnostic; only a new producer side is needed
- Verify the extension background can hold a second socket (Socket.IO) alongside the existing order-updates WS - no architectural conflict, both live in the service worker

### Step 2 - Shape A: data layer vertical slice
- Extension popup live tick panel: aggregated + per-venue price for the scraped TradingView symbol
- Backend: `GET /api/v1/market/tick` proxy with 1s in-memory cache (one token server-side, rate-limit safe)
- This proves the whole chain: BEA → client → extension UI. Reusable for any later shape.

### Step 3 - Shape B: native web app
- New Solid.js + vite app (reuse testudo-journal stack, lightweight-charts v5 already a dependency)
- MattDOM-data workspace: DOM ladder, tape, heatmap, Big-Positions-style panel (if API exposes OI), candles from `/api/history` (tokenless!)
- Testudo execution panel: symbol, side, SL/TP → `POST /api/v1/trades` (or `/signals`) with Kelly sizing preview
- Auth decision: BEA token for data (or proxy via Testudo), Testudo JWT for execution
- Branding stays neutral until decided

### Step 4 (optional) - Shape C: extension on MattDOM
- Only after an account exists and the SPA is inspected
- Add `*://*.bleedingedgealpha.net/*` to content-script targets; scrape symbol/price from MattDOM DOM
- If MattDOM has no chart/drawing surface, the extension still works as a symbol-aware execution panel

### Step 5 (deferred) - Desktop shell
- Tauri wrapper around the web app once it proves out

---

## 5. What each shape needs from MattDOM

| Need | A (data layer) | B (native app) | C (ext on MattDOM) |
|---|---|---|---|
| BEA token (user) | Optional (history is tokenless) | Optional | No (uses site session) |
| Testudo JWT | No | Yes (execution) | Yes |
| OpenAPI full spec | Useful | Useful | No |
| dom/tape/heatmap schemas | Yes | Yes | No |
| MattDOM account for inspection | No | No | **Yes** |

---

## 6. Decision gates

- **After Step 1:** is the live API richer than documented (more endpoints)? If yes, re-score shape D (inbound) and add any newly discovered shapes.
- **After Step 2:** extension tick panel works end-to-end → green light for Step 3.
- **After Step 3:** user tests the web app → decide branding, hosting, and whether to pursue Step 4/5.

---

## 7. Risks

- **Hidden endpoints don't exist** - OpenAPI may reveal nothing beyond documented; shapes stay as scored above (acceptable, A and B don't depend on it)
- **dom/tape/heatmap schemas differ from expectations** - captured in Step 1, before any UI is built
- **Rate limits bite under default tier** - mitigated by backend proxy cache (Shape A) and single shared subscription
- **BEA API changes** - out of our control; pin payload versions in captured schemas, validate with zod (extension) and serde (backend)
- **MattDOM SPA churn** (Shape C only) - scraping fragility; low priority path
