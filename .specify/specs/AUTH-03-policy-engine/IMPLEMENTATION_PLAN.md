# AUTH-03-policy-engine — Implementation Plan

## Current State Summary

The permission system lives in two places:

1. **`models/agent_key.rs`** — `AgentPermission` enum with 6 flat variants (`TradeExecute`, `JournalRead`, `JournalWrite`, `ExchangeManage`, `RiskConfigure`, `AccountRead`). Agent keys store a `Vec<AgentPermission>` in a JSONB column. `default_agent_permissions()` returns 4 scopes.

2. **`middleware/auth.rs`** — `AuthenticatedUser` has `has_permission(perm)` (SIWE = true, AgentKey = `permissions.contains(perm)`) and `require_permission(perm)` (returns 403 if denied). Routes call `user.require_permission(&AgentPermission::TradeExecute)` etc.

3. **`utils/auth_helpers.rs`** — `AuthContext` wraps `AuthenticatedUser` for identity checks (`authorize_user_id`, `parse_resource_id`). Used only by `routes/order.rs`. This is **identity validation, not permission checking** — it verifies the caller owns the resource, not whether the caller is authorized to perform an action. **Keep this module.**

4. **`services/agent_key.rs`** — resolves `X-Agent-Key` header → `AgentKeyClaims`. Deserializes `permissions` JSONB column into `Vec<AgentPermission>`. Also updates `last_used_at`.

5. **Routes using `require_permission`**:
   - `routes/signal.rs:81` — `TradeExecute`
   - `routes/agent_journal.rs:32,77,108` — `JournalRead`
   - `routes/risk_config.rs:111` — `RiskConfigure`
   - `routes/agent_keys.rs` — uses `AgentPermission` type in CRUD operations (create/list/update)

6. **Identified gap**: `routes/exchanges.rs` handles exchange account management (add, delete, test connection, agent approval, balance) but has **no permission check** — any authenticated user can call these endpoints. The spec says `ExchangeManage { exchanges: Option<Vec<String>> }` should gate these. This is a missing enforcement point not covered by the current system.

The existing system has zero parameterization — you either have a scope or you don't. No way to say "trade only BTC_USDT on Binance" or "max $500 per trade." All enforcement lives in route handlers as one-line `require_permission` calls.

---

## Checkpoints

### CP-1: Permission Enum + Policy Engine + Backward-Compat Deserialization ✅

Completed 2026-06-06 by /skill:vox build.

**Touches**: 1 new file (`policy/mod.rs`), `main.rs` (+module), `models/agent_key.rs` (type migration), `middleware/auth.rs` (sig update), `services/agent_key.rs` (type update), `routes/signal.rs`, `routes/agent_journal.rs`, `routes/risk_config.rs`, `routes/agent_keys.rs` (type ref changes)

**No wiring to routes yet.** Pure logic. Old system still works.

**Tasks**:
1. Add `pub mod policy;` to `crates/router/src/lib.rs` (or wherever modules are declared)
2. Create `crates/router/src/policy/mod.rs` with:
   - `Permission` enum (parameterized variants per spec)
   - `Action` enum (Trade, JournalRead, JournalWrite, ExchangeManage, RiskConfigure, AccountRead)
   - `ActionContext<'a>` struct
   - `PolicyEngine` struct with `authorize(user, action, ctx) -> Result<(), PolicyError>`
   - `PolicyError` enum (typed, one variant per denial reason)
   - `default_permissions()` function
   - Custom `serde::Deserialize` for `Permission` — accepts both old flat strings (`"trade_execute"`) and new parameterized objects (`{"scope": "trade_execute", "symbols": [...]}`). Old flat strings deserialize as unparameterized (all `None`).
   - `#[cfg(test)]` module covering:
     - SIWE user passes all actions
     - Agent with no TradeExecute fails Trade
     - Agent with symbol constraint: pass for allowed symbol, fail for disallowed
     - Agent with exchange constraint: pass for allowed exchange, fail for disallowed
     - Agent with risk limit: pass under limit, fail over
     - Agent with position limit: pass under limit, fail at/over
     - Agent with tag constraint: pass for allowed tag, fail for disallowed
     - Agent with all-None params passes all checks (backward compat)
     - Binary scopes (RiskConfigure, AccountRead): pass if present, fail if absent
     - Backward-compat deserialization: `"trade_execute"` → `Permission::TradeExecute { symbols: None, exchanges: None, max_risk_per_trade: None, max_open_positions: None }`
     - Backward-compat deserialization: `["trade_execute", "journal_read"]` → Vec of unparameterized permissions
