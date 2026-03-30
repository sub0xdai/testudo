# Implementation Plan

> Last updated: 2026-03-30
> Current spec: SEC-01-trade-auth-bypass
> Phase: COMPLETE

---

## Active Spec: SEC-01-trade-auth-bypass

Enforce JWT authentication on trade management routes. Gate live exchange operations behind `is_authenticated`.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Add JwtMiddleware to /trades scope in main.rs | complete | low | — |
| T2 | Gate cleanup_stale_trades live operations behind is_authenticated | complete | low | T1 |

### Key Decisions

- **Tests unaffected**: Integration tests build their own `App` without middleware — JwtMiddleware only applies via main.rs scope config.
- **DB persist gated too**: `mark_position_closed` via `trade_manager_live` is also gated behind `is_authenticated` — defense-in-depth.
- **Shadow ops always run**: Shadow engine cancel + status update run regardless of auth — paper trading cleanup still works.

### Discoveries

- Tests use direct route registration without middleware, so adding JwtMiddleware to the scope has zero test impact
- `extract_user_id` already handles JWT-first, X-User-Id fallback — with JwtMiddleware applied, the fallback path is unreachable for this scope
- 1,016 tests pass, 0 failures after change

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| SEC-01-trade-auth-bypass | 2026-03-30 |
| EXT-46-async-scraper-flow | 2026-03-29 |
| EXT-45-dexscreener-symbols | 2026-03-29 |
| EXT-44-hyperliquid-support | 2026-03-29 |
| EXT-43-main-world-bridge | 2026-03-29 |
| DOCS-01-comprehensive-documentation | 2026-03-27 |
| HIST-01-exchange-history-import | 2026-03-26 |
| ONBOARD-01-stepper-onboarding | 2026-03-26 |
| DESK-02-landing-strip | 2026-03-26 |
| EXT-41-desk-dashboard | 2026-03-24 |
| EXT-40-smart-card-grid | 2026-03-24 |
| EXT-39-pair-ux | 2026-03-24 |
| AUTH-03-frontend-auth | 2026-03-24 |
| AUTH-02-backend-auth | 2026-03-24 |
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2022-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
