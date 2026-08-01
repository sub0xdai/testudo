# Specification: Accessibility Hardening

**Spec ID:** EXT-27-accessibility-hardening
**Date:** 2026-03-14
**Status:** Draft
**Class:** Audit Fix
**Priority:** P0 — core accessibility failures block AT users entirely
**Audit Refs:** C-1, C-2, C-5, H-1, H-2, H-3, H-4, H-5, H-9, H-14, M-2, M-3, M-4, M-9, M-10, M-11, M-15, L-9, L-11, L-12, L-13, L-14, L-15
**Critique Refs:** ISSUE-2 (Shadow DOM Font Loading), ISSUE-5 (Hidden Scrollbars)
**Depends on:** EXT-26 (token normalization should land first — contrast fixes feed into this work)

---

## Overview

The extension has zero programmatic accessibility. The trade confirmation modal is invisible to screen readers (closed Shadow DOM), no form labels are associated, no ARIA patterns exist, and critical touch targets are below minimum size. This spec addresses all 25 accessibility-related audit findings.

**Current state:**
- Shadow DOM `mode: "closed"` makes modal invisible to assistive technology (C-1)
- 20 form inputs across 7 files have no `for`/`id` label association (C-2)
- Tab bar uses plain buttons with no tab role semantics (C-5)
- 7 dynamic error/status messages have no `aria-live` regions (H-1)
- ExchangeSelector dropdown lacks ARIA, keyboard navigation, Escape to close (H-2)
- Collapsed toggle card content remains keyboard-focusable (H-3)
- No `button:focus-visible` styles — browser default invisible on dark backgrounds (H-4)
- Interactive div in ExchangeManager unreachable by keyboard (H-5)
- Trade tab clips content with no scroll affordance (H-9)
- Cancel button is 17px tall — WCAG 2.5.5 requires 44px minimum (H-14)
- No heading hierarchy (`<h2>`, `<h3>`) — all titles are `<span>` (M-2)
- No landmark regions (`<main>`, `<nav>`, `<header>`) (M-3)
- LONG/SHORT toggles lack radio group semantics (M-4)
- Icon-only buttons ~27px — below 44px minimum (M-9)
- ON/OFF toggles ~19px tall (M-10)
- Long symbol text has no overflow protection (M-11)
- Modal fonts load by luck — no `@font-face` in Shadow DOM (M-15)
- Trail badge hardcoded "Trail: OFF" regardless of actual state (L-9)
- No `required` attribute on mandatory form fields (L-11)
- Auto-badge `<span onClick>` not keyboard-accessible (L-12)
- SVG icons inside buttons lack `aria-hidden="true"` (L-13)
- "Forgot password" is a non-interactive `<span>` (L-14)
- Password visibility toggle removed from tab order with `tabIndex={-1}` (L-15)
- Scrollbars globally hidden — no visual scroll affordance (Critique ISSUE-5)

