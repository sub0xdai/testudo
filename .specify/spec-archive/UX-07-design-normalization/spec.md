# Specification: Eliminate Design Drift — Unified Tokens, Button Hierarchy, Surface System

**Spec ID:** UX-07-design-normalization
**Date:** 2026-04-15
**Status:** Complete
**Class:** Refactor / Design System
**Priority:** P1 — visual inconsistency erodes brand identity and confuses action hierarchy
**Depends on:** None (first in series)
**Series:** UX-07 (single spec, may spawn follow-ups for component extraction)

---

## Problem Statement

The three Testudo frontend surfaces (journal/desk, landing/web, extension) each define their own design tokens with subtle but measurable drift. The extension uses a warm terra cotta accent (`#c4735a`) that exists nowhere else. Text secondary/tertiary values diverge across surfaces. The extension uses hex values while journal/web use space-separated RGB. Font stacks differ — journal uses Space Grotesk + Space Mono, web uses `system-ui`, extension uses a custom sans.

More critically, every interactive element across all surfaces uses the same visual treatment: `border + font-mono text-xs tracking-wider + invert-on-hover`. There is no button hierarchy. Primary CTAs ("Connect Exchange"), secondary actions ("Import"), tertiary controls ("Cancel"), and destructive confirms ("Delete") are all rendered at identical visual weight. When everything is emphasized, nothing is.

Additionally, `glass-panel` with `backdrop-filter: blur(12px)` is applied inconsistently — only on exchange cards and modals — and conflicts with the brutalist sharp-corner identity. The `rounded` CSS utility appears on some inputs and toolbar buttons but not on cards, creating fractured border-radius semantics.

This spec normalizes all three surfaces to a single token set, establishes a 3-tier button hierarchy, removes glass/blur effects, and resolves border-radius inconsistencies.

---

## User Stories

- **As a trader**, I want the primary action on each screen to be visually obvious, so that I can act quickly without scanning for the right button.
- **As a user** moving between extension and desk, I want the visual language to feel like one product, so that context-switching is seamless.
- **As the developer**, I want a single source of truth for design tokens, so that palette changes propagate everywhere without manual sync.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Unify design tokens: single canonical set of CSS custom properties used by journal, web, and extension | High | All frontends |
| FR-2 | Remove terra cotta accent (`#c4735a`) from extension; replace with `accent-steel` from shared tokens | High | Extension |
| FR-3 | Establish 3-tier button hierarchy: primary (filled), secondary (border), ghost (text-only) | High | All frontends |
| FR-4 | Primary buttons use `font-display` (Space Grotesk); secondary/ghost keep `font-mono` | High | All frontends |
| FR-5 | Remove `glass-panel` class and all `backdrop-filter: blur()` usage from cards and containers | High | Journal, Extension |
| FR-6 | Resolve `rounded` inconsistency: remove `rounded` from all interactive elements, keep `rounded-full` only for circular indicators (color dots, status badges) | Medium | Journal |
| FR-7 | Limit 1 primary button per view/section; all other actions must be secondary or ghost | Medium | All frontends |
| FR-8 | Differentiate disabled state beyond `opacity-50` — use `border-dashed` + `text-tertiary` + `cursor-not-allowed` | Medium | All frontends |
| FR-9 | Extension font stack aligned: `Space Grotesk` for display, `Space Mono` for mono — matching journal | Medium | Extension |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Canonical token file + journal integration | Journal renders correctly with shared tokens, no visual diff on dark/light themes |
| CP-2 | Extension token migration + terra cotta removal | Extension popup matches journal palette, no warm accent visible |
| CP-3 | Web token migration | Landing page and pricing use shared tokens |
| CP-4 | Button hierarchy across journal | 3-tier buttons in Account, Trades, Journal pages; 1 primary per view |
| CP-5 | Button hierarchy in extension + web | Extension popup and landing CTAs use correct tiers |
| CP-6 | Glass/blur removal + radius normalization | No `backdrop-filter`, no stray `rounded` on non-circular elements |

