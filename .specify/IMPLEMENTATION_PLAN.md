# Implementation Plan

> Last updated: 2026-04-01
> Current spec: UXA-03-extension-error-recovery
> Phase: COMPLETE

---

## Active Spec: UXA-03-extension-error-recovery

Extension error recovery: structured error codes, persistent banners for config errors, actionable messages.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | CP-1: Parse `error_code` from API error responses + add to `BackendResponseSchema` + `BackendResponse` type + `normalizeBackendAck` + fallback behavior (FR-1, FR-6) | complete | medium | — |
| T2 | CP-2: `classifyError()` + persistent `showBanner()` in Shadow DOM + banner CSS + update trade result handler for agent wallet errors (FR-2, FR-3, FR-7, FR-8) | complete | medium | T1 |
| T3 | CP-3: Toast refinements for rate_limited and insufficient_margin error codes (FR-4, FR-5) | complete | low | T1 |
| T4 | Validate: `bun run build` in testudo-extension, commit | complete | low | T1, T2, T3 |

### Key Decisions

- `classifyError()` lives in content.ts (not a separate file) — only 25 lines, no reuse outside content script
- Banner uses same Shadow DOM pattern as toasts with `TOAST_STYLES` reuse — consistent theming
- Single active banner at a time (new banner removes old) — prevents banner stack-up
- `DESK_URL` imported from utils.ts (already available, no new dependency) with fallback to production URL
- `error_code` propagated through both API error paths: HTTP error (apiRequest) and logical error (normalizeBackendAck)

### Discoveries

- `DESK_URL` already includes `/desk` suffix — spec template suggested `{DESK_URL}/desk/#/account` but correct path is `${DESK_URL}/#/account`
- `error_code` must flow through 4 layers: ErrorResponseSchema → ApiResult → normalizeBackendAck → BackendResponse for complete coverage
- content.js bundle size increased 2.9kb (49.3→52.2kb) — all from classifyError, banner CSS, and DESK_URL constant

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
- REL-02-hl-journal-pipeline (COMPLETE)
- REL-03-hl-group-reconciliation (COMPLETE)
- CON-01a-daily-stats-regression (COMPLETE)
- UXA-01-agent-wallet-visibility (COMPLETE)
- UXA-02-desk-reauth-ux (COMPLETE)
