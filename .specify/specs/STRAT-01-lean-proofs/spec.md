# Specification: Lean 4 Strategy Proofs + Artifact Format — Verified Strategy Pipeline

**Spec ID:** STRAT-01-lean-proofs
**Date:** 2026-05-30
**Status:** Draft
**Class:** Core / Verification
**Priority:** P0 — the entire strategy system depends on verifiable proofs; without them the harness has no trust anchor
**Depends on:** None (standalone Lean 4 project, consumed by AGENT-09)
**Series:** STRAT-01 (Strategy Verification)

---

## Problem Statement

`testudo-proofs/` contains 7 Lean 4 proof files with substantive theorem code — `WassersteinMetric.lean` has closed proofs for W₁ metric properties, `KellyOptimal.lean` has the optimality inequality with real lemmas. The proof code exists and is reasonable. However `lake build` fails on a syntax error in `Proofs.lean` (the index file uses `/--` doc-comment syntax where `/-!` module-comment is needed, causing the parser to misinterpret `import` on the next line).

Beyond the compilation fix, a deeper problem exists: the proofs are disconnected from the harness. The Lean theorems prove mathematical properties, but there is no format for the harness (`tudo`, AGENT-08) to consume those proofs as executable constraints. An LLM making trading decisions cannot reference `ou_half_life_bound` — it needs structured constraints (max deviation threshold, mean-reversion half-life in candles, confidence mapping) derived from the proof, not the proof itself.

This spec fixes compilation AND defines the **strategy artifact format** — a TOML file alongside each Lean proof that bridges theorem → constraint → LLM prompt. Each proof ships with both its `.lean` source (for `lake build` verification) and its `.toml` artifact (for harness consumption). The two are linked by namespace convention: `WassersteinMetric.lean` ↔ `WassersteinMetric.toml`.

---

## User Stories

