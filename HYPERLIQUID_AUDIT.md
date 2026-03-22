# Hyperliquid Implementation Audit Report

**Date:** 2026-03-16
**Scope:** All Hyperliquid-related code in testudo-exchange (22 files, ~4500 LOC)
**Auditors:** 5 parallel code-hound agents

---

## CRITICAL (Fix Before Ship)

### 1. `f64` for Financial Data — Violates Project Rules
**`ws_fills.rs:146-147`, `cex_client.rs:142-146`**

`OrderUpdateEvent` uses `Option<f64>` for `price`, `amount`, `filled`, `remaining`, `average`. The `translate` function parses to `f64`. Project rules mandate `rust_decimal::Decimal` — never `f64`. IEEE 754: `0.1 + 0.2 != 0.3`. This is a correctness bug in a system that moves real money.

### 2. `limit_px` Used as Average Fill Price
**`ws_fills.rs:160-163`**

```rust
let average = if update.status == "filled" {
    order.limit_px.parse().ok()  // WRONG: this is limit price, not avg fill price
} else { None };
```

For market/stop orders with slippage, `limit_px != avg_px`. The REST path in `exchange_api.rs:248` correctly uses `avg_px`. The WS fill subscriber lies about fill prices — catastrophic for P&L during volatile wicks.

### 3. Silent Zero on Parse Failure — Data Corruption
**`ws_fills.rs:146-147`, `exchange_api.rs:255-257`**

```rust
let orig_sz: f64 = order.orig_sz.parse().unwrap_or(0.0);  // ws_fills
fn parse_decimal(s: &str) -> Decimal { Decimal::from_str(s).unwrap_or(Decimal::ZERO) }  // exchange_api
```

Malformed API responses silently become zero. A zero-size fill makes positions invisible. A zero balance shows "no funds." In a financial system, parse failures must propagate as errors.

### 4. Race Condition: Double Approval — No Atomic Guard
**`routes/exchanges.rs:686-777`**

Two concurrent `POST /agent-wallet/approve` requests both succeed. No `is_active = false` precondition check before the external API call. Both submit to Hyperliquid, both set `is_active = true`. Fix: use `UPDATE ... WHERE is_active = false RETURNING ...`.

### 5. Race Condition: Migrate vs Approve Overlap
**`routes/exchanges.rs:782-844`, `repositories/exchange_account.rs:385-451`**

`migrate_to_agent_wallet` replaces the keypair while a concurrent `approve_agent` reads old credentials. If approve commits after migrate, account is active with the OLD replaced key. Needs `SELECT ... FOR UPDATE` row locking.

### 6. Exchange Validation vs Display List Mismatch
**`validation.rs:97` vs `routes/exchanges.rs:30-76`**

Validation allows: `binance, coinbase, kraken, hyperliquid`
UI displays: `binance, woo, bybit, okx, hyperliquid`

WOO, Bybit, OKX are shown but **rejected by validation**. Coinbase and Kraken pass validation but aren't displayed. Two sources of truth for the same knowledge.

---

## HIGH (Fix Soon)

### 7. TOCTOU Race in AuthCache `get_or_insert`
**`auth.rs:153-174, 177-196`**

Read-lock check → drop lock → construct signer → write-lock insert. Between drop and reacquire, N concurrent threads all see empty cache, all construct signers. During credential rotation, stale signer can overwrite fresh one. Fix: double-check under write lock.

### 8. Unbounded AuthCache — No Eviction
**`auth.rs:141-143`**

`HashMap<Uuid, HyperliquidAuth>` with no TTL, no max-capacity, no LRU. Only removal is explicit `invalidate()` on migrate/revoke. Deleted accounts leak forever.

### 9. Fill Loss During WebSocket Reconnect
**`ws_fills.rs:83-93`**

No reconciliation after reconnect — fills during the gap are permanently lost. No watermark or "fetch since last timestamp" mechanism. Fills matter most during volatile conditions, which are also when disconnects happen.

### 10. `amend_order` Defaults to Buy When Side is None
**`exchange_api.rs:374-378`**

```rust
None => true, // default; should not happen in practice
```

If side is unknown, order defaults to BUY. Could flip a stop-loss from sell to buy. "Should not happen" comments are time bombs. Return an error.

### 11. Double DB Query Per Operation
**`routing.rs:46-68` → `exchange_api.rs:64-120`**

Every `RoutingExchangeApi` call does `is_hyperliquid()` (DB query), then the delegated method does `load_auth()` (same DB query again). 2x latency on every operation.

