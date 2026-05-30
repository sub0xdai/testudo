# Specification: Strategy System Bridge — Proof-Backed Strategy Loading

**Spec ID:** AGENT-09-strategy-system
**Date:** 2026-05-30
**Status:** Draft
**Class:** Feature / Integration
**Priority:** P0 — connects the Lean verification layer (STRAT-01) to the trading harness (AGENT-08); without this bridge, proofs are inert math
**Depends on:** STRAT-01-lean-proofs, AGENT-08-trading-harness
**Series:** AGENT-09 (Strategy System Bridge)

---

## Problem Statement

STRAT-01 produces verified Lean 4 proofs and TOML strategy artifacts. AGENT-08 is a trading harness that runs LLM-powered strategies. The two are disconnected:

- The harness loads strategies from `strategies/builtins/mean_reversion.toml` — these are loose prompt templates with manually-coded constraints. A strategy author could set `max_leverage = 50` and the harness wouldn't complain.
- The proofs in `testudo-proofs/Proofs/KellyOptimal.toml` define mathematically-derived constraints (`max_leverage = 5` derived from Quarter-Kelly at p=0.60) — but nothing loads them.
- When a user selects `tudo agent start --strategy mean-reversion`, there's no guarantee the strategy's constraints are backed by any proof at all.

The bridge (this spec) loads strategy artifacts from STRAT-01, validates constraint consistency with the user's risk config, and injects proof-derived constraints into the LLM's tool definitions. The result: the LLM physically cannot over-leverage, cannot set a drawdown limit above the proven ruin bound, and cannot assign a confidence score to a regime it doesn't have a metric for.

---

## User Stories

