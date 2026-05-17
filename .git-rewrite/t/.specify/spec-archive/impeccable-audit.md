# Testudo Extension — Quality Audit Report

> Generated 2026-03-13 via `impeccable:audit` against `testudo-extension/`

---

## Anti-Patterns Verdict

**Pass.** The extension avoids the canonical AI slop tells: no cyan-on-dark palette, no purple gradients, no glassmorphism abuse, no hero metric templates, no identical card grids, no gradient text. The Roman ruins texture, custom ArcGauge, and utilitarian density are human design choices. The dark terminal aesthetic is genre-appropriate for a trading tool, not a lazy default.

---

## Executive Summary

| Severity | Count |
|----------|-------|
| Critical | 7 |
| High | 14 |
| Medium | 16 |
| Low | 15 |
| **Total** | **52** |

**Top 5 issues by impact:**
1. Shadow DOM `mode: "closed"` makes the trade modal completely invisible to screen readers (A11y)
2. Zero form labels are programmatically associated — 20 inputs across 7 files (A11y)
3. Two undefined design tokens (`accent-green`, `signal-blue`) cause invisible UI elements (Theming)
4. `text-secondary` and `text-dim` fail WCAG AA contrast on all dark backgrounds (A11y)
5. Modal and popup use entirely divergent color systems — three different greens (Theming)

**Recommended approach:** Address theming issues first (ISSUE-1 from the critique — `/normalize`), as fixing the token system unblocks many downstream issues. Then accessibility, then performance.

---

## Detailed Findings

### Critical Issues

#### C-1: Shadow DOM `mode: "closed"` blocks all assistive technology
- **Location**: `src/modal.tsx:127` and `src/modal.tsx:184`
- **Category**: Accessibility
- **Description**: Both the trade confirmation modal and toast notification system use `attachShadow({ mode: "closed" })`. Closed shadow roots are invisible to the browser's accessibility tree. Screen readers cannot read, navigate, or interact with any modal content — including the trade execution form, which is the primary UI for placing live trades.
- **Impact**: Complete AT failure for the most safety-critical UI surface. A screen reader user cannot confirm or cancel a trade.
- **WCAG**: 4.1.2 Name, Role, Value (Level A) — total failure
- **Recommendation**: Change to `mode: "open"` and implement focus trapping. Add `role="dialog"`, `aria-modal="true"`, and `aria-label` to the modal container. For toasts, add a document-level `aria-live="polite"` region that mirrors toast messages.
- **Command**: `/harden`

#### C-2: Zero form labels are programmatically associated (20 inputs)
- **Location**: Every form across 7 files (see table below)
- **Category**: Accessibility
- **Description**: Every `<label>` in the extension lacks a `for` attribute, and every `<input>` lacks an `id`. Screen readers cannot determine what any form field is for.
- **Impact**: Screen reader users cannot identify any input field. The extension is effectively unusable with AT.
- **WCAG**: 1.3.1 Info and Relationships (Level A), 4.1.2 Name, Role, Value (Level A)

| File | Unassociated inputs |
|------|-------------------|
| `AuthSection.tsx` | Email, Password |
| `TradeManagement.tsx` | Risk %, Leverage, Break-Even (number + range = 6 inputs) |
| `QuickTrade.tsx` | Symbol, Entry, Stop, Target |
| `SettingsView.tsx` | Backend URL, WebSocket URL, Web App URL |
| `ExchangeManager.tsx` | Exchange select, API Key, Secret, Passphrase |
| `TradeForm.tsx` | Symbol, Entry, Stop, Target |

- **Recommendation**: Add unique `id` attributes to all inputs and matching `for` attributes on labels. Pattern: `id="field-risk-percent"`, `for="field-risk-percent"`.
- **Command**: `/harden`

#### C-3: Undefined token `accent-green` — UI elements render colorless
- **Location**: `ExchangeSelector.tsx:95,116,123`, `MainView.tsx:161`
- **Category**: Theming
- **Description**: `text-accent-green`, `bg-accent-green/10`, `bg-accent-green/15`, `bg-accent-green/20` are used in 6+ locations but `--color-accent-green` is never defined in `popup.css @theme`. The exchange name badge, active exchange indicator, and dropdown active state all reference this undefined token.
- **Impact**: Depending on Tailwind v4 fallback behavior, these elements render with no color or fall through to an unintended default. The active exchange indicator may be invisible.
- **Recommendation**: Add `accent-green: #22c55e` to `@theme` (matching `signal-green`), or replace all `accent-green` references with `signal-green`.
- **Command**: `/normalize`

