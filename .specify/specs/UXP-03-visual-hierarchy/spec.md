# Specification: Establish Type Scale and Visual Hierarchy

**Spec ID:** UXP-03-visual-hierarchy
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / Typography
**Priority:** P1 — Amplifies the impact of all other UXP changes
**Depends on:** UXP-01-design-system-alignment
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

The journal's entire type scale is compressed into a 10-14px range (ratio 1.4x). The landing page uses a 10px to 96px range (ratio 9.6x). This means the journal has no visual crescendo — every element whispers at the same volume. Page titles, section headers, data labels, and table cells are nearly indistinguishable in size.

The landing page's type scale:
- Hero: `text-5xl md:text-7xl lg:text-8xl` (48-96px)
- Section: `text-2xl md:text-3xl` (24-30px)
- Card heading: `text-3xl` (30px)
- Body: `text-base md:text-lg` (16-18px)
- Nav/captions: `text-xs` (12px)

The journal's type scale:
- Page title: `text-lg` (18px)
- Card title: `text-xs` (12px)
- Data: `text-xs` (12px)
- Table header: `text-[10px]` (10px)

The fix: introduce a proper modular type scale that creates 3-4 distinct hierarchy levels, matching the landing page's dramatic range where appropriate.

---

## User Stories

- **As a trader**, I want the most important numbers to stand out visually, so that I can scan the dashboard quickly.
- **As a trader**, I want page titles to clearly signal which section I'm in, so that navigation context is always clear.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define a 6-level type scale in Tailwind config with named classes | High | Tokens |
| FR-2 | Page titles (OVERVIEW, CHARTS, TRADES, JOURNAL) use `text-2xl md:text-3xl font-display font-bold tracking-tight` | High | Layout |
| FR-3 | Section titles within pages use `text-sm font-display tracking-[0.2em] uppercase text-text-tertiary` (current pattern, preserved) | Medium | Components |
| FR-4 | Hero numbers (P&L, key metrics) use `text-3xl md:text-4xl font-mono font-bold` | High | Components |
| FR-5 | Body text (journal entries, descriptions) use `text-sm font-mono leading-relaxed` (14px, not 12px) | Medium | Components |
| FR-6 | Table data remains `text-xs font-mono` (12px) — dense data is appropriate here | Low | Tables |
| FR-7 | Ghost annotation pattern from landing page: `font-mono text-xs tracking-widest text-text-secondary/70` above section headings, e.g. `// ACCOUNT_OVERVIEW`, `// TRADE_HISTORY` | Medium | Components |

---

## Technical Implementation

### Type Scale Definition

| Level | Name | Classes | Size Range | Usage |
|-------|------|---------|------------|-------|
| 1 | Hero | `text-3xl md:text-4xl lg:text-5xl font-mono font-bold` | 30-48px | P&L total, account balance |
| 2 | Page | `text-2xl md:text-3xl font-display font-bold tracking-tight` | 24-30px | Page titles |
| 3 | Section | `text-sm font-display tracking-[0.2em] uppercase text-text-tertiary` | 14px | Card/section headers |
| 4 | Body | `text-sm font-mono leading-relaxed` | 14px | Descriptions, journal entries |
| 5 | Data | `text-xs font-mono` | 12px | Table cells, stat values, timestamps |
| 6 | Micro | `text-[10px] font-mono tracking-widest uppercase` | 10px | Table headers, badge labels |

### Ghost Annotations

```tsx
function GhostAnnotation(props: { text: string }) {
  return (
    <span class="font-mono text-xs tracking-widest text-text-secondary/50 mb-2 block">
      // {props.text}
    </span>
  );
}
```

Applied above section headings:
- Overview: `// ACCOUNT_OVERVIEW`
- Charts: `// CHART_SUITE`
- Trades: `// TRADE_HISTORY`
- Journal: `// JOURNAL_ENTRIES`

### Files

- `testudo-journal/tailwind.config.ts` — No changes needed (type scale is via utility classes)
- `testudo-journal/src/components/Layout.tsx` — Page titles enlarged
- `testudo-journal/src/components/Overview.tsx` — Hero P&L number, ghost annotation
- `testudo-journal/src/components/Charts.tsx` — Ghost annotation
- `testudo-journal/src/pages/Trades.tsx` — Ghost annotation, page title
- `testudo-journal/src/pages/Journal.tsx` — Ghost annotation, page title
- `testudo-journal/src/components/journal/EntryCard.tsx` — Body text to `text-sm leading-relaxed`
- `testudo-journal/src/components/GhostAnnotation.tsx` — New shared component

---

## Acceptance Criteria

- [ ] Page titles are visually distinct from section titles (minimum 2x size difference)
- [ ] Hero numbers are the largest text on any page they appear on
- [ ] Ghost annotations appear above each main section
- [ ] Journal entry body text uses `text-sm leading-relaxed` (14px)
- [ ] Table data remains dense at `text-xs`
- [ ] Type scale has at least 4 visually distinct levels
- [ ] `bun run build` passes

---

## Risks

1. **Larger page titles may feel inconsistent with the dense data aesthetic** — Mitigation: The landing page proves both scales coexist. Page titles anchor the section; data density lives inside.
2. **Ghost annotations may feel forced if overused** — Mitigation: Limit to one per page, above the main heading only.

---

## Completion Signal

This spec is complete when:
1. All six type scale levels are consistently applied
2. Ghost annotations appear on all four pages
3. Visual hierarchy is clearly scannable (hero > page > section > body > data > micro)
4. `bun run build` passes
5. Code committed to master