**Target state:**
- Modal uses `mode: "open"` with focus trapping, `role="dialog"`, `aria-modal="true"`
- All form inputs have programmatic label associations
- Full ARIA tab pattern on navigation, ARIA listbox on ExchangeSelector
- All dynamic messages announced via `aria-live` regions
- All interactive elements are keyboard-accessible with visible focus indicators
- All touch targets ≥44px
- Semantic heading hierarchy and landmark regions throughout
- Thin custom scrollbars replace hidden scrollbars
- Modal has `@font-face` declarations for reliable font loading

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Change Shadow DOM to `mode: "open"` on modal and toast containers. Add `role="dialog"`, `aria-modal="true"`, `aria-label="Trade Confirmation"` to modal. | Critical | Modal |
| FR-2 | Implement focus trap in modal — Tab cycles within modal, Escape closes. | Critical | Modal |
| FR-3 | Add unique `id` to every `<input>` and matching `for` to every `<label>`. Pattern: `id="field-{name}"`, `for="field-{name}"`. | Critical | All forms |
| FR-4 | Add `role="tablist"` to tab container, `role="tab"` + `aria-selected` to each tab button, `role="tabpanel"` + `aria-labelledby` to each content area. | Critical | TabBar |
| FR-5 | Add `role="alert"` or `aria-live="polite"` to all 7 dynamic error/status message locations. | High | Multiple |
| FR-6 | Add `aria-haspopup="listbox"`, `aria-expanded`, `role="listbox"`, `role="option"`, `aria-selected` to ExchangeSelector. Add Escape to close, arrow key navigation. | High | ExchangeSelector |
| FR-7 | Add `aria-hidden="true"` on collapsed toggle card content. Add `aria-pressed` to toggle buttons. Remove collapsed inputs from tab order. | High | TradeManagement |
| FR-8 | Add `button:focus-visible` style with visible ring. Pattern: `outline: 2px solid var(--color-accent-steel); outline-offset: 2px`. | High | popup.css |
| FR-9 | Replace interactive `<div>` in ExchangeManager with `<button>` (or add `role="button"`, `tabIndex={0}`, keyboard handler). | High | ExchangeManager |
| FR-10 | Add `scroll-area` class or `overflow-y: auto` to Trade tab content wrapper. | High | TradeManagement |
| FR-11 | Increase cancel button to ≥44px touch target. Increase all icon-only buttons to min-w-[44px] min-h-[44px]. Increase ON/OFF toggles. | High | Multiple |
| FR-12 | Add `<h2>` for section titles (Positions, Active, Settings, etc.) with appropriate hierarchy. | Medium | Multiple |
| FR-13 | Add `<main>` wrapper around primary content, `<nav>` around TabBar, `<header>` around HeaderBar. | Medium | App/MainView |
| FR-14 | Add `role="radiogroup"` to LONG/SHORT toggle container, `role="radio"` + `aria-checked` to each option. | Medium | QuickTrade, TradeForm |
| FR-15 | Add `text-overflow: ellipsis` + `overflow: hidden` + `white-space: nowrap` to symbol text. | Medium | PositionCard, ActiveOrders |
| FR-16 | Add `@font-face` declarations to modal `MODAL_STYLES` using `chrome.runtime.getURL()` for woff2 paths. Add fonts to `web_accessible_resources` in manifest. | Medium | Modal, manifest.json |
| FR-17 | Fix Trail badge to reflect actual trailing stop state from trade data. | Medium | PositionCard |
| FR-18 | Add `required` attribute to all mandatory form inputs. | Low | All forms |
| FR-19 | Replace auto-badge `<span onClick>` with `<button>` elements. | Low | TradeForm |
| FR-20 | Add `aria-hidden="true"` to all decorative SVG icons inside buttons. | Low | All icon buttons |
| FR-21 | Replace "Forgot password" `<span>` with `<a>` or `<button>` with proper keyboard access. | Low | AuthSection |
| FR-22 | Remove `tabIndex={-1}` from password visibility toggle. | Low | AuthSection |
| FR-23 | Replace `scrollbar-width: none !important` with thin custom scrollbars: `scrollbar-width: thin; scrollbar-color: rgba(148,163,184,0.3) transparent`. | Medium | popup.css |
| FR-24 | Add document-level `aria-live="polite"` region that mirrors toast messages for screen readers. | High | Modal (toasts) |

---

## Technical Implementation

### 1) Shadow DOM Open Mode + Focus Trap (FR-1, FR-2)

```typescript
// modal.tsx — change both instances
// Before:
const shadow = container.attachShadow({ mode: "closed" });

// After:
const shadow = container.attachShadow({ mode: "open" });

// Add to modal container element:
modalEl.setAttribute("role", "dialog");
modalEl.setAttribute("aria-modal", "true");
modalEl.setAttribute("aria-label", "Trade Confirmation");
```

Focus trap implementation:
```typescript
function trapFocus(container: HTMLElement) {
  const focusable = container.querySelectorAll(
    'button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
  );
  const first = focusable[0] as HTMLElement;
  const last = focusable[focusable.length - 1] as HTMLElement;

  container.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { closeModal(); return; }
    if (e.key !== "Tab") return;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault(); last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault(); first.focus();
    }
  });
  first?.focus();
}
```

