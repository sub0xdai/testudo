# EXT-05: Backend Auth & Live Exchange Execution

> Priority: P1 | Depends on: EXT-04 | Status: COMPLETE

## Overview
**Current:** Paper trading works via unauthenticated X-User-Id header. Binance adapter exists but is not activated.
**Target:** /trades routes accept JWT auth (with X-User-Id fallback for paper). Extension handles login/token storage. Execution mode toggle switches between Shadow (paper) and Live (Binance). Live trades require double-confirmation.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Dual auth on /trades -- accept JWT Bearer token (preferred) or X-User-Id header (fallback) | DONE |
| FR-2 | Extension login flow -- popup page with email/password, stores JWT in chrome.storage.local | DONE |
| FR-3 | Token refresh -- background worker refreshes token before expiry using POST /auth/refresh | DONE |
| FR-4 | Activate Binance adapter -- initialize ExecutionService with BinanceAdapter when env vars present | DONE |
| FR-5 | Execution mode routing -- X-Execution-Mode header controls Shadow vs Live; Live requires JWT | DONE |
| FR-6 | Live trade confirmation -- LIVE MODE warning in modal, double-confirm for real money | DONE |
| FR-7 | Bearer token in requests -- background worker sends Authorization header when JWT available | DONE |

## Architecture

### Backend (dual auth, no breaking changes)
- `extract_user_id` updated: checks JWT claims first (via auth_service.verify_token), falls back to X-User-Id
- No JwtMiddleware wrapping on /trades (preserves backward compat and existing tests)
- TradeManagementState gains `auth_service` field for token verification
- BinanceAdapter conditionally created when BINANCE_API_KEY env var is set
- X-Execution-Mode header: "shadow" (default) or "live" (requires JWT)

### Extension (login + token management)
- Popup: login form, token status display
- Background worker: login/refresh handlers, stores tokens, sends Bearer header
- Content script: passes execution mode with trade messages
- Modal: red LIVE warning badge, double-confirm Enter

## Key Files

| File | Change |
|------|--------|
| `router/src/routes/trade_management.rs` | Update extract_user_id for dual auth, add execution_mode handling |
| `router/src/main.rs` | Add auth_service to TradeManagementState, conditionally create BinanceAdapter |
| `testudo-extension/src/background.ts` | Login/refresh handlers, Bearer token in trade requests |
| `testudo-extension/src/popup/popup.html` | Login form UI |
| `testudo-extension/src/popup/popup.ts` | Login flow logic |
| `testudo-extension/src/modal.ts` | LIVE mode warning badge |
| `testudo-extension/src/content.ts` | Pass execution mode |

## Verification
```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
cd testudo-extension && bun run typecheck && bun run build
```
