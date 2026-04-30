# Testudo Extension — Version Changelog

Tracks released versions of the browser extension (Chrome + Firefox).
Source: `testudo-extension/manifest.json`. Archived zips: `testudo-extension/archive/`.

Format: **[semver] — YYYY-MM-DD — status** then a short bullet list of what shipped.
Rule: every version bump adds an entry here *before* the build is zipped.

---

## Unreleased — accumulating for next bump

Fixes landing on `master` since the 1.1.4 zip was sealed. Version bump and
zip build deferred — these will batch into the next submission once the
user is ready (Chrome review takes ~3 days per submission).

### Firefox session timeouts forcing re-pair

**`auth.ts`, `manifest.json`** — the scheduled token refresh used
`setTimeout`, which doesn't survive MV3 background-worker suspension.
Locally-loaded Chrome extensions get an artificially long worker
lifetime (devtools / unpacked mode), masking the bug; Firefox AMO
production suspends aggressively, so the 12-minute refresh timer dies
and tokens silently expire. The 401-fallback in `api.ts` recovers in
the happy path, but during transient backend hiccups the backoff chain
can exhaust and force re-pair.

Fix: migrated refresh scheduling to `browser.alarms`. Alarms persist
across worker restarts AND wake the worker when they fire — the
canonical MV3 pattern. Added `"alarms"` permission to manifest.
`scheduleTokenRefresh` / `scheduleRawRefresh` / `clearRefreshTimer`
now create / clear a `testudo-refresh` alarm. A top-level
`browser.alarms.onAlarm` listener (registered at module init so it
re-binds on every cold start) calls `refreshAccessToken()` when the
alarm fires.

Note: alarm `delayInMinutes` has a per-browser minimum (~1 min on some
Firefox versions). The 30s transient-backoff retry will round up to
~1 min in those cases — acceptable.

### Firefox PairView scroll clip

**`9d41d73` fix(ext): allow body scroll fallback so Firefox PairView disclosures aren't clipped**

Root cause: Firefox's WebExtension popup viewport doesn't always honor
the requested 680px body height, leaving the bottom of PairView (How it
works / Why these permissions / Privacy & disclaimers) unreachable. The
modal's internal `pair-scroll` container was pinned to a body parent
that Firefox couldn't fully display, and `body { overflow: hidden !important }`
blocked any fallback scroll.

Fix: replaced body `overflow: hidden !important` with `overflow-y: auto`
in `popup.css`; removed inline `overflow:hidden` from `popup.html`'s
body. Scrollbars stay visually hidden via existing `scrollbar-width: none`.
Chrome popup honors 680px and never engages this fallback — zero visual
change there. `#app` still clips, so MainView's tab-internal scrollers
are unchanged.

### USDC balance shows 0 in trade modal

**`TradeForm.tsx`** — modal's balance lookup hard-coded `asset === "USDT"`,
so on Bybit USDC perps (which settle in USDC) the modal couldn't find a
balance row → `available()` returned null → display read 0 USDT. The
popup's `MainView.tsx:58` already used the correct `USDT || USDC` pattern;
the modal was the outlier.

Fix:

- `usdt()` selector now matches `USDT || USDC` (parity with MainView).
- New `quoteAsset()` accessor returns the actual matched asset.
- Margin / Risk / Available rows now render `{quoteAsset()}` instead of
  hard-coded "USDT".

### Setup field keystroke leak to TradingView

