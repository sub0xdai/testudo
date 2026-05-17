# E.3 Binance Execution Implementation Prompt

Copy and paste everything below the line to start a new session:

---

## Task: Implement E.3 Binance Order Execution

You are implementing real order execution on Binance for the Testudo Hybrid Trading System. E.1 (API key storage) and E.2 (Decision Loop) are complete.

### Context Files
- **PRD**: `@hybrid_trading.json` - E.3 acceptance criteria
- **E.1 Design**: `@docs/plans/2026-01-10-e1-api-key-storage-design.md` - Credential storage
- **E.2 Design**: `@docs/plans/2026-01-10-e2-decision-loop-design.md` - Decision Loop

### Current State

**Completed (E.1 + E.2):**
- `CredentialValidator` validates Binance API keys
- `DecisionLoop` validates orders against risk rules
- `RiskService` calculates position size and returns `RiskCheckResult`
- Position sizing methods: fixed_fractional, kelly_criterion, volatility_adjusted
- CCXT adapters with HMAC authentication for Binance/Coinbase/Kraken

**Key Files:**
- `common_utils/src/adapters/ccxt_auth.rs` - Binance HMAC signing
- `common_utils/src/adapters/market_data.rs` - Order creation stubs
- `router/src/decision_loop.rs` - Decision Loop orchestration
- `router/src/routes/order.rs` - `/api/v1/order` endpoint (currently returns mock)

### What to Build

**Binance Executor** (`common_utils/src/adapters/binance_executor.rs`):
```rust
pub struct BinanceExecutor {
    client: CCXTHTTPClient,
    auth: CCXTAuthenticator,
}

impl BinanceExecutor {
    /// Execute a validated order on Binance
    pub async fn execute(&self, order: &ValidatedOrder) -> Result<BinanceOrderResult, ExecutionError>

    /// Get order status
    pub async fn get_order(&self, order_id: &str, symbol: &str) -> Result<OrderStatus, ExecutionError>

    /// Cancel an order
    pub async fn cancel(&self, order_id: &str, symbol: &str) -> Result<(), ExecutionError>
}
```

**Execution Types** (`common_utils/src/adapters/execution_types.rs`):
```rust
pub struct ValidatedOrder {
    pub symbol: String,           // "BTCUSDT" (Binance format)
    pub side: OrderSide,          // BUY or SELL
    pub order_type: OrderType,    // LIMIT or MARKET
    pub quantity: Decimal,
    pub price: Option<Decimal>,   // Required for LIMIT
    pub time_in_force: TimeInForce,
}

pub struct BinanceOrderResult {
    pub order_id: String,
    pub client_order_id: String,
    pub status: OrderStatus,
    pub filled_qty: Decimal,
    pub avg_price: Decimal,
    pub timestamp: i64,
}

pub enum ExecutionError {
    InsufficientBalance { required: Decimal, available: Decimal },
    RateLimited { retry_after_ms: u64 },
    InvalidSymbol(String),
    NetworkError(String),
    AuthenticationFailed,
    OrderRejected { code: i32, message: String },
}
```

**Integration Points:**

1. **Wire into Decision Loop** (`router/src/decision_loop.rs`):
   - After `RiskCheckResult::approved`, call `BinanceExecutor::execute()`
   - Add `execution_mode: ExecutionMode` to `DecisionInput` (Shadow | Live)
   - Only execute on Binance if mode is `Live`

2. **Update Order Route** (`router/src/routes/order.rs`):
   - Load user's API keys from credential storage
   - Pass execution mode from request body
   - Return Binance order ID in response when live

3. **Symbol Normalization**:
   - Internal: `BTC_USDC`
   - Binance: `BTCUSDT`
   - Create `normalize_symbol()` and `denormalize_symbol()` functions

### Acceptance Criteria (from PRD)

```json
{
  "order_mapping": "Shadow order -> Binance order format conversion",
  "error_handling": {
    "insufficient_balance": "Return error, do not retry",
    "rate_limit": "Exponential backoff, max 3 retries",
    "network_error": "Retry with timeout, alert user if persistent"
  },
  "confirmation": "Wait for Binance fill confirmation before updating shadow"
}
```

### TDD Required

Follow Red-Green-Refactor. Key tests:

1. `test_execute_limit_order` - Valid order returns BinanceOrderResult
2. `test_execute_market_order` - Market order fills immediately
3. `test_insufficient_balance` - Returns ExecutionError::InsufficientBalance
4. `test_rate_limit_retry` - Retries with backoff on 429
5. `test_symbol_normalization` - BTC_USDC <-> BTCUSDT
6. `test_shadow_mode_skips_execution` - Shadow mode doesn't hit Binance
7. `test_live_mode_executes` - Live mode calls Binance API

### API Endpoints (Binance)

**Create Order:**
```
POST /api/v3/order
Headers: X-MBX-APIKEY: <api_key>
Body: symbol, side, type, quantity, price, timeInForce, timestamp, signature
```

**Get Order:**
```
GET /api/v3/order?symbol=BTCUSDT&orderId=123&timestamp=...&signature=...
```

**Cancel Order:**
```
DELETE /api/v3/order?symbol=BTCUSDT&orderId=123&timestamp=...&signature=...
```

### Feature Flag

Use `#[cfg(feature = "real-api")]` for actual Binance calls:
```rust
#[cfg(feature = "real-api")]
async fn execute_real(&self, order: &ValidatedOrder) -> Result<BinanceOrderResult, ExecutionError> {
    // Real Binance API call
}

#[cfg(not(feature = "real-api"))]
async fn execute_real(&self, order: &ValidatedOrder) -> Result<BinanceOrderResult, ExecutionError> {
    // Return mock response
}
```

### Success Criteria

- [ ] BinanceExecutor creates/gets/cancels orders via authenticated API
- [ ] Symbol normalization works both directions
- [ ] Error handling covers all Binance error codes
- [ ] Retry logic with exponential backoff for rate limits
- [ ] Shadow mode skips Binance, Live mode executes
- [ ] Integration tests pass with mock responses
- [ ] All tests pass: `cargo test -p common_utils binance`
- [ ] Mark E.3 as "complete" in hybrid_trading.json

### Do NOT

- Do not store credentials in memory longer than needed
- Do not log API keys or signatures
- Do not skip TDD - write failing test first
- Do not implement WebSocket order updates (that's E.4)
- Do not implement position sync yet (that's E.4)
