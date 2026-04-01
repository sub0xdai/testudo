# Implementation Plan

> Last updated: 2026-04-01
> Current spec: UXA-01-agent-wallet-visibility
> Phase: COMPLETE

---

## Active Spec: UXA-01-agent-wallet-visibility

Surface inactive agent wallets in account listings and provide structured error codes for all exchange API errors.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | CP-1: Modify `list_by_user()` query to include inactive agent wallets + add `requires_reauthorization` field to `ExchangeAccountResponse` (FR-1, FR-2) | complete | low | — |
| T2 | CP-2: Add `AgentWalletInactive` variant to `ExchangeApiError` + modify HL `load_auth()` + update `format_exchange_error()` + `is_definitive_rejection()` (FR-3, FR-4, FR-5, FR-9) | complete | medium | — |
| T3 | CP-3: Expand `format_exchange_error()` for HL patterns + add `error_code` field to `ApiResponse` + `error_code_for()` helper (FR-6, FR-7, FR-8) | complete | medium | T2 |
| T4 | Validate: cargo clippy --all-targets && cargo test, commit | complete | low | T1, T2, T3 |

### Key Decisions

- `requires_reauthorization` uses `Option<bool>` with `skip_serializing_if = "Option::is_none"` — only present when true, maintaining backward compatibility
- `error_code` field also uses `Option<String>` with `skip_serializing_if` — zero impact on existing clients
- CexExchangeApi's `load_credentials` updated to filter active-only on fallback path and detect inactive agent wallets
- `error_code_for()` is a standalone function parallel to `format_exchange_error()` — both match on `ExchangeApiError` variants
- Error codes only emitted on the definitive rejection path (line 1028) — ambiguous errors don't send error_code since the response is a warning string

### Discoveries

- `ExchangeAccountResponse` is constructed in 3 places: `get_user_exchange_accounts()`, `add_exchange_account()`, and test in `types/exchanges.rs`
- Pre-existing clippy warnings unchanged: useless_conversion (cex_client.rs), unused_variables (actor.rs), manual_contains (evaluator.rs)
- `CexExchangeApi::load_credentials` also needed updating since it calls `list_by_user()` — now filters to active accounts on fallback and detects inactive agent wallets

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
- REL-02-hl-journal-pipeline (COMPLETE)
- REL-03-hl-group-reconciliation (COMPLETE)
- CON-01a-daily-stats-regression (COMPLETE)
