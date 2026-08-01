# Specification: Strategy Registry + Risk Pre-Check + Init

**Spec ID:** CLI-04-strategy-registry
**Date:** 2026-05-31
**Status:** Draft
**Class:** Feature / Application
**Priority:** P1 — strategies give the agent personality; risk pre-check saves LLM calls; init makes the harness self-bootstrapping
**Depends on:** CLI-03-agent-loop (agent loop, tools), CLI-02-api-client (API client for init flow)
**Series:** CLI-04 (Strategy + Risk + Init)

---

## Problem Statement

The agent loop works but it's generic — the LLM has no strategy-specific guidance beyond `AGENT_TRADING.md`. There's no way to load, validate, or switch strategies. Every signal goes to the backend for risk validation, wasting LLM calls and API bandwidth on signals that are obviously over-leveraged. And the user still has to manually edit `~/.config/tudo/config.toml` to set their agent key — there's no guided onboarding.

This spec adds strategy management, client-side risk pre-check, and the `tudo init` onboarding wizard. After this spec, the harness can run multiple strategy personalities, reject dumb signals before they hit the network, and bootstrap itself from scratch.

---

## User Stories

- **As a strategy developer**, I run `tudo strategy add my-strat --from ./my-strat.toml` and the strategy is registered and validated, so that I can deploy strategies without modifying the harness.
- **As a trader**, I run `tudo agent start --strategy momentum-breakout` and the harness loads the strategy's system prompt, constraints, and tool allowlist, so that the agent behaves differently for each strategy.
- **As a risk-conscious user**, I want the harness to check leverage, drawdown, and max positions client-side before submitting a signal, so that I don't burn API credits on rejected trades.
- **As a new user**, I run `tudo init` and the harness guides me through SIWE auth, exchange connection, risk config, and agent key creation in a single TUI flow, so that I don't need to read API docs.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Strategy registry: `StrategyRegistry` loads `.toml` files from `~/.config/tudo/strategies/` and `strategies/builtins/` (compiled-in). Strategies have `[meta]`, `[loop]`, `[prompt]`, `[parameters]`, `[constraints]`, `[allowed_tools]` sections. | High | Registry |
| FR-2 | 3 built-in strategies: `mean-reversion` (Bollinger Bands + RSI), `momentum-breakout` (volume + price levels), `funding-arb` (funding rate differential). Each TOML file compiled into the binary via `include_str!`. | High | Registry |
| FR-3 | `tudo strategy list` prints a table of registered strategies (name, version, description, source path). | Medium | CLI |
| FR-4 | `tudo strategy add <name> --from <path>` validates the TOML (all required sections present), copies to `~/.config/tudo/strategies/<name>.toml`, and registers it. | Medium | CLI |
| FR-5 | `tudo strategy show <name>` prints the strategy's system prompt, constraints, and parameters. | Medium | CLI |
| FR-6 | `tudo strategy remove <name>` deletes from `~/.config/tudo/strategies/` (only user-installed strategies, not builtins). | Low | CLI |
| FR-7 | `tudo agent start --strategy <name>` uses the strategy's `[prompt].system` as the LLM system prompt, its `[loop]` config for intervals, and its `[allowed_tools]` to filter tools. | High | Loop |
| FR-8 | Client-side risk pre-check: before `submit_signal` calls the API, validate: (a) leverage ≤ max_leverage from strategy constraints, (b) notional ≤ max_position_notional if set, (c) drawdown not exceeded (tracked locally), (d) max open positions not exceeded. Reject with structured error → LLM sees it as tool result. | High | Risk |
| FR-9 | `tudo init` onboarding TUI flow: Step 1 — enter Testudo base URL. Step 2 — SIWE auth (or paste agent key if already have one). Step 3 — select exchange (list from `GET /onboarding/status`). Step 4 — configure risk (leverage, account risk %, drawdown limit). Step 5 — save config. Uses `GET /onboarding/status` to determine current state and skip completed steps. | High | Init |
| FR-10 | `cargo clippy && cargo test` passes in `tudo/`. | High | CI |

---

## Technical Implementation

### Crate Structure (additions)