### Canonical Token Set

Create `shared/design-tokens.css` (or inline into each app's global CSS with identical values — given the different build systems, inlining is more practical). The canonical values:

```css
/* ── Dark theme (default) ─────────────────────────── */
:root, [data-theme="amoled"] {
  color-scheme: dark;
  --bg-core: 9 10 13;
  --bg-panel: 19 21 26;
  --bg-elevated: 26 28 35;
  --bg-hover: 35 38 46;
  --border: 45 48 58;
  --border-active: 237 237 237;
  --accent-steel: 148 163 184;
  --accent-steel-hover: 203 213 225;
  --accent-primary: 180 190 200;
  --signal-green: 34 197 94;
  --signal-red: 239 68 68;
  --signal-amber: 245 158 11;
  --text-primary: 235 237 242;
  --text-secondary: 185 190 200;
  --text-tertiary: 115 120 130;
}

/* ── Light theme ──────────────────────────────────── */
[data-theme="light"] {
  color-scheme: light;
  --bg-core: 241 243 247;
  --bg-panel: 255 255 255;
  --bg-elevated: 234 236 242;
  --bg-hover: 225 228 236;
  --border: 195 200 212;
  --border-active: 10 15 30;
  --accent-steel: 55 65 81;
  --accent-steel-hover: 35 45 60;
  --accent-primary: 80 90 108;
  --signal-green: 5 150 60;
  --signal-red: 200 20 25;
  --signal-amber: 180 110 0;
  --text-primary: 10 12 20;
  --text-secondary: 55 62 78;
  --text-tertiary: 95 102 118;
}
```

Use the **journal's** current values as canonical (they're the most refined after 15+ palette commits in April). The web and extension must adopt these exact values.

#### Extension-specific migration

The extension uses Tailwind v4 `@theme` with hex values. Convert to match:

| Extension current | Canonical replacement |
|---|---|
| `--color-bg-core: #0a0a0a` | `--color-bg-core: rgb(9 10 13)` or equivalent hex `#090a0d` |
| `--color-accent-primary: #c4735a` | **DELETE** — replace all usages with `--color-accent-steel` |
| `--color-text-secondary: #999999` | `#b9bec8` (185 190 200) |
| `--color-text-dim: #666666` | `#737882` (115 120 130) — maps to `text-tertiary` |

#### Web-specific migration

The web's `global.css` has different `--text-secondary` and `--text-tertiary` from the journal. Sync:

| Web current | Canonical replacement |
|---|---|
| `--text-secondary: 200 205 215` | `185 190 200` |
| `--text-tertiary: 145 150 165` | `115 120 130` |

Web body font is `system-ui` — change to `Space Grotesk, system-ui, sans-serif` to match journal.

### Button Hierarchy

Three tiers, applied everywhere:

```
┌─────────────────────────────────────────────────────────────┐
│ PRIMARY (1 per view)                                        │
│ bg-text-primary text-main-bg font-display font-bold         │
│ text-sm py-3 px-8 tracking-wider                            │
│ hover:opacity-90 transition-opacity                         │
│                                                             │
│ Use: "Connect Exchange", "Save Entry", "[ VIEW PLANS ]"    │
├─────────────────────────────────────────────────────────────┤
│ SECONDARY (supporting actions)                              │
│ border border-text-primary text-text-primary font-mono      │
│ font-bold text-xs py-2.5 px-5 tracking-wider                │
│ hover:bg-text-primary hover:text-main-bg transition-colors  │
│                                                             │
│ Use: "Import", "Reauthorize", "Add Tag", pagination         │
├─────────────────────────────────────────────────────────────┤
│ GHOST (tertiary / cancel / close)                           │
│ text-text-secondary font-mono text-xs tracking-wider        │
│ hover:text-text-primary transition-colors                   │
│ NO border, NO background                                    │
│                                                             │
│ Use: "Cancel", "×", "Export", kebab menu items              │
├─────────────────────────────────────────────────────────────┤
│ DESTRUCTIVE (confirmation-gated)                            │
│ border border-signal-red text-signal-red font-mono          │
│ text-xs py-2.5 px-5 tracking-wider                          │
│ hover:bg-signal-red hover:text-main-bg transition-colors    │
│                                                             │
│ Use: "Delete", "Disconnect", confirmed destructive actions  │
└─────────────────────────────────────────────────────────────┘
```

