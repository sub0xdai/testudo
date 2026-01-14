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

## Current State (2026-01-14)

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
| **DRAW-UX** | TradingView-style Drag Interaction | Completed |
| **COLUMNAR** | Wire-Efficient SoA Data Format | Completed |
| **V5** | Native Canvas Position Tool (Hybrid) | Completed |
| **GEOM** | Time-Anchored Bounded Zones | **Completed** |

### Recent Changes (2026-01-14)

**Position Tool Event Handling Fix**:
- Fixed race condition where mouse events weren't firing on the position tool
- Root cause: event listeners attached before `drawingState` transitioned to 'ready'
- Solution: attach to `window` + add `drawingState` to effect dependencies
- See "Fixed Bug" section below for details

**GEOM Geometry Polish - Time-Anchored Bounded Zones (Complete)**:

Zones now match TradingView style with time-anchored bounds and draggable right edge:

```
┌──────────────────────────────────────────────────────────────────────┐
│                          │                               ┃           │
│  Candlesticks        ════╬═══════════════════════════════┃═══════════╡ TP (dashed green)
│     ████                 │▓▓▓▓▓▓ PROFIT ZONE ▓▓▓▓▓▓▓▓▓▓▓┃           │
│   ██████             ════╬═══════════════════════════════┃═══════════╡ Entry (dashed orange)
│                          │░░░ LOSS ZONE ░░░░░░░░░░░░░░░░░┃           │
│                      ════╬═══════════════════════════════┃═══════════╡ SL (dashed red)
└──────────────────────────────────────────────────────────────────────┘
                           ▲                               ▲
                      startTime                       endTime (draggable)
```

