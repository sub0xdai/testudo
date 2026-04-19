# Testudo Extension — Version Changelog

Tracks released versions of the browser extension (Chrome + Firefox).
Source: `testudo-extension/manifest.json`. Archived zips: `testudo-extension/archive/`.

Format: **[semver] — YYYY-MM-DD — status** then a short bullet list of what shipped.
Rule: every version bump adds an entry here *before* the build is zipped.

---

## 1.1.5 — 2026-04-19 — **built, pending submission**

Phase 2-proper of the wallet/session-cookie audit (Phase 1 landed on
testudo-journal + testudo-exchange the same day). When the web session's
wallet changes — either via wallet switch in MetaMask or explicit
logout — the extension's paired JWT was previously stranded on the old
wallet, and Alt+X trades would silently route to the old wallet's
exchange account until the user manually re-paired. ~10% of users
maintain multiple wallets with distinct exchange accounts, so this was a
real footgun.

Fix delivered as a one-way web → extension session-change bridge:

- **Web (AuthContext.tsx):** on wallet-switch in the existing Phase 1
  guard, and on logout, dispatches
  `window.postMessage({ type: 'TESTUDO_WALLET_CHANGED', wallet_address })`
  with the new address (or `null` on logout).
- **Extension (content.ts):** new listener gated to
  `desk.testudo.vip` hostname only (TradingView / exchange content
  scripts can't spoof it), relays the event to the background worker
  via `chrome.runtime.sendMessage({ type: 'WEB_WALLET_CHANGED', ... })`.
- **Extension (schemas.ts, background/handlers.ts):** new
  `WEB_WALLET_CHANGED` message variant. Handler decodes the extension's
  paired JWT, compares `wallet_address` to the incoming web wallet, and
  clears tokens + refresh timer if web is logged out or bound to a
  different wallet. Popup reactively drops to the pair screen.

No modal / badge / banner — the session just invalidates silently, and
the user re-pairs when they next open the popup. Uniform with how the
rest of the auth flow works.

---

## 1.1.4 — 2026-04-19 — **built, pending submission**

Review-response UX patch. Addresses the Chrome Web Store reviewer's
concerns about functional clarity surfaced during the 1.1.3 appeal by
making the extension's value prop and permission scope legible without
requiring a paired backend account.

- **Manifest description rewritten.** From vague "risk management
  overlay / circuit breakers" marketing copy to concrete "TradingView
  companion. Press Alt+X on a chart to size a trade from your stop and
  route the order to your exchange."
- **Pre-pair explainer on the pair screen.** New paragraph above the
  pairing instructions explaining the Alt+X flow and stating
  explicitly that the extension holds no funds and never sees exchange
  API keys. Reviewers (and first-time users) now see the value prop
  before being asked to pair.
- **Permissions justification UI.** New collapsed `<details>` block
  below the PAIR button, grouped into Chart hosts / Testudo API /
  Storage, explaining each permission's purpose. Satisfies store
  review scrutiny on broad host-permission scope across financial
  domains.
- `387f538` (carry-over) refactor(ext): declutter trade modal — delete
  dead Management Rules block, sharpen Setup tag field.

**Commits:** staged as `feat(ext): 1.1.4 — review-response UX patch`.

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
