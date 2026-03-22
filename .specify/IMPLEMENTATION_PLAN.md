# Implementation Plan

> Last updated: 2026-03-22
> Current spec: UXP-20-strip-glassmorphism
> Phase: BUILD

---

## Active Spec: UXP-20-strip-glassmorphism

Remove all `backdrop-blur` glassmorphism from landing page. Increase background opacity to 95%.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Remove backdrop-blur from all landing page components, increase opacity to 95% | complete | simple | — |

### Key Decisions

- **Glass variant removed from Card.tsx**: No callers used `variant="glass"` — removed the variant entirely instead of making it match solid.
- **Features.tsx already clean**: UXP-19 layout refactor had already removed glassmorphism from Features section.
- **Header at 90% opacity**: Spec-recommended value for fixed header — sufficient to obscure scrolling content without blur.

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
| UXP-20-strip-glassmorphism | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
