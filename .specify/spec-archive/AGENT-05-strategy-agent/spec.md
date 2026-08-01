# Specification: Strategy Agent — Autonomous Signal Generation + Self-Evaluation

**Spec ID:** AGENT-05-strategy-agent
**Date:** 2026-05-21
**Status:** Draft
**Class:** Feature / AI Agent
**Priority:** P0 — required for Agora Agents Hackathon submission; Agentic Sophistication criterion (30%) depends on autonomous decision-making
**Depends on:** AGENT-01 (signal endpoint), AGENT-02 (WebSocket alerts), AGENT-03 (journal memory)
**Series:** AGENT-05 (Strategy Agent) — deferred, will refactor when circling back

---

## Problem Statement

AGENT-01 through AGENT-04 provide the infrastructure layer — signal ingestion, risk validation, exchange execution, WebSocket feedback, journal analysis, and Circle/Arc USDC settlement. But every piece expects an **external caller**. The `POST /api/v1/signals` endpoint has no producer. The WebSocket `agent.alert.*` channels have no subscriber. The journal `/summary?format=llm` endpoint has no consumer.

The Agora Agents Hackathon's primary criterion (Agentic Sophistication, 30% weighting) asks: *"How much does the AI actually decide vs just automate?"* Full autonomy beats meaningful agency beats AI-flavored automation. The judges want to see an agent that:
- Observes market conditions
- Makes autonomous trading decisions
- Routes through risk validation
- Learns from its performance

Testudo's infrastructure can execute, alert, and analyze — but without an agent making decisions, it's automation without agency. This spec builds the decision-maker.

The strategy is deliberately simple: a mean-reversion agent that trades ETH/USDC based on Bollinger Band deviations. Simple enough to build in a day, sophisticated enough to demonstrate autonomous decision-making, risk-aware enough to use Testudo's existing position sizing and drawdown limits, and self-evaluating enough to query its own journal.

---

## User Stories

- **As a hackathon judge**, I want to see an agent autonomously open and close trades without human intervention, demonstrating real agentic behavior.
- **As a hackathon judge**, I want to see the agent's reasoning in its signal payload, so that I can verify it's not just random automation.
- **As a trader**, I want to understand why the agent made each decision, so that I can build trust in autonomous trading.
- **As a strategy developer**, I want the agent's performance visible in the Testudo journal alongside human trades, so that I can compare strategies.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Agent polls price feed every 60 seconds (configurable) and computes Bollinger Bands (20-period SMA, 2σ deviation) | High | Agent service |
| FR-2 | When price crosses below lower band: `POST /api/v1/signals` with `side=LONG` and reasoning describing the deviation | High | Agent service |
| FR-3 | When price crosses above upper band: `POST /api/v1/signals` with `side=SHORT` and reasoning describing the deviation | High | Agent service |
| FR-4 | Signal payload includes `source = "agent:mean-reversion:v1.0"`, `confidence` score (based on deviation magnitude), and `reasoning` describing the statistical basis | High | Agent service |
| FR-5 | Agent subscribes to WebSocket `agent.alert.{user_id}` and pauses trading on drawdown warnings, resumes when alert clears | High | Agent service |
| FR-6 | Agent queries `GET /api/v1/journal/agent/summary?format=llm&source=agent:mean-reversion:v1.0` every 6 hours and logs the markdown summary | Medium | Agent service |
| FR-7 | Agent queries `GET /api/v1/journal/agent/insights` and adjusts behavior: reduces size on sizing_drift flags, pauses on frequency_spike flags | Medium | Agent service |
| FR-8 | Agent respects position limit: max 1 open position per symbol at a time (closes existing before opening opposite direction) | High | Agent service |
| FR-9 | Agent uses Circle/Arc USDC wallet for settlement (via the account linked in AGENT-04) | High | Agent service |
| FR-10 | Agent exposes `GET /health` and `GET /status` endpoints showing current state (last signal, open positions, P&L snapshot, drawdown %) | Low | Agent service |

---

