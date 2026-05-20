# Agent Integration Blueprint

> High-impact, low-effort integration vectors for coding agents (Hermes, OpenClaw, pi) to trade autonomously on Testudo.
>
> Generated from full architecture analysis — May 2026.

---

## Table of Contents

1. [Signal Endpoint](#1-signal-endpoint) — High Impact, Low Effort
2. [Agent WebSocket Alerts](#2-agent-websocket-alerts) — Medium Impact, Low Effort
3. [Journal as Agent Memory](#3-journal-as-agent-memory) — Medium Impact, Low-Medium Effort
4. [Implementation Roadmap](#implementation-roadmap)
5. [Agent Onboarding Flow](#agent-onboarding-flow)

---

## Quick Context

Testudo is a Rust-based crypto exchange platform with a comprehensive REST API (`/api/v1/`), WebSocket real-time streaming, a shadow (paper) trading engine, a risk engine with 8 validation checks + multi-method sizing, and an AI coach pipeline. The exchange supports both CEX (Binance, Bybit, OKX via a Node.js sidecar) and DEX (Hyperliquid via native Rust SDK).

The platform already has **agent wallet support** for Hyperliquid — cryptographic keypairs that trade on behalf of a user via EIP-712 approval. This is the primary foot-in-the-door for coding agents.

---

## 1. Signal Endpoint

> **Impact: High** — Unlocks programmatic trading for all agents
> **Effort: Low** — New route + thin service layer, reuses existing DecisionLoop
> **Files touched:** ~3 new, ~2 modified

### The Gap

Today, trading can only be initiated through the browser extension (Alt+X on TradingView → DOM scrape → modal → confirm). There is no programmatic way for an external agent to submit a trade signal. A coding agent working from a chart analysis, an on-chain event, or a sentiment signal has no API to act on it.

### The Solution

A dedicated signal endpoint that agents call directly, bypassing TradingView entirely:

```
POST /api/v1/signals
Authorization: Bearer <token>

{
  "symbol": "BTC_USDT",
  "side": "LONG",
  "entry_price": "50000.00",
  "stop_loss": "49000.00",
  "take_profit": [
    { "price": "51500.00", "quantity": "0.25" },
    { "price": "53000.00", "quantity": "0.25" }
  ],
  "exchange_account_id": "uuid",
  "execution_mode": "shadow",           // or "live"
  "reasoning": "RSI divergence 4H + volume spike at support",
  "source": "agent:hermes_v1.2",        // attribution
  "confidence": 0.72,                   // optional, stored in journal
  "idempotency_key": "uuid"
}
```

### What Reuses Existing Infrastructure

| Existing Component | Reused For |
|---|---|
| `DecisionLoop::execute()` | Risk validation + position sizing |
| `RiskService::validate()` | All 8 safety checks |
| `CexExchangeApi::place_order()` | CEX order routing |
| `HyperliquidExchangeApi::place_order()` | HL order routing |
| `ShadowExchangeApi::place_order()` | Paper trading |
| `journal_service.rs` | Recording trades with source attribution |
| `TradePayloadSchema` (Zod) | Base schema, extended with agent fields |

### New Surface Area

```
crates/router/src/
├── routes/signal.rs          # POST /api/v1/signals handler
├── services/agent_signal.rs  # Orchestrates signal → trade
├── models/agent_signal.rs    # SignalInput, SignalResult types
```

### Acceptance Criteria

- [x] Any agent with valid auth can submit a signal
- [x] Signal passes through DecisionLoop (never bypasses risk engine)
- [x] Shadow mode supported for strategy testing
- [x] `reasoning` and `source` fields stored in journal for attribution
- [x] `confidence` field stored for future Kelly calibration
- [x] Idempotency key prevents double-execution
- [x] Response includes `trade_group_id`, entry order ID, sizing method used, warnings

---

## 2. Agent WebSocket Alerts

> **Impact: Medium** — Real-time awareness for agents
> **Effort: Low** — New WebSocket channels, reuses existing pg_queue pub/sub
> **Files touched:** ~2 new, ~1 modified

### The Gap

Agents today have no real-time awareness of exchange state. They must poll REST endpoints to check if an order filled, if a stop-loss triggered, or if a drawdown limit was breached. This is both inefficient and slow — an agent may act on stale data.

### The Solution

Dedicated WebSocket channels for agent-critical events:

```json
// Agent alert: risk breach
{
  "stream": "agent.alert.550e8400-e29b-41d4-a716-446655440000",
  "data": {
    "type": "risk_breach",
    "severity": "warning",
    "message": "Daily drawdown at 4.2% — approaching 5% limit",
    "current_drawdown_pct": 4.2,
    "limit_pct": 5.0,
    "timestamp": "2026-05-20T14:30:00Z"
  }
}

// Agent execution report
{
  "stream": "agent.execution.550e8400-e29b-41d4-a716-446655440000",
  "data": {
    "type": "execution_report",
    "trade_group_id": "uuid",
    "order_id": "12345",
    "status": "filled",
    "fill_price": "50000.00",
    "latency_ms": 342,
    "exchange": "hyperliquid",
    "timestamp": "2026-05-20T14:30:01Z"
  }
}
```

### Channels

| Channel | Event Types |
|---|---|
| `agent.alert.{user_id}` | Risk breaches, drawdown warnings, margin calls, agent wallet expiry |
| `agent.execution.{user_id}` | Fill confirmations, order rejections, latency breakdowns, cancellation reports |
| `agent.order.{user_id}` | Order lifecycle: placed → partially filled → filled → cancelled → expired |
| `agent.balance.{user_id}` | Balance snapshots after each fill, daily PnL updates |

### What Reuses Existing Infrastructure

| Existing Component | Reused For |
|---|---|
| `pg_queue::notify()` | Already broadcasts events to WebSocket server |
| `ws-stream` crate | Already handles subscription management |
| `RiskService::validate()` warnings | Source of risk breach alerts |
| `CexClient` / HL SDK order responses | Source of execution reports |
| Existing `order.{user_id}` channel | Extend with agent-specific metadata |

### Why `pg_queue` Makes This Cheap

The WebSocket server already subscribes to PostgreSQL `LISTEN/NOTIFY` channels. The router calls `pg_notify('order_events', payload)` after every order event. Agent alert channels are just additional notification channels — no new infrastructure needed.

---

## 3. Journal as Agent Memory

> **Impact: Medium** — Gives agents trading history awareness
> **Effort: Low-Medium** — New query endpoints on existing journal data
> **Files touched:** ~2 new, ~2 modified

### The Gap

The journal stores every trade, fill, tag, note, and management decision. The analytics layer computes equity curves, win rates, R-multiples, symbol breakdowns, and time distributions. But an agent can't ask a natural question like "how did my ETH breakouts perform in Q1?" or "what's my win rate on trades where I moved the stop-loss?" without parsing raw analytics JSON themselves.

### The Solution

An agent-facing journal query layer:

```
GET /api/v1/journal/agent/summary
Authorization: Bearer <token>

{
  "query": "ETH setups with win rate above 50% in the last 90 days",
  "timeframe": "90d",
  "format": "llm"          // "json" or "llm" (markdown for LLM context window)
}
```

Response (format: "llm"):

```markdown
## Journal Summary: ETH Setups (Last 90 Days)

### Overall Performance
- Total ETH trades: 47
- Win rate: 53.2%
- Avg R-multiple: 1.8
- Total P&L: +$3,420.50
- Max drawdown: -$890.00

### By Setup Tag
| Setup | Trades | Win Rate | Avg R | P&L |
|---|---|---|---|---|
| breakout | 12 | 58.3% | 2.1 | +$1,240 |
| support_bounce | 15 | 60.0% | 1.9 | +$1,850 |
| trend_follow | 8 | 37.5% | 0.8 | -$420 |
| reversal | 12 | 50.0% | 1.6 | +$750 |

### Top Performers
- [T-a3f2b1c4] ETH_USDT long — breakout at 3500, 4.2R, opened 2026-03-15
- [T-b7c1d2e3] ETH_USDT short — support break at 3200, 3.1R, opened 2026-04-02
- [T-c1d2e3f4] ETH_USDT long — trend continuation at 3800, 2.8R, opened 2026-04-28

### Actionable Insights
- Breakout setups on ETH show edge: 58.3% WR, 2.1 avg R
- Trend-follow setups underperform: 37.5% WR, consider reducing size or avoiding
- All losing trades had stop-loss < 1.5% from entry — tight stops may be causing premature exits
```

### What Reuses Existing Infrastructure

| Existing Component | Reused For |
|---|---|
| `journal_stats.rs` `StatsEngine` | All analytics computation |
| `journal_timeseries.rs` | Equity curve, time distribution |
| `CoachDigest` types | Ad-hoc digest format |
| `journal_service.rs` `record_trade_close()` | Raw trade data |
| Tag system (`/journal/tags`) | Setup-based filtering |

### New Endpoints

| Endpoint | Description |
|---|---|
| `GET /journal/agent/summary` | Structured summary with filters |
| `GET /journal/agent/insights` | Actionable insights (low win-rate setups, stop distance analysis, session timing patterns) |
| `GET /journal/agent/compare` | Compare two time periods or two strategies |

### Acceptance Criteria

- [ ] Agent can query journal with timeframes and filters
- [ ] `format=llm` returns markdown suitable for LLM context windows (structured, citation-linked trade IDs)
- [ ] `format=json` returns the raw data for programmatic consumption
- [ ] Setup-based breakdowns use existing tag system
- [ ] Insights endpoint surfaces the same patterns the coach pipeline detects (sizing drift, frequency spikes, session anomalies)
- [ ] Response includes actionable recommendations (not just raw data)

---

## Implementation Roadmap

```
Week 1           Week 2           Week 3           Week 4
├───────────────┼────────────────┼────────────────┼────────────────┤
│                                                              │
│ Signal Endpoint   │ Agent WS Alerts  │ Journal Memory  │ Integration │
│ (3 days)          │ (2 days)         │ (5 days)        │ tests       │
│                   │                  │                  │ (3 days)    │
│                   │                  │                  │             │
│ POST /signals     │ agent.alert.*    │ /agent/summary   │ Hermes SDK  │
│ DecisionLoop      │ agent.execution.*│ /agent/insights  │ example     │
│ Shadow/Live mode  │ pg_queue notify  │ LLM format       │ E2E tests   │
│ Journal attribution│                 │ Compare periods  │ Docs        │
└─────────────────────────────────────────────────────────────┘
```

### Risk-Mitigated Rollout

1. **Shadow-first**: All agent functionality launches in shadow mode. No real money at risk.
2. **Agent-only risk config**: Agents get stricter risk limits than the human user (lower max positions, smaller position sizes, tighter drawdown limits).
3. **Journal audit trail**: Every agent action is recorded with `source: "agent:..."` for full traceability.
4. **Graduated live**: Agent graduates to live after shadow-mode performance meets thresholds (configurable, e.g., 50+ trades, >45% win rate, positive R-multiple).

### Verification Commands

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# If agent SDK is TypeScript
cd packages/testudo-agent-sdk && bun run build && bun test

# Extension
cd testudo-extension && bun run build
```

---

## Agent Onboarding Flow

Here's the complete flow for a coding agent (Hermes, OpenClaw, or a pi-harnessed Claude) to start trading on Testudo:

### Step 1: Auth — Get API Access

```
# Get SIWE nonce
curl -s https://api.testudo.vip/api/v1/auth/nonce

# Sign EIP-4361 message with agent's wallet
# (or use agent-wallet keypair if already generated)

# Verify signature, get bearer token
curl -X POST https://api.testudo.vip/api/v1/auth/verify-siwe \
  -H "Content-Type: application/json" \
  -d '{"message": "...", "signature": "0x..."}'

# Response: { "tokens": { "access_token": "...", "refresh_token": "...", "expires_in": 900 } }
```

### Step 2: Configure Exchange Account

```
# Hyperliquid (agent wallet path)
curl -X POST https://api.testudo.vip/api/v1/exchanges/agent-wallet/init \
  -H "Authorization: Bearer $TOKEN"
# → Returns agent wallet address + keypair (store securely!)

# User approves via MetaMask (handled in Desk UI)
curl -X POST https://api.testudo.vip/api/v1/exchanges/agent-wallet/approve \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"agent_address": "0x...", "signature": "0x...", "nonce": 1710600000000}'

# Or CEX (API key path)
curl -X POST https://api.testudo.vip/api/v1/exchanges/accounts \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"exchange_name": "binance", "api_key": "...", "secret": "...", "account_name": "agent:hermes"}'
```

### Step 3: Set Conservative Risk Config

```
curl -X PUT https://api.testudo.vip/api/v1/risk-config \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "account_risk_percent": 0.5,
    "max_risk_amount": 25.0,
    "max_position_size": 0.1,
    "max_leverage": 3,
    "daily_max_drawdown_percent": 3.0,
    "max_open_positions": 2,
    "require_stop_loss": true,
    "min_risk_reward_ratio": 2.0
  }'
```

### Step 4: Trade in Shadow Mode (Practice)

```
curl -X POST https://api.testudo.vip/api/v1/signals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "LONG",
    "entry_price": "50000.00",
    "stop_loss": "49000.00",
    "take_profit": [{"price": "52000.00", "quantity": "1.0"}],
    "execution_mode": "shadow",
    "reasoning": "Testing breakout strategy on 4H. RSI oversold bounce.",
    "source": "agent:hermes_v0.1",
    "confidence": 0.65,
    "idempotency_key": "550e8400-e29b-41d4-a716-446655440000"
  }'
```

### Step 5: Analyze Journal Performance

```
# Check how the shadow strategy performed
curl -s https://api.testudo.vip/api/v1/journal/agent/summary \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"timeframe": "90d", "format": "llm"}' | jq -r '.data'

# Get actionable insights
curl -s https://api.testudo.vip/api/v1/journal/agent/insights \
  -H "Authorization: Bearer $TOKEN" | jq

# Compare before/after strategy change
curl -s https://api.testudo.vip/api/v1/journal/agent/compare \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"period_a": {"from": "2026-01-01", "to": "2026-03-01"}, "period_b": {"from": "2026-03-01", "to": "2026-05-01"}}'
```

### Step 6: Graduate to Live

After shadow-mode thresholds are met (e.g., >50 trades, >45% win rate, positive avg-R):

```
# Same signal, but execution_mode: "live"
curl -X POST https://api.testudo.vip/api/v1/signals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "LONG",
    "entry_price": "50000.00",
    "stop_loss": "49000.00",
    "take_profit": [{"price": "52000.00", "quantity": "1.0"}],
    "execution_mode": "live",
    "reasoning": "Confirmed breakout. Shadow performance: 58% WR, 1.9 avg R.",
    "source": "agent:hermes_v0.1",
    "idempotency_key": "660e8400-e29b-41d4-a716-446655440000"
  }'
```

### Step 7: Monitor via WebSocket

```javascript
const ws = new WebSocket('wss://api.testudo.vip:4000');
ws.send(JSON.stringify({ method: 'SUBSCRIBE', params: ['agent.alert.<user_id>', 'agent.execution.<user_id>'], id: 1 }));

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.stream.startsWith('agent.alert')) {
    console.warn(`⚠️ Alert: ${msg.data.message}`);
    // Agent can react: pause trading, reduce size, notify user
  }
  if (msg.stream.startsWith('agent.execution')) {
    console.log(`✅ ${msg.data.status}: ${msg.data.fill_price} (${msg.data.latency_ms}ms)`);
  }
};
```

---

## Architecture Fit

All three vectors slot cleanly into the existing architecture:

```
                          Signal Endpoint
                               │
                               ▼
