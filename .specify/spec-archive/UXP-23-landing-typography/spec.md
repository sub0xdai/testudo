# Specification: Reserve Monospace for Data, Use Display Font for Landing Body Text

**Spec ID:** UXP-23-landing-typography
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / Visual Design
**Priority:** P2 — Readability improvement for marketing surface; product surfaces unaffected
**Depends on:** None
**Series:** UXP-19 through UXP-23 (Design critique remediation)

---

## Problem Statement

The landing page uses Space Mono (`font-mono`) for all body text — taglines, feature descriptions, pricing copy, and section subtitles. Monospace fonts have uniform character widths that reduce reading speed by ~10-15% compared to proportional fonts. This is the correct choice for the extension popup (trading terminal context, short data labels) but wrong for the landing page, which is a marketing surface where paragraphs need to persuade.

Current monospace body text locations on the landing page:

| File | Line | Text | Class |
|------|------|------|-------|
| `testudo-web/src/components/sections/Hero.tsx` | 19-20 | Main tagline ("Adapt to the chaos...") | `font-mono text-base md:text-lg` |
| `testudo-web/src/components/sections/Features.tsx` | 39-40 | Feature descriptions (all cards) | `font-mono text-sm` |
| `testudo-web/src/components/sections/Pricing.tsx` | 54-55 | Pricing subtitle ("Two tiers...") | `font-mono text-sm` |

Space Mono should be reserved for elements where the terminal aesthetic is intentional and text is short: the ghost metadata comments (`// SYSTEM_CAPABILITIES`), the price ticker data, code-like annotations, and inline data values. Space Grotesk (`font-display`) is already the heading font and reads significantly better at body sizes.

---

## User Stories

- **As a visitor**, I want body text on the landing page to be comfortable to read, so that I can quickly understand the product value proposition.
- **As a designer**, I want monospace typography used purposefully (data, annotations, code) not as a blanket "technical" aesthetic, so that the typography system has semantic meaning.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Hero tagline paragraph uses `font-display` (Space Grotesk) instead of `font-mono` | High | Hero.tsx |
| FR-2 | Feature card description text uses `font-display` instead of `font-mono` | High | Features.tsx |
| FR-3 | Pricing subtitle and body copy uses `font-display` instead of `font-mono` | High | Pricing.tsx |
| FR-4 | Ghost metadata comments (e.g., `// SYSTEM_CAPABILITIES`) remain in `font-mono` | High | Features.tsx |
| FR-5 | Price ticker data remains in `font-mono` | High | Hero.tsx |
| FR-6 | Form labels on auth pages remain in `font-mono` (if using terminal aesthetic intentionally) | Medium | Login/Register pages |

---

## Technical Implementation

### Class Replacements

Simple find-and-replace within each component:

**Hero.tsx** (line ~19-20):
```tsx
// Before
<p className="font-mono text-base md:text-lg text-text-secondary ...">

// After
<p className="font-display text-base md:text-lg text-text-secondary ...">
```

**Features.tsx** (line ~39-40):
```tsx
// Before — feature description
<p className="font-mono text-sm text-text-secondary ...">

// After
<p className="font-display text-sm text-text-secondary ...">
```

**Pricing.tsx** (line ~54-55):
```tsx
// Before — pricing subtitle
<p className="font-mono text-sm text-text-secondary ...">

// After
<p className="font-display text-sm text-text-secondary ...">
```

### What Stays Monospace

These elements should NOT be changed — they use monospace intentionally:

- Ghost metadata: `// CORE_LOOP`, `// SYSTEM_CAPABILITIES` annotations
- Price ticker: BTC/ETH/SOL price data in Hero.tsx
- Feature section header: `// SYSTEM_CAPABILITIES` comment
- Footer links (if styled as terminal output)
- Any element displaying numeric data, prices, or code-like content
- Extension popup (entire surface is terminal-context — out of scope)

### Files

- `testudo-web/src/components/sections/Hero.tsx` — tagline font change
- `testudo-web/src/components/sections/Features.tsx` — description font change
- `testudo-web/src/components/sections/Pricing.tsx` — subtitle/body font change

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Hero tagline paragraph renders in Space Grotesk
- [ ] Feature descriptions render in Space Grotesk
- [ ] Pricing body text renders in Space Grotesk
- [ ] Ghost metadata comments still render in Space Mono
- [ ] Price ticker still renders in Space Mono
- [ ] No `font-mono` on paragraph-length text (>20 words) on landing page
- [ ] Extension popup typography unchanged (out of scope)
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **Visual weight shift** — Space Grotesk at the same size as Space Mono will appear slightly larger due to proportional spacing. Mitigation: may need to adjust `text-base` to `text-sm` in some locations; test visually.
2. **Aesthetic coherence** — Mixing two fonts more aggressively may reduce the monolithic "terminal" feel. Mitigation: the headings are already in Space Grotesk; this extends a pattern that already exists. The code annotations in mono provide enough terminal character.

---

## Completion Signal

This spec is complete when:
1. All landing page body text (paragraphs, descriptions, subtitles) uses Space Grotesk
2. Monospace reserved for data displays, annotations, and code-like elements
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
