# Specification: Agent SDK — External AI Agent Integration Kit

**Spec ID:** AGENT-05-agent-sdk
**Date:** 2026-05-21
**Status:** Draft
**Class:** Platform / SDK
**Priority:** P0 — required for Agora Agents Hackathon submission; Agentic Sophistication criterion (30%) depends on the external AI agent calling Testudo
**Depends on:** AGENT-01 (signal endpoint), AGENT-02 (WebSocket alerts), AGENT-03 (journal memory), AGENT-04 (Circle adapter)
**Series:** AGENT-04 through AGENT-05 (Hackathon Delivery)

---

## Problem Statement

AGENT-01 through AGENT-04 provide the infrastructure: signal ingestion, risk validation, exchange execution, WebSocket feedback, journal analysis, and Circle/Arc USDC settlement. But every piece expects an **external caller** — an AI agent that decides what to trade.

The strategy itself is **not Testudo's concern**. The whole point of AGENT-01's `POST /api/v1/signals` endpoint is that **any** external AI agent (Claude, OpenClaw, Hermes, a custom Python script, a TradingView strategy) can call it. Testudo validates, sizes, routes, and settles — the agent decides.

What's missing is the **integration layer** that makes it trivial for an external AI agent to:
1. Discover what Testudo can do (capabilities manifest)
2. Receive market context to reason about (price, positions, risk state)
3. Submit trade decisions through the signal endpoint
4. Get real-time feedback via WebSocket alerts
5. Learn from performance via the journal's LLM-optimized summaries

This spec builds the **Agent SDK** — a thin integration kit (CLI tool + context template + MCP server) that bridges any external AI agent to Testudo's execution pipeline. The agent brings the intelligence; Testudo brings the execution, risk management, and settlement.

---

## User Stories

- **As Claude / Hermes / OpenClaw** (any AI coding agent), I want to query market context and submit trades through Testudo's API, so that I can act as an autonomous trading agent.
- **As a hackathon judge**, I want to see the AI agent's actual reasoning in the signal payload, proving it made the decision — not just a hardcoded strategy script.
- **As a developer**, I want a single CLI command that dumps all the context my agent needs into its session, so that I don't have to manually construct API calls.
- **As a user**, I want to give my AI agent permission to trade within strict risk limits, so that I can benefit from autonomous execution without unlimited exposure.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `agent context` CLI command dumps current market state + account state as a structured prompt ready for an AI agent's context window | High | CLI |
| FR-2 | Context includes: open positions, account balance (USDC), current drawdown %, daily P&L, risk limits, available symbols | High | CLI |
| FR-3 | `agent signal` CLI command submits a trade signal to `POST /api/v1/signals` with reasoning, confidence, and source attribution | High | CLI |
| FR-4 | `agent listen` CLI command connects to WebSocket `agent.alert.*` and `agent.execution.*` channels, printing events in real-time | Medium | CLI |
| FR-5 | `agent review` CLI command fetches `GET /journal/agent/summary?format=llm` and prints it for the agent to ingest | Medium | CLI |
| FR-6 | Agent SDK ships as an `AGENTS.md` template that any AI coding agent loads — describes the full Testudo API surface for agents | High | Docs |
| FR-7 | `AGENTS.md` template includes: signal schema, available symbols, risk limits, WebSocket channels, journal query examples, and a recommended decision loop | High | Docs |
| FR-8 | Agent SDK is a single Bash/Python script installable via `uv tool install` or `curl | bash`, with zero new infrastructure dependencies | High | CLI |
| FR-9 | The CLI validates all signal payloads against the AGENT-01 schema before submission (client-side validation) | Medium | CLI |
| FR-10 | `agent context --full` includes recent journal summary in LLM markdown format for the agent's reasoning cycle | Low | CLI |

---

## Technical Implementation

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  External AI Agent (Claude, OpenClaw, Hermes, etc.)           │
│                                                               │
│  Loads AGENTS.md → discovers Testudo API surface               │
│  Runs: agent context → reads market state + positions         │
│  Reasons: "ETH is oversold, risk budget allows 2% allocation" │
│  Runs: agent signal LONG ETH_USDC 2%                          │
│  Listens: agent listen → receives fill confirmation           │
│  Reviews: agent review → ingests journal summary              │
│  Iterates: context → reason → signal → listen → review        │
└──────────────────────┬───────────────────────────────────────┘
                       │ HTTP + WebSocket
                       ▼
