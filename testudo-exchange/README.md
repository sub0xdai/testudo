<p align="center">
    <img src="assets/logo.png" alt="Testudo Exchange" width="600" />
</p>

<p align="center">
    <strong>A trading overlay that validates, sizes, and manages positions across any exchange.</strong>
</p>

## What It Does

Testudo sits between your trading decisions and your exchange accounts. Every trade hits a validation gate — position sizing, margin checks, risk limits — all enforced in-memory. If it fails, the order never leaves the server.

Your strategy decides what to trade. Testudo decides how much and whether it's safe, then handles execution, monitoring, and exit management.

## Architecture

Three stages, one path:

1. **Validation** — Decision Loop checks exposure, Position Sizer picks quantity (smallest of account %, fixed risk, max size, margin capacity), Shadow Engine simulates against in-memory state. Rejected trades never leave.
2. **Execution** — Validated orders route to the exchange via CCXT (CEX) or native Hyperliquid SDK. Entry, stop-loss, and take-profit are placed as a linked group.
3. **Management** — Price Feed polls every 2 seconds. Trade Manager handles break-even, trailing stop, partial take-profit. Fill Detector listens via WebSocket and cancels OCO siblings when one leg fills.

If the server restarts, positions rehydrate from the database and verify against the exchange.

### Shadow Engine

In-memory mirror of all active positions and orders. Does two things: paper trading with full price-time-priority matching, and state tracking for live trades so decisions don't wait on API round-trips.

### Decision Loop

Single gate every trade passes through. Risk Service checks account exposure. Position Sizer takes the most conservative result across multiple sizing methods. When real money is on the line, the safest answer wins.

### Order Groups

Trades are groups, not individual orders. Entry, stop-loss, and take-profit link together with lifecycle tracking: Pending → Active → StoppedOut / TookProfit / Cancelled / Closed. One leg fills, the sibling cancels. Cancel a trade, all linked orders cancel atomically.

## Running It

### Prerequisites
- Rust (stable)
- PostgreSQL
- Node.js (for CCXT sidecar, optional — needed for live CEX trading)

### Setup

```sh
cp .env.example .env
# Set: DATABASE_URL, JWT_ACCESS_SECRET, JWT_REFRESH_SECRET, ENCRYPTION_KEY
```

### Start

```sh
# API server + all background services
cargo run --bin router

# WebSocket server
cargo run --bin ws-stream

# CCXT sidecar (for live exchange trading)
cd ../testudo-cex && bun run start
```

### Verify

```sh
cargo clippy --all-targets && cargo test
```

## API

All routes prefixed with `/api/v1`.

```
AUTH
  POST /auth/register         Register
  POST /auth/login            Login (returns JWT pair)
  POST /auth/refresh          Refresh access token
  POST /auth/logout           Invalidate session

TRADES
  POST /trades                Create trade with SL/TP
  GET  /trades                List active trades
  PUT  /trades/{id}/sl        Update stop-loss
  PUT  /trades/{id}/tp        Update take-profit
  PUT  /trades/{id}/breakeven Enable break-even automation
  DEL  /trades/{id}           Cancel trade + linked orders

EXCHANGES
  GET  /exchanges/accounts    List linked exchange accounts
  POST /exchanges/accounts    Link new API key
  DEL  /exchanges/accounts/{id} Remove account
  GET  /exchanges/accounts/{id}/balance  Fetch live balance

RISK
  GET  /risk-config           Get risk parameters
  PUT  /risk-config           Update risk parameters

MARKET DATA
  GET  /market-data/ticker    Live ticker (bid/ask/high/low)
  GET  /market-data/orderbook Live order book
  GET  /market-data/klines    Candlestick data

PAPER TRADING
  GET  /paper/balances        Shadow engine balances
  POST /paper/reset           Reset paper account

HEALTH
  GET  /health                Liveness
  GET  /metrics               Prometheus metrics
```

## Tech Stack

| Crate | Purpose |
|-------|---------|
| **engine** | Shadow Engine, OrderGroups, orderbook, balances |
| **router** | Actix-web server, Decision Loop, Risk Service, Hyperliquid SDK |
| **common_utils** | Position Sizer, risk types, exchange adapters |
| **pg_queue** | PostgreSQL queues + LISTEN/NOTIFY pub/sub |
| **sqlx_postgres** | Database client and migrations |
| **ws-stream** | WebSocket server (tokio-tungstenite) |

## License

AGPL-3.0. See [LICENSE](LICENSE).
