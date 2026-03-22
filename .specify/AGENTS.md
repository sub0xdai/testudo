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

### 2026-03-22 — EXT-38-background-decomposition (Build T1)
- **Module mock scope confirmed**: `vi.mock("webextension-polyfill")` applies to ALL modules that import it, including extracted `background/storage.ts`. No test changes needed for module extraction.
- **Pre-existing test failures (7)**: `EXECUTE_TRADE` missing `break_even_enabled`, token refresh mutex vitest compat, and 5 `ensureActiveExchange` tests using legacy `activeExchangeId` key instead of per-mode `activeCexAccountId`/`activeDexAccountId`. None related to decomposition.
- **Unused imports cleanup**: Extracting `getSettings` to storage.ts made `Settings` type, `DEFAULT_SETTINGS`, `SettingsSchema`, `StoredSettingsSchema` unused in background.ts — removed from imports.

### 2026-03-22 — EXT-38-background-decomposition (Build T2)
- **doRefresh reverted to raw fetch**: Successfully broke auth↔api circular dependency. The refresh endpoint only needs `refresh_token` in body (no JWT header), so raw `fetch` + `getSettings()` is sufficient. Behavior preserved: clears tokens on HTTP error, returns false on network error without clearing.
- **clearRefreshTimer helper**: `handleLogout` directly accessed `refreshTimer` variable. Added `clearRefreshTimer()` export to auth.ts so logout handler doesn't need private state access.
- **Unused imports cleanup (T2)**: Extracting auth functions made `AuthTokens`, `LoginResponse` types, `calculateRefreshDelay` util, `ExchangeMode` type, and 4 schema imports (`AuthTokensSchema`, `StoredTokensSchema`, `JwtEmailPayloadSchema`, `RefreshResponseSchema`) unused in background.ts — all removed. `LoginResponseSchema` stays (used by `authenticate()`).
- **Line reduction**: background.ts reduced by ~70 lines (998→~928). auth.ts is 109 lines.

### 2026-03-22 — EXT-38-background-decomposition (Planning)
- **Circular dep: auth ↔ api**: `doRefresh()` was migrated to `apiRequest()` (commit 27728c2) with `auth: "none"`. Since `apiRequest` calls `refreshAccessToken` on 401, this creates auth → api → auth cycle. Fix: revert `doRefresh` to raw `fetch` + `getSettings()` — refresh endpoint only needs the refresh token in body, not JWT.
- **Circular dep: storage ↔ api**: Spec placed `ensureActiveExchange()` and `migrateActiveExchangeId()` in storage.ts, but both call `listExchangeAccounts()` (api.ts), while `apiRequest()` needs `getSettings()` (storage.ts). Fix: move both to api.ts.
- **`getAuthStatus` misplacement**: Spec placed in storage.ts but it only calls `getTokens()` + `JwtEmailPayloadSchema.safeParse` — pure auth concern. Moved to auth.ts.
- **Hoisting constraint lifts**: EXT-37 required function declarations for hoisting (handlers → `debouncedConnectWebSocket`). In EXT-38, handlers.ts imports from websocket.ts, so arrow functions work fine.
- **Tab cache listeners**: `browser.tabs.onCreated/onRemoved` set `cachedContentTabs = null`. These can run at module import time in websocket.ts since the cache variable is module-scoped — no explicit `init()` function needed.
- **Test compatibility**: `background.test.ts` does `await import("./background")` and captures `onMessage.addListener`. Bootstrap still registers this listener, so tests work unchanged. `_disconnectWebSocket` re-exported from bootstrap.
- **Build compatibility**: esbuild entrypoint `src/background.ts` unchanged. esbuild follows import graph through `src/background/` modules automatically. No config changes.

### 2026-03-22 — EXT-38-background-decomposition (Build T3)
- **Largest extraction**: api.ts is 457 lines — the biggest module, containing all HTTP API wrappers, normalizers, trade execution, and exchange management. Background.ts reduced from 900→481 lines.
- **`authenticate` not re-exported**: `authenticate()` is only used by `login()` and `register()` (both in api.ts), so it doesn't need to be imported back into background.ts. Only `login` and `register` are needed in handler functions.
- **`getExchangeMode` still needed in bootstrap**: `handleGetExchangeMode` and `handleSetExchangeMode` still reference `getExchangeMode` from storage.ts — must keep in storage import even though most exchange logic moved to api.ts.
- **No test changes**: Module mocking continues to work across the new import graph. Same 7 pre-existing failures, 70 passing.

