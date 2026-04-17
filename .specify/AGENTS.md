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

### 2026-03-22 — JNL-14-markdown-hardening
- **Scrollbar `*` selector hid ALL scrollbars**: The journal used `* { scrollbar-width: none }` which suppressed scrollbars on every element including the editor textarea and preview pane. Moving to `body` selector preserves the hidden page-level scrollbar while letting internal scroll containers show native scrollbars.
- **SolidJS label+hidden-input pattern**: For file upload triggers, `<label><input type="file" class="hidden" onChange={...} /></label>` works without `for`/`id` attributes since the input is a child of the label. Reset `e.currentTarget.value = ''` after upload to allow re-selecting the same file.
- **Journal CSS uses space-separated RGB tokens**: Same `rgb(var(--border))` pattern as extension and web. Image border rule follows existing pattern from `.markdown-preview pre` which already uses `border: 1px solid rgb(var(--border))`.

### 2026-03-22 — JNL-15-export-with-images
- **FileReader for base64 conversion**: `FileReader.readAsDataURL()` produces `data:image/png;base64,...` strings that embed directly into markdown `![alt](data:...)` syntax. Works in all markdown viewers (VS Code, Obsidian, Typora, GitHub).
- **String.replace only replaces first match**: `result.replace(full, ...)` is safe here because each match is processed sequentially from `matchAll` — the full match string includes the unique URL, so duplicates aren't an issue unless the same image URL appears multiple times with the same alt text. If that becomes an issue, would need `replaceAll` or index-based replacement.
- **Async migration is minimal**: Making `exportEntry` async only affects 2 callers. EntryCard's onClick doesn't need await (fire-and-forget is fine since errors are caught internally). EntryEditor awaits for cleanliness but doesn't use the result.
- **Bulk export tagMap from cache**: `JournalTimeline` already loads trade details (including tags) into `tradeDetailCache` as entries render. The `getEntryTags()` helper pulls from this cache, so bulk export doesn't need additional API calls — it just builds the `Record<string, JournalTag[]>` from what's already loaded.

### 2026-03-22 — JNL-16-database-view
- **Client-side sorting is sufficient for <500 entries**: Backend `fetchEntries` only supports `page`/`limit`/`tradeId` query params — no `sort_by`, `sort_dir`, or `entry_type` filtering. Client-side sort on 200-entry fetch (current limit) is imperceptible. Server-side sorting can be added later when entry counts warrant it.
- **Asset column sorts via accessor, not entry field**: `entry.trade_id` maps to a symbol via `tradeDetailCache`. For sort comparisons, the `getTradeLabel` accessor is called per-entry. This is fine for <500 entries but would need denormalization for larger datasets.
- **View toggle lives in JournalTimeline, not Journal.tsx**: Journal.tsx is a 12-line wrapper. Lifting state to Journal.tsx would require passing 10+ props/callbacks. Keeping viewMode inside JournalTimeline alongside existing filter/data state is simpler — no refactor needed.
- **SolidJS `classList` for toggle buttons**: `classList={{ 'bg-text-primary text-main-bg': viewMode() === 'table', 'text-text-tertiary hover:text-text-primary': viewMode() !== 'table' }}` cleanly toggles active/inactive styling. More idiomatic than ternary in `class` string.
- **Markdown stripping for preview column**: Simple regex chain (images → links → formatting chars → newlines) produces clean 80-char previews. No need for a full markdown parser — the preview is intentionally lossy.
- **Removed `rounded` from buttons**: Spec requires "zero-radius, monochrome-first aesthetic". Existing buttons had `rounded` class. Removed from view toggle, export, tags, and new entry buttons to match.

### 2026-03-22 — JNL-17-nested-collections
- **localStorage prototype viable**: No backend `/journal/collections` endpoints exist. localStorage persistence with flat array + client-side `buildTree()` satisfies all CRUD + nesting requirements. Migration to backend requires swapping `readAll()`/`writeAll()` calls in `lib/collections.ts` for API calls — single file change.
- **Collection state in JournalTimeline, not Journal.tsx**: Journal.tsx is a 12-line wrapper. Lifting collection state there would require passing 10+ props. Keeping `activeCollection`, `collections`, `sidebarCollapsed` signals inside JournalTimeline matches the pattern from JNL-16 (viewMode lives there too).
- **Filter bridge is one-directional on select**: Clicking a collection writes its saved `filters` to the existing `typeFilter`, `tagFilter`, `dateFrom`, `dateTo` signals. Manual filter changes after selection still work (they override the collection's preset). "Clear" resets both filters and active collection.
- **Max depth enforcement**: `getDepth()` walks `parent_id` chain. `createCollection()` and `updateCollection()` throw if depth would reach 3. UI hides "+" button at depth 2 via `getCollectionDepth()` check.
- **Sidebar collapse uses vertical text rotation**: When collapsed, sidebar becomes an 8px-wide strip with "Collections" text rotated 90° via `rotate-90` class. This preserves affordance while minimizing space on mobile.
- **"Save Current Filters" in sidebar footer**: Only appears when filters are active (`hasActiveFilters()` check). Auto-generates name from active filter values (e.g., "post-trade + breakout"). Avoids a modal — creates immediately, user can rename inline.

### 2026-03-22 — JNL-18-storage-quotas
- **DB row before file write prevents quota desync**: Insert `journal_images` row first (reserves quota), then write file. On write failure, rollback the DB row. If reversed (file first, then DB), a failed insert leaves an untracked file consuming disk but not counted against quota.
- **`ErrorResponse::with_details` for structured errors**: Quota exceeded errors include `{ used_bytes, quota_bytes, remaining_bytes }` in the `details` field. Frontend `UploadError` class captures this structured data so the UI can show specific numbers without parsing the message string.
- **`UploadError` extends `Error` for type narrowing**: Frontend uses `instanceof UploadError` to distinguish quota errors from generic upload failures. The `code` field matches backend's `error` field (e.g., `"quota_exceeded"`).
- **StorageBar uses `createResource` with refresh key**: Passing `storageRefreshKey()` as the source parameter to `createResource` triggers a re-fetch whenever the key increments. EntryEditor calls `onStorageChange` after successful upload to bump the key.
- **No backfill for existing images**: Existing uploaded images (pre-JNL-18) won't have `journal_images` rows. They're "free" — not counted against quota. This avoids a complex migration scanning the filesystem. New uploads are tracked going forward.
- **Image deletion is best-effort for filesystem**: DB row deletion is the source of truth for quota reclamation. File deletion is wrapped in a non-failing log — if the file was already removed or the path is invalid, quota is still freed.

### 2026-03-24 — AUTH-01-infra-hardening
- **CexSidecarConfig struct literal in tests**: Adding a new field to `CexSidecarConfig` requires updating 3 test struct literals in `cex_client.rs` (test_config_defaults, test_ws_url_conversion × 2). Use `replace_all` for efficiency.
- **Express middleware ordering matters**: PSK middleware (`pskGuard`) must be mounted via `app.use()` BEFORE route handlers. Mounting after `app.get("/health", ...)` would bypass the guard for that route — but the guard explicitly exempts `/health` anyway, so ordering is cosmetic for health but critical for all other routes.
- **Sidecar Dockerfile build context**: Must be monorepo root (`../..` from `docker/` directory), not `testudo-cex/`, because `safe-cex-sub0` is a sibling directory. The `sed` rewrite in Dockerfile changes `file:../safe-cex-sub0` → `file:./vendor/safe-cex-sub0` inside the container.
- **Production compose sidecar service name**: Named `exchange-sidecar` (not `sidecar` or `testudo-cex`) — this is the Docker DNS hostname the router uses via `CCXT_SIDECAR_URL=http://exchange-sidecar:3100`.
- **ws-stream needs frontend network only**: WS-Stream doesn't talk to PostgreSQL directly (it reads from pg_queue via the queue's LISTEN/NOTIFY) — wait, actually it likely needs DB access. Placed on frontend only per spec diagram. May need `internal` too if it queries PG directly — verify in AUTH-02.
- **Migration timestamp convention**: `20260324000000` for wallet migration, `000001` for sessions — SQLx runs in lexicographic filename order, ensuring users table changes land before sessions FK.

### 2026-03-24 — AUTH-02-backend-auth (Build T1)
- **AuthService → TokenService is sync**: `AuthService` trait was async (DB lookups in verify_token). `TokenService` is sync — JWT verification needs no DB access. This eliminates `.await` in trade_management's `extract_user_id()` and trade_events dual auth, simplifying error handling.
- **AuthContext simplified**: Removing `AuthService` from `AuthContext` drops 1 field and 1 generic parameter. All callers updated from `AuthContext::new(user, auth_service.clone())` to `AuthContext::new(user)`. 2 call sites in order.rs, multiple in auth_helpers tests.
- **User model blast radius**: Changing User struct fields (email→wallet_address) touches 9 files across 2 crates: models/user.rs, models/mod.rs, auth/mod.rs, lib.rs (common_utils) + types/auth.rs, repositories/user.rs, utils/validation.rs, routes/exchanges.rs tests, middleware/auth.rs tests (router). Using `replace_all` on `email: "test@example.com"` → `wallet_address: "0x..."` catches most test fixtures.
- **Test count delta**: 960 tests pass (was 978). 18 fewer from removed email/password auth tests (register, login, password hashing, bcrypt-specific). All remaining tests pass clean.
- **Pre-existing warnings not from AUTH-02**: `engine/actor.rs:1814` unused variable, `cex_client.rs:599` useless conversion, `evaluator.rs:188` manual contains. None related to auth changes.
- **bcrypt removal clean**: `bcrypt = "0.15"` removed from common_utils/Cargo.toml. No transitive dependencies on bcrypt remain. `sha2` was already present for crypto operations — reused for `hash_token()`.

