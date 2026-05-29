import Mathlib
open Real

/-- @anchor domain:proofs:ou-mreversion
    @tags domain -/

theorem exp_neg_log_two_pow (n : ℕ) : exp (-(Real.log 2) * (n : ℝ)) = ((1 : ℝ)/2) ^ n := by
  have h_log : Real.log ((1 : ℝ)/2) = -(Real.log 2) := by
    simp
  calc
    exp (-(Real.log 2) * (n : ℝ)) = exp (Real.log ((1 : ℝ)/2) * (n : ℝ)) := by
      rw [← h_log]
    _ = ((1 : ℝ)/2) ^ (n : ℝ) := by rw [Real.rpow_def_of_pos (by norm_num : 0 < (1 : ℝ)/2)]
    _ = ((1 : ℝ)/2) ^ n := by norm_cast

theorem ou_half_life_bound (θ x : ℝ) (n : ℕ) :
    |θ + (x - θ) * exp (-(Real.log 2) * (n : ℝ)) - θ| ≤ |x - θ| / ((2:ℝ)^n) := by
  have h_exp := exp_neg_log_two_pow n
  have h_eq : |θ + (x - θ) * exp (-(Real.log 2) * (n : ℝ)) - θ| = |x - θ| / ((2:ℝ)^n) := by
    have h1 : θ + (x - θ) * exp (-(Real.log 2) * (n : ℝ)) - θ = (x - θ) * exp (-(Real.log 2) * (n : ℝ)) := by ring
    rw [h1, abs_mul, abs_of_nonneg (Real.exp_nonneg _), h_exp]
    field_simp; simp [mul_pow]
  exact le_of_eq h_eq

theorem ou_conditional_expectation (θ x κ t : ℝ) (_hκ : 0 < κ) (_ht : 0 ≤ t) :
    θ + (x - θ) * exp (-κ * t) - θ = (x - θ) * exp (-κ * t) := by ring