```
tudo/
├── strategies/
│   ├── mod.rs
│   ├── registry.rs        // StrategyRegistry: load, list, add, remove, validate
│   ├── template.rs        // StrategyTemplate struct + TOML parsing
│   └── builtins/
│       ├── mean_reversion.toml
│       ├── momentum_breakout.toml
│       └── funding_arb.toml
├── src/
│   ├── risk/
│   │   ├── mod.rs
│   │   └── precheck.rs    // Client-side risk validation
│   ├── cmd/
│   │   ├── strategy.rs    // strategy list/add/show/remove handlers
│   │   └── init.rs        // tudo init TUI flow
│   ├── view/
│   │   ├── init.rs        // Init TUI screens (5-step wizard)
│   │   └── strategy_list.rs // Strategy list screen
│   ├── model/
│   │   ├── state.rs       // Add risk_cache, strategy_registry to AppState
│   │   └── session.rs     // Session: auth status, exchanges, risk config (moved from state.rs)
│   ├── app.rs             // Wire init and strategy commands
│   └── main.rs            // Wire new commands
```

### Strategy Registry

```rust
// src/strategies/template.rs

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyTemplate {
    pub meta: StrategyMeta,
    pub loop_config: Option<LoopConfigSection>,
    pub prompt: StrategyPrompt,
    pub parameters: Option<HashMap<String, StrategyParam>>,
    pub constraints: Option<StrategyConstraints>,
    pub allowed_tools: Option<AllowedToolsSection>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyMeta {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoopConfigSection {
    pub interval_secs: Option<u64>,
    pub shadow_only: Option<bool>,
    pub max_signals_per_hour: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyPrompt {
    pub system: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyParam {
    #[serde(rename = "type")]
    pub param_type: String,
    pub default: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyConstraints {
    pub max_leverage: Option<u8>,
    pub max_position_notional: Option<f64>,
    pub allowed_symbols: Option<Vec<String>>,
    pub shadow_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AllowedToolsSection {
    pub tools: Vec<String>,
}
```

```rust
// src/strategies/registry.rs

pub struct StrategyRegistry {
    /// Built-in strategies (compiled into the binary).
    builtins: HashMap<String, StrategyTemplate>,

    /// User-installed strategies from ~/.config/tudo/strategies/.
    user_dir: PathBuf,
}

impl StrategyRegistry {
    pub fn new(config_dir: &Path) -> Self {
        let mut builtins = HashMap::new();

        // Compile-time embedded strategies
        for (name, content) in BUILTIN_STRATEGIES {
            match toml::from_str::<StrategyTemplate>(content) {
                Ok(tmpl) => { builtins.insert(name.to_string(), tmpl); }
                Err(e) => { tracing::warn!("Built-in strategy '{}' failed to parse: {}", name, e); }
            }
        }

        Self {
            builtins,
            user_dir: config_dir.join("strategies"),
        }
    }

    /// Get a strategy by name. User strategies override builtins.
    pub fn get(&self, name: &str) -> Option<StrategyTemplate> {
        // Check user directory first (overrides)
        let user_path = self.user_dir.join(format!("{}.toml", name));
        if user_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&user_path) {
                if let Ok(tmpl) = toml::from_str(&content) {
                    return Some(tmpl);
                }
            }
        }
        // Fall back to builtin
        self.builtins.get(name).cloned()
    }

    /// List all available strategies.
    pub fn list(&self) -> Vec<StrategyMeta> {
        let mut metas: Vec<StrategyMeta> = self.builtins.values()
            .map(|s| s.meta.clone())
            .collect();

        // Add user strategies
        if let Ok(entries) = std::fs::read_dir(&self.user_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map_or(false, |e| e == "toml") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(tmpl) = toml::from_str::<StrategyTemplate>(&content) {
                            metas.push(tmpl.meta);
                        }
                    }
                }
            }
        }

        metas
    }

    /// Add a user strategy.
    pub fn add(&self, name: &str, content: &str) -> Result<(), RegistryError> {
        // Validate TOML structure
        let tmpl: StrategyTemplate = toml::from_str(content)
            .map_err(|e| RegistryError::InvalidToml(e.to_string()))?;

        if tmpl.meta.name != name {
            return Err(RegistryError::NameMismatch {
                expected: name.to_string(),
                actual: tmpl.meta.name,
            });
        }

        let path = self.user_dir.join(format!("{}.toml", name));
        std::fs::create_dir_all(&self.user_dir)?;
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Remove a user strategy. Cannot remove builtins.
    pub fn remove(&self, name: &str) -> Result<(), RegistryError> {
        if self.builtins.contains_key(name) {
            return Err(RegistryError::CannotRemoveBuiltin(name.to_string()));
        }

        let path = self.user_dir.join(format!("{}.toml", name));
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        Ok(())
    }
}
```