### 2026-03-22 — EXT-38-background-decomposition (Build T4)
- **Sidecar health callback pattern**: `connectWebSocket`'s `ws.onmessage` handler calls `setSidecarStatus()` for sidecar.health stream messages. Since websocket.ts shouldn't depend on sidecar (not yet extracted), used injectable callback: `onSidecarHealth(handler)` setter in websocket.ts, wired by bootstrap via `onSidecarHealth(setSidecarStatus)`. Type-safe: callback accepts `"healthy" | "unreachable"` (subset of `SidecarStatus`).
- **State accessor exports**: `handleWsStatus` needed direct access to `wsState` and `wsReconnectTimer` (module-private in websocket.ts). Added `getWsState()`, `getWsReconnectTimer()`, and `resetReconnectDelay()` exports instead of exposing mutable state.
- **Tab listeners run at import time**: `browser.tabs.onCreated/onRemoved` listeners that invalidate `cachedContentTabs` execute when websocket.ts module is first imported. This is safe because the cache variable is module-scoped and the listeners only null it out. No explicit init function needed.
- **Line reduction**: background.ts reduced from 481→320 lines (-34%). websocket.ts is 175 lines containing 7 exported functions and 7 module-scoped state variables.
- **No test changes**: Same pre-existing failures (vi.stubGlobal compat, Playwright runner, legacy storage keys). Build passes for both Chrome and Firefox targets.

### 2026-03-22 — EXT-38-background-decomposition (Build T5)
- **Simplest extraction**: sidecar.ts is 44 lines — the smallest module. Contains 4 exported functions (`setSidecarStatus`, `getSidecarStatus`, `checkSidecarHealth`, `startSidecarHealthPolling`, `stopSidecarHealthPolling`), the `SidecarStatus` type, and 3 module-scoped state variables.
- **Added `getSidecarStatus()` accessor**: `handleSidecarStatus` previously read `sidecarStatus` directly. Added accessor function to avoid exposing mutable module state, consistent with websocket.ts pattern (`getWsState()`).
- **`apiRequest` removed from bootstrap imports**: With sidecar extraction, `apiRequest` is no longer used directly in background.ts — only in api.ts (internally) and sidecar.ts. Cleaned from imports.
- **Line reduction**: background.ts reduced from 320→286 lines (-11%). Cumulative from monolith: 1043→286 (73% reduction with T1-T5 complete).
- **No test changes**: Same 7 pre-existing failures, 70 passing.

### 2026-03-22 — EXT-38-background-decomposition (Build T6-T8)
- **T6+T7+T8 completed in single pass**: Extracting handlers.ts (T6) naturally reduced background.ts to 61 lines (T7), and build+test validation passed immediately (T8). All three tasks were interdependent — no value in separate iterations.
- **handlers.ts is 228 lines**: Contains all 28 handler functions, the dispatch map, and 3 type definitions (`ParsedMessage`, `MessageHandler`, `MsgOf`). Imports from all 5 extracted modules + `webextension-polyfill` (for `handleSetExchangeMode` and `handleAccountLinked` which use `browser.storage.local.set` directly).
- **Bootstrap at 61 lines**: background.ts now only contains: imports, unhandled rejection listener, onInstalled handler, onMessage listener (using imported `messageHandlers`), startup sequence (migrate → refresh → connect), sidecar health wiring, storage.onChanged listener, and test export.
- **No RuntimeMessageSchema import in handlers.ts**: Used `import type` since handlers.ts only needs the type for `ParsedMessage` derivation, not the runtime value. The actual `safeParse` call stays in bootstrap's `onMessage` listener.
- **Final module sizes**: storage.ts (~55), auth.ts (~109), api.ts (~457), websocket.ts (~175), sidecar.ts (~44), handlers.ts (~228), background.ts (~61). Total: ~1129 lines across 7 files vs original 1043-line monolith — ~8% overhead from module boundaries.
- **No test changes**: Same 7 pre-existing failures, 70 passing. Test captures `onMessage.addListener` from bootstrap — unchanged.
- **Decomposition complete**: All acceptance criteria met. background.ts <100 lines, 6 modules extracted, no circular imports, build passes, tests unchanged.

