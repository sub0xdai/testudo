# Specification: Infrastructure Hardening — Docker Compose Network Isolation & Session Migration

**Spec ID:** AUTH-01-infra-hardening
**Date:** 2026-03-24
**Status:** Draft
**Class:** Infrastructure / Auth
**Priority:** P0 — Foundation for all auth refactoring; sidecar is unauthenticated, no server-side sessions, users table needs wallet-primary schema
**Depends on:** None (first in series)
**Series:** AUTH-01 through AUTH-03 (authentication architecture hardening)

---

## Problem Statement

The current Docker infrastructure (`testudo-exchange/docker/docker-compose.yml` and `docker-compose-core.yml`) is development-oriented. All services share a flat `exchange` bridge network. The CCXT sidecar (`testudo-cex`) is not containerized — it runs as a bare `bun run src/server.ts` process binding `0.0.0.0:3100` with zero authentication. Any process on the host can execute trades by sending raw HTTP to it.

The backend's auth model is email/password with stateless JWTs. The logout handler in `crates/router/src/routes/user.rs:130-162` is a no-op. A stolen refresh token remains valid for 30 days. There is no `user_sessions` table for token revocation or rotation tracking.

The platform is moving to wallet-primary authentication (SIWE). The `users` table (`20250922164541_users.up.sql`) currently has `email`, `password_hash`, `email_verified` columns that must be replaced by `wallet_address` as the sole identity. The `exchange_accounts` table already handles CEX API key storage and agent wallet columns — it remains unchanged.

This spec delivers three infrastructure prerequisites: a production Docker Compose with network-isolated sidecar + PSK, the `user_sessions` migration for server-side session tracking, and a `users` table migration replacing email/password with wallet_address.

---

## User Stories

- **As the platform operator**, I want the CCXT sidecar unreachable from the host and authenticated via PSK, so that only the Rust backend can execute exchange operations.
- **As a user**, I want my session revocable server-side, so that logout actually invalidates my tokens.
- **As the developer**, I want the users table to reflect wallet-primary identity, so that AUTH-02 can implement SIWE without hybrid email/wallet schema complexity.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Produce `docker-compose.production.yml` running: PostgreSQL 16, Router, DB Processor, WS-Stream, and CCXT sidecar | High | Docker |
| FR-2 | Define two Docker networks: `frontend` (host-exposed) and `internal` (no host ports) | High | Docker |
| FR-3 | Router joins both networks; WS-Stream joins `frontend`; sidecar, DB Processor, and PostgreSQL join only `internal` | High | Docker |
| FR-4 | Sidecar exposes NO ports to host — reachable only via Docker DNS on `internal` | High | Docker |
| FR-5 | Sidecar validates `X-Internal-Secret` header (from `SIDECAR_PSK` env var) on all non-health requests; returns 401 on mismatch | High | Sidecar |
| FR-6 | Add health checks: PostgreSQL (`pg_isready`), Router (`/api/v1/health`), WS-Stream (TCP 4000), sidecar (`/health`) | Medium | Docker |
| FR-7 | Create Dockerfile for `testudo-cex` (Bun runtime, copies `safe-cex-sub0` vendor dep from monorepo root) | High | Docker |
| FR-8 | Create `user_sessions` table: `id` (UUID PK), `user_id` (FK → users CASCADE), `refresh_token_hash` (VARCHAR 255), `ip_address` (VARCHAR 45), `user_agent` (TEXT), `is_revoked` (BOOLEAN DEFAULT FALSE), `expires_at` (TIMESTAMPTZ), `created_at`, `last_used_at` | High | Database |
| FR-9 | Indexes: `user_sessions(user_id)`, `user_sessions(refresh_token_hash)`, partial on `expires_at WHERE is_revoked = FALSE` | Medium | Database |
| FR-10 | Migration to transform `users` table: add `wallet_address VARCHAR(42) UNIQUE NOT NULL`, drop `email`, `password_hash`, `email_verified` columns, drop email-related indexes/constraints/trigger | High | Database |
| FR-11 | Router injects `X-Internal-Secret` header on all `CexClient` requests (`crates/router/src/services/cex_client.rs`) | High | Router |
| FR-12 | Remove Redis from production compose (deprecated per pg_queue) | Low | Docker |

