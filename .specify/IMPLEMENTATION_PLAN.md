# Implementation Plan

> Last updated: 2026-03-22
> Current spec: UXP-22-signal-color-calibration
> Phase: BUILD

---

## Active Spec: UXP-22-signal-color-calibration

Recalibrate dark theme signal green (#00FF41→#22C55E) and red (#FF003C→#EF4444) across all surfaces.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Update all signal color definitions across extension, web, and journal | complete | simple | — |

### Key Decisions

- **#22C55E for green**: Tailwind green-500. Saturation 72% vs 100%. Still vivid, no halation on AMOLED.
- **#EF4444 for red**: Tailwind red-500. Saturation 70% vs 100%. Unmistakably red.
- **Journal included**: Spec only mentioned extension+web, but journal shares the same design tokens. Updating for consistency.
- **Light theme unchanged**: Already uses reduced-saturation values (#146426, #a00024).

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HL-11-status-transition-fix | 2026-03-21 |
| UXP-18-multi-theme | 2026-03-21 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