3. In `models/agent_key.rs`:
   - **Keep** the old `AgentPermission` enum for now (CP-2 removes it)
   - Add the new `Permission` enum alongside it (moved to `policy/mod.rs` in final state)
   - Update `default_agent_permissions()` → returns new `Vec<Permission>` using `default_permissions()` from policy module
   - Update structs (`AgentKeyClaims`, `CreateAgentKeyRequest`, `UpdateAgentKeyRequest`, `AgentKeySummary`, `CreateAgentKeyResponse`) to use `Vec<Permission>` instead of `Vec<AgentPermission>`

**Wait — this is a mistake.** We can't have two enums. CP-1 should just create the policy module with Permission and have agent_key.rs import from policy. Let me restructure.

**Revised CP-1 tasks**:
1. Create `crates/router/src/policy/mod.rs` with full Permission enum, Action, ActionContext, PolicyEngine, PolicyError, default_permissions, backward-compat deserializer, all unit tests
2. Register `pub mod policy;` in `crates/router/src/lib.rs` (or appropriate parent module)
3. In `models/agent_key.rs`: replace `AgentPermission` enum with `pub use crate::policy::Permission;` re-export. Update all structs that referenced `AgentPermission` → `Permission`. Remove `default_agent_permissions()` (now `policy::default_permissions()`).
4. In `services/agent_key.rs`: update `Vec<AgentPermission>` → `Vec<Permission>` in `resolve_agent_key`

**At this point compilation WILL break** because `middleware/auth.rs`, `routes/signal.rs`, etc. still reference `AgentPermission`. That's intentional — CP-1 is the type migration, CP-2 wires routes.

**Verification**: `cargo test -p router -- policy` passes all engine tests. `cargo build -p router` **will fail** (routes broken). This is acceptable — CP-1 is the foundation, CP-2 completes the build.

**Commit message**: `refactor(auth): replace AgentPermission with parameterized Permission enum + PolicyEngine`

---

### CP-2: Wire Policy Engine into Routes + Strip Old Methods ✅

Completed 2026-06-06 by /skill:vox build.

**Touches**: `middleware/auth.rs`, `routes/signal.rs`, `routes/agent_journal.rs`, `routes/risk_config.rs`, `routes/exchanges.rs`, `routes/agent_keys.rs`, `services/agent_key.rs`

**Makes the whole router compile and pass tests.**

