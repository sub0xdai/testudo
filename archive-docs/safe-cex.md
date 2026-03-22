# safe-cex Migration — Specification Series

## How to Use This Document

This document should be broken into a series of implementation specs using `/speckit.specify`. Each spec maps to one section below. Suggested spec sequence:

```
CEX-01: Fork safe-cex and strip broker IDs
CEX-02: Archive testudo-ccxt, scaffold testudo-cex
CEX-03: ExchangeGateway — safe-cex instance management
CEX-04: HTTP handlers — same contract, new engine
CEX-05: WebSocket fill streaming — event-driven OCO
CEX-06: Polling reconciler — orphaned order safety net
CEX-07: Symbol normalization and Rust backend updates
CEX-08: Integration testing — WOO X testnet end-to-end
```

Each spec should include acceptance criteria, key files, and test requirements. Specs are sequential — each builds on the previous.

---

# Replace CCXT Sidecar with safe-cex Fork

## Context

**Problem:** Orphaned limit orders after SL/TP triggers on WOO X. 15 fix attempts over 2 weeks addressed placement logic, but the real root cause is that **CCXT does not implement `watchOrders` for WOO X at all** — the base class throws `NotSupported`. The entire WebSocket fill detection path has never worked. OCO cancellation has never fired for WOO X.

**Why safe-cex:** The `safe-cex` library (MIT, gmtech-xyz/tuleep.trade) solves exactly this problem. For WOO X specifically, it subscribes to **both** `executionreport` AND `algoexecutionreportv2` WebSocket topics, providing fill events for regular orders AND algo/stop orders. It also maintains an internal Store (positions, orders, balances) with reactive updates. WOO X bracket orders use `POSITIONAL_TP_SL` algo type natively.

**Decisions:**
- **Fork safe-cex** — strip gmtech broker ID, maintain our own clean fork
- **Runtime: Bun** — already used in testudo-web and testudo-extension
- **Lean Rust engine** — Rust calculates size, sends payload, listens for success/fail events. Sidecar handles all exchange babysitting.
- **Archive `testudo-ccxt/`** — build `testudo-cex/` from scratch

## Architecture Shift

```
BEFORE (stateless proxy, dead WebSocket):
  Rust → HTTP → sidecar → CCXT → exchange
  Rust ← WS ← sidecar ← CCXT.watchOrders() ← NEVER WORKED FOR WOO X

AFTER (stateful gateway, event-driven):
  Rust → HTTP → sidecar → safe-cex → exchange
  Rust ← WS ← sidecar ← safe-cex.on("fill") ← executionreport + algoexecutionreportv2
```

Key difference: safe-cex maintains a persistent exchange connection with internal state. The sidecar becomes a **stateful gateway** that starts exchange instances on first use, keeps them alive, and pushes fill events to the Rust backend via WebSocket.

---

## CEX-01: Fork safe-cex and strip broker IDs

```bash
# Fork gmtech-xyz/safe-cex to your GitHub
# Clone as submodule or vendored dependency
git submodule add git@github.com:sub0xdai/safe-cex.git testudo-cex/vendor/safe-cex
```

- Strip broker ID injection from `src/exchanges/woo/woo.api.ts` (axios interceptor injects `broker_id`/`brokerId`)
- Strip any other exchange broker IDs (Binance, Bybit have similar interceptors)
- Keep everything else — the exchange implementations are battle-tested

**Acceptance Criteria:**
- Forked repo builds cleanly (`bun run build`)
- No broker IDs in any exchange API interceptor
- All existing safe-cex tests pass

## CEX-02: Archive old sidecar, scaffold testudo-cex

```bash
git mv testudo-ccxt testudo-ccxt-archived
```

Scaffold new sidecar:

```
testudo-cex/
├── package.json         # safe-cex (forked), express, ws, prom-client
├── tsconfig.json        # TypeScript config (Bun runtime)
├── src/
│   ├── server.ts        # Express + WS server (port 3100, same as before)
│   ├── gateway.ts       # ExchangeGateway — manages safe-cex instances per user
│   ├── handlers.ts      # HTTP route handlers (same endpoints as before)
│   ├── ws-fills.ts      # WebSocket fill streaming to Rust backend
│   ├── reconciler.ts    # Polling fallback — fetchOpenOrders safety loop
│   ├── types.ts         # Shared types, request/response shapes
│   ├── symbols.ts       # Symbol normalization (BTC_USDT ↔ BTCUSDT)
│   └── metrics.ts       # Prometheus metrics (reuse existing patterns)
└── tests/
    └── *.test.ts
```