**Key changes:**
- GEOM-01: Added `startTime: Time` and `endTime?: Time` to PositionLevels
- GEOM-02: Renderer now uses `ITimeScaleApi` for X coordinate conversion
- GEOM-03: Zones bounded from `startTime` to `endTime` (or chart edge if not set)
- GEOM-04: All lines now dashed, 1px, entry color changed to orange (#f0b90b)
- GEOM-05: `coordinateToTime()` and `timeToCoordinate()` added to ChartManager
- GEOM-06: Primitive auto-gets timeScale from chart on attach
- GEOM-07: 22 unit tests pass (added startTime tests)

**Additional UX improvements:**
- Draggable right-edge handle for setting `endTime` (vertical bar with grip dots)
- Double-click right edge to clear `endTime` (extends to chart boundary)
- Stats panel redesigned: compact, subtle (small ▶ button instead of large LONG/SHORT)
- Entry handle now orange to match entry line

**Trade timeout feature ready:** `endTime` enables visual trade timeout where zone right edge = expiry time.

---

**V5 Native Canvas Position Tool - Complete (V5-16 to V5-21)**:

- V5-16: Deleted legacy `PositionZoneOverlay.tsx` (superseded by hybrid architecture)
- V5-17: Added canvas hit-testing (`hitTestZone()`, `isPointInZone()`) for zone click detection
- V5-18: Verified z-order implementation (`zOrder: "bottom"` renders zones behind candles)
- V5-19: Added price axis labels for Entry/SL/TP via `priceAxisViews()`
- V5-20: Added 20 unit tests for PositionZonePrimitive (state, lifecycle, calculations)
- V5-21: End-to-end testing verified - zones pan/zoom natively with chart

**V5 Import Fix**: Changed from named import to namespace import for V5 compatibility:
```typescript
// Correct V5 import pattern
import * as LightweightCharts from "lightweight-charts";
chart.addSeries(LightweightCharts.CandlestickSeries, options);
```

---

**V5 Native Canvas Position Tool - Hybrid Architecture Complete (V5-01 to V5-15)**:

Implemented hybrid canvas + DOM architecture for native-feel position zones that pan/zoom with the chart.

**Architecture**:
```
┌─────────────────────────────────────────────────────────┐
│  Canvas Layer (PositionZonePrimitive)                   │
│  - Profit zone (green rectangle)                        │
│  - Loss zone (red rectangle)                            │
│  - Entry/SL/TP price lines                              │
│  - Auto pan/zoom via priceToCoordinate() per frame      │
└─────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────┐
│  DOM Layer (PositionHandleOverlay)                      │
│  - Draggable handles for entry/SL/TP                    │
│  - Stats panel (qty, risk $, R:R, execute button)       │
│  - Re-positions on crosshair move                       │
└─────────────────────────────────────────────────────────┘
```

**New Files**:
- `src/primitives/PositionZonePrimitive.ts` - V5 series primitive with canvas rendering
- `src/components/chart/PositionHandleOverlay.tsx` - Lightweight DOM handles + stats

**Modified Files**:
- `src/utils/chart_manager.ts` - V5 API + primitive attach/detach methods
- `src/components/chart/PositionDrawingTool.tsx` - Refactored to hybrid architecture

**V5 API Migration**:
```typescript
// Old (v4)
const series = chart.addCandlestickSeries(options);

// New (v5)
import { CandlestickSeries } from 'lightweight-charts';
const series = chart.addSeries(CandlestickSeries, options);
```

**ChartManager API**:
```typescript
// Attach primitive and get reference
const primitive = chartManager.attachPositionPrimitive(style?);

// Update levels (triggers canvas repaint) - includes time anchoring
chartManager.updatePositionLevels({
  entry, stopLoss, takeProfit, side,
  startTime,    // Time anchor for zone left edge
  endTime?,     // Optional: zone right edge (defaults to chart boundary)
});

// Time coordinate conversion (GEOM phase)
const time = chartManager.coordinateToTime(x);  // X pixel → Time
const x = chartManager.timeToCoordinate(time);  // Time → X pixel

// Detach when done
chartManager.detachPositionPrimitive();
```

**Completed Tasks (V5-01 to V5-18)**:
- V5-01 to V5-04: Upgrade to lightweight-charts v5.1.0, migrate API
- V5-05 to V5-06: PositionZonePrimitive with canvas rendering
- V5-07 to V5-08: Price lines and updateLevels() with requestUpdate()
- V5-09: ChartManager attach/detach lifecycle
- V5-10: Pan/zoom verification (architectural)
- V5-11 to V5-12: PositionHandleOverlay with drag events
- V5-13 to V5-14: Handle sync and stats panel
- V5-15: PositionDrawingTool refactored to hybrid
- V5-16: Deleted legacy PositionZoneOverlay.tsx
- V5-17: Canvas hit-testing (hitTestZone, isPointInZone)
- V5-18: z-order verification (zones behind candles)

**V5 Phase Complete (V5-19 to V5-21)**:
- V5-19: Price axis labels for Entry/SL/TP
- V5-20: 20 unit tests for PositionZonePrimitive
- V5-21: E2E testing verified

**All V5 tasks completed**: Including V5-22 (performance profiling), V5-23/V5-24 (documentation).

---

**Columnar Data Format (Structure-of-Arrays) - Wire Efficiency Optimization**:

Implemented Structure-of-Arrays (SoA) pattern for ~25% smaller JSON payloads on orderbook data.

Backend (Rust):
- `common_utils/src/columnar/mod.rs` - NEW: `DepthColumnStore`, `ColumnarOrderBook` structs
- `router/src/routes/market_data.rs` - Added `get_orderbook_columnar()` for v2 endpoint
- `router/src/main.rs` - Registered `/api/v2/market-data/orderbook` route

Frontend (TypeScript):
- `src/utils/ColumnDataView.ts` - NEW: Type-safe `ColumnDataView<T>` and `RowView<T>` classes
- `src/utils/ColumnDataView.test.ts` - NEW: 60 comprehensive tests (TDD)
- `src/utils/requests.ts` - Added `getDepthColumnar()` API function

Key design decisions:
- **Custom lightweight ColumnStore** - Not IndexMap or Polars (data sizes ~60 rows too small for Polars overhead)
- **Index shifting NOT needed** - Trading data is keyed by price, not sequential indices
- **Nonce at top level** - Avoids duplication on bids/asks
- **Payload validation** - Fail-fast error handling for malformed data

Wire format:
```json
{
  "symbol": "SOLUSDT",
  "bids": { "columns": ["price", "quantity"], "data": [["180.50", "100.5"], ...] },
  "asks": { "columns": ["price", "quantity"], "data": [["180.75", "25.0"], ...] },
  "nonce": 12345
}
```

Usage:
```typescript
const response = await getDepthColumnar('SOLUSDT');
const bidsView = new ColumnDataView<DepthRow>(response.bids);
const totalBidSize = bidsView.sumColumn('quantity');
bidsView.map((row) => row.get('price'));
```

---

### Next Phase: V5 Hybrid Position Tool

**Problem**: Current DOM overlay doesn't anchor to chart - zones don't pan/zoom with price action.

**Solution**: Upgrade to lightweight-charts V5 and use Pane Primitives for native canvas rendering.

**Architecture (Hybrid)**:
```
┌─────────────────────────────────────────────────────────┐
│  Canvas Layer (V5 Pane Primitive)                       │
│  - Profit/Loss zones (filled rectangles)                │
│  - Entry/SL/TP price lines                              │
│  - Moves with chart pan/zoom automatically              │
└─────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────┐
│  DOM Layer (React components)                           │
│  - Drag handles (positioned via priceToCoordinate)      │
│  - Stats panel (qty, risk $, R:R, execute button)       │
│  - Re-positions on chart movement events                │
└─────────────────────────────────────────────────────────┘
```

**Key Tasks (24 total)**:

| Phase | Tasks | Description |
|-------|-------|-------------|
| 1. Upgrade | V5-01 to V5-04 | Upgrade to V5, migrate series API, verify chart works |
| 2. Primitive | V5-05 to V5-10 | Build `PositionZonePrimitive`, canvas rendering, verify pan/zoom |
| 3. Hybrid | V5-11 to V5-15 | DOM handles, drag events, stats panel, refactor tool |
| 4. Polish | V5-16 to V5-24 | Hit-testing, z-order, tests, docs |

**New Files**:
- `src/primitives/PositionZonePrimitive.ts` - Canvas primitive implementing `IPanePrimitive`
- `src/components/chart/PositionHandleOverlay.tsx` - Lightweight DOM for handles only
- `src/components/chart/PositionStatsPanel.tsx` - Stats + execute button

**V5 Migration (Breaking Changes)**:
```typescript
// Old (v4.2.1)
const series = chart.addCandlestickSeries(options);

// New (v5.x)
import { CandlestickSeries } from 'lightweight-charts';
const series = chart.addSeries(CandlestickSeries, options);
```

**Critical Success Criteria**:
- V5-10: Zones pan and zoom correctly with chart
- V5-21: End-to-end test confirms native feel

**PRD**: `.ralph/prd.json` (V5-01 to V5-24)

---

### Previous Changes (2026-01-12)

**Position Tool UX Refactor - TradingView Style Drag Interaction**:

Major refactor from click-based to drag-based UX:
- **New State Machine**: `idle → ready → dragging → complete`
- **Drag-to-draw**: Click and hold to set entry, drag to set SL, release to complete
- **Auto-calculated TP**: Based on R:R ratio from risk config (default 2:1)
- **Adjustable zone width**: Draggable left edge to resize position rectangle
- **Compact UI**: Stats panel tucked inside zone, minimal control bar

Technical fixes:
- Fixed stale closure bug using refs for event handlers
- Removed duplicate lines (native chart lines + overlay)
- Lines now 1px thin (was 2-3px)
- Zones extend from adjustable left edge to right (match price lines)
- Minimum drag distance (0.1% of price) prevents accidental positions

Files changed:
- `src/components/chart/PositionDrawingTool.tsx` - Drag-based state machine with refs
- `src/components/chart/PositionZoneOverlay.tsx` - Bounded zones, compact UI, draggable width

**Architecture Decision - Position Tool Implementation**:

Evaluated options for native canvas rendering:
1. **difurious/lightweight-charts-line-tools** - ❌ Deprecated, based on v3.8.0 (we're on v4.2.1)
2. **Current DOM overlay** - ✅ Works, we control it, shipping now
3. **V5 Pane Primitive plugin** - ✅ Future option for native canvas integration

Decision: Keep DOM overlay for now, consider V5 plugin architecture for future native canvas rendering.

**Previously completed - Drawable Position Tool (DRAW-01 to DRAW-09)**:

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

## Phase Summary

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
| **COLUMNAR** | Wire-Efficient SoA Data Format | Completed |
| **V5** | Native Canvas Position Tool (Hybrid) | Completed |
| **GEOM** | Time-Anchored Bounded Zones | Completed |

### How to Use Position Tool

1. Click the position tool button (crosshair icon) in chart toolbar
2. Click and **hold** on chart at desired entry price
3. **Drag** up or down to set stop loss (zone grows as you drag)
4. **Release** mouse - TP auto-calculates based on R:R ratio
5. Adjust levels by dragging handles (entry, SL, TP)
6. Drag right edge to set trade timeout (endTime)
7. Double-click right edge to clear timeout (extend to chart boundary)
8. Click Execute button (▶) or press Enter to place order
9. Press Escape or click ✕ to cancel

**Market Persistence**: Positions persist per-market. Switch to another symbol and back - your position is still there.

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
├── columnar/
│   └── mod.rs               # SoA data structures (DepthColumnStore, ColumnarOrderBook)
├── risk/
│   ├── position_sizer.rs    # "Conservative Wins" sizing
│   ├── validator.rs         # Pre-trade validation
│   └── storage.rs           # RiskConfig Redis storage

crates/router/src/
├── decision_loop.rs         # Shadow → Live execution flow
└── routes/
    ├── market_data.rs       # /api/v1 + /api/v2 market-data endpoints
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
    ├── requests.ts          # API calls (v1 + v2 endpoints)
    ├── ColumnDataView.ts    # SoA data wrapper (ColumnDataView, RowView)
    └── format.ts            # parseMarketSymbol() for USDT pairs
```

---

## API Endpoints

### Market Data (v1 - Row format)
```
GET /api/v1/market-data/ticker?symbol=SOLUSDT
GET /api/v1/market-data/orderbook?symbol=SOLUSDT&limit=20
GET /api/v1/market-data/klines?symbol=SOLUSDT&interval=1h&limit=100
GET /api/v1/market-data/markets   # Returns 539 USDT perps
```

### Market Data (v2 - Columnar format, ~25% smaller)
```
GET /api/v2/market-data/orderbook?symbol=SOLUSDT&limit=20
# Returns: { symbol, bids: {columns, data}, asks: {columns, data}, nonce }
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

## Fixed Bug: Position Tool Mouse Events Not Firing (2026-01-14)

### Symptom
Position tool activated (showed instruction) but clicking/dragging on the chart did nothing.

### Root Cause (Identified via Diagnostic Logging)
**Two issues discovered:**

1. **Event listener attachment**: Listeners were attached to `chartElement` which could become stale after chart recreation.

2. **coordinateToTime() returning null**: Clicks in the empty chart area (beyond the last candle) caused `coordinateToTime()` to return `null`, silently aborting the handler.

### Solution
```typescript
// Attach to window, get fresh element each time, handle null gracefully
useEffect(() => {
  if (drawingState !== 'ready' && drawingState !== 'dragging') return;

  const handleMouseDown = (e: MouseEvent) => {
    const chartElement = chartManager.getChartElement(); // Fresh reference
    const rect = chartElement.getBoundingClientRect();
    // Check bounds manually...
    const time = chartManager.coordinateToTime(x);
    if (price !== null && time !== null) {
      // Only proceed if both conversions succeed
      setDrawingState('dragging');
    }
    // Clicks in empty area silently ignored (expected behavior)
  };

  window.addEventListener('mousedown', handleMouseDown);
}, [chartManager, isActive, drawingState]);
```

### Enhancement (2026-01-14): Click Anywhere
Added fallback for clicks in empty chart area (right of last candle):
```typescript
let time = chartManager.coordinateToTime(x);
if (time === null) {
  time = Math.floor(Date.now() / 1000) as Time; // Use current time as fallback
}
```

Now users can click **anywhere** on the chart to draw positions. Drag behavior works: mousedown=entry, drag=SL, release=auto TP.

### Files Changed
- `testudo-web/apps/web/src/components/chart/PositionDrawingTool.tsx`

---

## References

- **PRD**: `.ralph/prd.json`
  - RISK-01 to RISK-15: Completed
  - DRAW-01 to DRAW-10: Completed
  - V5-01 to V5-24: Completed (Native canvas position tool)
  - GEOM-01 to GEOM-08: Completed (Time-anchored bounded zones)
- **Context**: `.ralph/context.md` (V5 migration reference)
- **Progress**: `.ralph/progress.md` (Task tracking)

### Architecture Diagrams
- **Position Tool Dataflow**: `testudo-web/apps/web/docs/diagrams/position-tool-dataflow.md`
- **V5 Primitive Architecture**: `testudo-web/apps/web/docs/diagrams/position-primitive-architecture.md`
- **V5 Performance Analysis**: `testudo-web/apps/web/docs/diagrams/v5-performance-analysis.md`
- **Chart Data Flow**: `testudo-web/apps/web/docs/diagrams/chart-data-flow.md`

### Phase Plans
- **Phase E Plans**: `docs/plans/e*.md`
