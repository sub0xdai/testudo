# Specification: Lean 4 Strategy Proofs — Formal Verification Layer

**Spec ID:** STRAT-01-lean-proofs
**Date:** 2026-05-25
**Status:** Draft
**Class:** Core / Verification
**Priority:** P1 — `strat-lean-proofs.md` references these proofs as the trust anchor for LLM trading decisions; the proofs don't exist as compilable files yet
**Depends on:** None (greenfield Lean 4 project)
**Series:** STRAT-01 (Strategy Verification) — distinct from AGENT (agent infrastructure) and CLN (code cleanup)

---

## Problem Statement

`strat-lean-proofs.md` (root, 1,256 lines) provides an LLM-facing strategy document with 7 mathematical theorems as Lean 4 proof sketches. These sketches are pseudocode — they contain `sorry` placeholders, reference undefined functions (`expected_growth`, `pnl_of_position`, `cond_exp_gt_on_positive_correlation`), and would not compile if fed to `lake build`.

The architecture specification in `strat-lean-proofs.md` §1 defines Lean 4 as:

> "Lean 4 (Verification Layer): Static environment housing formal proofs for the p-Wasserstein metric and Kantorovich duality. Eliminates LLM mathematical hallucination."

The state-context pipeline step 1 says:

> "Verification (Offline): Hermes synthesizes strategy module → Evaluates against `optimal_transport.lean` → Outputs formally verified logic."

But there's no `optimal_transport.lean` file. There's no Lean 4 project at all. The verification layer is a prose claim, not compilable code.

