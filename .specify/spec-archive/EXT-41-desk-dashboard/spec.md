# Specification: Redesign Desk Dashboard for High-Signal Analytics

**Date:** 2026-03-24
**Status:** Draft
**Class:** Feature / Frontend UI
**Priority:** P1 — Essential for data legibility, reducing cognitive load, and enabling rapid time-series analysis.
**Depends on:** None
**Series:** UI-02 (Analytics & Journal Optimization)
**Target:** testudo-journal (Solid.js, Tailwind v3, ECharts v5)

---

## Problem Statement

The current Desk page (analytics and journal dashboard) presents high-quality metrics (Expectancy, R-Multiple, Max DD) but suffers from presentation flaws that degrade its utility as a professional trading terminal.

First, overlaying raw text onto the Hadrian's Wall background image creates legibility issues — the `.bg-overlay` class (88% opacity on dark, 82% on light) helps but panels lack `backdrop-blur` for true glass separation. Second, the layout duplicates the same hero metrics (Net P&L, R-Multiple, Expectancy, Win Rate, Profit Factor, Trades) in both the hero header and the sidebar's ACCOUNT/PERFORMANCE sections. Third, the time dimension (1W/1M/3M/YTD/ALL) is hidden behind a Filter popout that requires a click to reveal, increasing interaction cost for the most common user action. Fourth, chart type dropdowns (`ChartSelector`) float above their chart containers in a separate `div`, breaking visual containment. Finally, several directional metrics in the sidebar lack semantic coloring — Performance section items (Profit Factor, Expectancy, R-Multiple) don't use `pnlColor()`.

---

## User Stories

- **As a risk manager**, I want to filter my performance by Time and Account with zero extra clicks, so that I can rapidly analyze different historical periods.
- **As a trader**, I want my metrics and charts to sit on clean, glass-effect panels, so that I can read the data instantly without background noise interference.
- **As a user**, I want directional metrics (P&L, R-Multiple, Expectancy, Profit Factor) to be color-coded (green/red), so that I can immediately gauge performance trend at a glance.
- **As an analyst**, I want chart-specific controls (e.g., Symbol vs. Long/Short) embedded directly in the chart's header bar, so that vertical space is maximized for the ECharts canvas.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `backdrop-blur-md` + semi-transparent bg to sidebar, hero header, and chart containers for glass panel effect. | High | testudo-journal |
| FR-2 | Promote time presets (1W/1M/3M/YTD/ALL) from `FilterPopout` into `PageSubHeader` as always-visible segmented control. Keep symbol search and custom dates in the popout. | High | testudo-journal |
| FR-3 | Remove duplicate metrics — hero header shows Net P&L + R-Multiple (large) with sub-row of Expectancy, Win Rate, PF, Trades. Sidebar shows ACCOUNT (Total P&L, Net P&L, Fees, Trades), PERFORMANCE (Win Rate, PF, Expectancy, R-Multiple, Trades/Day), RISK (Max DD, Worst Day/Week, Streaks). Eliminate overlap between hero sub-row and sidebar. | High | testudo-journal |
| FR-4 | Apply `pnlColor()` / `rColor()` to all directional values in sidebar: Expectancy, R-Multiple, Profit Factor (>1 green, <1 red), Trades/Day (neutral). | High | testudo-journal |
| FR-5 | Embed chart type dropdown inside `ChartContainer` header bar (replacing the separate `ChartSelector` wrapper div). | Medium | testudo-journal |

---

## Technical Implementation

### Existing Infrastructure (DO NOT recreate)

These already exist and should be reused:
- `pnlColor()`, `rColor()`, `formatCurrency()` etc. in `src/lib/formatters.ts`
- `FilterPopout` with presets in `src/components/FilterPopout.tsx`
- `useFilters()` context in `src/components/filterContext.tsx`
- CSS tokens: `--signal-green`, `--signal-red`, `text-signal-green`, `text-signal-red`
- `.bg-overlay` class for background dimming

### FR-1: Glass Panel Styling

Add reusable glass panel class or apply directly:
```css
/* In app.css or as Tailwind utility */
.glass-panel {
  background: rgb(var(--bg-panel) / 0.7);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid rgb(var(--border) / 0.5);
}
```

Apply to: sidebar `<aside>`, hero metrics `<div>`, each `ChartContainer`.

### FR-2: Always-Visible Time Presets

Move the segmented time control from `FilterPopout` into `PageSubHeader`:
- Time presets (1W/1M/3M/YTD/ALL) render directly in the subheader bar
- Filter button still opens the popout for symbol search + custom date range
- `FilterPopout` retains custom date inputs but no longer shows the preset buttons