### 12. AgentRotationService is Dead Code
**`agent_rotation.rs:86` — never called from `main.rs`**

`spawn_rotation_checker` exists but is never invoked. Users get no rotation warnings. The AW-05 feature is shipped but doesn't execute.

### 13. 30-Line Decrypt Logic Copy-Pasted
**`repositories/exchange_account.rs:206-253` vs `311-358`**

`load_credentials` and `load_credentials_for_approval` contain identical decrypt-and-construct logic. A bug fix in one will be missed in the other.

### 14. `is_active` Defaults to `true` on NULL
**`routes/exchanges.rs:121, 218`**

```rust
is_active: row.is_active.unwrap_or(true),
```

Fail-safe default for exchange credentials should be `false` (inactive), not `true` (active). Deny by default.

---

## MEDIUM

| # | Finding | Location |
|---|---------|----------|
| 15 | `"agent_wallet"` magic string repeated 15+ times — no constant | Multiple files |
| 16 | `"hyperliquid"` comparison is case-sensitive in some places, case-insensitive in others | `ws_subscription_manager.rs:221` vs `validation.rs:101` |
| 17 | `reqwest::Client::new()` created per request — no connection pooling | `agent_approval.rs:135, 180` |
| 18 | No timeout on external HTTP calls to Hyperliquid API | `agent_approval.rs:135-141, 180-186` |
| 19 | Nonce not persisted or validated in approval flow | `routes/exchanges.rs:663-666, 686-777` |
| 20 | `hl_network()` reads env var on every request instead of from AppState | `routes/exchanges.rs:607-613` |
| 21 | WsSubscriptionManager entries never pruned — HashMap grows forever | `ws_subscription_manager.rs` |
| 22 | Rotation checker has no shutdown signal (`CancellationToken`) | `agent_rotation.rs:87-106` |
| 23 | `env::set_var` in tests — unsafe/UB in multithreaded programs | `agent_rotation.rs:129-149` |
| 24 | Account lookup pattern duplicated 3 times | `exchange_api.rs`, `routing.rs` |
| 25 | CexClientError→HttpResponse mapping duplicated 3 times | `routes/exchanges.rs:170, 405, 538` |
| 26 | `PositionRepository::new(pg_pool.clone())` constructed 5 times in main.rs | `main.rs:310, 429, 526, 577, 720` |
| 27 | `main()` is ~840 lines — monolithic SRP violation | `main.rs:125-963` |
| 28 | 15 clippy lint suppressions masking real issues | `main.rs:1-15` |
| 29 | `AssetUniverse` fetched once at startup, never refreshed | `main.rs:372-382` |
| 30 | `reconnect_delay` + `wait_or_cancel` duplicated identically in 2 files | `ws_fills.rs:182` vs `ws_subscription_manager.rs:429` |

---

## LOW

| # | Finding | Location |
|---|---------|----------|
| 31 | `AuthError::InvalidPrivateKey` reused for wallet address parse failures | `auth.rs:104-106` |
| 32 | `verifyingContract` zero address not a named constant | `agent_approval.rs:61` |
| 33 | CLOID uses DNS namespace for non-DNS data | `exchange_api.rs:31` |
| 34 | `_USDT` suffix hardcoded in `from_hl_coin` — assumes USDT-margined only | `universe.rs:112` |
| 35 | WS backoff parameters not configurable | `ws_fills.rs:182-184` |
| 36 | `Box<dyn Error>` return type instead of typed error enum | `agent_rotation.rs:49` |
| 37 | `stop_tx.send(true)` immediately followed by `handle.abort()` — abort masks graceful shutdown | `ws_subscription_manager.rs:123-124` |
| 38 | Market order slippage magic numbers `999_999_999` and `0.01` not named constants | `exchange_api.rs:179-189` |
| 39 | Hardcoded exchange list in route handler | `routes/exchanges.rs:30-76` |
| 40 | `wallet_address VARCHAR(42)` — zero margin for edge cases | Migration SQL |

---

## Hardcoded Values Summary

| Value | Location | Risk |
|-------|----------|------|
| `999_999_999` (market buy slippage) | `exchange_api.rs:182` | Breaks if asset > $999M |
| `0.01` (market sell floor) | `exchange_api.rs:187` | Arbitrary |
| `421614` (chain ID) | `agent_approval.rs:16` | Named constant — acceptable |
| `23` hours (rotation TTL) | `agent_rotation.rs:38` | Env-configurable — acceptable |
| `300` seconds (rotation poll) | `agent_rotation.rs:88` | Not configurable |
| `3600` / `2592000` (JWT expiry) | `main.rs:156-157` | Overrides config |
| `100` / `10` (rate limits) | `main.rs:158-159` | Overrides config |

