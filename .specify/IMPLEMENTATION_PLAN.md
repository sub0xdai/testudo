# Implementation Plan

> Last updated: 2026-04-16
> Current spec: UX-07-design-normalization
> Phase: BUILD

---

## Active Spec: UX-07-design-normalization

### Gap Analysis

**Journal (canonical — no token changes needed):**
- `glass-panel` class used in 3 files: ExchangeCard.tsx, AddExchangeCard.tsx, ChartSelector.tsx
- `backdrop-filter: blur(12px)` only via `.glass-panel` CSS class
- `rounded` on date inputs in FilterPopout.tsx (only non-`rounded-full` usage)
- No button hierarchy — all buttons use border+mono+invert pattern

**Extension (significant drift):**
- Hex values instead of space-separated RGB (different Tailwind v4 `@theme` syntax — keep hex, sync values)
- Terra cotta `#c4735a` in popup.css AND modal.tsx (inline Shadow DOM styles)
- Light theme warm parchment tones (`#f5f0e8`, `#e8e3d7`) — journal uses cool grays
- `--color-text-secondary: #999999` → should be `#b9bec8` (185 190 200)
- `--color-text-dim: #666666` → should be `#737882` (115 120 130)
- `backdrop-filter` in `.login-glass-card` (popup.css) and `.backdrop` (modal.tsx)
- Font stack already has Space Grotesk — no font changes needed
- Global `button` CSS rule in popup.css defines base invert-on-hover

**Web (minor drift):**
- `--text-secondary: 200 205 215` → should be `185 190 200`
- `--text-tertiary: 145 150 165` → should be `115 120 130`
- Light theme `--bg-elevated` and `--border` values diverge from journal
- Body font is `system-ui` → should be `Space Grotesk, system-ui, sans-serif`
- `.btn-primary` is border-style (actually secondary) → needs real filled primary
- No button hierarchy

### Parallel Track Detection

Three independent codebases with separate build systems:
```
Track A (Journal):  T1 → T2 → T3        (glass removal → radius → buttons)
Track B (Extension): T4 → T5 → T6       (tokens → blur removal → buttons)
Track C (Web):       T7 → T8            (tokens+font → buttons)
T9: Cross-surface verification (depends on all)
```

Tracks A, B, C are fully independent — no shared files, no shared build.

---

## Tasks

### Track A: Journal (testudo-journal/)

#### T1: Remove glass-panel and backdrop-filter from journal — `pending`
**Scope:** CP-6 partial — glass removal
**Files:**
- `testudo-journal/src/styles/app.css` — delete `.glass-panel` block and `[data-theme="light"] .glass-panel` block
- `testudo-journal/src/components/account/ExchangeCard.tsx` — replace `glass-panel` with `bg-container-bg`
- `testudo-journal/src/components/account/AddExchangeCard.tsx` — replace `glass-panel` with `bg-container-bg`
- `testudo-journal/src/components/overview/ChartSelector.tsx` — replace `glass-panel` with `bg-container-bg`

**Validate:** `cd testudo-journal && bun run build`

#### T2: Border radius normalization in journal — `pending`
**Scope:** CP-6 partial — radius cleanup
**Files:**
- `testudo-journal/src/components/trades/FilterPopout.tsx` — remove `rounded` from date inputs
- Grep for any other `rounded` (non-`rounded-full`) in journal components

**Validate:** `cd testudo-journal && bun run build`

#### T3: Button hierarchy in journal — `pending`
**Scope:** CP-4 — 3-tier buttons across journal views

Per-view primary button assignments:
- Account (no exchanges): "Connect Exchange" → PRIMARY (filled)
- Account (has exchanges): AddExchangeCard → keep dashed-border (special affordance)
- Trades: no primary needed — inline secondary only
- Journal: no primary currently
- Entry Editor: "Save" → PRIMARY
- Cancel/close buttons → GHOST (remove borders)
- Delete/disconnect → DESTRUCTIVE (signal-red border)

**Files:**
- `testudo-journal/src/components/account/AddExchangeForm.tsx` — "Connect Exchange" submit → primary filled
- `testudo-journal/src/components/account/ExchangeCard.tsx` — kebab actions → ghost, destructive items → red border
- `testudo-journal/src/components/trades/TradeTable.tsx` — sort headers → ghost
- `testudo-journal/src/components/trades/Pagination.tsx` — page nav → ghost
- `testudo-journal/src/components/journal/EntryCard.tsx` — edit/export → ghost, delete → destructive
- `testudo-journal/src/components/journal/EntryEditor.tsx` — save → primary, close → ghost

