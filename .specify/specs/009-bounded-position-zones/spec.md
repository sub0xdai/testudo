# Feature: Bounded Position Zones (Dexscreener Style)

> Spec ID: 009-bounded-position-zones
> Created: 2026-02-08
> Status: Draft
> Priority: P1 (UX Enhancement)

---

## Overview

Position zones currently render as full-width bands spanning the entire chart. Entry/SL/TP dashed lines go edge-to-edge and open positions use `startTime: 0`, making shaded areas cover every candle. This creates visual clutter when multiple positions exist and doesn't match modern trading UI conventions.

**The Goal:** Convert position zones into compact, self-contained rectangles similar to dexscreener's long/short position tool. Each trade should be a bounded rectangle: green profit zone above entry, red loss zone below, with all four sides defined. Zones should maintain proportional width (~15% of visible chart) at any zoom level.

---

## User Stories

- [ ] As a trader with multiple open positions, I want each position to render as a compact rectangle so the chart remains readable and uncluttered.
- [ ] As a trader drawing a new position, I want the zone to appear as a bounded rectangle immediately so I can see its spatial context relative to candles.
- [ ] As a trader zooming in/out, I want open position zones to maintain proportional width so they stay visually consistent regardless of zoom level.
- [ ] As a trader, I want entry/SL/TP dashed lines to stop at zone edges so they don't interfere with other chart elements.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | **Bounded Lines**: Entry, SL, and TP dashed lines must render from `scaledStartX` to `scaledEndX` (zone width) instead of `0` to `width` (full chart width). | High |
| FR-2 | **Visible Range API**: ChartManager must expose `getVisibleTimeRange()` returning `{ from: number; to: number } | null` via the lightweight-charts timeScale API. | High |
| FR-3 | **Auto-Bounded Drawing**: When a user completes a position drawing (mousedown), compute `endTime = startTime + (visibleDuration * 0.15)` so zones are compact from the first frame. Fallback to `startTime + 3600` if visible range is unavailable. | High |
| FR-4 | **Proportional Open Position Zones**: OpenPositionsLayer must compute zone bounds from the chart's visible time range, placing each open position as a ~15% width rectangle near the right edge (80%-95% of visible range). | High |
| FR-5 | **Dynamic Recomputation**: Open position zone bounds must recompute on zoom/pan events so zones maintain proportional width. | Medium |
| FR-6 | **Handle Compatibility**: Right-edge drag handle must continue to work for manual zone resizing after auto-bounded placement. | High |
| FR-7 | **Stats Panel Compatibility**: Position stats panel and price axis labels must remain functional and correctly positioned within bounded zones. | High |

---

## Acceptance Criteria

- [ ] New position drawings render as compact rectangles (~15% of visible chart width).
- [ ] Entry/SL/TP dashed lines terminate at zone edges, not chart edges.
- [ ] Open positions render as bounded rectangles near the right edge of the chart.
- [ ] Zooming in/out causes open position zones to dynamically resize proportionally.
- [ ] Panning the chart causes open position zones to reposition near the right edge.
- [ ] Right-edge drag handle still allows manual resizing of drawn positions.
- [ ] Stats panel (quantity, risk, R:R) renders correctly within bounded zones.
- [ ] `cd testudo-web && bun run build` succeeds with no TypeScript errors.
- [ ] Visual regression: no broken rendering with 0, 1, or multiple concurrent positions.

---

## Technical Notes

### Files to Modify

| File | Change |
|------|--------|
| `testudo-web/apps/web/src/primitives/PositionZonePrimitive.ts` | Lines 148-170: Bound entry/SL/TP dashed lines from `(0, y) → (width, y)` to `(scaledStartX, y) → (scaledEndX, y)` |
| `testudo-web/apps/web/src/utils/chart_manager.ts` | Add `getVisibleTimeRange()` method wrapping `chart.timeScale().getVisibleRange()` |
| `testudo-web/apps/web/src/components/chart/PositionDrawingTool.tsx` | In `handleMouseDown` (~line 287): compute `endTime` from visible range when setting startTime |
| `testudo-web/apps/web/src/components/chart/OpenPositionsLayer.tsx` | Subscribe to time scale visible range changes; compute proportional startTime/endTime for each open position |
| `testudo-web/apps/web/src/hooks/useOpenPositions.ts` | Keep `startTime: 0 as Time` as placeholder (actual bounds computed in OpenPositionsLayer) |

### Implementation Details

#### FR-1: Bounded Lines (PositionZonePrimitive.ts)

Replace full-width line drawing with zone-bounded drawing. The variables `scaledStartX` and `scaledEndX` are already computed for fill rectangles:

