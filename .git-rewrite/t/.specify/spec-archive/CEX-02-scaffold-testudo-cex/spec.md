# Specification: Archive testudo-ccxt, Scaffold testudo-cex

**Spec ID:** CEX-02-scaffold-testudo-cex
**Date:** 2026-03-15
**Status:** Complete
**Class:** Infrastructure / Migration
**Priority:** P1 — prerequisite for CEX-03+
**Depends on:** CEX-01 (fork safe-cex)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

The existing `testudo-ccxt/` sidecar is a Node.js/Express application using CCXT. It needs to be archived and replaced with a new `testudo-cex/` sidecar built on the safe-cex fork. The new sidecar uses Bun as runtime (already used in testudo-web and testudo-extension) and TypeScript.

---

## User Stories

- **As a developer**, I want the old sidecar archived (not deleted), so that I can reference it during migration.
- **As a developer**, I want a clean scaffold for testudo-cex with TypeScript/Bun, so that I can build the new sidecar incrementally.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Archive `testudo-ccxt/` to `testudo-ccxt-archived/` via `git mv` | High | Git |
| FR-2 | Create `testudo-cex/` directory with TypeScript project scaffold | High | Scaffold |
| FR-3 | `package.json` with safe-cex (forked), express, ws, prom-client dependencies | High | Config |
| FR-4 | TypeScript config targeting Bun runtime | High | Config |
| FR-5 | Express + WS server entry point on port 3100 (same as old sidecar) | High | Server |
| FR-6 | `GET /health` returns `{ok: true}` | High | Server |
| FR-7 | Stub files for gateway, handlers, ws-fills, reconciler, types, symbols, metrics | Medium | Scaffold |
| FR-8 | `bun install` and `bun run start` work | High | Build |

---

## Technical Implementation

### 1) Archive Old Sidecar (FR-1)

```bash
git mv testudo-ccxt testudo-ccxt-archived
```

### 2) Scaffold Directory (FR-2, FR-7)

```
testudo-cex/
├── package.json
├── tsconfig.json
├── vendor/
│   └── safe-cex/          # submodule from CEX-01
├── src/
│   ├── server.ts           # Express + WS server (port 3100)
│   ├── gateway.ts          # ExchangeGateway — manages safe-cex instances
│   ├── handlers.ts         # HTTP route handlers (same endpoints as before)
│   ├── ws-fills.ts         # WebSocket fill streaming to Rust backend
│   ├── reconciler.ts       # Polling fallback — orphaned order safety loop
│   ├── types.ts            # Shared types, request/response shapes
│   ├── symbols.ts          # Symbol normalization (BTC_USDT <-> BTCUSDT)
│   └── metrics.ts          # Prometheus metrics
└── tests/
    └── *.test.ts
```

### 3) Server Entry Point (FR-5, FR-6)

```typescript
// src/server.ts
import express from "express";
import { createServer } from "http";
import { WebSocketServer } from "ws";

const app = express();
app.use(express.json());

app.get("/health", (_req, res) => {
  res.json({ ok: true });
});

const PORT = process.env.PORT || 3100;
const server = createServer(app);
const wss = new WebSocketServer({ server, path: "/ws/orders" });

server.listen(PORT, () => {
  console.log(`testudo-cex listening on port ${PORT}`);
});
```

### 4) Package Configuration (FR-3, FR-4)

```json
{
  "name": "testudo-cex",
  "version": "0.1.0",
  "scripts": {
    "start": "bun run src/server.ts",
    "build": "bun build src/server.ts --outdir=dist --target=bun",
    "test": "bun test"
  },
  "dependencies": {
    "express": "^4.21.0",
    "ws": "^8.18.0",
    "prom-client": "^15.1.0"
  },
  "devDependencies": {
    "@types/express": "^5.0.0",
    "@types/ws": "^8.5.0",
    "typescript": "^5.7.0"
  }
}
```

---

## Acceptance Criteria

- [x] `testudo-ccxt-archived/` exists with full git history preserved
- [x] `testudo-cex/` directory exists with all stub files
- [x] `bun install` succeeds
- [x] `bun run start` launches server on port 3100
- [x] `GET /health` returns `{"ok": true}`
- [x] Old sidecar preserved in archive directory

---

## Completion Signal

This spec is complete when:
1. Old sidecar archived
2. New scaffold builds and starts
3. Health endpoint responds
4. Changes committed to master
