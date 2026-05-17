# Geometry Polish: Time-Anchored Bounded Zones

## Overview

Refactor position zones from full-width spans to time-anchored bounded rectangles, matching TradingView's visual style.

## Current State

- Zones span full chart width (`0` to `width`)
- Lines span full width at 1-2px
- No time anchor - zones don't have a meaningful start point

## Target State

```
┌──────────────────────────────────────────────────────────────────────┐
│                          │                                           │
│  Candlesticks        ════╬═══════════════════════════════════════════╡ TP (dashed green)
│     ████                 │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
│   ██████                 │▓▓▓▓▓▓ PROFIT ZONE ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
│     ████                 │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
│       ██             ════╬═══════════════════════════════════════════╡ Entry (dashed orange)
│                          │░░░ LOSS ZONE ░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│                      ════╬═══════════════════════════════════════════╡ SL (dashed red)
└──────────────────────────────────────────────────────────────────────┘
                           ▲
                      startTime (candle where position was drawn)
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Zone anchor | Time-anchored left, right edge at chart boundary | Matches TradingView, meaningful start point |
| Zone width | Variable (startTime to chart edge) | Expands as chart pans left |
| Zone borders | Filled only, no stroke | Clean, minimal |
| Line width | 1px all lines | Thin, non-intrusive |
| Line style | All dashed | Matches TradingView reference |
| Entry color | Orange (`#f0b90b`) | TradingView style (was white) |

## Data Model Changes

```typescript
// Before
interface PositionLevels {
  entry: number;
  stopLoss: number;
  takeProfit: number;
  side: "long" | "short";
}

// After
interface PositionLevels {
  entry: number;
  stopLoss: number;
  takeProfit: number;
  side: "long" | "short";
  startTime: Time;        // NEW: X anchor for zone left edge
  endTime?: Time;         // NEW (optional): Future timeout feature
}
```

## Renderer Changes

```typescript
// Zone rendering (before)
ctx.fillRect(0, profitTop, width, profitHeight);

// Zone rendering (after)
const startX = timeScale.timeToCoordinate(startTime);
const zoneWidth = width - scaledStartX;
ctx.fillRect(scaledStartX, profitTop, zoneWidth, profitHeight);
```

## Line Styling

| Line | Color | Style | Width |
|------|-------|-------|-------|
| TP | `#34cb88` (green) | Dashed `[5, 5]` | 1px |
| Entry | `#f0b90b` (orange) | Dashed `[5, 5]` | 1px |
| SL | `#ff615c` (red) | Dashed `[5, 5]` | 1px |

## Files to Modify

1. `src/primitives/PositionZonePrimitive.ts` - Core geometry changes
2. `src/utils/chart_manager.ts` - Pass timeScale reference
3. `src/components/chart/PositionDrawingTool.tsx` - Capture startTime

## Future Enhancement (Noted)

Time-boxed trades: `endTime` property enables visual trade timeout where right edge = expiry time instead of chart edge.

## Verification

```bash
cd testudo-web/apps/web && bun run lint && bun run build
```

Manual test:
1. Draw position - zone should start at click point
2. Pan left - zone should expand to maintain right edge at chart boundary
3. Pan right past startTime - zone should disappear (off-screen)
