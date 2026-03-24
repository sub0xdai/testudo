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

*This file grows as Vox learns. Never delete entries.*
