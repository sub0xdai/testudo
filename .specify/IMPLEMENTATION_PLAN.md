# Implementation Plan

> Last updated: 2026-03-24
> Current spec: AUTH-01-infra-hardening
> Phase: BUILD

---

## Active Spec: AUTH-01-infra-hardening

Infrastructure hardening — Docker Compose network isolation, sidecar PSK authentication, wallet-primary users table, and server-side session tracking. Foundation for AUTH-02 (backend auth) and AUTH-03 (frontend auth).

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Sidecar PSK middleware (`testudo-cex/src/middleware/psk.ts`) + mount in `server.ts` | complete | low | — |
| T2 | Sidecar Dockerfile (`testudo-cex/Dockerfile`) — Bun runtime, copies `safe-cex-sub0` vendor dep | complete | low | — |
| T3 | Router PSK injection — add `psk` field to `CexSidecarConfig`, inject `X-Internal-Secret` header in `CexClient::post()` and `health_check()` (GET excluded) | complete | medium | — |
| T4 | Wallet-primary users migration — add `wallet_address`, drop `email`/`password_hash`/`email_verified`, drop email constraints/trigger | complete | low | — |
| T5 | `user_sessions` migration — create table with FK → users, 3 indexes | complete | low | T4 |
| T6 | Production Docker Compose (`docker-compose.production.yml`) — two networks, health checks, `.env.production.example` | complete | medium | T1, T2 |
| T7 | Validate: `cargo clippy --all-targets && cargo test` + `cd testudo-cex && bun test` | complete | low | T1–T6 |

### Key Decisions

- **PSK dev-mode bypass**: If `SIDECAR_PSK` env var is unset, middleware passes all requests (dev mode open).
- **Health exempt**: `/health` endpoint bypasses PSK check always.
- **Separate production compose**: `docker-compose.production.yml` is new file — existing `docker-compose.yml` + `docker-compose-core.yml` untouched.
- **Migration ordering**: Wallet migration `000000`, sessions `000001` — SQLx runs in filename order.
- **No Redis in production**: Redis removed from production compose (deprecated per pg_queue).
- **Build context**: Sidecar Dockerfile uses monorepo root as build context to access `safe-cex-sub0` sibling.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
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
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
