import Mathlib

/-- @anchor domain:proofs:main
    @tags domain -/

-- Main entry point for the testudo-proofs verification layer.
-- All proof modules are imported via Proofs.lean.
-- Run `lake build` to verify all theorems.

def main : IO Unit :=
  IO.println "testudo-proofs: Lean 4 verification layer for Testudo autonomous trading strategies"
