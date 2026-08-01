# AUTH-03: Centralized Policy Engine

> **Status**: Design locked — ready for `/skill:vox plan`
> **Date**: 2026-06-06
> **Parent**: AGENT-07-agent-api-keys (scoped agent keys)

---

## Motivation

The current permission system (`has_permission` / `require_permission` with `contains()`) works for 6 flat scopes but will not scale to parameterized constraints. Adding resource-level scoping ("only trade on Binance", "only BTC_USDT") or numeric limits ("max $500 per trade") would require copying conditionals into every route handler.

**Goal**: One centralized evaluation point per action. Route handlers call a single `authorize()` function. All policy logic lives in one module.

**Non-goals** (explicitly out of scope):
- Multi-human delegation or team/org accounts
- Time-based or compound conditions ("trade only 08:00-20:00 UTC AND BTC_USDT")
- Policy-as-data (no DSL, no Cedar/Oso, no runtime policy editing by users)
- Changing the journal or extension frontends — they remain binary authenticated/not
- Changing SIWE humans — they keep the `allow *` fast path

---

## Architectural Principle

| Layer | What it does |
|-------|-------------|
| **Permission storage** (on the agent key) | What this key *is allowed to do* — parameterized scopes |
| **Policy engine** (`crates/router/src/policy/mod.rs`) | Takes `(user, action, context)` → `Result`. All evaluation logic in one file. |
| **Route handlers** | Call `policy::authorize(user, action, ctx)?` — one line, no conditionals |

---

## Data Model

### Permission (replaces `AgentPermission` enum)

```rust
/// Permission scopes for agent API keys.
/// Each variant carries optional constraints — None means unrestricted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum Permission {
    /// Place trades (shadow or live)
    TradeExecute {
        /// None = all symbols allowed
        symbols: Option<Vec<String>>,
        /// None = all exchanges allowed
        exchanges: Option<Vec<String>>,
        /// Max risk per trade in USDT. None = no limit.
        max_risk_per_trade: Option<Decimal>,
        /// Max concurrent open positions. None = no limit.
        max_open_positions: Option<u32>,
    },

    /// Read journal data
    JournalRead {
        /// None = all tags visible
        tags: Option<Vec<String>>,
    },

    /// Write journal entries
    JournalWrite {
        /// None = can write any tag
        tags: Option<Vec<String>>,
    },

    /// Manage exchange accounts
    ExchangeManage {
        /// None = all exchanges
        exchanges: Option<Vec<String>>,
    },

    /// Modify risk configuration (binary — no parameters)
    RiskConfigure,

    /// Read account info (binary — no parameters)
    AccountRead,
}
```

**Design rules**:
- `None` = unrestricted (no wildcard tokens, no `*` strings)
- Flat, not nested — one variant per action type, all constraints inline
- Closed vocabulary — no regex or glob patterns on symbol/exchange/tag matching

### Default permissions (for new agent keys)

```rust
pub fn default_permissions() -> Vec<Permission> {
    vec![
        Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: None,
        },
        Permission::JournalRead { tags: None },
        Permission::JournalWrite { tags: None },
        Permission::AccountRead,
    ]
}
```

---

## Policy Engine

### File: `crates/router/src/policy/mod.rs`

### Action enum

```rust
/// What the caller is trying to do.
pub enum Action {
    Trade,
    JournalRead,
    JournalWrite,
    ExchangeManage,
    RiskConfigure,
    AccountRead,
}
```

### Action context

```rust
/// What the caller is operating on.
/// Fields left as None are skipped during evaluation.
pub struct ActionContext<'a> {
    pub symbol: Option<&'a str>,
    pub exchange: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub risk_amount: Option<Decimal>,
    pub open_position_count: Option<u32>,
}
```

### Engine entry point

```rust
pub struct PolicyEngine;

impl PolicyEngine {
    /// Returns Ok if authorized, Err(PolicyError) with reason if denied.
    /// SIWE users always pass — only AgentKey auth_method hits evaluation.
    pub fn authorize(
        user: &AuthenticatedUser,
        action: Action,
        ctx: &ActionContext,
    ) -> Result<(), PolicyError>;
}
```

### Evaluation rules per action

```
Action::Trade →
    1. Find TradeExecute permission on key. If absent → deny
    2. If perm.symbols is Some → ctx.symbol must be in it
    3. If perm.exchanges is Some → ctx.exchange must be in it
    4. If perm.max_risk_per_trade is Some → ctx.risk_amount ≤ it
    5. If perm.max_open_positions is Some → ctx.open_position_count < it

Action::JournalRead →
    1. Find JournalRead permission on key. If absent → deny
    2. If perm.tags is Some → ctx.tag must be in it

Action::JournalWrite →
    1. Find JournalWrite permission on key. If absent → deny
    2. If perm.tags is Some → ctx.tag must be in it

Action::ExchangeManage →
    1. Find ExchangeManage permission on key. If absent → deny
    2. If perm.exchanges is Some → ctx.exchange must be in it

Action::RiskConfigure →
    RiskConfigure permission present? → allow

Action::AccountRead →
    AccountRead permission present? → allow
```

