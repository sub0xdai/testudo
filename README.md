# Testudo

Draw a position on TradingView. Hit Alt+X. The trade is sized against your risk rules, routed to your exchange, and managed automatically. Abstract the decision making which cooks most traders (oversizing)

## How it works

### [Exchange](testudo-exchange) 

Rust backend. It validates every trade before it touches your exchange account, sizes positions conservatively across multiple methods, and manages exits in real time.

### [Extension](testudo-extension) 

Overlays a panel on TradingView. Draw a position tool, hit Alt+X, done. Your risk config (how much per trade, max leverage, account limits) is pulled from the engine.

### Journal

Every trade, every fill, every management decision is recorded. See what's working across setups, timeframes, and market conditions. 

### [Agent Trading](AGENT_TRADING.md) 

AI agents (Hermes, OpenClaw, pi, Claude Code) can trade autonomously on Testudo using the same infrastructure. A REST API for trade execution, WebSocket channels for real-time fill and risk alerts, and a journal-as-memory interface that lets agents read their own performance history. Full paper-trading sandbox included.



## Running it

```sh
cd testudo-exchange
cp .env.example .env
cargo run --bin router
```

Extension: `cd testudo-extension && bun install && bun run build`, then load `dist/chrome` as an unpacked Chrome extension.

## License

AGPL-3.0.