### 2026-03-22 — bundle-size optimization: Remove Zod from content.js
- **Zod v4 was 85% of content.js bundle**: 304kb out of 358kb. Imported only by `scraper.ts` for 4 trivial schemas (`PositionToolDataSchema`, `TradeSetupSchema`, `ScraperHealthRecordSchema`, `ScraperHealthHistorySchema`).
- **Zod v4 bundles ALL locales**: Unlike v3, Zod v4 includes ~30 locale files (he, ru, ta, th, be, km, ka, uk, hy, bg, ur, ar, mk, fa...) plus JSON schema processors, regex library — none used by scraper's simple `.safeParse()` calls.
- **Plain validators are equivalent**: The 4 schemas only used `.safeParse()` checking positive numbers, string length, enum membership, nullable integers with range. Replaced with ~50 lines of plain TypeScript runtime checks. Identical validation behavior.
- **Result**: content.js dropped from 368,141 → 57,984 bytes (84.3% reduction, -310,157 bytes).
- **No functionality change**: All 6 scraper strategies, telemetry recording, and chart API health detection work identically. Build passes for Chrome and Firefox.
- **Lesson**: Audit ALL transitive dependencies in content scripts. Content scripts run on every page load — even one `import { z } from "zod"` anywhere in the dependency graph pulls the entire library into the bundle. Background scripts (which also use Zod) are unaffected since they load once.

### 2026-03-22 — bundle-size optimization: Remove webextension-polyfill from content scripts
- **webextension-polyfill was 16.5% of content.js bundle**: 9.3kb out of 57.9kb. Imported by both `content.ts` and `scraper.ts`.
- **MV3 makes the polyfill unnecessary for content scripts**: Chrome MV3 `chrome.*` APIs return Promises natively (since Chrome 110+). Firefox MV3 has native `browser.*` namespace. The polyfill's main purpose — wrapping callback-based Chrome APIs in Promises — is redundant in MV3.
- **Content script uses only 4 APIs**: `runtime.sendMessage()`, `runtime.onMessage.addListener()`, `storage.local.get()`, `storage.local.set()`. All Promise-based in both Chrome MV3 and Firefox MV3.
- **Replacement**: Single line `const browser = (globalThis as any).browser ?? (globalThis as any).chrome;` in each file. Firefox uses its native `browser`, Chrome falls back to `chrome`.
- **Result**: content.js dropped from 57,984 → 47,832 bytes (17.5% reduction, -10,152 bytes).
- **Cumulative**: content.js from 368,141 (baseline) → 47,832 (87.0% total reduction).
- **Background script still uses polyfill**: The polyfill remains in background.ts, popup, and other non-content-script modules where its broader API coverage is still used. This change is content-script-only.
- **Lesson**: For content scripts with minimal API surface, prefer native MV3 APIs over polyfills. The polyfill is only needed when targeting MV2 or using many different browser extension APIs.

### 2026-03-22 — css-dedup optimization: Tailwind v4 font variable naming mismatch
- **`@theme` `--font-family-mono` ≠ Tailwind `font-mono`**: The `@theme` block defines `--font-family-mono: "Space Mono", ...` but Tailwind v4's `font-mono` utility maps to `--font-mono` (the Tailwind default: `ui-monospace, SFMono-Regular, Menlo, ...`). Using `font-mono` class would silently change the font from Space Mono to the system monospace stack.
- **Custom utility required**: Created `.font-family-mono { font-family: var(--font-family-mono); }` in `@layer components` to bridge the naming gap. This avoids inline `style={{ "font-family": "var(--font-family-mono)" }}` while preserving the correct font.
- **Root cause**: The popup.css `@theme` uses `--font-family-*` convention (matching CSS spec naming), but Tailwind v4 utilities expect `--font-*` shorthand. The base styles (`body`, `input`, `select`, `button`) reference `--font-family-sans`/`--font-family-mono` directly, so renaming the theme vars would require updating all base style references.

### 2026-03-22 — UXP-19-features-layout
- **Hero + compact list beats terminal log**: The landing page already uses ghost annotations (`// SYSTEM_CAPABILITIES`, `// core_module`) as a terminal metaphor. Adding a second terminal-style feature list (Option B) would create competing metaphors. Option A (hero feature + compact secondary list) adds hierarchy without a new visual language.
- **`border-text-primary` for visual dominance**: Primary feature uses `border-text-primary` instead of `border-container-border` — brighter border creates immediate visual weight difference. Secondary features use `border-l` only (left border, no full box).
- **Ghost annotation as authenticity signal**: The hero block includes `// core_module — position_sizer.rs` referencing the actual Rust backend file. This ties the marketing surface to the real codebase, reinforcing the brutalist "this is real" aesthetic.
- **font-mono preserved intentionally**: UXP-23 spec handles the mono→display typography migration. This spec only changes layout structure to avoid scope creep between specs.