An LLM reading `strat-lean-proofs.md` cannot actually run `lake build` to verify the theorems. If a theorem has a subtle error in the pseudocode (and some of them do — the gambler's ruin proof has a `sorry`, the Kelly growth function is undefined), the LLM has no way to detect it. This defeats the purpose of "eliminates LLM mathematical hallucination" — the LLM is trusting pseudocode, not verified proofs.

This spec creates the verification layer. A `testudo-proofs/` directory with a proper Lean 4 project, Mathlib dependencies, and **compilable proofs** for all 7 theorems. Every theorem passes `lake build` with zero `sorry` statements.

---

## User Stories

- **As an LLM (Hermes, OpenClaw, pi)**, I want to run `lake build testudo-proofs` and verify that the strategy theorems are correct, so that I can trust the formulas I'm using for position sizing, stop placement, and regime detection.
- **As a strategy developer**, I want machine-checked proofs that the Kelly criterion maximizes geometric growth, so that I don't have to trust an LLM's math.
- **As a Testudo user**, I want to know that the autonomous agent's decision framework is backed by verifiable mathematics, not just prose claims.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `testudo-proofs/` directory with a `lakefile.lean` and `lean-toolchain` | High | Project scaffold |
| FR-2 | Mathlib4 dependency declared in `lakefile.lean` | High | Dependencies |
| FR-3 | `WassersteinMetric.lean` — proves W₁ is a metric (non-negativity, symmetry, triangle inequality) for empirical distributions on ℝ | High | Proofs |
| FR-4 | `KellyOptimal.lean` — proves Kelly fraction f* uniquely maximizes E[log(wealth)] for binary-outcome bets via Jensen's inequality | High | Proofs |
| FR-5 | `OUMreversion.lean` — proves OU process conditional expectation and half-life deviation bound | Medium | Proofs |
| FR-6 | `MomentumAutocorr.lean` — proves positive autocorrelation implies positive expected conditional return, negative autocorrelation implies negative | Medium | Proofs |
| FR-7 | `FundingArb.lean` — proves the no-arbitrage bound for perpetual futures and deterministic P&L of delta-neutral funding positions | Medium | Proofs |
| FR-8 | `DeltaNeutral.lean` — proves a portfolio of linear instruments is delta-neutral iff net signed size = 0, and a single opposing hedge achieves neutrality | Medium | Proofs |
| FR-9 | `GamblersRuin.lean` — proves the gambler's ruin upper bound for sequential fractional betting (no `sorry` — complete proof or explicitly stated assumptions) | Medium | Proofs |
| FR-10 | `Main.lean` imports all proof modules; `lake build` exits 0 with zero warnings | High | Build gate |
| FR-11 | All proofs use `theorem` keyword (not `example` or `def`); all are closed (no `sorry`, no `admit`) | High | Proofs |
| FR-12 | README in `testudo-proofs/` documents: what each file proves, how it connects to `strat-lean-proofs.md`, and how to build | Low | Docs |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Scaffold `testudo-proofs/` project with `lake new`, Mathlib dependency, `Main.lean` that imports nothing but compiles | `lake build` exits 0 |
| CP-2 | `WassersteinMetric.lean` — proves W₁ metric properties for empirical 1D distributions | Theorem compiles, `#check` confirms types |
| CP-3 | `KellyOptimal.lean` — proves Kelly optimality (derivative → stationary point → maximum) | Theorem compiles, verifies f* formula |
| CP-4 | `OUMreversion.lean` + `MomentumAutocorr.lean` — OU bounds + autocorrelation implications | Both modules compile |
| CP-5 | `FundingArb.lean` + `DeltaNeutral.lean` — no-arbitrage bound + delta neutrality | Both modules compile |
| CP-6 | `GamblersRuin.lean` — ruin bound proof (closed, no `sorry`) | Theorem compiles with all assumptions explicit |
| CP-7 | Integration: `Main.lean` imports all 7 modules, `lake build` passes, README written | Full build gate |

### Project Structure

```
testudo-proofs/
├── lakefile.lean              # Lean 4 project config, Mathlib dependency
├── lean-toolchain             # Pins Lean version (4.29.1)
├── Main.lean                  # Imports all proof modules, verifies they compile
├── Proofs/
│   ├── WassersteinMetric.lean # §6.1 — W₁ is a metric on ℝ
│   ├── KellyOptimal.lean      # §6.2 — Kelly criterion optimality
│   ├── OUMreversion.lean      # §6.3 — OU process mean reversion bounds
│   ├── MomentumAutocorr.lean  # §6.4 — Autocorrelation → expected return direction
│   ├── FundingArb.lean        # §6.5 — Funding rate no-arbitrage bound
│   ├── DeltaNeutral.lean      # §6.6 — Portfolio delta neutrality
│   └── GamblersRuin.lean      # §6.7 — Sequential betting ruin probability bound
└── README.md                  # Build instructions, theorem index, connection to strat-lean-proofs.md
```

### lakefile.lean Template

```lean4
import Lake
open Lake DSL

package «testudo-proofs» where
  leanOptions := #[
    ⟨`pp.unicode.fun, true⟩
  ]

@[default_target]
lean_lib «Proofs» where

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git"
```

### Theorem Specifications

Each `.lean` file must satisfy:

1. **No `sorry`** — every `theorem` is closed with an actual proof term.
2. **Explicit hypotheses** — assumptions (e.g., p in (0,1), b > 0) are stated as arguments.
3. **Type-correct** — `#check` on every theorem confirms the statement type.
4. **Naming convention** — theorems are named as in `strat-lean-proofs.md` §6, preserving the same mathematical content.
5. **Self-contained imports** — each file imports only what it needs from Mathlib.

#### CP-2 (Wasserstein): Content Specification

```lean4
import Mathlib

open MeasureTheory ProbabilityTheory

/-- Empirical 1-Wasserstein distance between two sorted lists of ℝ samples. -/
noncomputable def W1_emp (xs ys : List ℝ) : ℝ :=
  -- Mean absolute difference at matching quantile indices
  ...

/-- W1_emp is non-negative. -/
theorem W1_emp_nonneg (xs ys : List ℝ) : 0 ≤ W1_emp xs ys := ...

/-- W1_emp is symmetric. -/
theorem W1_emp_symm (xs ys : List ℝ) (h : xs.length = ys.length) : W1_emp xs ys = W1_emp ys xs := ...

/-- W1_emp satisfies the triangle inequality. -/
theorem W1_emp_triangle (xs ys zs : List ℝ) (h_xyz : xs.length = ys.length ∧ ys.length = zs.length) : W1_emp xs zs ≤ W1_emp xs ys + W1_emp ys zs := ...
```