Key changes from current:
- **Primary is filled** (currently everything is border-only)
- **Primary uses `font-display`** (Space Grotesk) — CTAs feel like actions, not data labels
- **Primary is slightly larger** (`text-sm` not `text-xs`)
- **Ghost has no border** — currently cancel/close buttons still have borders
- **Destructive is red-bordered** — currently uses same style as everything else

### Disabled States

Replace universal `disabled:opacity-50` with:

```css
/* Primary disabled */
disabled:bg-text-tertiary disabled:text-main-bg disabled:cursor-not-allowed

/* Secondary disabled */
disabled:border-dashed disabled:border-text-tertiary disabled:text-text-tertiary disabled:cursor-not-allowed

/* Ghost disabled */
disabled:text-text-tertiary/50 disabled:cursor-not-allowed
```

### Glass Panel Removal

Delete from `testudo-journal/src/styles/app.css`:
```css
/* DELETE entire .glass-panel block */
.glass-panel { ... }
[data-theme="light"] .glass-panel { ... }
```

Replace all `glass-panel` usages with `bg-container-bg` (solid). Affected files:
- `testudo-journal/src/components/account/ExchangeCard.tsx`
- `testudo-journal/src/components/account/AddExchangeCard.tsx`
- Any other `glass-panel` references (grep to find all)

In extension, remove `backdrop-filter` from `.balance-panel-overlay` gradient — keep the gradient but drop the blur.

### Border Radius Normalization

Search and replace in journal:
- Remove `rounded` from: `TradeSelector.tsx` (listbox, inputs), `TagManager.tsx` (inputs, tag rows), `CollectionSidebar.tsx` (inputs), `PageSubHeader.tsx` (time presets, filter toggles), `EntryEditor.tsx` (inputs)
- Keep `rounded-full` on: tag color swatches, status indicator dots
- Zero audit: confirm no cards have `rounded-*` classes

### Per-View Primary Button Assignments

| View | Primary Button | Notes |
|------|---------------|-------|
| Account (no exchanges) | "Connect Exchange" | Currently in AddExchangeForm |
| Account (has exchanges) | "Add Exchange" (+) card | The dashed-border card becomes the primary entry point |
| Trades | None needed | Data table view — actions are inline secondary |
| Journal | "New Entry" | If exists; otherwise sidebar create action |
| Entry Editor modal | "Save" | Top-right of editor toolbar |
| Extension popup (no exchange) | "Connect Account" | Empty state CTA |
| Extension popup (has exchange) | None — tab interface, no single CTA | Secondary tier for cancel/close |
| Landing Hero | Scroll-to or pricing link | Implicit via content, no explicit CTA button in hero currently |
| Pricing page | "[ SELECT ]" on highlighted tier | The EARLY LEGIONARY card's select label |

### Paved Roads

- Journal token system (`app.css` lines 1-48) — most mature, use as canonical
- Extension base button style (`popup.css` button block) — already border + invert, easy to refactor into tiers
- Web `btn-primary` utility (`global.css` line 45-48) — rename to `btn-secondary`, add real `btn-primary`

### Files