### 2) Form Label Association (FR-3)

Apply across all 7 files. Pattern:
```tsx
// Before:
<label class="...">Risk Per Trade</label>
<input type="range" ... />

// After:
<label for="field-risk-percent" class="...">Risk Per Trade</label>
<input id="field-risk-percent" type="range" ... />
```

Full list of inputs requiring `id`/`for`:
| File | Inputs |
|------|--------|
| `AuthSection.tsx` | `field-email`, `field-password` |
| `TradeManagement.tsx` | `field-risk-percent`, `field-risk-range`, `field-leverage`, `field-leverage-range`, `field-breakeven`, `field-breakeven-range`, `field-trail-distance`, `field-trail-range`, `field-partial-percent`, `field-partial-range` |
| `QuickTrade.tsx` | `field-qt-symbol`, `field-qt-entry`, `field-qt-stop`, `field-qt-target` |
| `SettingsView.tsx` | `field-backend-url`, `field-ws-url`, `field-web-url` |
| `ExchangeManager.tsx` | `field-exchange-select`, `field-api-key`, `field-api-secret`, `field-api-passphrase` |
| `TradeForm.tsx` | `field-tf-symbol`, `field-tf-entry`, `field-tf-stop`, `field-tf-target` |

### 3) Tab Pattern (FR-4)

```tsx
// TabBar.tsx
<div role="tablist" aria-label="Main navigation">
  <For each={tabs}>
    {(tab) => (
      <button
        role="tab"
        aria-selected={activeTab() === tab.id}
        aria-controls={`panel-${tab.id}`}
        id={`tab-${tab.id}`}
      >
        {tab.label}
      </button>
    )}
  </For>
</div>

// Content areas:
<div role="tabpanel" id="panel-trade" aria-labelledby="tab-trade">
  ...
</div>
```

### 4) Dynamic Error Announcements (FR-5)

Add to each error/status location:
```tsx
<div role="alert" class="...">{errorMessage()}</div>
// or for non-urgent:
<div aria-live="polite" class="...">{statusMessage()}</div>
```

Locations: `AuthSection.tsx:83`, `HeaderBar.tsx:54`, `QuickTrade.tsx:230`, `ActiveOrders.tsx:142,146`, `ExchangeManager.tsx:241`, `SettingsView.tsx:76`

### 5) Focus Visible Styles (FR-8)

```css
/* popup.css — add after existing input focus styles */
button:focus-visible,
[role="tab"]:focus-visible,
[role="button"]:focus-visible {
  outline: 2px solid var(--color-accent-steel);
  outline-offset: 2px;
}
```

### 6) Touch Target Minimum (FR-11)

```css
/* popup.css — icon button base */
.icon-btn {
  min-width: 44px;
  min-height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
```

Apply class to: settings gear (HeaderBar), refresh (ActiveOrders), back arrows (AuthSection, SettingsView), exchange selector trigger.

For cancel button in PositionCard: change from `px-3 py-1 text-[9px]` to `px-4 py-2.5 text-xs` (≥44px height).

### 7) Custom Scrollbars (FR-23)

```css
/* popup.css — replace scrollbar hiding */
/* Before: */
* { scrollbar-width: none !important; }
*::-webkit-scrollbar { display: none !important; }

/* After: */
* {
  scrollbar-width: thin;
  scrollbar-color: rgba(148,163,184,0.3) transparent;
}
*::-webkit-scrollbar { width: 6px; }
*::-webkit-scrollbar-track { background: transparent; }
*::-webkit-scrollbar-thumb {
  background: rgba(148,163,184,0.3);
  border-radius: 3px;
}
```

### 8) Shadow DOM Font Loading (FR-16)