---

## Mock/Test Data in Production

**CLEAN.** All test data is properly `#[cfg(test)]` gated. No dummy values, placeholder addresses, or test-only code paths leak into production.

---

## Quality Scores by Module

| Module | TDD | KISS | SOLID | DRY | Overall |
|--------|-----|------|-------|-----|---------|
| auth.rs | 75 | 85 | 88 | 70 | 78 |
| exchange_api.rs | 72 | 78 | 82 | 55 | 68 |
| ws_fills.rs | 45 | 82 | 65 | 60 | 48 |
| routes/exchanges.rs | 25 | 55 | 50 | 35 | 48 |
| main.rs integration | 55 | 35 | 40 | 45 | 46 |
| **Overall** | **54** | **67** | **65** | **53** | **58** |

---

## Top 5 Priority Fixes

1. **Replace `f64` with `Decimal` in `OrderUpdateEvent`** and fix `parse_decimal`/`unwrap_or(0.0)` to propagate errors — financial correctness
2. **Fix `limit_px` → `avg_px`** in ws_fills translate — wrong fill price reporting
3. **Add atomic guards to approval/migrate/revoke** — race conditions on credential state transitions
4. **Unify exchange validation list** with display list — users can't register displayed exchanges
5. **Add AuthCache eviction** and fix TOCTOU in `get_or_insert` — memory leak + stale credential race

---

## SDK Reconciliation

Cross-referenced against `hyperliquid-sdk-rs v0.1.2` documentation (Context7, crates.io, GitHub).

### SDK Version

```toml
# Cargo.toml
hyperliquid-sdk-rs = "0.1.2"
# Resolved from crates.io registry, checksum: 94a8bb9d...
```

### Confirmed SDK Limitations (Not Our Bugs)

#### `BasicOrder` lacks `avg_px` — CONFIRMS Critical #2
The SDK's WebSocket `OrderUpdate` → `BasicOrder` struct only exposes `limit_px`, `sz`, `orig_sz`, `oid`, `coin`, `side`, `cloid`, `timestamp`. There is **no `avg_px` field** on the WebSocket message.

The REST response type `FilledOrder` **does** have `avg_px` (used correctly at `exchange_api.rs:248`).

**Implication**: Using `limit_px` as fill price in `ws_fills.rs:161` is not a "wrong field" bug — it's a **missing data** problem. The WebSocket feed genuinely doesn't provide average fill price. The fix requires a REST reconciliation query after detecting a fill via WS:

```rust
// After WS reports status "filled":
// 1. Use InfoProvider::user_state() or frontend_open_orders() to get actual avg_px
// 2. Update the OrderUpdateEvent with the real fill price
```

#### SDK Examples Use `f64` — But Our Rules Override
The SDK's own documentation examples parse `szi` to `f64`:
```rust
pos.szi.parse::<f64>()?  // SDK example code
```
However, our project rules (`rust-backend.md`) mandate `rust_decimal::Decimal`. The SDK returns all values as `String`, so we can parse to either. Our rules are stricter and correct for a financial system. **Critical #1 stands.**

### API Usage Correctness