**Journal (testudo-journal/)**
- `src/styles/app.css` — remove `glass-panel`, update canonical tokens
- `src/components/account/ExchangeCard.tsx` — button tiers, remove glass-panel
- `src/components/account/AddExchangeCard.tsx` — remove glass-panel
- `src/components/account/AddExchangeForm.tsx` — primary button for CTA, ghost for cancel
- `src/components/PageSubHeader.tsx` — remove `rounded` from presets/filters
- `src/components/trades/TradeTable.tsx` — ghost buttons for sort headers
- `src/components/trades/Pagination.tsx` — ghost buttons for page nav
- `src/components/journal/TagManager.tsx` — remove `rounded` from inputs, keep `rounded-full` on dots
- `src/components/journal/EntryEditor.tsx` — primary for save, ghost for close, remove rounded
- `src/components/journal/TradeSelector.tsx` — remove `rounded` from listbox/inputs
- `src/components/journal/CollectionSidebar.tsx` — remove `rounded` from inputs
- `src/components/journal/DatabaseTable.tsx` — secondary for pagination, ghost for inline actions

**Extension (testudo-extension/)**
- `src/popup/popup.css` — migrate tokens, kill terra cotta, add button tier classes, align font stack
- `src/popup/components/MainView.tsx` — apply button tiers to "Connect Account" CTA
- `src/popup/components/PositionCard.tsx` — secondary for cancel, ghost for minor actions
- `src/popup/components/HeaderBar.tsx` — ghost tier for menu items

**Web (testudo-web/)**
- `src/styles/global.css` — sync tokens, rename `btn-primary` to `btn-secondary`, add filled `btn-primary`
- `src/components/Pricing.astro` — "VIEW PLANS" becomes primary (filled)
- `src/components/Footer.astro` — ghost tier for links (already close)
- `src/pages/pricing.astro` — highlighted tier card gets primary "SELECT"
- `tailwind.config.js` — update font family to Space Grotesk

### Dependencies Added

None. All changes are CSS/Tailwind class modifications.

---

## Acceptance Criteria

- [ ] All three surfaces use identical CSS custom property values for every shared token (dark + light)
- [ ] No reference to `#c4735a` or terra cotta exists anywhere in the codebase
- [ ] No reference to `glass-panel` or `backdrop-filter: blur` exists in journal or extension CSS
- [ ] No `rounded` class on non-circular interactive elements in journal
- [ ] Primary buttons are visually distinct (filled bg) and limited to 1 per view/section
- [ ] Primary buttons use `font-display` (Space Grotesk)
- [ ] Secondary buttons retain current border style with `font-mono`
- [ ] Ghost buttons have no border, no background
- [ ] Destructive buttons use signal-red border
- [ ] Disabled states differ structurally per tier (not just opacity-50)
- [ ] Extension body/display font is Space Grotesk (matching journal)
- [ ] Web body font is Space Grotesk (matching journal)
- [ ] `bun run build` passes for extension
- [ ] `bun run build` passes for web (if applicable / `astro build`)
- [ ] Visual spot-check: extension popup, desk Account page, landing pricing page all feel like one product

---

## Risks

1. **Extension Tailwind v4 syntax divergence** — Extension uses `@theme` blocks while journal/web use v3-style config. Token values can be identical but the declaration syntax differs. Mitigation: sync values manually; don't try to share a literal CSS file across build systems.

2. **Pricing card visual regression** — Removing the hover glow + lift on pricing cards could make them feel flat. Mitigation: replace with `hover:border-text-primary` (border brightens on hover) — simpler, on-brand.

3. **Font loading in extension** — Adding Space Grotesk to the extension increases bundle size. Mitigation: the extension already loads Space Mono; adding Grotesk (woff2, ~20kb) is acceptable for brand consistency.

---

## Completion Signal

This spec is complete when:
1. A single canonical token set is documented and applied to all three surfaces
2. The 3-tier button hierarchy is visually distinct and enforced across all views
3. Glass/blur effects and stray rounded corners are removed
4. All builds pass (`bun run build` for extension and web)
5. Code committed to master