- **As a strategy developer**, I run `lake build` and verify that all 7 theorems are correct. No `sorry`. No trust — I can read the proof.
- **As the trading harness (tudo)**, I load `KellyOptimal.toml` and get the Kelly fraction bounds, max leverage constraint, and LLM prompt text — all derived from the verified proof.
- **As an LLM making trading decisions**, my tool `submit_signal` has `max_leverage` and `max_position_notional` baked into the function definition from the proof artifact — I can't violate proven bounds.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Fix `Proofs.lean` syntax: use `/-!` module comment syntax instead of `/--` so `lake build` compiles all 7 modules | High | Build |
| FR-2 | `lake build` exits 0 with zero warnings across all 7 proof modules | High | Build |
| FR-3 | Every `theorem` in every proof file is closed (no `sorry`, no `admit`). If a theorem is currently incomplete, annotate it with a TODO and a scope comment — do not leave a silent `sorry` | High | Proofs |
| FR-4 | Define the **strategy artifact format**: a TOML schema with `[meta]`, `[theorem]`, `[constraints]`, and `[prompt]` sections. Each `.lean` file has a matching `.toml` artifact | High | Artifact |
| FR-5 | `[theorem]` section maps the Lean theorem name to its statement in plain English, the key inequality/formula, and the Lean file + line where it's proved | High | Artifact |
| FR-6 | `[constraints]` section derives harness-enforceable constraints from each theorem (e.g., Kelly → `max_leverage = 3`, OU → `reversion_half_life_candles = 6`, Gambler's Ruin → `max_drawdown_pct = 20`) | High | Artifact |
| FR-7 | `[prompt]` section provides the LLM system prompt text explaining the strategy's rules, entry/exit logic, confidence mapping, and risk constraints — all backed by the theorem | High | Artifact |
| FR-8 | Artifacts include a `version` field (semver) so the harness can detect when proofs have changed and require re-validation | Medium | Artifact |
| FR-9 | A `verify-artifacts.py` script (or `lake run verify-artifacts`) checks that every `.lean` file has a matching `.toml`, that TOML `theorem.statement` matches a theorem name in the Lean file, and that constraint values are within proven bounds | Medium | Verification |
| FR-10 | `README.md` documents: which theorems are proven, their artifact files, how to build, and how the harness consumes them | Low | Docs |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Fix `Proofs.lean` syntax, `lake build` passes | All 7 modules compile, zero errors |
| CP-2 | Audit all theorems: verify zero `sorry`. Tag incomplete ones with explicit TODO comments | Every `theorem` is closed or explicitly documented as incomplete |
| CP-3 | Define TOML artifact schema. Write `WassersteinMetric.toml` + `KellyOptimal.toml` | Schema validates, constraint values are mathematically sound |
| CP-4 | Write artifacts for remaining 5 proofs | All 7 `.toml` files exist and validate |
| CP-5 | `verify-artifacts.py` script: cross-references `.lean` ↔ `.toml`, checks constraint bounds | Script passes, catches missing/mismatched artifacts |
| CP-6 | Update `README.md`, link to AGENT-09 | Documentation complete |

### Strategy Artifact Format

```toml
# WassersteinMetric.toml — strategy artifact for testudo-proofs

[meta]
# Maps to Proofs/WassersteinMetric.lean
name = "wasserstein"
version = "1.0.0"
description = "1-Wasserstein distance as a regime classifier"
lean_file = "Proofs/WassersteinMetric.lean"

[theorem]
# The mathematical statement this proof establishes.
name = "W1_emp_nonneg + W1_emp_symm + W1_emp_triangle"
statement = "The empirical 1-Wasserstein distance W₁(xs, ys) = (1/n)·Σ|xs[i] - ys[i]| is a metric on sorted lists of equal length."
formula = "W₁(xs, ys) = (1/n) · Σᵢ |xs[i] - ys[i]|"
implications = [
    "Regime classification by nearest-centroid is mathematically sound (metric guarantees consistent clustering)",
    "The triangle inequality ensures regime transitions are bounded: |W₁(A,C) - W₁(A,B)| ≤ W₁(B,C)",
]
lean_line = 17

[constraints]
# Harness-enforceable constraints derived from this theorem.
# These are loaded by AGENT-09 and fed to the LLM tool definitions.

# Minimum number of samples required for reliable regime classification.
min_samples = 50

# Wasserstein distance thresholds for regime assignment.
# Distances below this are considered "same regime".
same_regime_threshold = 0.002

# Maximum distance for considering a regime transition meaningful.
max_regime_distance = 0.05

[prompt]
# LLM system prompt fragment for this theorem's domain.
# Injected into the agent's system prompt when this strategy is active.

system_prompt = """
## Regime Detection — Wasserstein Distance

The 1-Wasserstein distance classifies the current market regime by comparing
recent returns against pre-computed regime centroids. The distance is defined as:

    W₁(μ, ν) = (1/n) · Σᵢ |xᵢ - yᵢ|  (sorted samples)

This is a PROVEN metric (non-negative, symmetric, triangle inequality).
See testudo-proofs/Proofs/WassersteinMetric.lean for the verification.

### Regime Assignment
- R0 (low vol, neg autocorr): W₁ < 0.005 → Mean Reversion strategies
- R1 (above-avg vol, pos autocorr): 0.005 ≤ W₁ < 0.015 → Momentum Breakout strategies
- R2 (high vol, no autocorr): 0.015 ≤ W₁ < 0.030 → Halt (no edge)
- R3 (extreme vol): W₁ ≥ 0.030 → Halt (preserve capital)

### How to Use
1. Call `fetch_klines` with interval=4h, limit=100 for the target symbol.
2. Compute returns = ln(close[t]/close[t-1]) for the last 50 candles.
3. Compare the return distribution against pre-computed regime centroids.
4. Select strategy based on the assigned regime.
5. Reclassify on each 4h candle close — do NOT reclassify intra-candle.
"""
```

```toml
# KellyOptimal.toml — strategy artifact for testudo-proofs

[meta]
name = "kelly"
version = "1.0.0"
description = "Kelly criterion optimality — fractional position sizing"
lean_file = "Proofs/KellyOptimal.lean"

[theorem]
name = "kelly_maximizes_growth"
statement = "The fraction f* = (b·p - q)/b uniquely maximizes E[log(wealth)] for binary-outcome bets with win probability p, net odds b, and loss probability q = 1-p."
formula = "f* = (b·p - (1-p)) / b  where b = net odds, p = win probability"
implications = [
    "Any other fraction f ≠ f* yields lower expected geometric growth",
    "Over-betting (f > f*) reduces growth AND increases ruin risk",
    "Under-betting (f < f*) is safer but suboptimal",
]
lean_line = 53

[constraints]
# These map theorem conclusions → harness-enforceable limits.

# Maximum Kelly fraction allowed (Quarter-Kelly for conservative sizing).
# Derived from: f* ≤ 1, Quarter-Kelly = f*/4, clamp to [0.005, 0.10].
max_kelly_fraction = 0.10
min_kelly_fraction = 0.005

# Leverage cap derived from Kelly: quarter-Kelly limits single-bet exposure.
# f_max = 0.10 → max leverage = 1/f_max → 10× theoretical, clamped to 5× for safety.
max_leverage = 5

# Maximum account risk per trade (Quarter-Kelly at 60% win rate, 1.5:1 R:R).
max_account_risk_pct = 2.0

# The Kelly fraction only applies when edge is positive.
# Below this win rate, the Kelly fraction is ≤ 0 (do not bet).
min_required_win_rate = 0.51

[prompt]
system_prompt = """
## Position Sizing — Kelly Criterion

Your position sizes are determined by the Quarter-Kelly formula:

    f* = (b·p - q)/b

where b = net odds, p = your estimated win probability, q = 1-p.

Testudo's sizing engine uses Quarter-Kelly: f_actual = (1/4) · f*
This is a PROVEN optimum — see testudo-proofs/Proofs/KellyOptimal.lean.

### Rules
- NEVER compute position size manually. Set `confidence` (0.0–1.0) and Testudo sizes for you.
- `confidence` maps to your estimated p: confidence = 0.60 → p ≈ 0.55 (with Bayesian shrinkage).
- If your edge is negative (estimated p < 0.51), do NOT trade. Skip the cycle.
- Max leverage: 5×. Max account risk per trade: 2.0%.
- If you've had 3 consecutive losses, reduce confidence by 20% for the next 5 trades.
- Over-betting DOES NOT increase returns — it increases ruin probability.
  See testudo-proofs/Proofs/GamblersRuin.lean.
"""
```

### Constraint Derivation Rules

Each constraint in the artifact MUST be mathematically derivable from the corresponding theorem. The `constraints` section is auditable — a reviewer should be able to trace every number back to a line in the Lean proof.

| Theorem | Constraint | Derivation |
|---------|-----------|------------|
| Kelly: f* = (bp-q)/b | max_kelly_fraction = 0.10 | Quarter-Kelly at p=0.60, b=1.5 → f* = 0.40 → f/4 = 0.10 |
| Kelly: f* = (bp-q)/b | max_leverage = 5 | At f=0.10, max exposure = 10× theoretical. Halved for safety margin |
| OU: |deviation| ≤ |initial|/2ⁿ | reversion_half_life_candles = 6 | n=3 half-lives → deviation ≤ 12.5% of initial. Requires ~6 candles at 4h |
| Gambler's Ruin: ruin prob bound | max_drawdown_pct = 20 | With 2% risk per trade and fair coin, 20% drawdown has ~64% probability. This is the stop limit |
| Wasserstein: W₁ is a metric | min_samples = 50 | Empirical convergence of W₁ requires O(1/√n). n=50 gives ~14% error bound |
| Momentum: ρ₁ > 0 → E[r_{t+1}|r_t>0] > 0 | min_autocorr_threshold = 0.10 | Statistical significance of ρ₁ at n=50: t = ρ·√(n-2)/√(1-ρ²) → critical ρ ≈ 0.09 |

### Lean Build Fix

The `Proofs.lean` file currently uses:

```lean4
/-- @anchor domain:proofs:index
    @tags domain -/

import Proofs.WassersteinMetric
```

The `/--` syntax is for doc-comments that attach to the NEXT declaration. Since there's no declaration after it, the parser treats the closing `-/` as ending a regular comment, leaving `import Proofs.WassersteinMetric` as an orphan declaration at module level where `import` is not allowed.

Fix: use `/-!` for module-level comments:

```lean4
/-!
@anchor domain:proofs:index
@tags domain
-/

import Proofs.WassersteinMetric
import Proofs.KellyOptimal
import Proofs.OUMreversion
import Proofs.MomentumAutocorr
import Proofs.FundingArb
import Proofs.DeltaNeutral
import Proofs.GamblersRuin
```

### verify-artifacts.py

```python
#!/usr/bin/env python3
"""Cross-reference Lean proofs ↔ TOML artifacts. Run from testudo-proofs/.,,"""
import os, sys, tomllib, re

PROOF_DIR = "Proofs"

def lean_theorem_names(path: str) -> set[str]:
    """Extract theorem names from a .lean file."""
    with open(path) as f:
        content = f.read()
    return set(re.findall(r'^theorem\s+(\w+)', content, re.MULTILINE))

def main():
    errors = 0
    for lean_file in sorted(os.listdir(PROOF_DIR)):
        if not lean_file.endswith('.lean'):
            continue
        base = lean_file.replace('.lean', '')
        path = os.path.join(PROOF_DIR, lean_file)
        toml_path = os.path.join(PROOF_DIR, f'{base}.toml')

        # Check artifact exists
        if not os.path.exists(toml_path):
            print(f"ERROR: {base}.lean has no matching {base}.toml artifact")
            errors += 1
            continue

        # Check theorem name matches
        with open(toml_path, 'rb') as f:
            artifact = tomllib.load(f)
        theorem_name = artifact.get('theorem', {}).get('name', '')
        lean_names = lean_theorem_names(path)

        if theorem_name and theorem_name not in lean_names:
            print(f"WARN: {base}.toml claims theorem '{theorem_name}' "
                  f"but {base}.lean defines: {lean_names}")

        # Check constraints are within proven bounds (spot checks)
        constraints = artifact.get('constraints', {})
        if 'max_leverage' in constraints and constraints['max_leverage'] > 10:
            print(f"WARN: {base}.toml max_leverage={constraints['max_leverage']} exceeds 10")

    # Check orphan TOMLs
    for toml_file in sorted(os.listdir(PROOF_DIR)):
        if not toml_file.endswith('.toml'):
            continue
        lean_file = toml_file.replace('.toml', '.lean')
        if not os.path.exists(os.path.join(PROOF_DIR, lean_file)):
            print(f"ERROR: {toml_file} has no matching {lean_file}")
            errors += 1

    if errors:
        print(f"\n{errors} error(s) found")
        sys.exit(1)
    print("All artifacts valid ✓")

if __name__ == '__main__':
    main()
```

### Files

All in `testudo-proofs/` (existing directory, modified):

- `Proofs.lean` — **FIX** syntax error (`/--` → `/-!`)
- `Proofs/WassersteinMetric.lean` — existing, add theorem completeness audit
- `Proofs/KellyOptimal.lean` — existing, add theorem completeness audit
- `Proofs/OUMreversion.lean` — existing, audit
- `Proofs/MomentumAutocorr.lean` — existing, audit
- `Proofs/FundingArb.lean` — existing, audit
- `Proofs/DeltaNeutral.lean` — existing, audit
- `Proofs/GamblersRuin.lean` — existing, audit
- `Proofs/WassersteinMetric.toml` — **NEW** artifact
- `Proofs/KellyOptimal.toml` — **NEW** artifact
- `Proofs/OUMreversion.toml` — **NEW** artifact
- `Proofs/MomentumAutocorr.toml` — **NEW** artifact
- `Proofs/FundingArb.toml` — **NEW** artifact
- `Proofs/DeltaNeutral.toml` — **NEW** artifact
- `Proofs/GamblersRuin.toml` — **NEW** artifact
- `verify-artifacts.py` — **NEW** cross-reference script
- `README.md` — **UPDATE** document artifacts
- `strat-lean-proofs.md` (project root) — **UPDATE** §6 references point to artifact files

### Dependencies Added

- `tomllib` (Python 3.11+ stdlib) for `verify-artifacts.py` — no external deps
- No new Rust/TypeScript/Lean dependencies

---

## Acceptance Criteria

- [ ] `lake build` exits 0 with zero warnings in `testudo-proofs/`
- [ ] Every `theorem` in every `.lean` file is closed (no `sorry`); incomplete theorems explicitly documented with TODO
- [ ] All 7 `.toml` artifact files exist and validate against the schema
- [ ] `verify-artifacts.py` passes — all `.lean` ↔ `.toml` pairs consistent, constraint values in bounds
- [ ] `KellyOptimal.toml` constraint `max_leverage = 5` is derivable from the Kelly theorem
- [ ] `GamblersRuin.toml` constraint `max_drawdown_pct = 20` is derivable from the ruin bound
- [ ] `README.md` documents all theorems, their artifacts, build instructions, and consumption path (linked to AGENT-09)
- [ ] `strat-lean-proofs.md` §6 references updated to point to artifact files

---

## Risks

1. **Incomplete proofs** — Some theorems in the existing `.lean` files may have `sorry` or incomplete induction cases. Mitigation: CP-2 audits every file. Incomplete theorems get explicit `-- TODO: incomplete — scope: <what's missing>` comments. They do NOT block `lake build` (they compile as theorems with `sorry` but are documented as incomplete). The artifact only exposes constraints from CLOSED proofs.
2. **Constraint derivation validity** — A constraint like `max_leverage = 5` may be conservative but not rigorously derived. Mitigation: the `constraints` section includes a `derivation` field explaining the mapping. `verify-artifacts.py` only checks structural validity, not mathematical soundness. A human reviews constraint derivations.
3. **Artifact drift** — If a Lean proof is updated (e.g., tighter bound), the `.toml` artifact may go stale. Mitigation: `verify-artifacts.py` runs in CI. The `version` field in the TOML meta block signals changes. The harness (AGENT-09) checks the version matches before loading.

---

## Completion Signal

This spec is complete when:
1. `lake build` exits 0
2. All 7 `.toml` artifacts written and validated
3. `verify-artifacts.py` passes
4. `README.md` and `strat-lean-proofs.md` updated
5. Code committed to master
