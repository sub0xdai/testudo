# Testudo V5 Hybrid Position Tool - Project Context

## Tech Stack
- **Backend**: Rust (Cargo, Axum, Tokio)
- **Frontend**: TypeScript, React 18, Vite, Tailwind CSS
- **Charts**: lightweight-charts v5.x (upgrading from v4.2.1)
- **Package Manager**: Bun (frontend), Cargo (backend)
- **Testing**: cargo test, bun test

## Coding Standards

### TypeScript/React
- Functional components with hooks
- Use existing UI patterns from components/ui/
- Follow industrial/brutalist design system (no rounded corners)
- Use `font-mono` for numbers, `font-display` for labels

### Canvas Primitives (V5)
- Implement `IPanePrimitive` interface for custom drawings
- Use `IPrimitivePaneView` and `IPrimitivePaneRenderer` for canvas rendering
- Store price values, convert to Y coordinates at render time
- Call `requestUpdate()` to trigger repaints

### Hybrid Architecture
- **Canvas (Primitive)**: Zones, lines - moves with chart automatically
- **DOM (React)**: Drag handles, buttons, stats panel - easier interaction

## Key Files Reference

### Existing (to modify)
- Chart Manager: `testudo-web/apps/web/src/utils/chart_manager.ts`
- Position Tool: `testudo-web/apps/web/src/components/chart/PositionDrawingTool.tsx`
- Zone Overlay (replace): `testudo-web/apps/web/src/components/chart/PositionZoneOverlay.tsx`

### New Files (to create)
- Primitive: `testudo-web/apps/web/src/primitives/PositionZonePrimitive.ts`
- Handle Overlay: `testudo-web/apps/web/src/components/chart/PositionHandleOverlay.tsx`
- Stats Panel: `testudo-web/apps/web/src/components/chart/PositionStatsPanel.tsx`

## V5 Migration Reference

### Series Creation (Breaking Change)
```typescript
// Old (v4)
const series = chart.addCandlestickSeries(options);

// New (v5)
import { CandlestickSeries } from 'lightweight-charts';
const series = chart.addSeries(CandlestickSeries, options);
```

### Pane Primitive Interface
```typescript
interface IPanePrimitive {
  paneViews(): IPrimitivePaneView[];
  attached(params: { chart, requestUpdate }): void;
  detached(): void;
}

interface IPrimitivePaneView {
  renderer(): IPrimitivePaneRenderer;
}

interface IPrimitivePaneRenderer {
  draw(target: CanvasRenderingTarget2D): void;
  zOrder?: number;  // relative to series, crosshair, etc.
}
```

## Verification Commands
```bash
# Frontend
cd testudo-web/apps/web && bun run lint && bun run build

# Manual Test
# 1. Draw position on chart
# 2. Pan chart left/right - zones should follow
# 3. Zoom price axis - zones should scale
# 4. Drag handles - zones should update in real-time
```

## Design Goals
1. **Native feel**: Zones pan/zoom with chart like built-in indicators
2. **Responsive handles**: DOM drag handles reposition on chart movement
3. **No jank**: <16ms frame time during rapid pan/zoom
4. **TradingView style**: Match their color scheme and visual weight
