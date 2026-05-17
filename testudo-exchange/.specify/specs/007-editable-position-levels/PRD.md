# PRD: Editable Position Levels

| Metadata | Details |
|----------|---------|
| **Project** | Testudo Exchange |
| **Component** | Trading Interface (Frontend + Backend) |
| **Status** | Ready for Implementation |
| **Priority** | High |
| **Spec ID** | 007-editable-position-levels |
| **Owner** | Engineering |

## 1. Executive Summary

Users need the ability to adjust position levels (Entry, Stop Loss, Take Profit) directly on the chart by dragging handles, even after a trade has been submitted. Currently, existing positions render as static canvas primitives without interactive handles, forcing users to cancel and recreate orders to adjust levels.

This feature adds editable handles to existing positions, with appropriate locking (entry locked for filled orders) and API endpoints to persist changes.

---

## 2. User Story

> As a trader, I want to adjust my pending order's entry price and SL/TP levels by dragging handles on the chart, so I can fine-tune my position without canceling and recreating it.

---

## 3. Current State

```
PositionDrawingTool (for NEW positions)
├── PositionHandleOverlay (DOM: interactive handles) ✅
└── PositionZonePrimitive (Canvas: zones/lines) ✅

OpenPositionsLayer (for EXISTING positions)
└── PositionZonePrimitive only ❌ NO HANDLES
```

**Problem:** `OpenPositionsLayer` renders positions as static canvas primitives without the interactive `PositionHandleOverlay` component.

---

## 4. Target State

```
OpenPositionsLayer (for EXISTING positions)
├── PositionZonePrimitive (Canvas: zones/lines) ✅ existing
└── PositionHandleOverlay (DOM: interactive handles) 🆕 ADD THIS
```

Reuse the existing `PositionHandleOverlay` component with new props:
- `lockedHandles` - For locking entry on filled positions
- `isExistingPosition` - To hide execute button, show apply/cancel

---

## 5. Functional Requirements

### FR-5.1: Pending Order Handle Behavior

| Requirement | Description |
|-------------|-------------|
| FR-5.1.1 | Entry price handle is draggable |
| FR-5.1.2 | Stop Loss handle is draggable |
| FR-5.1.3 | Take Profit handle is draggable |
| FR-5.1.4 | Dragging calls appropriate API to persist change |
| FR-5.1.5 | Position updates visually after API success |

**Acceptance Criteria:**
- User can drag all three handles for pending orders
- API call made on drag release
- Toast notification on success/error
- Canvas primitive updates to reflect new levels

---

### FR-5.2: Filled Order Handle Behavior

| Requirement | Description |
|-------------|-------------|
| FR-5.2.1 | Entry price handle is locked (not draggable) |
| FR-5.2.2 | Stop Loss handle is draggable |
| FR-5.2.3 | Take Profit handle is draggable |
| FR-5.2.4 | Entry handle shows lock icon and cursor: not-allowed |

**Acceptance Criteria:**
- Entry handle displays lock icon
- Entry handle cursor is `not-allowed`
- Attempting to drag entry does nothing
- SL/TP remain fully draggable

---

### FR-5.3: UX Requirements

| Requirement | Description |
|-------------|-------------|
| FR-5.3.1 | Positions auto-enter edit mode when appearing on chart |
| FR-5.3.2 | Handles reposition correctly on chart zoom/pan |
| FR-5.3.3 | Visual feedback during drag (handle follows cursor) |
| FR-5.3.4 | Toast notification on API error |
| FR-5.3.5 | Escape key cancels pending changes |
| FR-5.3.6 | Enter key applies pending changes |

---

### FR-5.4: Entry Price Update API (Backend)

