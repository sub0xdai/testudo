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

AI agents (Hermes, OpenClaw, pi) can trade autonomously on Testudo using the same infrastructure. A REST API for trade execution, WebSocket channels for real-time fill and risk alerts, and a journal-as-memory interface that lets agents read their own performance history. Full paper-trading sandbox included.

### [Sheaf Engine](sheaf-engine)

Cellular sheaf topology perception layer. Ingests multi-venue tick data, builds a topology graph of arbitrage and correlation edges, and streams structural signals (spread dislocation, volatility diffusion, venue health) to AI agents over gRPC. Gives agents real-time market structure awareness beyond raw OHLCV.

### [Strategy Proofs](testudo-proofs)

Lean 4 formal verification of strategy primitives. Proves properties like delta-neutrality, funding arbitrage bounds, Kelly-optimal sizing, and gambler's ruin avoidance. Ensures agent strategies are mathematically sound before they touch real capital.

## Beyond crypto

The exchange backend is not a crypto trading engine — it's a general-purpose financial infrastructure platform. The adapter layer abstracts venues; everything else is venue-agnostic.

| If you swapped the adapter layer... | You'd get |
|---|---|
| Interactive Brokers / Alpaca | **Equities/futures trading platform** — same risk engine, same journal, same coach |
| Stripe / Braintree | **Payment orchestration** — risk validator screens transaction amounts instead of position sizes |
| Prediction market (Polymarket/Kalshi API) | **Betting/forecasting platform** — Dignitas scores become forecaster reputation |
| In-game economy (EVE-style) | **Game marketplace** — matching engine runs buy/sell orders for virtual items |
| DeFi protocol (Uniswap pool) | **LP management agent** — agent API places liquidity positions instead of trades |
| Banking fraud detection | **Transaction screening** — risk validator checks withdrawals against balance limits |
| SaaS feature flags | **Graduated rollout** — shadow/live execution becomes canary/production traffic routing |
| Supply chain | **Order routing** — multi-venue fabric routes purchase orders to suppliers based on health/price |
| Any LLM agent with a wallet | **Autonomous economic agent** — shadow-first, coach-supervised, journal-as-memory loop |

## Running it

```sh
cd testudo-exchange
cp .env.example .env
cargo run --bin router
```

Extension: `cd testudo-extension && bun install && bun run build`, then load `dist/chrome` as an unpacked Chrome extension.

## License

AGPL-3.0.
