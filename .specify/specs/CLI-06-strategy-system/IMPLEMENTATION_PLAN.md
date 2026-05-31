# CLI-06-strategy-system — Implementation Plan

## Current State Summary

CLI-01 through CLI-05 are complete — 149 tests, full harness. STRAT-01 produced 7 Lean 4 proofs with corresponding `.toml` artifacts in `testudo-proofs/Proofs/`. Each artifact has `[meta]`, `[theorem]`, `[constraints]` (numeric bounds like `max_leverage = 5`), and `[prompt]` (system prompt sections). The harness loads strategies from `strategies/builtins/*.toml` with manually-coded constraints — no proof backing. The `testudo-cli/` crate has access to `testudo-proofs/Proofs/` via a relative path (`../../testudo-proofs/Proofs/`).

No bridge code exists. The `strategies/template.rs` has no `required_proofs` field. No artifact loader, constraint merger, tool constrainer, or strategy validator exist.

### Gap Summary

| Requirement | Status | Detail |
|---|---|---|
| FR-1: StrategyLoader loads artifacts | ❌ None | No loader code |
| FR-2: Version validation | ❌ None | No version check |
| FR-3: ConstraintMerger (most conservative) | ❌ None | No merger |
| FR-4: User config intersection | ❌ None | No intersection |
| FR-5: ToolConstrainer modifies schemas | ❌ None | Tools are static |
| FR-6: Derived tool guards | ❌ None | No regime classifier |
| FR-7: StrategyValidator startup check | ❌ None | No validator |
| FR-8: TUI risk pane proof sources | ❌ None | risk_pane shows raw numbers |
| FR-9: Prompt assembly from artifacts | ❌ None | Uses single strategy prompt |
| FR-10: `strategy validate` CLI | ❌ None | No command |

---

## Checkpoints

### CP-1: Artifact loader + ConstraintMerger ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/strategies/loader.rs` (NEW), `src/strategies/constraints.rs` (NEW), `Cargo.toml` (add glob), `src/lib.rs`
- **Tasks**:
  1. Add `glob = "0.3"` to Cargo.toml.
  2. Implement `loader.rs`: `StrategyArtifact` struct (meta, theorem, constraints as `HashMap<String, f64>`, prompt). `StrategyLoader::new(proofs_dir)` + `load_all()` → globs `*.toml`, parses each, returns `HashMap<String, StrategyArtifact>`.
  3. Implement `constraints.rs`: `ConstraintSet` struct (max_leverage, max_account_risk_pct, max_drawdown_pct, min_samples, stop_loss_required). `merge(artifacts)` picks most conservative bound (min for caps, max for floors). `intersect_user(user_risk)` applies `min(artifact, user)` for all bounds.
  4. Unit test: load KellyOptimal.toml → verify max_leverage=5, max_account_risk_pct=2.0.
  5. Unit test: merge Kelly (max_leverage=5) + GamblersRuin (max_leverage=3) → effective=3.
  6. Unit test: merge produces max_leverage=5, user config has max_leverage=2 → intersect → 2.
- **Verification**: `cargo test -- loader` passes. Artifacts load from disk. Merge picks tighter bounds. User can only tighten.
- **Commit message**: `feat: proof artifact loader and constraint merger with user intersection`

### CP-2: Tool constrainer + strategy validation

- **Touches**: `src/strategies/tools.rs` (NEW), `src/strategies/validator.rs` (NEW), `src/strategies/template.rs` (add required_proofs), `src/strategies/registry.rs` (integrate validation)
- **Tasks**:
  1. Implement `tools.rs`: `ToolConstrainer::constrain_signal_tool(tool_def, constraints)` — clamps `leverage.maximum`, adds proof-backed constraints to description, enforces `stop_loss` in required fields.
  2. Implement `validator.rs`: `StrategyValidator::validate(strategy, artifacts)` — checks `required_proofs` all present, checks strategy constraints don't exceed proof bounds. Returns `ValidationResult { valid, errors, warnings }`.
  3. Add `required_proofs: Vec<String>` to `StrategyTemplate` (default empty).
  4. Integrate validator into `StrategyRegistry::get()`: when loading a strategy, validate against artifacts. Invalid → log warning, return the strategy with warnings but don't block (user may be developing).
  5. Unit test: validate strategy with missing required_proof → warning.
  6. Unit test: constrain submit_signal tool with max_leverage=3 → schema's leverage.maximum becomes 3.
- **Verification**: `cargo test -- tools` passes. Tool schemas reflect constraints. Validator catches missing proofs.
- **Commit message**: `feat: tool constrainer and strategy validator with proof cross-reference`

### CP-3: `strategy validate` CLI + prompt assembly + integration

- **Touches**: `src/cmd.rs` (strategy validate handler), `src/main.rs` (wire), Agent loop integration
- **Tasks**:
  1. Add `StrategyAction::Validate { name: String }` to clap enum.
  2. Implement `run_strategy_validate(config_dir, name)`: loads strategy, loads artifacts from `testudo-proofs/Proofs/`, runs validator, prints constraint summary + proof sources. Non-destructive.
  3. Wire `Command::Strategy(StrategyAction::Validate { .. })` in main.rs.
  4. Integrate prompt assembly into `run_agent`: when strategy has `required_proofs`, load those artifacts, concatenate their `[prompt].system_prompt` sections before the strategy's own prompt.
  5. Integrate tool constraining: after loading tools, apply `ToolConstrainer` to modify tool schemas with merged constraints.
  6. Unit test: `strategy validate mean-reversion` prints constraint table with proof sources.
  7. Integration test: `agent start --strategy mean-reversion` loads Kelly + OU artifacts, system prompt contains both, tool leverage capped at 5.
- **Verification**: `cargo test -- validate` passes. `testudo strategy validate mean-reversion` prints proof-backed constraints. Agent loop uses constrained tools.
- **Commit message**: `feat: strategy validate CLI and proof-backed prompt assembly`

---

## Risks & Open Questions

1. **Artifact path resolution** — The proofs are at `../../testudo-proofs/Proofs/` relative to `testudo-cli/`. This works in the monorepo but will break when the CLI is split to its own repo. Mitigation: add `proofs_dir` to config with a default that works in the monorepo. Users set it explicitly in the standalone CLI.
2. **TOML format for constraints** — The artifact TOML uses `max_leverage = 5` (bare int) which deserializes to `toml::Value::Integer`. Need to handle both int and float constraints in the merge logic.
3. **Glob dependency** — `glob = "0.3"` is simple and pulls no transitive deps. Alternative: use `std::fs::read_dir` with a `.toml` extension filter (simpler, no new dep).
4. **Proofs dir may not exist** — In a standalone CLI deployment, `testudo-proofs/` won't be present. The loader should fail gracefully: log a warning, return empty artifacts, and let the harness run without proof-backed constraints (just strategy-level constraints).
