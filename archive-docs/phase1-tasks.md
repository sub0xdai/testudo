# Phase 1 Atomic Task Breakdown: Foundation Refactoring

## Overview
This document breaks down Phase 1 of the Testudo Exchange refactoring into atomic, test-driven tasks. Each task follows TDD (Test-Driven Development), DRY (Don't Repeat Yourself), SOLID principles, and KISS (Keep It Simple, Stupid).

## Guiding Principles

### TDD Cycle for Each Task
1. **RED**: Write failing test defining the expected behavior
2. **GREEN**: Write minimal code to pass the test
3. **REFACTOR**: Improve code while keeping tests green

### SOLID Principles Application
- **S**ingle Responsibility: Each module has one clear purpose
- **O**pen/Closed: Traits allow extension without modification
- **L**iskov Substitution: Mock implementations satisfy same contracts
- **I**nterface Segregation: Small, focused interfaces
- **D**ependency Inversion: Depend on abstractions (traits) not concretions

---

## Task Group 1: Authentication Foundation (Week 1)

### 1.1 Create User Domain Model & Tests
- **File**: `testudo-exchange/crates/common_utils/src/models/user.rs` (NEW)
- **Tests**: `testudo-exchange/crates/common_utils/src/models/user_test.rs`
- **TDD Steps**:
  - Write failing tests for User struct with email, password_hash, created_at
  - Implement User struct with validation
  - Add serialization/deserialization tests
- **SOLID**: Single Responsibility - User model only handles user data
- **Acceptance Criteria**:
  - [x] User struct with email validation
  - [x] Password hash field (never stores plaintext)
  - [x] Timestamps for audit trail
  - [x] Tests pass with 100% coverage

### 1.2 Create Authentication Service Trait
- **File**: `testudo-exchange/crates/common_utils/src/auth/mod.rs` (NEW)
- **Tests**: Mock implementation tests first
- **TDD Steps**:
  - Define trait methods (register, login, verify_token)
  - Create mock implementation for testing
  - Test error cases (invalid credentials, expired tokens)
- **SOLID**: Dependency Inversion - Router depends on trait, not implementation
- **Acceptance Criteria**:
  - [x] AuthService trait defined
  - [x] Methods: register, login, verify_token, refresh_token
  - [x] Error types for auth failures
  - [x] Mock implementation for testing

### 1.3 Database Migration for Users Table
- **File**: `testudo-exchange/crates/sqlx_postgres/migrations/[timestamp]_users.up.sql`
- **Down Migration**: `[timestamp]_users.down.sql`
- **Schema**:
  ```sql
  CREATE TABLE users (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      email VARCHAR(255) NOT NULL UNIQUE,
      password_hash VARCHAR(255) NOT NULL,
      created_at TIMESTAMP DEFAULT NOW(),
      updated_at TIMESTAMP DEFAULT NOW(),
      is_active BOOLEAN DEFAULT true,
      email_verified BOOLEAN DEFAULT false
  );
  CREATE INDEX idx_users_email ON users(email);
  ```
- **Tests**: Migration rollback/forward tests
- **Acceptance Criteria**:
  - [x] Migration runs successfully
  - [x] Rollback works cleanly
  - [x] Indexes for performance
  - [x] Constraints enforced

### 1.4 Password Hashing Module
- **File**: `testudo-exchange/crates/common_utils/src/auth/password.rs`
- **Tests**: Hash verification, salt generation, timing attack resistance
- **TDD Steps**:
  - Test bcrypt/argon2 integration
  - Test password strength validation
  - Test constant-time comparison
- **KISS**: Simple wrapper around proven crypto library
- **Acceptance Criteria**:
  - [x] Secure hashing (bcrypt or argon2)
  - [x] Password strength validation
  - [x] Salt automatically generated
  - [x] Timing attack resistant

---

## Task Group 2: API Key Management (Week 2)

### 2.1 Exchange Account Model
- **File**: `testudo-exchange/crates/common_utils/src/models/exchange_account.rs`
- **Tests**: Serialization, validation, encryption tests
- **TDD Steps**:
  - Model with encrypted credentials fields
  - Validation for exchange types
  - Test data masking for logs
- **Acceptance Criteria**:
  - [x] ExchangeAccount struct defined
  - [x] Fields: user_id, exchange_name, encrypted_api_key, encrypted_secret
  - [x] Never exposes raw credentials
  - [x] Serialization masks sensitive data

### 2.2 Encryption Service
- **File**: `testudo-exchange/crates/common_utils/src/crypto/vault.rs`
- **Tests**: Encrypt/decrypt round-trip, key rotation tests
- **TDD Steps**:
  - Test AES-256-GCM encryption
  - Test key derivation
  - Test secure random IV generation
- **SOLID**: Interface Segregation - Separate encryption from storage
- **Acceptance Criteria**:
  - [x] AES-256-GCM encryption
  - [x] Secure key management
  - [x] IV/nonce handling
  - [x] Error handling for corruption

### 2.3 Database Migration for Exchange Accounts
- **File**: `testudo-exchange/crates/sqlx_postgres/migrations/[timestamp]_exchange_accounts.up.sql`
- **Schema**:
  ```sql
  CREATE TABLE exchange_accounts (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
      exchange_name VARCHAR(50) NOT NULL,
      api_key_encrypted BYTEA NOT NULL,
      api_secret_encrypted BYTEA NOT NULL,
      permissions JSONB DEFAULT '{}',
      is_active BOOLEAN DEFAULT true,
      created_at TIMESTAMP DEFAULT NOW(),
      last_used_at TIMESTAMP,
      UNIQUE(user_id, exchange_name)
  );
  ```
- **Acceptance Criteria**:
  - [x] Foreign key to users table
  - [x] Encrypted storage for credentials
  - [x] Unique constraint per user/exchange
  - [x] Audit fields

### 2.4 API Key Repository
- **File**: `testudo-exchange/crates/sqlx_postgres/src/repositories/api_keys.rs`
- **Tests**: CRUD operations with mocked database
- **TDD Steps**:
  - Test create, read, update, delete
  - Test encryption on save, decryption on load
  - Test query by user and exchange
- **DRY**: Reuse database connection patterns
- **Acceptance Criteria**:
  - [x] CRUD operations work
  - [x] Automatic encryption/decryption
  - [x] Proper error handling
  - [x] Transaction support

---

## Task Group 3: CEX Exchange Adapter Interface - CCXT Integration (Week 3)

### 3.1 StandardOrder Type Definition
- **File**: `testudo-exchange/crates/common_utils/src/types/order.rs`
- **Tests**: Validation, conversion, serialization tests
- **Structure**:
  ```rust
  pub struct StandardOrder {
      pub id: Uuid,
      pub user_id: Uuid,
      pub symbol: String,
      pub side: OrderSide, // Buy/Sell or Long/Short
      pub order_type: OrderType, // Market/Limit/Stop
      pub quantity: Decimal,
      pub price: Option<Decimal>,
      pub stop_price: Option<Decimal>,
      pub time_in_force: TimeInForce,
      pub exchange: Option<String>,
  }
  ```
- **KISS**: Simple, well-documented struct
- **Acceptance Criteria**:
  - [x] All order types supported
  - [x] Validation logic
  - [x] Conversion to/from exchange formats
  - [x] Clear documentation

### 3.2 ExchangeAdapter Trait for CEX
- **File**: `testudo-exchange/crates/router/src/exchange/mod.rs`
- **Tests**: Mock adapter implementation
- **Interface**:
  ```rust
  #[async_trait]
  pub trait ExchangeAdapter: Send + Sync {
      async fn place_order(&self, order: &StandardOrder) -> Result<OrderResponse, RoutingError>;
      async fn cancel_order(&self, order_id: &str) -> Result<(), RoutingError>;
      async fn get_order_status(&self, order_id: &str) -> Result<OrderResponse, RoutingError>;
      fn get_name(&self) -> &str;
      async fn health_check(&self) -> Result<(), RoutingError>;
  }
  ```
- **SOLID**: Open/Closed - New CEX exchanges extend via CCXT
- **Acceptance Criteria**:
  - [x] Trait methods defined
  - [x] Error handling types
  - [x] Mock implementation
  - [x] Async/await support

### 3.3 Exchange Router Module
- **File**: `testudo-exchange/crates/router/src/exchange/mod.rs`
- **Tests**: Routing logic tests with mock adapters
- **TDD Steps**:
  - Test routing decisions
  - Test fallback behavior
  - Test load balancing
- **Acceptance Criteria**:
  - [x] Routes orders to correct exchange
  - [x] Handles exchange failures
  - [x] Load balancing logic
  - [x] Metrics collection

### 3.4 Error Handling Module
- **File**: `testudo-exchange/crates/common_utils/src/errors/exchange.rs`
- **Tests**: Error conversion, propagation, serialization
- **Error Types**:
  ```rust
  pub enum ExchangeError {
      ConnectionError(String),
      AuthenticationError(String),
      InsufficientBalance(Decimal),
      OrderRejected(String),
      RateLimited(Duration),
      ExchangeUnavailable(String),
  }
  ```
- **DRY**: Centralized error types
- **Acceptance Criteria**:
  - [x] Comprehensive error types
  - [x] Error conversion traits
  - [x] User-friendly messages
  - [x] Proper HTTP status codes

### 3.5 CCXT Adapter Implementation
- **File**: `testudo-exchange/crates/common_utils/src/adapters/ccxt_adapter.rs`
- **Tests**: Integration tests with mock CCXT responses
- **Implementation**:
  ```rust
  pub struct CCXTAdapter {
      exchange_type: String,  // "binance", "coinbase", "kraken"
      api_credentials: ExchangeAccountWithCredentials,
      rate_limiter: RateLimiter,
  }
  ```
- **TDD Steps**:
  - Test order placement translation
  - Test balance fetching
  - Test error handling for API failures
  - Test rate limiting
- **Supported Exchanges (Phase 1)**:
  - Binance (spot and futures)
  - Coinbase Pro
  - Kraken
- **Acceptance Criteria**:
  - [x] CCXT library integration
  - [x] Order type translation
  - [x] Balance synchronization
  - [x] Rate limiting per exchange
  - [x] Error recovery strategies

---

## Task Group 4: Router Refactoring (Week 4)

### 4.1 JWT Middleware
- **File**: `testudo-exchange/crates/router/src/middleware/auth.rs`
- **Tests**: Token validation, expiry, refresh tests
- **TDD Steps**:
  - Test valid token acceptance
  - Test expired token rejection
  - Test malformed token handling
- **Implementation**:
  ```rust
  pub async fn jwt_middleware(
      req: ServiceRequest,
      next: Next<BoxBody>
  ) -> Result<ServiceResponse<BoxBody>, Error>
  ```
- **Acceptance Criteria**:
  - [ ] JWT validation
  - [ ] Token refresh logic
  - [ ] Rate limiting
  - [ ] Security headers

### 4.2 Refactor User Route
- **File**: `testudo-exchange/crates/router/src/routes/user.rs`
- **From**: UUID generation
- **To**: Registration/Login endpoints
- **New Endpoints**:
  - `POST /api/v1/auth/register`
  - `POST /api/v1/auth/login`
  - `POST /api/v1/auth/refresh`
  - `POST /api/v1/auth/logout`
- **Tests**: Integration tests for auth flow
- **Acceptance Criteria**:
  - [ ] Registration with email/password
  - [ ] Login returns JWT
  - [ ] Refresh token support
  - [ ] Logout invalidates token

### 4.3 Add Exchange Management Routes
- **File**: `testudo-exchange/crates/router/src/routes/exchanges.rs`
- **Endpoints**:
  - `GET /api/v1/exchanges` - List available exchanges
  - `GET /api/v1/exchanges/accounts` - User's exchange accounts
  - `POST /api/v1/exchanges/accounts` - Add API keys
  - `DELETE /api/v1/exchanges/accounts/:id` - Remove API keys
  - `POST /api/v1/exchanges/accounts/:id/test` - Test connection
- **Tests**: Route handler unit tests
- **Acceptance Criteria**:
  - [ ] CRUD for exchange accounts
  - [ ] Connection testing
  - [ ] Proper authorization
  - [ ] Input validation

### 4.4 Refactor Order Route
- **File**: `testudo-exchange/crates/router/src/routes/order.rs`
- **From**: Internal engine via Redis
- **To**: Exchange router with external execution
- **Changes**:
  ```rust
  // Before: Routes to internal engine
  redis_connection.push(RedisQueues::ORDERS, order_data)

  // After: Routes to exchange adapter
  exchange_router.route_order(standard_order, user_preferences).await
  ```
- **Tests**: Order routing logic tests
- **Acceptance Criteria**:
  - [ ] Routes to external exchanges
  - [ ] Validates against user's API keys
  - [ ] Returns standardized response
  - [ ] Error handling

---

## Implementation Strategy

### Development Order
1. **Week 1**: Authentication Foundation (Tasks 1.1-1.4)
2. **Week 2**: API Key Management (Tasks 2.1-2.4)
3. **Week 3**: Exchange Adapter Interface (Tasks 3.1-3.4)
4. **Week 4**: Router Refactoring (Tasks 4.1-4.4)

### Testing Strategy
- Unit tests for each module (80% coverage minimum)
- Integration tests for complete flows
- Mock external dependencies
- Property-based testing for critical paths

### Code Review Checklist
For each task:
- [ ] Tests written first (TDD)
- [ ] Tests pass
- [ ] Code follows SOLID principles
- [ ] No code duplication (DRY)
- [ ] Simple solution (KISS)
- [ ] Documentation complete
- [ ] Error handling comprehensive
- [ ] Performance acceptable

### Migration Path
- Maintain backward compatibility during transition
- Feature flags for gradual rollout
- Parallel running of old and new systems
- Comprehensive monitoring and rollback plan

---

## Success Metrics

### Technical Metrics
- [ ] All tests passing (100% of test suite)
- [ ] Code coverage > 80%
- [ ] No security vulnerabilities
- [ ] Performance benchmarks met

### Functional Metrics
- [ ] User registration and login working
- [ ] API key storage secure
- [ ] Exchange adapter interface defined
- [ ] Router successfully refactored

### Quality Metrics
- [ ] Zero critical bugs
- [ ] Documentation complete
- [ ] Code review approved
- [ ] Integration tests passing

---

## Next Steps

After Phase 1 completion:
1. **Phase 2**: Implement CCXT and Hyperliquid adapters
2. **Phase 3**: Risk management engine
3. **Phase 4**: UI updates for external exchanges
4. **Phase 5**: Production deployment

---

## Notes

- Each task should be completed in a single PR
- Tests must be written before implementation
- Code reviews required for all changes
- Documentation updates included with code
- Performance testing for critical paths