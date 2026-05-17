# EXT-25: Live Fill Subscription Reliability and OCO Closure Integrity

| Field    | Value                                              |
|----------|----------------------------------------------------|
| Status   | Complete                                              |
| Date     | 2026-03-05                                         |
| Depends  | EXT-21, EXT-22, EXT-24, 015-zod-runtime-stabilization |
| Phase    | Backend + Extension Reliability                    |

## 1. Overview

### Current State

The backend has all major components for live OCO closure handling:

- `FillDetectorService` exists and implements correct idempotent sibling cancellation.
- `CcxtClient.subscribe_orders()` exists and can consume sidecar order updates.
- `OrderGroupManager` exchange-order reverse index exists and is populated during trade creation.

But live fill events are not reaching the detector in production because there is no runtime orchestration layer that actually opens and maintains sidecar WebSocket subscriptions per live account/symbol set.

Result: SL/TP fills can occur on exchange while sibling orders remain open, funds remain locked, and extension exposure can stay artificially elevated.

### Root Cause

Primary:
- Missing runtime invocation/management of `CcxtClient.subscribe_orders()` for active live trades.

Secondary:
- No subscription lifecycle manager (dedupe, reconnect, account-symbol fan-in).
- No startup resubscription for rehydrated live groups after backend restart.
- Extension balance refresh depends on events that are delayed/absent when fill stream is missing.

### Target State

Live order update streams are established and maintained automatically for each active exchange account. Fill events are forwarded into `FillDetectorService` in near real time, sibling orders are cancelled idempotently, and extension balance/exposure refreshes promptly. Price-based OCO remains an explicit fallback path.

## 2. User Stories

- **US-1**: As a live trader, when SL or TP fills on the exchange, sibling orders are cancelled quickly and I do not get accidental reverse exposure.
- **US-2**: As a live trader, after backend restarts, active trades continue receiving fill detection without manual intervention.
- **US-3**: As a user, exposure and balance reflect post-close state quickly in the popup.
- **US-4**: As an operator, I can diagnose stream health from logs and metrics without guessing.

## 3. Functional Requirements

### FR-1: WebSocket Subscription Manager Service

**New file:** `testudo-exchange/crates/router/src/services/ws_subscription_manager.rs`

Implement a manager that:

1. Owns active sidecar WS subscription tasks keyed by `(user_id, exchange_account_id)`.
2. Maintains a symbol set per key and coalesces duplicate subscribe requests.
3. Calls `CcxtClient.subscribe_orders(...)` with current symbol set.
4. Forwards each `OrderUpdateEvent` into existing router `order_update_sender` broadcast channel.
5. Handles task restart/backoff on disconnect or stream errors.
6. Supports clean shutdown/cancellation of per-key task when no symbols remain (or on explicit unsubscribe, if added).

### FR-2: Trade-Creation Subscription Trigger

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs`

On successful live trade creation (after exchange IDs are registered):

1. Detect live mode (`is_authenticated == true`).
2. Call subscription manager with:
   - `user_id`
   - `exchange_account_id`
   - account credentials context (resolved through existing exchange account repository path)
   - symbol of the trade.
3. Ensure repeated trade placements for same key/symbol do not create duplicate sidecar WS tasks.
4. Log subscription intent and outcome with group/order identifiers.

### FR-3: Startup Rehydration Resubscribe

**Files:**
- `testudo-exchange/crates/router/src/main.rs`
- `testudo-exchange/crates/router/src/services/rehydration.rs` (or integration point that iterates restored groups)

After rehydration of live groups:

1. Build unique `(user_id, exchange_account_id, symbol)` tuples for active/pending live groups.
2. Register each tuple with WS subscription manager.
3. Guarantee no missed subscription on process restart for still-open live orders.

### FR-4: Event Forwarding Contract

**Files:**
- `testudo-exchange/crates/router/src/services/ws_subscription_manager.rs`
- `testudo-exchange/crates/router/src/main.rs`

Maintain the existing detector input contract:

1. `OrderUpdateEvent` enters the shared `order_update_sender` broadcast channel.
2. `FillDetectorService` remains unchanged as primary order-state actor.
3. Forwarding path includes structured logs for stream event rate, lag, and drops.

### FR-5: Fallback Preservation (Price-Based OCO)

**File:** `testudo-exchange/crates/router/src/services/price_feed.rs`

Ensure existing fallback behavior remains intact:

1. `exchange_cancels` still executes for live exchange API.
2. `OrderNotFound` remains idempotent no-op.
3. Fallback path is active even if sidecar WS stream is unavailable.

This FR is verification-hardening, not a net-new mechanism.

### FR-6: Fill Detector Observability

**File:** `testudo-exchange/crates/router/src/services/fill_detector.rs`

Add/standardize logs for:

1. Matched fill event -> group resolution.
2. Terminal-state idempotency skips.
3. Cancellation attempts and outcomes by group/order/account.
4. Event-to-cancel latency measurement fields where possible.

### FR-7: Extension Exposure/Balance Refresh Reliability

**Files:**
- `testudo-extension/src/background.ts`
- `testudo-extension/src/popup/components/MainView.tsx`
- `testudo-extension/src/schemas.ts` (if new runtime message shape introduced)

1. Keep existing `WS_ORDER_UPDATE` propagation.
2. Ensure popup balance refresh triggers for closure-relevant management events emitted by backend (`stopped_out`, `took_profit`, `entry_filled`, and any explicit close event introduced).
3. Avoid duplicate refresh storms by applying lightweight debounce/coalescing if needed.

### FR-8: Subscription Lifecycle Logging and Monitoring

**Files:**
- `testudo-exchange/crates/router/src/services/ws_subscription_manager.rs`
- `testudo-exchange/crates/router/src/main.rs`

Log lifecycle transitions:

1. `subscribe_requested`
2. `subscribe_started`
3. `subscribe_reused`
4. `stream_disconnected`
5. `stream_reconnect_scheduled`
6. `stream_reconnected`
7. `forward_error`

### FR-9: Contract and Regression Tests

Add tests to cover:

1. Subscription dedupe for same key/symbol.
2. Symbol fan-in update for existing key.
3. Event forwarding from WS manager -> broadcast channel -> fill detector.
4. Rehydration-driven resubscribe flow.
5. Fallback OCO still functional when WS manager is disabled/failing.
6. Extension runtime handling of WS order update events remains schema-safe post-zod stabilization.

## 4. Architecture and Data Flow

```
create_trade (live)
   -> register exchange IDs in OrderGroupManager index
   -> ws_subscription_manager.ensure_subscribed(user, account, symbol)
   -> CcxtClient.subscribe_orders(...)
   -> sidecar ws/order stream
   -> OrderUpdateEvent forwarded to order_update_sender
   -> FillDetectorService.handle_order_update
   -> cancel sibling on exchange (idempotent)
   -> emit management event (pg_notify)
   -> extension WS receives order.<user>
   -> WS_ORDER_UPDATE runtime message
   -> MainView fetchBalance() and exposure recompute