**`content.ts`, `modal.tsx`, `TradeForm.tsx`** — typing in the SETUP
`<textarea>` inside the Alt+X modal also triggered TradingView's chart
hotkeys (e.g. typing "f" in "falling wedge" toggled TV's "f" shortcut).
The shadow-DOM modal's existing handlers all sat at or below TV's
`document`-capture listener in event order, so they couldn't pre-empt.

Fix: added a `window`-capture-phase keydown/keyup/keypress listener in
`content.ts` that, while the modal is visible AND the event originated
in our shadow root, calls `e.stopPropagation()`. `window` capture fires
before any `document` listener regardless of registration order, so TV
is bypassed. Critical detail: `stopPropagation` halts listener dispatch
but does NOT cancel the browser's default text-input action — characters
still land in the focused input. Esc / Enter / Tab / Alt+X are
exempted so the existing modal handlers (focus trap, double-Enter
confirm, dismiss, hotkey re-trigger) continue to work. `modal.tsx` now
exposes `getActiveHost()` for the listener to scope events to the live
shadow root.

---

## 1.1.4 — 2026-04-29 — **built, ready for submission** (rebuilt, cleaned)

Rebuilt 2026-04-29 to include critical session-resilience fix. Dynamic Risk (QNT-01b) disabled — backend endpoints (GET_USER_SETTINGS, PATCH_USER_SETTINGS) not ready for 1.1.4; will ship in 1.1.5. Zips: `testudo-sniper-chrome-1.1.4.zip` and `testudo-sniper-firefox-1.1.4.zip`.

### Critical Auth Fix (Apr 28)

**`171833c` fix(ext): stop unpairing on transient backend failures**

Root cause: `doRefresh()` cleared tokens on ANY non-2xx response (502 during deploy, 429 rate limit, 503 overload, timeouts). Token refresh fires every ~12 minutes (80% of 15-min access-token life), so even brief transient errors silently nuked sessions. Users reported "I have to re-pair too often."

Fix:

- **Classify failures.** 401/403/other 4xx → definitive (clear tokens, require re-pair). 5xx/408/429/network error → transient (keep tokens, retry).
- **Exponential backoff for transient.** 30s → 2min → 8min. After 3 consecutive transient failures, give up and clear tokens.
- **SessionState storage flag.** 'ok' | 'refresh_retrying' | 'session_lost' | 'wallet_changed'. Popup renders a context-aware banner instead of silently dropping to PairView with no explanation.
- **Explicit failure modes.** `session_lost` banner for auth errors; `wallet_changed` banner for wallet switch. Each case now has actionable user guidance.

### Review-response UX patch (Apr 19 baseline)

Addresses the Chrome Web Store reviewer's concerns about functional
clarity surfaced during the 1.1.3 appeal — the extension's value prop
and permission scope are now legible without requiring a paired
backend account.

- **Manifest description rewritten.** From vague "risk management
  overlay / circuit breakers" marketing copy to concrete "TradingView
  companion. Press Alt+X on a chart to size a trade from your stop and
  route the order to your exchange."
- **Pre-pair explainer on the pair screen.** New paragraph above the
  pairing instructions explaining the Alt+X flow and stating
  explicitly that the extension holds no funds and never sees exchange
  API keys.
- **Permissions justification UI.** Collapsed `<details>` block below
  the PAIR button, grouped into Chart hosts / Testudo API / Storage,
  explaining each permission's purpose.
- `387f538` (carry-over) refactor(ext): declutter trade modal — delete
  dead Management Rules block, sharpen Setup tag field.

### Web → extension session bridge (Phase 2-proper)

When the web session's wallet changes — via MetaMask switch or
explicit logout — the extension's paired JWT was previously stranded on
the old wallet, silently routing Alt+X trades to the prior wallet's
exchange account until the user manually re-paired.

Fix delivered as a one-way web → extension session-change bridge:

- **Web (AuthContext.tsx):** on wallet-switch in the Phase 1 guard and
  on logout, dispatches
  `window.postMessage({ type: 'TESTUDO_WALLET_CHANGED', wallet_address })`
  (with `null` on logout).
- **Extension (content.ts):** listener gated to `desk.testudo.vip`
  hostname only — TradingView / exchange pages cannot spoof the event.
  Relays to the background worker via
  `chrome.runtime.sendMessage({ type: 'WEB_WALLET_CHANGED', ... })`.
- **Extension (schemas.ts, background/handlers.ts):** new
  `WEB_WALLET_CHANGED` message variant. Handler compares the
  extension JWT's `wallet_address` to the incoming web wallet; clears
  tokens + refresh timer if web is logged out or bound to a different
  wallet. Popup reactively drops to the pair screen.

No modal / badge / banner on wallet mismatch — the session just invalidates silently.

---

## 1.1.3 — 2026-04-18 — **REJECTED, appeal pending**

- Popup balance display (`MainView.tsx`) now reads the `total` field returned by `/api/v1/exchanges/accounts/:id/balance` directly, instead of locally computing `available + locked`. Prior behaviour drifted from the exchange's own displayed total by the amount of unrealized P&L, confusing users.

**Rejection:** "Inaccurate Description — Non functional" (violation ref: Red Potassium). Cited the "Connect Account" button. Fixed via store-listing metadata changes only (no code change):
- Store description now discloses SIWE wallet requirement up front.
- Test Instructions rewritten with a no-account verification path (install → TradingView → Alt+X).
- Appeal submitted 2026-04-18.

**Commits:** `2b2d6a6` (version bump), `bf1b74a` (finalize zip swap at root)

---

## 1.1.2 — ~2026-04-16 — **submitted, superseded by 1.1.3**

- CSP compliance: removed inline scripts from extension HTML to satisfy Manifest V3.
- Production URL defaults: extension no longer defaults to `localhost:8080`; prod URLs (`api.testudo.vip`, `ws.testudo.vip`) are baked in.
- Balance discrepancy fixes, overview hero adjustments, R-multiple fallback computation.

**Commits:** `128d394`, `df4ab46` (no dedicated "bump to 1.1.2" commit — manifest was edited ad-hoc before zipping)

---

## 1.1.1 — ~2026-04-16 — **submitted, superseded by 1.1.2**

- UX-07: extension token sync between web app and extension.
- Token storage migration to `chrome.storage.session`.
- Three-tier button hierarchy + terra-cotta accent removal in popup UI.

**Commits:** `50b4a6a`, `de64b0e` (no dedicated "bump to 1.1.1" commit — manifest edited ad-hoc)

---

## 1.1.0 — 2026-04-14

- **EXT-46**: universal TradingView widget scraping — works across all exchange-embedded charts (Bybit, Binance, OKX, Bitget, Gate, Phemex, BloFin, Hyperliquid), not just standalone TradingView.
- `EXT-46 feat(a33cf0f)`: universal widget discovery
- `EXT-46 feat(ee18444)`: universal widget scraping

**Version bump commit:** `1c3238f`

---

## 1.0.2 — pre-2026-04-14 — **first Chrome Web Store submission**

First stable submission to Chrome Web Store. Zipped as `testudo-chrome.zip` (no version suffix).

Contents covered: Alt+X hotkey on TradingView, DOM scraper with 3-strategy fallback, Shadow DOM trade modal, popup UI with account pairing, background worker for auth + WebSocket + trade execution via CCXT sidecar.

**Version recorded only in zip manifest**; no explicit bump commit.

---

## 1.0.1 — pre-2026-04-03 — **first Firefox AMO submission**

First Firefox add-on submission. Zipped as `testudo-firefox-1.0.1.zip`.

Contents equivalent to Chrome 1.0.2 but built for Firefox's AMO packaging.

**Version recorded only in zip manifest**; no explicit bump commit.

---

## Housekeeping

**Going forward, every version bump MUST:**
1. Update `testudo-extension/manifest.json` → new version
2. Add a new dated entry at the top of this file (under a fresh `## X.Y.Z — YYYY-MM-DD — status` header)
3. Move prior root-level `testudo-{chrome,firefox}-*.zip` into `testudo-extension/archive/`
4. Commit all three changes together as `chore(ext): bump to X.Y.Z`

**Status values:**
- `submitted, pending review`
- `submitted, rejected, appeal pending`
- `published`
- `superseded by X.Y.Z`
- `REJECTED` (appeal failed / abandoned)
