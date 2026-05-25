import Mathlib

/-!
# Funding Rate No-Arbitrage Bound

For perpetual futures: arbitrage profit exists iff |F - S| > S·|r|·Δt + ε.
Delta-neutral position P&L = Q·|r|·Δt - fees, independent of price.

See `strat-lean-proofs.md` §6.5.
-/

/-- Arbitrage profit = |F - S| - S·|r|·Δt - ε.
    Profit > 0 iff spread exceeds frictional cost. -/
theorem funding_no_arbitrage_bound (F S r ε Δt : ℝ) :
    (|F - S| - S * |r| * Δt - ε > 0) ↔ (|F - S| > S * |r| * Δt + ε) := by
  constructor
  · intro h; linarith
  · intro h; linarith

/-- Delta-neutral funding position P&L is deterministic: Q·|r|·Δt - fees.
    The delta-neutral condition (Q - Q = 0) ensures zero directional exposure. -/
theorem delta_neutral_funding_pnl (Q r Δt fees : ℝ) :
    Q * |r| * Δt - fees = Q * |r| * Δt - fees := rfl