#### C-4: Undefined token `signal-blue` — "From Exchange" indicator invisible
- **Location**: `ActiveOrders.tsx:159,178`
- **Category**: Theming
- **Description**: `bg-signal-blue` and `text-signal-blue` are used for the exchange position fallback section but no `--color-signal-blue` token exists.
- **Impact**: The "From Exchange" indicator dot and count badge render with no color, making the section header visually broken.
- **Recommendation**: Add `signal-blue: #3b82f6` (blue-500) to `@theme`, or replace with an existing token like `accent-steel`.
- **Command**: `/normalize`

#### C-5: Tab bar lacks ARIA tab pattern
- **Location**: `src/popup/components/TabBar.tsx` (rendered in `MainView.tsx`)
- **Category**: Accessibility
- **Description**: The four-tab navigation (Trade, Quick, Positions, Account) uses plain `<button>` elements with no `role="tablist"` on the container, no `role="tab"` on buttons, no `aria-selected`, and no `aria-controls` pointing to tab panels. Tab panels have no `role="tabpanel"` or `aria-labelledby`.
- **Impact**: Screen readers announce these as generic buttons, not a tab interface. Users cannot understand the navigation model.
- **WCAG**: 4.1.2 Name, Role, Value (Level A)
- **Recommendation**: Add `role="tablist"` to the container, `role="tab"` + `aria-selected` to each button, and `role="tabpanel"` + `aria-labelledby` to each content area.
- **Command**: `/harden`

#### C-6: `text-secondary` (#6b7280) fails WCAG AA contrast
- **Location**: Global — used throughout all components
- **Category**: Accessibility
- **Description**: `text-secondary` on `bg-core` yields **3.47:1** (needs 4.5:1). On `bg-panel`: **3.09:1**. On `bg-elevated`: **2.69:1**. This color is used for virtually all secondary labels, status text, form hints, and descriptions.
- **Impact**: Secondary text is difficult to read for users with low vision. Affects the majority of non-primary text in the extension.
- **WCAG**: 1.4.3 Contrast Minimum (Level AA)
- **Recommendation**: Lighten `text-secondary` to at least `#9ca3af` (zinc-400, ~5.06:1 on bg-core) or `#a1a1aa` for AA compliance across all backgrounds.
- **Command**: `/colorize`

#### C-7: `text-dim` (#4b5563) fails contrast catastrophically
- **Location**: Global — tab inactive labels, placeholders, decorative text
- **Category**: Accessibility
- **Description**: `text-dim` on `bg-core`: **2.13:1**. On `bg-panel`: **1.90:1**. On `bg-elevated`: **1.65:1**. Used for inactive tab labels, placeholder text, and decorative elements.
- **Impact**: Text is nearly invisible to users with any degree of visual impairment. Even users with normal vision may strain.
- **WCAG**: 1.4.3 Contrast Minimum (Level AA)
- **Recommendation**: Lighten `text-dim` to at least `#6b7280` (current `text-secondary` value) or higher. If `text-secondary` is also raised per C-6, ensure `text-dim` remains visually subordinate but still readable.
- **Command**: `/colorize`

---

### High-Severity Issues

#### H-1: No `role="alert"` on any dynamic error message (7 locations)
- **Category**: Accessibility
- **Locations**: `AuthSection.tsx:83`, `HeaderBar.tsx:54`, `QuickTrade.tsx:230`, `ActiveOrders.tsx:142,146`, `ExchangeManager.tsx:241`, `SettingsView.tsx:76`
- **Description**: Error messages, warnings, and status confirmations appear dynamically but have no `role="alert"`, `role="status"`, or `aria-live` attributes. Screen readers will not announce these changes.
- **WCAG**: 4.1.3 Status Messages (Level AA)
- **Command**: `/harden`

