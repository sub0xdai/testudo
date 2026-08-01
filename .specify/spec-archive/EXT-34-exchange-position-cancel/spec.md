# Specification: Add Cancel Button to Exchange Position Cards

**Spec ID:** EXT-34-exchange-position-cancel
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / UX
**Priority:** P1 — users cannot close untracked exchange positions from the extension
**Depends on:** None
**Series:** EXT-34 through EXT-36 (extension UX polish)

---

## Problem Statement

When positions show under "FROM EXCHANGE" in the extension popup, there is no way to close them from within the extension. The user must switch to the exchange's web interface (e.g. WOO X) to manage or close these positions. This breaks the workflow — the extension is supposed to be the single control surface.

Tracked positions (via PositionCard) already have cancel/close functionality. The same capability is needed for exchange-sourced positions displayed in ActiveOrders.tsx.

---

## User Stories

- **As a trader**, I want to close exchange positions directly from the extension, so that I don't need to switch to the exchange website.
- **As a trader**, I want a confirmation step before closing a position, so that I don't accidentally close it.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Each exchange position card displays a "Close" button | High | ActiveOrders.tsx |
| FR-2 | Clicking "Close" sends a market close order via the background worker | High | background.ts |
| FR-3 | A confirmation step prevents accidental closures (e.g. "Close 0.0087 BTC_USDT?") | High | ActiveOrders.tsx |
| FR-4 | Loading state shown while the close order is in flight | Medium | ActiveOrders.tsx |
| FR-5 | Success/error feedback displayed after close attempt | Medium | ActiveOrders.tsx |
| FR-6 | Position list auto-refreshes after successful close | Medium | ActiveOrders.tsx |

---

## Technical Implementation

### New Message Type

Add `CLOSE_EXCHANGE_POSITION` to RuntimeMessageSchema:

```typescript
z.object({
  type: z.literal("CLOSE_EXCHANGE_POSITION"),
  symbol: z.string(),
  side: z.enum(["long", "short"]),
  contracts: z.string(),
})
```

### Background Handler

The handler places a market order in the opposite direction with `reduceOnly: true`:

- Long position → sell market, reduceOnly
- Short position → buy market, reduceOnly

Route through existing `trade_manager_live.place_order()` or directly via `cex_client.create_order()`.

### UI — Button Placement

Add a footer row to each exchange position card matching the PositionCard pattern:

```tsx
<div class="flex items-center justify-end mt-2 pt-2 border-t border-border-subtle">
  <button
    class="px-3 py-1.5 text-[10px] font-bold tracking-wider text-signal-red
           bg-signal-red/10 rounded-md hover:bg-signal-red/20 transition-colors"
    onClick={() => confirmClose(pos)}
  >
    CLOSE
  </button>
</div>
```

### Files

- `testudo-extension/src/popup/components/ActiveOrders.tsx` — add Close button to exchange position cards
- `testudo-extension/src/schemas.ts` — add CLOSE_EXCHANGE_POSITION message type
- `testudo-extension/src/background.ts` — add handler for CLOSE_EXCHANGE_POSITION

---

## Acceptance Criteria

- [ ] Each "FROM EXCHANGE" position card has a "CLOSE" button
- [ ] Clicking CLOSE shows a confirmation prompt with symbol and size
- [ ] Confirming sends a reduce-only market order via the sidecar
- [ ] Loading spinner shown during order execution
- [ ] Success feedback: position disappears on next refresh
- [ ] Error feedback: toast or inline error shown
- [ ] `bun run build` passes

---

## Risks

1. **Exchange rejects reduce-only market** — some exchanges require specific order types for position closure. Mitigation: use the exchange's native close-position mechanism if available.
2. **Partial close** — if the market order partially fills, position remains. Mitigation: auto-refresh shows remaining size.

---

## Completion Signal

This spec is complete when:
1. Close button appears on all FROM EXCHANGE position cards
2. Market close orders execute successfully on WOO
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