### 2026-03-24 — AUTH-02-backend-auth (Build T2)
- **SessionRepository placed in router crate**: Spec said `crates/sqlx_postgres/src/session_repo.rs`, but the actual repo pattern lives in `crates/router/src/repositories/`. Both `PostgresUserRepository` and `ExchangeAccountRepository` are concrete types there (no trait abstraction). The sqlx_postgres crate has an older trait-based pattern that's no longer followed. Placed session.rs alongside user.rs for consistency.
- **AuthError reused, not RepoError**: `PostgresUserRepository` returns `Result<T, AuthError>` (not `RepoError`). SessionRepository follows the same convention since sessions are auth-domain objects. `ExchangeAccountRepository` uses its own `RepoError` because it has domain-specific errors (DuplicateAccount, Conflict, Encryption).
- **cleanup_expired deletes revoked too**: The `cleanup_expired` method removes both expired (`expires_at < NOW()`) and revoked (`is_revoked = TRUE`) sessions. Revoked sessions serve no purpose after the refresh rotation completes — cleaning them up prevents table bloat.
- **No AppState wiring in T2**: SessionRepository is created but not yet added to AppState. T4 (auth routes) will wire it when the routes that consume it are built. Adding it to AppState now would require a placeholder in main.rs construction.

### 2026-03-24 — AUTH-02-backend-auth (Build T3)
- **alloy 0.1.4 Signature API**: The type is `alloy::primitives::Signature` (not `PrimitiveSignature` — renamed in 0.8+). `from_bytes_and_parity(&[u8; 64], bool)` returns `Result<Signature, SignatureError>` (fallible). `recover_address_from_prehash(&B256)` returns `Result<Address, SignatureError>`.
- **`eip191_hash_message` exists in alloy 0.1.4**: Re-exported at `alloy::primitives::eip191_hash_message`. Takes `&[u8]`, returns `B256`. No need for manual keccak256 prefix computation.
- **Ethereum v normalization**: Wallets return v=27/28 (legacy) but alloy expects bool parity (0=false, 1=true). Must normalize: `v=27 → false`, `v=28 → true`, `v=0 → false`, `v=1 → true`. Anything else is invalid.
- **AuthError::InvalidToken is a unit variant**: No payload. For SIWE errors that need context strings, use `AuthError::Unauthorized(String)` instead. `InvalidToken` is for generic JWT validation failures; `Unauthorized(msg)` carries the specific reason.
- **DashMap cleanup-on-insert pattern**: Both NonceStore and PairingStore call `cleanup()` in their `generate()` methods, removing expired entries before inserting new ones. This mirrors the AuthCache pattern in `services/hyperliquid/auth.rs`. No background task needed — cleanup is amortized across insertions.
- **alloy::signers::Signer trait for testing**: `PrivateKeySigner::sign_message()` (async) produces EIP-191 personal signatures compatible with `recover_signer()`. The `Signer` trait must be imported for the method to be available. `signature.as_bytes()` returns the 65-byte `[r(32) + s(32) + v(1)]` representation.
- **No AppState wiring in T3**: NonceStore and PairingStore are created but not yet added to AppState. T4 will add them when building auth routes. This avoids touching main.rs construction prematurely.

### 2026-03-24 — AUTH-02-backend-auth (Build T4)
- **Auth routes use `web::Data<T>` extractors not AppState fields**: NonceStore, PairingStore, SessionRepository, and PostgresUserRepository are injected as `web::Data<T>` — keeps AppState unchanged, enables independent testing of each handler. T5 wires them via `.app_data()` in main.rs.
- **`actix_web::test` import shadows `#[test]` attribute**: When `use actix_web::test` is in scope, `#[test]` resolves to the actix macro (requires `async fn`). Fix: rename import to `use actix_web::test as actix_test` in test modules that also use sync `#[test]`.
- **Cookie builder returns `Cookie<'static>`**: `Cookie::build("name", value.to_string())` requires `.to_string()` on the value (not a borrowed &str) because the cookie must own its data for `'static` lifetime.
- **`Address` display format**: `format!("{recovered:#}")` produces checksummed 0x-prefixed address (e.g., `0xC285...5b36`). Without `#`, produces lowercase non-checksummed. The `#` alternate form matches what users expect from wallet addresses.
- **auth_error_to_response helper**: Routes return `Result<HttpResponse>` (always `Ok`) — auth errors are mapped to appropriate HTTP status codes in the response body rather than using actix's error propagation. This avoids needing `ResponseError` impl for `AuthError`.
- **Shared `rotate_refresh()` for cookie + JSON paths**: Both `/auth/refresh` (cookie) and `/auth/extension-refresh` (JSON body) use the same rotation logic. The only difference is where the refresh token comes from and how new tokens are returned.
- **16 unit tests in auth.rs**: Cookie property tests, error mapping tests, nonce endpoint tests (via actix test server), /me endpoint test (via JwtMiddleware with real token), pairing store tests, UserResponse serialization. All pass without a database connection.

### 2026-03-24 — AUTH-02-backend-auth (Build T5)
- **Nested scope for auth split**: Actix `web::scope("")` (empty path) inside `/auth` scope allows wrapping only authenticated routes with `JwtMiddleware` while leaving public routes (nonce, verify-siwe, refresh, extension-pair, extension-refresh) unwrapped. Routes resolve correctly — `/api/v1/auth/logout` goes through JWT middleware, `/api/v1/auth/nonce` does not.
- **`supports_credentials()` required for `allowed_origin_fn`**: When using `allowed_origin_fn` (dynamic origin checking), `supports_credentials()` must be called explicitly — it's not implied. Without it, browsers won't include cookies in requests even if `withCredentials: true` is set on the client.
- **app_data propagation**: `app_data()` calls on the `/api/v1` scope propagate to all nested scopes (including `/auth` and its nested `web::scope("")`). No need to repeat `.app_data(nonce_store.clone())` inside the auth scope — it inherits from the parent.
- **user.rs deletion clean**: The stub file had no consumers — `routes::user` was never imported in main.rs or any other module. Safe deletion with just `pub mod user` removal from routes/mod.rs.
- **Test count stable**: 905 tests pass (308 common_utils + 216 engine + 11 pg_queue + 451 router + 17 sqlx_postgres + 10 ws-stream), 0 failures. Pre-existing clippy warnings unchanged.

### 2026-03-24 — AUTH-02-backend-auth (Build T6)
- **All tests green with zero intervention**: T1-T5 left the test suite clean — no test fixes needed in T6. 1,013 tests pass across all crates (308 common_utils + 216 engine + 11 pg_queue + 451 router + 17 sqlx_postgres + 10 doc-tests), 0 failures. 23 ignored (expected — doc-test ignores + 1 sqlx integration test).
- **Test count increased from T5 (905→1,013)**: The 108-count jump is from engine tests being counted as both lib and bin test targets (108×2=216). T5's count of 905 was accurate for unique tests; the full `cargo test` run counts both targets. Not a real increase — just counting methodology.
- **Pre-existing clippy warnings stable**: Same 3 warnings as before AUTH-02: `useless_conversion` in cex_client.rs:599, `unused_variables` in actor.rs:1814, `manual_contains` in evaluator.rs:188. None introduced by AUTH-02.
- **AUTH-02 spec complete**: All 6 tasks done. SIWE is the sole auth method, HttpOnly cookies for web/journal, JSON tokens for extension via pairing, refresh rotation with server-side tracking, old email/password code deleted.

### 2026-03-24 — AUTH-03-frontend-auth (Build T1)
- **T1 scope expanded to include consumer fixes**: Changing AuthContext's `login` signature from `(email, password)` to `(user: User)` and removing `register` breaks LoginPage, RegisterPage. Deleting `AuthTokens`/`LoginResponse`/`TokenResponse` types breaks ForgotPasswordPage. Rather than shipping a non-building intermediate, T1 now includes: delete RegisterPage/ForgotPasswordPage, stub LoginPage with ConnectButton, clean App.tsx routes, remove login/register validation schemas.
- **RainbowKit already wired at app root**: `main.tsx` has `WagmiProvider` → `QueryClientProvider` → `RainbowKitProvider` wrapping `AuthProvider` wrapping `App`. The `ConnectButton` component works immediately in LoginPage — no provider changes needed.
- **wagmi config uses Arbitrum only**: `wagmi.ts` has `chains: [arbitrum]`. SIWE message construction (T2) should use `Chain ID: 42161` to match.
- **AccountPage.tsx has dead `isFreshRegistration` state**: Reads `location.state.freshRegistration` from RegisterPage navigation. With RegisterPage deleted, this is always false. Left in place to avoid unnecessary churn — can be cleaned up in T3 when AccountPage is reworked.
- **Axios 401 interceptor simplified dramatically**: Old pattern (163 lines): request interceptor + response interceptor with refresh queue + localStorage reads/writes. New pattern (40 lines): just `withCredentials: true` + single 401 retry with cookie refresh. No queue needed because cookies handle concurrent requests automatically (no per-request token injection).

