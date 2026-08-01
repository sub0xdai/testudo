# UXP-18 Implementation Plan: Multi-Theme Support

> Last updated: 2026-03-21
> Phase: COMPLETE

---

## Architecture Decisions

### AD-1: Tailwind v4 (Extension) — Native CSS var override
The popup's `@theme` block already creates CSS custom properties (e.g., `--color-bg-core`). Tailwind v4 handles opacity modifiers with CSS vars natively. Simply add `[data-theme]` selector blocks after `@theme` to override values. No structural changes to the build.

### AD-2: Tailwind v3 (Web + Journal) — RGB channels pattern
The shared preset (`packages/tailwind-preset/index.ts`) must change from hardcoded hex to the standard Tailwind v3 dynamic color pattern:
```js
// Before
'main-bg': '#050505',
// After
'main-bg': 'rgb(var(--main-bg) / <alpha-value>)',
```
This enables ALL opacity modifiers (`bg-main-bg/60`, `border-container-border/30`, etc.) to work with runtime theme switching. No component class name changes required.

CSS vars defined as space-separated RGB channels:
```css
:root { --main-bg: 5 5 5; }
[data-theme="soft-dark"] { --main-bg: 22 22 30; }
[data-theme="light"] { --main-bg: 245 240 232; }
```

### AD-3: Journal `var(--color-*)` migration
Journal's `app.css` currently uses `var(--color-main-bg)` (hex) in markdown/selection styles. These must become `rgb(var(--main-bg))` to use the RGB channel vars. Fallback pattern: `rgb(var(--main-bg, 5 5 5))`.

### AD-4: Journal charts — dispose + re-init on theme change
ECharts caches colors at init. Lightweight-charts hardcode colors in config. Strategy: MutationObserver on `document.documentElement` watching `data-theme` attribute. On change → dispose all chart instances and re-create with fresh color reads from `getComputedStyle`.

### AD-5: Extension modal — CSS-only theme variants
Include all three theme variant blocks in the Shadow DOM `<style>`. Set `data-theme` attribute on the host element from `document.documentElement.dataset.theme` at creation. No JS color mapping needed.

---

## Tasks

| ID | Task | Status | Complexity | Deps | Notes |
|----|------|--------|------------|------|-------|
| T1 | Extension popup: add `[data-theme]` override blocks to `popup.css` | complete | medium | — | Added soft-dark + light blocks after `@theme`; button:hover + login-glass-card converted to CSS vars |
| T2 | Extension popup: anti-flash script + theme picker in `SettingsView.tsx` | complete | medium | T1 | Anti-flash in popup.html; 3-button segmented control in SettingsView |
| T3 | Extension modal: theme-aware Shadow DOM in `modal.tsx` | complete | medium | T1 | `:host-context()` CSS-only approach (AD-5); TOAST_CSS also themed |
| T4 | Shared preset: change to `rgb(var(--*) / <alpha-value>)` pattern | complete | medium | T5, T6 | All 14 colors converted in `packages/tailwind-preset/index.ts` |
| T5 | Web app: add `:root` RGB channel vars + `[data-theme]` overrides to `index.css` | complete | medium | — | 14 channel vars per theme block; body/selection colors reference vars |
| T6 | Journal: add `[data-theme]` override blocks + migrate `var(--color-*)` refs in `app.css` | complete | medium | — | Converted `:root` to RGB channels; migrated all `var(--color-*)` refs |
| T7 | Anti-flash scripts in web `index.html` + journal `index.html` | complete | simple | T5, T6 | Both HTML files updated |
| T8 | Web app: theme picker in `Header.tsx` | complete | medium | T5 | Cycle button `[AMOLED]`/`[SOFT]`/`[LIGHT]` in nav |
| T9 | Journal: theme toggle in `Layout.tsx` | complete | medium | T6 | Cycle toggle in desktop + mobile nav |
| T10 | Journal: convert `tokens.ts` to runtime CSS readers | complete | medium | T6 | 15 getter functions; 13 consumer files updated |
| T11 | Journal: dynamic ECharts theme + chart re-render on theme change | complete | complex | T10 | MutationObserver in theme-observer.ts; EChart + lightweight-charts dispose+re-init |
| T12 | Web app: update `SpotlightBackground.tsx` + `main.tsx` RainbowKit | deferred | medium | T5, T8 | RainbowKit needs ThemeContext lift — TODO comment added |
| T13 | Verification: build all three surfaces + visual check | complete | simple | all | All 3 builds pass |