**Tasks**:
1. **`middleware/auth.rs`**:
   - Remove `has_permission()` and `require_permission()` from `AuthenticatedUser`
   - Remove `AgentPermission` import (use `crate::policy::Permission` if needed internally, but auth middleware shouldn't need it)
   - Remove `AgentKeyClaims` `permissions` type change (already done in CP-1 when model changed)
   - `AuthenticatedUser::from_request` — the `AgentKeyClaims` permissions field is now `Vec<Permission>`, no code change needed
   - Keep `AuthMethod` enum (SIWE vs AgentKey distinction still matters)
2. **`routes/signal.rs`**:
   - Replace `user.require_permission(&AgentPermission::TradeExecute)?` with `policy::PolicyEngine::authorize(&user, Action::Trade, &ActionContext { symbol: Some(&input.symbol), exchange: None, risk_amount: Some(position_size), open_position_count: Some(current_positions), ..Default::default() })?`
   - Import `crate::policy::{PolicyEngine, Action, ActionContext, PolicyError}`
   - Remove `AgentPermission` import
3. **`routes/agent_journal.rs`**:
   - Replace 3x `user.require_permission(&AgentPermission::JournalRead)?` with `policy::PolicyEngine::authorize(&user, Action::JournalRead, &ActionContext::default())?`
   - Import `crate::policy::{PolicyEngine, Action, ActionContext}`
   - Remove `AgentPermission` import
4. **`routes/risk_config.rs`**:
   - Replace `user.require_permission(&AgentPermission::RiskConfigure)?` with `policy::PolicyEngine::authorize(&user, Action::RiskConfigure, &ActionContext::default())?`
   - Import `crate::policy::{PolicyEngine, Action, ActionContext}`

5. **`routes/exchanges.rs`**:
   - Add `policy::PolicyEngine::authorize(&user, Action::ExchangeManage, &ActionContext { exchange: Some(&exchange_name), ..Default::default() })?` to endpoints that manage exchange accounts (add, delete, update)
   - Note: this is a **new enforcement point** — exchange management currently has no permission gate
6. **`routes/agent_keys.rs`**:
   - Update `create_key` to serialize new `Vec<Permission>` (already handled by serde if Permission is the type)
   - Update `list_keys` deserialization: already uses `serde_json::from_value`, which now uses the custom deserializer from CP-1
   - Update `update_key` to accept new permission schema
7. **`services/agent_key.rs`**:
   - `resolve_agent_key`: deserialization line `Vec<AgentPermission>` → `Vec<Permission>` (already done in CP-1)

**Verification**: `cd testudo-exchange && cargo clippy --all-targets && cargo test`

All existing tests must pass. The signal route tests exercise the full trade path through the policy engine. Agent journal tests exercise JournalRead checks. Risk config tests exercise RiskConfigure.

**Commit message**: `refactor(auth): wire policy engine into routes, remove legacy has_permission`

---

## Risks & Open Questions

1. **`AuthContext` in `order.rs`**: The spec says "remove `utils/auth_helpers.rs`." But `AuthContext` provides identity validation (`authorize_user_id`, `parse_resource_id`), not permission checking. These are separate concerns. **Decision: keep `AuthContext`.** It is used by `routes/order.rs` and is not related to the agent key permission engine.

2. **Exchange management permissions**: `routes/exchanges.rs` currently has no permission enforcement. Adding `ExchangeManage` check is a new enforcement point that should be explicitly called out. Existing tests for exchange routes will need the user to have `ExchangeManage` permission (or be SIWE, which is what test users likely are).

3. **Backward-compat custom deserializer**: The `Permission` enum uses `#[serde(tag = "scope")]` which expects objects. A custom `Deserialize` impl must handle both `"trade_execute"` (old string) and `{"scope": "trade_execute", ...}` (new object). Implementing this correctly is the main complexity in CP-1.

4. **`ActionContext` has a `tag` field but `JournalRead`/`JournalWrite` permissions have `tags` (plural, a Vec)**. The policy engine checks `ctx.tag ∈ perm.tags` when `perm.tags` is `Some`. This is correct — we check one tag at a time. If a route needs to check multiple tags, it calls `authorize` for each.

5. **CP-1 deliberately breaks the build**: The plan acknowledges that after CP-1, `cargo build` fails because routes still reference `AgentPermission`. CP-2 resolves this. For CP-1 verification, we only run `cargo test -p router -- policy`.

---

## Summary

**Plan ready**: 2 checkpoints, ~4-6 hours total.

- **CP-1**: New `policy` module with Permission enum, PolicyEngine, backward-compat deserializer. Pure logic, unit tested.
- **CP-2**: Wire engine into all route handlers, strip old `has_permission`/`require_permission`. Full integration.

Run `/skill:vox build AUTH-03-policy-engine` to start CP-1.