```tsx
// In PageSubHeader.tsx — add inline time presets
<div class="flex items-center gap-1">
  <For each={PRESETS.filter(p => p.key !== 'custom')}>
    {(p) => (
      <button
        class={`font-mono text-xs px-2.5 py-1 rounded transition-colors ${
          activePreset() === p.key
            ? 'bg-text-primary/10 text-text-primary'
            : 'text-text-tertiary hover:text-text-primary'
        }`}
        onClick={() => selectPreset(p.key)}
      >
        {p.label}
      </button>
    )}
  </For>
</div>
```

### FR-3: Metric Consolidation

**Keep hero header**: Net P&L (large) + R-Multiple (large). Remove the sub-row (Expectancy, Win Rate, PF, Trades) — these are already in the sidebar.

**Keep sidebar** as-is with its three sections. No duplication remains.

### FR-4: Semantic Coloring

In `Overview.tsx`, update `performanceItems()`:
```tsx
{ label: 'Profit Factor', value: formatNumber(d.performance.profit_factor),
  colorClass: parseFloat(d.performance.profit_factor) > 1 ? 'text-signal-green' : parseFloat(d.performance.profit_factor) < 1 ? 'text-signal-red' : undefined },
{ label: 'Expectancy', value: formatCurrency(d.performance.expectancy),
  colorClass: pnlColor(d.performance.expectancy) },
{ label: 'R-Multiple', value: formatNumber(d.performance.avg_r_multiple),
  colorClass: rColor(d.performance.avg_r_multiple) },
```

### FR-5: Embedded Chart Controls

Modify `ChartSelector` to render the dropdown inside the chart container header:
```tsx
<div class="glass-panel flex flex-col h-full overflow-hidden">
  <div class="flex justify-between items-center border-b border-container-border/50 px-4 py-2">
    <select ...>{/* chart options */}</select>
  </div>
  <div class="flex-grow relative min-h-[250px]">
    {/* Active chart renders here */}
  </div>
</div>
```

### Files to Modify

| File | Change |
|------|--------|
| `testudo-journal/src/components/Overview.tsx` | Glass panels on sidebar + hero, remove hero sub-row, semantic colors on performance items |
| `testudo-journal/src/components/PageSubHeader.tsx` | Add inline time presets, track active preset |
| `testudo-journal/src/components/FilterPopout.tsx` | Remove time preset buttons (moved to subheader) |
| `testudo-journal/src/components/ChartSelector.tsx` | Wrap chart in glass panel with embedded dropdown header |
| `testudo-journal/src/styles/app.css` | Add `.glass-panel` utility class |

### Files NOT Modified (no new files needed)

- `src/lib/formatters.ts` — already has `pnlColor()`, `rColor()`
- `src/components/filterContext.tsx` — already has global filter state
- `src/api/client.ts` — no API changes required

---

## Acceptance Criteria

- [ ] Sidebar, hero header, and chart containers have visible `backdrop-blur` glass effect distinguishing them from the background.
- [ ] Time presets (1W/1M/3M/YTD/ALL) are always visible in the subheader — no click required.
- [ ] Clicking a time preset updates the filter and triggers chart/metric recalculation.
- [ ] Symbol search + custom date range remain accessible via Filter popout.
- [ ] Hero header shows Net P&L + R-Multiple only (no duplicate sub-row of sidebar metrics).
- [ ] Sidebar Performance section has semantic colors: Expectancy (green/red), R-Multiple (green/amber/red), Profit Factor (green/red based on >1/<1).
- [ ] Chart type dropdown is embedded in the chart container's header bar.
- [ ] Charts resize correctly within glass-panel flex containers (ECharts `ResizeObserver` works).
- [ ] Mobile condensed stats strip still works.
- [ ] Light theme glass panels are correctly styled (light token values).
- [ ] `bun run build` succeeds for testudo-journal.

---

## Risks

1. **ECharts Canvas Resizing** — When wrapping ECharts in flex/grid glass panels, the canvas may fail to resize.
   * *Mitigation:* Existing `ChartContainer` already handles ResizeObserver. Verify it still works within the new glass panel wrapper.
2. **Backdrop-blur performance** — Multiple stacked blur layers can cause GPU overhead on low-end devices.
   * *Mitigation:* Use `backdrop-blur-md` (12px), not extreme values. Limit blur to 3-4 panels visible at once.

---

## Completion Signal

This spec is complete when:
1. All acceptance criteria pass.
2. `cd testudo-journal && bun run build` exits 0.
3. Code is committed to the master branch.
