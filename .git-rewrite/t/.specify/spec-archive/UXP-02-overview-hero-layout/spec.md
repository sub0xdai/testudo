# Specification: FXBlue-Style Overview with Hero Equity Curve

**Spec ID:** UXP-02-overview-hero-layout
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / UI Layout
**Priority:** P0 — The first thing a trader sees; must communicate account health instantly
**Depends on:** UXP-01-design-system-alignment
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

The current Overview page is three equal-weight stat card columns. Every value is `text-sm font-mono` — the total P&L has the same visual weight as margin level. A trader opening the app cannot answer "how am I doing?" in under 2 seconds. The equity curve is buried on a separate Charts page, invisible from the landing view.

FXBlue's design (the reference) solves this: account stats in a dense left column, a large equity curve dominating the right side, with secondary charts below. The equity curve is the hero — the single most important visualization. The stat cards become a sidebar summary, not the main event.

The current layout also wastes the full-width 1400px container on three narrow 400px cards with generous padding. The redesign uses a 2-column split (stats sidebar + hero chart) that leverages the width.

---

## User Stories

- **As a trader**, I want to see my equity curve and key stats immediately on load, so that I can assess account health in seconds.
- **As a trader**, I want my total P&L displayed prominently, so that the most important number is unmissable.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Redesign Overview as 2-column layout: left sidebar (account + performance + risk stats) and right main area (hero equity curve + secondary chart selector) | High | Layout |
| FR-2 | Hero equity curve spans full right column, minimum 400px height, no card wrapper — borderless with only a bottom border separator | High | Charts |
| FR-3 | Total P&L displayed as hero number: `font-mono text-4xl md:text-5xl font-bold` with pnlColor, positioned above or within the equity curve area | High | Typography |
| FR-4 | Account stats sidebar collapses the three current cards into a single dense list with section dividers (Account, Performance, Risk) | High | Layout |
| FR-5 | Secondary chart area below equity curve with dropdown selector (like FXBlue) — options: Symbol Distribution, Market Return, Duration/Profitability, Return Distribution, Time Heatmap | Medium | Charts |
| FR-6 | Win rate and profit factor displayed as secondary hero metrics alongside total P&L | Medium | Typography |
| FR-7 | Responsive: on mobile, stats stack above the equity curve as a condensed horizontal strip, chart goes full-width below | Medium | Responsive |

---

## Technical Implementation

### Layout Structure

```
┌─────────────────────────────────────────────────┐
│  TESTUDO_JOURNAL     OVERVIEW  CHARTS  TRADES   │
│  ───────────────────────────────────────────── │
│  [FilterBar]                                    │
├──────────────┬──────────────────────────────────┤
│              │                                  │
│  ACCOUNT     │   +$2,847.32        81% WR       │
│  ──────────  │   ─────────────────────────────  │
│  Balance     │                                  │
│  Equity      │   ╱──╲    ╱╲   ╱╲  ╱──          │
│  Floating    │  ╱    ╲──╱  ╲─╱  ╲╱             │
│  Closed P&L  │  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ (DD)     │
│  Free Margin │                                  │
│              │  EQUITY CURVE (400px, borderless) │
│  PERFORMANCE │                                  │
│  ──────────  ├──────────────────────────────────┤
│  Total Ret.  │                                  │
│  Monthly     │  [Symbol Distribution ▼]         │
│  Weekly      │                                  │
│  Peak DD     │   🍩 Donut / Bar / Scatter       │
│  Win Rate    │                                  │
│  Profit Fac. │  SECONDARY CHART (selector)      │
│              │                                  │
│  RISK        │                                  │
│  ──────────  │                                  │
│  Worst Day   │                                  │
│  Avg Win     │                                  │
│  Avg Loss    │                                  │
│              │                                  │
└──────────────┴──────────────────────────────────┘
```

### Hero P&L Display

```tsx
<div class="flex items-baseline gap-4 mb-4">
  <span class={`font-mono text-4xl md:text-5xl font-bold ${pnlColor(stats.net_pnl)}`}>
    {formatCurrency(stats.net_pnl)}
  </span>
  <span class="font-mono text-sm text-text-secondary">
    {formatPercent(stats.total_return)} return
  </span>
</div>
<div class="flex gap-6 font-mono text-sm">
  <span class="text-text-secondary">
    Win Rate <span class="text-text-primary font-bold">{formatPercent(stats.win_rate)}</span>
  </span>
  <span class="text-text-secondary">
    Profit Factor <span class="text-text-primary font-bold">{stats.profit_factor.toFixed(2)}</span>
  </span>
</div>
```

### Stats Sidebar

Replace three separate StatCard components with a single scrollable sidebar:

```tsx
<aside class="w-64 border-r border-container-border overflow-y-auto">
  <StatSection title="ACCOUNT" items={accountItems} />
  <StatSection title="PERFORMANCE" items={perfItems} />
  <StatSection title="RISK" items={riskItems} />
</aside>
```

Each `StatSection`:
- Title: `font-display text-xs tracking-[0.2em] text-text-tertiary uppercase px-4 py-3 border-b border-container-border`
- Rows: `flex justify-between px-4 py-1.5`, label in `font-display text-xs text-text-secondary`, value in `font-mono text-xs font-bold`
- Dense: 24px row height, no card wrapper, no padding bloat

### Chart Selector

A `<select>` or custom dropdown above the secondary chart area, styled as:
```
font-mono text-xs border border-container-border bg-elevated px-3 py-1.5
```

### Files

- `testudo-journal/src/components/Overview.tsx` — Complete rewrite to 2-column layout
- `testudo-journal/src/components/StatCard.tsx` — Replace with `StatSection.tsx` (dense row list)
- `testudo-journal/src/components/StatSection.tsx` — New dense stat list component
- `testudo-journal/src/components/HeroEquityCurve.tsx` — New borderless equity curve component (extracted from Charts)
- `testudo-journal/src/components/ChartSelector.tsx` — New secondary chart switcher

---

## Acceptance Criteria

- [ ] Overview loads with equity curve visible above the fold (no scrolling needed)
- [ ] Total P&L is the largest text element on the page (minimum `text-4xl`)
- [ ] Stats sidebar is dense — all three sections visible without scrolling on 1080p
- [ ] Equity curve has no card border wrapper — bleeds to edges of its column
- [ ] Secondary chart area has a working dropdown selector with at least 3 chart options
- [ ] Mobile layout stacks stats above chart
- [ ] CumulativeProfit no longer duplicates EquityCurve API call (shared data source)
- [ ] `bun run build` passes

---

## Risks

1. **Equity curve resize on column layout** — lightweight-charts needs explicit width. Mitigation: Use ResizeObserver (already implemented in current EquityCurve).
2. **Stats sidebar too narrow for long numbers** — Mitigation: Use abbreviated formatting (`$2.8K` instead of `$2,847.32`) for values exceeding column width, with tooltip for full precision.

---

## Completion Signal

This spec is complete when:
1. Overview renders as a 2-column stats + chart layout
2. Total P&L is the dominant visual element
3. Equity curve is borderless and minimum 400px tall
4. Secondary chart selector works with at least 3 chart types
5. `bun run build` passes
6. Code committed to master
