# Specification: Decompose Background Service Worker into Focused Modules

**Spec ID:** EXT-38-background-decomposition
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / Extension
**Priority:** P1 — reduces cognitive load of 1005-line monolith
**Depends on:** EXT-37-message-dispatch-refactor
**Series:** EXT-37 through EXT-38 (background.ts modularization)

---

## Problem Statement

After api-dedup (1368→1005 lines) and the upcoming EXT-37 dispatch refactor, `background.ts` will still be a single file containing 6 unrelated concerns: HTTP API calls, WebSocket lifecycle, auth/token management, storage helpers, sidecar health polling, and message dispatch. These concerns have minimal coupling — they share `getSettings()` and `getTokens()` but are otherwise independent.

A 1000+ line service worker is hard to navigate, hard to review, and hard to test in isolation. Decomposing into focused modules improves all three.

This spec depends on EXT-37 because the dispatch map must exist before we can extract handlers into a separate module — otherwise we'd be moving the if/else chain, which is the wrong abstraction to modularize.

---

## User Stories

- **As a developer**, I want each concern in its own file, so that I can find and modify code without scrolling through 1000 lines.
- **As a developer**, I want to test API functions without loading WebSocket or auth code, so that tests are faster and more focused.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extract HTTP API functions into `src/background/api.ts` | High | Extension |
| FR-2 | Extract WebSocket lifecycle into `src/background/websocket.ts` | High | Extension |
| FR-3 | Extract auth/token management into `src/background/auth.ts` | High | Extension |
| FR-4 | Extract storage helpers into `src/background/storage.ts` | Medium | Extension |
| FR-5 | Extract sidecar health polling into `src/background/sidecar.ts` | Medium | Extension |
| FR-6 | Extract dispatch map into `src/background/handlers.ts` | High | Extension |
| FR-7 | `src/background.ts` becomes a thin bootstrap (~50-80 lines): imports, wiring, startup | High | Extension |
| FR-8 | All exports are internal — no public API changes | High | Extension |
| FR-9 | esbuild continues to bundle from `src/background.ts` entrypoint | High | Build |

---

## Technical Implementation

### Module Map

```
src/background/
├── api.ts          # apiRequest(), authenticate(), all API wrappers
│                   # ~250 lines (from api-dedup helper + thin wrappers)
│
├── auth.ts         # getTokens(), storeTokens(), clearTokens(),
│                   # refreshAccessToken(), doRefresh(), scheduleTokenRefresh()
│                   # ~80 lines
│
├── storage.ts      # getSettings(), getActiveExchangeId(), setActiveExchangeId(),
│                   # ensureActiveExchange(), migrateActiveExchangeId(),
│                   # getExchangeMode(), getAuthStatus()
│                   # ~120 lines
│
├── websocket.ts    # connectWebSocket(), disconnectWebSocket(),
│                   # debouncedConnectWebSocket(), forwardOrderUpdate(),
│                   # getContentTabs(), WS state management
│                   # ~150 lines
│
├── sidecar.ts      # checkSidecarHealth(), startSidecarHealthPolling(),
│                   # stopSidecarHealthPolling(), setSidecarStatus()
│                   # ~40 lines
│
└── handlers.ts     # Dispatch map (from EXT-37), handler functions
                    # ~80 lines

src/background.ts   # Bootstrap: imports, onMessage listener, onInstalled,
                    # startup sequence, storage.onChanged listener
                    # ~50-80 lines
```

### Dependency Flow

```
background.ts (bootstrap)
  ├── handlers.ts (dispatch map)
  │     ├── api.ts (apiRequest + wrappers)
  │     │     ├── auth.ts (tokens, refresh)
  │     │     └── storage.ts (settings, exchange ID)
  │     ├── auth.ts
  │     ├── storage.ts
  │     ├── websocket.ts
  │     └── sidecar.ts
  ├── websocket.ts (startup connect)
  ├── auth.ts (startup token check)
  ├── storage.ts (startup migration)
  └── sidecar.ts (startup polling)
```

### Shared State

These module-level variables need careful placement:

| Variable | Current Location | Target Module | Notes |
|----------|-----------------|---------------|-------|
| `ws`, `wsState`, `wsReconnectDelay`, `wsReconnectTimer` | background.ts | websocket.ts | WS lifecycle |
| `refreshInFlight`, `refreshTimer` | background.ts | auth.ts | Token refresh |
| `tradeInFlight` | background.ts | api.ts | Trade dedup guard |
| `sidecarStatus`, `sidecarHealthTimer` | background.ts | sidecar.ts | Health polling |
| `cachedContentTabs` | background.ts | websocket.ts | Tab cache for forwarding |
| `wsDebounceTimer` | background.ts | websocket.ts | Reconnect debounce |

### Build Impact

esbuild bundles from `src/background.ts` as entrypoint. Since all new modules are imported from the entrypoint, esbuild will follow the import graph and bundle everything into a single `background.js`. No build config changes needed.

### Files

- `src/background/api.ts` — NEW
- `src/background/auth.ts` — NEW
- `src/background/storage.ts` — NEW
- `src/background/websocket.ts` — NEW
- `src/background/sidecar.ts` — NEW
- `src/background/handlers.ts` — NEW
- `src/background.ts` — MODIFIED (reduced to bootstrap)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `src/background.ts` is <100 lines (bootstrap only)
- [ ] 6 new modules created in `src/background/`
- [ ] No circular imports between modules
- [ ] `bun run build` produces identical `background.js` output (functionally)
- [ ] `bun run test` passes (same pre-existing failures)
- [ ] No new exports added to the extension's public surface
- [ ] esbuild entrypoint unchanged (`src/background.ts`)
- [ ] All module-level state variables placed in the correct module

---

## Risks

1. **Circular imports** — `api.ts` needs `auth.ts` (for refresh), `auth.ts` needs `storage.ts` (for token persistence), `handlers.ts` needs everything. Mitigation: dependency flow is acyclic by design (see diagram above). `auth.ts` does NOT import `api.ts`.
2. **Test imports break** — `background.test.ts` imports from `background.ts`. Mitigation: re-export test-facing functions from `background.ts` or update test imports. Keep the `_disconnectWebSocket` export.
3. **Module-level initialization order** — Some modules run code at import time (e.g., `browser.tabs.onCreated.addListener`). Mitigation: move all side-effectful initialization to explicit `init()` functions called from bootstrap.

---

## Completion Signal

This spec is complete when:
1. All 6 modules extracted and `background.ts` is bootstrap-only
2. All acceptance criteria met
3. `bun run build && bun run test` passes
4. Code committed to master
