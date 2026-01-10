# E.1 API Key Storage Design

**Epic**: E (Live Execution)
**Story**: E.1 - Securely connect Binance API keys
**Status**: Design approved, ready for implementation

## Acceptance Criteria (from hybrid_trading.json)

- `key_storage`: API keys encrypted at rest, never logged
- `key_validation`: Test connection validates read permissions before saving
- `key_permissions`: Minimum required: Spot trading, read balances

## Design Decision

**Inline validation** (Option A) - Single API call flow:
```
POST /api/v1/exchanges/accounts
  → Validate input
  → Test connection to Binance
  → Check permissions
  → Encrypt credentials
  → Save to DB
  → Return sanitized response
```

## Existing Infrastructure (Reuse)

| Component | Location | Status |
|-----------|----------|--------|
| `AesGcmVault` | `common_utils/src/crypto/vault.rs` | Complete |
| `PostgresExchangeAccountRepository` | `sqlx_postgres/src/repositories/api_keys.rs` | Complete |
| `CCXTHTTPClient` | `common_utils/src/adapters/ccxt_adapter.rs` | Complete |
| `BinanceAuth` | `common_utils/src/adapters/ccxt_auth.rs` | Complete |

## New Components

### 1. CredentialValidator

**File**: `common_utils/src/adapters/credential_validator.rs`

```rust
pub struct CredentialValidator {
    http_client: reqwest::Client,
}

impl CredentialValidator {
    /// Validates credentials by calling Binance GET /api/v3/account
    /// Returns Ok(permissions) on success, Err(reason) on failure
    pub async fn validate_binance(
        &self,
        api_key: &str,
        api_secret: &str,
    ) -> Result<ValidatedPermissions, CredentialValidationError>;
}

pub struct ValidatedPermissions {
    pub can_trade_spot: bool,
    pub can_read_balances: bool,
    pub account_type: String,
}

pub enum CredentialValidationError {
    InvalidCredentials,
    InsufficientPermissions { missing: Vec<String> },
    ExchangeUnreachable(String),
    RateLimited { retry_after_ms: u64 },
}
```

**Binance validation strategy:**
- Call `GET /api/v3/account` (requires HMAC signature)
- Response contains `canTrade`, `canWithdraw`, `canDeposit`, `balances`
- Single call validates both authentication and permissions

### 2. Error Mapping

| CredentialValidationError | HTTP Status | Error Code | User Message |
|---------------------------|-------------|------------|--------------|
| InvalidCredentials | 401 | `invalid_credentials` | "API key or secret is invalid" |
| InsufficientPermissions | 403 | `insufficient_permissions` | "Missing required permissions: {list}" |
| ExchangeUnreachable | 502 | `exchange_unreachable` | "Could not reach Binance, please try again" |
| RateLimited | 429 | `rate_limited` | "Exchange rate limit hit, retry in {n}s" |

### 3. Route Updates

**File**: `router/src/routes/exchanges.rs`

Update `add_exchange_account` handler:

```rust
pub async fn add_exchange_account(
    app_state: web::Data<AppState>,
    body: web::Json<AddExchangeAccountRequest>,
) -> Result<HttpResponse, ApiError> {
    // 1. Validate input
    let request = body.into_inner();
    request.validate()?;

    // 2. Test connection and check permissions
    let permissions = app_state.credential_validator
        .validate_binance(&request.api_key, &request.api_secret)
        .await?;

    // 3. Verify minimum permissions
    if !permissions.can_trade_spot || !permissions.can_read_balances {
        return Err(ApiError::InsufficientPermissions {
            missing: vec!["spot_trading", "read_balances"],
        });
    }

    // 4. Create account (repository handles encryption)
    let account = app_state.exchange_repo
        .create(CreateExchangeAccountRequest {
            user_id: request.user_id,
            exchange_name: "binance".to_string(),
            api_key: request.api_key,
            api_secret: request.api_secret,
            permissions: serde_json::to_value(&permissions)?,
        })
        .await?;

    // 5. Return sanitized response (no credentials)
    Ok(HttpResponse::Created().json(ExchangeAccountResponse::from(account)))
}
```

### 4. AppState Updates

**File**: `router/src/types/app.rs`

Add to AppState:
```rust
pub struct AppState {
    // ... existing fields
    pub credential_validator: Arc<CredentialValidator>,
    pub encryption_service: Arc<dyn EncryptionService>,
    pub exchange_repo: Arc<dyn ExchangeAccountRepository>,
}
```

## Security Considerations

- API keys never logged (enforced in vault.rs via `Zeroizing<String>`)
- Credentials only in memory during validation, then encrypted
- Failed validation attempts don't persist anything
- Successful response excludes credentials
- HTTPS only for Binance API calls

## Response Format

**Success (201 Created):**
```json
{
  "id": "uuid",
  "exchange_name": "binance",
  "is_active": true,
  "permissions": {
    "can_trade_spot": true,
    "can_read_balances": true,
    "account_type": "SPOT"
  },
  "created_at": "2026-01-10T12:00:00Z"
}
```

## Testing Strategy

### Unit Tests (no network)
- `CredentialValidator` with mocked HTTP responses
- Verify correct error types for each failure scenario
- Verify permissions parsing from Binance response

### Integration Tests (real encryption)
- `AesGcmVault` + `PostgresExchangeAccountRepository` round-trip
- Verify credentials encrypted at rest, decrypted correctly

### E2E Tests (feature-flagged)
- Real Binance testnet credentials
- Full flow: validate → encrypt → save → retrieve → decrypt
- Marked `#[ignore]` for CI

## TDD Implementation Order

1. Write failing test for `CredentialValidator::validate_binance`
2. Implement validation logic
3. Write failing test for route integration
4. Wire components in route
5. Green

## Files to Create/Modify

| File | Action |
|------|--------|
| `common_utils/src/adapters/credential_validator.rs` | **Create** |
| `common_utils/src/adapters/mod.rs` | **Modify** - add module |
| `router/src/routes/exchanges.rs` | **Modify** - wire validation |
| `router/src/types/app.rs` | **Modify** - add to AppState |
| `router/src/main.rs` | **Modify** - initialize components |