### Client-Side Risk Pre-Check

```rust
// src/risk/precheck.rs

pub struct RiskPrecheck {
    /// Constraints loaded from the active strategy.
    pub strategy_constraints: StrategyConstraints,

    /// User's risk config (loaded from API or local cache).
    pub user_risk: RiskConfigSummary,

    /// Locally tracked state.
    pub position_count: usize,
    pub max_positions: usize,
}

#[derive(Debug)]
pub struct PrecheckResult {
    pub passed: bool,
    pub reason: Option<String>,
    pub suggestion: Option<String>,
}

impl RiskPrecheck {
    /// Validate a signal before sending it to the backend.
    pub fn validate(&self, signal: &SubmitSignalArgs) -> PrecheckResult {
        // 1. Leverage check
        if let Some(leverage) = signal.leverage {
            let max_lev = self.strategy_constraints.max_leverage
                .unwrap_or(20) as i64;
            if leverage > max_lev {
                return PrecheckResult {
                    passed: false,
                    reason: Some(format!(
                        "Leverage {}× exceeds strategy max of {}×",
                        leverage, max_lev
                    )),
                    suggestion: Some(format!("Reduce leverage to {}× or lower", max_lev)),
                };
            }
        }

        // 2. Max positions check
        if self.position_count >= self.max_positions {
            return PrecheckResult {
                passed: false,
                reason: Some(format!(
                    "Max positions reached ({}/{}). Close a position before opening a new one.",
                    self.position_count, self.max_positions
                )),
                suggestion: Some("Wait for an existing position to close or increase max_positions in strategy config".into()),
            };
        }

        // 3. Drawdown check (local tracking)
        // (Simplified — real implementation tracks P&L from execution reports)

        // 4. Symbol allowed check
        if let Some(ref allowed) = self.strategy_constraints.allowed_symbols {
            if !allowed.contains(&signal.symbol) {
                return PrecheckResult {
                    passed: false,
                    reason: Some(format!(
                        "Symbol '{}' not in strategy's allowed list: {:?}",
                        signal.symbol, allowed
                    )),
                    suggestion: Some("Use an allowed symbol or update the strategy constraints".into()),
                };
            }
        }

        PrecheckResult { passed: true, reason: None, suggestion: None }
    }
}
```

### Init Flow

```rust
// src/cmd/init.rs

pub async fn run_init(config: &Config) -> Result<(), Box<dyn Error>> {
    // The init flow is a TUI wizard with 5 steps.
    // Each step is a screen the user navigates with Enter/Tab/Esc.

    let mut state = InitState::default();

    // Step 1: Base URL
    //   Prompt: "Enter Testudo base URL (e.g. http://localhost:8080/api/v1):"
    //   Default: config.api.base_url

    // Step 2: Authentication
    //   Option A: "Paste your agent key (tudo_sk_...):"
    //   Option B: "⬚ I need to create an agent key — run POST /agent-keys"
    //   If user has no key: call GET /onboarding/status → guide to agent key creation

    // Step 3: Exchange
    //   Call GET /onboarding/status → show available_exchanges
    //   User selects exchange → POST /exchanges/accounts (if needed)
    //   Shows next_step from onboarding status

    // Step 4: Risk Config
    //   Show current risk config from GET /risk-config
    //   User can adjust: leverage, account risk %, drawdown limit, stop-loss required
    //   PUT /risk-config with user's choices

    // Step 5: Save
    //   Write config to ~/.config/tudo/config.toml
    //   Display summary: "Ready to trade! Run 'tudo agent start' to begin."

    Ok(())
}

struct InitState {
    step: InitStep,
    base_url: String,
    agent_key: String,
    selected_exchange: Option<String>,
    risk_leverage: u8,
    risk_account_pct: f64,
    risk_drawdown_pct: f64,
    error: Option<String>,
}

enum InitStep {
    BaseUrl,
    Auth,
    Exchange,
    RiskConfig,
    Summary,
}
```

