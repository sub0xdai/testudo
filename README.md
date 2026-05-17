# Testudo

**You decide what to trade. Testudo handles everything else.**

Draw a position on TradingView. Hit Alt+X. The trade is sized against your risk rules, routed to your exchange, and managed automatically — stop-losses, take-profits, break-even, trailing stops. You go back to looking at charts.

## How it works

Three pieces that fit together:

### [Exchange](testudo-exchange) — the engine

The Rust backend that does the actual work. It validates every trade before it touches your exchange account, sizes positions conservatively across multiple methods, and manages exits in real time. Paper trading built in — practice strategies against live prices with no real money at risk.

### [Extension](testudo-extension) — how you trade

Overlays a panel on TradingView. Draw a position tool, hit Alt+X, done. Your risk config (how much per trade, max leverage, account limits) is pulled from the engine. The extension doesn't make decisions — it sends what you draw to the engine and shows you what happens next.

### Journal — where you learn

Every trade, every fill, every management decision is recorded. See what's working across setups, timeframes, and market conditions. Not just P&L — why trades closed, when break-even triggered, how your sizing held up. The data that turns trading from gambling into a repeatable process.

## The idea

Most trading tools are either too simple (a calculator) or too complex (a full automation platform with a PhD-level learning curve). Testudo sits in the middle: you stay in control of what to trade, but the math, the risk checks, and the exit management happen automatically. The engine is conservative by design — if your account can't handle the position, the order never leaves your machine. When real money is on the line, the safest answer wins.

## Running it

```sh
cd testudo-exchange
cp .env.example .env
cargo run --bin router
```

Extension: `cd testudo-extension && bun install && bun run build`, then load `dist/chrome` as an unpacked Chrome extension.

## License

AGPL-3.0.
