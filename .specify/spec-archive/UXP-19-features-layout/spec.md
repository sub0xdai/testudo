# Specification: Break Features Section Out of Uniform Card Grid

**Spec ID:** UXP-19-features-layout
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / Visual Design
**Priority:** P1 — Features section is the single most AI-templated element on an otherwise distinctive landing page
**Depends on:** None (first in series)
**Series:** UXP-19 through UXP-23 (Design critique remediation)

---

## Problem Statement

The Features section in `testudo-web/src/components/sections/Features.tsx` renders a uniform 2-column grid of identically-sized bordered cards (`grid md:grid-cols-2 gap-x-12 gap-y-8`, line 33). Each card uses the same structure: border, semi-opaque background with blur, consistent padding (`border border-container-border bg-main-bg/90 backdrop-blur-sm p-5`, line 35), icon, heading, description.

In a landing page that commits to brutalist aesthetics (zero-radius, CRT scan lines, spotlight tracking, monochrome chrome), this uniform card grid is the one element that reads as templated. It violates the design's own ethos — nothing has visual hierarchy within the section. A user scanning it processes zero individual features because every card competes equally for attention.

The fix is to break the grid and establish visual weight. The most important 1-2 features should command more space, while secondary features can be presented more compactly. The section should feel designed, not generated.

---

## User Stories

- **As a visitor**, I want the most important product capabilities to stand out visually, so that I immediately understand the product's core value proposition.
- **As a designer**, I want the features section to match the brutalist confidence of the rest of the landing page, so that the overall aesthetic is cohesive.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Primary feature(s) occupy a full-width row with asymmetric layout (image/diagram + text) | High | Features.tsx |
| FR-2 | Secondary features use a compact presentation (not identical cards) — e.g., tight list, inline descriptions, or varied card sizes | High | Features.tsx |
| FR-3 | Visual hierarchy is clear: primary features are immediately scannable in <2 seconds | High | Features.tsx |
| FR-4 | Section maintains zero-radius, monochrome-first aesthetic | High | Features.tsx |
| FR-5 | Section is responsive (single-column on mobile, asymmetric on md+) | Medium | Features.tsx |

---

## Technical Implementation

### Layout Direction

Replace the uniform `grid md:grid-cols-2` with an asymmetric composition. Options (pick one during implementation):

**Option A — Hero Feature + Compact List:**
```tsx
// Primary feature: full-width annotated block
<div className="border border-container-border p-8 mb-8">
  <h3 className="font-display text-2xl mb-4">{primaryFeature.title}</h3>
  <p className="text-text-secondary text-sm">{primaryFeature.description}</p>
</div>

// Secondary features: tight grid or list with minimal padding
<div className="grid md:grid-cols-3 gap-4">
  {secondaryFeatures.map(f => (
    <div className="border-l border-container-border pl-4 py-2">
      <h4 className="text-xs tracking-widest uppercase text-text-secondary">{f.title}</h4>
      <p className="text-sm text-text-tertiary mt-1">{f.description}</p>
    </div>
  ))}
</div>
```

**Option B — Terminal/Log Aesthetic:**
```tsx
// Render features as monospace command output
<div className="font-mono text-sm border border-container-border p-6">
  {features.map((f, i) => (
    <div className="flex gap-4 py-2 border-b border-container-border/30 last:border-0">
      <span className="text-text-tertiary w-8">{String(i).padStart(2, '0')}</span>
      <span className="text-text-primary">{f.title}</span>
      <span className="text-text-tertiary ml-auto">{f.shortDesc}</span>
    </div>
  ))}
</div>
```

### Files

- `testudo-web/src/components/sections/Features.tsx` — replace grid layout and card structure

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Features section no longer uses identical-sized card grid
- [ ] At least one feature has visually dominant presentation (more space, different layout)
- [ ] Secondary features are differentiated from primary (size, padding, or structure)
- [ ] Zero-radius aesthetic maintained (no rounded corners introduced)
- [ ] No `backdrop-blur` on feature elements (see UXP-20)
- [ ] Responsive: works on mobile (single-column) through desktop
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **Content restructuring** — Features may need rewriting to work in asymmetric layout. Mitigation: work with existing copy first; flag if content needs updating.
2. **Spacing regression** — Changing grid to flexbox or mixed layout may introduce inconsistent vertical rhythm. Mitigation: use Tailwind spacing scale consistently (4px increments).

---

## Completion Signal

This spec is complete when:
1. Features section renders with clear visual hierarchy (primary vs secondary)
2. Layout is asymmetric or varied (not uniform grid)
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