---

## Dependency Graph

```
T1 (ext CSS) ──┬── T2 (ext popup UI + anti-flash)
               └── T3 (ext modal)

T5 (web CSS) ──┐
               ├── T4 (shared preset) ← must land after T5+T6
T6 (jrnl CSS) ─┤
               ├── T7 (anti-flash scripts)
               ├── T8 (web header UI)
               ├── T9 (jrnl layout UI)
               ├── T10 (jrnl tokens) ── T11 (jrnl charts)
               └── T12 (web spotlight + rainbowkit)

T13 (verify) ← all
```

## Suggested Build Order

**Batch 1** (CSS foundations — parallelizable): T1, T5, T6
**Batch 2** (preset + UI + anti-flash): T4, T2, T3, T7, T8, T9
**Batch 3** (journal charts + web specifics): T10, T11, T12
**Batch 4** (verification): T13

---

## Color Reference

### RGB Channel Values (for Tailwind v3 `var()` pattern)

**AMOLED (default)**
```css
--main-bg: 5 5 5;                /* #050505 */
--container-bg: 10 10 10;        /* #0A0A0A */
--container-bg-hover: 17 17 17;  /* #111111 */
--elevated: 17 17 17;            /* #111111 */
--container-border: 63 63 70;    /* #3F3F46 */
--border-active: 255 255 255;    /* #FFFFFF */
--accent-steel: 148 163 184;     /* #94A3B8 */
--accent-steel-hover: 203 213 225; /* #CBD5E1 */
--signal-green: 0 255 65;        /* #00FF41 */
--signal-red: 255 0 60;          /* #FF003C */
--signal-amber: 245 158 11;      /* #F59E0B */
--text-primary: 255 255 255;     /* #FFFFFF */
--text-secondary: 136 136 136;   /* #888888 */
--text-tertiary: 85 85 85;       /* #555555 */
```

**Soft Dark**
```css
--main-bg: 22 22 30;             /* #16161e */
--container-bg: 30 30 42;        /* #1e1e2a */
--container-bg-hover: 38 38 54;  /* #262636 */
--elevated: 38 38 54;            /* #262636 */
--container-border: 59 59 82;    /* #3b3b52 */
--border-active: 82 82 107;      /* #52526b */
--accent-steel: 148 163 184;     /* #94A3B8 (unchanged) */
--accent-steel-hover: 203 213 225; /* #CBD5E1 (unchanged) */
--signal-green: 0 255 65;        /* #00FF41 (unchanged) */
--signal-red: 255 0 60;          /* #FF003C (unchanged) */
--signal-amber: 245 158 11;      /* #F59E0B (unchanged) */
--text-primary: 224 221 216;     /* #e0ddd8 */
--text-secondary: 138 136 152;   /* #8a8898 */
--text-tertiary: 90 88 104;      /* #5a5868 */
```

**Light (Paper)**
```css
--main-bg: 245 240 232;          /* #f5f0e8 */
--container-bg: 250 247 242;     /* #faf7f2 */
--container-bg-hover: 255 252 247; /* #fffcf7 */
--elevated: 255 252 247;         /* #fffcf7 */
--container-border: 212 205 194; /* #d4cdc2 */
--border-active: 176 168 152;    /* #b0a898 */
--accent-steel: 100 116 139;     /* #64748b (slate-500, darkened for contrast) */
--accent-steel-hover: 148 163 184; /* #94A3B8 (slate-400) */
--signal-green: 26 122 46;       /* #1a7a2e (WCAG AA darkened) */
--signal-red: 184 0 42;          /* #b8002a (WCAG AA darkened) */
--signal-amber: 180 83 9;        /* #b45309 (amber-700, WCAG AA) */
--text-primary: 26 23 20;        /* #1a1714 */
--text-secondary: 107 100 88;    /* #6b6458 */
--text-tertiary: 154 146 133;    /* #9a9285 */
```

### Extension-Specific Token Mapping (Tailwind v4 hex overrides)

Extension popup uses different names than the shared preset. Mapping to spec values:

| Extension Token | AMOLED | Soft Dark | Light |
|----------------|--------|-----------|-------|
| --color-bg-core | #050505 | #16161e | #f5f0e8 |
| --color-bg-panel | #0A0A0A | #1e1e2a | #faf7f2 |
| --color-bg-elevated | #111111 | #262636 | #fffcf7 |
| --color-bg-hover | #1A1A1A | #30304a | #ebe5db |
| --color-bg-surface | #0F0F0F | #1b1b28 | #f2ede5 |
| --color-border-subtle | #3F3F46 | #3b3b52 | #d4cdc2 |
| --color-border-active | #52525B | #52526b | #b0a898 |
| --color-border-grid | #27272A | #2d2d42 | #ddd6cb |
| --color-signal-green | #00FF41 | #00FF41 | #1a7a2e |
| --color-signal-red | #FF003C | #FF003C | #b8002a |
| --color-signal-orange | #f59e0b | #f59e0b | #b45309 |
| --color-signal-blue | #3b82f6 | #3b82f6 | #2563eb |
| --color-text-primary | #ffffff | #e0ddd8 | #1a1714 |
| --color-text-secondary | #888888 | #8a8898 | #6b6458 |
| --color-text-dim | #555555 | #5a5868 | #9a9285 |
| --color-accent-steel | #94a3b8 | #94a3b8 | #64748b |

Dim signal variants (12% opacity of base signal): derive using `oklch()` or `color-mix()` per theme.

### Journal Extra Vars

| Var | AMOLED | Soft Dark | Light |
|-----|--------|-----------|-------|
| --chart-bg | 17 17 17 | 38 38 54 | 255 252 247 |

---

## Discoveries

- **D1: Extension uses Tailwind v4 with `@theme` block (not the shared preset).** 22 color tokens. Tailwind v4 handles CSS var opacity natively. Build uses `npx @tailwindcss/cli` — no postcss config.

- **D2: Web and Journal share `packages/tailwind-preset/index.ts` (Tailwind v3).** 14 hardcoded hex colors. Changing to CSS vars affects both simultaneously.

- **D3: DECIDED — RGB channels for Tailwind v3 opacity support.** 9 unique opacity modifier usages across web+journal: `bg-main-bg/60`, `/80`, `/95`; `border-container-border/20`, `/30`, `/50`, `/80`; `bg-container-border/15`; `text-text-secondary/70`. The `rgb(var(--channel) / <alpha-value>)` pattern keeps all existing class names working. Trade-off: vars defined as `5 5 5` instead of `#050505` (less readable but standard Tailwind v3 pattern).

- **D4: Journal has dual charting libraries.** ECharts (6 chart types via EChart.tsx wrapper) uses registered theme `testudo-dark`. Lightweight-charts (EquityCurve, DailyPnl, CumulativeProfit) hardcode colors inline from `tokens.ts`. Both need dispose + re-init on theme change.

- **D5: Extension modal uses Shadow DOM isolation.** `:host` has 10 hardcoded CSS vars. Can't inherit `[data-theme]` from outer document. Solution: include all 3 theme variants in CSS, set `data-theme` attr on host element at creation.

- **D6: RainbowKit in web app needs conditional theme.** `darkTheme()` vs `lightTheme()` based on current theme. Requires reading `data-theme` attr — can use a simple `useState` + effect, doesn't need a full ThemeContext.

- **D7: Extension popup has separate localStorage.** Different origin from web — theme settings independent. Non-goal per spec.

- **D8: No existing theme infrastructure.** Greenfield implementation across all surfaces.

- **D9: Journal `app.css` has 13 `:root` CSS vars.** Must convert from hex to RGB channels and update `var(--color-*)` references to `rgb(var(--*))` throughout the file (12 references in markdown/selection/focus styles).

- **D10: Web `index.css` has hardcoded body colors.** `#050505` bg, `#FFFFFF` text, `#94A3B8` selection — must become CSS var references.

- **D11: SpotlightBackground.tsx hardcodes `rgba(5,5,5,0.85)`.** Must use `color-mix(in srgb, rgb(var(--main-bg)) 85%, transparent)` or similar.

- **D12: Journal `tokens.ts` has static exports + alpha helpers.** `SIGNAL_GREEN`, `signalGreenAlpha(a)`, `TAG_PALETTE`, `ENTRY_TYPE_COLORS`. All must become runtime readers. Alpha helpers can use `color-mix(in srgb, rgb(var(--signal-green)) ${a*100}%, transparent)`.

- **D13: Journal body style `background-color: #050505` (line 41) is hardcoded.** Must become `background-color: rgb(var(--main-bg))`.

- **D14: Journal focus ring uses hardcoded `#050505`.** Line 109: `box-shadow: 0 0 0 2px #050505, ...` — must use CSS var.

## Blockers

None.
