# E.4 Position Sync Implementation Prompt

Copy and paste everything below the line to start a new session:

---

## Task: Implement E.4 Position Sync

You are implementing position synchronization between Shadow (paper) and Binance (live) positions for the Testudo Hybrid Trading System. E.1-E.3 are complete.

### Context Files
- **PRD**: `@hybrid_trading.json` - E.4 acceptance criteria
- **E.3 Design**: `@docs/plans/e3-implementation-prompt.md` - Binance Executor
- **Progress**: `@.ralph/progress.md` - Completed work log

### Current State

**Completed (E.1 + E.2 + E.3):**
- `CredentialValidator` validates Binance API keys
- `DecisionLoop` validates orders against risk rules with `ExecutionMode`
- `BinanceExecutor` executes orders on Binance (create/get/cancel)
- Symbol normalization: `BTC_USDC` <-> `BTCUSDT`
- `ExecutionMode::Shadow` vs `ExecutionMode::Live` in DecisionInput/Result

**Key Files:**
- `common_utils/src/adapters/binance_executor.rs` - Order execution
- `common_utils/src/adapters/execution_types.rs` - ValidatedOrder, BinanceOrderResult
- `router/src/decision_loop.rs` - Decision Loop with execution mode
- `engine/src/shadow/positions.rs` - Shadow position tracking

### What to Build

**Position Syncer** (`common_utils/src/adapters/position_sync.rs`):
```rust
pub struct PositionSyncer {
    executor: BinanceExecutor,
}

impl PositionSyncer {
    /// Fetch current positions from Binance
    pub async fn fetch_binance_positions(&self) -> Result<Vec<BinancePosition>, SyncError>

    /// Compare shadow positions with Binance positions
    pub async fn compare_positions(
        &self,
        shadow: &[ShadowPosition],
        binance: &[BinancePosition],
    ) -> PositionDiff

    /// Reconcile positions (optional - alert user of discrepancies)
    pub async fn reconcile(&self, diff: &PositionDiff) -> ReconcileResult
}
```

**Position Types** (`common_utils/src/adapters/position_types.rs`):
```rust
pub struct BinancePosition {
    pub symbol: String,           // "BTCUSDT"
    pub side: PositionSide,       // LONG or SHORT (for futures), BUY for spot
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub timestamp: i64,
}

pub struct PositionDiff {
    /// Positions in shadow but not on Binance
    pub shadow_only: Vec<ShadowPosition>,
    /// Positions on Binance but not in shadow
    pub binance_only: Vec<BinancePosition>,
    /// Positions with quantity mismatch
    pub quantity_mismatch: Vec<QuantityMismatch>,
    /// Positions that match
    pub matched: Vec<MatchedPosition>,
}

pub struct QuantityMismatch {
    pub symbol: String,
    pub shadow_qty: Decimal,
    pub binance_qty: Decimal,
}

pub enum SyncError {
    NetworkError(String),
    AuthenticationFailed,
    RateLimited { retry_after_ms: u64 },
    ParseError(String),
}

pub enum ReconcileAction {
    /// Update shadow to match Binance
    UpdateShadow,
    /// Alert user to manually resolve
    AlertUser,
    /// Close orphaned position on Binance
    CloseOrphaned,
}
```

**Sync Triggers** (from PRD acceptance criteria):
1. **On app start** - Full position sync
2. **After each trade** - Incremental sync
3. **Background sync** - Every 60 seconds

**Integration Points:**

1. **Create sync service** (`router/src/services/sync_service.rs`):
   ```rust
   pub struct SyncService {
       syncer: PositionSyncer,
       shadow_engine: Arc<ShadowEngine>,
       sync_interval: Duration,
   }

   impl SyncService {
       pub async fn start_background_sync(&self) -> JoinHandle<()>
       pub async fn sync_now(&self) -> Result<SyncResult, SyncError>
       pub async fn sync_after_trade(&self, symbol: &str) -> Result<(), SyncError>
   }
   ```

2. **Add sync endpoint** (`router/src/routes/sync.rs`):
   ```
   POST /api/v1/sync              # Trigger manual sync
   GET  /api/v1/sync/status       # Get last sync result
   GET  /api/v1/sync/diff         # Get current position differences
   ```

3. **Wire into startup** (`router/src/main.rs`):
   - Start background sync task on app initialization
   - Sync on user login (when API keys are loaded)

### Acceptance Criteria (from PRD)

```json
{
  "sync_trigger": "On app start, after each trade, every 60s background",
  "discrepancy_handling": "If Binance position != shadow position, alert user and offer reconciliation",
  "balance_sync": "Update shadow balances from Binance on sync"
}
```

### TDD Required

Follow Red-Green-Refactor. Key tests:

1. `test_fetch_binance_positions` - Returns current positions
2. `test_compare_positions_match` - Matching positions identified
3. `test_compare_positions_mismatch` - Quantity differences detected
4. `test_compare_shadow_only` - Shadow-only positions identified
5. `test_compare_binance_only` - Binance-only positions identified
6. `test_sync_after_trade` - Incremental sync works
7. `test_background_sync_interval` - Background task runs at interval

### API Endpoints (Binance)

**Get Account Info (Spot):**
```
GET /api/v3/account
Headers: X-MBX-APIKEY: <api_key>
Params: timestamp, signature
Returns: balances array with asset, free, locked
```

**Get Open Orders:**
```
GET /api/v3/openOrders
Headers: X-MBX-APIKEY: <api_key>
Params: symbol (optional), timestamp, signature
```

**Get All Orders (history):**
```
GET /api/v3/allOrders
Headers: X-MBX-APIKEY: <api_key>
Params: symbol, timestamp, signature
```

### Feature Flag

Use `#[cfg(feature = "real-api")]` for actual Binance calls:
```rust
#[cfg(feature = "real-api")]
async fn fetch_real(&self) -> Result<Vec<BinancePosition>, SyncError> {
    // Real Binance API call
}

#[cfg(not(feature = "real-api"))]
async fn fetch_real(&self) -> Result<Vec<BinancePosition>, SyncError> {
    // Return mock positions
}
```

### Success Criteria

- [ ] PositionSyncer fetches Binance positions via authenticated API
- [ ] Position comparison identifies matches, mismatches, and orphans
- [ ] Background sync runs every 60 seconds
- [ ] Sync triggers after each trade execution
- [ ] Balance sync updates shadow balances from Binance
- [ ] Discrepancy alerts shown to user (not auto-reconciled)
- [ ] All tests pass: `cargo test -p common_utils position_sync`
- [ ] Mark E.4 as "complete" in hybrid_trading.json

### Do NOT

- Do not auto-reconcile positions without user consent
- Do not close Binance positions automatically
- Do not sync more frequently than every 60 seconds (rate limits)
- Do not skip TDD - write failing test first
- Do not implement WebSocket position updates (future enhancement)
