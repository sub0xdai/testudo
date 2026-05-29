import Mathlib

/-- @anchor domain:proofs:delta-neutral
    @tags domain -/

def portfolio_delta (positions : List (ℝ × ℝ)) : ℝ :=
  (positions.map (λ (s, d) => s * d)).sum

theorem hedge_achieves_neutrality (positions : List (ℝ × ℝ)) :
    let Δ := portfolio_delta positions
    portfolio_delta ((|Δ|, -SignType.sign Δ) :: positions) = 0 := by
  intro Δ
  dsimp [portfolio_delta]
  simp
  have hsum : (positions.map (λ (s, d) => s * d)).sum = Δ := rfl
  rw [hsum]
  have h : |Δ| * (-SignType.sign Δ) + Δ = 0 := by
    rw [show |Δ| * (-SignType.sign Δ) = -(|Δ| * SignType.sign Δ) by simp]
    simp
  simpa using h
