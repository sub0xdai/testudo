# Plan: Wiring the Router to the Engine (The "Decision Loop")

## Overview
This plan details the steps to implement the critical "Decision Loop" where the **Router** (API Gateway) requests permission from the **Engine** (Risk & Strategy Layer) before executing any trade on an external exchange.

## The Loop Architecture
1.  **Ingress:** Router receives `POST /order`.
2.  **Request:** Router publishes a `RiskCheckRequest` to Redis.
3.  **Process:** Engine (listening on Redis) consumes the request, validates against the "Shadow Ledger" (Balances, Limits, Strategy).
4.  **Reply:** Engine publishes a `RiskCheckResponse` back to a unique reply channel.
5.  **Action:** 
    *   **Approved:** Router forwards the order to the External Exchange (CCXT).
    *   **Rejected:** Router returns `400 Bad Request` to the user immediately.

## Implementation Steps

### Phase 1: Shared Definitions (`crates/common_utils`)
- [ ] Define `RiskCheckRequest` struct.
    - Fields: `user_id`, `symbol`, `side`, `quantity`, `price`, `request_id`.
- [ ] Define `RiskCheckResponse` struct.
    - Fields: `request_id`, `approved` (bool), `rejection_reason` (Option<String>), `shadow_balance_remaining`.
- [ ] Define Redis Channel constants (e.g., `RISK_CHECK_CHANNEL`).

### Phase 2: Engine Implementation (`crates/engine`)
- [ ] Create a new `RiskHandler` component in the Engine.
- [ ] Subscribe to `RISK_CHECK_CHANNEL` on startup.
- [ ] Implement the validation logic:
    - [ ] **Parse:** Deserialize the request.
    - [ ] **Lock:** Acquire lock on `UserBalances`.
    - [ ] **Check:** 
        - Does user exist?
        - Is `available_balance >= (price * quantity)`?
        - Are there position limits?
    - [ ] **Reserve:** If approved, *tentatively* lock the funds (Shadow Execution).
- [ ] Publish the result to `router:response:{request_id}`.

### Phase 3: Router Implementation (`crates/router`)
- [ ] Modify `execute_order` in `crates/router/src/routes/order.rs`.
- [ ] **Publish:** Serialize the order into `RiskCheckRequest` and publish to `RISK_CHECK_CHANNEL`.
- [ ] **Await:** Subscribe to `router:response:{request_id}` and wait (with a 500ms timeout).
    - *Note:* Use a temporary subscription or a dedicated response listener task to avoid overhead.
- [ ] **Handle Response:**
    - If `Approved`: Proceed to `ExchangeRouter` (mock or real).
    - If `Rejected`: Return JSON error to client.
    - If `Timeout`: Return "Engine unavailable" error (fail-safe).

### Phase 4: Integration Testing
- [ ] Start Redis, Engine, and Router.
- [ ] Send `POST /order` with sufficient funds -> Expect 200 OK + "Approved".
- [ ] Send `POST /order` with insufficient funds -> Expect 400 Bad Request + "Rejected".
