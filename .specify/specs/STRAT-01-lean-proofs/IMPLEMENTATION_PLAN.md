# STRAT-01-lean-proofs — Implementation Plan

## Current State Summary

`strat-lean-proofs.md` (root, 1,256 lines) documents 7 mathematical theorems as Lean 4 proof sketches for LLM consumption. These are pseudocode — they contain `sorry` placeholders and undefined functions. No compilable Lean 4 project exists. The architecture spec references a "Lean 4 Verification Layer" that doesn't exist as code.

Lean 4.29.1 + Lake 5.0.0 are installed on this system. Mathlib4 is available as a git dependency. The `testudo-proofs/` directory will be a new top-level directory following existing naming conventions (`testudo-exchange/`, `testudo-journal/`, etc.).

All work is greenfield — 8 new files in a single directory. Licensed AGPL-3.0.

## Checkpoints

### CP-1: Scaffold Lean 4 project ✅
- **Touches**: `testudo-proofs/` (new directory — `lakefile.lean`, `lean-toolchain`, `Main.lean`, `Proofs.lean`, `LICENSE`)
- Completed 2026-05-25 by /skill:vox build
- **Tasks**:
  1. Run `lake new testudo-proofs` or manually create project skeleton
  2. Add Mathlib4 dependency to `lakefile.lean`
  3. Create `lean-toolchain` pinning `leanprover/lean4:v4.29.1`
  4. Create `Main.lean` that imports Mathlib (empty `import Mathlib` is sufficient)
  5. Add `LICENSE` with AGPL-3.0 text
- **Verification**: `cd testudo-proofs && lake build` exits 0
- **Commit message**: `feat: scaffold testudo-proofs Lean 4 project`

### CP-2: WassersteinMetric.lean ✅
- **Touches**: `testudo-proofs/Proofs/WassersteinMetric.lean`, `testudo-proofs/Proofs.lean`
- Completed 2026-05-25 by /skill:vox build

### CP-3: KellyOptimal.lean ✅
- **Touches**: `testudo-proofs/Proofs/KellyOptimal.lean`, `testudo-proofs/Proofs.lean`
- Completed 2026-05-25 by /skill:vox build

### CP-4: OUMreversion.lean + MomentumAutocorr.lean ✅
- **Touches**: `testudo-proofs/Proofs/OUMreversion.lean`, `testudo-proofs/Proofs/MomentumAutocorr.lean`, `testudo-proofs/Proofs.lean`
- Completed 2026-05-25 by /skill:vox build

### CP-5: FundingArb.lean + DeltaNeutral.lean ✅
- **Touches**: `testudo-proofs/Proofs/FundingArb.lean`, `testudo-proofs/Proofs/DeltaNeutral.lean`, `testudo-proofs/Proofs.lean`
- Completed 2026-05-25 by /skill:vox build

### CP-6: GamblersRuin.lean ✅
- **Touches**: `testudo-proofs/Proofs/GamblersRuin.lean`, `testudo-proofs/Proofs.lean`
- Completed 2026-05-25 by /skill:vox build

### CP-7: Integration + README ✅
- **Touches**: `testudo-proofs/README.md`, `testudo-proofs/Proofs.lean`, `strat-lean-proofs.md`, `AGENT_TRADING.md`
- Completed 2026-05-25 by /skill:vox build

## Risks

1. **Mathlib compilation time** — First `lake build` downloads and compiles ~1GB. Mitigation: run with adequate time budget, use parallel jobs.
2. **Gambler's Ruin complexity (CP-6)** — Hardest theorem. Mitigation: prove the simpler symmetric random walk case if full ruin bound is intractable.
3. **Theorem scope creep** — Pseudocode in strat-lean-proofs.md may be more ambitious than Mathlib supports. Mitigation: simplify theorems to their simplest useful form.