┌──────────────────────────────────────────────────────────────┐
│  Testudo Exchange (Rust, :3000)                               │
│                                                               │
│  POST /api/v1/signals     ← agent signal                      │
│  GET  /api/v1/exchanges/accounts/{id}/balance                 │
│  GET  /api/v1/journal/agent/summary?format=llm                │
│  WS   agent.alert.*       → drawdown warnings, wallet expiry  │
│  WS   agent.execution.*   → fill confirmations                │
└──────────────────────────────────────────────────────────────┘
```

### Agent Decision Loop

The recommended cycle for any external AI agent integrating with Testudo:

```
1. CONTEXT   — agent context            → What's happening right now?
2. REASON    — AI analyzes context       → Should I trade? What? How much?
3. SIGNAL    — agent signal LONG ETH 2%  → Submit decision to Testudo
4. LISTEN    — agent listen              → Wait for fill confirmation + alerts
5. REVIEW    — agent review              → Periodically check performance
6. ADAPT     — Adjust sizing, pause, etc → Respond to insights + drawdown
7. GOTO 1
```

### Context Output Format

The `agent context` command produces structured output for an AI agent's context window:

```markdown
## Testudo Agent Context — 2026-05-21 14:30 UTC

### Account
- Exchange: Circle/Arc Testnet (5042002)
- Balance: 987.50 USDC
- Daily P&L: +12.30 USDC
- Drawdown: 2.1% / 15% limit

### Open Positions
| Symbol | Side | Size | Entry | Current | P&L |
|--------|------|------|-------|---------|-----|
| ETH_USDC | LONG | 19.75 USDC | 3098.50 | 3120.30 | +0.14 USDC |

### Risk Limits
- Max position size: 2% of account (19.75 USDC)
- Max positions: 3
- Max daily drawdown: 15%
- Current drawdown: 2.1% (OK)

### Available Symbols
ETH_USDC, BTC_USDC, SOL_USDC

### Recent Alerts (last hour)
- None

### API Reference
- Submit signal: agent signal <SIDE> <SYMBOL> <SIZE_PCT>
- Example: agent signal LONG ETH_USDC 2
- Get help: agent --help
```

### CLI Commands

```bash
# Core workflow
agent context              # Dump current state for agent reasoning
agent context --full       # Include journal summary (LLM markdown)
agent signal LONG ETH_USDC 2 --reasoning "Oversold bounce expected" --confidence 0.7
agent listen               # Subscribe to alerts + execution reports
agent review               # Fetch journal summary (LLM markdown)
agent review --timeframe 7d

# Journal write — build persistent agent memory
agent note <TRADE_ID> --type thesis --content "Entered on 2h support bounce..."
agent note <TRADE_ID> --type postmortem --content "Exited at TP, R:R held at 2.1"
agent note <TRADE_ID> --type observation --content "Volume confirmation aligned with entry"
agent tag <TRADE_ID> "momentum"
agent tag <TRADE_ID> "high-confidence"
agent tag <TRADE_ID> "news-event"
agent strategy create "ETH Mean Reversion v2" --description "..." --rules '{"entry":"...","exit":"..."}'
agent strategy list
agent strategy update <ID> --rules '{"entry":"..."}'

# Account management
agent balance              # Show USDC balance
agent positions            # Show open positions
agent limits               # Show risk limits

# Configuration
agent login                # Authenticate with Testudo (stores bearer token)
agent status               # Show connection status
agent --help               # Full command reference
```

### AGENTS.md Template

The SDK ships an `AGENTS.md` file that any AI coding agent loads at session start. This is the canonical way to give an AI agent awareness of Testudo:

```markdown
# Testudo Agent Integration

You have access to Testudo, a production trading platform with risk management,
multi-venue execution, and on-chain USDC settlement via Circle/Arc.

## Your Capabilities

