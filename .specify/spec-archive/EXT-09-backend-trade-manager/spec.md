# EXT-09: Backend Trade Management Engine

> Priority: P0 | Depends on: EXT-08, FILL-01 | Status: Complete | Completed: 2026-02-10

## Overview
**Current:** Backend receives trade orders via `POST /api/v1/trades` and places them on the exchange (or shadow engine). No post-placement management — orders sit until they hit stop or target. Position sizing uses a fixed quantity from the extension.
**Target:** Backend evaluates trade management rules (break-even, trailing stop, partial TP) against the live price feed and amends exchange orders automatically. Position sizing calculated server-side from risk % and real exchange balance via a new `ExchangeApi` trait.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Parse `management` block from trade payload and store rules with the position | pending |
| FR-2 | Calculate position quantity server-side: `(balance * risk_percent / 100) / abs(entry - stop)` using real exchange balance | pending |
| FR-3 | `ExchangeApi` trait with methods: `get_balance`, `place_order`, `amend_order`, `cancel_order`, `get_position` | pending |
| FR-4 | Binance implementation of `ExchangeApi` (futures API) | pending |
| FR-5 | Trade manager service: register managed positions, evaluate rules on each price tick | pending |
| FR-6 | Break-even rule: move stop to entry price when price covers `break_even_at` % of distance to target | pending |
| FR-7 | Trailing stop rule: after break-even triggers, trail stop at `distance_percent` of entry-to-target distance. Stop only moves in profitable direction, never back. | pending |
| FR-8 | Partial TP rule: close `close_percent` of position when price reaches target. Fire once only. | pending |
| FR-9 | Persist managed positions to PostgreSQL. Reload on backend restart and resume management. | pending |
| FR-10 | Emit WebSocket events for management actions: `order.amended`, `order.trailing`, `order.partial_close`, `order.stopped`, `order.tp_hit` | pending |
| FR-11 | `GET /api/v1/trades/:id/management` endpoint to query active management state for a position | pending |
| FR-12 | Order amendment endpoint `PATCH /api/v1/orders/:id` for manual overrides (future use) | pending |

## Data Model

### Managed Position (PostgreSQL)
```sql
CREATE TABLE managed_positions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL,
    symbol          VARCHAR(32) NOT NULL,
    side            VARCHAR(5) NOT NULL,       -- LONG / SHORT
    entry_price     DECIMAL NOT NULL,
    stop_price      DECIMAL NOT NULL,
    target_price    DECIMAL NOT NULL,
    quantity        DECIMAL NOT NULL,           -- calculated from risk %
    timeframe       VARCHAR(8),

    -- Management rules (from payload)
    risk_percent        DECIMAL NOT NULL,
    break_even_at       INTEGER NOT NULL,       -- % of distance to target
    trailing_enabled    BOOLEAN NOT NULL DEFAULT false,
    trailing_distance   INTEGER DEFAULT 0,      -- % of entry-to-target distance
    partial_tp_enabled  BOOLEAN NOT NULL DEFAULT false,
    partial_tp_percent  INTEGER DEFAULT 0,      -- % of position to close

    -- Runtime state
    status              VARCHAR(16) NOT NULL DEFAULT 'pending',
    -- pending → filled → managing → closed
    be_triggered        BOOLEAN NOT NULL DEFAULT false,
    partial_tp_fired    BOOLEAN NOT NULL DEFAULT false,
    current_stop        DECIMAL,                -- tracks amended stop
    remaining_quantity  DECIMAL,                -- after partial TP
    exchange_order_ids  JSONB,                  -- { entry: "...", stop: "...", tp: "..." }

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### ExchangeApi Trait
```rust
#[async_trait]
pub trait ExchangeApi: Send + Sync {
    async fn get_balance(&self, asset: &str) -> Result<Decimal, ExchangeError>;
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<OrderId, ExchangeError>;
    async fn amend_order(&self, order_id: &str, amend: AmendRequest) -> Result<(), ExchangeError>;
    async fn cancel_order(&self, order_id: &str) -> Result<(), ExchangeError>;
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, ExchangeError>;
}
```

### Trade Manager Rule Evaluation (per tick)
```rust
fn evaluate(position: &ManagedPosition, current_price: Decimal) -> Vec<Action> {
    let mut actions = vec![];
    let is_long = position.side == "LONG";
    let entry = position.entry_price;
    let target = position.target_price;
    let distance = (target - entry).abs();

    let progress = if is_long {
        (current_price - entry) / distance
    } else {
        (entry - current_price) / distance
    };

    // Break-even
    if !position.be_triggered
        && progress >= Decimal::from(position.break_even_at) / dec!(100)
    {
        actions.push(Action::AmendStop { new_price: entry });
    }

    // Trailing stop (only after BE)
    if position.be_triggered && position.trailing_enabled {
        let trail_dist = distance * Decimal::from(position.trailing_distance) / dec!(100);
        let new_stop = if is_long {
            current_price - trail_dist
        } else {
            current_price + trail_dist
        };
        let current_stop = position.current_stop.unwrap_or(entry);
        let should_move = if is_long {
            new_stop > current_stop
        } else {
            new_stop < current_stop
        };
        if should_move {
            actions.push(Action::AmendStop { new_price: new_stop });
        }
    }

    // Partial TP
    if !position.partial_tp_fired
        && position.partial_tp_enabled
        && progress >= dec!(1)
    {
        let close_qty = position.quantity
            * Decimal::from(position.partial_tp_percent) / dec!(100);
        actions.push(Action::PartialClose { quantity: close_qty });
    }

    actions
}
```

## Key Files

| File | Purpose |
|------|---------|
| `testudo-exchange/crates/router/src/services/trade_manager.rs` | Trade manager service — rule evaluation loop |
| `testudo-exchange/crates/router/src/services/exchange_api.rs` | `ExchangeApi` trait definition |
| `testudo-exchange/crates/router/src/services/binance_adapter.rs` | Binance futures implementation of `ExchangeApi` |
| `testudo-exchange/crates/router/src/routes/trade_management.rs` | REST endpoints (existing, extended) |
| `testudo-exchange/crates/router/src/services/execution_service.rs` | Updated to use `ExchangeApi` for position sizing |
| `testudo-exchange/crates/router/src/services/price_feed.rs` | Existing — trade manager subscribes to price ticks |
| `testudo-exchange/crates/router/src/main.rs` | Wire trade manager into app startup |
| `testudo-exchange/crates/ws-stream/src/lib.rs` | New event types for management actions |

## Architecture

### Service Interaction
```
POST /api/v1/trades
       │
       ▼
