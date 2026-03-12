# Testudo Platform Audit Report

**Date:** March 7, 2026
**Scope:** Race conditions, memory leaks, extension bugs, launch readiness
**Method:** 5 parallel investigation agents (~380k tokens of analysis)

---

## Executive Summary

| Metric | Count |
|--------|-------|
| Race Conditions | 11 (2 critical, 4 medium, 5 low) |
| Memory Leaks | 13 (3 critical, 3 high, 4 medium, 3 low) |
| Extension Bugs | 24 (8 medium, 16 low) |
| Launch Blockers | 15 (7 P0, 6 P1, 2 P2) |
| Tests Passing | 752 (709 Rust + 43 sidecar) |
| Est. to Launch | ~5 weeks |

**Verdict: NOT READY for production.** Core trading works (paper + live on WOO/Binance), but critical gaps in fill subscription reliability, trade safety, observability, and CI/CD must be addressed.

**P0 Blocker:** EXT-25 (Live Fill Subscription Reliability) is unimplemented. Without it, SL/TP fills on the exchange can leave sibling orders open indefinitely, creating unbounded financial risk.

---

## 1. Race Conditions — Rust Backend

### RC-1: TOCTOU Balance Check vs Live Order Placement [CRITICAL]

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs:515-833`

Balance fetched at line 517 for position sizing, but orders placed much later (lines 717-833). Two concurrent requests from the same user can consume the same balance:

```
Request A: fetch_balance → 1000 USDT → size = 0.5 BTC
Request B: fetch_balance → 1000 USDT → size = 0.5 BTC  (same balance!)
Request A: place_order → SUCCESS (0.5 BTC)
Request B: place_order → SUCCESS (0.5 BTC)  — DOUBLE EXPOSURE
```

**Fix:** Add per-user `Mutex` or `Semaphore(1)` around the live trade creation path.

### RC-2: Non-Atomic SL+TP Placement on Exchange [CRITICAL]

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs:716-842`

Entry, SL, and TP placed as 3 sequential API calls. If entry succeeds but SL fails (line 788-794), code logs `warn!("Failed to place SL on exchange (will manage locally)")` and proceeds. The "manage locally" claim is misleading — TradeManagerService only adjusts existing SL orders, it doesn't re-place missing ones.

**Fix:** If SL placement fails, cancel the entry order (rollback). Or retry with exponential backoff. At minimum, flag position as "unprotected" and alert the user.