You can trade autonomously within strict risk limits. Every trade you submit
goes through Testudo's DecisionLoop: risk validation → position sizing →
exchange routing → settlement. You cannot bypass risk limits.

## Trading via Testudo

### 1. Get context
Run: `agent context`
This gives you current positions, balance, risk limits, and drawdown status.

### 2. Submit a trade
Run: `agent signal <SIDE> <SYMBOL> <SIZE_PCT> --reasoning "<why>" --confidence <0-1>`

Examples:
  agent signal LONG ETH_USDC 2 --reasoning "ETH broke above 2h resistance, volume confirming" --confidence 0.75
  agent signal SHORT BTC_USDC 1.5 --reasoning "Bearish divergence on 4h RSI, BTC at resistance" --confidence 0.6

SIZE_PCT is percentage of account balance to risk (NOT position size).
Testudo sizes the position conservatively using Kelly criterion and your risk config.

### 3. Monitor
Run: `agent listen`
Real-time WebSocket feed: fill confirmations, drawdown warnings, wallet expiry.

### 4. Review performance
Run: `agent review`
Fetches your trading journal as LLM-optimized markdown. Study your win rate,
which setups work, and what the coach pipeline detected.

### 5. Adapt
Based on insights from `agent review`:
- If sizing_drift detected: reduce SIZE_PCT by 20%
- If frequency_spike detected: slow down, fewer trades
- If drawdown warning: pause until alert clears

### 6. Remember (write to journal)
After each trade, persist your reasoning:
  agent note <trade_group_id> --type thesis --content "Why I entered this trade..."
After each trade closes, record what you learned:
  agent note <trade_group_id> --type postmortem --content "What went right/wrong..."
Tag trades by strategy to filter later:
  agent tag <trade_group_id> "mean-reversion"
  agent tag <trade_group_id> "high-confidence"

### 7. Define your strategies
Persist your trading strategies so you can compare performance:
  agent strategy create "ETH Mean Reversion" --description "..." --rules '{"entry":"..."}'
Strategies persist across sessions. You can evolve them based on journal data.

### 8. In future sessions
Start with:
  agent context --full       # See what happened since you last ran
  agent review --timeframe 30d  # Review your long-term performance
  agent strategy list        # Recall your active strategies
Your notes, tags, and strategies are all still there. You don't start from zero.

## Risk Rules (Immutable)
- Max 1 trade per symbol at a time
- Max 3 concurrent positions total
- Max 15% daily drawdown (enforced by DecisionLoop, not advisory)
- Position sizing: conservative Kelly, never exceeds your risk config

## Your Identity
Your trades are tagged with source="agent:claude" (or your agent name).
You can review them with: agent review --source agent:claude
```

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Scaffold CLI: `agent --help`, `agent context`, `agent status` | CLI installs, context command prints account state |
| CP-2 | `agent signal` — construct and POST to `/api/v1/signals` | Signal accepted, trade appears in journal |
| CP-3 | `agent listen` — WebSocket subscriber, `agent review` — journal fetch | Real-time alerts received, markdown summary printed |
| CP-4 | AGENTS.md template + end-to-end AI agent integration test | Claude/OpenClaw loads AGENTS.md, runs context → signal → review cycle |
| CP-5 | Polish: error handling, config persistence, validation, demo script | Full hackathon demo: AI agent trades autonomously on Arc testnet |

### Files

```
agent-sdk/
├── pyproject.toml              # click, httpx, websockets, rich
├── AGENTS.md                   # Template for AI agent context
├── README.md                   # Quickstart guide
├── src/
│   ├── cli.py                  # Click CLI: context, signal, listen, review, balance, positions
│   ├── api.py                  # HTTP client for Testudo API (signals, journal, accounts)
│   ├── ws_client.py            # WebSocket client for agent.alert + agent.execution
│   ├── context_builder.py      # Builds the markdown context from API responses
│   ├── validators.py           # Signal payload validation against schema
│   └── config.py               # Config file (~/.testudo/agent/config.json): API URL, bearer token
└── demo/
    └── claude-prompt.md         # Example prompt to give Claude to act as a trading agent
