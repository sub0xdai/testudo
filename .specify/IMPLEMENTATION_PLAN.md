# Implementation Plan

> Last updated: 2026-03-24
> Current spec: —
> Phase: IDLE

---

## Active Spec: EXT-41-desk-dashboard

Redesign Desk dashboard for high-signal analytics — glass panels, always-visible time presets, metric deduplication, semantic coloring, embedded chart controls.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Glass panel styling — `.glass-panel` CSS utility + applied to sidebar, hero, chart containers. | complete | low | — |
| T2 | Always-visible time presets — moved from FilterPopout to PageSubHeader. | complete | medium | — |
| T3 | Metric consolidation + semantic coloring — hero sub-row removed, PF/Exp/R-Mult colored. | complete | low | T1 |
| T4 | Embedded chart controls — dropdown in glass-panel header bar (done with T1). | complete | medium | T1 |
| T5 | Build validation — 11/11 acceptance criteria verified, build passes. | complete | low | T1, T2, T3, T4 |

### Key Decisions

- **No new components needed**: All changes are modifications to existing files (Overview.tsx, PageSubHeader.tsx, FilterPopout.tsx, ChartSelector.tsx, app.css). The spec's proposal for MasterCommandBar.jsx and ChartPanel.jsx is unnecessary — PageSubHeader already IS the command bar, ChartSelector already IS the chart panel.
- **Preset state stays local, not in FilterContext**: PageSubHeader's preset signal derives dateFrom/dateTo and calls `setFilters()`. No need to pollute the global filter context with UI state. FilterPopout currently tracks preset locally too — same pattern.
- **`.glass-panel` CSS utility vs inline classes**: A single reusable class keeps the 5+ application points DRY and ensures light/dark theme consistency. Defined in app.css with CSS custom property for backdrop color.
- **Hero sub-row removal doesn't affect mobile**: Mobile condensed strip (lines 119-134) is independent (`md:hidden`). Desktop hero sub-row (lines 167-181) is `hidden md:flex`. Removing the desktop sub-row leaves mobile intact.
- **ECharts resize is safe in flex containers**: All ECharts components use `EChart.tsx` which attaches ResizeObserver. DailyPnl and CumulativeProfit (lightweight-charts) also have independent ResizeObservers. No additional work needed.
- **Semantic coloring for Profit Factor**: Use threshold-based coloring (>1 green, <1 red, =1 neutral) not `pnlColor()` which checks positive/negative. This is a distinct semantic meaning.

### Discoveries

- **`pnlColor()` and `rColor()` already exist** in `lib/formatters.ts` — no new formatting utilities needed.
- **StatSection already supports `colorClass` prop** — infrastructure is ready for FR-4.
- **Preset state is local to FilterPopout** (line 46: `createSignal<Preset>('all')`) — moving to PageSubHeader requires lifting the preset computation logic (`computeDateFrom()`) or importing it.
- **ChartContainer.tsx has title header** (h3) but ChartSelector doesn't use it — ChartSelector renders its own dropdown wrapper. The refactor merges these: dropdown replaces the static title in the glass panel header.
- **Layout.tsx background**: Fixed image with `.bg-overlay` at 88%/82% opacity. Glass panels will add a second layer of visual separation — the existing overlay dims, glass panels blur.
- **Header already uses `backdrop-blur-sm`** — confirms Tailwind backdrop-blur works in this build pipeline.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| EXT-41-desk-dashboard | 2026-03-24 |
| EXT-40-smart-card-grid | 2026-03-24 |
| EXT-39-pair-ux | 2026-03-24 |
| AUTH-03-frontend-auth | 2026-03-24 |
| AUTH-02-backend-auth | 2026-03-24 |
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