#### CP-3 (Kelly): Content Specification

```lean4
import Mathlib

/-- Expected log-wealth after one Kelly bet with fraction f, win prob p, net odds b. -/
noncomputable def expected_log_growth (p b f : ℝ) : ℝ :=
  p * Real.log (1 + b * f) + (1 - p) * Real.log (1 - f)

/-- The derivative of expected_log_growth with respect to f. -/
theorem growth_derivative (p b f : ℝ) (hf : f ≠ 1) (hfb : 1 + b * f > 0) : ... := ...

/-- The unique stationary point of expected_log_growth is the Kelly fraction. -/
theorem kelly_stationary (p b : ℝ) (hp : 0 < p) (hp' : p < 1) (hb : 0 < b) :
    f = (b * p - (1 - p)) / b ↔ growth_derivative p b f = 0 := ...

/-- The Kelly fraction is positive iff the edge is positive. -/
theorem kelly_positive_iff_edge_positive (p b : ℝ) (hb : 0 < b) :
    0 < (b * p - (1 - p)) / b ↔ b * p > 1 - p := ...

/-- The Kelly fraction uniquely maximizes expected_log_growth for f ∈ [0, 1). -/
theorem kelly_maximizes_growth (p b : ℝ) (hp : 0 < p) (hp' : p < 1) (hb : 0 < b) :
    ∀ (f : ℝ), 0 ≤ f → f < 1 → expected_log_growth p b f ≤ expected_log_growth p b ((b * p - (1 - p)) / b) := ...
```

#### CP-4 (OU + Momentum): Content Specification

`OUMreversion.lean`:
```lean4
/-- Expected value of OU process at time t given initial value x. -/
theorem ou_conditional_expectation (κ θ x t : ℝ) (hκ : 0 < κ) (ht : 0 ≤ t) :
    θ + (x - θ) * Real.exp (-κ * t) - θ = (x - θ) * Real.exp (-κ * t) := ...

/-- After n half-lives, deviation from mean is at most |x-θ| / 2^n. -/
theorem ou_half_life_bound (κ θ x : ℝ) (n : ℕ) (hκ : 0 < κ) :
    |(θ + (x - θ) * Real.exp (-κ * ((Real.log 2 / κ) * (n : ℝ)))) - θ| ≤ |x - θ| / ((2:ℝ)^n) := ...
```

`MomentumAutocorr.lean`:
```lean4
/-- For two random variables with positive covariance, conditioning on X > E[X] 
    increases the expectation of Y. -/
theorem cond_exp_gt_on_positive_cov (X Y : Ω → ℝ) (h_cov : cov X Y > 0) ... : ... := ...

/-- If returns have positive lag-1 autocorrelation, a positive return at t 
    predicts positive expected return at t+1. -/
theorem momentum_positive_autocorr (r : ℕ → ℝ) (h_ρ₁ : ρ₁ r > 0) ... : ... := ...
```

#### CP-5 (Funding + Delta): Content Specification

`FundingArb.lean`:
```lean4
/-- No-arbitrage bound: |F - S| ≤ S·|r|·Δt + ε ↔ no arbitrage profit exists. -/
theorem funding_no_arbitrage_bound (F S r ε : ℝ) (Δt : ℝ) (h_ε : 0 < ε) (h_Δt : 0 ≤ Δt) :
    (∃ profit > 0, arb_pnl F S r ε Δt profit) ↔ |F - S| ≤ S * |r| * Δt + ε := ...

/-- Delta-neutral funding position P&L = Q·|r|·Δt - fees, independent of price. -/
theorem delta_neutral_funding_pnl (Q r Δt fees : ℝ) (h_delta_nil : delta = 0) :
    pnl = Q * |r| * Δt - fees := ...
```

