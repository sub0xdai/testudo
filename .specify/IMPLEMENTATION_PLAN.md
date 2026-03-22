# Implementation Plan

> Last updated: 2026-03-22
> Current spec: JNL-14-markdown-hardening
> Phase: BUILD

---

## Active Spec: JNL-14-markdown-hardening

Harden markdown preview CSS (img/hr), scope scrollbar hiding, add attach button, allow textarea resize.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | CSS hardening — img/hr rules in .markdown-preview, scope scrollbar hiding to body only | complete | low | — |
| T2 | EntryEditor UX — attach image button in tab bar, textarea resize-y | complete | low | — |

### Key Decisions

- **All changes in single iteration**: Spec is small (2 files, ~20 lines of changes). Both tasks implemented together since they're independent and trivial.
- **Scrollbar scoping**: Moved `scrollbar-width: none` from `*` to `body` selector. Editor textarea and preview pane now show native scrollbars. The `body` rule already existed (for font/background) so added the scrollbar properties there.
- **Attach button as label+hidden input**: SolidJS `<label>` wrapping a hidden `<input type="file">` — clicking "Attach" text triggers the file picker. Reuses existing `uploadAndInsert()` function.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HL-11-status-transition-fix | 2026-03-21 |
| UXP-18-multi-theme | 2026-03-21 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
