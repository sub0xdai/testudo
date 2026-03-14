# EXT-15: Exchange Account Management

| Field    | Value                                    |
|----------|------------------------------------------|
| Status   | In Progress                              |
| Date     | 2026-02-24                               |
| Depends  | 012-ccxt-multi-exchange                  |
| Phase    | Extension — Account Management           |

## 1. Overview

### Current State
- Authentication supports login only (no registration from extension)
- No UI for managing exchange API credentials
- Database `check_exchange_name_supported` constraint blocks exchanges not in hardcoded list (e.g., WOO X)
- Backend exchange CRUD endpoints fully implemented in spec 012

### Target State
- Registration flow in AuthSection with confirm password
- ExchangeManager component in SettingsView for exchange credential CRUD
- CCXT sidecar validates credentials dynamically (no hardcoded whitelist)
- Users can add WOO X (or any 100+ CCXT exchanges), test connections, and trade live

## 2. User Stories

1. As a new user, I want to register an account from the extension so I can start trading
2. As a trader, I want to add my WOO X API credentials so I can trade on that exchange
3. As a trader, I want to test my exchange connection so I know my credentials work
4. As a trader, I want to remove an exchange account so I can manage my connections
5. As an unauthenticated user, I should not see exchange management (auth-gated)

## 3. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Drop check_exchange_name_supported | Yes | CCXT validates dynamically; hardcoded list creates friction |
| Register toggle on AuthSection | Reuse existing form | Avoids new navigation; simple toggle |
| ExchangeManager in SettingsView | Separation of concerns | Settings is the natural home for configuration |
| Never store credentials in extension | Signals only during form | Security: credentials go straight to backend AES-256-GCM |
| Native select styling | CSS only | Simpler than custom dropdown component |
| One account per exchange | Retain constraint | Simpler routing; matches backend unique constraint |

## 4. Functional Requirements

### FR-1: Database Migration
- Drop `check_exchange_name_supported` constraint from `exchange_accounts` table
- Retain `check_exchange_name_not_empty` and `check_exchange_name_lowercase` constraints

### FR-2: Registration
- Add `isRegister` toggle to AuthSection (Login / Register modes)
- Show confirm password field in register mode
- REGISTER message handler in background.ts → POST /api/v1/auth/register
- Auto-login on successful registration (store tokens, schedule refresh)

### FR-3: TypeScript Interfaces
- `ExchangeInfo`: id, name, type, description, supported_features, required_credentials, optional_credentials
- `ExchangeAccount`: id, exchange_name, account_name, is_active, permissions, created_at, last_used_at
- `AddExchangeAccountPayload`: exchange_name, account_name?, api_key, secret, passphrase?
- `TestConnectionResult`: account_id, exchange_name, status, message, tested_at, latency_ms

### FR-4: Background Message Handlers
- `LIST_EXCHANGES` → GET /api/v1/exchanges (with auth retry)
- `LIST_EXCHANGE_ACCOUNTS` → GET /api/v1/exchanges/accounts (with auth retry)
- `ADD_EXCHANGE_ACCOUNT` → POST /api/v1/exchanges/accounts (with auth retry)
- `DELETE_EXCHANGE_ACCOUNT` → DELETE /api/v1/exchanges/accounts/{id} (with auth retry)
- `TEST_EXCHANGE_CONNECTION` → POST /api/v1/exchanges/accounts/{id}/test (with auth retry)

### FR-5: ExchangeManager Component
- Fetch exchanges and accounts in parallel on mount
- Show connected accounts as cards with status dot (green=active)
- Collapsible add form with exchange select, API key, secret, optional passphrase
- Test connection button showing latency
- Delete with inline confirmation
- Filter exchange dropdown to exclude already-connected exchanges

### FR-6: SettingsView Integration
- Add ExchangeManager inside authenticated section
- Wrap body in scroll-area class for overflow

### FR-7: Select Element CSS
- Style native select to match design system (bg-elevated, border-subtle, rounded)

## 5. File Context

### New Files
| File | Purpose |
|------|---------|
| testudo-exchange/crates/sqlx_postgres/migrations/20260224000000_drop_exchange_name_whitelist.up.sql | Drop constraint |
| testudo-exchange/crates/sqlx_postgres/migrations/20260224000000_drop_exchange_name_whitelist.down.sql | Restore constraint |
| testudo-extension/src/popup/components/ExchangeManager.tsx | Exchange CRUD UI |

### Modified Files
| File | Changes |
|------|---------|
| testudo-extension/src/popup/components/AuthSection.tsx | Add register mode toggle |
| testudo-extension/src/background.ts | Add REGISTER + 5 exchange handlers |
| testudo-extension/src/types.ts | Add exchange account interfaces |
| testudo-extension/src/popup/components/SettingsView.tsx | Integrate ExchangeManager |
| testudo-extension/src/popup/popup.css | Add select element styles |

### Unchanged (Backend Complete)
| File | Status |
|------|--------|
| testudo-exchange/crates/router/src/routes/exchanges.rs | No changes needed |
| testudo-exchange/crates/router/src/repositories/exchange_account.rs | No changes needed |

## 6. Acceptance Criteria

1. Migration runs successfully: constraint dropped
2. Register from extension → auto-login → see main view
3. Invalid registration (short password) shows error
4. List exchanges shows available exchanges
5. Add WOO X credentials → validates via CCXT sidecar → shows in account list
6. Test connection shows latency in ms
7. Delete exchange account with confirmation
8. Unauthenticated users don't see ExchangeManager
9. Credentials never stored in browser.storage (grep verification)
10. Extension builds without TypeScript errors

## 7. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Credential exposure in extension memory | High | Signals cleared on submit/cancel, type=password, no storage persistence |
| CCXT sidecar unavailable | Medium | 502 error handling, graceful fallback message |
| Token expiry during validation | Low | 401 retry pattern on all handlers |

## 8. Completion Signal

```
feat: EXT-15 exchange account management — registration, CRUD UI, and migration
```