### 2026-03-24 — AUTH-03-frontend-auth (Build T2)
- **SIWE flow auto-triggers via useEffect**: `useEffect` watches `isConnected && address && siweState === 'idle'` — triggers SIWE immediately after RainbowKit wallet connect. The `siweState` guard prevents re-triggering on re-renders or after errors.
- **Signature rejection detection via regex**: wagmi's `signMessageAsync` throws with various messages across wallet providers ("user rejected", "denied", "cancelled"). Regex `/reject|denied|cancel/i` covers MetaMask, WalletConnect, and Coinbase Wallet. On rejection, wallet is disconnected so user can retry cleanly via ConnectButton.
- **No `useRef` needed for SIWE guard**: Earlier designs used `siweTriggered` ref to prevent double-triggering. The `siweState` signal ('idle' → 'signing' → 'verifying') handles this naturally — `useEffect` only fires when state is 'idle', and `handleSiwe` immediately sets 'signing'.
- **EIP-4361 Chain ID hardcoded to 42161**: Matches `wagmi.ts` config `chains: [arbitrum]`. If multi-chain support is added later, should read from wagmi's `useChainId()` hook instead.
- **Disconnect on error enables clean retry**: After any failure (rejection or backend error), `disconnect()` is called. This resets RainbowKit's ConnectButton to its initial state, so the user can click it again to start fresh. Without disconnect, the button would show "Connected" but auth would be stuck.

### 2026-03-24 — AUTH-03-frontend-auth (Build T3)
- **`isFreshRegistration` was dead code**: With RegisterPage deleted in T1, `location.state.freshRegistration` is never set. Removed the state variable and its two conditional references in the onboarding screen (welcome header + dynamic heading text). Simplifies onboarding to always show "GET STARTED".
- **ExtensionPairing uses interval ref + cleanup**: The countdown timer uses `setInterval` stored in a `useRef` to avoid stale closure issues. `clearTimer` is memoized with `useCallback` and used in `useEffect` cleanup to prevent memory leaks on unmount. The `setCountdown` callback form (`prev => prev - 1`) avoids capturing stale state.
- **authApi.pairExtension() already wired in T1**: The API client already had the `pairExtension` method from T1's rewrite, so no client changes were needed — just the UI component.
- **Pairing card placed after exchange accounts**: The `ExtensionPairing` component sits in its own `<Card>` below the exchange accounts card. This separates concerns (exchange management vs browser pairing) and avoids cluttering the exchange CRUD section.

### 2026-03-24 — AUTH-03-frontend-auth (Build T4)
- **Single function replaces four**: `fetchWithCredentials()` (13 lines) replaces `getToken()`, `refreshAccessToken()`, `refreshPromise`, and `fetchWithRefresh()` (40 lines). The cookie-based approach needs no token storage, no dedup mutex, and no header injection — `credentials: "include"` handles everything.
- **No consumer changes needed**: All 32 files importing from `api/client.ts` use the exported API functions (`fetchTrades`, `fetchOverview`, etc.), not the internal auth helpers. The migration is fully contained within client.ts — zero changes to components.
- **`fetchCrud` keeps Content-Type header**: Even though `Authorization` was removed, `Content-Type: application/json` must stay for POST/PUT/DELETE bodies. The `...init` spread after `headers` lets callers override when needed (e.g., `uploadJournalImage` omits Content-Type for FormData).
- **Upload drops Content-Type intentionally**: `uploadJournalImage` previously set `Authorization` but relied on browser auto-setting `Content-Type: multipart/form-data` with boundary. With cookies, it passes no headers at all — browser handles both cookie and multipart boundary.
- **No localStorage auth references remain**: Grep confirms zero `getToken`, `localStorage`, `Bearer`, `refreshPromise` references in client.ts. Only remaining localStorage usage in journal is `testudo-theme` in Layout.tsx (non-auth, theme preference).

### 2026-03-24 — AUTH-03-frontend-auth (Build T5)
- **`chrome.storage.session` for tokens, `chrome.storage.local` for settings**: Token storage (`getTokens`, `storeTokens`, `clearTokens`) migrated to `browser.storage.session` — tokens auto-clear on browser close (FR-19). Settings, exchange preferences, and active account IDs remain in `browser.storage.local` since they should persist across sessions.
- **Extension uses `/auth/extension-refresh` not `/auth/refresh`**: The cookie-based `/auth/refresh` endpoint expects HttpOnly cookies. Extensions can't send cookies (isolated context), so they use `/auth/extension-refresh` which accepts `{ refresh_token }` in the JSON body and returns new tokens in the response body.
- **`TOKEN_SYNCED_FROM_WEB` fully removed**: Deleted token-sync.ts, removed from manifest.json content_scripts, removed from build.ts IIFE_ENTRIES, removed RuntimeMessageSchema variant, removed handler + dispatch entry, removed 4 tests (2 ensureActiveExchange-via-sync, 2 direct TOKEN_SYNCED tests).
- **JWT claims now `{ sub, wallet_address }` not `{ email }`**: `JwtEmailPayloadSchema` → `JwtWalletPayloadSchema`. `getAuthStatus()` returns `{ authenticated, walletAddress }` instead of `{ authenticated, email }`. Popup AuthContext signal renamed accordingly (`email` → `walletAddress`).
- **`LoginResponse.user.email` → `wallet_address`**: Both type and schema updated to match backend's `UserResponse { id, wallet_address }`. T6 will replace login/register entirely with pairing, but the schema must match the current backend response shape for any remaining callers.
- **Popup consumers updated**: HeaderBar.tsx and MainView.tsx both read `auth.walletAddress()` instead of `auth.email()`. The `data-testid="footer-email"` updated to `"footer-wallet"`.

### 2026-03-24 — AUTH-03-frontend-auth (Build T6)
- **LOGIN/REGISTER/FORGOT_PASSWORD → PAIR**: Three message types replaced by single `PAIR: { code: string.length(6) }`. `LoginResponseSchema` renamed to `PairResponseSchema` (same shape — backend `/auth/extension-pair` returns `{ tokens, user }` identically). `LoginResponse` type deleted from types.ts (was unused).
- **`authenticate()`/`login()`/`register()`/`forgotPassword()` → `pair()`**: api.ts auth wrappers collapsed to single function. `pair(code)` POSTs to `/api/v1/auth/extension-pair` with `{ code }`, parses `PairResponseSchema`, stores tokens + schedules refresh. Same pattern as old `authenticate()` but simpler (no email/password).
- **AuthContext: 3 methods → 1**: Popup AuthContext dropped `login()` and `register()`, replaced by `pair(code)`. Same message-passing pattern (`browser.runtime.sendMessage({ type: "PAIR", code })`), same success flow (`checkAuth()` on success).
- **PairView.tsx replaces AuthSection.tsx**: Same glass card layout/styling, but email+password inputs replaced by single 6-digit numeric code input with centered monospace digits. Instructions direct user to web app account settings for code generation. AuthSection.tsx retained in tree but no longer imported.
- **Test mock needed `storage.session`**: T5 migrated auth to `browser.storage.session` but test mock only had `storage.local`. Added `makeStorageArea()` factory + `session` property to mock. Also added `sessionStorage()` test helper. Fixed LOGOUT assertion to check `session.remove` instead of `local.remove`.
- **Pre-existing test failures unchanged**: 5 remaining failures are pre-existing from EXT-38 era (EXECUTE_TRADE `break_even_enabled`, token refresh mutex vitest compat, ensureActiveExchange legacy storage keys). Not caused by T6.

### 2026-03-24 — AUTH-03-frontend-auth (Build T7 — Validation)
- **All three frontends build clean**: `bun run build` passes for testudo-web (tsc + vite, 11.6s), testudo-extension (esbuild Chrome+Firefox, <1s), testudo-journal (vite, 5.8s). No compilation errors.
- **14/14 acceptance criteria verified**: No email/password UI, no RegisterPage, no localStorage tokens, cookie-based auth with /auth/me, extension pairing flow, token-sync deleted, journal credentials: "include", extension chrome.storage.session — all confirmed via code inspection.
- **AUTH-03 spec complete**: All 7 tasks (T1-T7) done. Three frontends migrated from email/password + localStorage JWT to wallet-connect SIWE + HttpOnly cookies (web/journal) and device pairing + chrome.storage.session (extension).

### 2026-03-24 — EXT-39-pair-ux (Build T1)
- **OTP refs array pattern in Solid.js**: `let refs: HTMLInputElement[] = []` with `ref={(el) => (refs[i] = el)}` in JSX. Unlike React's `useRef`, Solid's `ref` callback assigns directly to the array slot during render. No `createSignal` or `createEffect` needed for ref management.
- **requestAnimationFrame for popup auto-focus**: `onMount(() => refs[0]?.focus())` may fire before the popup DOM is painted in Chrome extension context. Wrapping in `requestAnimationFrame` ensures the input element is visible and focusable.
- **Paste handler on container, not individual inputs**: `onPaste` is attached to the flex container `div` wrapping all six OTP boxes. This captures paste events regardless of which box is focused, and `e.preventDefault()` stops the browser from inserting text into the focused input.
- **Session expired detection via stored popupView**: `browser.storage.local.get("popupView")` persists which view the user was on. If auth check returns false but stored view is "main", the session expired between popup opens. Explicit logout sets `popupView: "auth"` first, so no false positive.
- **OTP box CSS needs light theme overrides**: Dark theme uses `rgba(255,255,255,0.08)` borders, but light theme needs `rgba(0,0,0,0.12)`. Added `[data-theme="light"] .otp-box` selectors in `@layer components` for border-color, focus, and placeholder colors.
- **Popup bundle size unchanged**: popup.js stayed at 83.4kb after adding OTP component — no new dependencies, just restructured DOM and event handlers.