## Technical Implementation

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  strategy-agent (Python, port 3102)                           │
│                                                               │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │ Price Feed   │  │ Bollinger    │  │ Signal Generator   │   │
│  │ (poll 60s)   │→ │ Band Engine  │→ │ POST /api/v1/      │   │
│  │              │  │ (20 SMA, 2σ) │  │ signals            │   │
│  └─────────────┘  └──────────────┘  └─────────┬──────────┘   │
│                                                │              │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────▼──────────┐   │
│  │ Journal      │  │ Alert        │  │ Testudo Exchange   │   │
│  │ Self-Eval    │← │ Handler      │→ │ DecisionLoop       │   │
│  │ (6h cycle)   │  │ (WebSocket)  │  │ → ExchangeApi      │   │
│  └─────────────┘  └──────────────┘  └────────────────────┘   │
│                                                               │
│  GET /health     GET /status     GET /api/v1/journal/agent/*  │
└──────────────────────────────────────────────────────────────┘
```

### Strategy Logic

**Bollinger Band Mean-Reversion** is chosen because:
- It's deterministic and explainable (no black-box ML)
- Generates clear reasoning strings ("Price $3,120 is 2.3σ below 20-period SMA $3,250")
- Natural position sizing: wider deviation → higher confidence
- Well-understood by judges (no need to explain a complex model)
- Mean-reversion works on ranging markets, which is what Arc testnet simulates

```
Pseudo-code per tick:
1. Fetch last 20 closing prices for ETH/USDC from price feed
2. Compute SMA_20 = mean(prices[-20:])
3. Compute σ = stdev(prices[-20:])
4. upper_band = SMA_20 + 2 * σ
5. lower_band = SMA_20 - 2 * σ
6. current_price = prices[-1]
7. deviation = (current_price - SMA_20) / σ

IF deviation < -2.0 AND no open LONG position:
    confidence = min(abs(deviation) / 4.0, 0.95)  // cap at 0.95
    POST /signals { side: "LONG", confidence, reasoning: "..." }
    
IF deviation > 2.0 AND no open SHORT position:
    confidence = min(abs(deviation) / 4.0, 0.95)
    POST /signals { side: "SHORT", confidence, reasoning: "..." }

IF deviation crosses back within ±0.5σ AND open position exists:
    POST /signals { side: "CLOSE", reasoning: "Mean reversion complete" }
```

### Price Feed

For hackathon purposes, use Binance ETH/USDT price as a proxy for ETH/USDC (USDT and USDC are both $1 stablecoins). Fetch via free REST endpoint:

```python
# Binance public API — no auth required
GET https://api.binance.com/api/v3/klines?symbol=ETHUSDT&interval=1m&limit=20
```

Alternatively, use the Arc testnet oracle if available, or mock a sinusoidal price for demo predictability.

### Signal Payload Format

Matches AGENT-01's expected `SignalRequest` schema:

```json
{
  "exchange_account_id": "uuid-of-circle-account",
  "symbol": "ETH_USDC",
  "side": "LONG",
  "execution_mode": "live",
  "source": "agent:mean-reversion:v1.0",
  "confidence": 0.72,
  "reasoning": "ETH/USDC at 3120.50, 2.3σ below 20-period SMA 3250.80. Upper band 3420.20, lower band 3081.40. Statistical mean-reversion probability: 97.7% within 2σ. Initiating LONG with 2% risk allocation."
}
```

### WebSocket Alert Handling

```python
# Subscribe to agent.alert.{user_id} on ws://localhost:4000
# On drawdown warning: pause signal generation, log warning
# On drawdown cleared: resume signal generation
# On max positions reached: skip this tick, log
# On agent wallet expiring: log warning (user must re-auth)
```

### Self-Evaluation Cycle

Every 6 hours, the agent:
1. Calls `GET /api/v1/journal/agent/summary?format=llm&source=agent:mean-reversion:v1.0&timeframe=24h`
2. Logs the markdown summary to `~/.testudo/agent/logs/`
3. Calls `GET /api/v1/journal/agent/insights`
4. If `sizing_drift` detected: reduces position size by 20%
5. If `frequency_spike` detected: increases poll interval to 120s
6. If `low_win_rate_setup` detected on ETH_USDC: pauses and logs warning

### State Management

Minimal state, persisted to a JSON file:

```python
{
  "open_positions": {
    "ETH_USDC": {"side": "LONG", "trade_group_id": "uuid", "entry_price": 3120.50}
  },
  "last_signal_at": "2026-05-21T10:30:00Z",
  "paused_until": null,
  "position_size_pct": 2.0,
  "poll_interval_seconds": 60,
  "drawdown_warning": false
}
```

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Scaffold Python service: FastAPI, health endpoint, config loading, state persistence | Service starts, GET /health returns 200 |
| CP-2 | Bollinger Band engine: price polling, SMA/σ computation, deviation → signal decisions | Agent generates LONG/SHORT/CLOSE decisions, logs reasoning |
| CP-3 | Signal submission: POST /api/v1/signals with correct payload format | Signal received by Testudo, trade appears in journal |
| CP-4 | WebSocket alert subscriber + self-evaluation cycle | Agent pauses on drawdown, logs 6h journal summary |
| CP-5 | Full integration test: strategy agent → Circle settlement → journal → self-eval | End-to-end autonomous agent trading loop on Arc testnet |

### Files

**New directory:** `strategy-agent/`

```
strategy-agent/
├── pyproject.toml              # FastAPI, httpx, websockets, numpy
├── .env.example                # TESTUDO_API_URL, TESTUDO_WS_URL, CIRCLE_ACCOUNT_ID, USER_ID
├── src/
│   ├── main.py                 # FastAPI app, health/status endpoints
│   ├── config.py               # Env var loading, state file path
│   ├── bollinger.py            # Price feed polling, SMA/σ, band computation
│   ├── signal.py               # SignalRequest builder, POST to testudo
│   ├── alerts.py               # WebSocket subscriber, drawdown/pause logic
│   ├── evaluator.py            # Journal query, self-evaluation cycle
│   ├── state.py                # JSON state persistence (open positions, params)
│   └── types.py                # Pydantic models for signal, state, config
└── README.md                   # Quickstart: uv run strategy-agent
```

### Dependencies

**Python (pyproject.toml):**
- `fastapi` + `uvicorn` — HTTP server
- `httpx` — async HTTP client for Testudo API calls
- `websockets` — WebSocket client for agent.alert subscriptions
- `numpy` — SMA/σ computation
- `pydantic` — config and state validation
- (No ML dependencies, no LLM SDK — pure statistical strategy)

---

## Acceptance Criteria

- [ ] Agent boots and reports healthy at `GET http://localhost:3102/health`
- [ ] Agent polls ETH/USDT price and computes Bollinger Bands every 60 seconds
- [ ] Agent generates LONG signal when price crosses below lower band
- [ ] Agent generates SHORT signal when price crosses above upper band
- [ ] Agent generates CLOSE signal when price reverts within ±0.5σ of SMA
- [ ] Signal payload includes `source = "agent:mean-reversion:v1.0"` with reasoning and confidence
- [ ] Agent subscribes to `agent.alert.*` WebSocket and pauses on drawdown warnings
- [ ] Agent resumes trading when drawdown alert clears
- [ ] Agent queries journal summary every 6 hours and logs markdown to file
- [ ] Agent queries insights and reduces size on sizing_drift detection
- [ ] Agent respects max 1 open position per symbol
- [ ] `GET /status` returns current state: open positions, last signal, P&L (if available), drawdown %
- [ ] Agent routes trades to Circle/Arc account via AGENT-04 adapter
- [ ] Full loop works: price deviation → signal → risk validation → Arc USDC transfer → journal record
- [ ] Agent continues running without crashing for 24+ hours (graceful error handling on API failures)
- [ ] `uv run strategy-agent` starts without errors on Python 3.13+

---

## Risks

1. **Price feed dependency** — Binance public API may rate-limit or be unavailable. Mitigation: cache last known price, skip tick on fetch failure, log warning. Fallback: accept a mock price feed for demo mode.
2. **Mean-reversion on trending markets** — Bollinger Bands produce false signals in strong trends (price walks the band). Mitigation: this is the hackathon version. The strategy doesn't need to be profitable — it needs to demonstrate autonomous decision-making. Judges score agency, not P&L.
3. **Agent-auth scope** — Agent uses the same bearer token as the user, granting full account access. Mitigation: acceptable for hackathon (single-user demo). AGENT-03's follow-up mentions scoped API keys.
4. **WebSocket reconnect** — If the WebSocket disconnects, the agent misses alerts. Mitigation: reconnect with exponential backoff, fetch current state via REST on reconnect.
5. **No paper mode for agent** — The agent trades the Circle testnet account directly. There's no paper-trading mode for on-chain settlement. Mitigation: Circle testnet USDC has no real value. "Live" mode on testnet is the paper mode.

---

## Completion Signal

This spec is complete when:
1. Strategy agent runs autonomously, generating signals based on Bollinger Band deviations
2. Agent subscribes to WebSocket alerts and responds to risk warnings
3. Agent self-evaluates via journal queries every 6 hours
4. Full end-to-end loop: price → signal → risk → Circle USDC transfer → journal → self-eval
5. All 15 acceptance criteria met
6. Agent runs stably for 24+ hours in test mode
7. Code committed to master

---

## Hackathon Submission Notes

This agent demonstrates:
- **Agentic Sophistication (30%)**: Fully autonomous decision-making. The agent observes, reasons, acts, and self-evaluates without human intervention. Reasoning strings are transparent and verifiable.
- **Circle tool usage (20%)**: All trades settle on-chain via Circle developer-controlled wallets on Arc testnet (AGENT-04).
- **Innovation (20%)**: The agent uses Testudo's production risk engine as a safety layer — no other hackathon project has a Kelly-position-sizing, drawdown-limited agent.
- **Traction (30%)**: During the event window, run the agent live. Every USDC transfer on Arc testnet is a transaction. Report volume, trade count, and uptime in the submission form.

**Demo walkthrough for Loom video:**
1. Show agent starting up, connecting to Testudo, computing bands
2. Price deviates → agent POSTs signal → trade appears in journal
3. Show USDC transfer on Arcscan block explorer
4. Show journal summary with agent attribution (`source: agent:mean-reversion:v1.0`)
5. Show self-evaluation log with markdown summary
6. Show agent pausing on drawdown alert, resuming when cleared
