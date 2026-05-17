# Specification: Extension Pairing UX — Six-Box OTP Input, Auto-Paste, State Feedback

**Spec ID:** EXT-39-pair-ux
**Date:** 2026-03-24
**Status:** Draft
**Class:** Feature / UX
**Priority:** P1 — Pairing works but has friction; UX improvements reduce time-to-paired from ~10s to ~3s
**Depends on:** AUTH-03-frontend-auth (PairView exists)
**Series:** EXT-39 (standalone)

---

## Problem Statement

The extension's `PairView.tsx` implements a functional pairing flow but has UX friction that slows users down. The current implementation (`testudo-extension/src/popup/components/PairView.tsx`, 173 lines) has four issues:

1. **Instructions are a wall of text** (line 99-108): A single sentence "Log in at testudo.xyz, then click Pair Extension in your account settings to get a 6-digit code." requires cognitive load to parse. Users scan, they don't read.

2. **The `000000` placeholder is a trap** (line 130): When an input field looks filled, users click in and backspace before typing. The field should look empty.

3. **Single text field instead of OTP boxes**: The industry standard for 6-digit codes is six individual input boxes. This visually communicates "exactly 6 digits" and enables per-digit cursor management.

4. **No auto-focus, no auto-paste, no success state**: The cursor doesn't auto-focus on popup open. Pasting a code requires manually clicking the field. After successful pairing there's no visual confirmation — it silently transitions to the main view.

The extension popup is a high-frequency surface for a trading terminal. Every extra click or cognitive pause is friction. This spec surgically addresses each issue.

---

## User Stories

- **As a trader opening the extension**, I want the cursor to be in the code input immediately, so that I can start typing without clicking.
- **As a trader pasting a code**, I want Ctrl+V to populate all six boxes and auto-submit, so that pairing is a single keypress after copying.
- **As a trader entering a code manually**, I want the cursor to auto-advance to the next box after each digit, so that I type naturally without tabbing.
- **As a trader**, I want clear visual feedback (success checkmark, error message, loading spinner, disabled state) so that I know exactly what's happening at every step.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace instruction paragraph with numbered steps: (1) link to settings, (2) generate code, (3) paste below | High | PairView |
| FR-2 | Settings link opens `${backendUrl}/account` in new tab (not hardcoded domain) | High | PairView |
| FR-3 | Replace single input with six individual center-aligned input boxes (OTP pattern) | High | PairView |
| FR-4 | Each box accepts exactly one digit; typing auto-advances cursor to the next box | High | PairView |
| FR-5 | Backspace in an empty box moves cursor to the previous box | High | PairView |
| FR-6 | Ctrl+V (paste) fills all six boxes from clipboard and auto-submits | High | PairView |
| FR-7 | Manual typing requires Enter key or PAIR button click to submit (no auto-submit on 6th digit) | High | PairView |
| FR-8 | PAIR button is disabled (lower opacity) until exactly 6 digits are entered | High | PairView |
| FR-9 | Auto-focus first input box on component mount (cursor blinking immediately) | High | PairView |
| FR-10 | Loading state: spinner in PAIR button, all inputs disabled, button text "PAIRING..." | Medium | PairView |
| FR-11 | Error state: red text below inputs with contextual message (expired, invalid, network) | Medium | PairView |
| FR-12 | Success state: brief checkmark animation (~800ms) before transitioning to main view | Medium | PairView |
| FR-13 | Placeholder: subtle underscore `_` in each box (not zeros) | Medium | PairView |
| FR-14 | Code expiry hint: "Code expires in 5 minutes" below the input boxes | Low | PairView |
| FR-15 | Session expired context: if returning to PairView after token expiry, show "Session expired — pair again to continue" above instructions | Low | PairView |

---

## Technical Implementation

### 1. Six-Box OTP Component

```tsx
// Solid.js OTP input — six individual refs managed via array
function OtpInput(props: {
  value: string;
  onChange: (val: string) => void;
  onComplete: (val: string) => void;
  disabled: boolean;
}) {
  let refs: HTMLInputElement[] = [];

  function handleInput(index: number, e: InputEvent) {
    const digit = (e.target as HTMLInputElement).value.replace(/\D/g, "").slice(-1);
    const digits = props.value.split("");
    digits[index] = digit;
    const newVal = digits.join("").slice(0, 6);
    props.onChange(newVal);
    // Auto-advance to next box
    if (digit && index < 5) refs[index + 1]?.focus();
  }

  function handleKeyDown(index: number, e: KeyboardEvent) {
    if (e.key === "Backspace" && !props.value[index] && index > 0) {
      refs[index - 1]?.focus();
    }
    if (e.key === "Enter" && props.value.length === 6) {
      props.onComplete(props.value);
    }
  }

  function handlePaste(e: ClipboardEvent) {
    e.preventDefault();
    const pasted = (e.clipboardData?.getData("text") || "").replace(/\D/g, "").slice(0, 6);
    if (pasted.length === 6) {
      props.onChange(pasted);
      refs[5]?.focus();
      // Auto-submit on paste — the "Ctrl+V happy path"
      props.onComplete(pasted);
    } else {
      props.onChange(pasted);
      refs[Math.min(pasted.length, 5)]?.focus();
    }
  }

  return (
    <div class="flex gap-2 justify-center" onPaste={handlePaste}>
      {[0, 1, 2, 3, 4, 5].map((i) => (
        <input
          ref={(el) => (refs[i] = el)}
          type="text"
          inputMode="numeric"
          maxLength={1}
          value={props.value[i] || ""}
          onInput={(e) => handleInput(i, e)}
          onKeyDown={(e) => handleKeyDown(i, e)}
          disabled={props.disabled}
          placeholder="_"
          class="otp-box"
          data-testid={`otp-${i}`}
          autocomplete="off"
        />
      ))}
    </div>
  );
}
```