execution_service.rs
  ├── exchange_api.get_balance() → calculate quantity
  ├── exchange_api.place_order() → entry, stop, TP
  └── trade_manager.register(position, rules, order_ids)
       │
       ▼
trade_manager.rs (background task)
  ├── subscribes to price_feed channel
  ├── on each tick: evaluate() all active positions
  ├── on action: exchange_api.amend_order() / partial close
  ├── update managed_positions table
  └── emit WS event to user channel
```

### State Machine
```
pending → filled → managing → closed
           │         │          ▲
           │         ├── BE triggered
           │         ├── trailing active
           │         ├── partial TP fired
           │         └── stopped / TP hit ──┘
           │
           └── rejected (insufficient balance, exchange error)
```

### Price Feed Integration
The trade manager spawns as a `tokio::spawn` background task on startup. It receives price ticks from the existing `price_feed.rs` broadcast channel. On each tick, it iterates active positions for that symbol and evaluates rules. Actions are executed asynchronously against the exchange API.

### Persistence & Recovery
On startup, trade manager loads all positions with status `filled` or `managing` from PostgreSQL and resumes evaluation. This ensures no management gap across backend restarts or deployments.

## Verification
```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
# All tests pass including trade manager unit tests
# Integration tests verify rule evaluation sequences
```

## Acceptance Criteria
- [ ] Trade payload with `management` block parsed and stored correctly
- [ ] Position quantity calculated from exchange balance and risk %
- [ ] `ExchangeApi` trait implemented for Binance futures
- [ ] Break-even rule fires at correct threshold and amends stop
- [ ] Trailing stop moves only in profitable direction
- [ ] Partial TP closes correct percentage, fires once only
- [ ] Managed positions survive backend restart
- [ ] WebSocket events emitted for all management actions
- [ ] `cargo clippy --all-targets` clean
- [ ] `cargo test` passes all tests

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Exchange API rate limits on frequent amendments | Debounce trailing stop amendments (max 1 per 5 seconds per position) |
| Price gap jumps past break-even threshold | Evaluate `>=` not `==`, so gaps still trigger rules |
| Backend restart during active management | PostgreSQL persistence, reload on startup |
| Partial fill on entry order | Track filled quantity, only manage filled portion |
| Exchange rejects amendment (price moved) | Retry once with current market price, log failure, notify user via WS |

## Completion Signal

### Implementation Checklist
- [ ] All functional requirements implemented
- [ ] All acceptance criteria verified
- [ ] Code follows project constitution standards (`Result<T,E>`, `rust_decimal`, no unwrap)
- [ ] No new linting warnings introduced

### Testing Requirements
- [ ] `cargo test` passes (unit + integration)
- [ ] Trade manager rule evaluation unit tests cover: BE trigger, trailing movement, partial TP, direction guards, restart recovery
- [ ] Integration test: mock exchange adapter → place trade → feed price ticks → verify amendment sequence

### Done Signal
<promise>DONE</promise>
Output only when ALL criteria pass.
