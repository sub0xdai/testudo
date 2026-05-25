# Strategy Primitives with Lean 4 Proofs

> **For LLMs (Hermes, OpenClaw, pi) implementing autonomous trading on Testudo.**
>
> This document provides mathematically proven strategy primitives. Every formula
> here has a corresponding Lean 4 proof. You don't compute — you reference.
> Your job: classify the regime, select the strategy, call Testudo's API.
>
> Companion: [AGENT_TRADING.md](./AGENT_TRADING.md) — API reference, authentication, journal queries.

---

## Table of Contents

1. [Philosophy — Why Proofs for LLM Trading](#0-philosophy)
2. [Regime Detection — Wasserstein Distance](#1-regime-detection)
3. [Position Sizing — Kelly + Correlation](#2-position-sizing)
4. [Entry & Exit — Strategy Primitives](#3-entry--exit)
   - [Mean Reversion](#31-mean-reversion)
   - [Momentum Breakout](#32-momentum-breakout)
   - [Funding Rate Arbitrage](#33-funding-rate-arbitrage)
   - [Delta-Neutral Hedge](#34-delta-neutral-hedge)
5. [Risk Constraints — Drawdown & Ruin](#4-risk-constraints)
6. [The Canonical Agent Loop](#5-the-canonical-agent-loop)
7. [Lean 4 Proofs](#6-lean-4-proofs)
8. [Testudo API Quick Reference](#7-testudo-api-quick-reference)

---

## 0. Philosophy

### The Problem

LLMs hallucinate quantitative reasoning. Ask an LLM to compute optimal position
size for a 58% win-rate strategy with a 1.8× reward-to-risk ratio, and you'll
get a different answer every time — sometimes close, sometimes dangerously wrong.

### The Solution

Every strategy primitive in this document is backed by a **Lean 4 proof**. The
Lean 4 verification layer eliminates mathematical hallucination. You reference
the proven formula, not your own computation.

### Division of Labor

| Component | Job |
|-----------|-----|
| **Lean 4** | Guarantee that the formulas are correct |
| **You (the LLM)** | Classify market regime, select strategy, call Testudo's API |
| **Testudo** | Enforce risk limits, size positions, execute orders, journal everything |

### How to Use This Document

1. Read §1 to understand how to detect the market regime
2. Based on the regime, pick a strategy from §3
3. Use §2 for position sizing (Testudo's Kelly engine does the heavy lifting)
4. Use §4 to know when to stop trading
5. Follow §5 for the canonical agent loop
6. Reference §6 for the formal proofs when you need mathematical certainty

---

## 1. Regime Detection

### 1.1 The Core Insight

Markets don't have one "correct" strategy. They have regimes. The same setup
that prints money in a mean-reverting regime will bleed dry in a trending
regime. The first job of any trading agent is **regime classification**.

### 1.2 1-Wasserstein Distance as a Regime Classifier

**Theorem (Wasserstein Regime Classification).** Let μ be the empirical
distribution of recent returns (last N candles) and let {ν₁, …, νₖ} be k
historical regime centroids. The current regime is argminᵢ W₁(μ, νᵢ), where
W₁ is the 1-Wasserstein distance (Earth Mover's Distance).

The 1-Wasserstein distance between two distributions on ℝ has a closed form:

$$
W_1(\mu, \nu) = \int_{-\infty}^{\infty} |F_\mu(x) - F_\nu(x)| \, dx
$$

where F_μ and F_ν are the cumulative distribution functions. For empirical
distributions (ordered samples x₁ ≤ … ≤ xₙ and y₁ ≤ … ≤ yₘ):

$$
W_1(\mu, \nu) = \frac{1}{n} \sum_{i=1}^{n} \left| x_{(i)} - y_{\text{quantile}(i/n)} \right|
$$

This is trivially computable — sort both arrays, align by quantile, sum
absolute differences.

**Lean 4 Proof:** See §6.1. Proves W₁ is a metric (non-negativity, symmetry,
triangle inequality) and preserves the ordering of distributional similarity.

### 1.3 Regime Centroids

Pre-computed historical centroids (offline). Each centroid is a tuple of the
empirical distribution of 4h returns over a 30-day window, classified by human
or algorithmic label:

| Regime ID | Label | Volatility Percentile | Autocorrelation (lag-1) | Characteristic |
|-----------|-------|----------------------|------------------------|----------------|
| R₀ | Low Vol / Mean-Reverting | < 30th | ρ₁ < -0.1 | Range-bound, oscillating |
| R₁ | Trending / Momentum | > 50th | ρ₁ > 0.1 | Directional, persistent |
| R₂ | High Vol / Regime Change | > 80th | ρ₁ ≈ 0 | Choppy, no edge |
| R₃ | Extreme | > 95th | — | Black swan, halt trading |

### 1.4 Computing W₁ from OHLCV Data

```
# Pseudocode — the LLM orchestrates this, Testudo's klines endpoint provides data

function compute_wasserstein_regime(symbol: string) -> Regime:
    # GET /api/v1/klines?symbol=BTC_USDT&interval=4h&limit=100
    candles = testudo.get_klines(symbol, "4h", 100)

    # Compute log returns
    returns = [ln(candles[i].close / candles[i-1].close) for i in 1..len(candles)]
    returns.sort()

    # Compute W₁ to each centroid
    distances = {}
    for (regime_id, centroid) in PRECOMPUTED_CENTROIDS:
        distances[regime_id] = wasserstein_1d(returns, centroid.samples)

    # Return nearest regime
    return argmin(distances)
```

### 1.5 Regime → Strategy Mapping

| Detected Regime | Strategy to Use | Rationale |
|-----------------|----------------|-----------|
| R₀ (low vol, mean-reverting) | MeanReversion | Price oscillates around mean — fade extremes |
| R₁ (trending, momentum) | MomentumBreakout | Price persists in direction — follow the trend |
| R₂ (high vol, choppy) | HaltExecution | No edge — don't trade noise |
| R₃ (extreme, black swan) | HaltExecution | Risk of ruin — preserve capital |
| Funding rate |funding_rate| > threshold | FundingArbitrage | Capture the spread regardless of regime |
| Any regime, hedging existing exposure | DeltaNeutralHedge | Reduce portfolio delta when correlation spikes |

### 1.6 Regime Detection Frequency

Compute regime once per 4h candle close. Don't reclassify intra-candle — noise
dominates signal at higher frequencies.

---

## 2. Position Sizing

### 2.1 The Kelly Criterion

**Theorem (Kelly Optimality).** For a sequence of independent bets each with
win probability p and net odds b (where a winning bet returns b× the amount
risked), the fraction f* of bankroll that maximizes the expected geometric
growth rate E[log(Wₙ)] is:

$$
f^* = \frac{b \cdot p - q}{b}, \quad q = 1-p
$$

When f* ≤ 0, no bet is placed (negative edge).

The optimal geometric growth rate is:

$$
G^* = p \cdot \log(1 + b \cdot f^*) + q \cdot \log(1 - f^*)
$$

**Lean 4 Proof:** See §6.2. Proves via Jensen's inequality that f* uniquely
maximizes E[log(1 + f·X)] for the binary-outcome random variable X.

**Critical:** The full Kelly fraction is aggressive and assumes independent
sequential bets. **Testudo already implements Quarter-Kelly** with a ±2× clamp
around a reference point. You do NOT compute Kelly yourself — you supply a
`confidence` score (0.0–1.0) in the SignalInput, and Testudo's calibration
engine maps it through the Kelly pipeline.

See `testudo-exchange/crates/common_utils/src/risk/kelly.rs`:

```
full_kelly(p_eff, avg_r_win, avg_r_loss) → quarter_kelly → edge_multiplier
→ effective_risk_percent = baseline × edge_multiplier (clamped [0.25×, 2×])
```

### 2.2 What the LLM Supplies

In every `POST /api/v1/signals`:

```json
{
  "confidence": 0.72,
  ...
}
```

The `confidence` field (0.0–1.0) is used by Testudo's Kelly pipeline to
fine-tune sizing when `dynamic_risk_enabled: true` in the user's risk config.
It's also stored in the journal for calibration feedback.

### 2.3 Correlation-Adjusted Position Sizing

**Theorem (Portfolio Variance).** For a portfolio of n positions with
weights w and correlation matrix Σ, the portfolio variance is w' Σ w.
The effective number of independent positions is:

$$
n_{\text{eff}} = \frac{(\sum_i \sigma_i)^2}{\sum_i \sum_j \sigma_i \sigma_j \rho_{ij}}
$$

When positions are correlated, the agent should reduce the number of
concurrent positions. If ρ > 0.7 between two assets you hold, don't add
a third correlated position — you're already overexposed.

**Rule of thumb for the LLM:**
- ρ > 0.8 → treat as a single position (max 1 of the correlated cluster)
- ρ > 0.5 → reduce max positions by 1 for each pair above this threshold
- ρ < 0.3 → positions are effectively independent

### 2.4 Sizing Summary (What You Actually Do)

```
function decide_position_size(signal, journal_summary, correlation_matrix):
    # 1. Supply confidence to Testudo — it handles Kelly
    signal.confidence = compute_confidence(signal)

    # 2. Check Testudo's max positions limit
    active_positions = len(journal_summary.active_trades)
    if active_positions >= RISK_CONFIG.max_open_positions:
        return NO_TRADE

    # 3. Correlation check
    for existing_position in journal_summary.active_trades:
        ρ = correlation(existing_position.symbol, signal.symbol)
        if ρ > 0.8:
            return NO_TRADE  # too correlated

    # 4. Proceed — Testudo sizes the position
    return PROCEED
```

---

## 3. Entry & Exit

### 3.1 Mean Reversion

#### Theorem (Ornstein-Uhlenbeck Process)

An OU process Xₜ follows the SDE dXₜ = κ(θ − Xₜ) dt + σ dWₜ, where:
- θ is the long-term mean
- κ is the mean-reversion speed (higher = faster reversion)
- σ is volatility
- Wₜ is a Wiener process

The conditional expectation after time t is:

$$
\mathbb{E}[X_t \mid X_0 = x] = \theta + (x - \theta)e^{-\kappa t}
$$

The half-life (time for deviation to decay by 50%) is τ = ln(2)/κ.

**Lean 4 Proof:** See §6.3. Proves the conditional expectation and bounds on
the probability of non-reversion within n half-lives.

#### When to Use

- Regime R₀ (low vol, mean-reverting)
- Detected when W₁ distance to R₀ centroid is smallest
- Confirm: ρ₁ < -0.1 (negative lag-1 autocorrelation)

#### Entry Condition

A deviation of ≥ 2σ from the rolling mean (20-period) where σ is the standard
deviation of the rolling window.

```
rolling_mean = SMA(close, 20)
rolling_std = STD(close, 20)
deviation = (close - rolling_mean) / rolling_std

if deviation ≤ -2.0:  # oversold
    side = LONG
    entry = close
elif deviation ≥ 2.0:  # overbought
    side = SHORT
    entry = close
else:
    NO_TRADE
```

**Confidence scaling for mean reversion:**

```
half_life = ln(2) / estimate_kappa(returns, 20)
if half_life < 10 candles:          # fast reversion — strong signal
    confidence = 0.80
elif half_life < 20 candles:        # moderate reversion
    confidence = 0.65
elif half_life < 40 candles:        # slow reversion — weak signal
    confidence = 0.50
else:                               # not mean-reverting
    NO_TRADE
```

To estimate κ from OHLCV data: fit AR(1) model — κ = −ln(φ)/Δt where φ is
the AR(1) coefficient.

#### Exit Conditions

```
# Stop loss
stop_loss = entry - (2.0 * rolling_std)  # puts stop at ~3σ from mean

# Take profit
take_profit = rolling_mean  # exit when price returns to mean

# Invalidation
if price moves beyond 3σ from mean in the OPPOSITE direction:
    EXIT_IMMEDIATELY  # the regime has changed — this isn't mean-reverting anymore
```

#### Testudo Signal Payload (Mean Reversion — LONG example)

```json
{
  "symbol": "ETH_USDT",
  "side": "LONG",
  "entry_price": 3050.00,
  "stop_loss": 3000.00,
  "take_profit": [{"price": 3150.00, "quantity": 1.0}],
  "execution_mode": "SHADOW",
  "reasoning": "ETH -2.3σ from 20-period mean (3150). Fast reversion half-life 6 candles (κ=0.115). R₀ regime confirmed (W₁=0.0012). Targeting mean reversion to 3150 (+3.3%). 1:3.3 R:R at 2σ stop.",
  "confidence": 0.78,
  "source": "agent:hermes_v2",
  "leverage": 1,
  "management": {
    "trailing_stop": {
      "enabled": true,
      "distance_percent": 50
    }
  }
}
```

#### Pre-Trade Journal Entry (immediately after signal acceptance)

```bash
POST /api/v1/journal/entries
{
  "trade_id": "<trade_group_id from signal response>",
  "entry_date": "2026-05-25",
  "title": "ETH Mean Reversion LONG — May 25 14:00 UTC",
  "body": "## Regime\nR₀ (low vol, mean-reverting). W₁ to centroid: 0.0012.\n\n## Thesis\nETH at 3050 is -2.3σ below 20-period SMA (3150). OU half-life estimated at 6 candles (κ=0.115) — fast mean reversion expected.\n\n## Entry\nLimit LONG at 3050. Stop at 3000 (-1.6%). Target 3150 (+3.3%). R:R = 1:2.1.\n\n## Confidence\n0.78 — strong signal from fast reversion speed + deep deviation.\n\n## Invalidation\n1. Price closes below 3000 (3σ) — regime change, cut immediately.\n2. Half-life extends beyond 20 candles — reversion speed has decayed, exit.",
  "entry_type": "pre-trade"
}
```

---

### 3.2 Momentum Breakout

#### Theorem (Return Autocorrelation and Momentum)

If returns exhibit positive autocorrelation at lag h (ρₕ > 0), then:

$$
\mathbb{E}[r_{t+h} \mid r_t > 0] > 0
$$

The conditional expected return after a positive signal is positive. The
strength of the momentum signal is proportional to the magnitude of ρₕ.

**Lean 4 Proof:** See §6.4. Proves that for any stationary process with
positive autocorrelation, a positive return predicts a positive expected
return at the autocorrelated lag.

#### When to Use

- Regime R₁ (trending / momentum)
- Detected when W₁ distance to R₁ centroid is smallest
- Confirm: ρ₁ > 0.1 (positive lag-1 autocorrelation)
- Volume confirmation: current volume > 1.5× 20-period average

#### Entry Condition

Break of a significant level with volume confirmation:

```
# Identify levels
resistance = max(high[-20:])  # 20-period resistance
support = min(low[-20:])      # 20-period support

volume_ratio = volume / SMA(volume, 20)

if close > resistance AND volume_ratio > 1.5:
    side = LONG
    entry = resistance + (0.2 * ATR(14))  # micro-buffer above breakout
elif close < support AND volume_ratio > 1.5:
    side = SHORT
    entry = support - (0.2 * ATR(14))
else:
    NO_TRADE
```

#### Exit Conditions

```
# Stop loss
stop_loss = entry - (2.0 * ATR(14))  # wide enough to survive noise

# Take profit
take_profit = entry + (3.0 * ATR(14))  # 1.5:1 R:R minimum

# Trailing stop — the key to momentum
trailing_activation = entry + (1.5 * ATR(14))  # activate after 1.5 ATR profit
trailing_distance_percent = 50  # trail at 50% of the move

# Invalidation
if price closes back inside the broken level (below resistance or above support):
    EXIT  # the breakout failed — fakeout
```

#### Testudo Signal Payload (Momentum Breakout — LONG example)

```json
{
  "symbol": "BTC_USDT",
  "side": "LONG",
  "entry_price": 89200.00,
  "stop_loss": 88250.00,
  "take_profit": [],
  "execution_mode": "SHADOW",
  "reasoning": "BTC breakout above 20-period resistance (89000). Volume 2.1× average. R₁ regime (W₁=0.0008, ρ₁=0.18). Momentum regime active, following trend.",
  "confidence": 0.70,
  "source": "agent:hermes_v2",
  "leverage": 2,
  "management": {
    "trailing_stop": {
      "enabled": true,
      "distance_percent": 40
    }
  }
}
```

Note: `take_profit: []` because a trailing stop manages the exit — the trade
runs until the trailing stop is hit. No fixed TP target in trending regimes.

#### Pre-Trade Journal Entry

```bash
POST /api/v1/journal/entries
{
  "trade_id": "<trade_group_id from signal response>",
  "entry_date": "2026-05-25",
  "title": "BTC Momentum Breakout LONG — May 25 16:00 UTC",
  "body": "## Regime\nR₁ (trending/momentum). W₁ to centroid: 0.0008. ρ₁ = 0.18.\n\n## Thesis\nBTC broke above 89000 resistance (20-period high) with 2.1× volume. Momentum regime confirmed. Trailing stop at 40% of move after 1.5 ATR activation.\n\n## Entry\nLimit LONG at 89200. Stop at 88250 (-1.1%). No fixed TP — trailing stop manages exit.\n\n## Confidence\n0.70 — clear breakout but leverage 2× amplifies risk.\n\n## Invalidation\nPrice closes back below 89000 (fakeout). Exit immediately. No second attempt on same level without re-accumulation.",
  "entry_type": "pre-trade"
}
```

---

### 3.3 Funding Rate Arbitrage

#### Theorem (No-Arbitrage Bound)

In an arbitrage-free market, the futures price F and spot price S satisfy:

$$
|F_t - S_t(1 + r \cdot \Delta t)| \leq \varepsilon
$$

where r is the risk-free rate and ε accounts for transaction costs and market
friction. When the funding rate |f| exceeds this bound, a delta-neutral
position captures the spread.

The instantaneous P&L from a funding-rate arbitrage position of size Q held
for time Δt is:

$$
\text{P\&L} = Q \cdot |f| \cdot \Delta t - \text{fees}
$$

This is **deterministic profit** — market-neutral, direction-independent. The
only risks are execution slippage and exchange solvency.

**Lean 4 Proof:** See §6.5. Proves the no-arbitrage bound and that a
delta-neutral position (long spot, short perp) earns exactly the funding
rate premium minus friction.

#### When to Use

- Any regime — funding arbitrage is market-direction independent
- Trigger: |funding_rate| > 0.01% per 8h (0.03% daily) on Hyperliquid
- Higher funding rates = stronger signal
- Confirm: sufficient liquidity on both legs

#### Entry Condition

| Condition | Signal |
|-----------|--------|
| funding_rate > +0.01% (perps overpriced) | Short perp + Long spot = capture positive funding |
| funding_rate < -0.01% (perps underpriced) | Long perp + Short spot = capture negative funding |
| |funding_rate| > 0.05% | Strong signal — increase size up to correlation-adjusted max |
| |funding_rate| < 0.005% | Not worth the fees — HaltExecution |

#### The Delta-Neutral Construction

```
# Positive funding (perps > spot): short the perp, long the spot
leg_1 = POST /api/v1/signals  # SHORT perp, e.g., ETH_USDT on Hyperliquid
leg_2 = POST /api/v1/signals  # LONG spot, same size, e.g., ETH on Hyperliquid spot

# The positions offset: PnL = funding_payments - fees
# Market direction risk is cancelled: dP/dS ≈ 0
```

**Currently:** Testudo supports Hyperliquid perps natively. Spot-side hedging
requires the CEX sidecar (Binance spot) or a separate Hyperliquid spot order.
For the initial implementation, trade only the perp leg and accept directional
exposure — or pair with a spot position manually.

#### Exit Condition

```
if |funding_rate| < 0.003%:
    EXIT  # spread has collapsed — no more edge
```

This is not a stop-loss-based exit. Funding arbitrage has no market-directional
risk (if properly hedged), so a stop loss makes no sense. Exit when the funding
rate normalizes.

---

### 3.4 Delta-Neutral Hedge

#### Theorem (Portfolio Delta)

For a portfolio with value V = Σᵢ wᵢ Pᵢ, the delta (sensitivity to
underlying price) is:

$$
\Delta = \sum_i w_i \frac{\partial P_i}{\partial S}
$$

A delta-neutral portfolio has Δ = 0, meaning the portfolio value is
insensitive to small movements in the underlying.

**Lean 4 Proof:** See §6.6. Proves that for linear instruments (spot,
futures, perpetuals), delta is the sum of signed position sizes, and
delta neutrality is achieved when Σ sign(posᵢ) × sizeᵢ = 0.

#### When to Use

- Any regime where you have existing directional exposure
- When adding a new position would create uncomfortable net delta
- Hedge ratio: size_of_hedge = existing_delta / hedge_instrument_delta_per_unit
- Not a standalone strategy — pairs with any other strategy to reduce risk

#### Construction

```
net_delta = sum(position.size * position.direction for position in active_positions)

# Direction convention: LONG = +1, SHORT = -1
# If net_delta > 0: you're net long — add a SHORT to hedge
# If net_delta < 0: you're net short — add a LONG to hedge

hedge_size = abs(net_delta)  # in base currency units

POST /api/v1/signals
{
  "symbol": "BTC_USDT",
  "side": "SHORT",              // opposite direction of net exposure
  "entry_price": <market>,
  "stop_loss": null,            // no stop on hedges — defeats the purpose
  "take_profit": [],
  "execution_mode": "SHADOW",   // start in shadow
  "reasoning": "Delta-neutral hedge: net delta +$1,500. Hedging with SHORT 0.02 BTC at market.",
  "confidence": 0.95,           // high confidence — this is mechanical, not discretionary
  "source": "agent:hermes_v2",
  "leverage": 1
}
```

---

## 4. Risk Constraints

### 4.1 The Risk of Ruin

**Theorem (Gambler's Ruin for Sequential Bets).** For n sequential trades each
risking fraction f of bankroll, the probability of drawdown to fraction α of
initial capital before reaching fraction β is bounded by:

$$
P(\text{ruin before } \beta) \leq \left(\frac{1-f}{1+f}\right)^{\log(1/\alpha)/f}
$$

Corollary: For f = 0.02 (2% risk per trade) and α = 0.80 (20% drawdown):

$$
P(\text{20% drawdown}) \leq \left(\frac{0.98}{1.02}\right)^{11.16} \approx 0.64
$$

Meaning: even with no edge (50% win rate, 1:1 R:R), there's a 64% chance of
hitting 20% drawdown before doubling with 2% risk per trade. At 5% risk,
this rises to 78%. At 10% risk, it's 88%.

**Lean 4 Proof:** See §6.7. Proves the bound via Doob's optional stopping
theorem on the log-wealth martingale.

### 4.2 What Testudo Already Enforces

Testudo's risk engine checks these before any order hits the exchange:

| Check | Default | What you must do |
|-------|---------|-----------------|
| Stop loss required | Yes | Always include `stop_loss` in SignalInput |
| Max positions | 5 | Don't exceed `max_open_positions` |
| Daily drawdown limit | 5% | Stop trading if you receive `DailyDrawdownExceeded` alert |
| Max leverage | 125 | Respect — but practically, use ≤ 3× for any strategy |
| Min risk/reward | 1.5 | Entry price to stop loss distance must be ≤ 2/3 of stop to target |
| Max position size | Account % | Let Testudo compute; don't override |
| Max risk amount | Configurable | Conservative default is 2% per trade |

### 4.3 When to HaltExecution

The following conditions require immediate trading cessation:

| Condition | Why |
|-----------|-----|
| Coach severity "concerning" on any insight | Patterns you can't see mid-session |
| Daily drawdown > 4% (approaching 5% limit) | Protection before the hard stop |
| 3 consecutive losing trades | Re-evaluate regime classification |
| Regime R₂ or R₃ (high vol, extreme) | No edge in these environments |
| Session outside optimal hours | Per journal, your best hours exist — trade then |
| `agent.alert.*` WebSocket message with `severity: "concerning"` | Real-time risk breach |

### 4.4 The HaltExecution Directive

```json
{
  "strategy_module": "HaltExecution",
  "thesis_summary": "Risk constraints triggered: [reason]. Resuming after: [condition].",
  "exchange": null,
  "max_leverage": 0,
  "margin_type": null,
  "invalidation_criteria": "[condition for resuming]"
}
```

A HaltExecution is NOT a trade. It's a state transition that the LLM records
in the journal as a note, then sleeps until the next evaluation cycle. No
signal is sent to Testudo.

Journal entry for HaltExecution:

```bash
POST /api/v1/journal/entries
{
  "trade_id": null,
  "entry_date": "2026-05-25",
  "title": "HaltExecution — Daily Drawdown at 4.2%",
  "body": "## Reason\nDaily drawdown at 4.2% approaching 5% limit. 3 consecutive losing trades (mean reversion signals in R₁-trending regime — regime misclassification suspected).\n\n## Action\nHalting all trading. Will re-evaluate regime at next 4h candle close (18:00 UTC).\n\n## Resume Condition\n1. Regime confirmed as R₀ or R₁ (not R₂/R₃).\n2. Drawdown below 2%.\n3. Coach insights clear of concerning patterns.",
  "entry_type": "note"
}
```

---

## 5. The Canonical Agent Loop

### 5.1 Complete Decision Cycle

```
# Run every 60 seconds
# All Testudo endpoints use Authorization: Bearer <token>

while True:
    # ─── Phase 1: Read Memory ───
    journal = GET /journal/agent/summary?format=llm&timeframe=90d
    insights = GET /journal/agent/insights

    # ─── Phase 2: Safety Gate ───
    concerning = [i for i in insights if i.severity == "concerning"]
    if concerning:
        log("HALT: {} active concerning patterns".format(len(concerning)))
        POST /journal/entries  # HaltExecution note
        sleep(3600)  # re-check in 1 hour
        continue

    # ─── Phase 3: Regime Detection ───
    klines = GET /api/v1/klines?{symbols}&interval=4h&limit=100
    regime = compute_wasserstein_regime(klines)
    funding_rate = get_funding_rate()  # from Hyperliquid API or Testudo ticker

    # ─── Phase 4: Strategy Selection ───
    if regime in [R2, R3]:
        POST /journal/entries  # HaltExecution note
        sleep(3600)
        continue

    if |funding_rate| > 0.01%:
        active_strategy = FundingArbitrage
    elif regime == R0:
        active_strategy = MeanReversion
    elif regime == R1:
        active_strategy = MomentumBreakout

    # ─── Phase 5: Signal Generation ───
    signal = generate_signal(active_strategy, klines, journal)
    if signal == NO_TRADE:
        log("No edge detected in current market")
        sleep(60)
        continue

    # ─── Phase 6: Correlation Check ───
    for existing_pos in journal.active_trades:
        ρ = correlation(existing_pos.symbol, signal.symbol)
        if ρ > 0.8 and active_strategy != DeltaNeutralHedge:
            log("Skipping {} — too correlated with existing {}".format(
                signal.symbol, existing_pos.symbol))
            sleep(60)
            continue

    # ─── Phase 7: Execute ───
    result = POST /api/v1/signals
    {
        "symbol": signal.symbol,
        "side": signal.side,
        "entry_price": signal.entry_price,
        "stop_loss": signal.stop_loss,
        "take_profit": signal.take_profit,
        "execution_mode": "SHADOW",  # always start shadow
        "reasoning": signal.reasoning,
        "confidence": signal.confidence,
        "source": "agent:your_agent_id",
        "leverage": signal.leverage,
        "idempotency_key": uuid4(),
        "management": signal.management
    }

    # ─── Phase 8: Journal Write ───
    if result.status == "approved":
        POST /journal/entries  # pre-trade thesis
        POST /journal/trades/{id}/tags  # tag with strategy label
        subscribe_websocket("agent.execution.{user_id}")
        subscribe_websocket("agent.alert.{user_id}")
        log("Trade {} opened: {} {} @ {} ({} mode, {} confidence)".format(
            result.trade_group_id, signal.side, signal.symbol,
            signal.entry_price, signal.execution_mode, signal.confidence))

    # ─── Phase 9: Wait ───
    sleep(60)
```

### 5.2 generate_signal() — Per-Strategy Logic

```
function generate_signal(strategy, klines, journal):
    if strategy == MeanReversion:
        return generate_mean_reversion_signal(klines)
    elif strategy == MomentumBreakout:
        return generate_momentum_signal(klines)
    elif strategy == FundingArbitrage:
        return generate_funding_arb_signal(funding_rate)
    elif strategy == DeltaNeutralHedge:
        return generate_delta_hedge_signal(journal.active_trades)
    else:
        return NO_TRADE
```

### 5.3 Shadow → Live Graduation Criteria

Do not switch from `SHADOW` to `LIVE` until:

| Metric | Threshold |
|--------|-----------|
| Shadow trades | ≥ 50 |
| Win rate | > 45% |
| Avg R-multiple | > 0.5 (positive edge) |
| Profit factor | > 1.1 |
| Weeks of shadow trading | ≥ 1 |
| Coach concerning patterns | 0 active |

Check with: `GET /journal/agent/summary?format=json&source=agent:your_id`

---

## 6. Lean 4 Proofs

Machine-checked theorems. See `testudo-proofs/Proofs/` for the verifiable
Lean 4 source. Each `.lean` file corresponds to one subsection below.

| § | Theorem | File |
|---|---------|------|
| 6.1 | W1 is a metric on R | `Proofs/WassersteinMetric.lean` |
| 6.2 | Kelly optimality | `Proofs/KellyOptimal.lean` |
| 6.3 | OU mean reversion bound | `Proofs/OUMreversion.lean` |
| 6.4 | Momentum autocorrelation | `Proofs/MomentumAutocorr.lean` |
| 6.5 | Funding no-arbitrage bound | `Proofs/FundingArb.lean` |
| 6.6 | Portfolio delta neutrality | `Proofs/DeltaNeutral.lean` |
| 6.7 | Gambler's ruin bound | `Proofs/GamblersRuin.lean` |

Build: `cd testudo-proofs && lake build`

---

## 7. Testudo API Quick Reference

---

## 7. Testudo API Quick Reference

### Core Endpoints for Strategy Execution

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/auth/nonce` | GET | Get SIWE nonce |
| `/api/v1/auth/verify-siwe` | POST | Exchange signature for JWT |
| `/api/v1/auth/me` | GET | Verify token + get user info |
| `/api/v1/signals` | POST | Place a trade (shadow or live) |
| `/api/v1/journal/agent/summary?format=llm` | GET | Read performance summary (inject into LLM context) |
| `/api/v1/journal/agent/insights` | GET | Coach pattern detection |
| `/api/v1/journal/agent/compare` | POST | Period-over-period comparison |
| `/api/v1/journal/entries` | POST | Write pre-trade/post-trade/note |
| `/api/v1/journal/tags` | GET/POST | Manage strategy tags |
| `/api/v1/journal/trades/{id}/tags` | POST | Tag a trade with strategy label |
| `/api/v1/journal/trades/{id}/notes` | PATCH | Update trade notes |
| `/api/v1/klines` | GET | OHLCV candles for regime detection |
| `/api/v1/tickers` | GET | Current prices |
| `wss://testudo.vip/ws` | WS | Subscribe to `agent.execution.{id}`, `agent.alert.{id}` |
| `/api/v1/exchanges/accounts` | GET/POST | Manage exchange accounts |
| `/api/v1/risk-config` | GET/PUT | Read/update risk parameters |

### SignalInput Reference

```json
{
  "symbol": "ETH_USDT",        // required — trading pair
  "side": "LONG",              // required — LONG or SHORT (uppercase)
  "entry_price": 3100.00,      // required — limit price
  "stop_loss": 3050.00,        // optional — omit or null if none
  "take_profit": [],           // required — can be empty array []
  "execution_mode": "SHADOW",  // required — SHADOW or LIVE (uppercase)
  "reasoning": "...",          // optional — stored in journal
  "confidence": 0.72,          // optional — 0.0–1.0, used by Kelly engine
  "source": "agent:hermes_v2", // optional — agent identifier for attribution
  "leverage": 1,               // optional — 1–20 (default exchange-dependent)
  "idempotency_key": "uuid",   // optional — prevents double execution
  "management": {              // optional — trade management config
    "trailing_stop": {
      "enabled": true,
      "distance_percent": 50
    }
  }
}
```

### Journal Summary (LLM Format) Example

The journal summary endpoint (`GET /journal/agent/summary?format=llm`) returns
markdown that you inject directly into your reasoning context. This is your
memory across sessions:

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
| mean_reversion | 34 | 55.9% | 1.8 | +$2,850 |
| trend_follow | 22 | 40.9% | 0.9 | -$920 |

### Actionable Insights
- **Strongest setup**: breakout shows 60.7% win rate with 2.10 avg R
- **Underperforming**: trend_follow has 40.9% win rate — consider reducing
```

---

## Appendix A: Precomputed Regime Centroids (Template)

The LLM should maintain a local copy of these centroids, updated weekly from
Testudo's journal. This is the "offline verification" step from the
architecture: periodically recompute centroids from historical data and store
them as constants.

```python
# Updated weekly from journal data
# Each centroid = empirical distribution of 4h returns, sorted

REGIME_CENTROIDS = {
    "R0_low_vol_mean_reverting": {
        "description": "Low volatility, oscillating. ρ₁ < -0.1.",
        "samples": [...],  # sorted list of 180 4h returns from a representative 30d window
        "vol_percentile": 25,
        "autocorr_lag1": -0.15,
    },
    "R1_trending_momentum": {
        "description": "Above-average volatility, persistent direction. ρ₁ > 0.1.",
        "samples": [...],
        "vol_percentile": 65,
        "autocorr_lag1": 0.18,
    },
    "R2_high_vol_choppy": {
        "description": "High volatility, no autocorrelation. No trade edge.",
        "samples": [...],
        "vol_percentile": 85,
        "autocorr_lag1": -0.02,
    },
    "R3_extreme_black_swan": {
        "description": "Extreme volatility. Halt all trading.",
        "samples": [...],
        "vol_percentile": 98,
        "autocorr_lag1": None,
    },
}
```

---

## Appendix B: Strategy Parameter Defaults

These are starting points. Refine based on journal performance data.

| Parameter | MeanReversion | MomentumBreakout | FundingArbitrage | DeltaNeutralHedge |
|-----------|---------------|------------------|------------------|-------------------|
| Lookback (candles) | 20 | 20 | — | — |
| Entry threshold (σ) | 2.0 | — | — | — |
| Entry volume ratio | — | 1.5× avg | — | — |
| Stop loss (ATR) | 2.0× | 2.0× | None | None |
| Take profit | Mean | 3.0× ATR or trail | Funding rate < 0.003% | — |
| Trailing stop | — | 40% of move | — | — |
| Max leverage | 1× | 2× | 1× | 1× |
| Confidence | κ-based (0.50–0.80) | Breakout clarity (0.60–0.75) | Funding rate (0.70–0.90) | Always 0.95 |
| Regime required | R₀ | R₁ | Any | Any |
| Re-entry cooldown | 4 candles | 8 candles | 1 candle | N/A |

---

## Appendix C: When to Ignore This Document

These proofs assume:
1. **Stationary distributions** — market regimes are sufficiently stable over
   the holding period.
2. **Independent trials** — trades are sequential and not overlapping.
3. **No adverse selection** — your orders don't move the market.

When any of these assumptions fail (e.g., during a flash crash, an exchange
outage, or a regulatory announcement), fall back to HaltExecution. The proofs
are your compass in normal markets, not a life jacket in a storm.

---

*This document is the canonical strategy reference for Testudo autonomous
agents. The Lean 4 proofs in §6 guarantee mathematical correctness. The
pseudocode in §5 implements them on Testudo's API. The regime detection in
§1 tells you which strategy to use when. Everything else is testudo's job.*