**Button class definitions (add to app.css or inline):**
- Primary: `bg-text-primary text-main-bg font-display font-bold text-sm py-3 px-8 tracking-wider hover:opacity-90 transition-opacity`
- Secondary: `border border-text-primary text-text-primary font-mono font-bold text-xs py-2.5 px-5 tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors`
- Ghost: `text-text-secondary font-mono text-xs tracking-wider hover:text-text-primary transition-colors` (NO border, NO background)
- Destructive: `border border-signal-red text-signal-red font-mono text-xs py-2.5 px-5 tracking-wider hover:bg-signal-red hover:text-main-bg transition-colors`

**Disabled states per tier:**
- Primary: `disabled:bg-text-tertiary disabled:text-main-bg disabled:cursor-not-allowed`
- Secondary: `disabled:border-dashed disabled:border-text-tertiary disabled:text-text-tertiary disabled:cursor-not-allowed`
- Ghost: `disabled:text-text-tertiary/50 disabled:cursor-not-allowed`

**Validate:** `cd testudo-journal && bun run build`

---

### Track B: Extension (testudo-extension/)

#### T4: Extension token migration — `pending`
**Scope:** CP-2 — sync extension tokens to canonical values, kill terra cotta

**Dark theme changes (popup.css @theme block):**
- `--color-bg-core: #0a0a0a` → `#090a0d` (9 10 13)
- `--color-bg-panel: #161616` → `#131519` (19 21 26) — wait, hex conversion: rgb(19,21,26) = #13151a
- `--color-bg-elevated: #1e1e1e` → `#1a1c23` (26 28 35)
- `--color-bg-hover: #282828` → `#23262e` (35 38 46)
- `--color-border-subtle: #323232` → `#2d303a` (45 48 58)
- `--color-accent-primary: #c4735a` → **DELETE** — replace all usages with `--color-accent-steel` (#94a3b8)
- `--color-text-secondary: #999999` → `#b9bec8` (185 190 200)
- `--color-text-dim: #666666` → `#737882` (115 120 130)

**Light theme changes (popup.css `[data-theme="light"]`):**
Kill warm parchment, sync to journal's cool palette:
- `--color-bg-core: #f5f0e8` → `#f1f3f7` (241 243 247)
- `--color-bg-panel: #e8e3d7` → `#ffffff` (255 255 255)
- `--color-bg-elevated: #f0ebe2` → `#eaecf2` (234 236 242)
- `--color-bg-hover: #ebe5db` → `#e1e4ec` (225 228 236)
- `--color-bg-surface: #fdf9f4` → `#f1f3f7` (same as core)
- `--color-border-subtle: #b4ada2` → `#c3c8d4` (195 200 212)
- `--color-border-active: #8a8278` → `#0a0f1e` (10 15 30)
- `--color-accent-steel: #505d70` → `#374151` (55 65 81)
- `--color-accent-primary: #9e5a44` → **DELETE** — replace with accent-steel
- `--color-signal-green: #146426` → `#05963c` (5 150 60)
- `--color-signal-red: #a00024` → `#c81419` (200 20 25)
- `--color-signal-orange: #be6405` → `#b46e00` (180 110 0)
- `--color-text-primary: #1a1714` → `#0a0c14` (10 12 20)
- `--color-text-secondary: #524c42` → `#373e4e` (55 62 78)
- `--color-text-dim: #787166` → `#5f6676` (95 102 118)

**Modal Shadow DOM styles (modal.tsx):**
- Update inline `--color-accent-primary: #c4735a` → `#94a3b8`
- Update light theme override `--color-accent-primary: #9e5a44` → `#374151`
- Sync any other divergent inline token values

**Also grep for:**
- Any remaining `#c4735a` or `#9e5a44` references
- Any `accent-primary` class usages that need semantic review

**Validate:** `cd testudo-extension && bun run build`

#### T5: Extension backdrop-filter cleanup — `pending`
**Scope:** CP-6 partial — blur removal in extension

**Files:**
- `testudo-extension/src/popup/popup.css` — `.login-glass-card`: remove `backdrop-filter: blur(16px)` and `-webkit-backdrop-filter`, keep background opacity and border
- `testudo-extension/src/modal.tsx` — `.backdrop`: remove `backdrop-filter: blur(4px)` and `-webkit-backdrop-filter`, keep `background: rgba(0,0,0,0.5)`

**Validate:** `cd testudo-extension && bun run build`

#### T6: Extension button hierarchy — `pending`
**Scope:** CP-5 partial — button tiers in extension popup

**Changes:**
- MainView.tsx "Connect Account" / "Connect Wallet" CTA → primary filled style
- Global `button` rule in popup.css → make it secondary-tier (current border+invert is secondary)
- Add `.btn-primary` class to popup.css for filled buttons
- Add `.btn-ghost` class for no-border text-only buttons
- Add `.btn-destructive` class for signal-red bordered buttons
- HeaderBar menu items → ghost tier
- PositionCard cancel → secondary, minor actions → ghost
- Disabled state: replace `opacity: 0.35` with tier-specific disabled states

**Note:** Primary buttons in extension use `font-family: var(--font-family-sans)` (Space Grotesk) — this is already the sans font in the extension.

**Validate:** `cd testudo-extension && bun run build`

---

### Track C: Web (testudo-web/)

#### T7: Web token migration + body font — `pending`
**Scope:** CP-3 — sync web tokens to canonical values

**Dark theme changes (global.css):**
- `--text-secondary: 200 205 215` → `185 190 200`
- `--text-tertiary: 145 150 165` → `115 120 130`

**Light theme changes (global.css):**
- `--bg-elevated: 248 249 252` → `234 236 242`
- `--bg-hover: 240 242 246` → `225 228 236`
- `--border: 218 222 230` → `195 200 212`
- `--accent-steel: 75 85 99` → `55 65 81`
- `--accent-steel-hover: 55 65 81` → `35 45 60`
- `--accent-primary: 120 128 140` → `80 90 108`
- `--signal-green: 22 163 74` → `5 150 60`
- `--signal-red: 220 38 38` → `200 20 25`
- `--signal-amber: 202 138 4` → `180 110 0`
- `--text-tertiary: 90 97 112` → `95 102 118`

**Body font change (global.css):**
- `font-family: system-ui, -apple-system, 'Segoe UI', sans-serif` → `font-family: 'Space Grotesk', system-ui, -apple-system, sans-serif`

**Validate:** `cd testudo-web && bun run build`

#### T8: Web button hierarchy — `pending`
**Scope:** CP-5 partial — button tiers on landing page

**Changes:**
- Rename existing `.btn-primary` to `.btn-secondary` (it's border-style, which IS secondary)
- Add new `.btn-primary` class: `bg-text-primary text-main-bg font-display font-bold text-sm py-3 px-8 tracking-wider hover:opacity-90 transition-opacity`
- Add `.btn-ghost` class
- Add `.btn-destructive` class
- Pricing.astro "[ VIEW PLANS ]" CTA → keep secondary (it's a nav link, not the primary action)
- pricing.astro highlighted tier "[ SELECT ]" → primary filled
- Other tier "[ SELECT ]" → secondary
- Update consumers of old `.btn-primary` to `.btn-secondary`

**Validate:** `cd testudo-web && bun run build`

---

### Cross-Surface

#### T9: Build verification + acceptance criteria audit — `pending`
**Scope:** Final validation

**Run all three builds:**
```bash
cd testudo-journal && bun run build
cd testudo-extension && bun run build
cd testudo-web && bun run build
```

**Acceptance criteria checklist:**
- [ ] All three surfaces use identical CSS custom property values (dark + light)
- [ ] No `#c4735a` or terra cotta anywhere in codebase
- [ ] No `glass-panel` or `backdrop-filter: blur` in journal or extension CSS
- [ ] No `rounded` on non-circular interactive elements in journal
- [ ] Primary buttons visually distinct (filled bg), limited to 1 per view
- [ ] Primary buttons use `font-display` (Space Grotesk)
- [ ] Secondary buttons retain border style with `font-mono`
- [ ] Ghost buttons have no border, no background
- [ ] Destructive buttons use signal-red border
- [ ] Disabled states differ per tier
- [ ] Extension body/display font is Space Grotesk
- [ ] Web body font is Space Grotesk
- [ ] All builds pass

**Validate:** All three build commands exit 0.
