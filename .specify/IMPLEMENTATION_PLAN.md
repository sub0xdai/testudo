# Implementation Plan

> Last updated: 2026-02-08
> Current spec: EXT-05-auth-live (next)
> Phase: 4 (Browser Extension - Testudo Sniper)

---

## Active Phase: Testudo Sniper Extension (EXT-01 through EXT-06)

### Spec Progress

| Spec | Status | Notes |
|------|--------|-------|
| EXT-01 Extension Scaffold | COMPLETE | Manifest V3, esbuild, Chrome+Firefox |
| EXT-02 DOM Scraper | COMPLETE | 3-strategy scraper with fallbacks |
| EXT-03 Confirmation Modal | COMPLETE | Shadow DOM modal, Alt+X hotkey, R:R display |
| EXT-04 REST Execution | COMPLETE | REST via background worker, symbol normalization, position sizing |
| EXT-05 Auth & Live | PENDING | JWT, BinanceAdapter activation |
| EXT-06 WebSocket | PENDING | WS upgrade, status indicator |

### Architecture

- **Directory:** `testudo-extension/`
- **Build:** TypeScript + esbuild → `dist/chrome/` + `dist/firefox/`
- **Communication:** REST first (POST /trades), WebSocket later (EXT-06)
- **Dependencies:** webextension-polyfill for cross-browser compat

### Discoveries

- TradingView uses obfuscated class names — must use data attributes and structural patterns
- `#header-toolbar-symbol-search` for ticker, `button[data-value][aria-checked="true"]` for timeframe
- Position tool dialogs appear in `#overlap-manager-root`
- Content scripts can't fetch cross-origin in MV3 — must route through background worker
- Backend uses `BTC_USDT` format (underscore-separated), TradingView uses `BTCUSDT` (concatenated)
- Backend requires `quantity` field — calculated client-side: `risk_amount / stop_distance`
- Backend side values: `"buy"` / `"sell"` (not LONG/SHORT)

---

## Previous Phase: 008-shadow-fill-engine (COMPLETE)

### Tasks

| ID | Task | Status | Notes |
|----|------|--------|-------|
| T1.1 | Add `get_active_symbols()` to ShadowEngine | complete | |
| T1.2 | Add test for `get_active_symbols()` | complete | |
| T2.1 | Create `PriceFeedService` in `services/price_feed.rs` | complete | |
| T2.2 | Export from `services/mod.rs` | complete | |
| T2.3 | Spawn in `router/main.rs` at startup | complete | |
| T2.4 | Add integration test (mock ticker -> order fills) | complete | |
| T3.1 | Replace hardcoded balance in `trade_management.rs` | complete | |
| T3.2 | Verify risk calculation uses actual balance | complete | |
| T4.1 | Update `OpenOrders.tsx` badge for pending vs filled | complete | |

---

## Completed Specs

| Spec | Completion Date | Notes |
|------|-----------------|-------|
| 001-deprecate-legacy-engine | 2026-01-20 | Shadow Engine routing |
| 002-panic-prevention | 2026-01-20 | Result propagation |
| 003-risk-enforcement | 2026-01-20 | risk_validated field |
| 004-read-compute-write | 2026-01-20 | Lock-minimizing pattern |
| 005-atomic-cascades | 2026-01-21 | TransactionContext |
| 006-execution-latency | 2026-01-26 | 3μs avg, 70k orders/sec |
| 007-redis-to-postgres | 2026-01-31 | Unified data layer, pg_queue crate |
| 008-shadow-fill-engine | 2026-02-07 | Price feed, balance wiring, UI labels |

---

## Next Up

Phase 3 candidates after 008:

- Analytics Dashboard (P&L tracking, win rate, drawdown)
- Multi-Strategy Support (strategy registry in Decision Loop)
- Live Exchange Integration (production Binance Futures)
- Mobile Optimization (responsive position tool, touch gestures)

---

*This file is persistent state. Ralph updates it each iteration.*
