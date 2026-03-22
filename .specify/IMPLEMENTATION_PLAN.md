# Implementation Plan

> Last updated: 2026-03-22
> Current spec: UXP-23-landing-typography
> Phase: BUILD

---

## Active Spec: UXP-23-landing-typography

Reserve monospace for data/annotations, use Space Grotesk (font-display) for landing page body text.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Replace font-mono with font-display on all body/paragraph text in Hero, Features, and Pricing | complete | simple | — |

### Key Decisions

- **Pricing feature list items changed**: List items like "Risk engine + position sizing" are persuasive body copy, not data — switched to font-display.
- **Section headings kept mono**: Short headings like "CORE [SYSTEMS]" and "[PRICING]" are terminal-aesthetic titles, not paragraph text.
- **Feature labels kept mono**: Short labels like "DEX + CEX" serve as data identifiers, not body copy.

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
| UXP-23-landing-typography | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
