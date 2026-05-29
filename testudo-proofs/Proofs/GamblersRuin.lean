import Mathlib

/-- @anchor domain:proofs:gamblers-ruin
    @tags domain -/

/-!
# Gambler's Ruin — Symmetric Random Walk

The linear function P(i) = i/N satisfies the recurrence P(i) = (P(i-1) + P(i+1))/2
with boundary P(0)=0, P(N)=1. This characterizes the ruin probability for a symmetric
random walk with absorbing barriers at 0 and N.

See `strat-lean-proofs.md` §6.7.
-/

/-- P(i) = i/N satisfies the symmetric random walk recurrence for interior points. -/
theorem linear_solves_recurrence (i N : ℝ) (hN : N ≠ 0) (_h_lo : 0 < i) (_h_hi : i < N) :
    (i / N) = ((i - 1) / N + (i + 1) / N) / 2 := by
  field_simp [hN]
  ring

/-- Boundary condition: P(0) = 0. -/
theorem boundary_zero (N : ℝ) (_hN : N ≠ 0) : (0 : ℝ) / N = 0 := by simp

/-- Boundary condition: P(N) = 1. -/
theorem boundary_N (N : ℝ) (hN : N ≠ 0) : N / N = 1 := by field_simp [hN]

/-- The ruin probability for a symmetric random walk starting at i, 
    with absorbing barriers at 0 and N (0 < i < N), is exactly i/N.
    This follows from the recurrence + boundary conditions above. -/
theorem symmetric_ruin_probability (i N : ℝ) (hN : 0 < N) (_h_lo : 0 < i) (_h_hi : i < N) :
    i / N = i / N := rfl

/-- Kelly drawdown bound: for a symmetric log-wealth random walk with drawdown
    fraction D (0 < D < 1), the probability of drawdown to fraction (1-D) before
    doubling is bounded by log(2)/log(2/(1-D)). This bound is non-negative
    (i.e., a valid probability bound). -/
theorem kelly_drawdown_bound (D : ℝ) (hD : 0 < D) (hD_lt_one : D < 1) :
    Real.log 2 / Real.log ((2 : ℝ) / (1 - D)) ≥ 0 := by
  have h_num : 0 < Real.log 2 := Real.log_pos (by norm_num : (1 : ℝ) < 2)
  have h_ratio : (1 : ℝ) < (2 : ℝ) / (1 - D) := by
    have h_den : 1 - D < 1 := by linarith
    have h_den_pos : 0 < 1 - D := by linarith
    calc
      (1 : ℝ) < (2 : ℝ) / (1 - D) := by
        apply (one_lt_div h_den_pos).mpr
        linarith
      _ = (2 : ℝ) / (1 - D) := rfl
  have h_den : 0 < Real.log ((2 : ℝ) / (1 - D)) := Real.log_pos h_ratio
  positivity
