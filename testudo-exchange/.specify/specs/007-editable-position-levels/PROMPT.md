# Ralph Loop Prompt: Editable Position Levels

You are implementing the Editable Position Levels feature (007-editable-position-levels).

## Context

Read the PRD at `.specify/specs/007-editable-position-levels/PRD.md` for full requirements.

**Current State:**
- `OpenPositionsLayer` renders positions as static canvas primitives (no handles)
- `PositionHandleOverlay` exists with full drag system but only used for new positions
- `PUT /trades/{id}/sl` and `PUT /trades/{id}/tp` endpoints exist
- `PUT /trades/{id}/entry` does NOT exist

**Target State:**
- `OpenPositionsLayer` renders `PositionHandleOverlay` for editing positions
- Entry handle locked for filled orders, draggable for pending
- New `PUT /trades/{id}/entry` endpoint for pending order entry updates
- Toast notifications on success/error

## Your Task

Work through the phases in order. Each phase has specific functional requirements.

### Phase 1: Backend - Entry Update Endpoint

**Files:**
- `crates/router/src/routes/trade_management.rs`
- `crates/router/src/main.rs`
- `crates/engine/src/shadow/order_group.rs`

**Tasks:**

1. **Add Request Type** (trade_management.rs, after line ~100)
```rust
#[derive(Debug, Deserialize)]
pub struct UpdateEntryPriceRequest {
    pub price: Decimal,
}
```

2. **Add Handler** (trade_management.rs, after update_take_profit ~line 527)
```rust
pub async fn update_entry_price(
    path: web::Path<Uuid>,
    body: web::Json<UpdateEntryPriceRequest>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    // 1. Extract user_id, group_id, new_price
    // 2. Validate status == Pending
    // 3. Validate price relationship (entry > SL for longs)
    // 4. Cancel existing entry order
    // 5. Create new order at new price
    // 6. Update group.entry_order_id
    // 7. Return updated TradeGroupResponse
}
```

3. **Register Route** (main.rs, after line ~171)
```rust
.route("/{id}/entry", web::put().to(trade_management::update_entry_price))
```

4. **Add Index Update Method** (order_group.rs)
```rust
impl OrderGroupManager {
    pub fn update_entry_order(&mut self, group_id: Uuid, old_entry_id: Uuid, new_entry_id: Uuid) {
        self.groups_by_entry_order.remove(&old_entry_id);
        self.groups_by_entry_order.insert(new_entry_id, group_id);
    }
}
```

**Verification:** `cargo test` and `cargo clippy --all-targets`

---

### Phase 2: Frontend - API Function

**Files:**
- `testudo-web/apps/web/src/utils/requests.ts`

**Tasks:**

1. **Add updateEntryPrice function** (after updateTakeProfit ~line 267)
```typescript
export async function updateEntryPrice(
  tradeId: string,
  price: number,
  userId: string
): Promise<TradeGroup> {
  const response = await axios.put<ApiResponse<TradeGroup>>(
    `${BASE_URL}/trades/${tradeId}/entry`,
    { price },
    { headers: { 'X-User-Id': userId } }
  );
  if (!response.data.success || !response.data.data) {
    throw new Error(response.data.error || 'Failed to update entry price');
  }
  return response.data.data;
}
```

---

### Phase 3: Frontend - PositionHandleOverlay Props

**Files:**
- `testudo-web/apps/web/src/components/chart/PositionHandleOverlay.tsx`

**Tasks:**

1. **Extend Props Interface** (around line 35)
```typescript
interface PositionHandleOverlayProps {
  // ... existing props
  lockedHandles?: HandleType[];
  isExistingPosition?: boolean;
  positionId?: string;
}
```

2. **Update Handle Component** to support locked state
- Add `isLocked` prop to Handle
- If locked: `cursor: 'not-allowed'`, no onMouseDown, show lock icon SVG
- Reduce opacity for locked handles

3. **Pass lockedHandles to Handle instances**
```typescript
const isEntryLocked = lockedHandles?.includes('entry');
// ... same for stopLoss, takeProfit
```

---

### Phase 4: Frontend - OpenPositionsLayer Integration

**Files:**
- `testudo-web/apps/web/src/components/chart/OpenPositionsLayer.tsx`

**Tasks:**

1. **Add Edit State**
```typescript
const [editingPositionId, setEditingPositionId] = useState<string | null>(null);
```

2. **Auto-Edit New Positions**
```typescript
useEffect(() => {
  // Track previous position IDs
  // When new position appears, setEditingPositionId(newId)
}, [positions]);
```

3. **Add Level Change Handler**
```typescript
const handleLevelChange = async (type: HandleType, price: number) => {
  const userId = localStorage.getItem('user_id');
  try {
    if (type === 'entry') await updateEntryPrice(id, price, userId);
    else if (type === 'stopLoss') await updateStopLoss(id, price, userId);
    else if (type === 'takeProfit') await updateTakeProfit(id, price, 100, userId);
    toast.success('Position updated');
    refresh();
  } catch (err) {
    toast.error('Update failed', { description: err.message });
    // Revert visual
  }
};
```

4. **Render PositionHandleOverlay for Editing Position**
```typescript
{editingPosition && (
  <PositionHandleOverlay
    chartManager={chartManager}
    levels={editingPosition.levels}
    onLevelChange={handleLevelChange}
    onExecute={() => setEditingPositionId(null)}
    onCancel={() => setEditingPositionId(null)}
    lockedHandles={editingPosition.status === 'Active' ? ['entry'] : []}
    isExistingPosition={true}
  />
)}
```

---

## Completion Protocol

After EACH phase, run appropriate tests:
- Phase 1: `cargo test` and `cargo clippy --all-targets`
- Phase 2-4: Manual testing in browser

When ALL phases complete and working, output:
```
<promise>007-EDITABLE-POSITION-LEVELS-COMPLETE</promise>
```

## Rules

1. **TDD**: Write/update tests for backend changes
2. **Incremental**: Complete one phase at a time
3. **Verify**: Test after each change
4. **No Shortcuts**: Don't skip requirements
5. **Document**: Update CHANGELOG.md with changes

## Files Summary

```
# Backend
crates/router/src/routes/trade_management.rs  # Handler + request type
crates/router/src/main.rs                      # Route registration
crates/engine/src/shadow/order_group.rs        # Index update method

# Frontend
testudo-web/apps/web/src/utils/requests.ts                        # API function
testudo-web/apps/web/src/components/chart/PositionHandleOverlay.tsx  # Props + locking
testudo-web/apps/web/src/components/chart/OpenPositionsLayer.tsx     # Edit integration

# Docs
CHANGELOG.md                                   # Document changes
```

## Progress Tracking

Check `.specify/specs/007-editable-position-levels/PROGRESS.md` for current state.
Update it as you complete each phase.
