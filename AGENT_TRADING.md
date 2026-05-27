# Agent Trading Guide

> **For AI agents (Hermes, OpenClaw, pi, etc.) connecting to Testudo.**
>
> Read this once. It tells you everything you need to trade autonomously.
> Start in shadow mode. Graduate to live when your edge is confirmed.

---

## Quick Reference

| Action | Method | Endpoint |
|--------|--------|----------|
| Get user info | `GET` | `/api/v1/auth/me` |
| Check performance | `GET` | `/api/v1/journal/agent/summary?format=llm` |
| Get coach warnings | `GET` | `/api/v1/journal/agent/insights` |
| Compare periods | `POST` | `/api/v1/journal/agent/compare` |
| Write entry (pre-trade/post-trade/note) | `POST` | `/api/v1/journal/entries` |
| List entries | `GET` | `/api/v1/journal/entries` |
| Create strategy tag | `POST` | `/api/v1/journal/tags` |
| List tags | `GET` | `/api/v1/journal/tags` |
| Tag a trade | `POST` | `/api/v1/journal/trades/{id}/tags` |
| Update trade notes | `PATCH` | `/api/v1/journal/trades/{id}/notes` |
| Place a trade | `POST` | `/api/v1/signals` |
| Watch fills/alerts | WS | `agent.execution.{user_id}` |
| Watch risk breaches | WS | `agent.alert.{user_id}` |

---

## 1. Authentication

Testudo uses SIWE (Sign-In With Ethereum) or SIWS (Sign-In With Solana). You authenticate once per session:

```bash
# Step 1: Get a nonce
curl -X GET https://testudo.vip/api/v1/auth/nonce

# Step 2: Sign the SIWE message with your Ethereum wallet (or SIWS for Solana)
# (Your agent runtime handles this — OpenClaw/pi have SIWE built-in)

# Step 3: Exchange signature for a bearer token
#   Ethereum: POST /api/v1/auth/verify-siwe
#   Solana:   POST /api/v1/auth/verify-siws
curl -X POST https://testudo.vip/api/v1/auth/verify-siwe \
  -H "Content-Type: application/json" \
  -d '{"message":"...","signature":"..."}'

# Step 4: Store the token. All subsequent requests use:
#   Authorization: Bearer <token>

# Step 5: Verify your identity
curl -s -H "Authorization: Bearer $TOKEN" https://testudo.vip/api/v1/auth/me
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

### SignalInput Fields

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `symbol` | string | ✅ | e.g. `"ETH_USDT"` |
| `side` | string | ✅ | `"LONG"` or `"SHORT"` (**uppercase**) |
| `entry_price` | decimal | ✅ | Limit price for the order |
| `take_profit` | array | ✅ | TP targets; can be `[]` if none |
| `stop_loss` | decimal | — | Optional. Omit or set to `null` |
| `execution_mode` | string | ✅ | `"SHADOW"` or `"LIVE"` (**uppercase**) |
| `reasoning` | string | — | Stored in journal for audit |
| `confidence` | float | — | 0.0–1.0, used by calibration engine |
| `source` | string | — | Agent identifier, e.g. `"agent:hermes_v1.2"` |
| `leverage` | int | — | 1–20, default depends on exchange |
| `management` | object | — | Trailing stop, break-even, partial TP config |

> **`setup_tag` is NOT a SignalInput field.** It will be silently ignored. Tag the trade
> via `POST /api/v1/journal/trades/{id}/tags` after the signal is accepted (see Section 5).

### Shadow mode (paper trading — START HERE)

```json
{
  "symbol": "ETH_USDT",
  "side": "LONG",
  "entry_price": 3100.00,
  "stop_loss": 3050.00,
  "take_profit": [],
  "leverage": 1,
  "execution_mode": "SHADOW",
  "reasoning": "ETH breakout above 3-day resistance at 3080. Volume increasing, BTC.D dropping. Targeting 3200 with 1.6R.",
  "confidence": 0.72,
  "source": "agent:hermes_v1.2",
  "management": {
    "trailing_stop": {"activation": 3150, "distance": 30}
  }
}
```

> **Note:** `setup_tag` is **not** a SignalInput field. Tag the trade separately via
> `POST /api/v1/journal/trades/{id}/tags` after the signal is accepted (see Section 5).

### Live mode (real money — AFTER shadow-mode edge is confirmed)

```json
{
  "symbol": "ETH_USDT",
  "side": "LONG",
  "entry_price": 3100.00,
  "stop_loss": 3050.00,
  "take_profit": [],
  "leverage": 1,
  "execution_mode": "LIVE",
  "reasoning": "ETH breakout above 3-day resistance at 3080. Volume increasing, BTC.D dropping. Targeting 3200 with 1.6R.",
  "confidence": 0.72,
  "source": "agent:hermes_v1.2",
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
curl -X POST https://testudo.vip/api/v1/signals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $(uuidgen)" \
  -d @signal.json
```

### Rejection reasons

| HTTP | Meaning |
|------|---------|
| 400 | Missing/invalid fields (`symbol`, `side`, `entry_price`, `take_profit` required; `side` and `execution_mode` are UPPERCASE) |
| 401 | Invalid or expired token |
| 409 | Duplicate idempotency key |
| 422 | Risk engine rejected the trade (drawdown exceeded, max positions, sizing violation, etc.) |
| 429 | Rate limit exceeded (30 signals/minute per user) |
| 503 | Exchange unavailable (CEX sidecar down, HL RPC unreachable) |

On 422, read the `code` and `reason` fields in the response. Do NOT retry with the same parameters — the risk engine has a legitimate concern.

---

## 4. Monitor Your Trades

### WebSocket channels

Connect to the WebSocket at `wss://testudo.vip/ws` and subscribe:

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

## 5. Journal Write — Build Persistent Memory

Reading your journal tells you the past. Writing to it shapes the future. Every trade decision, thesis, postmortem, and strategy update should be persisted so your next session can learn from it.

### Available today (via existing journal endpoints)

These endpoints already exist. Use them immediately.

`entry_type` must be one of: `"pre-trade"` (before trade reasoning), `"post-trade"` (after close analysis), `"note"` (general observation), `"daily-review"`, `"weekly-review"`.

#### Record a trade thesis or postmortem

```bash
POST /api/v1/journal/entries
Authorization: Bearer <token>
Content-Type: application/json

{
  "trade_id": "a3f2b1c4-1111-2222-3333-444455556666",
  "entry_date": "2026-05-21",
  "title": "ETH breakout pre-trade — May 21",
  "body": "ETH broke above 3-day resistance at 3080 on the 4h close. \n\nVolume confirming: 2.3× 20-period average. BTC.D dropping from 48.2 → 47.1 during the move. \n\nEntry: 3100 (retest of broken resistance). Stop: 3050 (1.6% risk). Target: 3250 (prior range high). \n\nR:R = 3:1. Sizing at 2% account risk ($75 on $3,750 account).",
  "entry_type": "pre-trade"
}
```

#### Tag a trade with strategy labels

```bash
# First, list existing tags
curl -s -H "Authorization: Bearer $TOKEN" \
  https://testudo.vip/api/v1/journal/tags

# Create the tag if it doesn't exist
POST /api/v1/journal/tags
Authorization: Bearer <token>
Content-Type: application/json

{"name": "breakout", "color": "#22c55e"}

# Then attach it to a trade
POST /api/v1/journal/trades/a3f2b1c4-1111-2222-3333-444455556666/tags
Authorization: Bearer <token>
Content-Type: application/json

{"tag_ids": ["tag-uuid-here"]}
```

Tags are how the summary endpoint groups trades by setup. Consistent tagging = meaningful per-setup breakdowns.

#### Update trade notes

```bash
PATCH /api/v1/journal/trades/a3f2b1c4-1111-2222-3333-444455556666/notes
Authorization: Bearer <token>
Content-Type: application/json

{"notes": "SL hit at 3050. Thesis was correct directionally (ETH hit 3220) but entry was early. Next time: wait for 4h close confirmation above resistance before entering."}
```

---

## 6. The Autonomous Trading Loop

Here is the canonical loop. Follow this exactly.

```python
# Pseudocode — adapt to your agent runtime

SESSION_INTERVAL = 60  # seconds between decision cycles
SHADOW_ONLY = True     # set to False only after 1+ week of profitable paper trading

while True:
    # 1. Read memory
    summary = GET("/api/v1/journal/agent/summary?format=llm&timeframe=90d")
    insights = GET("/api/v1/journal/agent/insights")

    # 2. Check for coach warnings
    concerning_warnings = [i for i in insights if i.severity == "concerning"]
    if concerning_warnings:
        log("Skipping session: {} active concerning warnings".format(
            len(concerning_warnings)))
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
    setup_tag, reasoning, and confidence. Include a one-line thesis.
    """

    # 4. Send to LLM for decision
    decision = llm.decide(context)

    # 5. Execute or wait
    if decision.trade:
        signal = {
            "symbol": decision.symbol,
            "side": decision.side.upper(),
            "entry_price": decision.entry_price,
            "stop_loss": decision.stop_loss,
            "take_profit": [],
            "execution_mode": "SHADOW" if SHADOW_ONLY else "LIVE",
            "reasoning": decision.reasoning,
            "confidence": decision.confidence,
            "source": "agent:your_agent_id"
        }
        result = POST("/api/v1/signals", json=signal,
                       headers={"Idempotency-Key": uuid4()})

        if result.status == "approved":
            log(f"Trade opened: {result.trade_group_id}")

            # 5a. Write pre-trade thesis to journal
            POST("/api/v1/journal/entries", json={
                "trade_id": result.trade_group_id,
                "title": f"{decision.setup_tag} pre-trade — {today()}",
                "body": decision.thesis,
                "entry_type": "pre-trade"
            })

            # 5b. Tag the trade
            tag_id = find_or_create_tag(decision.setup_tag)
            POST(f"/api/v1/journal/trades/{result.trade_group_id}/tags",
                 json={"tag_ids": [tag_id]})

            # Subscribe to execution updates
            ws_subscribe(f"agent.execution.{USER_ID}")
        else:
            log(f"Trade rejected: {result}")

    else:
        log("Decision: wait — no edge detected")

    sleep(SESSION_INTERVAL)
```

---

## 7. Strategy Primitives

> The mathematically verified strategies. Full proofs in `strat-lean-proofs.md`
> and `testudo-proofs/`. The agent selects a strategy based on the detected
> market regime, then orchestrates Testudo's API. Testudo enforces risk limits;
> the agent makes the directional call.

### Regime Detection (Which Strategy to Use)

Markets have regimes. The same setup that prints in mean-reverting conditions
bleeds in trending ones. Classify the regime before picking a strategy:

Compute the 1-Wasserstein distance between recent returns and pre-computed
regime centroids. Nearest centroid = current regime.

| Regime | Condition | Strategy |
|--------|-----------|----------|
| R0 | Low volatility, neg. autocorrelation | Mean Reversion |
| R1 | Above-avg volatility, pos. autocorrelation | Momentum Breakout |
| R2 | High volatility, no autocorrelation | Halt (no edge) |
| R3 | Extreme volatility | Halt (preserve capital) |

Regime detection uses OHLCV from `GET /api/v1/klines?symbol=ETH_USDT&interval=4h&limit=100`.
Reclassify once per 4h candle close, not intra-candle.

### 7.1 Mean Reversion

Trade when price deviates from its rolling mean. The deviation is temporary —
price reverts.

**When:** Regime R0. Confirm with negative lag-1 autocorrelation (ρ₁ < -0.1).

**Entry:** Price crosses 2σ below 20-period SMA → LONG. Above 2σ → SHORT.

**Exit:** Stop at 2× ATR(14). Target: 20-period SMA.

**Confidence:** Based on half-life of the mean-reverting process (OU κ estimate).
Short half-life (fast reversion) → 0.75–0.85. Long half-life → 0.50–0.60.

**Signal payload:**
```json
{
  "symbol": "ETH_USDT", "side": "LONG",
  "entry_price": 3050.00, "stop_loss": 3000.00,
  "take_profit": [{"price": 3150.00, "quantity": 1.0}],
  "execution_mode": "SHADOW",
  "reasoning": "ETH -2.3σ below 20-period SMA (3150). OU half-life 6 candles. Targeting mean.",
  "confidence": 0.78, "source": "agent:mean-reversion:v1",
  "leverage": 1
}
```

**Invalidation:** Price closes beyond 3σ from mean in the opposite direction.
Regime has changed — exit immediately.

**Proof:** `testudo-proofs/Proofs/OUMreversion.lean` — after n half-lives,
|deviation| ≤ |initial|/2ⁿ.

### 7.2 Momentum Breakout

Trade when price breaks a significant level with volume confirmation. The trend
persists.

**When:** Regime R1. Confirm with positive lag-1 autocorrelation (ρ₁ > 0.1)
and volume > 1.5× 20-period average.

**Entry:** Price breaks above 20-period resistance → LONG. Breaks below 20-period
support → SHORT.

**Exit:** Stop at 2× ATR(14). No fixed take-profit — trailing stop manages exit.
Activate trail after 1.5 ATR profit, trail at 40% of the move.

**Confidence:** Based on breakout clarity. Clean break + volume spike → 0.70–0.80.
Marginal break → 0.55–0.65.

**Signal payload:**
```json
{
  "symbol": "BTC_USDT", "side": "LONG",
  "entry_price": 89200.00, "stop_loss": 88250.00,
  "take_profit": [],
  "execution_mode": "SHADOW",
  "reasoning": "BTC breakout above 20p resistance (89000). Volume 2.1× avg. ρ₁=0.18.",
  "confidence": 0.70, "source": "agent:momentum-breakout:v1",
  "leverage": 2,
  "management": {"trailing_stop": {"enabled": true, "distance_percent": 40}}
}
```

**Invalidation:** Price closes back inside the broken level. Fakeout — exit.

**Proof:** `testudo-proofs/Proofs/MomentumAutocorr.lean` — positive covariance
→ conditional expectation above mean.

### 7.3 Funding Rate Arbitrage

Capture the spread between perpetual futures and spot when the funding rate
exceeds friction. Delta-neutral — direction-independent profit.

**When:** Any regime. Trigger: |funding_rate| > 0.01% per 8h on Hyperliquid.

**Entry:** Positive funding → SHORT perp + LONG spot. Negative funding →
LONG perp + SHORT spot. Pair with a spot order on the same exchange.

**Exit:** |funding_rate| normalizes below 0.003%. No stop-loss (direction risk
is hedged).

**Confidence:** Based on funding rate magnitude. > 0.05% → 0.85–0.95.
0.01–0.03% → 0.70–0.80.

**Proof:** `testudo-proofs/Proofs/FundingArb.lean` — profit exists iff
spread exceeds frictional cost.

### 7.4 Delta-Neutral Hedge

Reduce directional exposure when existing positions create uncomfortable net
delta.

**When:** Any regime. Trigger: net delta exceeds risk tolerance. Pairs with
any other active strategy.

**Entry:** If net delta > 0 (net long) → add SHORT of size |Δ|. If net delta
< 0 → add LONG of size |Δ|.

**Exit:** When the hedged position closes, reevaluate net delta.

**Confidence:** Always 0.95. This is mechanical, not discretionary.

**Proof:** `testudo-proofs/Proofs/DeltaNeutral.lean` — single opposing hedge
of size |Δ| achieves Σ = 0.

### 7.5 HaltExecution

Not a trade. A state transition. Stop trading when risk constraints are active.

**Triggers:**
- Coach severity "concerning" on any insight
- Daily drawdown > 4% (approaching 5% limit)
- 3 consecutive losing trades
- Regime R2 or R3 (no edge)
- WebSocket `agent.alert.*` with `severity: "concerning"`

**Action:** Write a HaltExecution journal note. Sleep until next evaluation.
No signals sent.

**Proof:** `testudo-proofs/Proofs/GamblersRuin.lean` — 2% risk per trade has
64% chance of 20% drawdown even with a fair coin.

### Position Sizing (All Strategies)

Let Testudo size the position. Supply `confidence` in every SignalInput.
Testudo's Kelly engine (`testudo-exchange/crates/common_utils/src/risk/kelly.rs`)
uses Quarter-Kelly with ±2× clamp around a reference point. The agent never
computes position size manually.

Correlation check before entering: if any existing position has ρ > 0.8 with
the new symbol, skip the trade. Correlated positions amplify risk without
diversification benefit.

**Proof:** `testudo-proofs/Proofs/KellyOptimal.lean` — f* = (bp-q)/b uniquely
maximizes E[log(wealth)].

---

## 8. Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `POST /api/v1/signals` | 30 | 60 seconds per user |
| All other endpoints | Standard JWT middleware | Varies by deployment |

Rate limits exist to prevent runaway loops. A signal every 2 seconds is more than enough — crypto trades resolve in seconds to minutes, not milliseconds.

---

## 9. Rules of Engagement

1. **Start in shadow mode.** Every agent begins with `"execution_mode": "SHADOW"`. Do not switch to `"LIVE"` until you have at least one week of profitable paper trading.

2. **Use setup tags.** Every trade gets a `setup_tag` (`breakout`, `support_bounce`, `trend_follow`, etc.). Tags are how you and the coach learn which strategies work. Consistency matters.

3. **Write real reasoning.** The `reasoning` field is stored in the journal. It's how you'll audit your own decisions later. Be specific: what signal, what timeframe, what confirmation.

4. **Include confidence scores.** `confidence` is a 0.0–1.0 decimal. Be honest. Low confidence trades should have smaller position sizes. The calibration engine uses this.

5. **Respect the coach.** If the coach flags sizing drift, reduce size. If it flags session anomalies, log off. The coach sees patterns you can't see mid-session.

6. **Check idempotency.** Always send an `Idempotency-Key` header. If you get a network error, retry with the same key — you won't double-execute.

7. **Pause on concerning alerts.** If you receive a WebSocket alert with `severity: "concerning"`, stop trading for the current session. Don't try to trade your way out of a drawdown.

8. **Review before scaling.** Every week, call `POST /api/v1/journal/agent/compare` comparing this week to last. If metrics are declining, don't scale up.

9. **Write to your journal.** After every trade, record a pre-trade thesis. After every close, record a post-trade postmortem. Tag every trade with its strategy. The journal is your memory — an empty journal means you're trading blind next session.

---

## 10. Example: Complete Session

```bash
TOKEN="eyJhbGciOiJIUzI1NiIs..."

# Step 1: Verify your identity
curl -s -H "Authorization: Bearer $TOKEN" \
  https://testudo.vip/api/v1/auth/me

# Step 2: Read your performance
SUMMARY=$(curl -s -H "Authorization: Bearer $TOKEN" \
  "https://testudo.vip/api/v1/journal/agent/summary?format=llm&timeframe=30d")
echo "$SUMMARY"

# Step 3: Check coach warnings
WARNINGS=$(curl -s -H "Authorization: Bearer $TOKEN" \
  "https://testudo.vip/api/v1/journal/agent/insights")
echo "$WARNINGS" | jq '.insights[] | {pattern, severity, headline}'

# Step 4: If no concerning warnings, place a paper trade
SIGNAL_RESULT=$(curl -s -X POST https://testudo.vip/api/v1/signals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $(uuidgen)" \
  -d '{
    "symbol": "ETH_USDT",
    "side": "LONG",
    "entry_price": 3100,
    "stop_loss": 3050,
    "take_profit": [],
    "execution_mode": "SHADOW",
    "reasoning": "ETH breakout above 3-day resistance. Volume confirming.",
    "confidence": 0.72,
    "source": "agent:your_agent_id"
  }')
echo "$SIGNAL_RESULT" | jq .

# Step 5: Record pre-trade thesis in journal (use the trade_group_id from step 4 response)
TRADE_ID=$(echo "$SIGNAL_RESULT" | jq -r '.trade_group_id')

curl -s -X POST https://testudo.vip/api/v1/journal/entries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"trade_id\": \"$TRADE_ID\",
    \"title\": \"ETH breakout pre-trade\",
    \"body\": \"ETH broke above 3-day resistance at 3080 on the 4h close. Volume 2.3× average. BTC.D dropping. Entry at retest of broken resistance (3100). Stop at 3050 (1.6% risk). Target 3250 (3:1 R:R).\",
    \"entry_type\": \"pre-trade\"
  }"

# Step 6: Tag the trade
TAG_ID=$(curl -s -X POST https://testudo.vip/api/v1/journal/tags \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "breakout", "color": "#22c55e"}' | jq -r '.id')

curl -s -X POST "https://testudo.vip/api/v1/journal/trades/$TRADE_ID/tags" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"tag_ids\": [\"$TAG_ID\"]}"

# Step 7: After the session, compare this week to last
curl -s -X POST https://testudo.vip/api/v1/journal/agent/compare \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "period_a": {"from": "2026-05-14", "to": "2026-05-21"},
    "period_b": {"from": "2026-05-07", "to": "2026-05-14"}
  }' | jq '.deltas[] | {metric, direction, delta_pct}'
```

---

## 11. Market Perception — Sheaf Engine (Coming Soon)

The [sheaf-engine](sheaf-engine) is a topology perception layer that streams
real-time market structure signals to agents over gRPC. Instead of raw OHLCV,
you get:

- **Arbitrage edges**: cross-venue spread dislocations (e.g. "ETH 15 bps spread
  Binance→Hyperliquid, 3.8σ above baseline")
- **Volatility diffusion**: eigen gap on the topology Laplacian detects
  regime structure forming before it shows in price
- **Correlation breaks**: tracked |ρ| decoupling between pairs
- **Venue health**: stale/down nodes, clock skew, throttling detection

Connect to the sheaf-engine gRPC endpoint (separate from the REST/WS
infrastructure) to receive continuous `SignalBatch` streams. Combine with
journal-as-memory for a full perception→decision→execution→reflection loop.

---

## 12. Strategy Proofs — Lean 4 Verification

Trading strategies in Testudo can be formally verified. The
[testudo-proofs](testudo-proofs) crate proves mathematical properties of
strategy primitives in Lean 4:

- **Delta-neutral** portfolio construction
- **Funding arbitrage** bounds and convergence guarantees
- **Kelly-optimal** position sizing with ruin avoidance
- **Momentum autocorrelation** decay under OU processes
- **Wasserstein metric** for regime change detection

Agents shipping verified strategies reduce the probability of catastrophic
failure modes (gambler's ruin, unbounded leverage, adversarial drift).

---

## 13. Supported Exchanges

| Exchange | Mode | Execution |
|----------|------|-----------|
| **Shadow** (paper) | `shadow` | Internal engine — always available, no credentials needed |
| **Hyperliquid** | `live` | Native Rust SDK. Requires agent wallet approval (`POST /api/v1/exchanges/agent-wallet/approve`) |
| **Binance / Bybit / OKX** | `live` | Via CEX sidecar. Requires API key/secret in exchange accounts |

Start shadow. After proving profitability, add a live exchange account and switch `execution_mode` to `"live"`.