### Dependencies Added

```toml
# Nothing new — toml and serde already in CLI-01. All types already imported.
```

---

## Checkpoints

### CP-1: Strategy registry + built-in strategies
- **Touches**: `tudo/src/strategies/mod.rs`, `template.rs`, `registry.rs` (NEW), `tudo/strategies/builtins/mean_reversion.toml`, `momentum_breakout.toml`, `funding_arb.toml` (NEW), `tudo/src/main.rs`
- **Tasks**:
  1. Define `StrategyTemplate` struct with all sections (`[meta]`, `[loop]`, `[prompt]`, `[parameters]`, `[constraints]`, `[allowed_tools]`). Implement `Deserialize` for TOML parsing.
  2. Write 3 built-in strategy TOML files:
     - `mean_reversion.toml`: Bollinger Bands (20 SMA, 2σ), RSI(14) confirmation, 2× ATR stop, max 3× leverage.
     - `momentum_breakout.toml`: Volume-based breakout, 20-period high/low channel, 1.5× ATR stop, max 5× leverage.
     - `funding_arb.toml`: Funding rate differential > 0.01%, delta-neutral hedge, max 2× leverage.
  3. Implement `StrategyRegistry`: `new()` loads builtins via `include_str!`, `get(name)` returns `Option<StrategyTemplate>`, `list()` returns `Vec<StrategyMeta>`, `add(name, content)` validates and saves, `remove(name)` deletes.
  4. Embed builtin strategy content: use a lazy_static or `phf` map with `include_str!` for each builtin.
  5. Unit test: registry loads 3 builtins. `get("mean-reversion")` returns correct prompt. `get("nonexistent")` returns None.
- **Verification**: `cargo test -p tudo -- strategies` passes. Registry contains 3 builtins with valid prompts.

### CP-2: Strategy CLI commands
- **Touches**: `tudo/src/cmd/strategy.rs` (NEW), `tudo/src/main.rs`, `tudo/src/view/strategy_list.rs` (NEW)
- **Tasks**:
  1. `tudo strategy list`: prints table with columns Name | Version | Description | Source. Source is "builtin" or "~/.config/tudo/strategies/<name>.toml".
  2. `tudo strategy add <name> --from <path>`: reads file, validates TOML (try `toml::from_str::<StrategyTemplate>()`), copies to user dir, prints confirmation.
  3. `tudo strategy show <name>`: prints strategy metadata, system prompt excerpt, constraints, and allowed tools.
  4. `tudo strategy remove <name>`: deletes from user dir. Error if builtin.
  5. Integration test: `add` → `list` includes it → `show` prints details → `remove` → `list` no longer includes it.
- **Verification**: `cargo test -p tudo -- strategy` passes. Manual: `tudo strategy list` shows 3 builtins. `tudo strategy add test --from ./test.toml` registers. `tudo strategy show test` prints details.

### CP-3: Client-side risk pre-check
- **Touches**: `tudo/src/risk/mod.rs`, `precheck.rs` (NEW), `tudo/src/tools/submit_signal.rs`
- **Tasks**:
  1. Implement `RiskPrecheck::validate()` with 4 checks: leverage bound, max positions, symbol allowlist, drawdown (stub — tracks position count only for now).
  2. Integrate into `SubmitSignalTool::execute()`: before calling `api.submit_signal()`, run precheck. On failure, return `ToolResult` with `status: "rejected_client_side"` and reason — LLM sees this as a tool response, not an error.
  3. Load risk constraints from active strategy's `[constraints]` section.
  4. Track position count from `ListPositionsTool` results (cached in `AgentState`).
  5. Unit test: leverage=10 with max_leverage=3 → rejected with clear reason. leverage=3 with max_leverage=5 → passes. max positions reached → rejected. symbol not in allowlist → rejected.
- **Verification**: `cargo test -p tudo -- risk` passes all 6 precheck scenarios.

