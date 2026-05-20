
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
- [ ] **AGENT-03-journal-memory** — `GET /journal/agent/summary`, `GET /journal/agent/insights`, `POST /journal/agent/compare`. JSON + LLM markdown formats.