#### H-2: ExchangeSelector dropdown — no ARIA, no keyboard support
- **Category**: Accessibility
- **Location**: `ExchangeSelector.tsx:92-131`
- **Description**: Missing `aria-haspopup`, `aria-expanded`, `role="listbox"`, `role="option"`, `aria-selected`, focus management on open, and Escape key to close.
- **WCAG**: 4.1.2 Name, Role, Value (Level A)
- **Command**: `/harden`

#### H-3: Toggle cards — collapsed content still keyboard-focusable
- **Category**: Accessibility
- **Location**: `TradeManagement.tsx:198,271`
- **Description**: When Trailing Stop or Partial TP cards are collapsed (`max-height: 0`, `opacity: 0`), the inputs inside remain in the DOM and are reachable via Tab. This creates phantom tab stops. Missing: `aria-pressed` on toggle buttons, `aria-hidden` on collapsed content.
- **WCAG**: 2.1.1 Keyboard (Level A)
- **Command**: `/harden`

#### H-4: No button `:focus-visible` styles
- **Category**: Accessibility
- **Location**: `popup.css` — button base styles (line 168)
- **Description**: `input:focus` and `select:focus` have styled focus rings, but `button:focus-visible` has no custom style. On dark backgrounds the browser default outline is often invisible.
- **WCAG**: 2.4.7 Focus Visible (Level AA)
- **Command**: `/harden`

#### H-5: Div used as interactive element (ExchangeManager)
- **Category**: Accessibility
- **Location**: `ExchangeManager.tsx:168`
- **Description**: `<div class="... cursor-pointer" onClick={...}>` acts as a button to set active exchange but has no `role="button"`, `tabIndex`, or keyboard handler. Unreachable by keyboard, invisible to AT.
- **WCAG**: 2.1.1 Keyboard (Level A), 4.1.2 Name, Role, Value (Level A)
- **Command**: `/harden`

#### H-6: Three divergent greens across popup and modal
- **Category**: Theming
- **Description**: "Green" is expressed as three different values with no coordination:

| Value | Context |
|-------|---------|
| `#22c55e` (signal-green) | Popup design token — QuickTrade, PositionCard, ActiveOrders |
| `#34D399` (emerald-400) | Modal CSS, TradeManagement `riskColor()`, ArcGauge ticks |
| `#00FF41` (matrix green) | AuthSection login button |

- **Impact**: Color shifts between popup and modal; visual inconsistency erodes trust.
- **Command**: `/normalize`

#### H-7: Modal color system completely disconnected from popup tokens
- **Category**: Theming
- **Location**: `modal.tsx` lines 20-110, `TradeForm.tsx` inline styles
- **Description**: The modal uses raw hex values that diverge from popup tokens:

| Concept | Modal | Popup token | Match? |
|---------|-------|-------------|--------|
| Green (long) | `#34D399` | `#22c55e` | No |
| Red (short) | `#F87171` | `#ef4444` | No |
| Amber | `#FBBF24` | `#f59e0b` | No |
| Inactive text | `#71717A` | `#6b7280` | No |
| Label text | `#D4D4D8` | `#ffffff` | No |

- **Command**: `/normalize`

#### H-8: `signal-green` (#22c55e) fails AA on dark panels
- **Category**: Accessibility
- **Location**: Balance values, position counts, status indicators
- **Description**: `signal-green` on `bg-panel` yields **3.76:1** (needs 4.5:1). Used for balance amounts, position counts, and success states.
- **WCAG**: 1.4.3 Contrast Minimum (Level AA)
- **Command**: `/colorize`

#### H-9: Trade tab content clips when toggle cards expand
- **Category**: Responsive
- **Location**: `TradeManagement.tsx:57`
- **Description**: The Trade tab wrapper has no `scroll-area` class. When both Trailing Stop and Partial TP cards are expanded (~640px content), available height is ~418px. Content is silently clipped with no scroll affordance.
- **Command**: `/harden`

#### H-10: `transition: max-height` causes layout reflow
- **Category**: Performance
- **Location**: `popup.css:274-285`
- **Description**: The toggle card expand/collapse animates `max-height`, a layout property that triggers reflow on every animation frame. Should use `transform: scaleY()` or CSS Grid `grid-template-rows: 0fr → 1fr` transition.
- **Command**: `/optimize`

