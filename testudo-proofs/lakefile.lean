import Lake
open Lake DSL

/-- @anchor domain:proofs:lakefile
    @tags domain -/

package «testudo-proofs» where
  leanOptions := #[
    ⟨`pp.unicode.fun, true⟩
  ]

@[default_target]
lean_lib «Proofs» where

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git"
