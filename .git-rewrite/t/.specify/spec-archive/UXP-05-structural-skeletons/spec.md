# Specification: Replace Pulse Blocks with Structural Skeleton Loaders

**Spec ID:** UXP-05-structural-skeletons
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / Loading States
**Priority:** P1 — Generic pulse blocks are the laziest loading pattern; structural skeletons reduce perceived wait time
**Depends on:** UXP-02-overview-hero-layout (skeleton must match new layout)
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

Every loading state in the journal uses the same pattern: `animate-pulse` on gray rectangles. The Overview shows three identical gray blocks. The Charts show "LOADING..." text. The TradeTable shows 10 rows of identical bars. The Journal shows three featureless cards. None of these convey the structure of what's loading.

Structural skeletons mirror the shape of the content they replace — a chart outline that fills in, a table grid with column-width bars, a stat list with label-width and value-width placeholders. This reduces perceived load time because the user's brain pre-processes the layout before data arrives.

For the terminal aesthetic, the skeleton should feel like "data incoming" — perhaps a subtle scan-line effect or sequential row revelation instead of generic pulse.

---

## User Stories

- **As a trader**, I want loading states to show me the structure of what's coming, so that the interface feels fast and predictable.
- **As a trader**, I want the loading animation to match the app's aesthetic, so that it feels intentional rather than generic.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Overview skeleton mirrors the 2-column layout: left sidebar with labeled row placeholders, right area with chart outline and hero number placeholder | High | Overview |
| FR-2 | TradeTable skeleton shows column headers (real headers, not placeholders) with shimmer bars at correct column widths | High | Trades |
| FR-3 | Chart skeletons show an empty chart frame (axis lines, grid pattern) with a subtle shimmer, not "LOADING..." text | High | Charts |
| FR-4 | Journal timeline skeleton shows 2-3 entry card outlines with left-border accent, type badge placeholder, and line placeholders | Medium | Journal |
| FR-5 | TradeDetail skeleton shows the panel structure: header bar, price grid, P&L grid, tags row — all as appropriately-sized shimmer bars | Medium | Trades |
| FR-6 | Skeleton shimmer uses a sweep animation (left-to-right gradient pass) instead of uniform pulse | Medium | All |
| FR-7 | Skeleton colors use `container-border/20` for bars and `container-border/10` for backgrounds — subtler than current `container-border/20` blocks | Low | All |

---

## Technical Implementation

### Shimmer Animation

Replace `animate-pulse` with a sweep shimmer:

```css
@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

.skeleton-shimmer {
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(63, 63, 70, 0.15) 50%,
    transparent 100%
  );
  background-size: 200% 100%;
  animation: shimmer 1.5s ease-in-out infinite;
}
```

### Skeleton Bar Component

```tsx
function SkeletonBar(props: { width?: string; height?: string; class?: string }) {
  return (
    <div
      class={`bg-container-border/15 rounded skeleton-shimmer ${props.class ?? ''}`}
      style={{ width: props.width ?? '100%', height: props.height ?? '12px' }}
    />
  );
}
```

### Overview Skeleton (Post UXP-02 Layout)

```tsx
function OverviewSkeleton() {
  return (
    <div class="flex gap-6">
      {/* Stats sidebar */}
      <div class="w-64 border-r border-container-border">
        <For each={['ACCOUNT', 'PERFORMANCE', 'RISK']}>
          {(section) => (
            <div class="px-4 py-3 border-b border-container-border">
              <span class="font-display text-xs tracking-[0.2em] text-text-tertiary uppercase">
                {section}
              </span>
              <div class="mt-3 space-y-2">
                <For each={Array(4)}>
                  {() => (
                    <div class="flex justify-between">
                      <SkeletonBar width="60px" />
                      <SkeletonBar width="80px" />
                    </div>
                  )}
                </For>
              </div>
            </div>
          )}
        </For>
      </div>
      {/* Hero area */}
      <div class="flex-1">
        <SkeletonBar width="200px" height="40px" class="mb-4" />
        <div class="h-[400px] border border-container-border/30 rounded relative">
          {/* Axis lines */}
          <div class="absolute left-8 top-4 bottom-8 w-px bg-container-border/20" />
          <div class="absolute left-8 right-4 bottom-8 h-px bg-container-border/20" />
          <div class="absolute inset-0 skeleton-shimmer rounded" />
        </div>
      </div>
    </div>
  );
}
```

### Chart Skeleton

```tsx
function ChartSkeleton(props: { height?: string }) {
  return (
    <div class={`relative ${props.height ?? 'h-[250px]'}`}>
      {/* Y-axis ticks */}
      <div class="absolute left-0 top-0 bottom-6 w-8 flex flex-col justify-between">
        <SkeletonBar width="30px" height="8px" />
        <SkeletonBar width="24px" height="8px" />
        <SkeletonBar width="28px" height="8px" />
      </div>
      {/* Chart area with grid */}
      <div class="ml-10 h-full border-l border-b border-container-border/20 relative">
        <div class="absolute inset-0 skeleton-shimmer" />
      </div>
    </div>
  );
}
```

### Files

- `testudo-journal/src/styles/app.css` — Add `@keyframes shimmer` and `.skeleton-shimmer` class
- `testudo-journal/src/components/SkeletonBar.tsx` — New shared skeleton primitive
- `testudo-journal/src/components/Overview.tsx` — Replace pulse blocks with structural skeleton
- `testudo-journal/src/components/charts/ChartContainer.tsx` — Replace "LOADING..." with chart skeleton
- `testudo-journal/src/components/trades/TradeTable.tsx` — Replace row-bar skeleton with column-aware skeleton
- `testudo-journal/src/components/trades/TradeDetail.tsx` — Replace bar skeleton with structured skeleton
- `testudo-journal/src/components/journal/JournalTimeline.tsx` — Replace card skeleton with entry-shaped skeleton

---

## Acceptance Criteria

- [ ] All loading states show structural skeletons that mirror the loaded content's layout
- [ ] No remaining `animate-pulse` usage in the app (search for it)
- [ ] Shimmer animation sweeps left-to-right at 1.5s interval
- [ ] Chart skeletons show axis outlines
- [ ] Table skeleton shows real column headers with appropriately-sized bars below
- [ ] Journal skeleton shows entry card outlines with left-border accents
- [ ] Skeleton colors are subtler than content borders (`container-border/15`)
- [ ] `bun run build` passes

---

## Risks

1. **Skeleton maintenance burden** — Every layout change requires skeleton update. Mitigation: Keep skeletons simple (bars + structure), not pixel-perfect mirrors.
2. **Shimmer animation performance** — CSS `background-position` animation is GPU-composited and cheap. No risk.

---

## Completion Signal

This spec is complete when:
1. All five loading contexts (Overview, Charts, TradeTable, TradeDetail, Journal) use structural skeletons
2. `animate-pulse` has zero occurrences in the codebase
3. Shimmer animation is smooth at 60fps
4. `bun run build` passes
5. Code committed to master