```

### Dependencies

**Python (pyproject.toml):**
- `click` — CLI framework
- `httpx` — async HTTP client for Testudo API
- `websockets` — WebSocket client
- `rich` — formatted terminal output
- `pydantic` — config and validation
- (Zero heavy deps, installs in <2s)

---

## Acceptance Criteria

- [ ] `agent context` prints account balance, open positions, risk limits, drawdown, available symbols
- [ ] `agent context --full` includes journal summary in LLM markdown format
- [ ] `agent signal LONG ETH_USDC 2 --reasoning "test" --confidence 0.5` submits valid signal to Testudo
- [ ] `agent signal` validates SIZE_PCT (1-100), SIDE (LONG/SHORT), SYMBOL before submission
- [ ] `agent signal` returns the trade_group_id and execution status from Testudo
- [ ] `agent listen` connects to WebSocket and prints alert + execution events in real-time
- [ ] `agent listen` survives WebSocket disconnect and reconnects with backoff
- [ ] `agent review` fetches and prints LLM markdown summary from journal
- [ ] AGENTS.md template is complete and parseable by any AI coding agent
- [ ] `agent login` stores bearer token persistently in `~/.testudo/agent/config.json`
- [ ] All CLI commands fail gracefully when not authenticated (clear error message, not a crash)
- [ ] `agent --help` shows full command tree with examples
- [ ] SDK installs via `uv tool install .` or `pip install -e .`
- [ ] End-to-end: external AI agent runs `agent context` → reasons → `agent signal` → `agent listen` → `agent review`
- [ ] `agent signal` with Circle account routes to Arc testnet USDC settlement via AGENT-04

---

## Risks

1. **Agent auth scope** — The agent uses the user's full bearer token, granting full account access. Mitigation: acceptable for hackathon. Future: scoped API keys with per-agent limits (AGENT-03 follow-up).
2. **CLI vs direct API calls** — The `agent` CLI is a convenience wrapper. AI agents can also call Testudo's REST API directly. Mitigation: the AGENTS.md template documents both approaches.
3. **Context window budget** — `agent context --full` could be large (journal summary + positions + alerts). Mitigation: `--full` is opt-in. Default `agent context` is compact (<500 words).
4. **Agent identity collision** — Multiple agents trading the same account would have overlapping `source` tags. Mitigation: `source` field is free-form. Document convention: `agent:<name>:<version>`.

---

## Completion Signal

This spec is complete when:
1. Agent SDK CLI is installable and all commands work
2. AGENTS.md template is production-quality
3. External AI agent can execute the full decision loop via the SDK
4. All 15 acceptance criteria met
5. Demo script verifies end-to-end: context → signal → listen → review
6. Code committed to master

---

## Hackathon Submission Notes

The Agent SDK is the final piece of the hackathon architecture. Combined with AGENT-01 through AGENT-04:

**Demo flow for Loom video:**
1. Launch Testudo exchange + Circle sidecar
2. Give Claude the AGENTS.md + `agent context`
3. Claude reasons: "ETH at support, risk budget allows 2%, I'll go LONG"
4. Claude runs: `agent signal LONG ETH_USDC 2 --reasoning "..." --confidence 0.72`
5. Signal hits Testudo → DecisionLoop validates → CircleExchangeApi executes → USDC on Arc
6. `agent listen` shows real-time fill confirmation
7. Arcscan block explorer shows the USDC transfer
8. `agent review` shows the trade in the journal with agent attribution
9. Claude adapts: "Win rate on ETH breakouts is 60%, increasing confidence threshold"

**This demonstrates:**
- **Agentic Sophistication (30%)**: Claude/Hermes/OpenClaw makes the autonomous decision. Testudo is the execution platform. The reasoning is in the signal payload — transparent, verifiable, not hardcoded.
- **Circle tool usage (20%)**: USDC settlement on Arc testnet via Circle developer-controlled wallets.
- **Innovation (20%)**: Production risk engine as an AI agent's safety layer. No other hackathon project has Kelly-position-sizing, drawdown-limited agent execution.
- **Traction (30%)**: Real AI-agent-initiated transactions on Arc testnet during the event window.