### RC-3: TOCTOU in update_stop_loss / update_entry_price [MEDIUM]

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs:1067-1457`

Group status read with read lock, lock dropped, then old order cancelled and new placed. Between read and write, group status could change (e.g., FillDetectorService marks it as StoppedOut). Could result in cancelling an already-filled order and creating a replacement — two positions.

**Fix:** Use write lock for entire operation, or re-validate status after acquiring write lock.

### RC-4: FillDetector No Retry on Transient Cancel Failures [MEDIUM]

**File:** `testudo-exchange/crates/router/src/services/fill_detector.rs:321-329`

When sibling order cancel fails with non-OrderNotFound error (network timeout, rate limit), error is only logged. No retry mechanism. Orphan order remains on exchange.

**Fix:** Add 2-3 retry with exponential backoff for transient failures.

### RC-5: Dual OCO Cancel Paths Race [MEDIUM]

**Files:** `price_feed.rs:186-223`, `fill_detector.rs:200-280`

Both PriceFeedService and FillDetectorService can independently trigger OCO cancellation. Mostly idempotent due to terminal state checks, but makes reasoning about correctness harder.

### RC-6: WsSubscriptionManager abort() Drops In-Flight Events [MEDIUM]

**File:** `testudo-exchange/crates/router/src/services/ws_subscription_manager.rs:105-111`

When adding a symbol, current task is `abort()`'d immediately. If processing an order event mid-flight, that fill is lost forever.

**Fix:** Replace `abort()` with graceful shutdown — send stop signal and `await` the handle.

### RC-7 through RC-11 [LOW]

| # | Issue | File |
|---|-------|------|
| RC-7 | Triple write lock in process_price_update (latency, not correctness) | `shadow/mod.rs:430-434` |
| RC-8 | Unbounded management event channel | `main.rs:241` |
| RC-9 | Balance check + reserve not atomic (false alarm — reserve re-checks) | `shadow/mod.rs:222-237` |
| RC-10 | Nested write locks in rehydration (startup only, safe) | `rehydration.rs:55-56` |
| RC-11 | `std::sync::RwLock` with `unwrap()` in async context | `main.rs:72` |

---

## 2. Memory & Resource Leaks

### ML-1: ShadowOrderManager Never Removes Filled/Cancelled Orders [CRITICAL]

**File:** `testudo-exchange/crates/engine/src/shadow/orders.rs:331-497`

`orders: HashMap<Uuid, ShadowOrder>` stores every order ever placed. When cancelled or filled, status is updated but entry is never removed. `orders_by_user` also accumulates every order ID forever.

**Impact:** ~400 bytes per order, unbounded growth.

### ML-2: ShadowPositionManager Never Removes Closed Positions [CRITICAL]

**File:** `testudo-exchange/crates/engine/src/shadow/positions.rs:136-211`

Closed positions removed from `open_positions_by_symbol` but never from `positions` or `positions_by_user`. `get_total_realized_pnl` iterates all positions including closed, getting progressively slower.

### ML-3: OrderGroupManager: 5 HashMaps Never Pruned [CRITICAL]

**File:** `testudo-exchange/crates/engine/src/shadow/order_group.rs:247-258`

Five maps (`groups`, `groups_by_user`, `groups_by_entry_order`, `groups_by_linked_order`, `groups_by_exchange_order`) — terminal groups (StoppedOut, TookProfit, Cancelled, Closed) stay in all five maps forever. ~500+ bytes per group with 3-5 exchange order registrations each. Largest unbounded growth source.

### Recommended Fix for ML-1/2/3

Implement `prune_terminal()` on ShadowOrderManager, ShadowPositionManager, and OrderGroupManager. Keep terminal entries for 1 hour for query purposes, then evict. Call periodically from PriceFeedService tick or a dedicated background task.

### ML-4: TradeManagerService positions + last_amend Never Purge [HIGH]

**File:** `testudo-exchange/crates/router/src/services/trade_manager/service.rs:32-38`

Closed positions stay in `RwLock<HashMap>`. `last_amend` debounce timestamps never cleaned for closed positions.

### ML-5: CcxtClient Spawns Orphaned WebSocket Read Tasks [HIGH]

**File:** `testudo-exchange/crates/router/src/services/ccxt_client.rs:482-516`

`tokio::spawn` JoinHandle is dropped (fire-and-forget). Old read loop not cancelled when WsSubscriptionManager restarts subscription. Zombie tasks accumulate, each holding a broadcast Sender and WebSocket read half.

### ML-6: 6+ Background Tasks with No Shutdown Coordination [HIGH]

**File:** `testudo-exchange/crates/router/src/main.rs:184-473`

Six `tokio::spawn` calls (sidecar health, mgmt event forwarder, PriceFeedService, TradeManagerService x2, FillDetectorService) — none have shutdown signals. No `CancellationToken`, no `watch` channel. Orphaned when Actix server shuts down.

**Fix:** Create `tokio_util::sync::CancellationToken` in main(), pass to all tasks, wire `tokio::signal::ctrl_c()` handler.

### ML-7 through ML-13

| # | Sev | Issue | Component |
|---|-----|-------|-----------|
| ML-7 | Medium | SyncService weak shutdown signal | `sync_service.rs:97-137` |
| ML-8 | Medium | OrderBook user_orders index leaks on fills (acknowledged in comment) | `orderbook.rs:173-177` |
| ML-9 | Medium | WsSubscriptionManager entries never cleaned up for abandoned users | `ws_subscription_manager.rs:34` |
| ML-10 | Medium | PgWsManager stale subscription indices on disconnect | `pg_ws_manager.rs:39-44` |
| ML-11 | Low | CCXT pool doesn't call exchange.close() on eviction | `pool.js:71-78` |
| ML-12 | Low | ws-orders.js hot retry loop on transient errors (no backoff) | `ws-orders.js:103-159` |
| ML-13 | Low | No SIGTERM handler, no graceful drain | `main.rs` |

---

## 3. Extension Bugs — TypeScript

### Race Conditions & State Management

| # | Sev | Issue | File |
|---|-----|-------|------|
| EX-4 | Medium | `ensureActiveExchange` not awaited after login — popup gets success before exchange ready | `background.ts:929-933` |
| EX-3 | Medium | No concurrency guard on trade execution — rapid clicks fire duplicates | `background.ts:325` |
| EX-1 | Low | Token refresh races with in-flight API calls (self-healing via 401 retry) | `background.ts:218-345` |
| EX-2 | Low | WebSocket reconnect race with stale closure | `background.ts:824-838` |
| EX-5 | Low | WS_STATUS auto-reconnect returns stale "disconnected" state | `background.ts:947-954` |

### Memory Leaks

| # | Sev | Issue | File |
|---|-----|-------|------|
| EX-6 | Medium | Sidecar health polling interval (30s) never stopped — keeps worker alive forever | `background.ts:761-767` |
| EX-7 | Medium | `refreshTimer` not cleared on logout — fires after logout, fails silently | `background.ts:257-265, 935` |
| EX-8 | Low | WebSocket not disconnected on logout — receives updates for logged-out user | `background.ts:935-937` |
| EX-9 | Low | Toast DOM nodes — MAX_TOASTS cap prevents real leak but orphaned timeouts remain | `modal.tsx:184-217` |
| EX-10 | Low | token-sync.ts monkey-patches produce unhandled rejections after extension uninstall | `token-sync.ts:55-74` |

### Promise/Async Issues

| # | Sev | Issue | File |
|---|-----|-------|------|
| EX-12 | Medium | `.parse()` throws verbose ZodError shown to user (should use `.safeParse()`) | `background.ts:208, 488` |
| EX-11 | Low | Silent error swallowing in telemetry queue `.catch(() => {})` | `scraper.ts:560-568` |
| EX-13 | Low | `tabs.query()` promise has no `.catch()` | `background.ts:896-909` |

### UI/UX Bugs

| # | Sev | Issue | File |
|---|-----|-------|------|
| EX-14 | Medium | Stale balance displayed after exchange switch on fetch failure | `MainView.tsx:96-101` |
| EX-20 | Medium | ActiveOrders re-fetches on every WS update — no debounce (unlike MainView's 250ms) | `ActiveOrders.tsx:59-64` |
| EX-21 | Medium | AuthContext.checkAuth has no error handling — popup hangs on background crash | `AuthContext.tsx:19-27` |
| EX-15 | Low | QuickTrade `enterCount` not reset on error — safety guard bypassed | `QuickTrade.tsx:17, 75-118` |
| EX-16 | Low | `cancelTrade` missing 401 retry logic (inconsistent with all other API functions) | `background.ts:433-466` |
| EX-23 | Low | ExchangeSelector stale after account deletion in sibling component | `ExchangeSelector.tsx:62-67` |

### Content Script & Modal

| # | Sev | Issue | File |
|---|-----|-------|------|
| EX-18 | Medium | TradeForm keydown listener on host document — fragile with TradingView capture handlers | `TradeForm.tsx:137-142` |
| EX-17 | Low | Alt+X keydown listener orphaned on extension reload — can fire twice | `content.ts:36-105` |
| EX-19 | Low | content.ts missing return for WS_ORDER_UPDATE message type | `content.ts:146-163` |
| EX-22 | Low | StatusBar and HeaderBar both register listeners for same messages | `StatusBar.tsx / HeaderBar.tsx` |
| EX-24 | Low | JWT decoded without signature verification (acceptable client-side) | `background.ts:273` |

---

## 4. Launch Readiness

### P0 — Must Fix Before Launch

| # | Issue | Category | Effort |
|---|-------|----------|--------|
| 1 | EXT-25: Live fill subscription reliability (draft spec, 0% code) | Feature | 3 days |
| 2 | Per-user trade execution lock (RC-1) | Safety | 0.5 day |
| 3 | Atomic SL+TP placement with rollback (RC-2) | Safety | 1 day |
| 4 | CORS: Replace wildcard `*` with allowlist | Security | 30 min |
| 5 | K8s liveness/readiness probes | Infrastructure | 2 hours |
| 6 | CI/CD pipeline (no automated testing or deployment) | Infrastructure | 3-5 days |
| 7 | Silent error handlers in extension (15+ `.catch(() => {})`) | Reliability | 1 day |

### P1 — Should Fix Before Launch

| # | Issue | Category | Effort |
|---|-------|----------|--------|
| 8 | Shadow Engine GC — prune_terminal() for ML-1/2/3 | Memory | 1 day |
| 9 | Structured logging + correlation IDs | Observability | 2 days |
| 10 | Database backups | Infrastructure | 1 day |
| 11 | Prometheus metrics + Grafana dashboards | Observability | 2 days |
| 12 | Graceful shutdown coordination — CancellationToken (ML-6/13) | Reliability | 1 day |
| 13 | Encryption key must fail-fast, not fallback to ephemeral | Security | 30 min |

### P2 — Should Fix Soon After Launch

| # | Issue | Category | Effort |
|---|-------|----------|--------|
| 14 | Idempotency tokens on trade creation | Safety | 4 hours |
| 15 | Password reset flow (referenced but not implemented) | UX | 2 hours |

### What IS Working Well

- **Core trading engine:** 709 Rust tests + 43 sidecar tests passing
- **Spec-first development:** 16 specs completed with documentation trail
- **Extension UX:** Shadow DOM isolation, double-Enter safety, Zod validation, 3-strategy scraper
- **Security baseline:** AES-256-GCM encryption, JWT auth, bcrypt, rate limiting, security headers
- **Multi-exchange:** WOO and Binance Futures verified working via CCXT sidecar

---

## 5. Commit History Insights

### Hotspot Files (Highest Churn)

| File | Commits | Risk | Pattern |
|------|---------|------|---------|
| `background.ts` (1053 lines) | 22 | Critical | Monolithic: auth + WS + REST + exchange + trade execution |
| `trade_management.rs` (2247 lines) | 50+ | High | Precision fixes, response formatting, state sync |
| `engine.rs` | 22 | High | Orderbook logic, fill detection refinements |
| `modal.tsx` | 15 | High | Shadow DOM isolation, form validation iterations |
| `ws-orders.js` | Multiple | High | NotSupported handling, reconnection patches |

### Recurring Bug Patterns

1. **Exchange Compatibility (80/20 Problem):** CCXT/sidecar assumes response shape, exchange returns different shape, silent failure, later patched. Each new exchange reveals 3-5 undiscovered assumptions. **Need:** Response normalization adapter layer.

2. **State Sync & Rehydration:** Features work in isolation but break on restart/disconnect. Ghost positions (spec 016), symbol format mismatches (`BTC_USDT` vs `BTC/USDT:USDT`). **Need:** Comprehensive startup verification.

3. **Numeric Precision:** Multiple commits fixing Decimal/float/string conversions at extension-backend-CCXT boundaries. 8-decimal rounding, quantity sign, margin/leverage calculations.

4. **Zod SafeParse Inconsistency:** Some paths use `.parse()` (throws), some use `.safeParse()` (returns). Pattern not standardized across codebase.

### Velocity Note

25 of the last 100 commits are bug fixes (25% fix rate). Rapid feature delivery is outpacing quality — a common pre-launch pattern that needs rebalancing.

---

## 6. Prioritized Action Plan

### Phase 1: Safety-Critical (Week 1-2)

- [ ] **Trade safety:** Per-user lock + atomic SL/TP rollback (RC-1, RC-2)
- [ ] **CORS fix** + encryption key fail-fast (30 min each)
- [ ] **EXT-25:** Wire WsSubscriptionManager, fix abort() race (RC-6), add fill retry (RC-4)
- [ ] **Extension error handling:** Remove silent `.catch(() => {})`, fix Zod `.parse()`→`.safeParse()`, add `unhandledRejection` handler
- [ ] **Logout cleanup:** Clear refreshTimer, disconnect WebSocket, clear balance state

### Phase 2: Reliability (Week 2-3)

- [ ] **Shadow Engine GC:** `prune_terminal()` for orders, positions, groups (ML-1/2/3)
- [ ] **Graceful shutdown:** CancellationToken for all background tasks (ML-6/13)
- [ ] **Structured logging + Prometheus + Grafana**
- [ ] **K8s hardening:** Probes, resource limits, PDBs, DB backups
- [ ] **CI/CD pipeline** (GitHub Actions)

### Phase 3: Hardening (Week 4-5)

- [ ] **Extension UX fixes:** Debounce ActiveOrders, stale balance, enterCount reset, cancelTrade retry
- [ ] **Remaining race conditions:** TOCTOU in update_stop_loss (RC-3), CcxtClient lifecycle (ML-5)
- [ ] **Load testing + chaos engineering**

### Pre-Launch Go/No-Go Checklist

- [ ] Per-user trade lock implemented
- [ ] Atomic SL/TP rollback on failure
- [ ] EXT-25 implemented, all tests passing
- [ ] CI/CD pipeline running on all commits
- [ ] K8s liveness/readiness probes configured
- [ ] Structured logging with correlation IDs
- [ ] Prometheus metrics exported, Grafana dashboards created
- [ ] CORS restricted to known origins
- [ ] Database backups automated and tested
- [ ] Silent error handlers removed
- [ ] Shadow Engine GC implemented
- [ ] Graceful shutdown with CancellationToken
- [ ] Load test: p99 < 500ms under 1000 concurrent users
- [ ] `cargo audit` + `npm audit` clean
- [ ] All 752+ tests passing
- [ ] Incident response plan documented

---

*Generated by 5 parallel investigation agents — Mar 7, 2026*