---

## Technical Implementation

### 1. Docker Network Topology

```
                    ┌─── host ports ───┐
                    │  8080 (REST API)  │
                    │  4000 (WebSocket) │
                    └────────┬─────────┘
                             │
              ┌──────────────┼──────────────┐
              │         frontend net         │
              │                              │
         ┌────┴────┐                  ┌──────┴──────┐
         │ Router  │                  │  WS-Stream  │
         │  :8080  │                  │    :4000    │
         └────┬────┘                  └─────────────┘
              │
              │  (also on internal)
              │
   ┌──────────┼──────────────────────────────┐
   │          │        internal net           │
   │          │       (no host ports)         │
   │          │                               │
   │     ┌────┴────┐  ┌──────────┐  ┌───────┐│
   │     │DB Proc. │  │ Sidecar  │  │  PG   ││
   │     │  :8083  │  │  :3100   │  │ :5432 ││
   │     └─────────┘  └──────────┘  └───────┘│
   └──────────────────────────────────────────┘
```

### 2. Sidecar PSK Middleware

```typescript
// testudo-cex/src/middleware/psk.ts
import type { Request, Response, NextFunction } from "express";

const SIDECAR_PSK = process.env.SIDECAR_PSK;

export function pskGuard(req: Request, res: Response, next: NextFunction) {
  if (!SIDECAR_PSK) return next(); // dev mode: no PSK = open
  if (req.path === "/health") return next(); // health exempt
  if (req.headers["x-internal-secret"] !== SIDECAR_PSK) {
    return res.status(401).json({ error: "unauthorized" });
  }
  next();
}
```

### 3. Router PSK Injection

```rust
// crates/router/src/services/cex_client.rs — CexSidecarConfig
pub struct CexSidecarConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    pub psk: Option<String>, // NEW: from SIDECAR_PSK env var
}
```

Inject `X-Internal-Secret` header via reqwest default headers when `psk` is `Some`.

### 4. Sidecar Dockerfile

```dockerfile
# testudo-cex/Dockerfile — build context is monorepo root
FROM oven/bun:1-alpine
WORKDIR /app
COPY testudo-cex/package.json testudo-cex/bun.lock ./
COPY safe-cex-sub0/ /app/vendor/safe-cex-sub0/
RUN sed -i 's|file:../safe-cex-sub0|file:./vendor/safe-cex-sub0|' package.json
RUN bun install --frozen-lockfile
COPY testudo-cex/src/ ./src/
COPY testudo-cex/tsconfig.json ./
EXPOSE 3100
CMD ["bun", "run", "src/server.ts"]
```

### 5. Users Table Migration (Wallet-Primary)

```sql
-- 20260324000000_wallet_primary_users.up.sql

-- Add wallet_address as new identity column
ALTER TABLE users ADD COLUMN wallet_address VARCHAR(42);
CREATE UNIQUE INDEX idx_users_wallet_address ON users(wallet_address);

-- Drop email-based auth columns and infrastructure
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
DROP INDEX IF EXISTS idx_users_email;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_email_not_empty;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_email_format;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_password_hash_not_empty;
ALTER TABLE users DROP COLUMN IF EXISTS email;
ALTER TABLE users DROP COLUMN IF EXISTS password_hash;
ALTER TABLE users DROP COLUMN IF EXISTS email_verified;

-- Make wallet_address NOT NULL after migration
-- (existing rows must be handled — see Risks section)
ALTER TABLE users ALTER COLUMN wallet_address SET NOT NULL;

-- Add wallet address format constraint (0x + 40 hex chars)
ALTER TABLE users ADD CONSTRAINT check_wallet_address_format
    CHECK (wallet_address ~ '^0x[0-9a-fA-F]{40}$');
```

