# Operational Learnings

> Vox's accumulated knowledge. Loaded each iteration.

---

## Codebase Patterns

### Rust Backend

- Exchange status normalization: Use `normalize_status()` in `exchange_api.rs` — maps SDK variants to CCXT strings
- `PlaceOrderResult.status` uses CCXT convention: `"closed"` = filled, `"open"` = resting
- `TradeManagerService::cancel_order(user_id, order_id, symbol, exchange_account_id)` — all fields available from `OrderGroup`

### File Locations

- Hyperliquid exchange API: `crates/router/src/services/hyperliquid/exchange_api.rs`
- Trade management routes: `crates/router/src/routes/trade_management.rs`
- OrderGroup model: `crates/engine/src/shadow/order_group.rs`
- SDK types: `hyperliquid-sdk-rs 0.1.2` — `ExchangeDataStatus` enum in `types/responses.rs`

---

## Anti-Patterns (Don't Do This)

- Don't use `format!("{:?}", ExchangeDataStatus)` for status — Debug format produces strings like `"Filled(FilledOrder { ... })"` that don't match CCXT conventions
- Don't cancel Active groups in cleanup — only Pending ghosts should be purged

---

## Signs (Discoverable Patterns)

### When you see OrderGroups stuck in Pending forever
-> Status normalization bug: `place_order` returned non-CCXT status string, so `is_filled` check failed. Fix: use `normalize_status()`.

### When you see CLEAR ALL not canceling exchange orders
-> `cleanup_stale_trades()` only canceled shadow engine orders. Fix: also call `TradeManagerService::cancel_order()` for entry/SL/TP exchange order IDs.

---

## Discoveries Log

### 2026-03-21 — HL-11-status-transition-fix
- `ExchangeDataStatus` has 6 variants: `Success`, `WaitingForFill`, `WaitingForTrigger`, `Error(String)`, `Resting(RestingOrder)`, `Filled(FilledOrder)`
- `WaitingForFill` existed in SDK but was only referenced in comments
- Extracting `normalize_status()` as a standalone function enables both production use and direct unit testing (DRY)
- `cleanup_stale_trades()` previously used `is_terminal()` filter which let Active groups through — changed to explicit `!= Pending` skip

### 2026-03-21 — UXP-18-multi-theme (Planning)
- Tailwind v4 (extension) handles CSS var opacity natively; v3 (web/journal) requires `rgb(var(--channel) / <alpha-value>)` pattern
- 9 unique opacity modifier usages in web+journal prevent simple `var(--color)` approach in preset
- Extension popup uses DIFFERENT token names than shared preset (e.g., `bg-core` vs `main-bg`) — theme values must be mapped, not copied
- Journal has dual charting libraries: ECharts (registered theme) + lightweight-charts (inline config) — both need dispose+re-init on theme change
- Shadow DOM modal can't inherit `[data-theme]` from outer document — must set attribute on host element at creation
- Journal `app.css` already has 13 `:root` CSS vars — only needs override blocks, not greenfield
- `SpotlightBackground.tsx` uses hardcoded `rgba(5,5,5,...)` — must use `color-mix()` or `rgb(var())` for theming
- RainbowKit `darkTheme()` vs `lightTheme()` — conditional in `main.tsx` based on current theme

### 2026-03-22 — EXT-37-message-dispatch-refactor
- 28 message types in `RuntimeMessageSchema` (spec said 27, but ACCOUNT_LINKED was added since spec draft)
- `ReturnType<typeof RuntimeMessageSchema.parse>` avoids needing `z` import for type inference
- `Extract<ParsedMessage, { type: T }>` utility type (`MsgOf<T>`) cleanly narrows union in handlers
- Handler functions must be declarations (not arrows) to enable hoisting — `debouncedConnectWebSocket` is defined after the handler block
- Pre-existing test failures: `vi.stubGlobal` incompatibility (bun vs vitest) + Playwright runner incompatibility — not related to dispatch refactor

---

*This file grows as Vox learns. Never delete entries.*
