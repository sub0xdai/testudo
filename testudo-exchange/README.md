<p align="center">
    <img src="assets/logo.png" alt="Testudo Exchange" width="600" />
</p>

<p align="center">
    <strong>A trading overlay that validates, sizes, and manages positions across any exchange.</strong>
    <br /> <br />
    <a href="#what-it-does"><strong>What It Does</strong></a> ·
    <a href="#architecture"><strong>Architecture</strong></a> ·
    <a href="#the-engine"><strong>The Engine</strong></a> ·
    <a href="#use-cases"><strong>Use Cases</strong></a> ·
    <a href="#tech-stack"><strong>Tech Stack</strong></a> ·
    <a href="#api-endpoints"><strong>API Endpoints</strong></a> ·
    <a href="#local-development"><strong>Local Development</strong></a>
</p>

## What It Does

Testudo sits between your trading decisions and your exchange accounts. Every trade hits a validation gate before it touches the exchange — position sizing, margin checks, risk limits — all enforced in-memory. If it fails, the order never leaves the server.

The idea is simple: your strategy decides what to trade, Testudo decides how much and whether it's safe, then handles execution, monitoring, and exit management. It connects to any exchange through [CCXT](https://github.com/ccxt/ccxt) adapters, so your risk rules follow you no matter where you trade.

## Architecture

![Testudo Architecture](assets/testudo-architecture.png)

Three stages, one path:

1. **Validation** — The Router runs every trade through the Decision Loop. Risk Service checks account exposure, Position Sizer picks the quantity (smallest of account %, fixed risk, max size, margin capacity), and the Shadow Engine simulates the trade against in-memory state. Rejected trades never leave the server.

2. **Execution** — Validated orders route to the exchange. CEX orders (Binance, Bybit, etc.) go through the Node.js CCXT sidecar. Hyperliquid orders use the native Rust SDK. Entry, stop-loss, and take-profit are placed as a linked group tracked by `clientOrderId` convention (`testudo:{group_id}:{role}`).

3. **Management** — Background services take over. Price Feed polls every 2 seconds. Trade Manager evaluates break-even, trailing stop, and partial take-profit on each tick. Fill Detector listens for fills via WebSocket and handles OCO cancellation (SL fills cancel TP, vice versa). State changes broadcast to clients through PostgreSQL LISTEN/NOTIFY. If the server restarts, positions rehydrate from the database and verify against the exchange — no manual recovery.

## The Engine

The engine does the actual work. The API, the extension, the sidecar — they all exist to feed trades into it and act on its output.

### Shadow Engine

An in-memory mirror of every active position and order. Uses `RwLock` for global state and `DashMap` for lock-free per-user balance access. Two jobs:

- **Paper trading** — Full order matching against live prices with BTreeMap price-time priority. Place, fill, cancel without touching a real exchange.
- **State tracking** — For live trades, the Shadow Engine mirrors exchange state so decisions don't wait on API round-trips.

### Decision Loop

Single gate every trade passes through:

```
Request → Decision Loop → Risk Service → Position Sizer → Shadow Engine → Execute
```

The Decision Loop enforces risk rules that can't be bypassed. The Position Sizer picks the most conservative result across multiple sizing methods. When real money is on the line, the safest answer wins.

### Order Groups

Trades are groups, not individual orders. Entry, stop-loss, and take-profit link together as one `OrderGroup` with tracked lifecycle:

```
Pending → Active → StoppedOut / TookProfit / Cancelled / Closed
```

One leg fills, Fill Detector cancels the sibling. Cancel a trade, all linked orders cancel atomically. Exchange order IDs indexed for sub-millisecond fill event lookup.

### Trade Management

Once a position is active, Trade Manager runs on every price tick:

- **Break-even** — Moves stop-loss to entry when profit hits threshold
- **Trailing stop** — Ratchets the stop-loss up as price moves
- **Partial take-profit** — Closes a percentage at target levels
- **5-second debounce** — One amendment per position per interval

Everything persisted to PostgreSQL, broadcast to clients.

## Use Cases

The engine exposes a REST API. Anything that speaks HTTP can use it.

### Browser Extension

The primary client. Overlays TradingView, reads chart drawings (position tool), submits trades via Alt+X. Shows live positions, balances, and fill notifications. Built in TypeScript with Solid.js.

### Trading Bots

Connect any algorithmic strategy. The engine handles sizing, risk limits, and position management. Your bot sends entry signals. Multiple concurrent users with isolated risk configs.

### Portfolio Dashboard

`/trades`, `/exchanges/accounts/{id}/balance`, and `/market-data` give you a real-time portfolio view across exchanges. WebSocket subscriptions deliver live order updates.

### Trading Journal

Historical trades, management events, and risk decisions are all persisted. Captures what happened and why: which risk rule sized the position, when break-even triggered, how the trailing stop moved.