### CP-4: `tudo agent start --strategy` integration
- **Touches**: `tudo/src/cmd/agent.rs`, `tudo/src/cmd/strategy.rs`, `tudo/src/main.rs`
- **Tasks**:
  1. When `--strategy <name>` is passed to `agent start`, load from `StrategyRegistry::get()`.
  2. Strategy's `[prompt].system` replaces default system prompt.
  3. Strategy's `[loop]` config overrides defaults: `interval_secs`, `shadow_only`, `max_signals_per_hour`.
  4. Strategy's `[allowed_tools].tools` filters the tool list — only listed tools are sent to the LLM.
  5. If strategy not found, print `tudo strategy list` output with error message.
  6. Integration test: `agent start --strategy mean-reversion` uses Bollinger Bands system prompt, only sends allowed tools, respects strategy constraints.
- **Verification**: `cargo test -p tudo -- agent` passes with strategy-specific tests.

### CP-5: `tudo init` onboarding flow
- **Touches**: `tudo/src/cmd/init.rs` (NEW), `tudo/src/view/init.rs` (NEW), `tudo/src/model/session.rs` (NEW), `tudo/src/app.rs`
- **Tasks**:
  1. Implement 5-step TUI wizard:
     - Step 1 (Base URL): text input field, validates URL format, Enter to confirm.
     - Step 2 (Auth): calls `GET /onboarding/status`. If `has_api_key` → skip. Else → guide to create key, paste input field. Validate `tudo_sk_` prefix.
     - Step 3 (Exchange): shows `available_exchanges` from onboarding status. User selects with up/down arrows.
     - Step 4 (Risk): shows current risk config. Editable fields for leverage (1-125), account risk (0.1-100%), drawdown limit (1-100%).
     - Step 5 (Summary): prints config summary, "Save and exit" → writes to `~/.config/tudo/config.toml`.
  2. TUI rendering: each step renders as a bordered form with label + input. Tab cycles fields. Enter confirms step. Esc goes back.
  3. Config saved atomically: write to temp file, then rename.
  4. Integration test: run `tudo init` with mock API → config written correctly. Run again → reads existing config, pre-fills fields.
- **Verification**: `cargo test -p tudo -- init` passes. Manual: `tudo init` runs 5-step wizard, saves config, `tudo journal` works with saved key.

---

## Acceptance Criteria

- [ ] `StrategyRegistry` loads 3 built-in strategies from embedded TOML
- [ ] `tudo strategy list` shows all strategies with metadata
- [ ] `tudo strategy add/remove` works for user strategies
- [ ] `tudo agent start --strategy mean-reversion` uses strategy's system prompt + constraints + tool filter
- [ ] Client-side risk pre-check rejects over-leveraged signals, max position violations, and disallowed symbols
- [ ] Rejected signals return structured tool results (not errors) so LLM can adapt
- [ ] `tudo init` completes 5-step wizard and writes valid config
- [ ] Init flow reads onboarding status to skip completed steps
- [ ] `cargo clippy --all-targets && cargo test` passes in `tudo/`

---

## Risks

1. **Built-in strategy maintenance** — Hardcoded TOML files in the binary mean strategy updates require a new release. Mitigation: user strategies in `~/.config/tudo/strategies/` take priority over builtins. Users can override builtins without waiting for a release.
2. **Init flow complexity** — 5 TUI steps with API calls is the most complex UI in the harness. Mitigation: each step is an independent screen; API call failures don't crash the flow (they show errors inline). The user can Esc at any step to save partial progress.
3. **Risk pre-check drift** — The client-side check may diverge from the server-side `RiskService`. Mitigation: the pre-check is a subset of server checks (leverage, positions, symbols). If the server rejects a signal the client approved, the LLM sees the server rejection as a tool result — it can adapt. The pre-check catches the most common rejections early.

---

## Completion Signal

This spec is complete when:
1. 3 built-in strategies registered and loadable
2. Strategy CLI commands all work
3. `tudo agent start --strategy <name>` runs with strategy-specific personality
4. Risk pre-check catches common violations client-side
5. `tudo init` guides user from blank config to ready-to-trade
6. `cargo clippy --all-targets && cargo test` passes in `tudo/`
7. Code committed to master

---

## Next Spec

**CLI-05-daemon-polish** — Daemon mode, Unix socket, TUI attach, P&L sparkline, live panes wired with real data, integration test suite. The final polish spec.
