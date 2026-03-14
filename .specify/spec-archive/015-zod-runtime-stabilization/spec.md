# Specification: Zod Runtime Stabilization for Trade Reliability

## Overview
Stabilize post-Zod trading behavior by preserving strict validation for critical inputs while restoring tolerant handling of backend response variants. The objective is to eliminate trade placement/cancel/list regressions, prevent orphaned orders, and standardize API contracts for predictable extension behavior.

Current state:
- Runtime regressions appeared after strict response parsing was introduced.
- Trade lifecycle reliability has been impacted by non-uniform response envelopes and close-path edge cases.
- UI flow regressions (confirmation behavior, unreadable error surfaces) increased operational risk.

Target state:
- Trading path is reliable in live usage: limit+OCO entry, consistent position listing, deterministic cancel/close cleanup.
- Backend trade endpoints always return a canonical envelope.
- Extension runtime validation remains strict for outbound input, tolerant-normalized for inbound responses.

---
## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Introduce a stabilization branch and baseline reproducible smoke checklist for create->list->cancel trade flow | High | Process |
| FR-2 | Enforce canonical response envelope `{ success, data?, error? }` for all trade endpoints in backend | High | Backend API |
| FR-3 | Add backend contract tests for envelope consistency across success/error status codes | High | Backend Test |
| FR-4 | Keep strict Zod validation for critical outbound inputs (trade payloads, settings/env/forms) | High | Extension Validation |
| FR-5 | Implement tolerant response normalizers for inbound extension trade responses before schema validation | High | Extension Runtime |
| FR-6 | Ensure trade close path cancels all related pending orders for the affected group id/symbol, idempotently | High | Order Lifecycle |
| FR-7 | Ensure entry remains `Limit` with SL/TP OCO behavior; prevent market-entry regression | High | Execution Logic |
| FR-8 | Keep explicit modal confirm UX (click path + key path) and readable, non-translucent error surfaces | Medium | UX Safety |
| FR-9 | Add integration/regression tests for response variants and full trade lifecycle transitions | High | Verification |
| FR-10 | Add release gate checklist requiring successful backend + extension + web verification commands | High | CI/Release |

---
## Technical Implementation

### 1) Stabilization Baseline (FR-1)
- Create working branch in affected repos:
  - `testudo-exchange`
  - `testudo-extension`
- Add a manual smoke checklist document for:
  1. Place live limit trade
  2. Confirm exchange order acceptance
  3. List positions in extension
  4. Cancel trade and verify no orphaned pending orders

### 2) Canonical Trade API Envelope (FR-2, FR-3)
Standardize trade route responses in:
- `testudo-exchange/crates/router/src/routes/trade_management.rs`

Contract:
```json
{ "success": true, "data": <payload>, "error": null }
{ "success": false, "data": null, "error": "message" }
```

Requirements:
- No bare arrays/strings from trade endpoints.
- Error responses must include `success: false` and an `error` message.
- Add route tests proving contract for create/list/get/cancel and expected non-2xx cases.

### 3) Extension Parsing Policy (FR-4, FR-5)
Update extension runtime parsing in:
- `testudo-extension/src/background.ts`
- `testudo-extension/src/schemas.ts`

Policy:
- Strict Zod for outbound and local critical inputs:
  - trade payload construction
  - stored settings
  - auth token structures
  - env/form validation in web
- Tolerant-normalized handling for inbound backend responses:
  - normalize legacy variant shapes first
  - validate normalized shape with Zod
  - preserve deterministic user-facing errors on true malformed responses

Introduce normalizers (or equivalent helpers):
- `normalizeTradeListResponse(raw)`
- `normalizeBackendAck(raw)`

### 4) Close Path Orphan Cleanup (FR-6, FR-7)
Ensure fill/close events trigger cancellation of all related pending orders for the same group context.

Files:
- `testudo-exchange/crates/router/src/services/fill_detector.rs`
- `testudo-exchange/crates/router/src/routes/trade_management.rs`

Requirements:
- Idempotent cancellation (`OrderNotFound` treated as no-op).
- No pending SL/TP orphan remains after trade closure.
- Entry remains `ApiOrderType::Limit`, SL remains stop-loss type, TP remains limit type.
- No accidental market-entry path in trade creation.

### 5) UX Safeguards (FR-8)
Files:
- `testudo-extension/src/components/TradeForm.tsx`
- `testudo-extension/src/modal.tsx`

Requirements:
- Confirmation modal supports explicit clickable confirmation flow and keyboard flow.
- Error toasts remain high-contrast and readable.
- Failure messages map to actionable causes where possible.

### 6) Regression Coverage + Release Gate (FR-9, FR-10)
Add regression tests for:
- wrapped and legacy trade-list response shapes
- create->list->cancel lifecycle
- SL fill cancels TP, TP fill cancels SL
- failed live entry rolls back shadow state

Release gate commands (must pass before completion):
```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Extension
cd testudo-extension && bun run typecheck && bun run test && bun run build

# Frontend web
cd testudo-web && bun run lint && bun run build
```

---
## Non-Goals
- New exchange integrations
- Strategy/risk model redesign
- UI redesign beyond confirmation/error-readability safeguards

---
## Acceptance Criteria
- Live trade can be created via limit+OCO and appears in extension positions.
- Cancel/close removes trade and does not leave related pending orders behind.
- Extension no longer throws malformed-response errors for valid backend variants.
- Trade endpoints consistently emit canonical envelope.
- All verification commands pass.

---
## Completion Signal
When ALL above criteria are satisfied and verification commands pass, output:
`<promise>DONE</promise>`