| Requirement | Description |
|-------------|-------------|
| FR-5.4.1 | New endpoint: `PUT /api/v1/trades/{id}/entry` |
| FR-5.4.2 | Request body: `{ price: Decimal }` |
| FR-5.4.3 | Validate trade status is "Pending" |
| FR-5.4.4 | Validate price relationship (entry > SL for longs) |
| FR-5.4.5 | Cancel existing entry order |
| FR-5.4.6 | Create new order at new price |
| FR-5.4.7 | Update trade group's entry_order_id |
| FR-5.4.8 | Return updated TradeGroupResponse |

**Acceptance Criteria:**
```rust
// Request
PUT /api/v1/trades/{id}/entry
{ "price": 95000.0 }

// Success Response (status="Pending")
{ "success": true, "data": { ...updatedTradeGroup } }

// Error Response (status="Active")
{ "success": false, "error": "Entry price can only be modified for pending orders" }
```

---

## 6. API Endpoints Summary

| Endpoint | Purpose | Status |
|----------|---------|--------|
| `PUT /trades/{id}/sl` | Update stop loss | ✅ EXISTS |
| `PUT /trades/{id}/tp` | Update take profit | ✅ EXISTS |
| `PUT /trades/{id}/entry` | Update entry price | 🆕 REQUIRED |

---

## 7. Files to Modify

### Backend (Rust)

| File | Changes |
|------|---------|
| `crates/router/src/routes/trade_management.rs` | Add `UpdateEntryPriceRequest`, `update_entry_price` handler |
| `crates/router/src/main.rs` | Register route `/{id}/entry` |
| `crates/engine/src/shadow/order_group.rs` | Add `update_entry_order` method to OrderGroupManager |

### Frontend (TypeScript/React)

| File | Changes |
|------|---------|
| `apps/web/src/utils/requests.ts` | Add `updateEntryPrice()` function |
| `apps/web/src/components/chart/PositionHandleOverlay.tsx` | Add `lockedHandles`, `isExistingPosition` props |
| `apps/web/src/components/chart/OpenPositionsLayer.tsx` | Add edit state, render PositionHandleOverlay |

---

## 8. Validation Rules

### Entry Price Update (Backend)

```rust
// For LONG positions
if new_entry_price <= stop_loss_price {
    return Err("Entry price must be above stop loss for long positions");
}

// For SHORT positions
if new_entry_price >= stop_loss_price {
    return Err("Entry price must be below stop loss for short positions");
}
```

### Status Check

```rust
if group.status != OrderGroupStatus::Pending {
    return Err("Entry price can only be modified for pending orders");
}
```

---

## 9. Edge Cases

| Scenario | Handling |
|----------|----------|
| User edits while position fills | Backend checks status; returns error if filled |
| Multiple tabs editing same position | Last-write-wins; refresh shows actual state |
| Entry update fails after cancel | Return error; user can create new position |
| Drag entry past SL | Validate on release, show error toast, revert visual |

---

## 10. Test Scenarios

| ID | Scenario | Expected Result |
|----|----------|-----------------|
| T1 | Pending order - drag entry | API called, position updates |
| T2 | Pending order - drag SL | API called, position updates |
| T3 | Pending order - drag TP | API called, position updates |
| T4 | Filled order - try drag entry | Handle doesn't move, shows lock |
| T5 | Filled order - drag SL | API called, position updates |
| T6 | Filled order - drag TP | API called, position updates |
| T7 | API error during update | Toast shows error, position reverts |
| T8 | Chart zoom/pan during edit | Handles reposition correctly |
| T9 | Escape during edit | Pending changes cancelled |
| T10 | Enter during edit | Changes applied via API |

---

## 11. Out of Scope

- Multiple position selection/editing
- Keyboard shortcuts for level adjustment (arrow keys)
- Undo/redo for level changes
- Break-even automation UI integration

---

## 12. Verification

All changes must pass:
1. `cargo test` - All existing tests + new entry update tests
2. `cargo clippy --all-targets` - No errors
3. Frontend: Manual testing of all 10 test scenarios
4. Frontend: Verify handles render on chart pan/zoom