### 2026-03-24 — EXT-39-pair-ux (Build T2 — Validation)
- **All 18 acceptance criteria verified via code inspection**: Six OTP boxes, single-digit enforcement, auto-advance, backspace navigation, paste auto-submit, no manual auto-submit, Enter key submit, button disabled state, auto-focus with rAF, underscore placeholder, numbered instructions, dynamic settings URL, loading spinner + disabled inputs, red error text, success checkmark with 800ms delay, expiry hint, session expired banner, Chrome+Firefox build — all confirmed.
- **Build sizes stable**: popup.js 83.4kb, content.js 47.1kb, background.js 333.2kb — identical for Chrome and Firefox. No bundle size regression from T1.
- **EXT-39 spec complete**: Both tasks done. PairView upgraded from single text input to six-box OTP with full state feedback. No new dependencies added.

### 2026-03-24 — EXT-40-smart-card-grid (Build T1)
- **KebabMenu confirmation inline in dropdown**: Rather than replacing the entire dropdown content on confirm, each destructive action (delete, revoke) independently toggles to a CONFIRM/NO row via `confirmAction` state. This lets the user confirm one action without losing access to other menu items.
- **`isDeleting`/`isRevoking` props unused in ExchangeCard**: These props exist on the interface for T3 wiring (AccountPage tracks which account is mid-delete/revoke for async state), but T1's KebabMenu handles confirmation locally. The kebab closes on confirm click, so the card doesn't need to show "DELETING..." state — the parent's `handleDelete` async updates the list.
- **`auth_mode` determines CEX/DEX badge, not `ExchangeInfo.type`**: The spec suggested using `ExchangeInfo.type` for the badge, but the card only receives `ExchangeAccount` props (no join with exchanges list). `auth_mode === 'agent_wallet'` reliably indicates DEX (Hyperliquid) vs CEX. Simpler than threading `ExchangeInfo` through props.
- **Test result display shows latency OR error**: Successful test shows `{latency_ms}ms` in green; failed test shows `{message}` in red at smaller font size. No test result shows `---` as balance placeholder. This dual-purpose area serves as balance placeholder until a future `fetchBalance` integration.
- **HTML entity `&#x22EE;` for kebab icon**: Using the Unicode vertical ellipsis character directly avoids SVG overhead and matches the spec's `⋮` requirement. Renders consistently across browsers.

### 2026-03-24 — EXT-40-smart-card-grid (Build T2)
- **AddExchangeCard is stateless**: Pure presentational component — single `onClick` prop, no internal state. The ghost card pattern (dashed border, "+" icon, hover color shift) matches the spec exactly. 12 lines total.
- **ExtensionPairingBanner duplicates logic from ExtensionPairing**: New component rather than variant prop on existing `ExtensionPairing.tsx` — keeps SoC clean since the banner layout (horizontal flex, inline code display, condensed padding) is structurally different from the card layout (vertical stack, large code block). Both will coexist until T3 swaps the AccountPage import.
- **Banner code display inline with title**: When a code is active, the banner shows title + code + countdown + "NEW CODE" button in a single flex row. This is much more compact than the original vertical layout with its centered 4xl code block. `flex-wrap` handles narrow viewports gracefully.

### 2026-03-24 — EXT-40-smart-card-grid (Build T3)
- **Balance fetch is fire-and-forget per card**: After `fetchData()` resolves accounts, each account's balance is fetched individually with `.catch(() => {})`. Cards render immediately with "---", then update as balances arrive. No loading spinner for balances — keeps initial paint fast.
- **`formatBalance` prioritizes USDT/USDC**: The helper scans `balances[]` for USDT or USDC first (the primary trading assets), falling back to `balances[0]`. Formats as `$X,XXX.XX` with locale-aware number formatting.
- **Balance and test result now coexist**: T1's ExchangeCard showed test result OR "---" in a single area. T3 separates: balance is the large primary display, test result appears as a small annotation below. Both can be visible simultaneously.
- **`handleMigrate` opens form with exchange pre-selected**: Instead of navigating or showing a modal, migration reuses the existing form by setting `formExchange` to `'hyperliquid'` and `showForm` to `true`. Same pattern as the old inline migration prompt but triggered from the card's kebab menu.
- **Form section is full-width below grid, not inside a card**: The add-exchange form spans the full container width below the grid with its own CANCEL button. This avoids grid alignment issues from trying to expand one grid cell.
- **`max-w-5xl` replaces `max-w-2xl`**: Only on the normal management return path. Onboarding and setupComplete screens retain `max-w-2xl` since they don't need grid width.
- **Old `ExtensionPairing` import replaced by `ExtensionPairingBanner`**: The card-wrapped vertical ExtensionPairing component is no longer used on AccountPage. The compact banner renders directly via `<ExtensionPairingBanner />` with its own border-top separator.

### 2026-03-24 — EXT-40-smart-card-grid (Build T4 — Validation)
- **All 13 acceptance criteria verified**: Responsive grid (1/2/3 cols), heartbeat dots (pulsing green / static red), kebab menu with TEST/DELETE/REVOKE, click-outside-close, signal-red destructive styling, ghost card with dashed border, form toggle on ghost click, compact extension pairing banner, max-w-5xl container, all existing functionality preserved, design tokens only (no raw colors), bun run build passes.
- **AccountPage reduced from 532→438 lines**: Grid layout is more concise than the old stacked-row approach despite adding balance fetching logic. The form section is cleaner with its own CANCEL button below the grid.
- **EXT-40 spec complete**: All 4 tasks done. Account management UI redesigned from stacked rows to responsive card grid with heartbeat indicators, kebab menus, ghost card, and compact pairing banner. No new dependencies added.

### 2026-03-29 — EXT-43-main-world-bridge
- **Separate esbuild entry for bridge**: `page-bridge.ts` is bundled as a third IIFE build (no Solid plugin, no sourcemap needed since it runs in page context). Build output: 2.4kb minified.
- **Bridge IIFE self-executes**: Wrapped in `(function() { ... })()` — runs immediately when injected via `<script>` tag. No exports, no module system.
- **`isChartPlatform()` replaces `isTradingView()` for symbol-only guard**: The existing symbol-only fallback was gated on `isTradingView()`. Changed to `isChartPlatform()` which includes tradingview.com, dexscreener.com, hyperliquid, gmx.io, bybit.com — prevents spurious modal opens on random websites while enabling multi-platform support.
- **Bridge tried before DOM strategies**: In the Alt+X handler, bridge `getPositionTool` is attempted first (async), falling back to synchronous DOM scraper strategies. This prioritizes zero-dialog extraction over dialog-dependent strategies.
- **content.js bundle size +0.4kb**: From 47.8kb to 48.2kb — bridge injection code is minimal (postMessage listener, inject function, bridgeRequest helper with timeout).
- **No changes to scraper.ts**: Existing 6-strategy scraper is completely untouched. Bridge is additive — runs as a pre-strategy step in content.ts, not inserted into the strategy array.