**Acceptance Criteria:**
- `bun install` succeeds
- `bun run start` launches server on port 3100
- `GET /health` returns `{ok: true}`
- Old sidecar preserved in archive directory

## CEX-03: ExchangeGateway — safe-cex instance management

Manages one `safe-cex` exchange instance per (exchange_id, api_key) pair.

```typescript
import { createExchange, type Exchange } from "safe-cex";

class ExchangeGateway {
  private instances: Map<string, Exchange> = new Map();

  async getOrCreate(
    exchangeId: string,
    credentials: { key: string; secret: string; applicationId?: string; passphrase?: string },
    sandbox: boolean,
    onFill: (fill: FillEvent) => void
  ): Promise<Exchange> {
    const cacheKey = hash(exchangeId + credentials.key + sandbox);

    if (this.instances.has(cacheKey)) {
      return this.instances.get(cacheKey)!;
    }

    const exchange = createExchange(exchangeId as any, {
      key: credentials.key,
      secret: credentials.secret,
      applicationId: credentials.applicationId,
      testnet: sandbox,
    });

    // Wire fill events — this is the critical path that CCXT never had
    exchange.on("fill", onFill);
    exchange.on("error", (err) => console.error(`[${exchangeId}] error:`, err));
    exchange.on("log", (msg, severity) => console.log(`[${exchangeId}] ${severity}:`, msg));

    await exchange.start();  // establishes WS connections, fetches initial state
    this.instances.set(cacheKey, exchange);
    return exchange;
  }

  async dispose(cacheKey: string) { ... }
  async disposeAll() { ... }
}
```

**Key:** `exchange.on("fill", ...)` fires for BOTH regular and algo order fills on WOO X because safe-cex subscribes to both WebSocket topics internally.

**Acceptance Criteria:**
- Gateway creates and caches exchange instances
- `exchange.start()` establishes WebSocket connections
- Fill events fire for test scenarios
- Graceful disposal of instances

## CEX-04: HTTP handlers — same contract, new engine

Same endpoint contract as before — the Rust backend's `CcxtClient` needs minimal changes.

| Endpoint | safe-cex method | Notes |
|----------|----------------|-------|
| `POST /balance` | `exchange.store.balance` | Read from Store (no HTTP call) |
| `POST /order` | `exchange.placeOrder(opts)` | Returns `string[]` (order IDs) |
| `POST /order/cancel` | `exchange.cancelOrders([order])` | Find order in Store by ID |
| `POST /orders/cancel-all` | `exchange.cancelSymbolOrders(symbol)` | Cancel all for symbol |
| `POST /position` | `exchange.store.positions` | Read from Store |
| `POST /leverage` | `exchange.setLeverage(symbol, leverage)` | Direct call |
| `POST /orders/open` | `exchange.store.orders` | Read from Store |
| `POST /order/edit` | `exchange.updateOrder({order, update})` | Cancel+replace internally |
| `GET /health` | Check Store loaded flags | `store.loaded.balance && store.loaded.orders` |

**Response format stays the same** — all numerics as strings, same field names. The Rust `CcxtClient` and `SidecarOrderResponse` struct don't need changes for basic operations.

### Request envelope (unchanged):
```json
{
  "exchange_id": "woo",
  "credentials": { "apiKey": "...", "secret": "...", "password": "..." },
  "sandbox": false,
  "params": { ... }
}
```

### Bracket Orders (entry + SL + TP):
```typescript
// safe-cex handles this natively per exchange
const orderIds = await exchange.placeOrder({
  symbol: "BTCUSDT",
  type: OrderType.Limit,
  side: OrderSide.Buy,
  amount: 0.01,
  price: 70000,
  stopLoss: 69000,      // auto-creates SL order (algo on WOO X)
  takeProfit: 72000,     // auto-creates TP order (algo on WOO X)
});
// Returns: ["entry-id", "sl-id", "tp-id"]
```

Response maps to existing `SidecarOrderResponse` with `stop_loss_order_id` and `take_profit_order_id`.

**Acceptance Criteria:**
- All 10 HTTP endpoints respond with same shapes as old sidecar
- Bracket order placement returns entry + SL + TP order IDs
- Balance/position reads come from Store (no redundant HTTP calls to exchange)
- Error mapping matches existing `CcxtClientError` enum (401, 402, 404, 429, 502)

## CEX-05: WebSocket fill streaming — event-driven OCO

Replace the dead `watchOrders` loop with safe-cex's event-driven fills.

