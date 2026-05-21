
- [ ] Break-even trigger, trailing stop, partial tp (perhaps remove that) not important for now
- [ ] change pw in env on n0x

## Agent Integration Series (AGENT-01 → AGENT-03)

Blueprint: `agent-integration-blueprint.md` (project root)

Workflow:
```
/skill:vox plan AGENT-01-signal-endpoint    # gap analysis → IMPLEMENTATION_PLAN.md
/skill:vox build AGENT-01-signal-endpoint   # CP-1 (repeat for each checkpoint)
# ... repeat build until all CPs complete ...
/skill:vox plan AGENT-02-websocket-alerts
/skill:vox build AGENT-02-websocket-alerts
# ... etc.
```

- [ ] **AGENT-01-signal-endpoint** — `POST /api/v1/signals`, programmatic trade execution via DecisionLoop. Shadow + live modes, agent attribution in journal.
- [ ] **AGENT-02-websocket-alerts** — `agent.alert.*`, `agent.execution.*` WebSocket channels. Risk breaches, execution reports, wallet expiry via pg_notify.
- [ ] **AGENT-03-journal-memory** — Bidirectional journal API. **Read**: `GET /journal/agent/summary` (JSON + LLM markdown), `/insights` (coach patterns), `POST /compare` (period-over-period). **Write**: `POST /journal/agent/note` (thesis/postmortem/observation), `/tag` (strategy labels), `/strategy` (named strategy persistence). Agents build persistent memory across sessions.

## Hackathon Delivery Series (AGENT-04 → AGENT-05)

Agora Agents Hackathon (deadline: May 25). Arc Network = Circle's EVM L1, USDC native gas, Chain ID `5042002`, testnet only (mainnet beta TBA 2026).

- [ ] **AGENT-04-circle-adapter** — `AuthMode::CircleAgent`, `CircleExchangeApi` via Bun sidecar (port 3101). Circle dev wallets → USDC settlement on Arc testnet. Same delegate-key pattern as Hyperliquid AW series.
- [ ] **AGENT-05-agent-sdk** — Agent integration kit (Python CLI, port 3102). `agent context` → `agent signal` → `agent listen` → `agent review` → `agent note/tag/strategy`. Ships with AGENTS.md template for any external AI agent (Claude, Hermes, OpenClaw). Testudo is the execution platform; the AI agent brings the intelligence.


