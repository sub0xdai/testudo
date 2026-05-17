# Testudo

A trading overlay that validates, sizes, and manages positions across any exchange — before the order ever leaves your machine.

Draw a position on TradingView. Hit Alt+X. The engine sizes the trade against your risk rules, routes it to your exchange, and manages exits automatically.

## What's in this repo

| Directory | Description |
|-----------|-------------|
| [`testudo-exchange/`](testudo-exchange) | Rust backend — engine, matching, risk, order management |
| [`testudo-extension/`](testudo-extension) | Browser extension — TradingView overlay, trade submission |
| `testudo-journal/` | Trading journal and analytics dashboard |
| `testudo-cex/` | CCXT sidecar for centralized exchange connectivity |

## Running it

Backend (needs Rust and PostgreSQL):

```sh
cd testudo-exchange
cp .env.example .env     # set DATABASE_URL, JWT secrets, encryption key
cargo run --bin router   # API on :8080
cargo run --bin ws-stream  # WebSocket on :4000
```

Extension:

```sh
cd testudo-extension
bun install && bun run build
# Load dist/chrome as an unpacked Chrome extension
```

## License

Backend and extension are AGPL-3.0.