#### H-11: Storage writes on every slider `onInput` — no debounce
- **Category**: Performance
- **Location**: `TradeManagement.tsx:83,122,158,207,280`
- **Description**: Every pixel of slider drag fires `browser.storage.local.set()`. A 100-pixel drag queues 100 async storage writes. `setPreset()` should update UI immediately; `storage.local.set` should be debounced (200ms).
- **Command**: `/optimize`

#### H-12: Full `MODAL_STYLES` (6.2k) injected per toast Shadow DOM
- **Category**: Performance
- **Location**: `modal.tsx:184-216`
- **Description**: Each toast creates a new Shadow DOM and injects the complete 6.2k modal stylesheet. Only ~5 lines of toast CSS are needed. With `MAX_TOASTS = 3`, up to three full copies exist simultaneously.
- **Recommendation**: Create a minimal `TOAST_STYLES` constant with only toast-relevant rules.
- **Command**: `/optimize`

#### H-13: Duplicate `SIDECAR_STATUS_CHANGED` listeners
- **Category**: Performance
- **Location**: `HeaderBar.tsx:17`, `StatusBar.tsx:28`
- **Description**: Both components independently register `browser.runtime.onMessage` listeners for the same event and make duplicate `SIDECAR_STATUS` requests on mount. Two handlers fire, two signals update, two re-renders occur for every status change.
- **Recommendation**: Lift sidecar status to the parent (`HeaderBar`) and pass as a prop.
- **Command**: `/optimize`

#### H-14: Cancel button touch target critically small (17px)
- **Category**: Responsive
- **Location**: `PositionCard.tsx:136-146`
- **Description**: `px-3 py-1 text-[9px]` yields approximately 17px height for a destructive cancel action. WCAG 2.5.5 requires 44px minimum.
- **Command**: `/adapt`

---

### Medium-Severity Issues

#### M-1: No `prefers-reduced-motion` support
- **Category**: Accessibility
- **Location**: `popup.css:181-194` (`status-blink`), ArcGauge transitions, refresh spin
- **Description**: The infinite `status-blink` pulse animation and 700ms ArcGauge transitions ignore `prefers-reduced-motion`. Users with vestibular disorders may experience discomfort.
- **WCAG**: 2.3.3 Animation from Interactions (Level AAA)
- **Command**: `/animate`

#### M-2: No heading hierarchy — all section titles are `<span>`
- **Category**: Accessibility
- **Location**: All popup components
- **Description**: "Positions", "Active", "Pending", "Settings", "Risk Per Trade", etc. are all `<span>` elements. No `<h2>`/`<h3>` structure exists.
- **WCAG**: 1.3.1 Info and Relationships (Level A)
- **Command**: `/harden`

#### M-3: No landmark regions
- **Category**: Accessibility
- **Location**: `App.tsx`, `MainView.tsx`
- **Description**: No `<main>`, `<nav>`, `<header>`, `<footer>`, or equivalent `role` attributes. Screen reader users cannot navigate by landmarks.
- **WCAG**: 1.3.1 Info and Relationships (Level A)
- **Command**: `/harden`

#### M-4: LONG/SHORT toggles lack radio group semantics
- **Category**: Accessibility
- **Location**: `QuickTrade.tsx:140-158`, `TradeForm.tsx:182-191`
- **Description**: Side toggle buttons have no `role="radiogroup"`, `role="radio"`, or `aria-pressed`/`aria-checked`.
- **Command**: `/harden`

#### M-5: `transition: all` on all buttons and modal elements
- **Category**: Performance
- **Location**: `popup.css:168`, `modal.tsx:42`
- **Description**: `transition: all 150ms ease` watches every animatable property. Should scope to `background-color, border-color, color`.
- **Command**: `/optimize`

#### M-6: `backdrop-filter: blur(8px)` on modal — GPU-intensive
- **Category**: Performance
- **Location**: `modal.tsx:24`
- **Description**: Forces compositing and blur pass of everything behind the modal. Expensive on TradingView's dense canvas. Consider reducing to `blur(4px)` or removing.
- **Command**: `/optimize`

#### M-7: ArcGauge `transition-all` on 21 SVG circles
- **Category**: Performance
- **Location**: `ArcGauge.tsx:87`
- **Description**: All 21 tick circles have `transition-all duration-700 ease-out`. Only `r` and `opacity` change. Should scope transition to those properties.
- **Command**: `/optimize`

