# 007-editable-position-levels Progress

**Started:** 2026-01-22
**Last Updated:** 2026-01-22
**Status:** COMPLETE

---

## Phase 1: Backend - Entry Update Endpoint

### FR-5.4: Entry Price Update API
- [x] FR-5.4.1: Add `UpdateEntryPriceRequest` struct
- [x] FR-5.4.2: Add `update_entry_price` handler
- [x] FR-5.4.3: Validate trade status is "Pending"
- [x] FR-5.4.4: Validate price relationship (entry > SL for longs)
- [x] FR-5.4.5: Cancel existing entry order
- [x] FR-5.4.6: Create new order at new price
- [x] FR-5.4.7: Update trade group's entry_order_id
- [x] FR-5.4.8: Register route `PUT /{id}/entry`
- [x] FR-5.4.9: Add `update_entry_order` method to OrderGroupManager

**Phase 1 Tests:** `cargo test` passing? [x]
**Phase 1 Clippy:** `cargo clippy --all-targets` clean? [x]

---

## Phase 2: Frontend - API Function

### requests.ts
- [x] Add `updateEntryPrice()` function
- [x] Follow existing pattern (updateStopLoss, updateTakeProfit)
- [x] Error handling with proper messages

**Phase 2 Verified:** Manual test in browser? [x]

---

## Phase 3: Frontend - PositionHandleOverlay Props

### PositionHandleOverlay.tsx
- [x] Add `lockedHandles?: HandleType[]` prop
- [x] Add `isExistingPosition?: boolean` prop
- [x] Add `positionId?: string` prop
- [x] Update Handle component for locked state
  - [x] `cursor: 'not-allowed'` when locked
  - [x] No onMouseDown when locked
  - [x] Lock icon SVG for locked handles
  - [x] Reduced opacity (0.6) when locked

**Phase 3 Verified:** Locked handles display correctly? [x]

---

## Phase 4: Frontend - OpenPositionsLayer Integration

### OpenPositionsLayer.tsx
- [x] Add `editingPositionId` state
- [x] Add auto-edit for new positions (track previous IDs)
- [x] Add `handleLevelChange` callback
  - [x] Call updateEntryPrice for entry changes
  - [x] Call updateStopLoss for SL changes
  - [x] Call updateTakeProfit for TP changes
  - [x] Toast success/error notifications
  - [x] Refresh positions after update
- [x] Render `PositionHandleOverlay` for editing position
  - [x] Pass `lockedHandles={['entry']}` for Active positions
  - [x] Pass `isExistingPosition={true}`

**Phase 4 Verified:**
- [x] Pending order: all handles draggable?
- [x] Active order: entry locked, SL/TP draggable?
- [x] API calls on drag release?
- [x] Toast notifications working?

---

## Test Scenarios

| ID | Scenario | Status |
|----|----------|--------|
| T1 | Pending order - drag entry | [x] |
| T2 | Pending order - drag SL | [x] |
| T3 | Pending order - drag TP | [x] |
| T4 | Filled order - try drag entry | [x] |
| T5 | Filled order - drag SL | [x] |
| T6 | Filled order - drag TP | [x] |
| T7 | API error during update | [x] |
| T8 | Chart zoom/pan during edit | [x] |
| T9 | Escape during edit | [x] |
| T10 | Enter during edit | [x] |

---

## Final Verification

- [x] `cargo test` - All tests passing
- [x] `cargo clippy --all-targets` - No errors
- [x] All 10 test scenarios verified
- [x] CHANGELOG.md updated

---

## Notes

Implementation completed successfully. Key changes:

1. **Backend**: Added `PUT /api/v1/trades/{id}/entry` endpoint with full validation
2. **Frontend API**: Added `updateEntryPrice()` function following existing patterns
3. **PositionHandleOverlay**: Extended with `lockedHandles` prop, lock icon, and proper cursor states
4. **OpenPositionsLayer**: Integrated edit functionality with auto-edit for new positions

---

## Files Modified

```
# Backend
crates/router/src/routes/trade_management.rs  # +UpdateEntryPriceRequest, +update_entry_price handler
crates/router/src/main.rs                      # +Route registration
crates/engine/src/shadow/order_group.rs        # +update_entry_order method

# Frontend
testudo-web/apps/web/src/utils/requests.ts                        # +updateEntryPrice function
testudo-web/apps/web/src/components/chart/PositionHandleOverlay.tsx  # +lockedHandles, +isExistingPosition, +LockIcon
testudo-web/apps/web/src/components/chart/OpenPositionsLayer.tsx     # +Edit state, +API integration
```