```typescript
// BEFORE (full width)
ctx.moveTo(0, scaledEntryY);
ctx.lineTo(width, scaledEntryY);

// AFTER (bounded)
ctx.moveTo(scaledStartX, scaledEntryY);
ctx.lineTo(scaledEndX, scaledEntryY);
```

Apply to all three lines (entry at line 152-153, SL at line 160-161, TP at line 168-169).

#### FR-2: Visible Range API (chart_manager.ts)

```typescript
public getVisibleTimeRange(): { from: number; to: number } | null {
  const range = this.chart.timeScale().getVisibleRange();
  if (!range) return null;
  return { from: Number(range.from), to: Number(range.to) };
}
```

#### FR-3: Auto-Bounded Drawing (PositionDrawingTool.tsx)

In `handleMouseDown`, after capturing `startTime`, compute `endTime`:

```typescript
// Compute bounded endTime from visible range
const visibleRange = chartManager.getVisibleTimeRange();
let endTime: Time;
if (visibleRange) {
  const duration = visibleRange.to - visibleRange.from;
  endTime = (time as number + duration * 0.15) as unknown as Time;
} else {
  endTime = ((time as number) + 3600) as unknown as Time; // 1 hour fallback
}

setLevels({
  entryPrice: price,
  stopLossPrice: price,
  takeProfitPrice: null,
  startTime: time,
  endTime,
});
```

#### FR-4/FR-5: Proportional Open Position Zones (OpenPositionsLayer.tsx)

Subscribe to time scale visible range changes and recompute zone bounds:

```typescript
// Subscribe to visible range changes for proportional zone sizing
useEffect(() => {
  if (!chartManager) return;

  const updateZoneBounds = () => {
    const visibleRange = chartManager.getVisibleTimeRange();
    if (!visibleRange) return;

    const duration = visibleRange.to - visibleRange.from;
    const startTime = (visibleRange.to - duration * 0.20) as unknown as Time;
    const endTime = (visibleRange.to - duration * 0.05) as unknown as Time;

    // Update each open position with proportional bounds
    for (const position of positions) {
      if (position.levels) {
        chartManager.updateOpenPositionLevels(position.id, {
          ...position.levels,
          startTime,
          endTime,
        });
      }
    }
  };

  // Subscribe via crosshair move (fires on zoom/pan)
  const unsub = chartManager.subscribeCrosshairMove(updateZoneBounds);
  updateZoneBounds(); // Initial computation

  return unsub;
}, [chartManager, positions]);
```

### Effect Summary

| Change | Effect |
|--------|--------|
| Lines bounded to zone | Entry/SL/TP dashes only span the rectangle |
| Auto endTime on draw | New drawings are immediately compact |
| Visible-range-proportional sizing | Zones stay ~15% of chart width at any zoom |
| Open position bounded zones | Existing trades appear as rectangles, not bands |

### Dependencies

- `lightweight-charts` v5 `timeScale().getVisibleRange()` API
- Existing `PositionZonePrimitive` coordinate system (`scaledStartX`, `scaledEndX`)
- Existing `ChartManager` coordinate conversion methods

### Assumptions

- The `getVisibleRange()` API returns `null` before chart data is loaded (handled by fallback).
- Open position zones near the right edge (80%-95%) don't conflict with the drawing tool's zones.
- Crosshair move subscription fires on zoom/pan events (confirmed by lightweight-charts behavior).

---

## Completion Signal

### Implementation Checklist
- [ ] `PositionZonePrimitive` draws lines bounded to `scaledStartX`/`scaledEndX`.
- [ ] `ChartManager.getVisibleTimeRange()` method exists and returns correct values.
- [ ] Drawing tool auto-computes `endTime` on mousedown.
- [ ] Open positions render as proportional rectangles near right edge.
- [ ] Zones recompute on zoom/pan.
- [ ] All functional requirements implemented.

### Testing Requirements
- [ ] `cd testudo-web && bun run build` passes with no TypeScript errors.
- [ ] Visual verification: draw a new position - zone is a compact rectangle.
- [ ] Visual verification: entry/SL/TP lines stop at zone edges.
- [ ] Visual verification: open positions render as bounded rectangles.
- [ ] Visual verification: zoom in/out - zones maintain proportional width.
- [ ] Visual verification: right-edge handle still works for resizing.
- [ ] Visual verification: stats panel renders correctly.

### Iteration Protocol
If any check fails:
1. Identify the issue (rendering bug vs coordinate math vs API issue).
2. If rendering: Check `scaledStartX`/`scaledEndX` values in dev tools canvas debugger.
3. If coordinate: Verify `getVisibleRange()` returns expected `{ from, to }` values.
4. If API: Confirm lightweight-charts v5 `timeScale()` methods are available.
5. Re-run verification.

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

*Template version: 1.0*
