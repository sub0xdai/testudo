# Implementation Plan

> Last updated: 2026-03-22
> Current spec: EXT-38-background-decomposition
> Phase: PLANNING

---

## Active Spec: EXT-38-background-decomposition

Decompose 1043-line `background.ts` monolith into 6 focused modules + thin bootstrap.

### Key Discoveries (Gap Analysis)

1. **Circular dep: auth ↔ api** — `doRefresh()` was migrated to use `apiRequest()` (commit 27728c2), but `apiRequest()` calls `refreshAccessToken()` from auth. Since refresh uses `auth: "none"` (just a fetch wrapper), solution: revert `doRefresh` to raw `fetch` + `getSettings()`.
2. **Circular dep: storage ↔ api** — Spec places `ensureActiveExchange()` and `migrateActiveExchangeId()` in storage.ts, but both call `listExchangeAccounts()` from api.ts, while `apiRequest()` calls `getSettings()` from storage.ts. Solution: move both to api.ts.
3. **`getAuthStatus` placement** — Spec says storage.ts, but it only uses `getTokens()` + JWT parsing. Belongs in auth.ts.
4. **Hoisting no longer needed** — EXT-37 used function declarations for handler hoisting (handlers referenced `debouncedConnectWebSocket` defined below). In EXT-38, handlers.ts will import `debouncedConnectWebSocket` from websocket.ts — arrow functions are fine.
5. **Side-effectful code** — Tab listeners (`browser.tabs.onCreated/onRemoved`) invalidate `cachedContentTabs` in websocket.ts. These can register at import time (no init function needed) since the cache variable is module-scoped.
6. **Test import path** — `background.test.ts` uses `await import("./background")` and captures `onMessage.addListener`. Bootstrap still registers this listener, so tests work unchanged. `_disconnectWebSocket` export re-exported from bootstrap.

### Corrected Dependency Graph (Acyclic)

```
storage.ts  (leaf — no internal deps)
auth.ts     → storage.ts (getSettings for raw-fetch refresh URL)
api.ts      → auth.ts, storage.ts
websocket.ts → auth.ts, storage.ts
sidecar.ts  → api.ts
handlers.ts → api.ts, auth.ts, storage.ts, websocket.ts, sidecar.ts
background.ts → handlers.ts, api.ts, auth.ts, websocket.ts, sidecar.ts
```

### Corrected Module Map (vs Spec)

| Module | Functions | Deviation from Spec |
|--------|-----------|---------------------|
| storage.ts | `getSettings`, `getExchangeMode`, `getActiveExchangeId`, `setActiveExchangeId` | Removed `ensureActiveExchange`, `migrateActiveExchangeId`, `getAuthStatus` |
| auth.ts | `getTokens`, `storeTokens`, `clearTokens`, `refreshAccessToken`, `doRefresh`, `scheduleTokenRefresh`, `getAuthStatus` | Added `getAuthStatus`; `doRefresh` uses raw fetch |
| api.ts | `apiRequest`, `normalizeBackendAck`, `normalizeTradeListResponse`, `authenticate`, `login`, `register`, `forgotPassword`, `executeTrade`, `listTrades`, `cancelTrade`, `cleanupTrades`, `listExchanges`, `listExchangeAccounts`, `addExchangeAccount`, `deleteExchangeAccount`, `testExchangeConnection`, `getLiveBalance`, `fetchExchangePositions`, `closeExchangePosition`, `ensureActiveExchange`, `migrateActiveExchangeId` | Added `ensureActiveExchange`, `migrateActiveExchangeId` |
| websocket.ts | `connectWebSocket`, `disconnectWebSocket`, `debouncedConnectWebSocket`, `getWsState`, `getWsReconnectTimer`, `resetReconnectDelay`, `onSidecarHealth` | Added state accessors + sidecar health callback setter; internal: `getUserId`, `setWsState`, `scheduleReconnect`, `getContentTabs`, `forwardOrderUpdate` |
| sidecar.ts | `setSidecarStatus`, `checkSidecarHealth`, `startSidecarHealthPolling`, `stopSidecarHealthPolling` | No change |
| handlers.ts | 28 `handle*` functions + `messageHandlers` dispatch map | No change |

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Extract `src/background/storage.ts` — 4 storage/settings functions | complete | simple | — |
| T2 | Extract `src/background/auth.ts` — 7 auth functions, refactor `doRefresh` to raw fetch | complete | medium | T1 |
| T3 | Extract `src/background/api.ts` — 21 API functions + normalizers + exchange helpers, types | complete | medium | T1, T2 |
| T4 | Extract `src/background/websocket.ts` — 8 WS functions + tab cache + listeners | complete | medium | T1, T2 |
| T5 | Extract `src/background/sidecar.ts` — 4 sidecar functions + SidecarStatus type | complete | simple | T3 |
| T6 | Extract `src/background/handlers.ts` — 28 handlers + dispatch map + types | complete | medium | T1-T5 |
| T7 | Reduce `src/background.ts` to bootstrap (<100 lines) | complete | simple | T1-T6 |
| T8 | Verify build + test, fix any issues | complete | simple | T7 |

### State Variable Placement

| Variable | Target Module |
|----------|---------------|
| `refreshInFlight` | auth.ts |
| `refreshTimer` | auth.ts |
| `tradeInFlight` | api.ts |
| `sidecarStatus` | sidecar.ts |
| `sidecarHealthTimer` | sidecar.ts |
| `sidecarHealthInitialTimer` | sidecar.ts |
| `ws` | websocket.ts |
| `wsState` | websocket.ts |
| `wsReconnectDelay` | websocket.ts |
| `wsReconnectTimer` | websocket.ts |
| `wsSubscriptionId` | websocket.ts |
| `cachedContentTabs` | websocket.ts |
| `wsDebounceTimer` | websocket.ts |

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HL-11-status-transition-fix | 2026-03-21 |
| UXP-18-multi-theme | 2026-03-21 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