- **As a harness operator**, I run `tudo agent start --strategy mean-reversion` and the harness automatically loads the Kelly, OU, and Wasserstein constraints — I never configure a leverage cap manually.
- **As a strategy developer**, I write a Lean proof → write a `.toml` artifact → the harness picks it up. No glue code needed.
- **As an LLM**, my `submit_signal` tool has `max_leverage: 5` and `max_account_risk_pct: 2.0` baked into the function definition — I can't violate proven bounds even if I hallucinate.
- **As a risk auditor**, I can trace every constraint in the harness back to a specific line in a Lean proof via the artifact's `lean_file` and `lean_line` fields.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `StrategyLoader` loads `.toml` artifacts from `testudo-proofs/Proofs/` at harness startup. Parses `[meta]`, `[theorem]`, `[constraints]`, `[prompt]` sections. | High | Loader |
| FR-2 | Loader validates artifact version matches the user's installed `testudo-proofs` version (from git hash or `lake build` timestamp). Rejects mismatched artifacts with a clear error. | Medium | Loader |
| FR-3 | `ConstraintMerger` combines constraints from multiple artifacts (e.g., Kelly + Gambler's Ruin): picks the MOST conservative value when theorems overlap (e.g., max drawdown from Kelly vs. Gambler's Ruin — use the tighter bound). | High | Constraints |
| FR-4 | `ConstraintMerger` intersects with user's risk config: if user sets `max_leverage = 2` and Kelly artifact says `max_leverage = 5`, the effective value is `min(user, artifact) = 2`. | High | Constraints |
| FR-5 | `ToolConstrainer` applies merged constraints to the LLM's tool definitions: modifies `submit_signal`'s JSON Schema to clamp `leverage.maximum`, injects `max_account_risk_pct` into the function description, adds Wasserstein `min_samples` validation to `fetch_klines`. | High | Tools |
| FR-6 | `ToolConstrainer` adds derived tool guards: if `mean-reversion` strategy loads Wasserstein + OU artifacts, the harness adds a `classify_regime` tool that the LLM MUST call before `submit_signal`. | Medium | Tools |
| FR-7 | `StrategyValidator` runs at startup: checks that every strategy in `strategies/builtins/` references valid proof artifacts, constraint values don't contradict proven bounds, and required theorems are present. Strategies with invalid references are disabled (logged, not loaded). | Medium | Validation |
| FR-8 | The merged constraint set is surfaced in the TUI's risk pane: shows which proof backs each limit, the effective value, and whether the user's config is tightening or loosening the artifact's bound. | Low | TUI |
| FR-9 | Strategy prompt assembly: the harness concatenates `[prompt].system_prompt` from each artifact loaded for the active strategy into the LLM's system prompt. Ordering follows strategy dependency chain (Wasserstein → OU → Kelly → Gambler's Ruin). | High | Prompt |
| FR-10 | `tudo strategy validate <name>` CLI command: loads the strategy's artifacts, runs `ConstraintMerger` + `StrategyValidator`, prints constraint summary and any conflicts. Non-destructive — doesn't start trading. | Medium | CLI |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `StrategyLoader` reads a single `.toml` artifact (KellyOptimal). Parses all sections into typed structs. | Loader deserializes without errors, constraint values are correct types |
| CP-2 | `ConstraintMerger` merges Kelly + Gambler's Ruin constraints (overlapping: drawdown). Picks tighter bound. | `max_drawdown = min(Kelly_drawdown, Ruin_drawdown)` |
| CP-3 | `ConstraintMerger` intersects with user risk config. `max_leverage = min(user_config, artifact)`. | User can only TIGHTEN constraints, never loosen |
| CP-4 | `ToolConstrainer` modifies `submit_signal` tool schema: clamps leverage, adds risk_pct to description. | LLM tool JSON Schema reflects merged constraints |
| CP-5 | `StrategyValidator` cross-references `mean_reversion.toml` strategy against proof artifacts. | Missing theorem → strategy disabled with log warning |
| CP-6 | Full integration: `tudo agent start --strategy mean-reversion` loads artifacts → merges constraints → constrains tools → assembles prompt → starts loop. | End-to-end: LLM gets proof-backed tools, can't violate bounds |

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      AGENT-09 Bridge                         │
│                                                              │
│  testudo-proofs/Proofs/          strategies/builtins/        │
│  ├── KellyOptimal.toml    ─┐     ├── mean_reversion.toml    │
│  ├── GamblersRuin.toml    ─┤     ├── momentum_breakout.toml  │
│  ├── WassersteinMetric... ─┤     └── funding_arb.toml        │
│  ├── OUMreversion.toml    ─┤                                  │
│  ├── MomentumAutocorr...  ─┤                                  │
│  ├── FundingArb.toml      ─┤                                  │
│  └── DeltaNeutral.toml    ─┘                                  │
│            │                                                  │
│            ▼                                                  │
│  ┌─────────────────────┐                                     │
│  │   StrategyLoader    │  Parses TOML → typed structs        │
│  └────────┬────────────┘                                     │
│           │                                                   │
│           ▼                                                   │
│  ┌─────────────────────┐     ┌──────────────────────┐       │
│  │  ConstraintMerger   │────▶│  User risk config     │       │
│  │                     │     │  (~/.config/tudo/...) │       │
│  │  Merge(Kelly, Ruin) │     └──────────────────────┘       │
│  │  = most conservative│                                     │
│  │  = min(all bounds)  │                                     │
│  └────────┬────────────┘                                     │
│           │                                                   │
│           ▼                                                   │
│  ┌─────────────────────┐                                     │
│  │   ToolConstrainer   │  Modifies LLM tool JSON Schemas     │
│  │                     │                                     │
│  │  submit_signal:     │                                     │
│  │    leverage.max = 5 │  ← from KellyOptimal.toml           │
│  │    stop_loss req'd  │  ← from GamblersRuin.toml           │
│  │                     │                                     │
│  │  fetch_klines:      │                                     │
│  │    limit.min = 50   │  ← from WassersteinMetric.toml      │
│  └────────┬────────────┘                                     │
│           │                                                   │
│           ▼                                                   │
│  ┌─────────────────────┐                                     │
│  │  Prompt Assembler   │  Concatenates [prompt] sections     │
│  │                     │                                     │
│  │  System prompt =    │                                     │
│  │    Wasserstein.prompt│                                    │
│  │    + OU.prompt      │                                     │
│  │    + Kelly.prompt   │                                     │
│  │    + Ruin.prompt    │                                     │
│  │    + strategy.prompt│                                     │
│  └────────┬────────────┘                                     │
│           │                                                   │
│           ▼                                                   │
│  ┌─────────────────────┐                                     │
│  │  StrategyValidator  │  Startup check: artifacts present?  │
│  │                     │  constraint bounds consistent?      │
│  │                     │  required theorems all loaded?      │
│  └─────────────────────┘                                     │
│                                                              │
│  Consumed by: AGENT-08 (harness) → AgentState.strategy       │
└─────────────────────────────────────────────────────────────┘
```

### Key Types

```rust
// testudo-cli/src/strategies/loader.rs — NEW

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// A loaded strategy artifact from a TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct StrategyArtifact {
    pub meta: ArtifactMeta,
    pub theorem: TheoremRef,
    pub constraints: HashMap<String, ConstraintValue>,
    pub prompt: PromptSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub lean_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TheoremRef {
    pub name: String,
    pub statement: String,
    pub formula: String,
    pub implications: Vec<String>,
    pub lean_line: u32,
}