### 2026-03-29 — EXT-44-hyperliquid-support
- **FR-7 was pre-satisfied by EXT-43**: `isChartPlatform()` already included `host.includes("hyperliquid")`, so bridge injection worked on Hyperliquid without any changes. The `isHyperliquid()` function was added for readability and used to replace the inline check.
- **Leaf div walk is the only viable selector strategy**: Hyperliquid uses styled-components with hash classes (`sc-bjfHbI`, `bFBYgR`) that change every build. Text-content matching via regex (`/^[A-Z0-9]{2,10}-USDC$/`) on childless divs is the only stable approach.
- **Symbol format conversion is a simple hyphen strip**: `BTC-USDC` → `BTCUSDC`. No exchange prefix stripping or perpetual suffix handling needed (unlike TradingView's `BINANCE:BTCUSDT.P`).
- **DOM strategy [2] won't work on Hyperliquid**: Content script runs in isolated world — `findPositionToolByChartApi()` (strategy 2) can't access `window.TradingViewApi`. The bridge handles this instead. On non-TradingView sites, the DOM fallback path tries `[2]` but silently returns null.
- **content.js bundle size +0.8kb**: From 48.2kb to 48.6kb — Hyperliquid scraper function and platform detection are minimal additions.
- **Manifest uses exact domain**: `*://app.hyperliquid.xyz/*` not `*://*.hyperliquid.xyz/*` — the trading app is only on the `app` subdomain. This minimizes permission scope.

### 2026-03-29 — EXT-45-dexscreener-symbols
- **FR-5 pre-satisfied by EXT-43**: `isChartPlatform()` already included `dexscreener.com`, so bridge injection worked without changes. Same pattern as EXT-44 where FR-7 was pre-satisfied.
- **`isDexScreener()` added for readability**: Not strictly needed (bridge injection uses `isChartPlatform()`), but follows the `isHyperliquid()` pattern for platform-specific scraper guards and future use in content.ts.
- **4-strategy fallback for DexScreener symbols**: (1) TradingView legend selectors — DexScreener embeds charting lib directly, so `[data-name="legend-source-item"]` may match; (2) title parens — `(SYMBOL)` in `document.title`; (3) title slash — `TOKEN / SOL` in title; (4) leaf element scan — `XXX / YYY` pattern in childless spans/divs/anchors.
- **DexScreener check before generic SYMBOL_SELECTORS**: Same cascade pattern as Hyperliquid — platform-specific scraper runs first in `scrapeSymbol()`, falling through to TradingView-targeted selectors only if platform check fails.
- **No manifest changes needed**: DexScreener was already in `host_permissions` and `content_scripts.matches` from initial manifest setup. Unlike EXT-44 (Hyperliquid), no permission additions required.
- **content.js bundle size +0.7kb**: From 48.6kb to 49.3kb — DexScreener scraper function is ~35 lines.

### 2026-03-29 — EXT-46-async-scraper-flow
- **`strategiesToTry` was the root bug**: The existing code passed `isTradingView() ? undefined : [2]` to `scrapeTradeSetup()`, meaning non-TV sites tried only Strategy 2 (chart API via `findPositionToolByChartApi()`). Strategy 2 accesses `window.TradingViewApi` which is inaccessible from the content script's isolated world. With the bridge (EXT-43) handling chart API access in main world, the DOM fallback should only run on TradingView where strategies 0-5 actually work.
- **`scrapeTimeframe()` was unexported**: Bridge-sourced setups on TradingView should get the real timeframe (e.g., "4h", "1D"), not a generic "chart" label. Added `export` to the existing function — zero behavioral change, just visibility.
- **`isChartPlatform()` vs `bridgeReady` for outer guard**: `bridgeRequest()` already returns null when bridge isn't ready (internal `bridgeReady` check at line 59). Using `isChartPlatform()` as the outer guard is semantically clearer and doesn't add latency — the promise resolves immediately if bridge isn't ready.
- **Three-step cascade is cleaner**: Bridge (async, all platforms) → DOM strategies (sync, TradingView only) → symbol-only (sync, all chart platforms). Each step has a clear scope and no wasted work. Previous code attempted DOM strategies on non-TV sites where they could never succeed.
- **content.js bundle unchanged at 49.3kb**: Code reorganization is length-neutral — removed `strategiesToTry` logic, added `scrapeTimeframe` import and conditional.
- **Completes EXT-43→46 series**: This spec ties together the main-world bridge (EXT-43), Hyperliquid platform detection (EXT-44), DexScreener symbol extraction (EXT-45), and the revised async scraper flow into a coherent multi-platform Alt+X experience.

### 2026-03-31 — REL-02-hl-journal-pipeline
- **HL trigger orders never match FillDetector OID gate**: WaitingForTrigger orders return `"cloid:..."` as OID at placement, but the actual exchange assigns a numeric OID when the trigger fires. FillDetector's `get_group_by_exchange_order(oid)` → None → dropped. Neither Python nor Rust HL SDKs solve this. Industry pattern: match on `closedPnl != "0"`, not order ID.
- **Shared `build_trade_close_event()` extracted to `hl_fill_journal.rs`**: Both `import_worker` (batch) and `ws_fills` (live 30s poll) now use the same conversion logic. Source field distinguishes: `"import_hl"` vs `"live_poll"`.
- **Borrow checker pattern for `&mut self` + `Arc` in async loop**: When iterating fills and calling `self.record_tid()` (mutable borrow), can't hold `self.journal.as_ref()` (immutable borrow) across the loop. Fix: clone `Arc<JournalService>`, copy `Uuid`, clone `Option<PgPool>` into locals before the loop.
- **`journal_service` creation moved earlier in main.rs**: Was after WsSubscriptionManager creation. Moved before it so `with_journal()` builder can wire it into the manager.
- **`open_times` HashMap seeded from startup reconciliation**: 24h REST lookback populates `coin → timestamp` from `fill.dir.starts_with("Open")` fills. Subsequent 30s polls also update it. Used as `open_time_ms` parameter to `build_trade_close_event()` for accurate duration.
- **Dedup layer: `seen_tids` (in-memory HashSet) + DB unique index**: `seen_tids` prevents redundant DB writes within a process lifetime. `idx_unique_import_fill` on `(user_id, exchange, exchange_fill_id)` catches cross-process duplicates (poller vs import worker).

### 2026-03-31 — REL-03-hl-group-reconciliation
- **`reconcile_group` is a static async method**: Takes explicit params (`engine_handle`, `exchange_api`, `user_id`, etc.) instead of `&self`. This avoids borrow checker issues — the method is called inside the `for fill in &fills` loop where `self.record_tid()` already holds a mutable borrow.
- **Symbol format mismatch between HL fills and OrderGroups**: HL fills use bare coin name ("BTC"), OrderGroups use "BTC_USDT" format. The reconciler matches both with `g.symbol == symbol_usdt || g.symbol == symbol`.
- **`fill.dir` carries close direction**: Values like "Close Long", "Close Short" — used to determine terminal status (StoppedOut vs TookProfit) by comparing exit_price against group entry_price.
- **Dual pg_notify path**: When REL-03 engine_handle is present, `reconcile_group` emits pg_notify with specific event_type ("stopped_out"/"took_profit") and group_id. When absent (REL-02-only fallback), original generic "trade_closed" notify fires.
- **Sibling cancel cancels all 3 exchange order IDs**: entry, SL, and TP — the filled one returns OrderNotFound (no-op), the others get cancelled. Same pattern as `fill_detector.rs:cancel_all_related_orders()`.

### 2026-04-01 — CON-01a-daily-stats-regression
- **TradeManagementState lacks pool**: Needed `pool: Option<PgPool>` field with `with_pool()` builder to resolve exchange_name from exchange_account_id during trade placement. Tests use `new()` which defaults to None — backward compatible.
- **RehydrationService needed pool for batch exchange_name lookup**: Added `pool: PgPool` to constructor. Uses `SELECT id, exchange_name FROM exchange_accounts WHERE id = ANY($1)` to batch-fetch exchange names for all rehydrated positions. Groups get exchange_name set post-build before engine load.
- **EngineCommand::ConfigureGroup extended**: Added `exchange_name: Option<String>` field. Handler sets it on the group alongside `exchange_account_id`. Only one call site in trade_management.rs.
- **parse_trade_close_payload default changed**: From `"cex"` to `"unknown"` for missing exchange field. Consistent with fill_detector's `group.exchange_name.as_deref().unwrap_or("unknown")` fallback.
- **Daily stats upsert after tx.commit() is correct**: The TradeEventWriter's transaction covers trade_events + managed_positions + journal_trades atomically. Daily stats are post-commit fire-and-forget because they're recomputable from journal_trades. Same SQL as JournalService::upsert_daily_stats — no abstraction needed, just copy the queries.
- **Draft notes merge pattern**: DELETE FROM journal_trade_drafts RETURNING notes → UPDATE journal_trades SET notes WHERE trade_group_id. Uses pool (not transaction) since it's post-commit. Same pattern as JournalService lines 167-191.

### 2026-04-01 — UXA-01-agent-wallet-visibility
- **`ExchangeAccountResponse` constructed in 3 places**: `get_user_exchange_accounts()` (exchanges.rs:150), `add_exchange_account()` (exchanges.rs:260), and test in `types/exchanges.rs:275`. All three need `requires_reauthorization` field when adding new fields.
- **`CexExchangeApi::load_credentials` also uses `list_by_user()`**: When modifying `list_by_user()` to include inactive accounts, CexExchangeApi's fallback path (no explicit account ID) now needs to filter to active accounts explicitly, otherwise it could pick an inactive agent wallet as the "first" account.
- **`error_code_for()` parallel to `format_exchange_error()`**: Both functions match on the same `ExchangeApiError` enum. Keeping them separate (not merging into a single return struct) preserves backward compatibility — `format_exchange_error` is used in warning strings too, not just API responses.
- **`ApiResponse` Deserialize requires `error_code` optional**: Since `ApiResponse` derives both Serialize and Deserialize, and existing JSON from clients/tests won't have `error_code`, it must be `Option<String>` with `skip_serializing_if` and default None for deserialization.

### 2026-04-01 — UXA-02-desk-reauth-ux
- **Account.tsx API key `type="text"` security leak**: The inline add-exchange form in Account.tsx used `type="text"` for the API key input while OnboardingFlow used `type="password"`. Extracting `AddExchangeForm` as a shared component fixed this by using `type="password"` for all credential fields.
- **WalletConnectFlow step progress adapts to re-auth mode**: Normal flow has 4 steps (Connect, Initialize, Sign, Approve). Re-auth skips Initialize, so step labels and indices are dynamically computed based on `isReauth()` flag. The `init-agent` step in re-auth mode maps to index 0 (same as idle) since it's just fetching approve-data, not generating a keypair.
- **OnboardingFlow simplified to binary state**: Previously used a 4-step state machine (`select`, `credentials`, `submitting`, `success`) with its own form rendering. After extracting `AddExchangeForm`, OnboardingFlow only tracks `success` boolean — all form logic lives in the shared component.
- **Account.tsx had 6 dead form signals**: `showWalletConnect`, `formApiKey`, `formSecret`, `formPassphrase`, `formSubmitting`, `needsPassphrase` were all replaced by `AddExchangeForm`'s internal state. Only `formInitialExchange` signal needed (to pre-select exchange for migration flow).
- **`AddExchangeForm.initialExchange` prop enables migration pre-selection**: When "Migrate to agent wallet" is clicked on a direct-key Hyperliquid card, Account.tsx opens the form with `initialExchange='hyperliquid'`, auto-selecting the WalletConnectFlow path.

### 2026-04-01 — UXA-03-extension-error-recovery
- **`error_code` flows through 4 layers**: `ErrorResponseSchema` (apiRequest HTTP error path) → `ApiResult` type → `normalizeBackendAck` (HTTP 200 logical error path) → `BackendResponse` type+schema → content.ts response cast. All four must carry `error_code` for end-to-end propagation.
- **`DESK_URL` already includes `/desk` suffix**: Value is `http://localhost:3002/desk`, so account link is `${DESK_URL}/#/account`, NOT `${DESK_URL}/desk/#/account` as the spec template suggested. Fallback: `https://testudo.vip/desk/#/account`.
- **Banner uses same Shadow DOM pattern as toasts**: Each banner gets its own shadow host (`testudo-sniper-banner`), reusing `TOAST_STYLES` (which includes `TOAST_CSS` + theme vars). Only one banner active at a time (tracked via `activeBanner` module var) — showing a new one removes the old.
- **content.ts safely imports from utils.ts**: `DESK_URL` is a plain string constant. `utils.ts` imports only `type { Settings }` which tree-shakes. No Zod or webextension-polyfill pulled into content bundle. content.js grew 2.9kb (49.3→52.2kb).

### 2026-04-17 — RSK-01 T1 (Backend types + route stub)
- **`rust_decimal` serializes as string by default**: With `rust_decimal = "1.36"` and no explicit serde config, `Decimal` fields serialize as JSON strings (e.g., `"0"`, `"3.5"`). Matches the spec's "decimal as string (convention)" and the existing `ExchangeBalanceEntry` pattern. No `#[serde(with = "rust_decimal::serde::str")]` needed.
- **Route-scope registration pattern**: New route scopes inside `/api/v1` follow the existing `/risk-config` shape — `web::scope("/foo").wrap(JwtMiddleware::new(token_service.clone())).route(...)`. The `token_service.clone()` is crucial; `JwtMiddleware` takes ownership.
- **Service-layer types live in the service file**: `risk_config.rs` keeps `RiskConfigResponse` + `ErrorResponse` next to handlers rather than in `types/`. Followed that convention — `RiskSnapshot`, `VenuePositions`, `VenueMargin`, `CorrelationBucket`, `PositionEntry`, `RiskError` all live in `services/risk_snapshot.rs`.
- **`AuthenticatedUser` post-AUTH-02**: The extractor now yields `{ user_id: Uuid, wallet_address: String }` (not `email`). Always use `user.user_id` for DB lookups.
- **Stubbed service returns `Ok(zeroed)` for T1**: The spec's vertical slicing calls for a wire-contract task (T1) that the frontend can mock against before real aggregation (T2). `build_snapshot` is async from day one so T2's DB fan-out requires zero signature churn.

### 2026-04-17 — RSK-01 T2 (Backend aggregation logic)
- **`list_by_user` includes inactive agent wallets, but `load_credentials` filters to active**: `list_by_user` returns rows where `is_active = true OR (auth_mode = 'agent_wallet' AND is_active = false)` so the UI can flag re-auth needs, but `load_credentials` enforces `is_active = true`. The snapshot service skips inactive accounts before fan-out — calling `load_credentials` on them returns `RepoError::NotFound` and would just log noise.
- **`OnceLock<DashMap<Uuid, _>>` is the lighter-weight cache singleton**: `lazy_static` is in Cargo.toml but `std::sync::OnceLock<DashMap>` plus a `cache()` accessor (`SNAPSHOT_CACHE.get_or_init(DashMap::new)`) needs no macro and gives the same lazy-init semantics. Used for the 5s TTL snapshot cache.
- **Mark-price approximation for CEX positions**: The CEX sidecar's `SidecarPositionResponse` has `symbol/side/contracts/entry_price/unrealized_pnl` — no mark price. Compute it as `entry_price ± (unrealized_pnl / contracts)` based on side. HL exposes `markPx` and `positionValue` directly in `clearinghouseState.assetPositions[].position` — use them when available.
- **Stablecoin-only USD-equivalent margin sum**: `fetch_cex_margin` sums entries whose `asset` is in `["USDT","USDC","USD","BUSD","DAI","TUSD","FDUSD"]`. Non-stable assets (BTC/ETH spot balances) are intentionally excluded — futures margin is always stable-denominated, and treating volatile balances as USD-equivalent would lie about free margin.
- **Async closures in `iter().map(|acc| async move { … })`**: With `app_state: &AppState` captured into the future, `tokio::join!` per-account works without `Box::pin`. `futures_util::future::join_all` collects them. Keep the per-account `acc_id`/`exchange_name` clone outside the `async move` block so the future doesn't borrow from `acc` across the `.await`.
- **Symbol → base-asset extraction**: Handles `BTC`, `BTC/USDT`, `BTC/USDT:USDT`, `BTC_USDT`, `BTC-USDT`, `BTCUSDT`. Order matters — separator-based splitting first, then suffix stripping. `BTCUSDT` only strips the `USDT` suffix; `BTC` (no suffix) returns as-is.
- **9 unit tests cover bucket map, base-asset extraction, correlation aggregation, cache invalidation, and JSON shape**: Integration tests against a real `AppState` are deferred to T3 (which needs stub `ExchangeApi` adapters per the plan). The pure helpers (`bucket_for`, `extract_base_asset`, `build_correlation_stack`) are fully testable without DB or HTTP.
- **`(Vec<_>, Vec<_>)` from `Iterator::unzip()`**: Cleaner than two `.iter().map().collect()` calls when the per-account fan-out returns `Option<(VenueMargin, VenuePositions)>` — `flatten()` drops the failed accounts, then `unzip()` splits margin/positions.

### 2026-04-17 — RSK-01 T3 (Backend aggregation tests)
- **Router is a binary-only crate**: No `src/lib.rs`, no `[[lib]]` target in `Cargo.toml`. A top-level `tests/*.rs` integration file can't `use router::services::…` because there's no library target to link against. The project's established pattern is inline `#[cfg(test)] mod tests` inside each module — `services/hyperliquid/tests/` is the only exception and lives as a submodule too.
- **Extract pure aggregation for testability**: Splitting `build_snapshot` into `(fan-out) → (aggregate_snapshot pure fn) → (cache_put)` isolates the math from HTTP/DB dependencies. Tests construct `Vec<VenuePositions>` + `Vec<VenueMargin>` fixtures directly and assert on the returned `RiskSnapshot`. No mock `AppState` or `CexClient` needed.
- **Spec's "stub ExchangeApi adapters" hint was wrong**: The `ExchangeApi` trait in `services/exchange_api.rs` is for order placement/amendment/cancellation — not balance or positions. `build_snapshot` uses `cex_client` + `hl_http_client` directly, not through `ExchangeApi`. Refactoring to a balance+positions trait would be a cross-cutting change out of T3 scope.
- **`dec!(0.6)` exact for 12000/20000**: `rust_decimal` division is exact for these fixture values, so `assert_eq!(aggregate_leverage, dec!(0.6))` passes without precision tolerance. Same for `0.5` long/short split — exact in decimal.
- **`margin_by_venue` sort is a contract**: Tests assert ordering (`hyperliquid` before `bybit` when hl has more free margin) — FR-3 calls for "sorted descending" and the frontend widget relies on it. Verifying order in tests prevents regression if future refactors drop the `sort_by`.

### 2026-04-17 — RSK-01 T4 (Frontend types + API client)
- **`rust_decimal` → string convention is universal in this client**: Existing interfaces (`AccountStats`, `EquityPoint`, `ExchangeBalanceEntry`, `JournalTrade`) all type Decimal fields as `string`. The `formatters.ts` helpers `parseFloat` at render time. RiskSnapshot follows the same pattern — `net_exposure_usd: string`, `aggregate_leverage: string`, etc.
- **`Uuid` → `string` in payloads**: The backend's `exchange_id: Uuid` serializes to a plain string via serde. The frontend already types `ExchangeAccount.id: string`, so `VenuePositions.exchange_id: string` matches the existing convention.
- **`DateTime<Utc>` → ISO string**: `chrono` serializes to RFC3339 strings by default. Same pattern as `JournalEntry.created_at: string`, `ExchangeAccount.created_at: string`.
- **Narrowed union types for `side` / `direction`**: Used `'long' | 'short'` and `'long' | 'short' | 'mixed'` literal unions. Backend returns these as plain `String`, but the constraints are enforced by the aggregator code (`build_correlation_stack` hard-codes the values). Typing them narrowly on the frontend enables exhaustive switches in T7/T8 widgets.
- **`fetchWithCredentials` already handles 401 + refresh**: The existing helper (cookie-based) is reused — no new auth path. `fetchRiskSnapshot` is a one-liner that throws on non-2xx (matches `fetchOverview` etc.).

### 2026-04-17 — RSK-01 T5 (LiveRiskStrip + PulseStrip components — mock data)
- **`formatPercent` expects percent-scale input, not 0..1**: `formatPercent(num)` returns `${num.toFixed(1)}%`. The backend emits `long_pct` as a 0..1 fraction per the type contract, so callers must multiply by 100 before passing in. Failing to do so yields `"0.6%"` for a 60/40 split.
- **`formatCurrency` always prefixes sign**: Returns `"+$1,000.00"` for positive, `"-$1,000.00"` for negative. For UI fields that are semantically absolute (NET EXPOSURE, FREE MARGIN), strip the leading `+` with `.replace(/^\+/, '')`. Never strip the `-` — we want negative values to show.
- **Signal-green pulse indicator is the "live" convention**: `WalletChip` and `ExchangeCard` both use `inline-block w-2 h-2 rounded-full bg-signal-green animate-pulse`. PulseStrip uses a slightly smaller `w-1.5 h-1.5` variant to fit the ≤32px row. Amber without `animate-pulse` signals `stale` per spec risk #1.
- **`useNavigate` from `@solidjs/router` inside a component**: Hook must be called at component top-level (not inside an event handler). Returns a function that accepts a path string (e.g., `/account`). The router is router-root-relative — BrowserRouter `/account` resolves to the `/desk/account` hash route without extra prefixing.
- **Responsive collapse via Tailwind `md:` breakpoint (768px)**: `grid-cols-2 md:grid-cols-4` handles the 4-metric strip's mobile 2x2 layout. `hidden md:flex` + `flex md:hidden` toggle between compact (`$X / Yx`) and full (`$X · Yx · $Z free`) formats on PulseStrip.
- **`divide-x` + `divide-container-border` for internal cell dividers**: Cleaner than `border-r` on each child — Tailwind's divide utilities handle the "last-child no border" edge case automatically. Combined with `divide-y md:divide-y-0 md:divide-x` for mode-switching between stacked-mobile and columnar-desktop.
- **Solid `<For>` preferred over `.map()` even for fixed arrays**: Overview.tsx + StatSection.tsx both use `<For each={...}>`. While `.map()` works for static data, `<For>` is the idiomatic signal-reactive primitive and matches codebase style.

### 2026-04-17 — RSK-01 T6 (Wire real endpoint + Layout + Account composition)
- **`createResource` with reactive source gates the fetch**: `createResource(() => auth.isAuthenticated(), async (authed) => authed ? fetchRiskSnapshot() : null)`. The source function `() => auth.isAuthenticated()` is tracked; when it flips true the fetcher re-runs and returns `null` for unauthenticated state. Avoids `if (!auth.isAuthenticated()) return null` inside the fetcher, which wouldn't re-trigger on auth-state change.
- **Positioning context required for flex-column z-stacking**: The Layout has a `fixed z-0` background div, then relatively-positioned `<header class="relative z-50">` and `<main class="relative z-10">`. A static-positioned PulseStrip button would not layer above the fixed background — needed to wrap in `<div class="relative z-50 shrink-0">` before z-index takes effect. CSS z-index requires a positioned ancestor.
- **Derive ExchangeBalanceResponse from VenueMargin to keep ExchangeCard contract stable**: Rather than refactor `ExchangeCard` to accept `VenueMargin | ExchangeBalanceResponse`, synthesized a minimal `{ balances: [{ asset: 'USDT', total, available, used }] }` from `margin_by_venue` entries. `formatBalance` inside ExchangeCard already looks up USDT/USDC first — the synthesis matches its contract exactly. No component changes needed.
- **Snapshot owned twice (Layout + Account) is fine**: Both call `createResource(fetchRiskSnapshot)` independently. T2's 5s server-side cache makes the second fetch O(1). Avoided building a RiskSnapshotContext — extra plumbing, no measurable cost savings. Revisit only if the cache window becomes a bottleneck.
- **Removed `onMount(async () => {})` empty block** in Account.tsx along with `fetchBalances` and `balances` signal. The spec-plan "remove per-account balance fan-out" was satisfied by deleting the initializer, the helper, and the signal together — a single logical change.
- **Children-render-prop `<Show when={snapshot()}>{(snap) => …}`** flows the narrowed non-null accessor into the child without an explicit non-null assertion. LiveRiskStrip's prop type is `RiskSnapshot` (not `RiskSnapshot | null`), so this pattern avoids both `!` and redundant null-checks in markup.
- **Help entry keys are flat strings keyed by dotted namespace**: `HELP['risk.exposure']`, `HELP['risk.pulse']` — no nested object. HelpTip renders nothing when the string is empty/undefined (guard in HelpTip.tsx:15), so adding entries is additive without touching consumers. T7/T8 will wire the actual tooltip triggers.

### 2026-04-17 — RSK-01 T7 (PositionsByVenue widget)
- **Filter venues with zero positions before render, keep them in the count for the empty-state fallback**: `positions_by_venue` ships every connected venue (so `MarginByVenue` in T8 renders them all), but a per-venue positions table for an idle venue is noise. Filter at render time, but compute `venueCount()` from the unfiltered array so the "No open positions across N venue(s)" message stays truthful.
- **Pulse dot is conditional**: Same `w-2 h-2 rounded-full bg-signal-green animate-pulse` as `ActivePositions.tsx`, but swapped to `bg-text-tertiary/40` (no pulse) when `totalPositions() === 0`. The pulse carries semantic meaning ("live, active exposure") — always-pulsing on an empty hub would be a lie.
- **`overflow-x-auto` on table container**: Six-column position tables can overflow on ≤ sm viewports. Wrapping `<table>` in `<div class="overflow-x-auto">` gives horizontal scroll rather than column crush. T12 mobile QA will confirm this is sufficient.
- **Symbol shown verbatim from backend**: `ActivePositions.tsx` strips `_` via `(pos.symbol || '').replace('_', '')` because engine-managed trades use `BTC_USDT` internally. Exchange-side positions returned by the risk snapshot already come in the venue's native format (`BTC`, `BTC/USDT`, `BTC/USDT:USDT`), so no transformation is needed. If UX wants unified display, that's a backend normalization task, not a per-widget one.
- **Mount outside `max-w-4xl` but inside `!isOnboarding()` Show**: Account.tsx's existing card grid lives in `max-w-4xl mx-auto w-full px-8 py-10`. Positions need wider real estate (6 columns), so the new block sits as a sibling below that container at `px-8 pb-10` full-width — matching LiveRiskStrip's layout convention, not the cards'. The re-auth modal and add-exchange modal are `fixed inset-0` overlays, so they're unaffected by tree placement.

### 2026-04-17 — RSK-01 T8 (MarginByVenue + CorrelationStack widgets)
- **Sort via spread then `sort_by`**: `[...props.snapshot.margin_by_venue].sort(...)` — `Array.prototype.sort` mutates; spreading avoids mutating the snapshot prop (SolidJS signals would flag this as a reactivity violation even if the data were owned).
- **Min-width floor on correlation bars**: Raw `(value/max)*100` can produce bar widths under 1% when one bucket dominates (e.g. 95% BTC, 5% ETH → ETH bar invisible). `Math.max(2, Math.min(100, pct))` floors at 2% so small buckets remain visible. This is a UX choice (visibility > proportional fidelity); the numeric value next to the bar stays truthful.
- **Scale bars relative to max bucket, not sum**: Spec said "width proportional to effective_notional_usd" — scaling to `max` rather than `sum` makes the largest bucket always fill 100% and communicates *relative* stack rather than *share of total*. "Share of total" would need a different chart (stacked bar). Max-scaled matches the "directional stacking" mental model in the spec.
- **Direction coloring uses signal palette**: `bg-signal-green` for long, `bg-signal-red` for short, `bg-signal-amber` for mixed — matches T5's LiveRiskStrip leverage threshold colors and repository-wide convention. No new colors introduced.
- **Contributing symbols both as `title` attr AND inline row**: Spec said "hover/touch shows contributing symbols" — but `title` alone is invisible on touch devices and non-discoverable. Inline `·`-separated row below the bar makes the information always visible at a low-noise level. `title` attr preserved for desktop hover affordance and A11y tools.
- **Account.tsx outer container switched to `flex flex-col gap-8`**: T7 used `px-8 pb-10` directly on the positions wrapper. For T8's grid-beside-positions layout, lifted the gap to a parent flex column so the rhythm is consistent. Single source of vertical spacing, avoids accumulating `mt-8` per sibling.
- **`stripSign` as shared helper pattern**: `formatCurrency` always prefixes `+` for positive values. For FREE/USED/TOTAL fields that are semantically absolute (never negative in display), `.replace(/^\+/, '')` strips the sign. Repeated the helper in both MarginByVenue and CorrelationStack — duplication is acceptable (3 lines each) over premature abstraction to a shared util. If a third consumer emerges, lift it to `formatters.ts`.

### 2026-04-17 — RSK-01 T10 (WebSocket live push + polling fallback + stale indicator)
- **ws-stream subscription wire format is unchanged across surfaces**: `{ method: "SUBSCRIBE", params: ["order.{user_id}"], id: 1 }` matches `ws-stream/src/types.rs:10` and the extension's `testudo-extension/src/background/websocket.ts:103`. Response frames have `{ stream: "order.{user_id}", data: {...} }` — we only need `stream.startsWith("order.")` to treat them as risk events (ignore sidecar.health etc.). No Zod parsing needed in the journal; any malformed frame is swallowed by the `try/catch`.
- **`createResource` returns `[resource, { refetch, mutate }]`**: Existing Layout code only destructured `[pulseSnapshot]`, losing the control object. Destructuring the second element as `{ refetch: refetchPulse }` keeps the source-gated auto-refresh and unlocks manual triggering for WS-push + polling fallback — no changes to the fetcher itself.
- **`createEffect` reconciliation is idempotent — use it**: Gate the WS on `auth.isAuthenticated() && auth.user()?.id`. `wsClient.connect(uid)` tears down any existing socket before reconnecting, and `wsClient.disconnect()` is safe to call repeatedly. This means one effect can cover login, logout, and user switching without state tracking.
- **Polling fallback as a second effect, not a combined one**: One effect handles WS lifecycle, a second drives the 30s interval. Splitting them means `wsClient.connected()` is the only dependency of the polling effect — when WS state flips, polling flips in response without re-running the connect/disconnect logic.
- **Reactive staleness requires a time tick**: `Date.now() - Date.parse(snapshot.as_of) > 60_000` is correct but computed only when its inputs change. Without a `setInterval(setNow(Date.now()), 10_000)` signal, the PulseStrip would stay green forever after a WS disconnect — `snapshot` never changes, so the derived `pulseStale()` never re-evaluates. The 10s tick is the cheapest way to get reactive freshness.
- **`VITE_WS_URL` resolved via `import.meta.env` with inline default**: No `.env.example` touched (security policy: never read/create `.env*`). `vite-env.d.ts` doesn't exist in this project — Vite auto-types `VITE_`-prefixed vars as `string | undefined`. A cast `(import.meta.env.VITE_WS_URL as string | undefined) || 'ws://localhost:4000'` is sufficient.
- **`connected` prop defaults true on PulseStrip**: Existing mock callers (and any future consumer passing only `snapshot`) keep the pulsing-green dot. Only Layout's real call-site threads through `wsClient.connected()` and flips to the polling (no-pulse) state. Zero breakage.

### 2026-04-17 — RSK-01 T11 (Pulse Strip preference toggle)
- **Module-scoped `createSignal` is the cross-component shared-state pattern here**: `createSignal` called at module top-level yields one persistent signal for every importer. No Context provider, no event bus, no `window.dispatchEvent(new StorageEvent(...))` hack — Layout reacts to Account's toggle automatically because both read the same accessor. Matches the codebase's existing `lib/` conventions (no `lib/` modules currently own reactive state, but the mechanism is the cleanest option and costs no extra plumbing).
- **localStorage default is "on" via `!== 'off'` check**: Simpler than `value === null || value === 'on'` because it handles both the unset case and any legacy/typo-ed value by defaulting to enabled. FR-11 spec says "default on" — this is the minimal expression.
- **Try/catch around `localStorage` is load-bearing**: Private mode / iframes can throw on read or write. Signal initializer and setter both wrap in try/catch so the UI never hard-crashes on first render; the preference is just best-effort persistence.
- **Toggle lives in the existing subheader `justify-between`**: Account.tsx's `<div class="flex items-center justify-between ...">` already had `justify-between` with only the `<h1>`, so the toggle drops into the right-hand slot without layout churn. No new row added above/below.
- **`aria-pressed={pulseStripEnabled()}` for A11y**: Toggle button semantics without a full switch role — simpler and sufficient for a binary ON/OFF. Screen readers announce pressed/unpressed state.

### 2026-04-17 — RSK-01 T12 (Final verification)
- **Mechanical verification is the autonomous scope; live QA is deferred**: Vox can assert on cargo test counts, clippy cleanliness, build success, git history (Overview untouched), and structural responsiveness (Tailwind breakpoint classes in the rendered JSX). It cannot exercise a real 2s-WS-push flow or sweep pixel-level viewport widths without a browser + live exchange state. The plan's T12 entry distinguishes "performed" from "deferred to live session" so future audits know what was actually checked.
- **Full test suite: 1,098 passing, 0 failing** across common_utils (304), engine (108×2 for lib+bin counts), pg_queue (11), router (540), sqlx_postgres (17), ws_stream (10). Up from AUTH-02-era 1,013 — +85 tests across RSK-01 backend (risk_snapshot + downstream).
- **Clippy warnings stable at 3 pre-existing**: `cex_client.rs:649` `useless_conversion`, `actor.rs:1814` `unused_variables`, `evaluator.rs:188` `manual_contains`. None introduced by RSK-01.
- **Output file truncation with `| tail -N` on background tasks**: Background Bash output is captured as the literal stdout of the command — when the command pipes through `tail -15`, only 15 lines land in the output file. For full test output, omit the tail pipe or grep the raw output file. Important for autonomous verification tasks that need complete logs.
- **Overview regression check via `git log -- path`**: Simpler than a DOM snapshot test. `git log --since="$SPEC_DATE" -- testudo-journal/src/components/Overview.tsx` returning zero commits, combined with a clean `git diff` working tree, is sufficient evidence that the Overview component tree is byte-identical at source level. The rendered DOM can still change via CSS tokens — but those are intentional theming-layer changes, not RSK-01 regressions.

### 2026-04-18 — RSK-01a T1 (Overview hero consolidation + strip deletions)
- **WS ownership migrates cleanly from Layout to Overview**: Because `createRiskWsClient` is a factory (not a module singleton), Overview can own its own instance with no special teardown. `onCleanup` inside Overview fires on route change (`/` → `/account`), so the WS disconnects before Account mounts its own future resource. No double-subscription possible when the two consumers are sibling routes.
- **Hero layout gains `flex-wrap` for multi-metric overflow**: Original hero had only 2 items (`gap-10`). With 5 inline metrics + live dot, the row can exceed viewport width on mid-sized displays. Adding `flex-wrap` to both desktop and mobile hero rows prevents overflow without dropping the shared baseline alignment.
- **Secondary metrics use `text-2xl md:text-3xl`, dominant metrics keep `text-4xl md:text-5xl`**: Visual hierarchy preserves Net P&L + balance as dominant; exposure / leverage / free as subordinate. Labels shrink from `ml-3` to `ml-2` and all use `text-sm` to keep the hero scannable.
- **`hidden md:flex` almost broke mobile accidentally**: Wrapping the desktop column in `hidden md:flex` would have hidden the calendar + charts on mobile. Caught before commit — the desktop container must stay unconditional (`flex flex-1 min-h-0`); only the aside sidebar is `hidden md:block`. The *mobile strip* is `md:hidden` (conditional on the inverse breakpoint), not the container.
- **Live/stale dot uses same classList trichotomy as PulseStrip used to**: green + pulse (WS connected, fresh), green static (polling, fresh), amber (stale). Preserved by copying the three-case `classList` from PulseStrip into the dot's `<span>` in Overview. No new visual grammar.
- **`relativeTime(snap)` helper returns "last updated: Ns ago"** and is used verbatim as `title` attr on the dot. Hover reveals the timestamp for desktop affordance; mobile screen readers announce it via aria-label.
- **Layout.tsx drops ~65 lines**: snapshot + WS + polling + stale ticker + pulse-strip-enabled block all gone. Remaining responsibility: nav header, theme cycling, lock/connecting/error screens, standalone-page carve-out.
- **Stale comment cleanup**: `RSK-01 T6: Snapshot drives LiveRiskStrip and per-card balance display` referred to deleted consumers. Removed because both LiveRiskStrip (T1) and the synthesized balance path (T2 upcoming) are going away. Keeping stale comments on file boundaries creates "what does this even do" confusion later.

### 2026-04-18 — RSK-01a T2 (ExchangeCard margin breakdown)
- **`snapshot?: RiskSnapshot` prop beats pre-synthesized `balance?`**: Moving the `margin_by_venue.find(m => m.exchange_id === props.account.id)` lookup into the card eliminates the `venueMarginFor` / `balanceForCard` helpers on Account.tsx entirely. The card is the correct owner of venue-specific derivation — Account just hands over the raw snapshot and lets each tile self-serve. Net ~15-line deletion on Account.tsx, with the lookup cost identical (one `.find` per card either way).
- **Free-ratio bar reuses CorrelationStack's grammar**: `h-1.5 bg-text-primary/5 w-full` track + inline `bg-signal-green` fill with `style={{ width: `${pct}%` }}`. Same two-div pattern as CorrelationStack:86–91. Keeps visual language coherent; no new primitive needed.
- **`formatBalanceUsd(raw)` strips sign at the source**: Unlike `formatCurrency` which prefixes `+`/`-`, margin values are always semantically non-negative (total/free/used in USD-equivalent). Using `Math.abs()` inside a dedicated helper is simpler than `.replace(/^[+-]/, '')` post-hoc, and expresses intent clearly.
- **`freeRatio` guards against `total <= 0` and non-finite values**: A venue with 0 total (pre-balance-sync or deleted) would produce `NaN`/`Infinity`. Clamp to `[0, 100]` with `Math.max(0, Math.min(100, pct))` and `isFinite` checks. Prevents a 0-width bar from silently showing a broken style attribute.
- **`Show when={venueMargin()}` fallback = "Margin unavailable"**: Spec risk #1 calls for a defensive lookup-miss path. Rendering a small tertiary-colored inline message (not a crash, not "---") signals: "the card is healthy but snapshot hasn't provided data for this venue yet" — distinct from the reauth error state (which uses an amber banner + button).
- **`mt-auto flex flex-col gap-2` preserves vertical stacking**: Previous card had `mt-auto` on the balance block alone. New version nests the margin breakdown (which uses internal `gap-1.5`) + optional test result inside a single flex column that still pushes to the card's bottom. Positions slot (T3) will land in this same container below test-result.

*This file grows as Vox learns. Never delete entries.*