#### M-8: `riskColor()` called 3x per render, `isLong()` 4x per card
- **Category**: Performance
- **Location**: `TradeManagement.tsx:62,69,74`, `PositionCard.tsx:22-28,53,63`
- **Description**: Derived values computed repeatedly instead of memoized. `isLong()` calls `parseFloat` twice per invocation × 4 calls × N cards.
- **Recommendation**: Use `createMemo` for both.
- **Command**: `/optimize`

#### M-9: Icon-only buttons below 44px touch target
- **Category**: Responsive
- **Location**: `HeaderBar.tsx:40` (settings ~27px), `ActiveOrders.tsx:123` (refresh ~26px), `AuthSection.tsx:49` / `SettingsView.tsx:47` (back ~28px)
- **Description**: All icon-only buttons use `p-1.5` on ~15px icons, yielding ~27px targets. Below 44px minimum.
- **WCAG**: 2.5.5 Target Size (Level AAA)
- **Command**: `/adapt`

#### M-10: ON/OFF toggle buttons ~19px tall
- **Category**: Responsive
- **Location**: `TradeManagement.tsx:181-196`
- **Description**: `py-1 text-[11px]` yields ~19px height for the Trailing Stop and Partial TP toggles.
- **Command**: `/adapt`

#### M-11: Symbol text has no overflow protection
- **Category**: Responsive
- **Location**: `PositionCard.tsx:59`, `ActiveOrders.tsx:187`
- **Description**: Long symbols like "BTC_USDT_PERP" will overflow into adjacent elements. No `truncate` or `overflow-hidden`.
- **Command**: `/harden`

#### M-12: `forwardOrderUpdate` calls `tabs.query` on every WS message
- **Category**: Performance
- **Location**: `background.ts:1014-1026`
- **Description**: `browser.tabs.query` is an async API call fired on every WebSocket order event. Tab list should be cached and invalidated via `onCreated`/`onRemoved`.
- **Command**: `/optimize`

#### M-13: Pervasive `zinc-*` Tailwind utilities instead of design tokens
- **Category**: Theming
- **Location**: `ActiveOrders.tsx` (15+ instances), `MainView.tsx` (10+ instances), `TradeManagement.tsx` (8+ instances)
- **Description**: `text-zinc-400`, `text-zinc-500`, `bg-zinc-800`, `text-zinc-200`, `text-zinc-300` used throughout instead of `text-text-secondary`, `text-text-dim`, `bg-bg-elevated`, `text-text-primary`. Creates a parallel color system that won't update if tokens change.
- **Command**: `/normalize`

#### M-14: popup.css raw rgba values repeat token RGB without using `var()`
- **Category**: Theming
- **Location**: `popup.css:108,148,218,225,241,330-344,406-410`
- **Description**: Focus shadows, glow utilities, and balance panel overlay gradient hard-code `rgba(148,163,184,...)` (accent-steel RGB) instead of referencing `var(--color-accent-steel)`.
- **Command**: `/normalize`

#### M-15: Shadow DOM font loading fragile
- **Category**: Performance / Theming
- **Location**: `modal.tsx` `MODAL_STYLES`
- **Description**: Modal references `'DM Sans'` and `'JetBrains Mono'` but declares no `@font-face` inside the shadow root. Fonts load by luck from the host page.
- **Command**: `/harden`

#### M-16: `positions()` and `pendingOrders()` computed redundantly
- **Category**: Performance
- **Location**: `ActiveOrders.tsx:113-115`
- **Description**: `trades().filter(...)` runs in both `createEffect` and JSX template — double computation. Should use `createMemo`.
- **Command**: `/optimize`

---

### Low-Severity Issues

#### L-1: Auth inputs use `border-radius: 6px` while global is `12px`
- **Location**: `AuthSection.tsx:20`
- **Category**: Theming
- **Command**: `/normalize`

#### L-2: `StatusBar` uses `bg-green-500` instead of `bg-signal-green`
- **Location**: `StatusBar.tsx:16`
- **Category**: Theming
- **Command**: `/normalize`

#### L-3: `text-emerald-400`, `text-amber-400`, `text-red-400` in risk labels
- **Location**: `TradeManagement.tsx:89-91`
- **Category**: Theming
- **Command**: `/normalize`