```

Fallback path (always enabled):

```
PriceFeedService.poll -> ShadowEngine.process_price_update -> exchange_cancels -> cancel sibling
```

## 5. Files to Modify

| File | Change | Component |
|------|--------|-----------|
| `testudo-exchange/crates/router/src/services/ws_subscription_manager.rs` | New service for live subscription lifecycle and forwarding | Backend |
| `testudo-exchange/crates/router/src/services/mod.rs` | Export manager module and types | Backend |
| `testudo-exchange/crates/router/src/main.rs` | Instantiate/inject manager, startup resubscribe wiring | Backend |
| `testudo-exchange/crates/router/src/routes/trade_management.rs` | Trigger ensure_subscribed on successful live trade create | Backend |
| `testudo-exchange/crates/router/src/services/rehydration.rs` | Provide/live tuple extraction for restart resubscribe | Backend |
| `testudo-exchange/crates/router/src/services/fill_detector.rs` | Enhanced structured logs and latency fields | Backend |
| `testudo-exchange/crates/router/src/services/price_feed.rs` | Fallback-path verification/log hardening (no behavior regression) | Backend |
| `testudo-extension/src/background.ts` | Ensure WS message forwarding includes closure events for popup refresh | Extension |
| `testudo-extension/src/popup/components/MainView.tsx` | Balance refresh triggers/debounce refinements | Extension |
| `testudo-extension/src/schemas.ts` | Update runtime schemas if additional message types are introduced | Extension |

## 6. Non-Goals

- Replacing `FillDetectorService` business logic.
- Replacing existing price-feed OCO fallback with sidecar-only logic.
- Adding new exchange integrations or changing exchange credential storage format.
- Redesigning popup UI visuals.

## 7. Acceptance Criteria

- [ ] Live trade creation opens/reuses WS subscription path for its `(user, account, symbol)` context.
- [ ] `OrderUpdateEvent` from sidecar reaches `FillDetectorService` without manual intervention.
- [ ] SL fill cancels TP and TP fill cancels SL within operational target (<500ms typical under healthy sidecar).
- [ ] On backend restart, rehydrated live groups are automatically resubscribed.
- [ ] If WS stream fails, fallback price-based OCO still closes sibling orders safely.
- [ ] Extension balance/exposure updates after live close events without manual refresh.
- [ ] Duplicate subscriptions are prevented for repeated trade creation on same context.
- [ ] Logs expose clear subscription lifecycle and cancellation outcomes.
- [ ] All verification commands pass.

## 8. Verification Plan

### Automated

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Extension
cd testudo-extension && bun run typecheck && bun run test && bun run build

# Web app (repo gate)
cd testudo-web && bun run lint && bun run build
```

### Manual Scenarios

1. Place live trade with SL/TP -> verify subscription lifecycle logs show active stream.
2. Trigger SL on exchange -> verify sibling TP cancelled quickly and group transitions terminal.
3. Trigger TP on exchange -> verify sibling SL cancelled quickly.
4. Restart backend with open live groups -> verify automatic resubscribe and continued fill handling.
5. Simulate sidecar WS interruption -> verify fallback price-based OCO still cancels sibling orders.
6. Observe extension popup -> verify exposure drops and balance refreshes after closure events.

## 9. Risks and Mitigations

- **Risk:** Task explosion from one WS task per symbol.
  - **Mitigation:** Key by `(user, account)` with symbol fan-in set.
- **Risk:** Event loss during reconnect windows.
  - **Mitigation:** Reconnect backoff + fallback price path + restart resubscribe.
- **Risk:** Balance refresh storms in popup.
  - **Mitigation:** Debounced refresh on grouped WS events.

## 10. Completion Signal

When all acceptance criteria are satisfied and verification commands pass, output:

`<promise>DONE</promise>`