```sql
-- 20260324000000_wallet_primary_users.down.sql
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_wallet_address_format;
ALTER TABLE users ADD COLUMN email VARCHAR(255);
ALTER TABLE users ADD COLUMN password_hash VARCHAR(255);
ALTER TABLE users ADD COLUMN email_verified BOOLEAN DEFAULT FALSE;
DROP INDEX IF EXISTS idx_users_wallet_address;
ALTER TABLE users DROP COLUMN IF EXISTS wallet_address;
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
```

### 6. user_sessions Migration

```sql
-- 20260324000001_create_user_sessions.up.sql
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refresh_token_hash VARCHAR(255) NOT NULL,
    ip_address VARCHAR(45),
    user_agent TEXT,
    is_revoked BOOLEAN DEFAULT FALSE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(refresh_token_hash);
CREATE INDEX idx_user_sessions_active ON user_sessions(expires_at)
    WHERE is_revoked = FALSE;
```

```sql
-- 20260324000001_create_user_sessions.down.sql
DROP TABLE IF EXISTS user_sessions;
```

### Files

- `testudo-exchange/docker/docker-compose.production.yml` — **new**
- `testudo-exchange/docker/.env.production.example` — **new**
- `testudo-cex/Dockerfile` — **new**
- `testudo-cex/src/middleware/psk.ts` — **new**
- `testudo-cex/src/server.ts` — **modified** — mount pskGuard
- `testudo-exchange/crates/router/src/services/cex_client.rs` — **modified** — add psk field, inject header
- `testudo-exchange/crates/sqlx_postgres/migrations/20260324000000_wallet_primary_users.up.sql` — **new**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260324000000_wallet_primary_users.down.sql` — **new**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260324000001_create_user_sessions.up.sql` — **new**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260324000001_create_user_sessions.down.sql` — **new**

### Dependencies Added

- None

---

## Acceptance Criteria

- [ ] `docker compose -f docker-compose.production.yml up` starts all 5 services + PostgreSQL
- [ ] `curl http://localhost:8080/api/v1/health` returns 200 (router exposed)
- [ ] WebSocket connects on `ws://localhost:4000` (WS exposed)
- [ ] `curl http://localhost:3100/health` from host FAILS (sidecar not exposed)
- [ ] From router container: `curl http://exchange-sidecar:3100/health` returns 200
- [ ] Sidecar returns 401 without `X-Internal-Secret` header
- [ ] Sidecar returns 200 with correct `X-Internal-Secret` header
- [ ] `/health` exempt from PSK check
- [ ] `docker compose ps` shows all services `healthy`
- [ ] `sqlx migrate run` applies both migrations without error
- [ ] `users` table has `wallet_address VARCHAR(42) UNIQUE NOT NULL`, no `email`/`password_hash`/`email_verified`
- [ ] `user_sessions.user_id` FK references `users(id)` with CASCADE delete
- [ ] `sqlx migrate revert` reverts cleanly (both migrations)
- [ ] Existing dev `docker-compose.yml` still works
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `cd testudo-cex && bun test` passes

---

## Risks

1. **Existing user rows** — Current users table has email-based rows. The wallet_address migration adds a NOT NULL column. Mitigation: If existing test data exists, run a data migration script first (assign placeholder wallet addresses or truncate test data). This is acceptable since the platform is pre-launch with only the developer's test accounts.
2. **`safe-cex-sub0` vendor path** — Sidecar depends on `file:../safe-cex-sub0`. Mitigation: Build context is monorepo root; Dockerfile rewrites the path.
3. **Migration ordering** — `user_sessions` FK requires `users` table. Mitigation: Users migration is `000000`, sessions is `000001` — SQLx runs in filename order.
4. **Dev workflow** — Production compose is a separate file; existing dev flow untouched.

---

## Completion Signal

This spec is complete when:
1. Production Docker Compose starts all services with network isolation
2. Sidecar has zero host ports and validates PSK
3. Router injects PSK header on all sidecar requests
4. Users table is wallet-primary (no email/password columns)
5. `user_sessions` table exists with proper indexes
6. All acceptance criteria pass
7. Code committed to master
