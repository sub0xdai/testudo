# CLI-04-strategy-registry — Implementation Plan

## Current State Summary

CLI-01 through CLI-03 are complete. The `testudo` binary has a TUI, config, API/WS clients, LLM provider abstraction (Anthropic), 7 tool definitions, and a working `agent start` loop. Strategy and risk infrastructure exists as empty stubs: `strategies/registry.rs`, `strategies/template.rs`, `risk/precheck.rs` (4 lines each, anchor tags only). The `cmd.rs` dispatches `Command::Strategy(StrategyAction::List/Add/Show/Remove)` to a generic "not yet implemented" stub. `Command::Init` is also a stub. No `strategies/builtins/` directory exists. No new Cargo.toml dependencies needed — `toml` and `serde` already present from CLI-01.

The backend's onboarding flow (`GET /api/v1/onboarding/status`) returns readiness state, missing items, available exchanges, and risk config — enough to drive a guided init wizard. The `ApiClient` already has `get_onboarding_status()`, `get_risk_config()`, and `update_risk_config()` methods.

### Gap Summary

| Requirement | Status | Detail |
|---|---|---|
| FR-1: StrategyRegistry with TOML loading | ❌ None | 2 stub files |
| FR-2: 3 built-in strategies | ❌ None | No TOML files exist |
| FR-3: `strategy list` command | ❌ None | Stub dispatch |
| FR-4: `strategy add` command | ❌ None | Stub dispatch |
| FR-5: `strategy show` command | ❌ None | Stub dispatch |
| FR-6: `strategy remove` command | ❌ None | Stub dispatch |
| FR-7: `agent start --strategy` integration | ❌ None | run_agent ignores strategy_name |
| FR-8: Client-side risk pre-check | ❌ None | precheck.rs is a stub |
| FR-9: `tudo init` TUI wizard | ❌ None | Init command is a stub |
| FR-10: build/test | ✅ Pass | 85 tests, clean clippy |

---

## Checkpoints

### CP-1: Strategy registry + 3 built-in strategies ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/strategies/template.rs`, `src/strategies/registry.rs`, `strategies/builtins/*.toml` (NEW), `src/lib.rs`
- **Tasks**:
  1. Define `StrategyTemplate` struct with all TOML sections: `[meta]` (name, version, description), `[loop]` (interval_secs, shadow_only, max_signals_per_hour), `[prompt]` (system), `[constraints]` (max_leverage, max_position_notional, allowed_symbols, shadow_only), `[allowed_tools]` (tools list). Derive Deserialize for TOML parsing.
  2. Write 3 built-in strategy TOML files in `strategies/builtins/`:
     - `mean_reversion.toml`: Bollinger Bands (20 SMA, 2σ), RSI confirmation, 2× ATR stop, max 3× leverage, tools: fetch_klines, submit_signal, read_journal, write_journal.
     - `momentum_breakout.toml`: Volume breakout, 20-period high/low channel, max 5× leverage, tools: all.
     - `funding_arb.toml`: Funding rate differential, delta-neutral hedge, max 2× leverage, tools: all.
  3. Implement `StrategyRegistry`: `new(config_dir)` loads builtins via embedded TOML strings (use `include_str!` or lazy_static), `get(name)` returns user override or builtin, `list()` returns all, `add(name, content)` validates + saves to `~/.config/testudo/strategies/`, `remove(name)` deletes user file (rejects builtins).
  4. Unit test: registry loads 3 builtins. `get("mean-reversion")` returns valid prompt. `get("nonexistent")` returns None. `add` + `remove` round-trip. Remove builtin → error.
- **Verification**: `cargo test -- strategies` passes. 3 builtins present with valid TOML structure.
- **Commit message**: `feat: strategy registry with 3 built-in trading strategies`

### CP-2: Strategy CLI commands (list/add/show/remove) ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/cmd.rs` (rewrite strategy dispatch), `src/main.rs` (wire)
- **Tasks**:
  1. `strategy list`: loads `StrategyRegistry`, prints table with Name | Version | Description | Source columns. Source is "builtin" or path to user file.
  2. `strategy add <name> --from <path>`: reads file, validates as `StrategyTemplate`, copies to user strategies dir, prints confirmation.
  3. `strategy show <name>`: prints full strategy details — metadata, system prompt, constraints, allowed tools.
  4. `strategy remove <name>`: deletes from user dir. Clear error if builtin.
  5. Add `--strategy <name>` to `Command::Agent(AgentAction::Start)`. For now, just load the strategy and print its name (integration comes in CP-4).
  6. Integration test: `add` → `list` includes it → `show` prints details → `remove` → `list` no longer includes it.
