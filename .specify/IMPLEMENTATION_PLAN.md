# Implementation Plan

> Last updated: 2026-03-22
> Current spec: UXP-21-light-theme-parity
> Phase: BUILD

---

## Active Spec: UXP-21-light-theme-parity

Give light theme atmospheric parity with dark — spotlight, texture overlay, heavier borders, dynamic RainbowKit theme.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Create ThemeContext, lift theme state from Header, wire dynamic RainbowKit theme | complete | medium | — |
| T2 | Enable light theme atmospheric effects — spotlight, texture overlay, heavier borders, background visibility | complete | medium | — |

### Key Decisions

- **ThemeContext created**: Lifted theme state from Header.tsx to a new ThemeContext.tsx. Header was managing its own state with getStoredTheme/applyTheme/cycleTheme — all moved to context so RainbowKitProvider can consume the reactive theme value.
- **RainbowKitThemeWrapper component**: Created a wrapper inside main.tsx that reads `useTheme()` and computes the RainbowKit theme prop. RainbowKitProvider must be a child of ThemeProvider for this to work. Light theme uses `borderRadius: 'none'` to match brutalist aesthetic.
- **Light theme accent color**: Dark uses `#22C55E` (signal green), light uses `#146426` (darker forest green) for better contrast on cream backgrounds.
- **Spotlight enabled in both themes**: Removed the `isLight` conditional that disabled mouse tracking. Light spotlight uses 0.70→0.92 opacity range (darker center, lighter edge) vs dark's transparent→0.95.
- **Texture overlay replaces conditional scan-lines**: Instead of `!isLight && <div className="scan-lines" />`, now renders `isLight ? 'texture-grain' : 'scan-lines'` unconditionally. Light texture uses `--text-primary` at 4% opacity for paper grain effect.
- **Heavier borders via CSS overrides**: Used `[data-theme="light"] .border { border-width: 2px; }` targeting Tailwind utilities. Simpler than custom properties since no component changes needed.

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
| UXP-21-light-theme-parity | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