```typescript
// When Rust backend connects to /ws/orders and subscribes:
exchange.on("fill", (fill) => {
  // Find the order in the Store that just filled
  // Emit to Rust backend via WebSocket
  ws.send(JSON.stringify({
    event: "order_update",
    data: {
      id: matchedOrder.id,
      symbol: fill.symbol,
      status: "closed",
      side: fill.side,
      price: matchedOrder.price,
      amount: matchedOrder.amount,
      filled: matchedOrder.filled,
      remaining: matchedOrder.remaining,
      average: fill.price,
      timestamp: Date.now(),
    }
  }));
});

// Also emit on Store order removals (cancellations)
exchange.on("update", (store) => {
  // Diff previous orders vs current — detect removals/status changes
  // Emit "canceled" status for removed orders
});
```

**The response shape stays identical to what `fill_detector.rs` expects** — `OrderUpdateEvent` with `id`, `symbol`, `status`, `side`, `price`, `amount`, `filled`, `remaining`, `average`, `timestamp`.

**Acceptance Criteria:**
- Fill events for regular orders (limit, market) arrive at Rust backend
- Fill events for algo orders (stop-market SL, conditional TP) arrive at Rust backend
- Cancellation events arrive at Rust backend
- Event shape matches existing `OrderUpdateEvent` struct exactly
- `fill_detector.rs` can process events without any changes

## CEX-06: Polling reconciler — orphaned order safety net

Safety net for dropped WebSocket packets. Runs every 15 seconds.

```typescript
async function reconcile(exchange: Exchange, activeGroups: Map<string, GroupInfo>) {
  const storeOrders = exchange.store.orders;
  const storePositions = exchange.store.positions;

  for (const [groupId, group] of activeGroups) {
    // If position is gone but group is Active → force cancel all sibling orders
    const hasPosition = storePositions.some(p => p.symbol === group.symbol);
    if (!hasPosition && group.status === "active") {
      await exchange.cancelSymbolOrders(group.symbol);
      // Emit synthetic fill event so Rust backend updates group status
    }
  }
}
```

This catches the exact scenario: SL fills (position closes), but the fill event was missed — the reconciler sees no position and cancels the orphaned TP.

**Acceptance Criteria:**
- Reconciler runs on configurable interval (default 15s)
- Detects orphaned orders when position is gone
- Cancels orphaned orders via `cancelSymbolOrders`
- Emits synthetic events so Rust backend updates state
- Does not interfere with normal fill processing

## CEX-07: Symbol normalization and Rust backend updates

### Symbol mapping:
```typescript
// safe-cex uses "BTCUSDT" internally
// Rust backend uses "BTC_USDT" internally

function fromInternal(symbol: string): string {
  // "BTC_USDT" → "BTCUSDT"
  return symbol.replace("_", "");
}

function toInternal(symbol: string): string {
  // "BTCUSDT" → "BTC_USDT"
  // Use market data to split correctly
}
```

### Rust changes:

**`ccxt_client.rs` → `cex_client.rs` (rename)**
1. Symbol conversion: `to_ccxt_symbol("BTC_USDT")` changes from `"BTC/USDT:USDT"` → `"BTCUSDT"`
2. `SidecarOrderResponse`: Handle `string[]` return from `placeOrder`
3. All other HTTP client code stays the same

**`exchange_api.rs` — no changes**
The `ExchangeApi` trait stays identical. Only `to_ccxt_symbol()` / `from_ccxt_symbol()` update.

**`fill_detector.rs` — simplification opportunity**
With safe-cex handling reliable fill events, the fill_detector finally receives data on its `order_rx` channel. The existing OCO logic (`cancel_all_related_orders`) works as-is. Defense in depth.

**`trade_management.rs` — leaner flow**
With safe-cex's `placeOrder({ stopLoss, takeProfit })`, the 3-step sequential placement (entry → SL → TP) becomes a single call. The deferred placement logic and instant-fill detection can be removed — safe-cex handles the sequencing internally per exchange.

**Acceptance Criteria:**
- Symbol conversion works for all traded pairs
- `cargo clippy --all-targets` passes
- `cargo test` passes (all 814+ tests)
- Rust backend communicates with new sidecar using updated symbol format

## CEX-08: Integration testing — WOO X testnet end-to-end

```bash
# 1. New sidecar builds and starts
cd testudo-cex && bun install && bun run build && bun run start

# 2. Health check
curl http://127.0.0.1:3100/health

# 3. Backend tests
cd testudo-exchange && cargo clippy --all-targets && cargo test

# 4. Integration test: place bracket order on WOO X testnet
# Verify fill events arrive via WebSocket for entry, SL, TP
# Verify OCO: trigger SL → confirm TP is cancelled

# 5. Reconciler test: simulate dropped WebSocket packet
# Verify reconciler detects orphaned orders and cancels them
```

