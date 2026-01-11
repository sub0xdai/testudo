# Testudo Hybrid Trading System - AI Engineer Handoff

## Quick Start

```bash
# Backend
cd /home/m0xu/1-projects/testudo/testudo-exchange
cargo run --bin router

# Frontend (separate terminal)
cd /home/m0xu/1-projects/testudo/testudo-web/apps/web
bun run dev
```

**Navigate to**: http://localhost:5173/trade/SOLUSDT

---

## Project Overview

**Testudo** is a perpetual futures trading system that:
- Displays **live market data from Binance Futures API** (539 USDT perp pairs)
- Executes orders through a **Shadow + Live** execution model
- Provides **automated risk management** with "Conservative Wins" policy
- Supports **paper trading** (Shadow mode) before live execution

### Architecture
```
testudo/
├── testudo-exchange/    # Rust backend
│   └── crates/
│       ├── common_utils/  # Services, adapters, risk engine
│       ├── engine/        # Order matching + Shadow engine
│       ├── router/        # API gateway
│       └── ...
├── testudo-web/         # React/TypeScript frontend
└── testudo-ops/         # Kubernetes infrastructure
```

---

## Current State (2026-01-11)

### Completed Phases

| Phase | Description | Status |
|-------|-------------|--------|
| **A** | Market Data Pipeline (Binance → API) | ✅ Complete |
| **B** | Shadow Engine (Paper Trading) | ✅ Complete |
| **C** | Risk Engine (Position Sizing) | ✅ Complete |
| **D** | Trade Management (SL/TP, Break-even) | ✅ Complete |
| **E.1** | API Key Storage (encrypted) | ✅ Complete |
| **E.2** | Decision Loop (Shadow → Live flow) | ✅ Complete |
| **E.3** | Binance Order Execution | ✅ Complete |
| **E.4** | Position Sync (Shadow ↔ Binance) | ✅ Complete |
| **E.5** | Mode Toggle UI (Shadow/Live) | ✅ Complete |
| **F** | Binance Futures Migration | ✅ Complete |

### Recent Changes (2026-01-11)

**Implemented Binance WebSocket Streaming**:
- Frontend connects directly to `wss://fstream.binance.com`
- Real-time orderbook updates every 100ms (`@depth@100ms` stream)
- Real-time trade streaming (`@aggTrade` stream)
- Real-time price updates (`@bookTicker` stream)
- Removed internal ws-stream dependency for market data
- Auto-reconnect with exponential backoff

**Switched from Binance Spot to Binance Futures (Perpetuals)**:
- Backend now uses `fapi.binance.com` instead of `api.binance.com`
- All endpoints changed from `/api/v3/*` to `/fapi/v1/*`
- Markets filtered by `contractType=PERPETUAL`
- Frontend MarketSelector shows 539 USDT perpetual pairs
- Symbol format: `SOLUSDT`, `BTCUSDT` (native Binance format)

---

## Key Files

### Backend (testudo-exchange)
```
crates/common_utils/src/
├── services/
│   └── binance_data.rs    # Binance Futures API (fapi.binance.com)
├── adapters/
│   ├── binance_executor.rs  # Live order execution
│   ├── position_sync.rs     # Shadow ↔ Binance sync
│   └── ccxt_auth.rs         # API key authentication
├── risk/
│   ├── position_sizer.rs    # "Conservative Wins" sizing
│   └── validator.rs         # Pre-trade validation

crates/router/src/
├── decision_loop.rs         # Shadow → Live execution flow
└── routes/
    ├── market_data.rs       # /api/v1/market-data/*
    └── trade_management.rs  # /api/v1/trades/*
```

### Frontend (testudo-web)
```
apps/web/src/
├── App.tsx                  # Routes, default market: SOLUSDT
├── pages/Trade.tsx          # Main trading page
├── components/
│   ├── Depth.tsx            # Orderbook + trades (WebSocket)
│   ├── MarketBar.tsx        # Price, stats display (WebSocket)
│   ├── MarketSelector.tsx   # Fuzzy search 539 markets
│   └── ui/ModeToggle.tsx    # Shadow/Live mode switch
└── utils/
    ├── binance_ws.ts        # Binance WebSocket manager (real-time data)
    ├── requests.ts          # API calls
    └── format.ts            # parseMarketSymbol() for USDT pairs
```

---

## API Endpoints

### Market Data
```
GET /api/v1/market-data/ticker?symbol=SOLUSDT
GET /api/v1/market-data/orderbook?symbol=SOLUSDT&limit=20
GET /api/v1/market-data/klines?symbol=SOLUSDT&interval=1h&limit=100
GET /api/v1/market-data/markets   # Returns 539 USDT perps
```

### Order Execution
```
POST /api/v1/order
{
  "market": "SOLUSDT",
  "side": "buy",
  "quantity": "0.1",
  "price": "140.00",
  "user_id": "...",
  "execution_mode": "shadow" | "live"
}
```

### Trade Management
```
POST   /api/v1/trades              # Create trade with SL/TP
GET    /api/v1/trades              # List active trades
PUT    /api/v1/trades/{id}/sl      # Update stop loss
PUT    /api/v1/trades/{id}/tp      # Update take profit
DELETE /api/v1/trades/{id}         # Cancel trade group
```

---

## Execution Modes

| Mode | Description |
|------|-------------|
| **Shadow** | Paper trading, no real orders. Default mode. |
| **Live** | Real orders sent to Binance Futures. Requires API keys. |

The frontend ModeToggle component switches between modes. Live mode shows red indicator, Shadow shows green.

---

## Development Commands

### Backend
```bash
cd testudo-exchange
cargo build --bin router    # Build
cargo run --bin router      # Run API server (port 8080)
cargo test                  # Run tests
cargo clippy                # Lint
```

### Frontend
```bash
cd testudo-web/apps/web
bun install                 # Install deps
bun run dev                 # Dev server (port 5173)
bun run build               # Production build
bun run lint                # ESLint
```

---

## Known Issues / Warnings

1. **Rust 2024 compatibility warning** in `cache.rs` - needs type annotations for `!` fallback
2. **Unused imports** in several files - cosmetic warnings only
3. **Landing app lint fails** - missing vite dependency (not related to main app)

---

## Next Steps (Potential)

1. **WebSocket real-time data** - Replace polling with Binance WS streams
2. **Position display** - Show open positions with P&L
3. **Order history** - Display filled orders
4. **Leverage settings** - Allow configuring margin/leverage
5. **Multi-account** - Support multiple Binance API keys

---

## References

- **PRD**: `hybrid_trading.json`
- **Implementation Plan**: `testudo-web/docs/plans/2026-01-11-perps-charts-implementation.json`
- **Phase E Plans**: `docs/plans/e*.md`
