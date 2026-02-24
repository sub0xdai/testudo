# EXT-16 Quality Checklist

| Field    | Value                                    |
|----------|------------------------------------------|
| Spec     | EXT-16-live-trading-readiness            |
| Date     | 2026-02-24                               |

## Pre-Implementation Baseline

- [ ] Confirm sidecar GET /health endpoint responds
- [ ] Confirm `load_credentials()` uses `.first()` pattern (to be changed)
- [ ] Confirm TradePayload has no `exchange_account_id` field (to be added)
- [ ] Confirm 6 OrderEventType values exist (to be extended)
- [ ] Confirm TradeManagerService does not publish WS events (to be added)

## FR-1: Sidecar Health Monitoring

- [ ] Background task spawned in main.rs calling health_check() every 30s
- [ ] SidecarHealth enum: Healthy, Degraded, Unreachable
- [ ] State change published to WS stream as `sidecar.health` event
- [ ] State transitions logged at warn/info level
- [ ] GET /api/v1/health/sidecar endpoint returns current state

## FR-2: Sidecar Status in Extension

- [ ] SIDECAR_STATUS message handler in background.ts
- [ ] WS listener for sidecar.health events
- [ ] StatusBar shows compound status in LIVE mode (WS + sidecar)
- [ ] Orange dot + "Sidecar Down" when unreachable
- [ ] Warning banner below header in LIVE mode when sidecar unreachable

## FR-3: Exchange Account Selection

- [ ] `activeExchangeId` persisted in browser.storage.local
- [ ] Active toggle on ExchangeManager account cards
- [ ] `exchange_account_id` field added to TradePayload
- [ ] executeTrade() attaches exchange_account_id from storage
- [ ] Backend load_credentials() accepts optional exchange_account_id
- [ ] Fallback to .first() when no exchange_account_id provided
- [ ] Active exchange name displayed as badge in confirmation modal
- [ ] On account delete, clear activeExchangeId if matches

## FR-4: Trade Management Event Visibility

- [ ] TradeManagerService publishes events on break-even trigger
- [ ] TradeManagerService publishes events on trailing stop amend
- [ ] TradeManagerService publishes events on partial TP fill
- [ ] Event payload: { event, symbol, detail }
- [ ] New OrderEventType values: order.break_even, order.trailing_moved, order.partial_tp
- [ ] ORDER_EVENT_STYLES entries for new event types
- [ ] Toasts render correctly for each management event type

## Post-Implementation Verification

- [ ] `cargo clippy --all-targets` passes
- [ ] `cargo test` passes (all existing + new tests)
- [ ] `npx tsc --noEmit` passes (extension)
- [ ] `bun run build` succeeds (Chrome + Firefox)
- [ ] `bun run test` passes (65+ tests)
- [ ] Sidecar down → StatusBar updates within 30s
- [ ] Sidecar restart → StatusBar recovers to blue
- [ ] Trade with explicit exchange_account_id routes correctly
- [ ] Trade without exchange_account_id uses first account (backwards compat)
- [ ] Management events appear as toasts in popup