### 2026-03-22 — UXP-22-signal-color-calibration
- **7 source files, 3 surfaces**: Signal colors defined in extension (popup.css, modal.tsx, ArcGauge.tsx), web (index.css, main.tsx), and journal (app.css, tokens.ts). Spec only listed 5 files — journal was missed. Always grep to find all occurrences.
- **Journal tokens.ts has dual fallbacks**: Both `getCSSVarRGB` fallback strings (comma-separated) and `getCSSVarRaw` fallback strings (space-separated) plus hardcoded `rgba()` fallback returns. Three places per color to update.
- **Build artifact lag**: `testudo-journal/dist/` retains old colors in pre-built CSS until journal is rebuilt. Not a source code issue but shows up in grep.
- **RainbowKit accent**: `testudo-web/src/main.tsx` passes signal green as `accentColor` to `darkTheme()` — easy to miss since it's in a config call, not CSS.

### 2026-03-22 — UXP-20-strip-glassmorphism
- **Features.tsx already clean**: UXP-19 layout refactor removed glassmorphism from feature cards before this spec ran. Spec's FR-1 was already satisfied — always verify current state before implementing spec line items.
- **Card.tsx glass variant unused**: The `variant="glass"` option existed but had zero callers. Removed entirely rather than making it match `solid` — dead code elimination over backwards compatibility.
- **4 files, not 5**: Spec listed 5 files but Features.tsx needed no changes. Actual changes: Card.tsx (variant removal), Header.tsx (60%→90% opacity), Hero.tsx (60%→95% opacity), Pricing.tsx (90%→95% opacity).

### 2026-03-22 — UXP-23-landing-typography
- **Pricing feature list items are body copy**: Items like "Risk engine + position sizing" are persuasive descriptions, not data labels — changed to `font-display` even though they're short. The distinction is semantic role (persuading vs displaying data), not word count.
- **Section headings stay mono**: Short headings like `CORE [SYSTEMS]` and `[PRICING]` use terminal bracket notation as intentional aesthetic — these are title elements, not paragraph text. The >20 word acceptance criterion confirms headings don't need changing.
- **5 class replacements across 3 files**: Hero tagline, Features primary + secondary descriptions, Pricing subtitle + feature list. All remaining `font-mono` usages (ghost annotations, price ticker, CTA buttons, feature labels, footer) are intentional terminal-aesthetic or data-display elements.

### 2026-03-22 — UXP-21-light-theme-parity
- **ThemeContext lifted from Header**: Theme state (`getStoredTheme`, `applyTheme`, `cycleTheme`) was duplicated knowledge in Header.tsx. Extracting to ThemeContext.tsx enables any component (including RainbowKit wrapper in main.tsx) to read and react to theme changes. Header.tsx dropped from 130→103 lines.
- **RainbowKitProvider requires wrapper component**: Can't call `useTheme()` inside `createRoot().render()` directly — hooks only work inside components. Created `RainbowKitThemeWrapper` as a thin component between `ThemeProvider` and `RainbowKitProvider` to bridge the gap.
- **Mouse tracking unconditional**: Previously `isLight` guarded the mousemove listener — mouse tracking only ran in dark mode. Removed the guard since both themes now use the spotlight. The `useEffect` no longer depends on `isLight` (changed to `[]` deps), preventing listener teardown/re-attach on theme toggle.
- **Light theme borders via CSS attribute selectors**: `[data-theme="light"] .border { border-width: 2px; }` overrides Tailwind's generated `border-width: 1px` with higher specificity (0-2-0 vs 0-1-0). Covers `.border`, `.border-b`, `.border-l`, `.border-t`, `.border-r`. No component changes needed.
- **Texture grain uses `--text-primary` not `--bg-core`**: Dark scan-lines use `--bg-core` at 15% opacity because the overlay should darken. Light texture uses `--text-primary` at 4% opacity because the grain should add subtle dark marks on the cream background. Using `--bg-core` would be invisible (cream on cream).

---

*This file grows as Vox learns. Never delete entries.*