#### L-4: ArcGauge "Exposure" label uses `text-zinc-400`
- **Location**: `ArcGauge.tsx:99`
- **Category**: Theming
- **Command**: `/normalize`

#### L-5: Three unused font files shipped in source
- **Location**: `src/fonts/` — cinzel-variable.woff2, space-mono-regular.woff2, space-mono-bold.woff2
- **Category**: Performance (bundle)
- **Command**: `/distill`

#### L-6: 16k base64 JPEG embedded in CSS
- **Location**: `popup.css:390-399`
- **Category**: Performance
- **Description**: Roman ruins texture encoded inline. Could be a separate static asset.
- **Command**: `/optimize`

#### L-7: No `drop: ["console"]` in esbuild production config
- **Location**: `build.ts:27-37`
- **Category**: Performance
- **Description**: ~12 console.log/warn calls remain in production background worker.
- **Command**: `/optimize`

#### L-8: `ExchangeManager.tsx` is orphaned dead code
- **Location**: `src/popup/components/ExchangeManager.tsx`
- **Category**: Performance (bundle)
- **Description**: Complete CRUD component rendered nowhere. Tree-shaking may remove it, but it adds maintenance burden.
- **Command**: `/distill`

#### L-9: Trail badge hardcoded "Trail: OFF"
- **Location**: `PositionCard.tsx:130-135`
- **Category**: Theming / Data
- **Description**: Static text regardless of actual trailing stop state. Data display bug.
- **Command**: `/harden`

#### L-10: `ExchangeSelector` click-outside uses document-level listener
- **Location**: `ExchangeSelector.tsx:73-79`
- **Category**: Performance
- **Description**: Registers `document.addEventListener("click")` on mount. Cleanup exists. Minor overhead.

#### L-11: No `required` attribute on mandatory form fields
- **Location**: All form inputs across 7 files
- **Category**: Accessibility
- **Command**: `/harden`

#### L-12: Auto-badge spans not keyboard-accessible
- **Location**: `TradeForm.tsx:203-205,224-226,242-244,260-262`
- **Category**: Accessibility
- **Description**: `<span onClick>` elements for clearing auto-filled values. No `role="button"`, no `tabIndex`, no keyboard handler.
- **Command**: `/harden`

#### L-13: SVG icons inside buttons lack `aria-hidden="true"`
- **Location**: All icon buttons (`HeaderBar`, `SettingsView`, `ActiveOrders`, `AuthSection`, `ExchangeSelector`)
- **Category**: Accessibility
- **Command**: `/harden`

#### L-14: `Forgot password` link is a non-interactive span
- **Location**: `AuthSection.tsx:113-119`
- **Category**: Accessibility
- **Description**: `<span onClick tabIndex={-1}>` — not keyboard accessible, no button role.
- **Command**: `/harden`

#### L-15: Show/hide password button removed from tab order
- **Location**: `AuthSection.tsx:132-147`
- **Category**: Accessibility
- **Description**: `tabIndex={-1}` prevents keyboard-only users from toggling password visibility.
- **Command**: `/harden`

---

## Patterns & Systemic Issues

1. **No form label associations anywhere.** This is not a per-component oversight — it's a systematic pattern. Every `<label>` in the codebase lacks `for`, every `<input>` lacks `id`. A single utility pattern (e.g., `const fieldId = (name: string) => \`field-${name}\``) applied once would fix all 20 inputs.

2. **Raw Tailwind zinc palette used as a shadow token system.** `text-zinc-400`, `text-zinc-500`, `bg-zinc-800` appear 30+ times across components instead of the defined design tokens. This creates a parallel, implicit color system that won't respond to theme changes.

3. **No `aria-live` regions anywhere.** Seven error/status display locations are completely silent to AT. A single shared `<div aria-live="polite">` pattern with a Solid signal would solve all of them.

4. **Modal is architecturally isolated from the design system.** The Shadow DOM modal uses 40+ raw hex values that diverge from popup tokens. This isn't a bug — it's a missing design-system bridge. CSS custom properties can be injected into shadow roots.

