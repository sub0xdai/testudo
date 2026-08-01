# Specification: Color Token Centralization

**Spec ID:** UXP-14-color-token-centralization
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / Design System
**Priority:** P1 — Rebranding or theme adjustment requires editing 14+ files with inconsistent hex values
**Depends on:** None (independent of UXP-13)
**Series:** UXP-13 through UXP-15 (Overview redesign + critique fixes)

---

## Problem Statement

The journal UI's color system is fractured across 14+ locations with no single source of truth. The CSS token file (`app.css`) defines only 7 variables, while the actual UI uses 15+ distinct colors — most hardcoded as hex literals inside JavaScript chart configs and component files.

Specific problems:

1. **Signal colors scattered across 8 chart files**: `#00FF41` (profit green) and `#FF003C` (loss red) are hardcoded in `HeroEquityCurve.tsx`, `EquityCurve.tsx`, `DailyPnl.tsx`, `CumulativeProfit.tsx`, `SymbolDonut.tsx`, `MarketReturn.tsx`, `DurationScatter.tsx`, and `ReturnHistogram.tsx`. Some use rgba variants (`rgba(0, 255, 65, 0.6)`) which are marginally different from the hex (`#00FF41` = `rgb(0, 255, 65)`).

2. **Chart background `#111111` is a phantom token**: Used in 6 chart files as `background: { color: '#111111' }` but not defined in the CSS variable system. It represents a third background level between `--color-main-bg` (#050505) and `--color-container-bg` (#0A0A0A).

3. **Three tag color palettes with divergent values**: `TagBadge.tsx`, `TagManager.tsx`, and `SymbolDonut.tsx` each define their own color array. TagBadge and TagManager share colors in different order. SymbolDonut swaps `#06B6D4` for `#14B8A6` and `#10B981` for `#F97316`.

4. **EntryCard type colors disconnected from signal system**: Post-trade uses `#22C55E` (Tailwind green-500), completely unrelated to signal green `#00FF41`. Pre-trade amber `#f59e0b` also appears in chart palettes but isn't tokenized.

5. **`tracking-[0.2em]` magic string in 6 files**: Not a color, but the same pattern — a design token expressed as an arbitrary Tailwind value in `StatSection.tsx`, `Overview.tsx`, `StatCard.tsx`, `ChartContainer.tsx` (x2), and `TagManager.tsx`.

---

## User Stories

- **As a developer**, I want to change the profit color in one place, so that all charts, stats, and indicators update consistently.
- **As a developer**, I want a complete token inventory, so that I can reason about the color system from a single file.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add signal color CSS variables: `--color-signal-green`, `--color-signal-red`, `--color-signal-amber` to `app.css :root`. | High | Tokens |
| FR-2 | Add chart background CSS variable: `--color-chart-bg` to `app.css :root`. | High | Tokens |
| FR-3 | Add elevated background CSS variable: `--color-elevated` to `app.css :root` (currently defined only in Tailwind config). | Medium | Tokens |
| FR-4 | Create `testudo-journal/src/lib/tokens.ts` exporting all color constants for use in JS chart configs: `SIGNAL_GREEN`, `SIGNAL_RED`, `SIGNAL_AMBER`, `CHART_BG`, and `TAG_PALETTE`. | High | Tokens |
| FR-5 | Replace all hardcoded `#00FF41` / `#FF003C` in chart files with imports from `tokens.ts`. | High | Charts |
| FR-6 | Replace all hardcoded `#111111` chart backgrounds with `CHART_BG` from `tokens.ts`. | High | Charts |
| FR-7 | Unify tag color palettes: `TagBadge.tsx`, `TagManager.tsx`, and `SymbolDonut.tsx` all import `TAG_PALETTE` from `tokens.ts`. | High | Components |
| FR-8 | Replace `EntryCard.tsx` type color `#22C55E` with a token (`ENTRY_TYPE_COLORS` map in `tokens.ts`). | Medium | Components |
| FR-9 | Add `tracking-section` to Tailwind config as a custom tracking value (`0.2em`). Replace all 6 instances of `tracking-[0.2em]` with `tracking-section`. | Medium | Config |
| FR-10 | Unify animation duration: export `CLOSE_ANIMATION_MS = 200` from `tokens.ts`. Replace the 150ms/200ms setTimeout values in `TradeDetail.tsx`, `EntryEditor.tsx`, and `TagManager.tsx` with the shared constant. | Low | Animation |

---

## Technical Implementation

### CSS Variables (app.css)

```css
:root {
  /* Existing */
  --color-main-bg: #050505;
  --color-container-bg: #0A0A0A;
  --color-container-border: #3F3F46;
  --color-accent-steel: #94A3B8;
  --color-text-primary: #FFFFFF;
  --color-text-secondary: #888888;
  --color-text-tertiary: #555555;

  /* New — signal colors */
  --color-signal-green: #00FF41;
  --color-signal-red: #FF003C;
  --color-signal-amber: #F59E0B;

  /* New — chart and elevated backgrounds */
  --color-chart-bg: #111111;
  --color-elevated: #141414;

  /* Layout */
  --header-h: 57px;
}
```

### Token Module (tokens.ts)

```typescript
// testudo-journal/src/lib/tokens.ts

// Signal colors — for chart JS configs where CSS vars can't reach
export const SIGNAL_GREEN = '#00FF41'
export const SIGNAL_RED = '#FF003C'
export const SIGNAL_AMBER = '#F59E0B'

// Derived rgba variants
export const signalGreenAlpha = (a: number) => `rgba(0, 255, 65, ${a})`
export const signalRedAlpha = (a: number) => `rgba(255, 0, 60, ${a})`

// Chart background
export const CHART_BG = '#111111'

// Tag color palette — single source for TagBadge, TagManager, SymbolDonut
export const TAG_PALETTE = [
  '#00FF41', '#FF003C', '#3B82F6', '#F59E0B',
  '#8B5CF6', '#EC4899', '#06B6D4', '#10B981',
]

// Entry type colors
export const ENTRY_TYPE_COLORS: Record<string, string> = {
  'note':           '#94A3B8',
  'pre-trade':      '#F59E0B',
  'post-trade':     '#22C55E',
  'daily-review':   '#888888',
  'weekly-review':  '#888888',
}

// Animation timing
export const CLOSE_ANIMATION_MS = 200
```

### Migration Map

| File | Current | Replacement |
|------|---------|-------------|
| `HeroEquityCurve.tsx:40` | `'#00FF41'` | `SIGNAL_GREEN` |
| `EquityCurve.tsx:40` | `'#00FF41'` | `SIGNAL_GREEN` |
| `EquityCurve.tsx:22` | `'#111111'` | `CHART_BG` |
| `DailyPnl.tsx:20` | `'#111111'` | `CHART_BG` |
| `DailyPnl.tsx:57` | `'#00FF41' : '#FF003C'` | `SIGNAL_GREEN : SIGNAL_RED` |
| `CumulativeProfit.tsx:21` | `'#111111'` | `CHART_BG` |
| `CumulativeProfit.tsx:37` | `'#00FF41'` | `SIGNAL_GREEN` |
| `SymbolDonut.tsx:9` | inline palette | `TAG_PALETTE` |
| `SymbolDonut.tsx:32` | `'#111111'` | `CHART_BG` |
| `MarketReturn.tsx:47` | `'#111111'` | `CHART_BG` |
| `MarketReturn.tsx:70` | `'#00FF41' : '#FF003C'` | `SIGNAL_GREEN : SIGNAL_RED` |
| `DurationScatter.tsx:40` | `'#111111'` | `CHART_BG` |
| `DurationScatter.tsx:73-74` | rgba inline | `signalGreenAlpha(0.6) : signalRedAlpha(0.6)` |
| `ReturnHistogram.tsx:38` | `'#111111'` | `CHART_BG` |
| `ReturnHistogram.tsx:61` | `'#00FF41' : '#FF003C'` | `SIGNAL_GREEN : SIGNAL_RED` |
| `TagBadge.tsx:3` | inline palette | `TAG_PALETTE` |
| `TagManager.tsx:6` | inline palette | `TAG_PALETTE` |
| `EntryCard.tsx:7-13` | inline color map | import from `ENTRY_TYPE_COLORS` |
| `EntryEditor.tsx:19-25` | inline color map | import from `ENTRY_TYPE_COLORS` |
| `TradeDetail.tsx:40` | `setTimeout(..., 200)` | `setTimeout(..., CLOSE_ANIMATION_MS)` |
| `EntryEditor.tsx:55` | `setTimeout(..., 150)` | `setTimeout(..., CLOSE_ANIMATION_MS)` |
| `TagManager.tsx` | `setTimeout(..., 150)` | `setTimeout(..., CLOSE_ANIMATION_MS)` |

### Tailwind Config

```js
// tailwind.config.js or equivalent
tracking: {
  section: '0.2em',  // replaces tracking-[0.2em] in 6 files
}
```

### Files

- `testudo-journal/src/styles/app.css` — add 5 new CSS variables
- `testudo-journal/src/lib/tokens.ts` — new file, single source of truth
- `testudo-journal/src/components/HeroEquityCurve.tsx` — import tokens
- `testudo-journal/src/components/charts/EquityCurve.tsx` — import tokens
- `testudo-journal/src/components/charts/DailyPnl.tsx` — import tokens
- `testudo-journal/src/components/charts/CumulativeProfit.tsx` — import tokens
- `testudo-journal/src/components/charts/SymbolDonut.tsx` — import tokens
- `testudo-journal/src/components/charts/MarketReturn.tsx` — import tokens
- `testudo-journal/src/components/charts/DurationScatter.tsx` — import tokens
- `testudo-journal/src/components/charts/ReturnHistogram.tsx` — import tokens
- `testudo-journal/src/components/trades/TagBadge.tsx` — import TAG_PALETTE
- `testudo-journal/src/components/journal/TagManager.tsx` — import TAG_PALETTE + CLOSE_ANIMATION_MS
- `testudo-journal/src/components/journal/EntryCard.tsx` — import ENTRY_TYPE_COLORS
- `testudo-journal/src/components/journal/EntryEditor.tsx` — import ENTRY_TYPE_COLORS + CLOSE_ANIMATION_MS
- `testudo-journal/src/components/trades/TradeDetail.tsx` — import CLOSE_ANIMATION_MS
- `testudo-journal/src/components/StatSection.tsx` — tracking-section
- `testudo-journal/src/components/StatCard.tsx` — tracking-section
- `testudo-journal/src/components/charts/ChartContainer.tsx` — tracking-section
- `testudo-journal/src/components/Overview.tsx` — tracking-section (skeleton)
- `testudo-journal/tailwind.config.*` — add tracking-section

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `grep -r '#00FF41\|#FF003C\|#111111' testudo-journal/src/components/` returns zero matches (all moved to tokens)
- [ ] `grep -r "tracking-\[0.2em\]" testudo-journal/src/` returns zero matches (all use tracking-section)
- [ ] Signal colors, chart background, and tag palette are each defined in exactly one location
- [ ] All charts render identically before and after (visual regression: same colors, same layouts)
- [ ] `bun run build` passes with zero errors

---

## Risks

1. **Chart.js / lightweight-charts config objects** — These take hex strings, not CSS variables. The `tokens.ts` JS module solves this by exporting string constants. CSS variables are for Tailwind utility classes; JS constants are for chart configs.
2. **SymbolDonut palette divergence was intentional?** — The two swapped colors (#14B8A6 / #F97316) may have been chosen for better contrast in pie chart segments. Unifying to `TAG_PALETTE` is correct unless there's a documented reason for divergence (there isn't).

---

## Completion Signal

This spec is complete when:
1. `tokens.ts` exists with all color constants
2. All 14+ hardcoded color locations import from tokens
3. CSS variables added to app.css
4. tracking-section replaces all tracking-[0.2em] instances
5. `bun run build` passes
6. Code committed to master
