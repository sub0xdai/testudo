# Quality Checklist - EXT-15 Exchange Account Management

**Spec:** EXT-15-exchange-account-management
**Date:** 2026-02-24

## FR-1: Database Migration

- [ ] `check_exchange_name_supported` constraint dropped from `exchange_accounts`
- [ ] `check_exchange_name_not_empty` constraint still present
- [ ] `check_exchange_name_lowercase` constraint still present
- [ ] Up migration runs without error on existing data
- [ ] Down migration restores constraint correctly
- [ ] Existing exchange accounts unaffected by migration

## FR-2: Registration

- [ ] `isRegister` toggle switches between Login and Register modes
- [ ] Confirm password field visible only in register mode
- [ ] Password mismatch shows validation error before submit
- [ ] Short password (< minimum) shows error from backend
- [ ] REGISTER handler in background.ts sends POST /api/v1/auth/register
- [ ] Successful registration stores tokens and schedules refresh
- [ ] Auto-login after registration navigates to main view
- [ ] Registration error displays message to user
- [ ] Toggle resets form fields when switching modes

## FR-3: TypeScript Interfaces

- [ ] `ExchangeInfo` interface matches backend GET /exchanges response shape
- [ ] `ExchangeAccount` interface matches backend GET /exchanges/accounts response shape
- [ ] `AddExchangeAccountPayload` includes optional `account_name` and `passphrase`
- [ ] `TestConnectionResult` includes `latency_ms` field
- [ ] All interfaces exported from types.ts
- [ ] No `any` types used in interface definitions

## FR-4: Background Message Handlers

- [ ] `LIST_EXCHANGES` handler calls GET /api/v1/exchanges with auth header
- [ ] `LIST_EXCHANGE_ACCOUNTS` handler calls GET /api/v1/exchanges/accounts with auth header
- [ ] `ADD_EXCHANGE_ACCOUNT` handler calls POST /api/v1/exchanges/accounts with payload
- [ ] `DELETE_EXCHANGE_ACCOUNT` handler calls DELETE /api/v1/exchanges/accounts/{id}
- [ ] `TEST_EXCHANGE_CONNECTION` handler calls POST /api/v1/exchanges/accounts/{id}/test
- [ ] All five handlers implement 401 retry with token refresh
- [ ] Error responses propagated to caller with meaningful messages
- [ ] Network failures handled gracefully (sidecar down, timeout)

## FR-5: ExchangeManager Component

- [ ] Exchanges and accounts fetched in parallel on mount
- [ ] Connected accounts rendered as cards with exchange name
- [ ] Active status dot (green) shown on account cards
- [ ] Add form is collapsible (hidden by default)
- [ ] Exchange select dropdown populated from LIST_EXCHANGES
- [ ] Already-connected exchanges filtered out of dropdown
- [ ] API key input is type="password"
- [ ] Secret input is type="password"
- [ ] Passphrase input shown only when exchange requires it (or always optional)
- [ ] Submit clears credential signals immediately after request
- [ ] Cancel clears credential signals
- [ ] Test connection button sends TEST_EXCHANGE_CONNECTION
- [ ] Test connection result displays latency in ms
- [ ] Delete button shows inline confirmation before executing
- [ ] Delete removes account from list on success
- [ ] Loading states shown during async operations
- [ ] Error states displayed inline (not alert dialogs)

## FR-6: SettingsView Integration

- [ ] ExchangeManager rendered inside authenticated section of SettingsView
- [ ] ExchangeManager hidden when user is not authenticated
- [ ] Settings body has scroll-area class for overflow handling
- [ ] ExchangeManager does not break existing SettingsView layout

## FR-7: Select Element CSS

- [ ] Native select styled with `bg-elevated` background
- [ ] Native select styled with `border-subtle` border
- [ ] Native select has rounded corners matching design system
- [ ] Select focus state matches input focus state
- [ ] Select appearance consistent across Chrome and Firefox

## Safety

- [ ] Credentials never written to browser.storage (grep verification)
- [ ] Credential signals cleared on form submit
- [ ] Credential signals cleared on form cancel/close
- [ ] No credential values in console.log statements
- [ ] All credential inputs use type="password"
- [ ] Credentials sent only over HTTPS to backend

## Build and Type Safety

- [ ] `esbuild` builds extension without errors
- [ ] No TypeScript errors in new or modified files
- [ ] No new `any` types introduced
- [ ] Existing extension tests still pass
