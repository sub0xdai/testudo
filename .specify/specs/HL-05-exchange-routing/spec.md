# Specification: Exchange Routing & Dependency Injection

**Spec ID:** HL-05-exchange-routing
**Date:** 2026-03-16
**Status:** Draft
**Class:** Feature / Architecture
**Priority:** P1 — integrates all HL components
**Depends on:** HL-03 (exchange_api), HL-04 (ws_fills)
**Series:** HL-01 through HL-06 (native Hyperliquid integration)

---

## Problem Statement

The current DI in `main.rs` creates a single `CexExchangeApi` for all live trading. We need to route Hyperliquid accounts to `HyperliquidExchangeApi` and all others to `CexExchangeApi`, transparently behind the `ExchangeApi` trait. `TradeManagerService` and `FillDetectorService` must remain unchanged.

---

## User Stories

- **As a trader**, I want Hyperliquid to route through the native Rust SDK automatically based on my exchange account, so that I get lower latency without any configuration.
- **As a developer**, I want a routing wrapper that delegates based on `exchange_name`, so that existing services don't need modification.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `RoutingExchangeApi` wrapper implementing `ExchangeApi` trait | High | Router |
| FR-2 | On each call: load exchange account → check `exchange_name` → delegate | High | Router |
| FR-3 | `"hyperliquid"` → delegate to `HyperliquidExchangeApi` | High | Router |
| FR-4 | Everything else → delegate to `CexExchangeApi` | High | Router |
| FR-5 | Extend `WsSubscriptionManager` to detect `exchange_name == "hyperliquid"` | High | Router |
| FR-6 | For HL accounts: spawn `HyperliquidFillSubscriber` instead of sidecar WS | High | Router |
| FR-7 | All events feed into same `mpsc::Sender<OrderUpdateEvent>` channel | High | Router |
| FR-8 | `main.rs`: when `HYPERLIQUID_ENABLED=true`, create both APIs and wrap in `RoutingExchangeApi` | High | Router |
| FR-9 | `TradeManagerService` receives `RoutingExchangeApi` as `Arc<dyn ExchangeApi>` — no changes needed | High | Router |

---

## Technical Implementation

### RoutingExchangeApi

```rust
pub struct RoutingExchangeApi {
    cex_api: Arc<CexExchangeApi>,
    hl_api: Arc<HyperliquidExchangeApi>,
    account_repo: ExchangeAccountRepository,
}

#[async_trait]
impl ExchangeApi for RoutingExchangeApi {
    async fn get_balance(&self, user_id, asset, exchange_account_id) -> Result<Decimal> {
        let account = self.load_account(user_id, exchange_account_id).await?;
        match account.exchange_name.as_str() {
            "hyperliquid" => self.hl_api.get_balance(user_id, asset, exchange_account_id).await,
            _ => self.cex_api.get_balance(user_id, asset, exchange_account_id).await,
        }
    }
    // ... same pattern for all 6 methods
}
```

### main.rs Changes

```rust
// When CCXT_ENABLED=true AND HYPERLIQUID_ENABLED=true:
let hl_exchange_api = Arc::new(HyperliquidExchangeApi::new(...));
let routing_api = Arc::new(RoutingExchangeApi::new(
    cex_exchange_api.clone(),
    hl_exchange_api.clone(),
    exchange_account_repo.clone(),
));
// Pass routing_api as Arc<dyn ExchangeApi> to TradeManagerService
```

### WS Subscription Routing

- In `WsSubscriptionManager::run_subscription_task()`:
  - Load credentials → check `exchange_name`
  - `"hyperliquid"` → spawn `HyperliquidFillSubscriber`
  - Everything else → existing sidecar WS path

### Files

- `crates/router/src/services/hyperliquid/routing.rs` — RoutingExchangeApi
- `crates/router/src/main.rs` — conditional DI
- `crates/router/src/services/ws_subscription_manager.rs` — HL detection
- Update `crates/router/src/services/hyperliquid/mod.rs`

---

## Acceptance Criteria

- [ ] `RoutingExchangeApi` delegates to correct backend based on `exchange_name`
- [ ] `"hyperliquid"` routes to `HyperliquidExchangeApi`
- [ ] All other exchange names route to `CexExchangeApi`
- [ ] `TradeManagerService` unchanged — receives `Arc<dyn ExchangeApi>`
- [ ] `FillDetectorService` unchanged — consumes same `OrderUpdateEvent` channel
- [ ] WS subscription manager spawns HL fill subscriber for hyperliquid accounts
- [ ] `main.rs` conditionally creates routing based on `HYPERLIQUID_ENABLED` env var
- [ ] Unit tests verify routing to correct backend
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Account loading on every call** — adds a DB query per operation. Mitigation: results are already cached in `ExchangeAccountRepository`.
2. **Mixed-exchange users** — users may have both HL and sidecar accounts. The routing handles this per-call via `exchange_account_id`.

---

## Completion Signal

This spec is complete when:
1. RoutingExchangeApi transparently delegates per-exchange
2. WS subscription manager routes HL accounts natively
3. main.rs DI wires everything conditionally
4. All existing tests continue passing
5. Code committed to master