`DeltaNeutral.lean`:
```lean4
/-- Portfolio delta for linear instruments is summed signed position size. -/
theorem portfolio_delta_linear (positions : List (ℝ × ℝ)) :
    portfolio_delta positions = (positions.map (λ (s, d) => s * d)).sum := ...

/-- A portfolio is delta-neutral iff net signed size = 0. -/
theorem delta_neutral_iff (positions : List (ℝ × ℝ)) :
    portfolio_delta positions = 0 ↔ ... := ...

/-- Adding a single opposing position of size |Δ| achieves delta neutrality. -/
theorem hedge_achieves_neutrality (positions : List (ℝ × ℝ)) :
    let Δ := (positions.map (λ (s, d) => s * d)).sum
    portfolio_delta ((|Δ|, -sign Δ) :: positions) = 0 := ...
```

#### CP-6 (Gambler's Ruin): Content Specification

This is the hardest theorem. The pseudocode in `strat-lean-proofs.md` has a `sorry`. The spec requires a **closed proof** — if a complete proof from first principles is infeasible for this scope, the proof must explicitly state its assumptions and prove the bound from those assumptions.

Accepted approaches:
- Prove the bound for a **symmetric random walk** (fair coin) as a special case.
- Or: state the optional stopping theorem as a hypothesis and derive the bound.
- Or: prove the bound for a **binary betting model** with explicit win/loss probabilities.

```lean4
/-- For a symmetric random walk with step size ±1 (fair coin), the probability 
    of hitting -a before hitting +b starting from 0 is b/(a+b). -/
theorem symmetric_ruin_probability (a b : ℕ) (ha : 0 < a) (hb : 0 < b) :
    ℙ[ruin before a b] = (b : ℝ) / ((a + b) : ℝ) := ...

/-- For fractional Kelly betting with fraction f, the probability of drawdown 
    to fraction α before reaching fraction β is bounded. -/
theorem kelly_drawdown_bound (f α β : ℝ) (hf_pos : 0 < f) (hf_lt : f < 1) ... :
    ℙ[drawdown_to α before β] ≤ ((1 - f) / (1 + f)) ^ (Real.log (1 / α) / f) := ...
```

### Paved Roads

