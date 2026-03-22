# Specification: Replace Message Handler If/Else Chain with Typed Dispatch Map

**Spec ID:** EXT-37-message-dispatch-refactor
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / Extension
**Priority:** P1 — prerequisite for EXT-38 background decomposition
**Depends on:** None (api-dedup already merged)
**Series:** EXT-37 through EXT-38 (background.ts modularization)

---

## Problem Statement

The extension's message handler (`background.ts:806-969`) dispatches 27 runtime message types through a flat if/else chain spanning 163 lines. Each branch checks `msg.type === "..."` and returns a handler result.

This pattern has three problems:
1. **O(n) dispatch** — the 27th message type checks all 26 preceding conditions first.
2. **No isolation** — all handlers live in one monolithic block, making them hard to test individually.
3. **Side-effect coupling** — some branches include inline side effects (e.g., `LOGOUT` clears timers + disconnects WS + clears tokens; `ACCOUNT_LINKED` has a 500ms delay hack). These are invisible in the control flow.

A typed `Record<MessageType, Handler>` dispatch map gives O(1) lookup, makes each handler independently testable, and surfaces side effects as explicit handler functions.

---

## User Stories

- **As a developer**, I want message handlers to be individually addressable, so that I can test and modify them in isolation.
- **As a developer**, I want the dispatch to be a data structure rather than control flow, so that adding a new message type is a one-line addition rather than finding the right spot in a 163-line chain.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace the if/else chain with a `Record<string, MessageHandler>` dispatch map | High | background.ts |
| FR-2 | Each handler is a standalone function (named, not anonymous) | High | background.ts |
| FR-3 | Handler signature: `(msg: ParsedMessage) => Promise<unknown> \| unknown` | High | background.ts |
| FR-4 | Handlers with side effects (LOGOUT, TOKEN_SYNCED_FROM_WEB, ACCOUNT_LINKED, LOGIN, REGISTER, ADD/DELETE_EXCHANGE_ACCOUNT, SET_EXCHANGE_MODE, WS_STATUS) retain identical behavior | High | background.ts |
| FR-5 | Unknown message types return `undefined` (same as current behavior) | Medium | background.ts |
| FR-6 | Zero behavior change — all 27 message types produce identical responses | High | background.ts |

---

## Technical Implementation

### Handler Type

```typescript
type MessageHandler = (msg: z.infer<typeof RuntimeMessageSchema>) => Promise<unknown> | unknown;

const handlers: Record<string, MessageHandler> = {
  GET_SETTINGS: () => getSettings(),
  EXECUTE_TRADE: (msg) => executeTrade(msg.payload),
  LOGIN: (msg) => login(msg.email, msg.password).then(async (result) => {
    if (result.success) await ensureActiveExchange();
    return result;
  }),
  // ... all 27 types
};
```

### Dispatch

```typescript
browser.runtime.onMessage.addListener((message: unknown) => {
  const parsed = RuntimeMessageSchema.safeParse(message);
  if (!parsed.success) return undefined;

  const handler = handlers[parsed.data.type];
  return handler ? handler(parsed.data) : undefined;
});
```

### Side-Effect Handlers (require attention)

| Type | Side Effects | Notes |
|------|-------------|-------|
| `LOGIN` | `ensureActiveExchange()` after success | Chained `.then()` |
| `REGISTER` | `ensureActiveExchange()` after success | Same pattern as LOGIN |
| `LOGOUT` | Clear timer, disconnect WS, stop sidecar polling, clear tokens | Multi-step teardown |
| `ADD_EXCHANGE_ACCOUNT` | `ensureActiveExchange()` fire-and-forget | Note: no `await` |
| `DELETE_EXCHANGE_ACCOUNT` | `ensureActiveExchange()` awaited | Different from ADD |
| `TOKEN_SYNCED_FROM_WEB` | Schedule refresh, ensure exchange, debounce WS connect | Multi-step setup |
| `SET_EXCHANGE_MODE` | Write to storage, then `ensureActiveExchange()` | Storage + side effect |
| `ACCOUNT_LINKED` | 500ms delay, list accounts, write to storage, ensure exchange | Most complex handler |
| `WS_STATUS` | Auto-reconnect if disconnected | Conditional side effect |

### Files

- `testudo-extension/src/background.ts` — replace lines 806-969 with dispatch map

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] All 27 message types routed through dispatch map
- [ ] Each handler is a named function (not anonymous arrow in the map)
- [ ] Side-effect handlers produce identical behavior (LOGOUT teardown, ACCOUNT_LINKED delay, etc.)
- [ ] `RuntimeMessageSchema` parse failure returns `undefined`
- [ ] Unknown message types return `undefined`
- [ ] `bun run build` passes
- [ ] `bun run test` passes (same pre-existing failures)
- [ ] Message handler block reduced from 163 lines to <40 lines (map + dispatcher)

---

## Risks

1. **TypeScript narrowing** — The discriminated union narrows `msg` per type, but a generic handler function receives the union. Mitigation: cast within handlers or use type-narrowing helpers. The map values already know which type they handle from their key.
2. **Return type variance** — Some handlers return `Promise<BackendResponse>`, others `Promise<{ state: WsState }>`, etc. Mitigation: handler return type is `Promise<unknown> | unknown`, same as current implicit behavior.

---

## Completion Signal

This spec is complete when:
1. Dispatch map replaces the if/else chain
2. All acceptance criteria met
3. `bun run build && bun run test` passes
4. Code committed to master