| SDK Feature | Our Usage | Status |
|---|---|---|
| `ExchangeProvider::mainnet(signer)` | `build_exchange` at `exchange_api.rs:133` | Correct — compiles with `PrivateKeySigner` directly |
| `ExchangeProvider::mainnet_agent(signer, addr)` | `build_exchange` at `exchange_api.rs:139` | Correct |
| `ExchangeProvider::testnet_agent(signer, addr)` | `build_exchange` at `exchange_api.rs:142` | Correct |
| `InfoProvider::new(network)` | Constructor at `exchange_api.rs:53` | Correct |
| `InfoProvider::user_state(address)` | Balance + positions queries | Correct |
| `InfoProvider::open_orders(address)` | `cancel_all_orders` filter | Correct |
| `InfoProvider::frontend_open_orders(address)` | CLOID→OID fallback at `exchange_api.rs:594` | Correct |
| `InfoProvider::meta()` | `AssetUniverse::fetch` | Correct |
| `RawWsProvider::connect(network)` | `ws_fills.rs:122` | Correct |
| `RawWsProvider::subscribe(Subscription)` | `ws_fills.rs:130` | Correct |
| `RawWsProvider::start_reading()` | `ws_fills.rs:135` | Correct |
| `OrderRequest::limit(asset, is_buy, px, sz, tif)` | `build_order_request` | Correct |
| `OrderRequest::trigger(asset, is_buy, trigger_px, sz, tpsl, is_market)` | Stop-loss orders | Correct |
| `.reduce_only(bool)` | Applied correctly to all order types | Correct |
| `.with_cloid(Option<Uuid>)` | Deterministic UUID v5 from `testudo:{group}:{role}` | Correct |
| `exchange.place_order(&order)` | `exchange_api.rs:306` | Correct |
| `exchange.place_order_with_cloid(order, uuid)` | `exchange_api.rs:303` | Correct |
| `exchange.modify_order(oid, order)` | `exchange_api.rs:415` | Correct |
| `exchange.cancel_order(asset, oid)` | `exchange_api.rs:474` | Correct |
| `exchange.cancel_order_by_cloid(asset, uuid)` | `exchange_api.rs:466` | Correct |
| `exchange.bulk_cancel(vec)` | `exchange_api.rs:525` | Correct |
| `ExchangeDataStatus::Resting(r)` | `extract_order_id` | Correct |
| `ExchangeDataStatus::Filled(f)` | `extract_order_id` + `extract_avg_price` | Correct |
| `ExchangeDataStatus::WaitingForTrigger` | Handled as None in `extract_order_id` | Correct |
| `response.into_result()` | Used after all exchange operations | Correct |

### SDK Patterns — Divergences (Not Bugs)

#### 1. `AlloySigner` Wrapper Not Used
SDK docs show: `AlloySigner { inner: signer }` wrapper.
Our code: passes `PrivateKeySigner` directly.
**Verdict**: SDK v0.1.2 provides a blanket `HyperliquidSigner` impl for `PrivateKeySigner` (the generic bound on `ExchangeProvider<S>`). Our code compiles and works. The `AlloySigner` wrapper may be newer or from a fork. **No action needed.**

#### 2. `RawWsProvider` Instead of `WsProvider`
SDK has high-level `WsProvider` with convenience methods: `subscribe_open_orders()`, `subscribe_clearinghouse_state()`, `subscribe_l2_book()`.
Our code uses low-level `RawWsProvider` with manual `Subscription::OrderUpdates` construction.
**Verdict**: `WsProvider` doesn't expose a direct `OrderUpdates` subscription convenience method. `RawWsProvider` is the correct choice for custom subscription types. **No action needed.**

#### 3. Builder Pattern Not Used
SDK offers: `exchange.order(0).limit_buy("50000", "0.001").send()`
Our code uses: `OrderRequest::limit(asset_index, is_buy, price, sz, tif)`
**Verdict**: Both patterns are valid. The builder is newer and more ergonomic. Our direct construction gives explicit control over all fields. **Optional refactor — low priority.**

#### 4. `ModifyRequest` Not Used for Bulk
SDK has `ModifyRequest { oid, order }` for `bulk_modify()`.
Our code uses single `modify_order(oid, new_order)`.
**Verdict**: We currently only amend one order at a time. Bulk modify would be useful if we ever amend SL+TP together. **No action needed now.**

### SDK Feature Gaps (Not Used But Available)

| Feature | SDK Method | Potential Use |
|---|---|---|
| MEV Builder support | `ExchangeProvider::mainnet_builder(signer, builder_addr)` | Not needed — no MEV protection required |
| Clearinghouse state WS | `subscribe_clearinghouse_state(user)` | Could replace REST polling for balance updates |
| TWAP orders | `subscribe_twap_states(user)` | Not needed — no TWAP strategy |
| All mid prices WS | `subscribe_all_mids()` | Could be used for real-time price feeds |
| L2 book WS | `subscribe_l2_book(coin)` | Could enable orderbook display |
| Trades WS | `subscribe_trades(coin)` | Could enable trade feed |
| Bulk modify | `bulk_modify(Vec<ModifyRequest>)` | Atomic SL+TP amendments |

### Final SDK Verdict

The implementation correctly uses the SDK's core APIs for order management, authentication, and WebSocket subscriptions. The API surface coverage is appropriate for the trading use case. Two areas need attention:

1. **Fill price reconciliation** (Critical #2): Architectural gap — WS doesn't provide avg_px. Add a REST query after fill detection to get true average price from `FilledOrder.avg_px`.
2. **Financial precision** (Critical #1/3): The SDK returns strings. Parse to `Decimal`, not `f64`. The SDK's own examples use `f64` but our project rules are correct for production trading.
