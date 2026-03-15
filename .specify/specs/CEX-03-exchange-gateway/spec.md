# Specification: ExchangeGateway — safe-cex Instance Management

**Spec ID:** CEX-03-exchange-gateway
**Date:** 2026-03-15
**Status:** Draft
**Class:** Core / Infrastructure
**Priority:** P1 — all handlers depend on this
**Depends on:** CEX-01 (fork), CEX-02 (scaffold)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

The old sidecar (`testudo-ccxt`) used a simple pool to cache CCXT exchange instances (`pool.getOrCreate()`). The new sidecar needs a similar gateway that manages **stateful** safe-cex exchange instances — each instance maintains persistent WebSocket connections and an internal reactive Store.

Unlike CCXT instances (stateless HTTP wrappers), safe-cex instances are long-lived and event-driven. The gateway must handle lifecycle: creation, WebSocket connection, fill event wiring, and graceful disposal.

---

## User Stories

- **As the sidecar**, I want to create and cache safe-cex instances per (exchange_id, api_key, sandbox) tuple, so that WebSocket connections persist across requests.
- **As the sidecar**, I want fill events from safe-cex routed to a callback, so that fill streaming (CEX-05) can forward them to the Rust backend.
- **As the sidecar**, I want graceful disposal of exchange instances on shutdown, so that WebSocket connections are cleaned up.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Cache key derived from `hash(exchange_id + api_key + sandbox)` | High | Gateway |
| FR-2 | `getOrCreate()` returns existing instance if cache key exists | High | Gateway |
| FR-3 | `getOrCreate()` creates new safe-cex instance via `createExchange()` if not cached | High | Gateway |
| FR-4 | Wire `exchange.on("fill", onFill)` callback during creation | High | Gateway |
| FR-5 | Wire `exchange.on("error", ...)` and `exchange.on("log", ...)` for observability | Medium | Gateway |
| FR-6 | Call `exchange.start()` to establish WebSocket connections and fetch initial state | High | Gateway |
| FR-7 | `dispose(cacheKey)` stops exchange instance and removes from cache | Medium | Gateway |
| FR-8 | `disposeAll()` stops all instances (for graceful shutdown) | Medium | Gateway |
| FR-9 | Handle `exchange.start()` failures gracefully (log error, don't cache failed instance) | High | Gateway |

---

## Technical Implementation

### ExchangeGateway Class

**File:** `testudo-cex/src/gateway.ts`

```typescript
import { createExchange, type Exchange } from "safe-cex";
import crypto from "crypto";

export interface FillEvent {
  amount: number;
  price: number;
  side: "buy" | "sell";
  symbol: string;
}

export interface Credentials {
  key: string;
  secret: string;
  applicationId?: string;  // WOO X
  passphrase?: string;     // OKX, Bitget, Blofin
}

export class ExchangeGateway {
  private instances: Map<string, Exchange> = new Map();

  private cacheKey(exchangeId: string, apiKey: string, sandbox: boolean): string {
    return crypto
      .createHash("sha256")
      .update(`${exchangeId}:${apiKey}:${sandbox}`)
      .digest("hex")
      .slice(0, 16);
  }

  async getOrCreate(
    exchangeId: string,
    credentials: Credentials,
    sandbox: boolean,
    onFill: (fill: FillEvent) => void
  ): Promise<Exchange> {
    const key = this.cacheKey(exchangeId, credentials.key, sandbox);

    const existing = this.instances.get(key);
    if (existing) return existing;

    const exchange = createExchange(exchangeId as any, {
      key: credentials.key,
      secret: credentials.secret,
      applicationId: credentials.applicationId,
      testnet: sandbox,
    });

    exchange.on("fill", onFill);
    exchange.on("error", (err) => console.error(`[${exchangeId}] error:`, err));
    exchange.on("log", (msg, severity) =>
      console.log(`[${exchangeId}] ${severity}:`, msg)
    );

    try {
      await exchange.start();
    } catch (err) {
      console.error(`[${exchangeId}] start() failed:`, err);
      throw err;
    }

    this.instances.set(key, exchange);
    return exchange;
  }

  async dispose(key: string): Promise<void> {
    const instance = this.instances.get(key);
    if (instance) {
      // safe-cex cleanup
      this.instances.delete(key);
    }
  }

  async disposeAll(): Promise<void> {
    for (const [key] of this.instances) {
      await this.dispose(key);
    }
  }

  get size(): number {
    return this.instances.size;
  }
}
```

### Integration with Request Handlers

HTTP handlers call `gateway.getOrCreate()` on each request, passing credentials from the request envelope. The cached instance is returned for subsequent requests with the same credentials.

### Key Design Decision

**Stateful gateway vs stateless proxy:** Unlike the old CCXT pool, safe-cex instances maintain WebSocket connections. The `exchange.on("fill", ...)` callback fires for BOTH regular and algo order fills on WOO X because safe-cex subscribes to both `executionreport` and `algoexecutionreportv2` WebSocket topics internally.

---

## Acceptance Criteria

- [ ] Gateway creates and caches exchange instances by (exchange_id, api_key, sandbox) tuple
- [ ] Duplicate `getOrCreate()` calls return the same instance
- [ ] `exchange.start()` establishes WebSocket connections
- [ ] Fill events fire via the `onFill` callback
- [ ] Error and log events are forwarded to console
- [ ] `dispose()` removes instance from cache
- [ ] `disposeAll()` cleans up all instances
- [ ] Failed `start()` does not cache the broken instance

---

## Completion Signal

This spec is complete when:
1. ExchangeGateway class implemented and tested
2. Unit tests cover: create, cache hit, dispose, start failure
3. Changes committed to master
