# Quality Checklist: 009-bounded-position-zones

> Spec: 009-bounded-position-zones
> Date: 2026-02-08

---

## Code Quality

- [ ] No TypeScript errors (`bun run build` passes)
- [ ] No new `any` types introduced
- [ ] Follows existing code patterns in each modified file
- [ ] No hardcoded magic numbers without constants or comments
- [ ] Type safety maintained for `Time` type conversions

## Functional Completeness

- [ ] FR-1: Entry/SL/TP lines bounded to zone width
- [ ] FR-2: `getVisibleTimeRange()` method added to ChartManager
- [ ] FR-3: Drawing tool auto-computes `endTime` from visible range
- [ ] FR-4: Open positions render as proportional rectangles
- [ ] FR-5: Zones recompute dynamically on zoom/pan
- [ ] FR-6: Right-edge drag handle still functional
- [ ] FR-7: Stats panel and price axis labels unaffected

## Visual Regression

- [ ] Single position renders correctly (drawing tool)
- [ ] Single open position renders correctly (OpenPositionsLayer)
- [ ] Multiple open positions don't overlap confusingly
- [ ] Zooming preserves proportional zone sizing
- [ ] Panning repositions open position zones near right edge
- [ ] Price axis labels still visible and correctly positioned
- [ ] No rendering artifacts when chart has no data or minimal data

## Edge Cases

- [ ] Drawing right of last candle (coordinateToTime returns null → fallback)
- [ ] Chart with no visible range yet (initial load → fallback)
- [ ] Very narrow zoom (zones don't collapse to invisible)
- [ ] Very wide zoom (zones don't become oversized)
- [ ] Position with missing TP (levels null → no render, no crash)

## Performance

- [ ] No excessive re-renders from visible range subscription
- [ ] Zone recomputation doesn't cause frame drops during pan/zoom
- [ ] Crosshair move handler is lightweight (no heavy computation per event)

---

*Quality checklist for spec 009-bounded-position-zones*
