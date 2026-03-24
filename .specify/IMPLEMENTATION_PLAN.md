# Implementation Plan

> Last updated: 2026-03-24
> Current spec: EXT-39-pair-ux
> Phase: BUILD

---

## Active Spec: EXT-39-pair-ux

Extension pairing UX — six-box OTP input, auto-paste, numbered instructions, success/error/loading states, auto-focus, session expired banner.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Rewrite PairView.tsx — six-box OTP input with auto-advance/backspace/paste, numbered instructions, success checkmark, error display, loading spinner, auto-focus, code expiry hint. Add OTP CSS to popup.css. Update App.tsx for session expired detection. | complete | medium | AUTH-03 |
| T2 | Build validation — bun run build for Chrome + Firefox, verify all 18 acceptance criteria | pending | low | T1 |

### Key Decisions

- **Single task for all PairView changes**: OTP component, instructions, states, CSS, and App.tsx session detection are tightly coupled — implementing separately would create non-building intermediates.
- **WEB_APP_URL/account for settings link**: The spec says `backendUrl` but the account settings page lives on the web frontend, not the API server. Using existing `WEB_APP_URL` constant.
- **Session expired detection via stored popupView**: If `browser.storage.local` has `popupView: "main"` but auth check returns false, the session expired. Explicit logout sets `popupView: "auth"` before next open.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| AUTH-03-frontend-auth | 2026-03-24 |
| AUTH-02-backend-auth | 2026-03-24 |
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