### 2. OTP Box Styling

```css
.otp-box {
  width: 40px;
  height: 48px;
  text-align: center;
  font-size: 20px;
  font-family: var(--font-mono);
  letter-spacing: 0;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: transparent;
  color: var(--text-primary);
  outline: none;
  transition: border-color 200ms ease;
}
.otp-box:focus {
  border-color: var(--text-primary);
}
.otp-box::placeholder {
  color: rgba(255, 255, 255, 0.15);
}
.otp-box:disabled {
  opacity: 0.4;
}
```

### 3. Instructions (Numbered Steps)

```tsx
<ol class="text-[11px] text-text-secondary font-mono leading-relaxed mb-6 list-none space-y-1.5">
  <li>
    <span class="text-text-dim mr-1">1.</span> Open{" "}
    <button
      class="text-text-primary hover:underline border-0 bg-transparent p-0 font-mono text-[11px] cursor-pointer"
      onClick={() => window.open(`${settingsUrl}/account`, "_blank")}
    >
      Settings
    </button>
  </li>
  <li>
    <span class="text-text-dim mr-1">2.</span> Click{" "}
    <span class="text-text-primary">Connect Extension</span>
  </li>
  <li>
    <span class="text-text-dim mr-1">3.</span> Paste the code below
  </li>
</ol>
```

### 4. Success State

```tsx
// After successful pair, show checkmark for 800ms before transitioning
const [showSuccess, setShowSuccess] = createSignal(false);

async function handlePair() {
  // ... existing pair logic ...
  if (result.success) {
    setShowSuccess(true);
    setTimeout(() => {
      props.onAuthenticated();
    }, 800);
  }
}

// In the render:
<Show when={showSuccess()}>
  <div class="flex flex-col items-center gap-2 py-8">
    <svg width="32" height="32" viewBox="0 0 24 24" fill="none"
         stroke="var(--signal-green, #22c55e)" stroke-width="2.5">
      <path d="M20 6L9 17l-5-5" />
    </svg>
    <span class="text-sm font-mono text-signal-green">PAIRED</span>
  </div>
</Show>
```

### 5. Auto-Focus on Mount

```tsx
import { onMount } from "solid-js";

// Inside the component:
onMount(() => {
  // Focus first OTP box when popup opens
  refs[0]?.focus();
});
```

### 6. Session Expired Context

The `PairView` receives an optional `sessionExpired` prop. When true, a banner appears above the instructions:

```tsx
<Show when={props.sessionExpired}>
  <div class="text-[11px] text-text-dim font-mono mb-4 text-center border border-border-subtle px-3 py-2">
    Session expired — pair again to continue
  </div>
</Show>
```

Set from `App.tsx` when the auth check returns `authenticated: false` after previously being `true`.

### Files

- `testudo-extension/src/popup/components/PairView.tsx` — **rewritten** — OTP boxes, numbered steps, success state, auto-focus
- `testudo-extension/src/popup/popup.css` — **modified** — add `.otp-box` styles

### Dependencies Added

- None

---

## Acceptance Criteria

- [ ] Six individual input boxes render instead of single text field
- [ ] Each box accepts exactly one digit
- [ ] Typing a digit auto-advances cursor to the next box
- [ ] Backspace in empty box moves cursor to previous box
- [ ] Ctrl+V with 6-digit clipboard fills all boxes and auto-submits
- [ ] Manual typing does NOT auto-submit on 6th digit
- [ ] Enter key submits when all 6 digits present
- [ ] PAIR button disabled until exactly 6 digits entered
- [ ] First box auto-focused on popup open
- [ ] Placeholder is `_` (underscore) not `0`
- [ ] Instructions are numbered: (1) Open Settings, (2) Click Connect Extension, (3) Paste below
- [ ] Settings link opens `${backendUrl}/account` in new tab
- [ ] Loading state: spinner in button, inputs disabled, "PAIRING..."
- [ ] Error state: red text below inputs
- [ ] Success state: checkmark shown for ~800ms before transitioning to main view
- [ ] "Code expires in 5 minutes" hint visible below OTP boxes
- [ ] Session expired banner shows when returning after token expiry
- [ ] `cd testudo-extension && bun run build` passes (Chrome + Firefox)

---

## Risks

1. **Paste event handling** — `ClipboardEvent.clipboardData` is synchronous and well-supported in MV3 extensions. No async clipboard API needed. Risk: minimal.
2. **Auto-focus in popup context** — Browser extensions may delay focus in popups. Mitigation: Use `requestAnimationFrame` wrapper around `.focus()` if `onMount` fires before the DOM is painted.
3. **Auto-submit on paste could feel jarring** — Mitigated by the 800ms success checkmark animation providing visual closure before the view transitions.

---

## Completion Signal

This spec is complete when:
1. Six-box OTP input with auto-advance, backspace, paste, and auto-focus
2. Auto-submit on paste, manual submit on Enter/click
3. Numbered instructions with dynamic settings link
4. Success checkmark, error display, loading spinner, disabled state
5. `bun run build` passes for both Chrome and Firefox
6. Code committed to master
