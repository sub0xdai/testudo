# Testudo

A trading overlay that validates, sizes, and manages positions across any exchange.

Draw a position on TradingView → Alt+X → the engine sizes it against your risk config, routes it to your exchange, and manages exits automatically.

## What's in here

| Directory | What |
|-----------|------|
| [`testudo-exchange/`](testudo-exchange) | Rust backend — engine, router, risk service, shadow order matching |
| [`testudo-extension/`](testudo-extension) | Browser extension — TradingView overlay, trade submission |
| `testudo-journal/` | Trading journal and analytics dashboard |
| `testudo-cex/` | CCXT sidecar for CEX connectivity |
| `testudo-web/` | Landing page (submodule) |
| `testudo-ops/` | Kubernetes deployment configs (submodule) |

## Quick start

```sh
# Backend
cd testudo-exchange
cp .env.example .env
cargo run --bin router

# Extension
cd ../testudo-extension
bun install && bun run build
# Load dist/chrome as unpacked extension
```

## Deploy

```sh
ssh your-server 'bash -s' < scripts/deploy.sh
```

## License

Backend and extension are AGPL-3.0. See [testudo-exchange/LICENSE](testudo-exchange/LICENSE) and [testudo-extension/LICENSE](testudo-extension/LICENSE).
