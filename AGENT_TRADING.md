# Agent Trading Guide

> **For AI agents (Hermes, OpenClaw, pi, Claude Code, etc.) connecting to Testudo.**
>
> Read this once. It tells you everything you need to trade autonomously.
> Start in shadow mode. Graduate to live when your edge is confirmed.

---

## Quick Reference

| Action | Method | Endpoint |
|--------|--------|----------|
| Check performance | `GET` | `/journal/agent/summary?format=llm` |
| Get coach warnings | `GET` | `/journal/agent/insights` |
| Compare periods | `POST` | `/journal/agent/compare` |
| Place a trade | `POST` | `/api/v1/signals` |
| Watch fills/alerts | WS | `agent.execution.{user_id}` |
| Watch risk breaches | WS | `agent.alert.{user_id}` |

---

## 1. Authentication

Testudo uses SIWE (Sign-In With Ethereum). You authenticate once per session:

```bash
# Step 1: Get a nonce
curl -X GET https://testudo.example.com/api/v1/auth/nonce

# Step 2: Sign the SIWE message with your Ethereum wallet
# (Your agent runtime handles this — OpenClaw/pi have SIWE built-in)

# Step 3: Exchange signature for a bearer token
curl -X POST https://testudo.example.com/api/v1/auth/verify-siwe \
  -H "Content-Type: application/json" \
  -d '{"message":"...","signature":"..."}'

# Step 4: Store the token. All subsequent requests use:
#   Authorization: Bearer <token>
```

**Token expires after 1 hour.** Refresh with `POST /api/v1/auth/refresh` before it expires. If using OpenClaw or pi, the runtime handles token lifecycle — you just call the endpoints.

---

## 2. Pre-Trade Research (Read Your Memory)

Before every trade decision, read your journal. This is your performance history, formatted for direct context-window injection.

### Performance Summary (LLM format)

```
GET /api/v1/journal/agent/summary?format=llm&timeframe=90d
Authorization: Bearer <token>
```

Returns markdown you can inject directly into your reasoning context:

```markdown
## Journal Summary: BTC + ETH (Last 90 Days)

### Overall Performance
- Total trades: 112
- Win rate: 54.5%
- Avg R-multiple: 1.72
- Total P&L: +$8,420.50
- Max drawdown: -$1,890.00
- Profit factor: 1.83

### By Setup Tag
| Setup | Trades | Win Rate | Avg R | P&L |
|---|---|---|---|---|
| breakout | 28 | 60.7% | 2.1 | +$3,240 |
| support_bounce | 34 | 55.9% | 1.8 | +$2,850 |
| trend_follow | 22 | 40.9% | 0.9 | -$920 |
| reversal | 28 | 53.6% | 1.5 | +$3,250 |

### Top Performers
- [T-a3f2b1c4] BTC_USDT long — breakout, 4.2R, opened 2026-03-15
- [T-b7c1d2e3] ETH_USDT short — support break, 3.1R, opened 2026-04-02

### Actionable Insights
- **Strongest setup**: breakout shows 60.7% win rate with 2.10 avg R over 28 trades. Consider increasing allocation.
- **Underperforming setup**: trend_follow has 40.9% win rate over 22 trades. Review entry criteria or reduce position size.
```

**Filter by anything:**

```
?symbol=ETH_USDT                     # one market only
?setup_tag=breakout                  # one strategy
?source=agent:hermes_v1.2            # your own trades
?timeframe=30d                       # last month (also: 7d, 90d, all)
?side=LONG                           # longs only
?format=json                         # structured JSON instead of markdown
```

All filters combine: `?symbol=BTC_USDT&setup_tag=breakout&timeframe=90d&format=llm`

### Coach Warnings

```
GET /api/v1/journal/agent/insights
Authorization: Bearer <token>
```

Returns patterns the coach detected in your last weekly analysis:

```json
{
  "insights": [
    {
      "pattern": "sizing_drift",
      "severity": "concerning",
      "headline": "Position sizes are 2.1× your 30-day average",
      "detail": "Your recent trades show position sizes significantly above your 1000 USD baseline. This increases risk of ruin and drawdown depth.",
      "recommendation": "Reduce position size to baseline levels or lower until confidence in edge is restored.",
      "evidence_count": 5
    },
    {
      "pattern": "session_anomaly",
      "severity": "notable",
      "headline": "Trading outside your typical session hours",
      "detail": "Your best performance historically falls in UTC hours [14, 15, 16]. Recent trades deviate from this pattern.",
      "recommendation": "Restrict trading to your historically optimal hours where possible."
    }
  ],
  "total": 2
}
```

**Acknowledge every warning.** If a coach flag is active, factor it into your sizing or skip the trade.

### Period Comparison

```
POST /api/v1/journal/agent/compare
Authorization: Bearer <token>
Content-Type: application/json

{
  "period_a": {"from": "2026-01-01", "to": "2026-03-31"},
  "period_b": {"from": "2026-04-01", "to": "2026-06-30"},
  "filters": {"symbol": "ETH_USDT", "setup_tag": "breakout"}
}
```