/// A single constraint value — always a numeric bound or enum.
#[derive(Debug, Clone)]
pub enum ConstraintValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
}

impl<'de> Deserialize<'de> for ConstraintValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ConstraintValue::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(ConstraintValue::Float(f))
                } else {
                    Err(serde::de::Error::custom("invalid number"))
                }
            }
            serde_json::Value::Bool(b) => Ok(ConstraintValue::Bool(b)),
            serde_json::Value::String(s) => Ok(ConstraintValue::String(s)),
            _ => Err(serde::de::Error::custom("unsupported constraint type")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromptSection {
    pub system_prompt: String,
}
```

```rust
// testudo-cli/src/strategies/loader.rs — NEW (continued)

/// Loads all strategy artifacts from a directory.
pub struct StrategyLoader {
    proofs_dir: PathBuf,
}

impl StrategyLoader {
    pub fn new(proofs_dir: PathBuf) -> Self {
        Self { proofs_dir }
    }

    /// Load all `.toml` artifacts from the proofs directory.
    pub fn load_all(&self) -> Result<HashMap<String, StrategyArtifact>, LoadError> {
        let mut artifacts = HashMap::new();
        let pattern = self.proofs_dir.join("*.toml");

        for entry in glob::glob(&pattern.to_string_lossy())? {
            let path = entry?;
            let content = std::fs::read_to_string(&path)?;
            let artifact: StrategyArtifact = toml::from_str(&content)?;

            let name = artifact.meta.name.clone();
            if artifacts.contains_key(&name) {
                return Err(LoadError::DuplicateName(name));
            }
            artifacts.insert(name, artifact);
        }

        if artifacts.is_empty() {
            return Err(LoadError::NoArtifactsFound(self.proofs_dir.clone()));
        }

        Ok(artifacts)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("no artifacts found in {0}")]
    NoArtifactsFound(PathBuf),
    #[error("duplicate artifact name: {0}")]
    DuplicateName(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("glob error: {0}")]
    Glob(#[from] glob::PatternError),
}
```

```rust
// testudo-cli/src/strategies/constraints.rs — NEW

use std::collections::HashMap;

/// Merged constraint set — the effective limits for the current strategy session.
#[derive(Debug, Clone)]
pub struct ConstraintSet {
    pub max_leverage: i64,
    pub max_account_risk_pct: f64,
    pub max_drawdown_pct: f64,
    pub min_kelly_fraction: f64,
    pub max_kelly_fraction: f64,
    pub min_samples: i64,
    pub min_required_win_rate: f64,
    pub stop_loss_required: bool,
    /// Which artifact provided each constraint (for audit trail).
    pub sources: HashMap<String, String>,
}

impl ConstraintSet {
    /// Merge multiple artifacts' constraints, picking the most conservative value.
    /// Then intersect with user's risk config (user can only tighten, never loosen).
    pub fn merge(
        artifacts: &HashMap<String, StrategyArtifact>,
        user_risk: &UserRiskConfig,
    ) -> Self {
        let mut cs = ConstraintSet::defaults();
        let mut sources = HashMap::new();

        for (name, artifact) in artifacts {
            for (key, value) in &artifact.constraints {
                cs.apply(name, key, value, &mut sources);
            }
        }

        // Intersect with user config: use min(artifact, user) for bounds
        cs.max_leverage = cs.max_leverage.min(user_risk.max_leverage);
        cs.max_account_risk_pct = cs.max_account_risk_pct.min(user_risk.account_risk_pct);
        cs.max_drawdown_pct = cs.max_drawdown_pct.min(user_risk.daily_drawdown_limit.unwrap_or(20.0));

        cs
    }

    fn apply(&mut self, source: &str, key: &str, value: &ConstraintValue, sources: &mut HashMap<String, String>) {
        let previous = sources.get(key).cloned();
        match key {
            "max_leverage" => self.set_bound(&mut self.max_leverage, value, source, key, sources),
            "max_account_risk_pct" => self.set_bound_f64(&mut self.max_account_risk_pct, value, source, key, sources),
            "max_drawdown_pct" => self.set_bound_f64(&mut self.max_drawdown_pct, value, source, key, sources),
            // ... other constraints
            _ => {}
        }
    }

    fn set_bound(&mut self, field: &mut i64, value: &ConstraintValue, source: &str, key: &str, sources: &mut HashMap<String, String>) {
        if let ConstraintValue::Int(v) = value {
            if *field == Self::default_i64(key) || *v < *field {
                *field = *v;
                sources.insert(key.to_string(), source.to_string());
            }
        }
    }

    // ... similar for f64 fields
    // pick MIN for caps (more conservative = smaller bound)
    // pick MAX for floors (more conservative = larger floor, e.g., min_samples)
}
```

```rust
// testudo-cli/src/strategies/tools.rs — NEW

use crate::tools::types::ToolDef;

/// Applies merged constraints to LLM tool definitions.
pub struct ToolConstrainer;

impl ToolConstrainer {
    /// Modify `submit_signal` tool schema to reflect current constraints.
    pub fn constrain_signal_tool(
        tool: &mut ToolDef,
        constraints: &ConstraintSet,
    ) {
        // Clamp leverage max
        if let Some(props) = tool.parameters
            .get_mut("properties")
            .and_then(|p| p.get_mut("leverage"))
            .and_then(|l| l.get_mut("maximum"))
        {
            *props = serde_json::json!(constraints.max_leverage);
        }

        // Add account risk to function description
        tool.description = format!(
            "{}\n\nRisk constraints (proof-backed, DO NOT VIOLATE):\n\
             - Max leverage: {}×\n\
             - Max account risk per trade: {}%\n\
             - Stop loss REQUIRED: {}\n\
             - Max drawdown: {}%",
            tool.description,
            constraints.max_leverage,
            constraints.max_account_risk_pct,
            constraints.stop_loss_required,
            constraints.max_drawdown_pct,
        );

        // Make stop_loss required if constraint says so
        if constraints.stop_loss_required {
            if let Some(required) = tool.parameters.get_mut("required") {
                if let Some(arr) = required.as_array_mut() {
                    if !arr.iter().any(|v| v.as_str() == Some("stop_loss")) {
                        arr.push(serde_json::json!("stop_loss"));
                    }
                }
            }
        }
    }

    /// Add strategy-specific tools. e.g., if Wasserstein is loaded, add `classify_regime`.
    pub fn strategy_tools(artifacts: &HashMap<String, StrategyArtifact>) -> Vec<ToolDef> {
        let mut tools = vec![];
        if artifacts.contains_key("wasserstein") {
            tools.push(classify_regime_tool());
        }
        tools
    }
}
```

### Strategy Validation

```rust
// testudo-cli/src/strategies/validator.rs — NEW

/// Validates that a user strategy references valid proof artifacts
/// and its constraints don't contradict proven bounds.
pub struct StrategyValidator;

#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl StrategyValidator {
    /// Validate a strategy TOML against loaded proof artifacts.
    pub fn validate(
        strategy_path: &Path,
        artifacts: &HashMap<String, StrategyArtifact>,
    ) -> ValidationResult {
        let mut result = ValidationResult { valid: true, errors: vec![], warnings: vec![] };

        let strategy: StrategyTemplate = match std::fs::read_to_string(strategy_path)
            .map_err(|e| e.to_string())
            .and_then(|s| toml::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(s) => s,
            Err(e) => {
                result.errors.push(format!("Failed to parse strategy: {}", e));
                result.valid = false;
                return result;
            }
        };

        // Check required artifacts are present
        for required in &strategy.required_proofs {
            if !artifacts.contains_key(required) {
                result.errors.push(format!(
                    "Strategy requires proof artifact '{}' but it's not installed. \
                     Run `lake build` in testudo-proofs/ and ensure {}.toml exists.",
                    required, required
                ));
                result.valid = false;
            }
        }

        // Check strategy constraints don't violate proven bounds
        if let Some(ref strat_constraints) = strategy.constraints {
            for (name, artifact) in artifacts {
                if let Some(ref artifact_constraints) = artifact.constraints {
                    if let Some(leverage) = strat_constraints.max_leverage {
                        if let Some(ConstraintValue::Int(max_lev)) = artifact_constraints.get("max_leverage") {
                            if leverage > *max_lev {
                                result.errors.push(format!(
                                    "Strategy max_leverage={} exceeds {}'s proven bound of {}",
                                    leverage, name, max_lev
                                ));
                                result.valid = false;
                            }
                        }
                    }
                }
            }
        }

        result
    }
}
```

### Files

All in `testudo-cli/` (within AGENT-08's crate):

- `src/strategies/loader.rs` — **NEW** — `StrategyLoader`, artifact TOML parsing
- `src/strategies/constraints.rs` — **NEW** — `ConstraintSet`, merge logic, user config intersection
- `src/strategies/tools.rs` — **NEW** — `ToolConstrainer`, schema modification
- `src/strategies/validator.rs` — **NEW** — `StrategyValidator`, cross-reference checks
- `src/strategies/registry.rs` — **MODIFY** — integrate loader + validator into strategy registry
- `src/strategies/template.rs` — **MODIFY** — add `required_proofs: Vec<String>` field to `StrategyTemplate`
- `src/tools/submit_signal.rs` — **MODIFY** — tool schema built from `ToolConstrainer` output
- `src/tools/fetch_klines.rs` — **MODIFY** — `limit.minimum` set from constraint
- `src/view/risk_pane.rs` — **MODIFY** — show proof sources for each risk limit

### Dependencies Added

```toml
# In testudo-cli/Cargo.toml
glob = "0.3"       # File globbing for artifact discovery
```

---

## Acceptance Criteria

- [ ] `StrategyLoader` successfully parses all 7 artifact TOML files from `testudo-proofs/Proofs/`
- [ ] `ConstraintMerger` picks the tighter bound when Kelly and Gambler's Ruin both define `max_drawdown`
- [ ] `ConstraintMerger` intersects with user config: `max_leverage = min(artifact, user_config)`
- [ ] `ToolConstrainer` modifies `submit_signal` JSON Schema: `leverage.maximum` reflects merged constraint
- [ ] `ToolConstrainer` adds `classify_regime` tool when Wasserstein artifact is loaded
- [ ] `StrategyValidator` rejects a strategy that requires a missing artifact with a clear error message
- [ ] `StrategyValidator` rejects a strategy whose constraints exceed proven bounds
- [ ] `tudo strategy validate mean-reversion` prints constraint summary with proof sources
- [ ] `tudo agent start --strategy mean-reversion` assembles system prompt from all loaded artifact prompts
- [ ] Risk pane in TUI dashboard shows which proof backs each limit (e.g., "max leverage: 5× (Kelly optimality)")
- [ ] Harness refuses to start if a required proof artifact is missing or version-mismatched
- [ ] `cargo clippy && cargo test` passes in `testudo-cli/`

---

## Risks

1. **Artifact loading overhead** — Loading and validating 7 TOML files at every startup adds latency. Mitigation: cache validated constraint sets in `~/.config/tudo/cache/` keyed by proof version. Only re-validate on version change.
2. **Constraint conflict resolution ambiguity** — If Kelly says `max_leverage = 5` but a user-written strategy says `max_leverage = 3`, which wins? Mitigation: the `ConstraintMerger` rule is **min(artifact, user_config, strategy_config)** — the most conservative value always wins. This is documented as the invariant.
3. **Missing proof dependency** — A strategy might reference a proof artifact that exists but has an incomplete theorem (documented `sorry`). Mitigation: `StrategyValidator` checks the artifact's `theorem.name` appears in the `.lean` file and the theorem is closed. Incomplete theorems generate a WARNING (strategy loads with reduced constraints) not an ERROR.
4. **TOML schema evolution** — The artifact format may evolve. Mitigation: the `version` field in `[meta]` enables graceful degradation. Loader supports a minimum version; older artifacts are rejected with "upgrade your proofs" message.

---

## Completion Signal

This spec is complete when:
1. `StrategyLoader` loads all 7 artifacts from testudo-proofs
2. `ConstraintMerger` produces correct merged + user-intersected constraints
3. `ToolConstrainer` modifies LLM tool schemas with proof-derived bounds
4. `tudo strategy validate <name>` works end-to-end
5. `tudo agent start --strategy mean-reversion` assembles full proof-backed prompt + constrained tools
6. All 12 acceptance criteria met
7. `cargo clippy && cargo test` passes
8. Code committed to master
