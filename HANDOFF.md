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

## Current State (2026-01-12)

### Completed Phases

| Phase | Description | Status |
|-------|-------------|--------|
| **A** | Market Data Pipeline (Binance → API) | Completed |
| **B** | Shadow Engine (Paper Trading) | Completed |
| **C** | Risk Engine (Position Sizing) | Completed |
| **D** | Trade Management (SL/TP, Break-even) | Completed |
| **E.1** | API Key Storage (encrypted) | Completed |
| **E.2** | Decision Loop (Shadow → Live flow) | Completed |
| **E.3** | Binance Order Execution | Completed |
| **E.4** | Position Sync (Shadow ↔ Binance) | Completed |
| **E.5** | Mode Toggle UI (Shadow/Live) | Completed |
| **F** | Binance Futures Migration | Completed |
| **RISK** | User Risk Config + Position Calculator | Completed |
| **DRAW** | Drawable Position Tool | Completed |

### Recent Changes (2026-01-12)

**Implemented Drawable Position Tool (DRAW-01 to DRAW-09)**:

Frontend:
- `src/utils/chart_manager.ts` - Added coordinate conversion and price line methods
  - `coordinateToPrice()` / `priceToCoordinate()` for chart interaction
  - `createPriceLine()` / `removePriceLine()` for visual entry/SL/TP lines
  - `subscribeClick()` / `subscribeCrosshairMove()` for mouse events
- `src/components/chart/PositionDrawingTool.tsx` - Drawing state machine
  - States: `idle → drawing_entry → drawing_sl → drawing_tp → complete`
  - Keyboard shortcuts: Escape (cancel), Enter (execute)
  - Order execution via `createOrder()` on completion
- `src/components/chart/PositionZoneOverlay.tsx` - Visual overlay
  - Green profit zone, red loss zone
  - Draggable handles for adjusting SL/TP
  - Position sizing display and execute button
- `src/components/trade_interface/TradeView.tsx` - Toolbar integration
  - "POSITION" button in chart header with icon
  - Toggle activates/deactivates drawing mode
- `src/components/RiskAutomaton.tsx` - Converted to config-only panel
  - Shows risk %, max position size, require SL toggle
  - Entry/SL/TP inputs removed (now handled by Position Tool)

**UX Polish Applied**:

HUD (Control Panel):
- Green/red color bar on left edge indicates Long/Short direction
- Size truncated to 2 decimals (was 4)
- Visual dividers between Size | Risk | R:R sections
- Execute button has dark text on bright background (WCAG AA contrast)

Price Labels:
- TP: Dark green text (#052e16) on green background
- SL: Dark red text (#450a0a) on red background
- Entry: Black text on white background with padding

Interactions:
- CANCEL styled as ghost button with border and hover state
- Lines thicken from 2px → 3px on hover
- Handles glow and scale on hover/drag
- Cursor shows `ns-resize` on handles

**Previously completed - User Risk Configuration System (RISK-01 to RISK-15)**:

Backend:
- `common_utils/src/risk/storage.rs` - RiskConfig storage with Redis
- `router/src/routes/risk_config.rs` - GET/PUT `/api/v1/risk-config` endpoints
- `common_utils/src/adapters/account_state.rs` - Shadow/Live balance adapter
- Modified `order.rs` to load user's risk config per user_id

Frontend:
- `src/pages/RiskSettings.tsx` - Settings page for risk configuration
- `src/hooks/useRiskCalculation.ts` - Position sizing hook
- `src/components/RiskDisplay.tsx` - Risk metrics display

---

## All Phases Complete

All planned phases have been implemented:

| Phase | Description | Status |
|-------|-------------|--------|
| **A** | Market Data Pipeline (Binance → API) | Completed |
| **B** | Shadow Engine (Paper Trading) | Completed |
| **C** | Risk Engine (Position Sizing) | Completed |
| **D** | Trade Management (SL/TP, Break-even) | Completed |
| **E.1-E.5** | Live Execution Flow | Completed |
| **F** | Binance Futures Migration | Completed |
| **RISK** | User Risk Config | Completed |
| **DRAW** | Drawable Position Tool | Completed |

### How to Use Position Tool

1. Click the position tool button (crosshair icon) in chart toolbar
2. Click on chart to set entry price
3. Click again to set stop loss (red zone appears)
4. Click again to set take profit (green zone appears)
5. Adjust levels by dragging handles if needed
6. Click Execute or press Enter to place order
7. Press Escape to cancel at any time

### Reference

- **Dataflow Diagram**: `testudo-web/apps/web/docs/diagrams/position-tool-dataflow.md`
- **PRD Tasks**: `.ralph/prd.json` (all DRAW-01 to DRAW-10 completed)

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
│   ├── account_state.rs     # Balance adapter (Shadow/Live)
│   └── ccxt_auth.rs         # API key authentication
├── risk/
│   ├── position_sizer.rs    # "Conservative Wins" sizing
│   ├── validator.rs         # Pre-trade validation
│   └── storage.rs           # RiskConfig Redis storage

crates/router/src/
├── decision_loop.rs         # Shadow → Live execution flow
└── routes/
    ├── market_data.rs       # /api/v1/market-data/*
    ├── trade_management.rs  # /api/v1/trades/*
    ├── risk_config.rs       # /api/v1/risk-config
    └── order.rs             # /api/v1/order (uses risk config)
```

### Frontend (testudo-web)
```
apps/web/src/
├── App.tsx                  # Routes, default market: SOLUSDT
├── pages/
│   ├── Trade.tsx            # Main trading page
│   └── RiskSettings.tsx     # Risk configuration page
├── components/
│   ├── Depth.tsx            # Orderbook + trades (WebSocket)
│   ├── MarketBar.tsx        # Price, stats display (WebSocket)
│   ├── MarketSelector.tsx   # Fuzzy search 539 markets
│   ├── RiskAutomaton.tsx    # Position calculator
│   ├── RiskDisplay.tsx      # Risk metrics display
│   └── ui/ModeToggle.tsx    # Shadow/Live mode switch
├── hooks/
│   └── useRiskCalculation.ts # Position sizing logic
└── utils/
    ├── chart_manager.ts     # Lightweight-charts wrapper
    ├── binance_ws.ts        # Binance WebSocket manager
    ├── requests.ts          # API calls (incl. risk config)
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

### Risk Configuration
```
GET /api/v1/risk-config
PUT /api/v1/risk-config
{
  "account_risk_percent": "2",
  "max_risk_amount": null,
  "max_position_size": null,
  "max_leverage": 1,
  "daily_max_drawdown_percent": "5",
  "max_open_positions": 5,
  "require_stop_loss": true,
  "default_stop_atr_multiplier": "2",
  "min_risk_reward_ratio": "1.5"
}
```

### Order Execution
```
POST /api/v1/order
{
  "market": "SOLUSDT",
  "side": "buy",
  "quantity": 0.1,
  "price": 140.00,
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

## References

- **PRD**: `.ralph/prd.json` (RISK-01 to RISK-15 completed, DRAW-01 to DRAW-10 pending)
- **Dataflow Diagram**: `testudo-web/apps/web/docs/diagrams/position-tool-dataflow.md`
- **Plan File**: `.claude/plans/lexical-wandering-firefly.md`
- **Phase E Plans**: `docs/plans/e*.md`
