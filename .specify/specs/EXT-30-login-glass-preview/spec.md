# Specification: Login Glass Preview

**Spec ID:** EXT-30-login-glass-preview
**Date:** 2026-03-14
**Status:** Draft
**Class:** Cosmetic Enhancement
**Priority:** P3 — visual polish, conversion UX
**Depends on:** EXT-26 (design tokens), EXT-29 (UX polish)

---

## Overview

Replace the opaque full-screen login page with a frosted glass card overlaying a static preview of the extension dashboard. This gives potential users a glimpse of the product before signing up — a proven SaaS conversion pattern.

**Current state:**
- Login screen is a full-page dark background (`bg-bg-core`) with centered form
- No visual indication of what the product looks like before authentication
- `AuthSection` occupies the entire 520x680 popup

**Target state:**
- A static, blurred mockup of the main dashboard visible behind the login
- Login form rendered as a centered frosted glass card
- Background mock is purely decorative — hardcoded HTML/CSS, no API calls

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create a `LoginPreview` component that renders a static mockup of the main dashboard with hardcoded demo data | High | popup/components |
| FR-2 | Apply `filter: blur(6px)` and `opacity: 0.4` to the preview background layer | High | popup.css |
| FR-3 | Wrap the existing login form in a frosted glass card with `backdrop-filter: blur(16px)` and semi-transparent background | High | AuthSection |
| FR-4 | Center the glass card vertically within the popup, leaving the preview visible above and below | High | AuthSection |
| FR-5 | Ensure the preview is `aria-hidden="true"`, `pointer-events: none`, `user-select: none` — purely decorative | Medium | LoginPreview |
| FR-6 | Preview must not trigger any `browser.runtime.sendMessage` calls or storage reads | High | LoginPreview |

---

## Technical Implementation

### 1) LoginPreview Component (FR-1, FR-5, FR-6)

**File:** `src/popup/components/LoginPreview.tsx`

A stateless component with zero runtime dependencies. Renders hardcoded HTML that mimics the MainView layout:

```
┌─────────────────────────────┐
│  ⚙          TESTUDO SNIPER │  ← Mock header bar
├─────────────────────────────┤
│         Balance             │
│       $12,450.00            │  ← Hardcoded demo balance
│    $10,200 avail · $2,250   │
│                             │
│     ╭── Arc Gauge ──╮       │  ← Static SVG arc at ~18%
│     ╰───────────────╯       │
├─────────────────────────────┤
│  Trade │ Quick │ Pos │ Acct │  ← Mock tab bar
├─────────────────────────────┤
│  ┌─ Stop Loss ─────── ON ┐  │
│  │ Trailing: 1.5%        │  │  ← Mock toggle cards
│  └───────────────────────┘  │
│  ┌─ Take Profit ───── ON ┐  │
│  │ Partial: 50% @ 2:1    │  │
│  └───────────────────────┘  │
│  ┌─ Break Even ────── ON ┐  │
│  │ Trigger: 1:1 R        │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
```

Key constraints:
- No imports from `webextension-polyfill`, `AuthContext`, or any service modules
- All values are string literals
- Uses the same Tailwind classes and design tokens as the real UI for visual consistency

### 2) Glass Card Styling (FR-2, FR-3)

**File:** `src/popup/popup.css`

```css
/* Login glass overlay */
.login-preview-bg {
  filter: blur(6px);
  opacity: 0.4;
  pointer-events: none;
  user-select: none;
}

.login-glass-card {
  background: rgba(11, 14, 17, 0.82);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 16px;
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.4),
    inset 0 1px 0 rgba(255, 255, 255, 0.04);
}
```

### 3) AuthSection Layout Change (FR-3, FR-4)

**File:** `src/popup/components/AuthSection.tsx`

Current structure:
```tsx
<div class="flex flex-col h-full">
  <!-- back button -->
  <div class="flex-1 flex flex-col items-center justify-center px-8 py-6">
    <!-- logo, form -->
  </div>
</div>
```

New structure:
```tsx
<div class="relative w-full h-full overflow-hidden">
  {/* Background: static preview */}
  <div class="absolute inset-0 login-preview-bg" aria-hidden="true">
    <LoginPreview />
  </div>

  {/* Foreground: glass login card */}
  <div class="absolute inset-0 flex items-center justify-center p-6">
    <div class="login-glass-card w-full max-w-[440px] px-8 py-8">
      <!-- existing logo, divider, form fields -->
    </div>
  </div>
</div>
```

### 4) App.tsx — No Changes Required

The `Switch/Match` in `App.tsx` already renders `AuthSection` as the auth view. No routing changes needed.

---

## Design Constraints

- **Glass opacity balance:** The card must be opaque enough to read form fields comfortably but translucent enough that the preview is discernible. Target: `rgba(11,14,17,0.82)` with `blur(16px)`.
- **Preview blur:** Background blur should make text unreadable but shapes recognizable. Target: `blur(6px)` + `opacity: 0.4`.
- **Space budget:** The popup is 520x680. The glass card should be ~440px wide with 8px padding on each side, leaving ~20px of preview visible at top/bottom edges.
- **No layout shift:** The transition from login to main view should feel natural — the mock positions are close to where real content appears.

---

## Files Modified

| File | Change |
|------|--------|
| `src/popup/components/LoginPreview.tsx` | **New** — static dashboard mockup |
| `src/popup/components/AuthSection.tsx` | Wrap form in glass card, add preview background |
| `src/popup/popup.css` | Add `.login-preview-bg` and `.login-glass-card` classes |

---

## Acceptance Criteria

- [ ] Login screen shows a blurred preview of the extension dashboard behind the form
- [ ] Login form is readable and usable — no text contrast issues from transparency
- [ ] Preview is fully inert: no API calls, no pointer events, no screen reader exposure
- [ ] `bun run build` passes with no errors
- [ ] Glass card looks cohesive with the existing dark theme and design tokens
- [ ] Transition from auth → main view feels natural (preview ≈ real layout positions)

---

## Out of Scope

- Animated transitions between login and main view (could be a follow-up)
- Dynamic/live preview data
- Mobile/responsive variations (popup has fixed dimensions)
