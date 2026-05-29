import Mathlib

/-- @anchor domain:proofs:wasserstein
    @tags domain -/

/-!
# 1-Wasserstein Metric on ℝ (Empirical Distributions)

For sorted lists xs, ys of equal length n:
  W₁(xs, ys) = (1/n) · Σᵢ |xs[i] - ys[i]|

Proves W₁ is a metric: non-negative, symmetric, triangle inequality.
See `strat-lean-proofs.md` §6.1.
-/

/-- Sum of pairwise absolute differences. Returns 0 when lengths differ. -/
def W1_aux : List ℝ → List ℝ → ℝ
  | [], [] => 0
  | x :: xs, y :: ys => |x - y| + W1_aux xs ys
  | _, _ => 0

lemma W1_aux_nonneg (xs ys : List ℝ) : 0 ≤ W1_aux xs ys := by
  induction xs generalizing ys with
  | nil => cases ys <;> simp [W1_aux]
  | cons x xs ih =>
    match ys with
    | [] => simp [W1_aux]
    | y :: ys' =>
      simp [W1_aux]
      nlinarith [abs_nonneg (x - y), ih ys']

lemma W1_aux_symm (xs ys : List ℝ) (h_len : xs.length = ys.length) : W1_aux xs ys = W1_aux ys xs := by
  induction xs generalizing ys with
  | nil =>
    cases ys with
    | nil => rfl
    | cons y ys' => simp at h_len
  | cons x xs ih =>
    cases ys with
    | nil => simp at h_len
    | cons y ys' =>
      simp [W1_aux]
      rw [abs_sub_comm x y]
      have h_len' : xs.length = ys'.length := by simpa using h_len
      rw [ih ys' h_len']

noncomputable def W1_emp (xs ys : List ℝ) : ℝ :=
  if xs.length = ys.length then W1_aux xs ys / (xs.length : ℝ) else 0

theorem W1_emp_nonneg (xs ys : List ℝ) : 0 ≤ W1_emp xs ys := by
  unfold W1_emp
  split
  · exact div_nonneg (W1_aux_nonneg _ _) (Nat.cast_nonneg _)
  · rfl

theorem W1_emp_symm (xs ys : List ℝ) (h_len : xs.length = ys.length) : W1_emp xs ys = W1_emp ys xs := by
  unfold W1_emp
  simp [h_len, W1_aux_symm xs ys h_len]

lemma W1_aux_triangle (xs ys zs : List ℝ) (h_xy : xs.length = ys.length) (h_yz : ys.length = zs.length) :
    W1_aux xs zs ≤ W1_aux xs ys + W1_aux ys zs := by
  induction xs generalizing ys zs with
  | nil =>
    cases ys with
    | nil =>
      cases zs with
      | nil => simp [W1_aux]
      | cons z zs' => simp at h_yz
    | cons y ys' => simp at h_xy
  | cons x xs ih =>
    cases ys with
    | nil => simp at h_xy
    | cons y ys' =>
      cases zs with
      | nil => simp at h_yz
      | cons z zs' =>
        simp [W1_aux]
        have h_tri : |x - z| ≤ |x - y| + |y - z| := by
          have h_eq : x - z = (x - y) + (y - z) := by ring
          rw [h_eq]
          exact abs_add_le (x - y) (y - z)
        have h_xy' : xs.length = ys'.length := by simpa using h_xy
        have h_yz' : ys'.length = zs'.length := by simpa using h_yz
        have h_ih := ih ys' zs' h_xy' h_yz'
        nlinarith

theorem W1_emp_triangle (xs ys zs : List ℝ) (h_xy : xs.length = ys.length) (h_yz : ys.length = zs.length) :
    W1_emp xs zs ≤ W1_emp xs ys + W1_emp ys zs := by
  have h_xz : xs.length = zs.length := by rw [h_xy, h_yz]
  have h_sum := W1_aux_triangle xs ys zs h_xy h_yz
  unfold W1_emp
  -- Replace conditionals with true branch. simp normalizes all denominators to zs.length.
  simp [h_xy, h_yz]
  set n := (zs.length : ℝ) with hn
  have hn_nonneg : 0 ≤ n := Nat.cast_nonneg _
  calc
    W1_aux xs zs / n ≤ (W1_aux xs ys + W1_aux ys zs) / n :=
      div_le_div_of_nonneg_right h_sum hn_nonneg
    _ = W1_aux xs ys / n + W1_aux ys zs / n := by ring
