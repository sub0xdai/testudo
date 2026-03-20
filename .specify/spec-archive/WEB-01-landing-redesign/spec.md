# WEB-01: Landing Page Redesign — Nexus/Terminal Aesthetic

**Status:** In Progress
**Component:** testudo-web
**Priority:** P1

## Summary

Restyle the landing page from a clean card-wrapped layout to a dark terminal/HUD aesthetic matching the Nexus reference image. Full-bleed content over background, scan-line overlay, bracket notation, ghost metadata annotations, monospace-first typography.

## Preserved

- `SpotlightBackground` component — same Trajan's column image, same mouse-following spotlight, same z-layering
- Scan-line overlay is additive ON TOP of existing background

## Content Reduction

| Keep | Drop |
|------|------|
| Hero | Problem |
| Features (merged) | Solution |
| Pricing | RiskEngine |
| Footer | HowItWorks |
| | Exchanges |
| | FAQ |
| | FinalCTA |

## Design Elements

- **Full-bleed** — no Card wrappers, content floats over background
- **Bracket notation** — `[Markets]` highlight style
- **Terminal metadata** — ghost annotations (e.g., `// RISK_OVERLAY_ACTIVE`)
- **Scan-line overlay** — repeating 2px horizontal lines at low opacity
- **Outlined CTAs** — bordered buttons, no fills
- **Data tickers** — decorative BTC/ETH/SOL price block in hero
- **Monospace-first** — `font-mono` dominant, display font only for hero headline

## Files Modified

| File | Change |
|------|--------|
| `src/index.css` | Scan-line CSS, flicker animation |
| `tailwind.config.js` | Animation keyframes |
| `src/pages/LandingPage.tsx` | Remove 7 sections, add Features |
| `src/components/ui/SpotlightBackground.tsx` | Add scan-line overlay |
| `src/components/ui/Header.tsx` | Terminal nav, outlined CTA |
| `src/components/sections/Hero.tsx` | Full rewrite |
| `src/components/sections/Features.tsx` | NEW |
| `src/components/sections/Pricing.tsx` | Remove Card, terminal style |
| `src/components/sections/Footer.tsx` | Remove Card, minimal |

## Files Deleted

Problem.tsx, Solution.tsx, RiskEngine.tsx, HowItWorks.tsx, Exchanges.tsx, FAQ.tsx, FinalCTA.tsx

## Acceptance Criteria

- [ ] Scan-line overlay visible across full page
- [ ] Hero has bracket notation, ghost metadata, data tickers
- [ ] No Card wrappers on any section
- [ ] Features section replaces 4 dropped sections
- [ ] 7 unused section files deleted
- [ ] `bun run build` passes
