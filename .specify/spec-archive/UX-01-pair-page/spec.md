# Specification: Standalone Extension Pairing Page

**Spec ID:** UX-01-pair-page
**Date:** 2026-03-30
**Status:** Complete
**Class:** Feature / UX
**Priority:** P1 — Current pairing flow sends new users to a non-existent "Settings" page; broken first-run experience
**Depends on:** None
**Series:** Standalone

---

## Problem Statement

The extension's `PairView.tsx` instructs new users to "Open Settings" and "Click Connect Extension", then links to `desk.testudo.vip/account`. This has three problems:

1. **There is no "Settings" page** — the page is called "Account" and lives inside the Desk app, which requires launching the Desk first. A new user who just installed the extension from the store has no context for what "Desk" is.
2. **The pairing code UI is buried in the navbar** — the `ExtensionChip` component in `Layout.tsx` is a small button in the header. Users have to hunt for it.
3. **Three context switches** — extension popup → web app → find button → generate code → copy → back to extension. Too much friction for a first-run flow.

The fix is a dedicated standalone `/pair` page in the Desk app that handles the entire flow (wallet connect → code generation) in one place, with auto-detection of whether the extension is installed. The extension PairView updates its instructions to point at `testudo.vip/pair`.

---

## User Stories

- **As a new user who installed the extension first**, I want a single link that takes me to a page where I can connect my wallet and get a pairing code, so that I don't have to figure out what "Settings" or "Desk" means.
- **As a new user who found the website first**, I want to know I need the extension and where to get it, so that I can complete the setup without guessing.
- **As a returning user**, I want to re-pair my extension from the Desk navbar without visiting a separate page, so that the existing flow still works.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | New `/pair` route in Desk app — standalone page, no sidebar/navbar | High | Desk (testudo-journal) |
| FR-2 | Extension auto-detection via `window.__TESTUDO_INSTALLED` flag set by content script | High | Extension + Desk |
| FR-3 | Three-state page: install prompt → wallet connect → pairing code | High | Desk |
| FR-4 | Pairing code auto-generates on entering authenticated state (no extra click) | High | Desk |
| FR-5 | Same background image as landing page and lock screen (CDN bg) | Medium | Desk |
| FR-6 | Extension PairView instructions updated to point at `testudo.vip/pair` | High | Extension |
| FR-7 | `testudo.vip/pair` redirects to `desk.testudo.vip/pair` | Medium | Landing (testudo-web) |
| FR-8 | Navbar ExtensionChip remains unchanged for re-pairing | Low | Desk |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Create `/pair` route with 3-state UI, no auto-detection yet | Page renders, wallet connect works, code generates |
| CP-2 | Add content script flag + auto-detection logic | Page adapts based on extension presence |
| CP-3 | Update extension PairView instructions + landing site redirect | Full flow works end-to-end from extension |

### Page States

The `/pair` page is a standalone route outside the Desk layout (no sidebar, no navbar). It renders a single centered card over the landing page background image.

**State 1 — No Extension Detected** (`window.__TESTUDO_INSTALLED` is falsy):

```
// CONNECT_EXTENSION

Install the Testudo Sniper extension
to start trading from any chart.

[ CHROME WEB STORE ]
[ FIREFOX ADD-ONS  ]

───────────────────
Already installed? Refresh this page.
```

- Store buttons link to Chrome Web Store / Firefox Add-ons listing
- "Refresh this page" is a subtle text link

**State 2 — Extension Detected, Not Authenticated:**

```
// AUTHENTICATE

Connect your wallet to
link your extension.

[ CONNECT WALLET ]
```

- Single button triggers existing SIWE flow
- On success, transitions directly to State 3 (no reload)
- If already authenticated (session cookies present), skip to State 3

**State 3 — Authenticated:**

```
// PAIR_EXTENSION

Enter this code in your
extension popup.

  4  7  2  8  1  5

       0:47

Click code to copy

───────────────────
Code expired? [ GENERATE NEW CODE ]
```

- Code auto-generates via `POST /api/v1/auth/pair-extension` on entering this state
- Large monospace digits, clickable to copy to clipboard
- Countdown timer (60s TTL from backend)
- On expiry, digits fade and "Generate New Code" becomes primary action

### Extension Content Script Flag

In `testudo-extension/src/content.ts`, on pages matching `desk.testudo.vip/pair` (or localhost equivalent), set a window flag:

