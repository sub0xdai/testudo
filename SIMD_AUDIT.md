# SIMD Audit — Testudo Exchange

**Scope:** SIMD-enhanced utilities that add trading-platform value (tick-to-candle, technical indicators, Monte Carlo risk, cross-exchange parsing).

**Date:** 2026-05-12

---

## Candidate A: `simd-ohlcv` — Tick-to-Candle Engine

**The gap:** Testudo ingests raw ticks from CEX sidecar WebSockets and Hyperliquid order updates, but has **no real-time candle aggregation**. The journal dashboard shows equity curves but no price-action overlay. The Dignitas pipeline has no market-regime context.

**What it does:** Takes `[(f64, f64, f64)]` (timestamp, price, volume) tick arrays and buckets into OHLCV candles using `f64x4` SIMD.

| Lane | Layout |
|------|--------|
| `f64x4` lane 0 | Open (first price in bucket) |
| `f64x4` lane 1 | High (max price) |
| `f64x4` lane 2 | Low (min price) |
| `f64x4` lane 3 | Close (last price) |

With `f64x4::max`/`min` you update OHLC per tick in a **single SIMD compare-blend**. 4 candles at once = massive throughput for multi-symbol aggregation.

**User value:**
- Real-time price overlays on the equity curve (journal dashboard)
- Volatility-adjusted position sizing via ATR computed from candles
- Market-regime classification (trending/ranging/volatile) feeds Dignitas scoring

---

## Candidate B: `simd-indicators` — Vectorized Technical Analysis

**The gap:** The journal service computes cumulative P&L and return buckets but has **zero technical indicators** — no SMA, EMA, RSI, Bollinger Bands, MACD, or VWAP anywhere.

**What it does:** SIMD-accelerated rolling-window indicators:

| Indicator | SIMD Strategy | Lane Width |
|-----------|--------------|------------|
| **SMA** | Prefix-sum + sliding window subtraction | `f64x4` |
| **EMA** | Recursive (not vectorizable), but batch-compute across N symbols with lane-per-symbol | `f64x4` (4 symbols) |
| **RSI** | SIMD average gain/loss over rolling windows | `f64x4` |
| **Bollinger Bands** | SMA + SIMD variance (sum of squared deviations) | `f64x4` |
| **VWAP** | Cumulative (price × volume) / cumulative volume per candle | `f64x4` |
| **ATR** | True range = `max(high-low, |high-prev_close|, |low-prev_close|)` — a single `f64x3` → `f64` reduction | `f64x4` |

**Key trick:** Process **4 symbols** in parallel by loading `[prices_sym0, prices_sym1, prices_sym2, prices_sym3]` into one `f64x4`, computing indicators lane-wise. For a user monitoring 20 pairs, that's 5× throughput.

**User value:**
- "Signal scanner" feature: scan user's watchlist for EMA crossovers, RSI divergences, Bollinger squeeze setups
- Dignitas pipeline gets technical context (is the market trending or choppy?)
- Entry/exit timing quality scoring: "did you buy into RSI > 70?"

---

## Candidate C: `simd-risk-mc` — Monte Carlo VaR Engine

**The gap:** `risk_snapshot.rs` computes a static point-in-time leverage/exposure/delta snapshot. There's **no forward-looking risk** — no VaR, no scenario analysis, no stress testing.

**What it does:** Given a user's current positions vector (symbols, quantities, mark prices), runs N Monte Carlo simulations of correlated price moves using SIMD:

```
for each simulation path:
    correlated_returns = Cholesky(L) × random_normals   // f64x4 mat-vec
    new_prices = mark_prices * (1 + correlated_returns) // f64x4 element-wise
    new_pnl = Σ quantity × (new_price - mark_price)     // f64x4 dot product
```

With `f64x4`, 4 positions per SIMD lane. For 20 positions × 10,000 simulations, that's 200K dot products — SIMD makes this a ~50µs operation instead of milliseconds.

**User value:**
- "What-if" widget: "If BTC drops 10%, ETH drops 15%, SOL drops 20% — what's my P&L?"
- VaR display in the risk strip: "95% VaR: −$2,340 over 24h"
- Dignitas gets a "risk-adjusted expectancy" score component

---

## Candidate D: `simd-tick-normalizer` — Cross-Exchange Tick Parser

**The gap:** `cex_history.rs` has per-exchange REST fetchers (Binance, Bybit, OKX, etc.) each with their own JSON shape. WS fills in `ws_fills.rs` parse Hyperliquid's custom format. These are ad-hoc, single-tick-at-a-time parsers.

**What it does:** Bulk-parse tick arrays using SIMD-accelerated JSON scanning:

1. Load raw bytes into `Simd<u8, 32>` lanes
2. Scan for `"price"`, `"qty"`, `"side"` delimiters via byte-match (like `simd_json`)
3. Parse numeric values into `f64` arrays in bulk
4. Output uniform `[[timestamp, price, volume, side_flag]; N]` arrays

**Lane width:** `u8x32` or `u8x64` for the byte-scanning phase, then `f64x4` for bulk number parsing.

**User value:**
- Faster history import (bulk-parsing 1000 trades at once instead of one-at-a-time)
- Unified tick feed for indicator computation regardless of exchange origin
- Enables real-time multi-exchange order book comparison

---

## Recommendation

**Build A + B as a single crate** `testudo-simd-market-data` with this structure:

```
crates/simd-market-data/
├── Cargo.toml              # depends on std::simd (nightly) or wide crate
├── src/
│   ├── lib.rs
│   ├── tick.rs             # Tick → f64x4 batch layout
│   ├── ohlcv.rs            # Candle aggregation engine
│   ├── indicators/
│   │   ├── sma.rs          # f64x4 prefix-sum SMA
│   │   ├── ema.rs          # Lane-parallel EMA (4 symbols)
│   │   ├── rsi.rs          # SIMD gain/loss average
│   │   ├── bollinger.rs    # SMA + SIMD variance
│   │   ├── atr.rs          # f64x4 true-range reduction
│   │   └── vwap.rs         # Cumulative price×volume SIMD
│   ├── scanner.rs          # Multi-symbol crossover/divergence scanner
│   └── regime.rs           # Market regime classifier (trending/ranging/volatile)
└── benches/
    └── throughput.rs       # Criterion benchmarks: ticks/sec, candles/sec
```

**Integration points into Testudo:**

| Where | What changes |
|-------|-------------|
| `journal_timeseries` | Overlay SMA/Bollinger on equity curve endpoint |
| `dignitas/inputs.rs` | Add "market_regime" input (0–1 score) |
| `risk_snapshot.rs` | Feed ATR for volatility-adjusted position sizing suggestion |
| New route: `GET /api/v1/market/scanner` | Watchlist crossover alerts |
| Frontend journal dashboard | Price chart overlays with indicators |

**Note on existing code:** The current codebase is not SIMD-friendly by design, and that's a **correct architectural choice** — `Decimal` is used for monetary precision, datasets are small per-user, and heavy compute is pushed to PostgreSQL. These proposed crates are **new SIMD-native additions**, not rewrites of existing code.