```typescript
// modal.tsx — prepend to MODAL_STYLES
const fontCSS = `
  @font-face {
    font-family: 'DM Sans';
    src: url('${chrome.runtime.getURL("fonts/dm-sans-variable.woff2")}') format('woff2');
    font-weight: 100 900;
    font-display: swap;
  }
  @font-face {
    font-family: 'JetBrains Mono';
    src: url('${chrome.runtime.getURL("fonts/jetbrains-mono-regular.woff2")}') format('woff2');
    font-weight: 400;
    font-display: swap;
  }
`;
```

Add to `manifest.json`:
```json
"web_accessible_resources": [{
  "resources": ["fonts/*.woff2"],
  "matches": ["<all_urls>"]
}]
```

### 9) Trail Badge Fix (FR-17)

```tsx
// PositionCard.tsx — replace hardcoded text
// Before:
<span>Trail: OFF</span>

// After:
<span>Trail: {trade.trailing_stop_enabled ? "ON" : "OFF"}</span>
```

---

## Affected Files

| File | Changes |
|------|---------|
| `src/modal.tsx` | Open shadow mode, focus trap, dialog role, font-face, toast aria-live |
| `src/popup/popup.css` | Focus-visible styles, scrollbar fix, icon-btn class |
| `src/popup/components/TabBar.tsx` | ARIA tab pattern |
| `src/popup/components/AuthSection.tsx` | Label IDs, forgot password, password toggle tabindex |
| `src/popup/components/TradeManagement.tsx` | Label IDs, toggle card aria-hidden, scroll wrapper |
| `src/popup/components/QuickTrade.tsx` | Label IDs, radio group semantics |
| `src/popup/components/ExchangeSelector.tsx` | ARIA listbox, keyboard navigation |
| `src/popup/components/ExchangeManager.tsx` | Interactive div → button, label IDs |
| `src/popup/components/ActiveOrders.tsx` | Error alerts, symbol overflow, heading |
| `src/popup/components/PositionCard.tsx` | Cancel button size, trail badge fix, symbol overflow |
| `src/popup/components/MainView.tsx` | Landmarks, headings |
| `src/popup/components/HeaderBar.tsx` | Icon button size, error alert |
| `src/popup/components/StatusBar.tsx` | Error alert |
| `src/popup/components/SettingsView.tsx` | Label IDs, error alert |
| `src/popup/App.tsx` | Main landmark wrapper |
| `src/components/TradeForm.tsx` | Label IDs, radio group, auto-badge buttons |
| `manifest.json` | web_accessible_resources for fonts |

---

## Verification

```bash
cd testudo-extension && bun run build
```

- [ ] Build succeeds with no errors
- [ ] Modal uses `mode: "open"` — `grep 'mode.*closed' src/modal.tsx` returns nothing
- [ ] Every `<label>` has a `for` attribute — `grep -c 'for=' src/**/*.tsx` matches input count
- [ ] Tab bar has `role="tablist"` — `grep 'role="tablist"' src/` returns match
- [ ] Dynamic errors use `role="alert"` or `aria-live` — `grep -c 'role="alert"\|aria-live' src/` ≥ 7
- [ ] `scrollbar-width: none` removed — `grep 'scrollbar-width.*none' src/popup/popup.css` returns nothing
- [ ] No `tabIndex={-1}` on password toggle — check AuthSection.tsx
- [ ] All icon-only buttons have `min-width: 44px` or `min-height: 44px`
- [ ] Cancel button ≥ 44px height (inspect in DevTools)
- [ ] Trail badge shows actual state, not hardcoded "OFF"
- [ ] Modal font-face declarations present in MODAL_STYLES
- [ ] Manual: Tab through entire popup — every interactive element receives visible focus
- [ ] Manual: open modal, Tab stays trapped within modal, Escape closes
- [ ] Manual: content scrolls with visible thin scrollbar on overflow

---

*Consolidates audit issues C-1, C-2, C-5, H-1, H-2, H-3, H-4, H-5, H-9, H-14, M-2, M-3, M-4, M-9, M-10, M-11, M-15, L-9, L-11, L-12, L-13, L-14, L-15 and critique issues 2, 5.*
