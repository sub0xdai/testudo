# Implementation Plan

> Last updated: 2026-03-22
> Current spec: UXP-19-features-layout
> Phase: BUILD

---

## Active Spec: UXP-19-features-layout

Break features section out of uniform card grid into asymmetric hero + compact layout.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Replace uniform 2-col grid with hero feature + compact border-left secondary list | complete | simple | — |

### Key Decisions

- **RISK ENGINE as hero**: Core value prop (automated position sizing) — gets full-width treatment with `border-text-primary` (brighter than `border-container-border`), `font-display` heading, and ghost annotation referencing actual codebase (`position_sizer.rs`).
- **Option A chosen**: Hero + compact list over terminal/log aesthetic. Terminal style would clash with the ghost annotations already present — two competing metaphors. Hero + compact gives clear hierarchy without adding a new visual language.
- **No backdrop-blur**: Removed per UXP-20 cross-reference in acceptance criteria. Solid backgrounds only.
- **font-mono preserved on descriptions**: UXP-23 will handle the mono→display typography migration. This change is layout-only.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HL-11-status-transition-fix | 2026-03-21 |
| UXP-18-multi-theme | 2026-03-21 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