- **Verification**: `cargo test -- strategy` passes. Manual: `testudo strategy list` shows 3 builtins. Add/show/remove user strategies works.
- **Commit message**: `feat: strategy list/add/show/remove CLI commands`

### CP-3: Client-side risk pre-check

- **Touches**: `src/risk/precheck.rs`, `src/cmd.rs` (wire into agent loop)
- **Tasks**:
  1. Implement `RiskPrecheck` struct: holds strategy constraints + user risk config.
  2. `validate(signal_args) → PrecheckResult`: checks leverage ≤ max_leverage, max positions not exceeded, symbol in allowlist. Returns structured result with reason + suggestion on failure.
  3. Integrate into `submit_signal` tool execution path in `run_agent`: before calling tool, run precheck. On failure, return tool result with `status: "rejected_client_side"` + reason — the LLM sees it as a tool response to adapt.
  4. Load constraints from active strategy's `[constraints]` section.
  5. Unit test: leverage=10 with max=3 → rejected with clear reason. Leverage=3 with max=5 → passes. Max positions reached → rejected. Symbol not in allowlist → rejected. No constraints → passes.
- **Verification**: `cargo test -- risk` passes 6 check scenarios.
- **Commit message**: `feat: client-side risk pre-check for leverage, positions, and symbols`

### CP-4: `testudo agent start --strategy` integration

- **Touches**: `src/cmd.rs` (run_agent loads strategy), `src/main.rs` (pass strategy name)
- **Tasks**:
  1. When `--strategy <name>` is passed, load from `StrategyRegistry::get()`. If not found, print error + `strategy list` output.
  2. Strategy's `[prompt].system` replaces the default AGENT_TRADING.md system prompt.
  3. Strategy's `[loop]` fields override defaults (interval_secs, shadow_only, max_signals_per_hour).
  4. Strategy's `[allowed_tools]` filters which tool definitions are sent to the LLM.
  5. Strategy's `[constraints]` feeds into `RiskPrecheck` for signal validation.
  6. Integration test: `agent start --strategy mean-reversion` uses strategy prompt, filtered tools, and constraint-enforced precheck.
- **Verification**: `cargo test -- agent` passes strategy-specific tests. Manual: printed system prompt differs per strategy.
- **Commit message**: `feat: strategy-aware agent loop with prompt, constraints, and tool filtering`

### CP-5: `testudo init` onboarding TUI

- **Touches**: `src/cmd.rs` (run_init), `src/main.rs` (wire Init command)
- **Tasks**:
  1. Implement `run_init(config)` — 5-step guided flow:
     - Step 1 (Base URL): text input, validates URL format. Defaults to current config.
     - Step 2 (Auth): calls `GET /onboarding/status`. Shows next_step guidance. Accepts agent_key input (validates `testudo_sk_` prefix).
     - Step 3 (Exchange): shows `available_exchanges` from onboarding. User selects one.
     - Step 4 (Risk): fetches current risk config via `GET /risk-config`. Editable fields: leverage, account risk %, drawdown limit.
     - Step 5 (Save): writes to `~/.config/testudo/config.toml` atomically, prints summary.
  2. For CLI-04, implement as a terminal-prompt flow (not TUI). Each step uses stdin/stdout. TUI wizard version deferred to CLI-05 (daemon polish) which can reuse this logic.
  3. Atomic config save: write to `.tmp` file, validate by re-reading, then rename over config.toml.
  4. Unit test: init flow produces valid config with mock input. Empty agent_key → guided to create one.
- **Verification**: `cargo test -- init` passes. Manual: `testudo init` runs interactive flow, saves valid config.
- **Commit message**: `feat: guided init onboarding flow with API-driven step detection`

---

## Risks & Open Questions

1. **Init as terminal-prompt vs TUI** — The spec calls for a TUI wizard. Full TUI is complex. Terminal-prompt flow is simpler, works over SSH, and can be upgraded to TUI in CLI-05. Users can still edit `config.toml` directly.
2. **Strategy TOML format** — Must match exactly what `toml::from_str` expects. `[[take_profit]]` arrays, `[constraints]` tables, etc. Write tests against each builtin to catch format errors.
3. **`include_str!` for builtins** — The strategies directory is at `testudo-cli/strategies/builtins/`. From `src/strategies/registry.rs`, the relative path is `../../strategies/builtins/mean_reversion.toml`. Verify path at build time.
4. **Risk precheck vs backend risk engine** — The precheck is a subset of backend checks. If the backend adds new rules, the precheck must be updated. Acceptable: precheck catches common LLM mistakes; backend is the final authority.
5. **Init flow API dependencies** — Requires a running Testudo backend for `/onboarding/status` and `/risk-config`. The flow handles API errors gracefully (shows error, lets user retry).
