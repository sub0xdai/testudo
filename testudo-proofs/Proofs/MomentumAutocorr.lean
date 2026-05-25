import Mathlib

/-!
# Momentum Autocorrelation

2-point equally-weighted model. cov2 = (x₁-x₂)(y₁-y₂)/4.

cov2 > 0 → conditional expectation above mean > unconditional mean (momentum).
cov2 < 0 → conditional expectation above mean < unconditional mean (mean-reversion).

See `strat-lean-proofs.md` §6.4.
-/

/-- Covariance for 2-point equally-weighted distribution. -/
noncomputable def cov2 (x₁ x₂ y₁ y₂ : ℝ) : ℝ :=
  ((x₁ - (x₁ + x₂)/2) * (y₁ - (y₁ + y₂)/2) + (x₂ - (x₁ + x₂)/2) * (y₂ - (y₁ + y₂)/2)) / 2

/-- Conditional expectation of y given x above its mean. -/
noncomputable def cond_exp_above (x₁ x₂ y₁ y₂ : ℝ) : ℝ :=
  if x₁ > (x₁ + x₂)/2 ∧ x₂ > (x₁ + x₂)/2 then (y₁ + y₂)/2
  else if x₁ > (x₁ + x₂)/2 then y₁
  else if x₂ > (x₁ + x₂)/2 then y₂
  else (y₁ + y₂)/2

/-- Algebraic simplification: cov2 = (x₁-x₂)(y₁-y₂)/4. -/
lemma cov2_simplified (x₁ x₂ y₁ y₂ : ℝ) : cov2 x₁ x₂ y₁ y₂ = (x₁ - x₂) * (y₁ - y₂) / 4 := by
  dsimp [cov2]; ring

/-- Positive covariance → x and y same-sign ordered. -/
lemma cov2_pos_same_sign {x₁ x₂ y₁ y₂ : ℝ} (h : cov2 x₁ x₂ y₁ y₂ > 0) :
    (x₁ > x₂ ∧ y₁ > y₂) ∨ (x₁ < x₂ ∧ y₁ < y₂) := by
  rw [cov2_simplified] at h
  have hpos : 0 < (x₁ - x₂) * (y₁ - y₂) := by linarith
  have hx_ne_zero : x₁ - x₂ ≠ 0 := by
    intro hzero; rw [hzero, zero_mul] at hpos; linarith
  have hy_ne_zero : y₁ - y₂ ≠ 0 := by
    intro hzero; rw [hzero, mul_zero] at hpos; linarith
  rcases lt_or_gt_of_ne hx_ne_zero with (hx_neg | hx_pos)
  · -- x₁ - x₂ < 0
    have hy_neg : y₁ - y₂ < 0 := by nlinarith
    right; constructor <;> linarith
  · -- x₁ - x₂ > 0
    have hy_pos : 0 < y₁ - y₂ := by nlinarith
    left; constructor <;> linarith

/-- cov2 > 0 → conditional exp above mean > unconditional mean (momentum). -/
theorem momentum_pos_cov (x₁ x₂ y₁ y₂ : ℝ) (h_cov : cov2 x₁ x₂ y₁ y₂ > 0) :
    cond_exp_above x₁ x₂ y₁ y₂ > (y₁ + y₂)/2 := by
  rcases cov2_pos_same_sign h_cov with (⟨hx, hy⟩ | ⟨hx, hy⟩)
  · -- x₁ > x₂, y₁ > y₂: only x₁ > mean, cond_exp = y₁ > mean
    have hx1_above : x₁ > (x₁ + x₂)/2 := by linarith
    have hx2_not : ¬ (x₂ > (x₁ + x₂)/2) := by linarith
    dsimp [cond_exp_above]
    simp [hx1_above, hx2_not]
    linarith
  · -- x₁ < x₂, y₁ < y₂: only x₂ > mean, cond_exp = y₂ > mean
    have hx1_not : ¬ (x₁ > (x₁ + x₂)/2) := by linarith
    have hx2_above : x₂ > (x₁ + x₂)/2 := by linarith
    dsimp [cond_exp_above]
    simp [hx1_not, hx2_above]
    linarith

/-- Negative covariance → x and y opposite-sign ordered. -/
lemma cov2_neg_opp_sign {x₁ x₂ y₁ y₂ : ℝ} (h : cov2 x₁ x₂ y₁ y₂ < 0) :
    (x₁ > x₂ ∧ y₁ < y₂) ∨ (x₁ < x₂ ∧ y₁ > y₂) := by
  rw [cov2_simplified] at h
  have hneg : (x₁ - x₂) * (y₁ - y₂) < 0 := by linarith
  have hx_ne_zero : x₁ - x₂ ≠ 0 := by
    intro hzero; rw [hzero, zero_mul] at hneg; linarith
  have hy_ne_zero : y₁ - y₂ ≠ 0 := by
    intro hzero; rw [hzero, mul_zero] at hneg; linarith
  rcases lt_or_gt_of_ne hx_ne_zero with (hx_neg | hx_pos)
  · -- x₁ - x₂ < 0 → y₁ - y₂ > 0 (opposite signs)
    have hy_pos : 0 < y₁ - y₂ := by nlinarith
    right; constructor <;> linarith
  · -- x₁ - x₂ > 0 → y₁ - y₂ < 0
    have hy_neg : y₁ - y₂ < 0 := by nlinarith
    left; constructor <;> linarith

/-- cov2 < 0 → conditional exp below mean (mean-reversion). -/
theorem momentum_neg_cov (x₁ x₂ y₁ y₂ : ℝ) (h_cov : cov2 x₁ x₂ y₁ y₂ < 0) :
    cond_exp_above x₁ x₂ y₁ y₂ < (y₁ + y₂)/2 := by
  rcases cov2_neg_opp_sign h_cov with (⟨hx, hy⟩ | ⟨hx, hy⟩)
  · -- x₁ > x₂, y₁ < y₂: cond_exp = y₁ < mean
    have hx1_above : x₁ > (x₁ + x₂)/2 := by linarith
    have hx2_not : ¬ (x₂ > (x₁ + x₂)/2) := by linarith
    dsimp [cond_exp_above]
    simp [hx1_above, hx2_not]
    linarith
  · -- x₁ < x₂, y₁ > y₂: cond_exp = y₂ < mean
    have hx1_not : ¬ (x₁ > (x₁ + x₂)/2) := by linarith
    have hx2_above : x₂ > (x₁ + x₂)/2 := by linarith
    dsimp [cond_exp_above]
    simp [hx1_not, hx2_above]
    linarith