┌──────────────────────────────────────────────────────────────┐
│                         Router (Actix-web)                    │
│                                                               │
│  routes/signal.rs ──► services/agent_signal.rs                │
│                              │                                │
│                              ▼                                │
│                      DecisionLoop::execute()                  │
│                              │                                │
│                    ┌─────────┼─────────┐                     │
│                    ▼         ▼         ▼                      │
│              RiskService  Shadow   Live                       │
│              (8 checks)  Engine   (CexExchangeApi /           │
│                         (paper)   HyperliquidExchangeApi)     │
│                              │                                │
│                              ▼                                │
│                     pg_notify('agent_events', ...)            │
│                              │                                │
└──────────────────────────────┼────────────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────┐
│                  WebSocket Server (port 4000)                  │
│                                                               │
│  agent.alert.{user_id}  ◄── pg_queue LISTEN                  │
│  agent.execution.{user_id}                                    │
│  agent.order.{user_id}                                        │
└──────────────────────────────────────────────────────────────┘
                               │
                               ▼
                        Coding Agent
                    (Hermes / OpenClaw / pi)

                          Journal Memory
                               │
                               ▼
┌──────────────────────────────────────────────────────────────┐
│  routes/journal.rs                                            │
│                                                               │
│  GET /journal/agent/summary  ──► StatsEngine                  │
│  GET /journal/agent/insights ──► CoachDigest patterns         │
│  GET /journal/agent/compare  ──► TimeSeriesService            │
└──────────────────────────────────────────────────────────────┘
                               │
                               ▼
                        Coding Agent
              (Analyzes performance, adjusts strategy)
```

---

## Summary

| Vector | Why Now? |
|---|---|
| **Signal Endpoint** | The biggest unlock. Agents can't trade programmatically today. Extension scraping is fragile and not agent-friendly. One new route, reuses DecisionLoop entirely. |
| **Agent WebSocket Alerts** | pg_queue LISTEN/NOTIFY already does the heavy lifting. Adding agent channels is configuration, not architecture. Agents need real-time awareness to be effective. |
| **Journal as Agent Memory** | Journal data is rich — 11 chart types, CoachDigest patterns, tag-based filtering. Making it queryable by agents with structured + LLM-ready output gives them the feedback loop they need to improve. |

All three can be built in ~2 weeks. They build on existing, battle-tested infrastructure. No new databases, no new messaging systems, no new auth flows. They extend what Testudo already does well — risk-managed execution, real-time streaming, and comprehensive journaling — into the agent domain.