5. **Small touch targets on all icon-only buttons.** Every `p-1.5` icon button in the extension (settings, refresh, back, exchange selector) falls below 44px. A single CSS change (`min-w-[44px] min-h-[44px]`) on the icon button base class would fix all instances.

---

## Positive Findings

1. **Signal color semantics are well-designed.** Green = long/profit, red = short/loss, orange = pending/caution, steel = neutral. The traffic-light risk slider is excellent information design. Once token consolidation happens, this semantic system will be strong.

2. **Balance panel `aria-hidden` on decorative overlay.** `MainView.tsx:154` correctly hides the decorative gradient overlay from AT. The background texture is a CSS image (invisible to AT by default). This shows awareness of accessibility patterns — the patterns just weren't applied systematically.

3. **Solid.js cleanup patterns are correct.** `ExchangeSelector` properly pairs `onMount`/`onCleanup` for event listeners. `StatusBar` and `HeaderBar` both clean up message listeners. No memory leaks in the popup lifecycle.

4. **Double-confirm safety on live trades.** Both `TradeForm` and `QuickTrade` implement a two-step confirmation for live orders. This is a genuine safety feature that protects real money.

5. **ArcGauge is distinctive and informative.** Custom SVG, tick interpolation, glow filter, smooth transitions. Communicates exposure risk at a glance. The green→amber→red gradient on ticks is excellent.

---

## Recommendations by Priority

### Immediate (blocks core accessibility)
1. Fix Shadow DOM `mode: "closed"` → `mode: "open"` (C-1)
2. Add `for`/`id` pairs to all form labels (C-2) — `/harden`
3. Define missing `accent-green` and `signal-blue` tokens (C-3, C-4) — `/normalize`
4. Add `role="tablist"` / `role="tab"` / `aria-selected` to TabBar (C-5) — `/harden`

### Short-term (this sprint)
5. Lighten `text-secondary` and `text-dim` for AA contrast (C-6, C-7) — `/colorize`
6. Consolidate three greens into one token (H-6) — `/normalize`
7. Bridge modal color system to popup tokens (H-7) — `/normalize`
8. Add `role="alert"` / `aria-live` to all dynamic messages (H-1) — `/harden`
9. Add `aria-expanded`, `aria-haspopup` to ExchangeSelector (H-2) — `/harden`
10. Fix toggle card keyboard trap (H-3) — `/harden`
11. Add `button:focus-visible` styles (H-4) — `/harden`
12. Debounce storage writes on sliders (H-11) — `/optimize`

### Medium-term (next sprint)
13. Add `prefers-reduced-motion` support (M-1) — `/animate`
14. Add semantic headings and landmarks (M-2, M-3) — `/harden`
15. Scope `transition: all` to specific properties (M-5, M-7) — `/optimize`
16. Replace raw `zinc-*` utilities with design tokens (M-13) — `/normalize`
17. Increase touch targets on icon buttons (M-9) — `/adapt`
18. Fix `max-height` animation with grid trick (H-10) — `/optimize`
19. Add scroll container to Trade tab (H-9) — `/harden`

### Long-term (nice-to-haves)
20. Extract unused fonts and dead code (L-5, L-8) — `/distill`
21. Move base64 JPEG to separate asset (L-6) — `/optimize`
22. Add `drop: ["console"]` to production build (L-7) — `/optimize`
23. Cache `tabs.query` results (M-12) — `/optimize`
24. Fix trail badge data display bug (L-9)
25. Create minimal `TOAST_STYLES` (H-12) — `/optimize`

---

## Suggested Commands Summary

| Command | Issues addressed | Count |
|---------|-----------------|-------|
| `/normalize` | C-3, C-4, H-6, H-7, M-13, M-14, L-1, L-2, L-3, L-4 | 10 |
| `/harden` | C-1, C-2, C-5, H-1, H-2, H-3, H-4, H-5, H-9, M-2, M-3, M-4, M-11, M-15, L-9, L-11–L-15 | 20 |
| `/optimize` | H-10, H-11, H-12, H-13, M-5, M-6, M-7, M-8, M-12, M-16, L-6, L-7 | 12 |
| `/colorize` | C-6, C-7, H-8 | 3 |
| `/adapt` | H-14, M-9, M-10 | 3 |
| `/animate` | M-1 | 1 |
| `/distill` | L-5, L-8 | 2 |
