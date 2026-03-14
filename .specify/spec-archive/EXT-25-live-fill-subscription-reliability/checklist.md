# EXT-25 Live Fill Subscription Reliability Checklist

## WebSocket Subscription Manager (FR-1)

- [ ] Add `ws_subscription_manager.rs` service with `(user_id, exchange_account_id)` keyed subscription registry.
- [ ] Implement symbol fan-in and dedupe so repeated subscribe requests do not spawn duplicate tasks.
- [ ] Forward sidecar `OrderUpdateEvent` into router `order_update_sender` broadcast channel.
- [ ] Add reconnect/backoff behavior and lifecycle logs for disconnect/reconnect.

## Trade Creation Trigger (FR-2)

- [ ] Wire subscription manager into trade management route state.
- [ ] On successful live trade create, call `ensure_subscribed(user, account, symbol)`.
- [ ] Ensure subscription trigger happens after exchange IDs are registered in order-group index.
- [ ] Add logs with user/group/order identifiers for subscription actions.

## Rehydration Resubscribe (FR-3)

- [ ] Collect active/pending live `(user, account, symbol)` tuples during startup rehydration.
- [ ] Register all tuples with subscription manager after startup initialization.
- [ ] Verify duplicate tuples do not create duplicate subscriptions.

## Event Forwarding and Detector Path (FR-4, FR-6)

- [ ] Preserve `order_update_sender -> FillDetectorService` pipeline.
- [ ] Add/standardize fill-detector logs for match, skip, cancel, and error cases.
- [ ] Include timing/latency fields where feasible for fill-to-cancel observability.

## Fallback Integrity (FR-5)

- [ ] Confirm `PriceFeedService` fallback OCO cancellation remains enabled for live exchange API.
- [ ] Confirm `OrderNotFound` remains idempotent no-op behavior.
- [ ] Add logs clarifying when fallback path performs cancellation.

## Extension Refresh Reliability (FR-7)

- [ ] Ensure background forwards closure-relevant WS updates (`stopped_out`, `took_profit`, `entry_filled`, etc.).
- [ ] Ensure `MainView` balance refresh triggers on closure events and avoids refresh storms (debounce/coalesce if needed).
- [ ] Validate schemas/runtime parsing if new message types are introduced.

## Lifecycle Logging and Monitoring (FR-8)

- [ ] Emit lifecycle events: `subscribe_requested`, `subscribe_started`, `subscribe_reused`.
- [ ] Emit stream health events: `stream_disconnected`, `stream_reconnect_scheduled`, `stream_reconnected`.
- [ ] Emit forwarding diagnostics: `forward_error` and event rate context.

## Regression Coverage (FR-9)

- [ ] Add tests for subscription dedupe and symbol fan-in.
- [ ] Add tests for WS manager forwarding into fill detector channel.
- [ ] Add tests for startup rehydration resubscribe behavior.
- [ ] Add tests proving fallback OCO still functions with WS disabled/failing.
- [ ] Keep extension runtime tests green for WS order update handling.

## Manual Validation

- [ ] Place live SL/TP trade and confirm subscription is active in logs.
- [ ] Trigger SL fill and verify sibling TP is cancelled quickly.
- [ ] Trigger TP fill and verify sibling SL is cancelled quickly.
- [ ] Restart backend with open live groups and verify automatic resubscribe.
- [ ] Simulate sidecar WS interruption and verify fallback OCO still protects position.
- [ ] Verify popup exposure/balance updates after closure events.

## Release Gate

- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] `cd testudo-extension && bun run typecheck && bun run test && bun run build`
- [ ] `cd testudo-web && bun run lint && bun run build`
