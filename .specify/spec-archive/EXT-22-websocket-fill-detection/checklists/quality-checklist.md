# Quality Checklist — EXT-22: WebSocket Fill Detection

| Field   | Value                          |
|---------|--------------------------------|
| Spec    | EXT-22-websocket-fill-detection |
| Date    | 2026-03-01                     |

## Implementation Checklist

- [ ] FR-1: Sidecar WebSocket endpoint with `watchOrders()` loop
- [ ] FR-2: Rust WebSocket client in `CcxtClient`
- [ ] FR-3: `FillDetectorService` with OCO cancel logic
- [ ] FR-4: `OrderGroupManager` exchange order ID reverse index
- [ ] FR-5: Extension handles real-time fill events
- [ ] FR-6: Graceful degradation to shadow engine OCO on WS disconnect

## Testing Checklist

- [ ] Unit: FillDetector cancels TP when SL fill event received
- [ ] Unit: FillDetector cancels SL when TP fill event received
- [ ] Unit: FillDetector ignores events for unknown exchange order IDs
- [ ] Unit: FillDetector is idempotent — second fill event for same order is no-op
- [ ] Unit: OrderGroupManager exchange order ID index lookup works
- [ ] Integration: Sidecar WebSocket streams mock order events
- [ ] Integration: End-to-end fill → cancel with mock exchange
- [ ] Manual: Live WOO trade, SL triggers, TP cancelled <1s

## Quality Gates

- [ ] `cargo test` — all pass
- [ ] `cargo clippy` — no new warnings
- [ ] `npm test` (sidecar) — all pass
- [ ] `npx vitest run` (extension) — all pass
- [ ] No double cancellation under concurrent fill events
- [ ] Shadow engine OCO still works when sidecar WebSocket is down
