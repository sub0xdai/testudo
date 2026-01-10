# Testudo Hybrid Trading System - Implementation Plan

## Overview

Transform Testudo from an isolated demo exchange into a **hybrid trading system** that:
- Displays **live market data from Binance**
- Executes orders through a **Shadow + External** model (simulate internally, execute on Binance)
- Provides **automated risk management** and position sizing
- Supports **advanced trade management** (SL/TP, break-even, multi-target exits)

## User Flow

1. **Demo Mode (No API Keys)**: User explores with paper trading against real Binance prices
2. **Live Mode (API Keys Connected)**: Orders validated by risk engine, then executed on Binance

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         FRONTEND (React)                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
│  │ Market Data │  │  Order Form │  │ Risk Config │  │ Positions  │ │
│  │  (Binance)  │  │  + Targets  │  │   Panel     │  │   Panel    │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────┬──────┘ │
└─────────┼────────────────┼────────────────┼───────────────┼────────┘
          │                │                │               │
          ▼                ▼                ▼               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      API GATEWAY (Router)                           │
├─────────────────────────────────────────────────────────────────────┤
│  /market-data/*  │  /order  │  /risk-config  │  /positions         │
│   (proxy to      │  (shadow │  (user params) │  (aggregated)       │
│    Binance)      │   loop)  │                │                     │
└─────────┬────────┴────┬─────┴───────┬────────┴──────────┬──────────┘
          │             │             │                   │
          ▼             ▼             ▼                   ▼
┌──────────────┐  ┌───────────┐  ┌──────────┐  ┌─────────────────────┐
│ CCXT Adapter │  │  SHADOW   │  │   RISK   │  │  POSITION TRACKER   │
│  (Binance)   │  │  ENGINE   │  │  ENGINE  │  │ (internal + Binance)│
│              │  │           │  │          │  │                     │
│ • Tickers    │  │ • Simulate│  │ • Size   │  │ • Shadow positions  │
│ • Orderbook  │  │ • Validate│  │ • Limits │  │ • Real positions    │
│ • Klines     │  │ • Track   │  │ • ATR    │  │ • P&L calculation   │
└──────┬───────┘  └─────┬─────┘  └────┬─────┘  └──────────┬──────────┘
       │                │             │                   │
       │                ▼             │                   │
       │         ┌────────────┐       │                   │
       │         │  DECISION  │◄──────┘                   │
       │         │   LOOP     │                           │
       │         │            │                           │
       │         │ Approved?  │───Yes──► CCXT Execute ────┤
       │         │   │        │         (real Binance)    │
       │         │   No       │                           │
       │         │   ▼        │                           │
       │         │ Reject     │                           │
       │         └────────────┘                           │
       │                                                  │
       └──────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase A: Market Data Pipeline (Display Layer)
**Goal**: Live Binance data showing in the UI

| # | Task | File(s) |
|---|------|---------|
| 1 | Binance Data Service | `crates/common_utils/src/services/binance_data.rs` (NEW) |
| 2 | Market Data API Routes | `crates/router/src/routes/market_data.rs` (NEW) |
| 3 | Redis Caching Layer | `crates/common_utils/src/services/cache.rs` (NEW) |
| 4 | Frontend API Updates | `testudo-web/apps/web/src/utils/requests.ts` |
| 5 | Market Selector Fix | `testudo-web/apps/web/src/components/MarketSelector.tsx` |
| 6 | Disable Order Execution | `testudo-web/apps/web/src/components/ExecuteOrder.tsx` |

**Result**: Live charts, orderbook, tickers from Binance. Trading shows "Demo mode" message.

---

### Phase B: Demo Trading (Paper Mode)
**Goal**: Internal simulation with paper P&L

| # | Task | File(s) |
|---|------|---------|
| 7 | Shadow Engine | `crates/engine/src/shadow/mod.rs` (NEW) |
| 8 | Paper Balance System | `crates/engine/src/shadow/balances.rs` (NEW) |
| 9 | Order Simulation | `crates/engine/src/shadow/orders.rs` (NEW) |
| 10 | Position Tracker | `crates/engine/src/shadow/positions.rs` (NEW) |
| 11 | Paper Trading API | `crates/router/src/routes/paper_trade.rs` (NEW) |
| 12 | Frontend Paper Mode | `testudo-web/apps/web/src/components/ExecuteOrder.tsx` |

**Result**: Users can paper trade, see simulated P&L against real market prices.

---

### Phase C: Risk Engine
**Goal**: Automated position sizing and validation

| # | Task | File(s) |
|---|------|---------|
| 13 | Risk Configuration Model | `crates/common_utils/src/models/risk_config.rs` (NEW) |
| 14 | ATR Calculator | `crates/common_utils/src/risk/atr.rs` (NEW) |
| 15 | Position Sizer | `crates/common_utils/src/risk/position_sizer.rs` (NEW) |
| 16 | Risk Validation | `crates/common_utils/src/risk/validator.rs` (NEW) |
| 17 | Risk Config API | `crates/router/src/routes/risk_config.rs` (NEW) |
| 18 | Risk Config UI | `testudo-web/apps/web/src/components/RiskConfig.tsx` (NEW) |

**Position Sizing Inputs** (most conservative wins):
- Account % risk (e.g., 2% per trade)
- ATR-based volatility adjustment
- User-defined max position size
- User-defined stop-loss distance

**Result**: Order form auto-calculates position size based on risk params.

---

### Phase D: Trade Management
**Goal**: SL/TP, break-even, multi-target exits

| # | Task | File(s) |
|---|------|---------|
| 19 | Order Groups Model | `crates/common_utils/src/models/order_group.rs` (NEW) |
| 20 | SL/TP Linking | `crates/engine/src/shadow/sl_tp.rs` (NEW) |
| 21 | Break-even Automation | `crates/engine/src/shadow/breakeven.rs` (NEW) |
| 22 | Multi-target Exit Logic | `crates/engine/src/shadow/multi_target.rs` (NEW) |
| 23 | Trade Management API | `crates/router/src/routes/trade_management.rs` (NEW) |
| 24 | Order Form Enhancements | `testudo-web/apps/web/src/components/ExecuteOrder.tsx` |

**Trade Management Features**:
- **Stop-loss / Take-profit**: Automatic exit orders placed with entry
- **Break-even automation**: Move SL to entry at X% profit
- **Multi-target exits**: Exit 50% at T1, 25% at T2, let rest run

**Result**: Full trade management in paper mode.

---

### Phase E: Live Execution (Binance Integration)
**Goal**: Real money trading with full risk management

| # | Task | File(s) |
|---|------|---------|
| 25 | API Key Connection UI | `testudo-web/apps/web/src/components/ExchangeConnect.tsx` (NEW) |
| 26 | Decision Loop | `crates/engine/src/execution/decision_loop.rs` (NEW) |
| 27 | Binance Order Execution | `crates/common_utils/src/adapters/binance_executor.rs` (NEW) |
| 28 | Position Sync | `crates/engine/src/execution/position_sync.rs` (NEW) |
| 29 | Live Mode Toggle | `testudo-web/apps/web/src/components/ModeToggle.tsx` (NEW) |

**Decision Loop Flow**:
1. User submits order
2. Shadow engine simulates and tracks
3. Risk engine validates (size, limits, balance)
4. If approved → Execute on Binance via CCXT
5. If rejected → Return reason to user
6. Sync positions between internal tracker and Binance

**Result**: Full hybrid system - automated risk + real execution.

---

## Phase A Technical Detail

### New Backend Components

**1. Binance Data Service** (`crates/common_utils/src/services/binance_data.rs`)

```rust
pub struct BinanceDataService {
    ccxt_adapter: MarketDataLoader,
    cache: Arc<RedisCache>,
}

impl BinanceDataService {
    pub async fn get_ticker(&self, symbol: &str) -> Result<Ticker, Error>;
    pub async fn get_orderbook(&self, symbol: &str, limit: i32) -> Result<OrderBook, Error>;
    pub async fn get_klines(&self, symbol: &str, interval: &str, limit: i32) -> Result<Vec<Candle>, Error>;
    pub async fn get_supported_markets(&self) -> Result<Vec<Market>, Error>;
}
```

**2. New API Routes** (`crates/router/src/routes/market_data.rs`)

```
GET /api/v1/market-data/ticker?symbol=BTC_USDC
GET /api/v1/market-data/orderbook?symbol=BTC_USDC&limit=20
GET /api/v1/market-data/klines?symbol=BTC_USDC&interval=1h&limit=100
GET /api/v1/market-data/markets
```

**3. Cache Strategy**

| Data Type | Redis Key Pattern | TTL |
|-----------|-------------------|-----|
| Ticker | `binance:ticker:{symbol}` | 5 seconds |
| Orderbook | `binance:orderbook:{symbol}` | 1 second |
| Klines | `binance:klines:{symbol}:{interval}` | 60 seconds |
| Markets | `binance:markets` | 5 minutes |

### Frontend Changes

**4. Update API endpoints** (`testudo-web/apps/web/src/utils/requests.ts`)
- Point ticker/depth/klines calls to new `/market-data/*` routes
- Remove hardcoded fallback markets
- Fetch real market list from `/market-data/markets`

**5. Update Market Selector** (`testudo-web/apps/web/src/components/MarketSelector.tsx`)
- Fetch available markets from API (BTC_USDC, ETH_USDC, SOL_USDC)
- Remove hardcoded fallbacks in 3 locations

**6. Symbol Translation**

| Frontend Format | Binance Format |
|-----------------|----------------|
| BTC_USDC | BTCUSDT |
| ETH_USDC | ETHUSDT |
| SOL_USDC | SOLUSDT |

---

## Verification

### Phase A Verification
1. Start services: `./scripts/start-exchange.sh --background`
2. Open http://localhost:5173
3. Verify:
   - [ ] Chart shows live Binance candles
   - [ ] Orderbook shows live bids/asks
   - [ ] Ticker shows price, 24h change, volume
   - [ ] Market selector shows BTC/ETH/SOL pairs
   - [ ] Order form shows "Demo mode" message
4. Test API directly:
   ```bash
   curl http://localhost:8080/api/v1/market-data/ticker?symbol=BTC_USDC
   curl http://localhost:8080/api/v1/market-data/orderbook?symbol=BTC_USDC
   curl http://localhost:8080/api/v1/market-data/klines?symbol=BTC_USDC&interval=1h&limit=10
   ```

### Full System Verification (After Phase E)
1. Connect Binance API keys (testnet recommended)
2. Configure risk params (2% risk, 100 USDC max position)
3. Place a limit buy order for BTC
4. Verify:
   - [ ] Position size auto-calculated by risk engine
   - [ ] SL/TP orders created with entry
   - [ ] Order appears on Binance
   - [ ] Position tracked internally
   - [ ] Break-even automation triggers at profit threshold

---

## Files to Modify (Summary)

### Backend (testudo-exchange)
```
crates/common_utils/src/
├── services/
│   ├── mod.rs (NEW)
│   ├── binance_data.rs (NEW)
│   └── cache.rs (NEW)
├── risk/
│   ├── mod.rs (NEW)
│   ├── atr.rs (NEW)
│   ├── position_sizer.rs (NEW)
│   └── validator.rs (NEW)
└── models/
    ├── risk_config.rs (NEW)
    └── order_group.rs (NEW)

crates/router/src/routes/
├── mod.rs (MODIFY - add new routes)
├── market_data.rs (NEW)
├── paper_trade.rs (NEW)
├── risk_config.rs (NEW)
└── trade_management.rs (NEW)

crates/engine/src/
├── shadow/
│   ├── mod.rs (NEW)
│   ├── balances.rs (NEW)
│   ├── orders.rs (NEW)
│   ├── positions.rs (NEW)
│   ├── sl_tp.rs (NEW)
│   ├── breakeven.rs (NEW)
│   └── multi_target.rs (NEW)
└── execution/
    ├── mod.rs (NEW)
    ├── decision_loop.rs (NEW)
    └── position_sync.rs (NEW)
```

### Frontend (testudo-web)
```
apps/web/src/
├── utils/
│   └── requests.ts (MODIFY)
├── components/
│   ├── MarketSelector.tsx (MODIFY)
│   ├── ExecuteOrder.tsx (MODIFY)
│   ├── RiskConfig.tsx (NEW)
│   ├── ExchangeConnect.tsx (NEW)
│   └── ModeToggle.tsx (NEW)
└── pages/
    └── Trade.tsx (MODIFY)
```

---

## Implementation Progress

### Phase A: Market Data Pipeline ✅ COMPLETE
**Completed: 2026-01-10**

| # | Task | Status | File(s) |
|---|------|--------|---------|
| 1 | Binance Data Service | ✅ Done | `crates/common_utils/src/services/binance_data.rs` |
| 2 | Market Data API Routes | ✅ Done | `crates/router/src/routes/market_data.rs` |
| 3 | Redis Caching Layer | ✅ Done | `crates/common_utils/src/services/cache.rs` |

**New API Endpoints:**
- `GET /api/v1/market-data/ticker?symbol=BTC_USDC`
- `GET /api/v1/market-data/orderbook?symbol=BTC_USDC&limit=20`
- `GET /api/v1/market-data/klines?symbol=BTC_USDC&interval=1h&limit=100`
- `GET /api/v1/market-data/markets`

### Phase B: Demo Trading (Paper Mode) ✅ COMPLETE
**Completed: 2026-01-10**

| # | Task | Status | File(s) |
|---|------|--------|---------|
| 7 | Shadow Engine | ✅ Done | `crates/engine/src/shadow/mod.rs` |
| 8 | Paper Balance System | ✅ Done | `crates/engine/src/shadow/balances.rs` |
| 9 | Order Simulation | ✅ Done | `crates/engine/src/shadow/orders.rs` |
| 10 | Position Tracker | ✅ Done | `crates/engine/src/shadow/positions.rs` |

**Shadow Engine Features:**
- Virtual balance management (default 10,000 USDC per user)
- Order placement with balance validation and reservation
- Fill simulation based on live price conditions:
  - Buy Limit: Fills when `Low <= Limit Price`
  - Sell Limit: Fills when `High >= Limit Price`
  - Market orders: Fill immediately at best bid/ask
- Position tracking with unrealized P&L calculation
- Mark price updates from live data

**Test Coverage:** 25 unit tests passing

### Phase C: Risk Engine ✅ COMPLETE
**Completed: 2026-01-10**

| # | Task | Status | File(s) |
|---|------|--------|---------|
| 13 | Risk Configuration Model | ✅ Done | `crates/common_utils/src/risk/config.rs` |
| 15 | Position Sizer | ✅ Done | `crates/common_utils/src/risk/position_sizer.rs` |
| 16 | Risk Validation | ✅ Done | `crates/common_utils/src/risk/validator.rs` |

**Risk Engine Features:**
- **RiskConfig**: User-defined risk parameters (account %, max risk, max size, leverage, etc.)
- **PositionSizer**: "Conservative Wins" - calculates position size as minimum of:
  - Account % risk limit
  - Fixed risk amount limit
  - Maximum position size limit
- **RiskValidator**: Pre-trade validation with violations and warnings:
  - Stop-loss requirement
  - Position size limits
  - Leverage limits
  - Open position limits
  - Daily drawdown limits
  - Risk/reward ratio checks
  - Balance checks

**Presets:**
- `RiskConfig::conservative()` - 1% risk, $50 max, 3 positions, require SL
- `RiskConfig::aggressive()` - 5% risk, 10x leverage, optional SL
- `RiskConfig::default()` - 2% risk, balanced settings

**Test Coverage:** 26 unit tests passing

### Phase D: Trade Management 🔜 NEXT
- Order Groups (SL/TP linking)
- Break-even Automation
- Multi-target Exit Logic

### Phase E: Live Execution 📋 PLANNED
- API Key Connection
- Decision Loop
- Binance Order Execution
- Position Sync

---

## Current Issues (From Screenshot)

| Issue | Root Cause | Fixed In |
|-------|------------|----------|
| "NO CHART DATA AVAILABLE" | No klines data from internal DB | Phase A ✅ |
| "No asks" / "No bids" | Empty internal orderbook | Phase A ✅ |
| "Request failed with status code 404" | Wrong API endpoint | Phase A ✅ |
| "Network Error" | Auth/endpoint issues | Phase A ✅ |
| BTC/USDC showing but unsupported | Hardcoded fallback markets | Phase A ✅ |