### PolicyError

```rust
pub enum PolicyError {
    MissingScope { required: &'static str },
    SymbolNotAllowed { symbol: String, allowed: Vec<String> },
    ExchangeNotAllowed { exchange: String, allowed: Vec<String> },
    RiskLimitExceeded { requested: Decimal, max: Decimal },
    MaxPositionsExceeded { current: u32, max: u32 },
    TagNotAllowed { tag: String, allowed: Vec<String> },
}
```

Each variant carries enough data for the route handler to return a specific 403 body.

---

## Integration

### Changes to existing files

| File | Change |
|------|--------|
| `models/agent_key.rs` | Replace `AgentPermission` enum with new `Permission` enum + `default_permissions()` |
| `middleware/auth.rs` | Remove `has_permission()` and `require_permission()` from `AuthenticatedUser`. Keep identity fields only. |
| `utils/auth_helpers.rs` | Remove (superseded by policy engine) |
| `routes/signal.rs` | Replace `user.require_permission(&AgentPermission::TradeExecute)?` with `policy::authorize(&user, Action::Trade, &ctx)?` |
| `routes/agent_journal.rs` | Same pattern for `JournalRead` |
| `routes/risk_config.rs` | Same pattern for `RiskConfigure` |
| `routes/agent_keys.rs` | Update create/list to use new `Permission` schema |
| `routes/exchanges.rs` | Add policy check for `ExchangeManage` where applicable |
| `services/agent_key.rs` | Update resolution to deserialize new permission format |

### New file

| File | Purpose |
|------|---------|
| `crates/router/src/policy/mod.rs` | Policy engine — all authorization evaluation logic |

### No changes

- JWT middleware (dual extraction: Bearer + cookie)
- Agent key resolution in `X-Agent-Key` header
- SIWE/SIWS auth flows
- `AgentKeyClaims` struct structure (just carries `Vec<Permission>`)
- `agent_keys` table schema (permissions column is JSONB — schema change within it, no migration)
- Journal, extension, sidecar — zero changes

---

## Route handler usage pattern

**Before** (current):
```rust
user.require_permission(&AgentPermission::TradeExecute)?;
```

**After** (AUTH-03):
```rust
policy::authorize(&user, Action::Trade, &ActionContext {
    symbol: Some(&input.symbol),
    exchange: Some(&account.exchange_name),
    risk_amount: Some(position_size),
    open_position_count: Some(current_positions),
})?;
```

The handler passes whatever context it has. Fields left as `None` are skipped by the engine.

---

## API Changes

### POST /api/v1/agent-keys — request body

**Before**:
```json
{
  "name": "momentum-bot",
  "permissions": ["trade_execute", "journal_read", "account_read"]
}
```

**After**:
```json
{
  "name": "momentum-bot",
  "permissions": [
    {
      "scope": "trade_execute",
      "symbols": ["BTC_USDT", "ETH_USDT"],
      "exchanges": ["binance"],
      "max_risk_per_trade": "500",
      "max_open_positions": 3
    },
    {
      "scope": "journal_read",
      "tags": ["#momentum", "#breakout"]
    },
    { "scope": "account_read" }
  ]
}
```

### GET /api/v1/agent-keys — response

Updated to include parameterized permission details in each `AgentKeySummary`.

### PUT /api/v1/agent-keys/:id — request body

Updated to accept the new permission schema for partial updates.

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

Policy engine tests (in `policy/mod.rs`):
- SIWE user passes all actions
- Agent with no TradeExecute permission fails Trade
- Agent with symbol constraint fails Trade for unlisted symbol
- Agent with exchange constraint fails Trade for unlisted exchange
- Agent with risk limit fails Trade when exceeding it
- Agent with position limit fails Trade when at max
- Agent with tag constraint fails JournalRead for unlisted tag
- Agent with no params (all None) passes all checks (backward compatibility)

---

## Migration Notes

- `AgentPermission` enum is replaced by `Permission` — a breaking compile-time change that the compiler catches at every usage site
- Existing agent keys stored in the DB have flat permission arrays like `["trade_execute", "journal_read"]`. A deserialization fallback treats bare string scopes as unparameterized permissions (all `None`). No DB migration needed.
- Default key creation switches to the new schema immediately