**Acceptance Criteria:**
- Bracket order (entry + SL + TP) placed on WOO X testnet
- Entry fill event received by Rust fill_detector
- SL fill event received by Rust fill_detector (algo stream)
- OCO fires: SL triggers → TP cancelled automatically
- Reconciler catches any missed events
- No orphaned orders after trade lifecycle completes
- Deploy and verify on live WOO X with small position

---

## Reference: safe-cex API Surface

### Initialization
```typescript
const exchange = createExchange("woo", {
  key: string,
  secret: string,
  applicationId?: string,    // WOO X only
  passphrase?: string,       // OKX, Bitget, Blofin only
  testnet?: boolean,
});
await exchange.start();
```

### Events
```typescript
exchange.on("fill", (fill: { amount: number; price: number; side: "buy"|"sell"; symbol: string }) => {});
exchange.on("update", (store: Store) => {});
exchange.on("error", (error: string) => {});
exchange.on("log", (message: string, severity: "warning"|"error"|"info") => {});
```

### Order Placement
```typescript
const orderIds: string[] = await exchange.placeOrder({
  symbol: string,
  type: "market" | "limit" | "stop_market" | "take_profit_market" | "trailing_stop_market",
  side: "buy" | "sell",
  amount: number,
  price?: number,
  stopLoss?: number,
  takeProfit?: number,
  reduceOnly?: boolean,
  timeInForce?: "GoodTillCancel" | "ImmediateOrCancel" | "FillOrKill" | "PostOnly",
});
```

### Order Management
```typescript
await exchange.cancelOrders([order]);
await exchange.cancelSymbolOrders("BTCUSDT");
await exchange.cancelAllOrders();
await exchange.updateOrder({ order, update: { price?: number; amount?: number } });
await exchange.nuke();  // emergency close all positions + cancel all orders
```

### Store (reactive state)
```typescript
exchange.store.balance    // { used, free, total, upnl }
exchange.store.orders     // Order[]
exchange.store.positions  // Position[]
exchange.store.tickers    // Ticker[]
exchange.store.markets    // Market[]
exchange.store.loaded     // { balance, orders, markets, tickers, positions }
```

### Supported CEXs
| Exchange | ID | Notes |
|----------|----|-------|
| WOO X | `"woo"` | Requires `applicationId`. `POSITIONAL_TP_SL` algo type. |
| Binance | `"binance"` | Futures API. Batch orders (groups of 5). |
| Bybit | `"bybit"` | Unified accounts only. Batch orders (groups of 10). |
| OKX | `"okx"` | Requires `passphrase`. |
| Gate.io | `"gate"` | Standard implementation. |
| Bitget | `"bitget"` | Requires `passphrase`. |
| Blofin | `"blofin"` | Requires `passphrase`. |
| Phemex | `"phemex"` | Cross-mode leverage. |

---

## Risks & Mitigations

1. **JS floats, not Decimal** — stringify all numerics at the sidecar boundary before sending to Rust. safe-cex has `adjust()` + market precision metadata internally.
2. **Browser-first (axios)** — verify Bun compatibility without CORS proxy. axios works fine in Bun/Node.
3. **No fill history** — safe-cex emits fills in real-time but doesn't store them. Every `fill` event must be captured and forwarded. The reconciler catches any missed events.
4. **Store is in-memory** — sidecar restart loses state. Rust backend's PostgreSQL + rehydration handles persistence. safe-cex re-fetches state on `start()`.
5. **Broker ID stripped** — our fork strips gmtech's broker IDs. Must maintain fork when safe-cex updates.
6. **`nuke()` retries only 3x** — if emergency close fails after 3 retries, it silently gives up. May need to increase or add alerting.

---

## Existing Contracts (preserved)

### Request Envelope (unchanged)
```json
{
  "exchange_id": "woo",
  "credentials": { "apiKey": "...", "secret": "...", "password": "..." },
  "sandbox": false,
  "params": { ... }
}
```

### WebSocket Event Shape (unchanged)
```json
{
  "event": "order_update",
  "data": {
    "id": "string",
    "symbol": "string",
    "status": "closed|canceled",
    "side": "buy|sell",
    "price": number,
    "amount": number,
    "filled": number,
    "remaining": number,
    "average": number,
    "timestamp": number
  }
}
```

### Rust Types (unchanged)
- `OrderUpdateEvent` in `ccxt_client.rs`
- `SidecarOrderResponse` in `ccxt_client.rs`
- `ExchangeApi` trait in `exchange_api.rs`
- `PlaceOrderRequest` / `PlaceOrderResult` in `exchange_api.rs`