Returns side-by-side deltas:

```json
{
  "deltas": [
    {"metric": "win_rate", "value_a": 52.0, "value_b": 61.5, "delta_pct": 18.3, "direction": "improved"},
    {"metric": "max_drawdown", "value_a": -1200.00, "value_b": -800.00, "delta_pct": 33.3, "direction": "improved"},
    {"metric": "total_pnl", "value_a": 2500.00, "value_b": 4200.00, "delta_pct": 68.0, "direction": "improved"}
  ]
}
```

Use this to confirm strategy changes are working before scaling up.

---

## 3. Execute a Trade

```
POST /api/v1/signals
Authorization: Bearer <token>
Content-Type: application/json
Idempotency-Key: <uuid>
```

### Shadow mode (paper trading — START HERE)

```json
{
  "symbol": "ETH_USDT",
  "side": "long",
  "entry_price": 3100.00,
  "stop_loss": 3050.00,
  "leverage": 1,
  "execution_mode": "shadow",
  "reasoning": "ETH breakout above 3-day resistance at 3080. Volume increasing, BTC.D dropping. Targeting 3200 with 1.6R.",
  "confidence": 0.72,
  "source": "agent:hermes_v1.2",
  "setup_tag": "breakout",
  "management": {
    "trailing_stop": {"activation": 3150, "distance": 30}
  }
}
```

### Live mode (real money — AFTER shadow-mode edge is confirmed)

```json
{
  "symbol": "ETH_USDT",
  "side": "long",
  "entry_price": 3100.00,
  "stop_loss": 3050.00,
  "leverage": 1,
  "execution_mode": "live",
  "reasoning": "ETH breakout above 3-day resistance at 3080. Volume increasing, BTC.D dropping. Targeting 3200 with 1.6R.",
  "confidence": 0.72,
  "source": "agent:hermes_v1.2",
  "setup_tag": "breakout",
  "management": {
    "trailing_stop": {"activation": 3150, "distance": 30}
  }
}
```

### Response (200 OK)

```json
{
  "status": "approved",
  "trade_group_id": "a3f2b1c4-1111-2222-3333-444455556666",
  "position_size": 0.15,
  "sizing_method": "fixed_risk_pct",
  "risk_amount": 75.00,
  "warnings": []
}
```

### Idempotency

Always include an `Idempotency-Key` header (UUID). If the same key is sent again, Testudo returns the cached result instead of placing a duplicate order. This means you can safely retry on network errors.

```bash
curl -X POST https://testudo.example.com/api/v1/signals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $(uuidgen)" \
  -d @signal.json
```

### Rejection reasons

| HTTP | Meaning |
|------|---------|
| 400 | Missing/invalid fields (symbol, side, entry_price, stop_loss required) |
| 401 | Invalid or expired token |
| 409 | Duplicate idempotency key |
| 422 | Risk engine rejected the trade (drawdown exceeded, max positions, sizing violation, etc.) |
| 429 | Rate limit exceeded (30 signals/minute per user) |
| 503 | Exchange unavailable (CEX sidecar down, HL RPC unreachable) |

On 422, read the `code` and `reason` fields in the response. Do NOT retry with the same parameters — the risk engine has a legitimate concern.

---

## 4. Monitor Your Trades

### WebSocket channels

Connect to the WebSocket at `wss://testudo.example.com/ws` and subscribe:

```json
{"type": "subscribe", "channel": "agent.execution.<user_id>"}
{"type": "subscribe", "channel": "agent.alert.<user_id>"}
```

### Execution reports

Sent when your order is filled, SL/TP placed, or management actions fire:

```json
{
  "stream": "agent.execution.a3f2b1c4-...",
  "data": {
    "trade_group_id": "a3f2b1c4-1111-2222-3333-444455556666",
    "order_id": "fill-abc123",
    "status": "filled",
    "fill_price": 3101.50,
    "exchange": "hyperliquid",
    "latency_ms": 145
  }
}
```

### Risk alerts

Sent when drawdown approaches limits or patterns are detected mid-session:

```json
{
  "stream": "agent.alert.a3f2b1c4-...",
  "data": {
    "type": "approaching_drawdown_limit",
    "severity": "notable",
    "message": "Drawdown at 82% of daily limit (limit: $1,000, current: -$820)"
  }
}
```

**On receiving a `concerning` severity alert: stop trading immediately.** Pause until the next session.

---

## 5. The Autonomous Trading Loop

Here is the canonical loop. Follow this exactly.

