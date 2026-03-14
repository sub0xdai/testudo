# EXT-24: Startup Rehydration — Reconstruct OrderGroups from Persistent State

| Field    | Value                                              |
|----------|----------------------------------------------------|
| Status   | Draft                                              |
| Date     | 2026-03-02                                         |
| Depends  | EXT-21, EXT-22, 012-ccxt-multi-exchange            |
| Phase    | Backend — State Persistence & Recovery             |

## 1. Problem Statement

The ShadowEngine's `OrderGroupManager` is entirely in-memory. When the backend process restarts, all OrderGroup data is lost. The `list_trades` endpoint returns an empty list, the FillDetectorService's reverse index is empty (WebSocket fill events can't find their groups), and the extension shows "No active positions" despite live orders existing on the exchange.

Meanwhile, `ManagedPosition` already has PostgreSQL persistence via `PositionRepository` — it stores exchange order IDs, management rules, position state, and all fields needed to reconstruct OrderGroups. The `load_from_db()` method already recovers ManagedPositions on startup. The gap is that nobody creates corresponding OrderGroups in the ShadowEngine from that data.

## 2. Design Principle

Keep the hot path in-memory. Don't add I/O to `TransactionContext::commit` or `process_price_update`. Instead, rehydrate once at startup from the data we already persist, and optionally verify against the exchange.

## 3. Research Findings

### 3.1 The Grouping Problem

The user identified the core "gotcha": mapping a flat list of exchange orders back into linked OrderGroups. Research reveals this problem is **already solved** — the `managed_positions` table stores `exchange_order_ids` as JSONB containing `{entry_order_id, stop_loss_order_id, take_profit_order_id}`. No clientOrderId parsing or heuristic matching needed for rehydration.

### 3.2 clientOrderId Support Across Exchanges

| Exchange | Stamp on createOrder | Recovered via fetchOpenOrders | Notes |
|----------|---------------------|------------------------------|-------|
| **WOO X** | Supported | **BROKEN** — returns `0` for limit orders | Known CCXT adapter bug (woo.js line 2005) |
| **Binance** | Supported | Reliable | Consistently returns exact clientOrderId |
| **Bybit** | Supported (`orderLinkId`) | Reliable | Empty strings normalized to undefined |

**Conclusion**: clientOrderId is NOT a viable primary strategy for cross-exchange group reconstruction. WOO X breaks it for the most common order type (limit). However, stamping clientOrderId is still valuable as defense-in-depth for exchanges that support it.

### 3.3 fetchOpenOrders Behavior

| Exchange | Requires symbol param | Returns all symbols |
|----------|-----------------------|--------------------|
| **WOO X** | Yes | No |
| **Binance** | No (optional) | Yes |
| **Bybit** | Yes | No |

Most exchanges require per-symbol queries. The rehydration service must know which symbols to check — the `managed_positions` table provides this.

### 3.4 Existing Persistence Infrastructure

| Component | Persisted | Location |
|-----------|-----------|----------|
| `ManagedPosition` | Yes (PostgreSQL) | `managed_positions` table |
| `ExchangeOrderIds` | Yes (JSONB column) | `managed_positions.exchange_order_ids` |
| `ManagementRules` | Yes | `managed_positions` columns |
| `OrderGroup` | **No** (in-memory only) | `ShadowEngine.order_groups` |
| `OrderGroupManager` reverse index | **No** (in-memory only) | `groups_by_exchange_order` HashMap |
| `exchange_account_id` on ManagedPosition | **No** (hardcoded to None on load) | `repository.rs:275` |
| `leverage` on ManagementRules | **No** (hardcoded to 1 on load) | `repository.rs:264` |

## 4. Approach: DB-First Rehydration with Optional Exchange Verification

### Why not pure CCXT rehydration?

Pure CCXT rehydration (fetching open orders and reconstructing groups heuristically) has three problems:

1. **Grouping ambiguity** — A flat list of orders has no inherent grouping. Heuristic matching by symbol + quantity + side works for single trades per symbol but breaks with multiple concurrent trades on the same pair.
2. **Missing management rules** — The exchange doesn't store break_even_at, trailing_stop configuration, partial_tp rules. These would need defaults, losing user configuration.
3. **Missing position state** — `be_triggered`, `partial_tp_fired`, `current_stop` (if trailing has moved it) are all lost. The trade manager would re-evaluate from scratch, potentially re-triggering already-fired actions.

### Why DB-first is better

The `managed_positions` table already has everything:
- Exchange order IDs (entry, SL, TP) — solves the grouping problem
- Management rules with user's actual configuration
- Position state (`be_triggered`, `partial_tp_fired`, `current_stop`)
- User ID, symbol, side, prices, quantities

Rehydration becomes: load from DB → create OrderGroups → register exchange IDs in reverse index. No guessing, no heuristics.

### Optional exchange verification

After creating OrderGroups from DB, optionally call `fetchOpenOrders` to verify which orders are still alive on the exchange. This handles cases where orders filled or were cancelled while the backend was down. Without verification, the engine assumes all orders are in their last-known DB state — acceptable for short restarts, problematic for extended downtime.

## 5. Functional Requirements

### FR-1: Persist `exchange_account_id` and `leverage` in ManagedPosition

**Files:** `crates/router/src/services/trade_manager/repository.rs`

The `managed_positions` table is missing two columns needed for full rehydration:

1. `exchange_account_id UUID` — required for routing CCXT calls during exchange verification and for OCO cancellation
2. `leverage SMALLINT` — the management rules leverage (currently hardcoded to 1 on load)

Add columns via ALTER TABLE (idempotent, with IF NOT EXISTS pattern). Update `insert()`, `load_active()`, and `into_position()` to include both fields.

### FR-2: OrderGroup Reconstruction from ManagedPosition

**New file:** `crates/router/src/services/rehydration.rs`

A startup-only service that:

1. Loads active ManagedPositions from DB (reuse `PositionRepository::load_active()`)
2. For each position, creates a corresponding `OrderGroup` in the ShadowEngine:
   - `group.id` = position.id (use same UUID for correlation)
   - `group.user_id` = position.user_id
   - `group.symbol` = position.symbol
   - `group.entry_order_id` = generate a synthetic UUID (no real shadow order exists)
   - `group.entry_price` = Some(position.entry_price)
   - `group.entry_quantity` = position.quantity
   - `group.stop_loss_price` = Some(position.current_stop) (may have been amended)
   - `group.take_profit_targets` = single target at position.target_price
   - `group.status` = map PositionState → OrderGroupStatus:
     - Pending → Pending
     - Filled → Active
     - Managing → Active
     - Closed → Closed
   - `group.exchange_order_id` = position.exchange_order_ids.entry_order_id
   - `group.exchange_sl_order_id` = position.exchange_order_ids.stop_loss_order_id
   - `group.exchange_tp_order_id` = position.exchange_order_ids.take_profit_order_id
   - `group.exchange_account_id` = position.exchange_account_id
   - `group.break_even_config` = reconstruct from position.rules.break_even_at + position.be_triggered
3. Inserts each OrderGroup directly into `OrderGroupManager`
4. Registers all exchange order IDs in the reverse index via `register_exchange_order()`

### FR-3: Exchange Verification (Optional, Enabled by Flag)

**Files:** `crates/router/src/services/rehydration.rs`, `crates/router/src/services/ccxt_client.rs`

When `REHYDRATION_VERIFY_EXCHANGE=true`:

1. For each rehydrated position, call `fetchOpenOrders` via the CCXT sidecar to check which exchange orders still exist
2. If an entry order is no longer open (filled or cancelled), update OrderGroup status to Active
3. If SL or TP is no longer open (filled), trigger OCO logic — mark group as StoppedOut/TookProfit, cancel sibling
4. If none of the orders exist, mark group as Closed and update DB

This requires a new sidecar endpoint:

**Sidecar:** `GET /orders/open?symbol=SOL/USDT:USDT`

Calls CCXT `exchange.fetchOpenOrders(symbol)` and returns the unified order list with fields: `id`, `clientOrderId`, `symbol`, `status`, `side`, `type`, `price`, `stopPrice`, `amount`, `filled`, `remaining`, `timestamp`.

### FR-4: Startup Sequencing in main.rs

**File:** `crates/router/src/main.rs`

Add rehydration call after:
- PostgreSQL pool initialized
- ShadowEngine created
- CcxtClient created
- ExchangeAccountRepository created

But strictly before:
- PriceFeedService spawned
- TradeManagerService spawned
- FillDetectorService spawned
- HTTP server bound

```
PostgreSQL pool ──► ShadowEngine ──► CcxtClient ──► ExchangeAccountRepo
                                                           │
                                                    ┌──────▼───────┐
                                                    │ Rehydration   │
                                                    │ Service       │
                                                    │               │
                                                    │ 1. Load from  │
                                                    │    Postgres   │
                                                    │ 2. Create     │
                                                    │    OrderGroups│
                                                    │ 3. Register   │
                                                    │    exchange   │
                                                    │    IDs        │
                                                    │ 4. (Optional) │
                                                    │    Verify vs  │
                                                    │    exchange   │
                                                    └──────┬───────┘
                                                           │
PriceFeedService ◄─ TradeManagerService ◄─ FillDetector ◄──┘
                                                           │
                                                    HTTP Server bound
```

### FR-5: Stamp clientOrderId on Future Orders (Defense-in-Depth)

**Files:** `testudo-ccxt/src/handlers.js`, `crates/router/src/services/ccxt_client.rs`

For all future order placements, stamp `clientOrderId` with a parseable format:

```
testudo:{group_id}:{role}
```

Where role is `entry`, `sl`, or `tp`. Example: `testudo:a1b2c3d4-...:entry`

This provides:
- A fallback identification mechanism for exchanges that support clientOrderId (Binance, Bybit)
- No-op on WOO X (stamped but not recoverable — that's fine, DB is primary)
- Useful for manual debugging on exchange dashboards (human-readable trade origin)

**Sidecar changes:**
- `handleOrder()`: Accept optional `clientOrderId` param, pass to CCXT `createOrder()`
- Return `clientOrderId` in response (if exchange echoes it back)

**Rust client changes:**
- Add `client_order_id: Option<String>` to `PlaceOrderRequest` and `SidecarOrderResponse`
- Generate the stamped ID in `create_trade` before calling `place_order`

## 6. Files to Modify

| File | Change | Component |
|------|--------|-----------|
| `crates/router/src/services/trade_manager/repository.rs` | FR-1: Add exchange_account_id + leverage columns | Backend |
| `crates/router/src/services/rehydration.rs` | FR-2: New rehydration service | Backend |
| `crates/router/src/services/mod.rs` | FR-2: Register new module | Backend |
| `crates/router/src/main.rs` | FR-4: Call rehydration at startup | Backend |
| `crates/engine/src/shadow/order_group.rs` | FR-2: Add insert method to OrderGroupManager | Engine |
| `crates/router/src/services/ccxt_client.rs` | FR-3, FR-5: fetchOpenOrders + clientOrderId | Backend |
| `testudo-ccxt/src/handlers.js` | FR-3: GET /orders/open endpoint | Sidecar |
| `testudo-ccxt/src/server.js` | FR-3: Route registration | Sidecar |
| `crates/router/src/routes/trade_management.rs` | FR-5: Generate clientOrderId on create_trade | Backend |

## 7. Edge Cases

### 7.1 Backend down during order fill

If the backend restarts and an SL/TP filled while it was down:
- **Without exchange verification (FR-3 disabled):** OrderGroup shows as Active, but the exchange order no longer exists. The FillDetectorService will receive no WebSocket events for it (already filled). The PriceFeedService's shadow engine OCO will eventually detect the cross and update status, but the exchange cancel will get `OrderNotFound` (sibling already filled). This is a no-op — correct but slow.
- **With exchange verification (FR-3 enabled):** Rehydration detects the filled order, updates group status immediately, cancels sibling if still open. Clean recovery.

### 7.2 Partial fills during downtime

ManagedPosition stores `remaining_qty`. If exchange verification shows `filled > 0` on the entry order, we could update the entry state. For v1, treat partial fills as still-open — the TradeManager will handle ongoing management once price ticks resume.

### 7.3 Multiple positions on same symbol

The DB stores each position independently with its own exchange order IDs. No ambiguity — each position maps to exactly one OrderGroup.

### 7.4 Exchange account credentials rotated during downtime

If API keys changed, `fetchOpenOrders` will fail for that account. Rehydration should log a warning and skip exchange verification for that account, still creating the OrderGroup from DB data.

### 7.5 Synthetic shadow orders

Rehydrated OrderGroups reference synthetic entry_order_ids (generated UUIDs, not real shadow orders). The ShadowEngine's order book won't have matching orders. This is acceptable — the shadow order book is for paper trading simulation. Live trades don't need shadow orders; they only need OrderGroups for the `list_trades` endpoint and exchange ID reverse index for the FillDetectorService.

## 8. Implementation Phases

### Phase 1: Core Rehydration (FR-1, FR-2, FR-4)
- Add missing DB columns
- Implement `RehydrationService` that loads from DB and creates OrderGroups
- Wire into main.rs startup sequence
- **Outcome:** Positions survive backend restarts

### Phase 2: Exchange Verification (FR-3)
- Add sidecar `GET /orders/open` endpoint
- Add `CcxtClient::fetch_open_orders()` method
- Implement verification logic in RehydrationService
- **Outcome:** Stale positions detected and reconciled on startup

### Phase 3: clientOrderId Stamping (FR-5)
- Stamp IDs on all new order placements
- Return and store clientOrderId alongside exchange order ID
- **Outcome:** Defense-in-depth for future pure-CCXT recovery scenarios

## 9. Acceptance Criteria

- [ ] Backend restart preserves active positions in `list_trades` response
- [ ] FillDetectorService reverse index populated on startup (WebSocket fill events find groups)
- [ ] Extension shows active positions after backend restart
- [ ] Breakeven/trailing/partial TP resume correctly after restart (ManagedPosition state preserved)
- [ ] Exchange verification correctly detects filled orders during downtime (Phase 2)
- [ ] clientOrderId stamped on all new orders for Binance/Bybit/WOO (Phase 3)
- [ ] No regression: all existing tests pass (`cargo test`, `npm test`, `npx vitest run`)
- [ ] Rehydration completes before HTTP server accepts connections
- [ ] Rehydration logs summary: "Rehydrated N positions across M exchange accounts"

## 10. Verification

1. `cd testudo-exchange && cargo test` — all tests pass
2. `cd testudo-ccxt && npm test` — sidecar tests pass (Phase 2+)
3. Manual: Place live trade → restart backend → open extension → verify position appears
4. Manual: Place live trade → stop backend → wait for SL to fill on exchange → restart backend with REHYDRATION_VERIFY_EXCHANGE=true → verify position shows as StoppedOut and TP cancelled

## 11. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| DB schema migration on production | Downtime during ALTER TABLE | Use IF NOT EXISTS / ADD COLUMN IF NOT EXISTS |
| Synthetic entry_order_ids conflict with real shadow orders | Unlikely — UUIDs are random | Use a dedicated UUID namespace or prefix check |
| Rehydration slow with many positions | Startup delay | Log timing, consider pagination if >100 positions |
| Exchange verification fails (sidecar down) | Positions created from DB only | Acceptable degradation — log warning, proceed without verification |
