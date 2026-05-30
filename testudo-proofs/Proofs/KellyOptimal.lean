import Mathlib

/-!
@anchor domain:proofs:kelly-optimal
@tags domain

# Kelly Criterion Optimality

The Kelly fraction f* = (b·p - q)/b maximizes the expected geometric
growth rate for sequential independent binary-outcome bets.

Growth function: G(f) = p·log(1 + b·f) + q·log(1 - f)
  where p = win probability, q = 1-p, b = net odds, f = fraction bet

Proof: G(f*) - G(f) = p·log(r) + q·log(s) for r = (1+b·f*)/(1+b·f), s = (1-f*)/(1-f).
Using log(x) ≥ 1 - 1/x: p·log(r) + q·log(s) ≥ 1 - (p/r + q/s).
Algebraic simplification shows p/r + q/s = 1 for ALL f. Therefore G(f*) ≥ G(f).

See `strat-lean-proofs.md` §6.2 and `testudo-exchange/crates/common_utils/src/risk/kelly.rs`.
-/

open Real

/-- Expected log-wealth after one Kelly bet with fraction f.
    Win (probability p): wealth multiplier = 1 + b·f
    Loss (probability q = 1-p): wealth multiplier = 1 - f -/
noncomputable def expected_log_growth (p b f : ℝ) : ℝ :=
  p * log (1 + b * f) + (1 - p) * log (1 - f)

/-- The Kelly fraction: f* = (b·p - q)/b where q = 1-p. -/
noncomputable def kelly_fraction (p b : ℝ) : ℝ :=
  (b * p - (1 - p)) / b

/-- Alternative factorization: 1 + b·f* = p·(b+1). -/
lemma one_plus_b_kelly (p b : ℝ) (hb : b ≠ 0) : 1 + b * kelly_fraction p b = p * (b + 1) := by
  dsimp [kelly_fraction]
  field_simp [hb]
  ring

/-- Alternative factorization: 1 - f* = q·(b+1)/b, where q = 1-p. -/
lemma one_minus_kelly (p b : ℝ) (hb : b ≠ 0) : 1 - kelly_fraction p b = (1 - p) * (b + 1) / b := by
  dsimp [kelly_fraction]
  field_simp [hb]
  ring

/-- log(x) ≥ 1 - 1/x for all x > 0.
    Follows from log(y) ≤ y - 1 (with y = 1/x) which is `Real.log_le_sub_one_of_pos`. -/
lemma log_ge_one_sub_inv {x : ℝ} (hx : 0 < x) : log x ≥ 1 - x⁻¹ := by
  have h_inv : 0 < x⁻¹ := inv_pos.mpr hx
  have h := Real.log_le_sub_one_of_pos h_inv
  rw [log_inv, sub_eq_add_neg] at h
  linarith

/-- The Kelly fraction is positive iff the edge is positive (b·p > 1-p). -/
theorem kelly_positive_iff_edge_positive (p b : ℝ) (hb : 0 < b) :
    0 < kelly_fraction p b ↔ b * p > 1 - p := by
  constructor
  · intro h_pos
    have h_num_pos : 0 < b * p - (1 - p) := by
      rcases (div_pos_iff.mp h_pos) with (⟨hn, hd⟩ | ⟨hn, hd⟩)
      · exact hn
      · linarith
    linarith
  · intro h_edge
    dsimp [kelly_fraction]
    have h_num_pos : 0 < b * p - (1 - p) := by linarith
    exact div_pos h_num_pos hb

/-- The Kelly fraction uniquely maximizes expected log-growth on [0, 1). -/
theorem kelly_maximizes_growth (p b f : ℝ) (hp : 0 < p) (hp_lt_one : p < 1) (hb : 0 < b)
    (hf_nonneg : 0 ≤ f) (hf_lt_one : f < 1) :
    expected_log_growth p b f ≤ expected_log_growth p b (kelly_fraction p b) := by
  set q := 1 - p
  have hq_pos : 0 < q := by linarith
  have hb_ne_zero : b ≠ 0 := by linarith
  set fstar := kelly_fraction p b
  -- Denominators must be positive
  have h_den1_pos : 0 < 1 + b * f := by nlinarith
  have h_den2_pos : 0 < 1 - f := by linarith
  have h_den1star_pos : 0 < 1 + b * fstar := by
    rw [one_plus_b_kelly p b hb_ne_zero]; nlinarith
  have h_den2star_pos : 0 < 1 - fstar := by
    rw [one_minus_kelly p b hb_ne_zero]
    refine div_pos (by nlinarith) hb
  -- Define ratios r = (1+b·f*)/(1+b·f), s = (1-f*)/(1-f)
  set r := (1 + b * fstar) / (1 + b * f) with hr
  set s := (1 - fstar) / (1 - f) with hs
  have hr_pos : 0 < r := div_pos h_den1star_pos h_den1_pos
  have hs_pos : 0 < s := div_pos h_den2star_pos h_den2_pos
  -- Express G(f*) - G(f) = p·log(r) + q·log(s)
  have h_diff : expected_log_growth p b fstar - expected_log_growth p b f = p * log r + q * log s := by
    dsimp [expected_log_growth, r, s, q]
    rw [log_div (by linarith) (by linarith), log_div (by linarith) (by linarith)]
    ring
  -- Apply log(x) ≥ 1 - 1/x to bound the difference from below
  have h_bound : p * log r + q * log s ≥ 0 := by
    have h_log_r : log r ≥ 1 - r⁻¹ := log_ge_one_sub_inv hr_pos
    have h_log_s : log s ≥ 1 - s⁻¹ := log_ge_one_sub_inv hs_pos
    have h_ineq : p * log r + q * log s ≥ p * (1 - r⁻¹) + q * (1 - s⁻¹) := by nlinarith
    -- Simplify: p*(1 - r⁻¹) + q*(1 - s⁻¹) = (p+q) - (p/r + q/s) = 1 - (p/r + q/s)
    have h_simp : p * (1 - r⁻¹) + q * (1 - s⁻¹) = 1 - (p / r + q / s) := by
      ring
    rw [h_simp] at h_ineq
    -- Key algebraic identity: p/r + q/s = 1
    have h_key : p / r + q / s = 1 := by
      dsimp [r, s]
      field_simp [show 1 + b * f ≠ 0 from by linarith, show 1 - f ≠ 0 from by linarith]
      rw [one_plus_b_kelly p b hb_ne_zero, one_minus_kelly p b hb_ne_zero]
      field_simp [hb_ne_zero]
      ring
    rw [h_key] at h_ineq
    nlinarith
  -- Conclude: G(f*) ≥ G(f)
  linarith