```python
# Pseudocode — adapt to your agent runtime

SESSION_INTERVAL = 60  # seconds between decision cycles
SHADOW_ONLY = True     # set to False only after 1+ week of profitable paper trading

while True:
    # 1. Read memory
    summary = GET("/journal/agent/summary?format=llm&timeframe=90d")
    insights = GET("/journal/agent/insights")

    # 2. Check for coach warnings
    concerning_warnings = [i for i in insights if i.severity == "concerning"]
    if concerning_warnings:
        log("Skipping session: {} active concerning warnings".format(len(concerning_warnings)))
        sleep(SESSION_INTERVAL)
        continue

    # 3. Build context window
    context = f"""
    Your trading journal:
    {summary}

    Active coach warnings:
    {json.dumps(insights, indent=2)}

    You are trading in {"shadow (paper)" if SHADOW_ONLY else "live"} mode.
    Decide: trade or wait. If trading, specify symbol, side, entry, stop_loss,
    setup_tag, reasoning, and confidence.
    """

    # 4. Send to LLM for decision
    decision = llm.decide(context)

    # 5. Execute or wait
    if decision.trade:
        signal = {
            "symbol": decision.symbol,
            "side": decision.side,
            "entry_price": decision.entry_price,
            "stop_loss": decision.stop_loss,
            "execution_mode": "shadow" if SHADOW_ONLY else "live",
            "reasoning": decision.reasoning,
            "confidence": decision.confidence,
            "source": "agent:your_agent_id",
            "setup_tag": decision.setup_tag
        }
        result = POST("/api/v1/signals", json=signal,
                       headers={"Idempotency-Key": uuid4()})

        if result.status == "approved":
            log(f"Trade opened: {result.trade_group_id}")
            # Subscribe to execution updates
            ws_subscribe(f"agent.execution.{USER_ID}")
        else:
            log(f"Trade rejected: {result}")

    else:
        log("Decision: wait — no edge detected")

    sleep(SESSION_INTERVAL)
```

---

## 6. Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `POST /api/v1/signals` | 30 | 60 seconds per user |
| All other endpoints | Standard JWT middleware | Varies by deployment |

Rate limits exist to prevent runaway loops. A signal every 2 seconds is more than enough — crypto trades resolve in seconds to minutes, not milliseconds.

---

## 7. Rules of Engagement

1. **Start in shadow mode.** Every agent begins with `"execution_mode": "shadow"`. Do not switch to `"live"` until you have at least one week of profitable paper trading.

2. **Use setup tags.** Every trade gets a `setup_tag` (`breakout`, `support_bounce`, `trend_follow`, etc.). Tags are how you and the coach learn which strategies work. Consistency matters.

3. **Write real reasoning.** The `reasoning` field is stored in the journal. It's how you'll audit your own decisions later. Be specific: what signal, what timeframe, what confirmation.

4. **Include confidence scores.** `confidence` is a 0.0–1.0 decimal. Be honest. Low confidence trades should have smaller position sizes. The calibration engine uses this.

5. **Respect the coach.** If the coach flags sizing drift, reduce size. If it flags session anomalies, log off. The coach sees patterns you can't see mid-session.

6. **Check idempotency.** Always send an `Idempotency-Key` header. If you get a network error, retry with the same key — you won't double-execute.

7. **Pause on concerning alerts.** If you receive a WebSocket alert with `severity: "concerning"`, stop trading for the current session. Don't try to trade your way out of a drawdown.

8. **Review before scaling.** Every week, call `POST /journal/agent/compare` comparing this week to last. If metrics are declining, don't scale up.

---

## 8. Example: Complete Session

```bash
TOKEN="eyJhbGciOiJIUzI1NiIs..."

# Step 1: Read your performance
SUMMARY=$(curl -s -H "Authorization: Bearer $TOKEN" \
  "https://testudo.example.com/api/v1/journal/agent/summary?format=llm&timeframe=30d")
echo "$SUMMARY"

# Step 2: Check coach warnings
WARNINGS=$(curl -s -H "Authorization: Bearer $TOKEN" \
  "https://testudo.example.com/api/v1/journal/agent/insights")
echo "$WARNINGS" | jq '.insights[] | {pattern, severity, headline}'

# Step 3: If no concerning warnings, place a paper trade
curl -s -X POST https://testudo.example.com/api/v1/signals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $(uuidgen)" \
  -d '{
    "symbol": "ETH_USDT",
    "side": "long",
    "entry_price": 3100,
    "stop_loss": 3050,
    "execution_mode": "shadow",
    "reasoning": "ETH breakout above 3-day resistance. Volume confirming.",
    "confidence": 0.72,
    "source": "agent:your_agent_id",
    "setup_tag": "breakout"
  }' | jq .

# Step 4: After the session, compare this week to last
curl -s -X POST https://testudo.example.com/api/v1/journal/agent/compare \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "period_a": {"from": "2026-05-14", "to": "2026-05-21"},
    "period_b": {"from": "2026-05-07", "to": "2026-05-14"}
  }' | jq '.deltas[] | {metric, direction, delta_pct}'
```

---

## 9. Supported Exchanges

| Exchange | Mode | Execution |
|----------|------|-----------|
| **Shadow** (paper) | `shadow` | Internal engine — always available, no credentials needed |
| **Hyperliquid** | `live` | Native Rust SDK. Requires agent wallet approval (`POST /api/v1/exchanges/agent-wallet/approve`) |
| **Binance / Bybit / OKX** | `live` | Via CEX sidecar. Requires API key/secret in exchange accounts |

Start shadow. After proving profitability, add a live exchange account and switch `execution_mode` to `"live"`.
