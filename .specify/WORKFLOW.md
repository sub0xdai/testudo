# Vox Workflow — Advisor Gate Protocol

> **Adopted 2026-04-20** in response to QNT-01a FR-10 breach (silent
> architectural violation caught only retroactively after a live Bybit
> bracket regression). Cheaper to prove the plan is sound for one
> Opus-advisor turn than to discover a flaw in iteration 12.

---

## The Protocol

```
/vox plan <spec>                          # Opus (one-shot planner)
  └─ writes IMPLEMENTATION_PLAN.md

advisor()                                 # GATE 1 — review the plan
  └─ if advisor flags issues: fix plan, re-gate
  └─ if advisor clears: proceed

/vox build <spec>                         # Sonnet (iterative builder)
  └─ writes spec code + per-spec LEARNINGS.md
  └─ halts on <promise>DONE</promise> or max-iterations

advisor()                                 # GATE 2 — review the delivered spec
  └─ if advisor flags gaps: address as hot-fix commits
  └─ if advisor clears: archive spec, move to next
```

**Two advisor turns per spec.** On a 4-spec roadmap, that's 8 advisor turns — negligible next to the cost of one unreviewed flawed plan (up to 15 wasted build iterations).

---

## Gate 1 — After Plan, Before Build

**Purpose:** catch plan-level flaws while they're still cheap to fix.

The advisor sees:
- The full conversation transcript (including any back-and-forth that shaped the spec)
- The just-written `IMPLEMENTATION_PLAN.md`
- The spec doc itself

**What to ask the advisor to verify:**
- Does the plan's task decomposition cover every FR in the spec?
- Are architectural decisions (deviations, chosen patterns) sound?
- Does the plan honor the spec's explicit contracts (additive-only, byte-for-byte, etc.)?
- Are there spec-file claims the gap analysis has proven wrong but the plan still assumes?
- Are shared-code paths touched by the plan? Does the plan guard regressions on adjacent features?
- Is any CP overscoped / would benefit from splitting?

**Red flags the advisor should catch at Gate 1:**
- "Only dynamic mode touched" specs that insert code into the shared fixed-mode path (QNT-01a FR-10 breach).
- FR-X that has no task covering it.
- Plan assumes backend feature-complete when grep would show the opposite (or vice-versa).
- Task ordering that would produce a working test-passing state before the actual user-facing feature works.

---

## Gate 2 — After Build Complete, Before Archive

**Purpose:** verify the delivered spec actually satisfies acceptance criteria, not just passes tests.

The advisor sees:
- Full build transcript (every iteration's output, commit messages, validation sweeps)
- Final commit ladder
- The original acceptance criteria

**What to ask the advisor to verify:**
- Every acceptance-criteria checkbox traceable to a commit?
- Were any deviations silently papered over?
- Did the baseline hold (pre-existing test count, clippy warning count)?
- If the spec had an FR-X "regression guard" — was it actually tested, or just asserted?
- Any iteration that completed suspiciously fast / with vague commit message?

**Red flags the advisor should catch at Gate 2:**
- Spec archived while some FRs have no traceable commit.
- Test baselines quietly drifted (e.g., 655 passing yesterday, 653 today — 2 tests missing not fixed).
- Changes in shared modules that weren't scoped for this spec.
- FR-10-style contracts with output parity tests passing but path-equivalence not enforced.

---

## When to Skip Gates

- **Pure refactors / chores** (submodule bumps, docs fixes) — advisor gate is overkill.
- **Atomic carves** from an already-advised parent spec — if ENG-01 was gated, its atomic children (ENG-01a/b/c) can share the same gate judgment.
- **Explicit "time-critical" hot-fix** — skip Gate 1, but NEVER skip Gate 2 (the post-fix verification).

---

## Cost Accounting

A single advisor call (Opus 4.7, full context): comparable to 1–2 vox build iterations on Opus.
A wasted 15-iteration loop on Sonnet: comparable to ~5–7 advisor calls.
A wasted 15-iteration loop on Opus: comparable to ~30 advisor calls, plus the hot-fix work.

Gate discipline is cheap insurance — especially now that:
- plan runs on Opus (cache hits for advisor if session is fresh)
- build runs on Sonnet (cheap enough that loop-waste is less catastrophic, but still not free)