### Multi-Strategy

Multiple strategies, one account, shared risk limits. Each strategy submits independently. The engine keeps aggregate exposure within bounds.

## Tech Stack

| Crate | Purpose |
|-------|---------|
| **engine** | Shadow Engine, OrderGroups, BTreeMap orderbook, DashMap balances |
| **router** | Actix-web HTTP server, Decision Loop, Risk Service, native Hyperliquid integration |
| **common_utils** | Position Sizer, risk types, exchange adapters |
| **pg_queue** | PostgreSQL SKIP LOCKED queues + LISTEN/NOTIFY pub/sub + UNLOGGED cache |
| **sqlx_postgres** | Database client and migrations |
| **ws-stream** | WebSocket server (tokio-tungstenite, port 4000) |
| **db-processor** | Database write operations |

- **Runtime:** Rust (Tokio, Actix-web)
- **Database:** PostgreSQL (SQLx) — persistence, queues, pub/sub, caching
- **Exchange:** Native Hyperliquid SDK, CCXT via Node.js sidecar (port 3100)
- **WebSockets:** Tokio-Tungstenite with TCP_NODELAY
- **Observability:** Structured JSON logging (tracing), Prometheus metrics
- **Security:** JWT auth, AES-256-GCM credential encryption, CORS allowlisting

## API Endpoints

All routes prefixed with `/api/v1`.

```
AUTH
  POST /auth/register              Register new account
  POST /auth/login                 Login, returns JWT pair
  POST /auth/refresh               Refresh access token
  POST /auth/logout                Invalidate session
  POST /auth/forgot-password       Request password reset
  POST /auth/reset-password        Complete password reset

TRADES (managed positions with SL/TP)
  POST /trades                     Create trade with stop-loss & take-profit
  GET  /trades                     List active trades
  GET  /trades/{id}                Get single trade
  PUT  /trades/{id}/sl             Update stop-loss price
  PUT  /trades/{id}/tp             Update take-profit price
  PUT  /trades/{id}/entry          Update entry price (pending only)
  PUT  /trades/{id}/breakeven      Enable break-even automation
  GET  /trades/{id}/management     Get management rule state
  DEL  /trades/{id}                Cancel trade + linked orders

ORDERS (shadow engine — paper trading)
  POST /order                      Place order
  GET  /order                      Get open order
  DEL  /order                      Cancel order
  GET  /orders                     Get all open orders
  DEL  /orders                     Cancel all orders

EXCHANGES
  GET  /exchanges                  List connected exchanges
  GET  /exchanges/supported        List supported exchange types
  GET  /exchanges/accounts         Get user's linked accounts
  POST /exchanges/accounts         Link new API key
  DEL  /exchanges/accounts/{id}    Remove linked account
  POST /exchanges/accounts/{id}/test   Test connectivity
  GET  /exchanges/accounts/{id}/balance  Fetch live balance

RISK
  GET  /risk-config                Get risk parameters
  PUT  /risk-config                Update risk parameters

MARKET DATA
  GET  /market-data/ticker         Live ticker (bid/ask/high/low)
  GET  /market-data/orderbook      Live order book
  GET  /market-data/klines         Candlestick data
  GET  /market-data/markets        Available trading pairs
  GET  /v2/market-data/orderbook   Columnar format (~25% smaller)

PAPER TRADING
  GET  /paper/balances             Shadow engine balances
  POST /paper/reset                Reset paper account

LEGACY / UTILITY
  GET  /trade-history              Executed trades (cached 5s)
  GET  /depth                      Order book depth
  GET  /klines                     Historical candles
  GET  /tickers                    All tickers

SYNC
  POST /sync                       Trigger manual position sync
  GET  /sync/status                Last sync result
  GET  /sync/diff                  Position differences

HEALTH & OBSERVABILITY
  GET  /health                     Liveness probe
  GET  /health/ready               Readiness probe (DB + sidecar)
  GET  /health/sidecar             CCXT sidecar status
  GET  /metrics                    Prometheus metrics
```

## Local Development

Needs Rust, PostgreSQL (or Docker), and Node.js for the CCXT sidecar.

```sh
git clone https://github.com/sub0xdai/testudo-exchange.git
cd testudo-exchange
cp .env.example .env
# Set DATABASE_URL, JWT_ACCESS_SECRET, JWT_REFRESH_SECRET, ENCRYPTION_KEY
```

Three terminals:

```sh
# Terminal 1: Router
cargo run --bin router

# Terminal 2: WebSocket server
cargo run --bin ws-stream

# Terminal 3: CCXT sidecar (for live trading)
cd ../testudo-ccxt && bun run start
```

Verify:

```sh
cargo clippy --all-targets && cargo test
```

## License

AGPL-3.0. See [LICENSE](LICENSE).