- **Existing:** `strat-lean-proofs.md` §6 has pseudocode proofs for all 7 theorems. These serve as blueprints — implement the real proofs, not the pseudocode.
- **Existing:** Lean 4.29.1 + Mathlib4 are installed on this system (`lake --version` reports 5.0.0). No toolchain installation needed.
- **Pattern:** The TigerBeetle project in the repo uses formal methods (though it's Zig, not Lean). The project already values correctness proofs.
- **Existing:** `scripts/vox.sh` supports `/skill:vox plan STRAT-01-lean-proofs` and `/skill:vox build STRAT-01-lean-proofs`.

### Files

All files are new — greenfield Lean 4 project:

```
testudo-proofs/
├── lakefile.lean              # Lean 4 project config
├── lean-toolchain             # Pins to leanprover/lean4:v4.29.1
├── Main.lean                  # Import all proof modules
├── Proofs/
│   ├── WassersteinMetric.lean # FR-3
│   ├── KellyOptimal.lean      # FR-4
│   ├── OUMreversion.lean      # FR-5
│   ├── MomentumAutocorr.lean  # FR-6
│   ├── FundingArb.lean        # FR-7
│   ├── DeltaNeutral.lean      # FR-8
│   └── GamblersRuin.lean      # FR-9
└── README.md                  # FR-12
```

### Dependencies Added

- **Mathlib4** — the Lean theorem proving library. Already cached locally if previously built. Declared in `lakefile.lean` as a git dependency.
- No Rust/TypeScript/Python dependencies. This is a standalone Lean 4 project.

---

## Acceptance Criteria

### Build Gate
- [ ] `lake new testudo-proofs` succeeds (or manual scaffold with equivalent structure)
- [ ] `lake build` exits 0 with all 7 proof modules imported in `Main.lean`
- [ ] `lake build` produces zero warnings (no unused variables, no deprecated imports)

### Individual Theorems
- [ ] `WassersteinMetric.lean`: `#check W1_emp_nonneg`, `#check W1_emp_symm`, `#check W1_emp_triangle` all succeed
- [ ] `KellyOptimal.lean`: `#check kelly_maximizes_growth` succeeds; theorem correctly proves f* = (bp-q)/b uniquely maximizes E[log(1+fX)]
- [ ] `OUMreversion.lean`: `#check ou_half_life_bound` succeeds; bound shows |deviation| ≤ |initial|/2ⁿ after n half-lives
- [ ] `MomentumAutocorr.lean`: `#check momentum_positive_autocorr` succeeds; correctly handles covariance condition
- [ ] `FundingArb.lean`: `#check funding_no_arbitrage_bound` succeeds; correctly states iff condition
- [ ] `DeltaNeutral.lean`: `#check hedge_achieves_neutrality` succeeds; constructing the hedge yields Δ=0
- [ ] `GamblersRuin.lean`: `#check kelly_drawdown_bound` succeeds; closed proof (no `sorry`)

### Integrity
- [ ] All theorems use `theorem` keyword, not `example`
- [ ] Zero `sorry` or `admit` calls in any file
- [ ] Each file's theorems match their corresponding §6 pseudocode in `strat-lean-proofs.md`
- [ ] `README.md` exists with: build instructions (`lake build`), theorem index, and link back to `strat-lean-proofs.md`

### Spec Integration
- [ ] `strat-lean-proofs.md` updated: §6 references changed from inline pseudocode to "See `testudo-proofs/Proofs/[Name].lean` for the verifiable proof"
- [ ] `.specify/specs/notes.md` lists STRAT-01 as in-progress

---

## Risks

1. **Gambler's Ruin complexity** — A complete proof from first principles requires stochastic calculus (Itô's lemma, optional stopping theorem for continuous-time martingales). This may be beyond scope for a Lean 4 proof.
   - **Mitigation A:** Prove the discrete symmetric random walk case (coin-flip model), which is tractable with elementary combinatorics.
   - **Mitigation B:** State Doob's optional stopping theorem as a hypothesis (import via Mathlib) and derive the bound from it.
   - **Decision:** CP-6 accepts either approach. A closed proof for the discrete case or a proof that reduces the bound to an existing Mathlib theorem. Do not leave `sorry`.

2. **Mathlib compilation time** — First `lake build` with Mathlib downloads and compiles ~1GB of dependencies. This could take 20–40 minutes on first build.
   - **Mitigation:** Use `LAKE_JOBS=4` for parallel builds. Check if Mathlib is cached from previous Lean usage on this system. If not, CP-1 should run before other CPs and the build time is accounted for.

3. **Theorem scope mismatch** — The pseudocode in `strat-lean-proofs.md` is more ambitious than what can be proven in Lean 4 without a full finance library. Some theorems (e.g., Kelly optimality for arbitrary return distributions) require probability theory that Mathlib's `ProbabilityTheory` module may not fully support yet.
   - **Mitigation:** Scope each theorem to its simplest useful form. Kelly: binary outcome (win/loss). OU: deterministic bound on conditional expectation (no stochastic calculus needed). Momentum: correlation implication only.

4. **Toolchain version drift** — `lean-toolchain` pins 4.29.1. If Mathlib advances past this version, `lake build` may fail.
   - **Mitigation:** Pin the Mathlib commit hash in `lakefile.lean`. Accept that this verification layer is a snapshot — the proofs are correct for this version. Upgrade is a future spec.

---

## Completion Signal

This spec is complete when:
1. `testudo-proofs/` directory exists with a valid Lean 4 project
2. All 7 proof files contain closed `theorem` statements (no `sorry`)
3. `lake build` exits 0 with zero warnings
4. `strat-lean-proofs.md` §6 updated to reference the actual proof files
5. README documents the verification layer
6. `.specify/specs/notes.md` updated
7. Code committed to master
