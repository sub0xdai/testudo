# Operational Learnings

> Ralph's accumulated knowledge. Loaded each iteration.

---

## Codebase Patterns

### Rust Backend (testudo-exchange)

- **Error handling**: Use `Result<T, E>` with custom error enums, not `.unwrap()`
- **Async**: All services use `tokio` async runtime
- **Decimal math**: Use `rust_decimal` for financial calculations, not f64
- **Lock pattern**: Use `lock_or_recover!` macro for mutex access
- **Testing**: Tests live in same file or `tests/` directory

### Browser Extension (testudo-extension)

- **Framework**: Solid.js (popup + modal), vanilla TS (background worker)
- **Styling**: Tailwind CSS for popup, inline CSS for Shadow DOM modal
- **Build**: esbuild + esbuild-plugin-solid, Tailwind CLI
- **Testing**: Vitest with vite-plugin-solid (unit), Playwright (E2E)
- **Output**: ESM (background), IIFE (content + popup), Chrome + Firefox
- **State**: chrome.storage.local for settings, management presets, auth tokens
- **Selectors**: Use `data-testid` attributes for E2E test selectors

### File Locations

- Execution types: `crates/common_utils/src/adapters/execution_types.rs`
- Risk calculations: `crates/common_utils/src/risk/`
- Router services: `crates/router/src/services/`
- Shadow engine: `crates/engine/src/shadow/`
- Extension popup: `testudo-extension/src/popup/`
- Extension modal: `testudo-extension/src/modal.tsx`
- Extension scraper: `testudo-extension/src/scraper.ts` (DO NOT MODIFY)
- Extension types: `testudo-extension/src/types.ts`

### Import Patterns

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Instant;
```

---

## Anti-Patterns (Don't Do This)

- Don't use `.unwrap()` on fallible operations
- Don't use `f64` for money/prices
- Don't hold locks across await points
- Don't modify tests to make them pass - fix the implementation

---

## Signs (Discoverable Patterns)

### When you see "clippy warning"
→ Fix the warning, don't suppress with `#[allow(...)]` unless truly necessary

### When you see "test failed"
→ Read the assertion message, trace back to find root cause, don't guess

### When you see "lock poisoned"
→ Use `lock_or_recover!` macro or handle with `.unwrap_or_else()`

### When latency is too high
→ Check for: unnecessary clones, allocations in hot path, await in loops

---

## Discoveries Log

<!-- Ralph adds discoveries here during implementation -->

### 2026-02-10 (EXT-08)
- Solid.js + Tailwind CSS extension rewrite completed
- `esbuild-plugin-solid` handles JSX compilation for esbuild builds
- `vite-plugin-solid` handles JSX compilation for vitest tests
- Tailwind v4 uses `@import "tailwindcss"` in CSS, auto-detects content from project
- `@tailwindcss/cli` for building CSS separately from JS bundle
- When removing `.ts` file and replacing with `.tsx`, must delete the old `.ts` file to avoid TypeScript resolver conflicts
- Background worker stays vanilla TS (no framework needed for service worker)
- Shadow DOM modals require inline CSS styles (Tailwind utilities don't work inside Shadow DOM)
- vitest environment set to `jsdom` for Solid component tests (was `node`)
- `tsconfig.json` needs `jsx: "preserve"` and `jsxImportSource: "solid-js"` for Solid JSX
- Trade payload now sends `management` block instead of `quantity` — backend calculates position size

### 2026-02-11 (EXT-10)
- BinanceFuturesExecutor follows same `real-api` feature flag pattern as spot BinanceExecutor
- Futures API uses `/fapi/v1` and `/fapi/v2` endpoints (not `/api/v3` like spot)
- `positionSide=BOTH` required for one-way mode on Binance Futures
- Rate limit tracking via `X-MBX-USED-WEIGHT-1M` response header, threshold at 90% of 2400
- `AtomicU32` for lock-free rate limit weight tracking across async tasks
- Order amend on Futures uses native `PUT /fapi/v1/order` with cancel+replace fallback
- Mode-aware trade manager: `TradeManagementState` holds `trade_manager_shadow` + `trade_manager_live`
- Route selection via `select_trade_manager(is_authenticated)`: JWT -> live, X-User-Id -> shadow
- Both trade managers subscribe to same PriceFeedService broadcast for price ticks
- Testnet default: `BINANCE_FUTURES_LIVE=true` env var required for production
- `BINANCE_FUTURES_LEVERAGE` env var configures default leverage (default: 1)
- Leverage is set lazily per symbol on first order (avoids unnecessary API calls)
- Amend order IDs use `ORDER_ID:SYMBOL` convention for Binance Futures context passing

### 2026-02-11 (EXT-11)
- Tailwind v4 `@theme` works with custom `--color-*` and `--font-family-*` tokens — generates utilities like `bg-bg-core`, `text-signal-green`, `font-display`, `font-mono`
- WOFF2 fonts from Google Fonts CDN can be downloaded via latin subset URL (smallest file size)
- Chrome Manifest V3 CSP blocks external font loading — must bundle WOFF2 files and reference via relative URL in `@font-face`
- `@font-face` URL paths in CSS are relative to CSS file location in dist, not source
- Global `* { border-radius: 0 !important; }` enforces zero radius across all Tailwind utilities
- Solid.js `createContext`/`useContext` pattern works well for shared auth state across popup views
- Signal-based view router (`"auth" | "main" | "settings"`) is simpler than importing a router library for 3 states
- `browser.storage.local.remove("key")` properly clears individual keys without affecting other stored data

### 2026-02-11 (EXT-12)
- Solid.js `Show` with `fallback` prop is the idiomatic pattern for loading/error states — cleaner than nested conditionals
- `toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })` handles comma separators and decimal places in one call
- `browser.runtime.onMessage` listeners in multiple components (ActiveOrders + MainView) both receive the same `WS_ORDER_UPDATE` broadcast — no conflicts
- Balance fetch reuses existing `GET_BALANCES` handler — zero backend changes needed
- Font size bump across 6 files with zero structural changes — purely visual spec, no logic changes required

### 2026-01-26
- Clippy warnings cleaned up across all crates
- `rust_decimal` already in dependencies

---

*This file grows as Ralph learns. Never delete entries.*