```typescript
// Set flag for pair page auto-detection
if (window.location.pathname === "/pair" || window.location.pathname === "/desk/pair") {
  window.postMessage({ type: "TESTUDO_INSTALLED" }, "*");
}
```

The `/pair` page listens for this message:

```typescript
window.addEventListener("message", (e) => {
  if (e.data?.type === "TESTUDO_INSTALLED") {
    setExtensionDetected(true);
  }
});
```

Using `postMessage` rather than `window.__TESTUDO_INSTALLED` because content scripts run in an isolated world and cannot set properties on the page's `window` object directly.

### Extension PairView Changes

In `testudo-extension/src/popup/components/PairView.tsx`, update the instructions block (lines 143-160):

**Before:**
```
1. Open Settings
2. Click Connect Extension
3. Paste the code below
```

**After:**
```
1. Visit testudo.vip/pair
2. Connect your wallet
3. Paste the code below
```

Update the "Open Settings" button to open `testudo.vip/pair` instead of `${DESK_URL}/account`.

### Landing Site Redirect

In `testudo-web` (Astro/React on Cloudflare Pages), add a redirect from `/pair` to `desk.testudo.vip/pair`. This can be a Cloudflare Pages `_redirects` file:

```
/pair https://desk.testudo.vip/pair 302
```

Or an Astro page that does a client-side redirect.

### Paved Roads

- **Lock screen design** (`testudo-journal/src/pages/Desk.tsx`): The existing lock screen uses the same centered-card-over-bg-image pattern. Reuse the same CSS classes (`bg-main-bg/75 backdrop-blur-md`, thin borders, ghost labels).
- **ExtensionChip** (`testudo-journal/src/components/Layout.tsx` lines 86-284): Code generation, countdown timer, and clipboard copy logic already implemented. Extract or reuse.
- **SIWE wallet connect**: Already wired in the Desk app's auth context.
- **Content script matches**: Extension manifest already declares `desk.testudo.vip` in content_scripts matches (via the `*://*/*` pattern for chart platforms).

### Files

- `testudo-journal/src/pages/Pair.tsx` — New standalone pairing page (3-state UI)
- `testudo-journal/src/App.tsx` (or router config) — Add `/pair` route outside layout
- `testudo-extension/src/popup/components/PairView.tsx` — Update instructions text + link
- `testudo-extension/src/content.ts` — Add `TESTUDO_INSTALLED` postMessage on /pair pages
- `testudo-extension/manifest.json` — Ensure content_scripts matches desk.testudo.vip (may already)
- `testudo-web/_redirects` or equivalent — `/pair` → `desk.testudo.vip/pair` redirect

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `desk.testudo.vip/pair` renders standalone page with no Desk sidebar/navbar
- [ ] Page uses same background image as landing page
- [ ] Without extension installed: shows Chrome/Firefox store links
- [ ] With extension installed, not authenticated: shows wallet connect button
- [ ] With extension installed + authenticated: auto-generates and displays pairing code
- [ ] Code is clickable to copy, countdown timer works, expired state shows regenerate button
- [ ] Extension PairView instructions say "Visit testudo.vip/pair" (not "Open Settings")
- [ ] Extension PairView "Open Settings" button opens `testudo.vip/pair`
- [ ] `testudo.vip/pair` redirects to `desk.testudo.vip/pair`
- [ ] Navbar ExtensionChip still works for re-pairing (unchanged)
- [ ] `bun run build` passes for testudo-journal and testudo-extension

---

## Risks

1. **Content script isolation** — Content scripts cannot set `window.*` on the page's JS context. Mitigation: Use `postMessage` bridge (same pattern as the TradingView page-bridge in EXT-43).
2. **Manifest content_scripts match** — Extension may not inject on `desk.testudo.vip/pair` if the match pattern doesn't cover it. Mitigation: Verify manifest matches or add `*://desk.testudo.vip/*`.
3. **Cloudflare Pages redirects** — `_redirects` file has specific syntax requirements. Mitigation: Test redirect locally before deploying.

---

## Completion Signal

This spec is complete when:
1. `/pair` page renders with all 3 states working
2. Extension auto-detection works via postMessage
3. Extension PairView points to `testudo.vip/pair`
4. Landing site redirect configured
5. All acceptance criteria met
6. `bun run build` passes for testudo-journal and testudo-extension
7. Code committed to master
