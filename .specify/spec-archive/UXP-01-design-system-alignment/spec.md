# Specification: Align Journal Design System with Landing Page

**Spec ID:** UXP-01-design-system-alignment
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / Design System
**Priority:** P0 — Foundation for all subsequent UXP specs; mismatched tokens block visual coherence
**Depends on:** None (first in series)
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

The testudo-journal app diverged from the testudo-web landing page's design language during rapid feature development. While both apps share the same color hex values and Google Font imports, the journal uses a compressed, uniform visual style that lacks the landing page's distinctive personality — its ghost annotations, bracket conventions, button inversion patterns, scan-line texture, and dramatic type scale.

The landing page's primary CTA is a white-bordered button that inverts on hover (`hover:bg-text-primary hover:text-main-bg`). The journal's primary CTA is a green ghost button (`bg-signal-green/10 border-signal-green`). The landing page uses `font-mono text-2xl md:text-3xl` for section headings; the journal never exceeds `text-lg`. The landing page reserves brackets for nav buttons (`[ LOGIN ]`); the journal uses them for inline text actions (`[Edit]`, `[Delete]`).

This spec unifies the token system, button vocabulary, and typographic conventions so the journal feels like part of the same product.

---

## User Stories

- **As a trader**, I want the journal to feel like the same app I signed up on, so that the experience feels cohesive and trustworthy.
- **As a developer**, I want a single shared design token set, so that changes propagate across both apps without drift.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extract shared design tokens into a common Tailwind preset importable by both testudo-web and testudo-journal | High | Build |
| FR-2 | Unify button vocabulary: primary = white-border invert on hover, filled = signal-green (auth/destructive only), secondary = dim-border upgrade on hover, tertiary = `[bracket]` text actions | High | Components |
| FR-3 | Add `accent-steel` (#94A3B8) and `accent-steel-hover` (#CBD5E1) tokens to journal palette | High | Tokens |
| FR-4 | Remove dead `panel-bg` token (duplicate of `container-bg`) | Low | Tokens |
| FR-5 | Add `signal-amber` (#F59E0B) token to replace hardcoded `text-orange-400` in formatters.ts | Medium | Tokens |
| FR-6 | Match border-radius convention: default 4px (`rounded`), auth forms `rounded-md`, cards sharp (no radius) on data-dense views | Medium | Components |
| FR-7 | Standardize focus rings: `focus-visible:ring-2 ring-signal-green/30 ring-offset-1 ring-offset-main-bg` replacing bare `focus:border-signal-green outline-none` | High | Components |

---

## Technical Implementation

### Shared Tailwind Preset

Create a shared preset file that both apps import:

```typescript
// packages/tailwind-preset/index.ts
export default {
  theme: {
    extend: {
      colors: {
        'main-bg': '#050505',
        'container-bg': '#0A0A0A',
        'container-bg-hover': '#111111',
        'elevated': '#111111',
        'container-border': '#3F3F46',
        'border-active': '#FFFFFF',
        'accent-steel': '#94A3B8',
        'accent-steel-hover': '#CBD5E1',
        'signal-green': '#00FF41',
        'signal-red': '#FF003C',
        'signal-amber': '#F59E0B',
        'text-primary': '#FFFFFF',
        'text-secondary': '#888888',
        'text-tertiary': '#555555',
      },
      fontFamily: {
        display: ['Space Grotesk', 'system-ui', 'sans-serif'],
        mono: ['Space Mono', 'JetBrains Mono', 'monospace'],
      },
    },
  },
}
```

### Button Hierarchy

| Level | Pattern | Usage |
|-------|---------|-------|
| **Primary** | `border border-text-primary text-text-primary hover:bg-text-primary hover:text-main-bg` | Main page actions (Apply filter, Save) |
| **Filled** | `bg-signal-green text-main-bg font-bold hover:bg-white` | Auth actions, irreversible confirms |
| **Secondary** | `border border-container-border text-text-secondary hover:border-text-primary hover:text-text-primary` | Alternative actions (Clear, Cancel) |
| **Tertiary** | `font-mono text-text-tertiary hover:text-text-primary` with `[bracket]` notation | Inline actions (Edit, Delete, Close) |

### Files

- `packages/tailwind-preset/index.ts` — New shared preset
- `testudo-journal/tailwind.config.ts` — Import preset, remove inline tokens
- `testudo-web/tailwind.config.js` — Import preset, remove inline tokens
- `testudo-journal/src/lib/formatters.ts` — Replace `text-orange-400` with `text-signal-amber`
- `testudo-journal/src/components/FilterBar.tsx` — Update APPLY button to primary style
- `testudo-journal/src/components/journal/JournalTimeline.tsx` — Update "+ New Entry" to primary style
- `testudo-journal/src/components/journal/EntryEditor.tsx` — Update Save to primary style, Close to tertiary

---

## Acceptance Criteria

- [ ] Both apps compile with shared preset (`bun run build` in each)
- [ ] No hardcoded color values outside design tokens (grep for raw hex in component files)
- [ ] Button hierarchy follows the four-level system consistently
- [ ] `panel-bg` token removed, no references remain
- [ ] `signal-amber` token used for break-even R-multiples
- [ ] Focus rings use `focus-visible:ring` pattern on all inputs
- [ ] `accent-steel` token available in journal

---

## Risks

1. **Shared preset complicates monorepo build** — Mitigation: Use a simple relative import rather than a published package. Both apps already share a git root.
2. **Button style change breaks muscle memory** — Mitigation: The journal is pre-release; no users to disrupt.

---

## Completion Signal

This spec is complete when:
1. Shared Tailwind preset exists and is imported by both apps
2. All button styles follow the unified hierarchy
3. All design tokens are centralized with no dead aliases
4. `bun run build` passes in both testudo-web and testudo-journal
5. Code committed to master
