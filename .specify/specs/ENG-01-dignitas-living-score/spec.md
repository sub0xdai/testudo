# Specification: Dignitas Living Score — INDEX

**Spec ID:** ENG-01-dignitas-living-score
**Date:** 2026-04-17 (carved into atomic specs 2026-04-20)
**Status:** **SUPERSEDED** — split into three atomic specs
**Class:** Series index

---

## Why This Was Split

The original monolithic spec packed three independent deliverables into one 325-line document: score computation + pill, public profile + handles, and streak counter. Implementation planning produced a 600-line plan that hid the natural seams. Per the project's "atomic specs over monolithic plans" convention, ENG-01 has been carved into three independently specifiable, implementable, and shippable slices.

Each atomic spec is self-contained and does not require reading the others to be implemented.

---

## Atomic Specs

| Spec | Scope | Depends on | Priority |
|------|-------|------------|----------|
| [ENG-01a — Dignitas Score as Living Artifact](../ENG-01a-dignitas-score-living/spec.md) | Daily snapshot, history, top-nav pill, panel, sparkline, transparency page, hide-pill preference, ungameability test | None | P1 |
| [ENG-01b — Dignitas Public Profile](../ENG-01b-dignitas-public-profile/spec.md) | User handles (claim / release / rate-limit), opt-in visibility toggles, public profile route `/desk/d/:handle`, `IdentitySettings` on Account | ENG-01a | P2 |
| [ENG-01c — Dignitas Streak Counter](../ENG-01c-dignitas-streak/spec.md) | Days-since-Concerning counter, `longest_ever`, silent reset, optional display on public profile | ENG-01a, ENG-01b, RSK-03 (live) | P2 |

---

## Recommended Sequencing

1. **ENG-01a** first — delivers standalone retention value (living score, transparent formula, pill). No external blockers.
2. **ENG-01b** second — converts the private score into a shareable identity once the score engine is trustworthy.
3. **ENG-01c** third — layers discipline weight onto an already-established visual surface. RSK-03 is already live so no external sequencing gate.

Each spec can ship independently via its own `/vox plan` + `/vox build` cycle.

---

## Shared Principles (apply across all three)

- **Ungameable by design.** Trading more, less, or bigger cannot directly improve the score. Only adherence to disciplined risk behavior can.
- **Opt-in everywhere.** Default installation reveals nothing publicly. Pill visible by default; profile and streak strictly opt-in.
- **Brutalist-serious tone.** No confetti, no level-ups, no milestone celebrations, no emoji in Dignitas UI. Gamification UI reads as insincere on a financial product.
- **No leaderboards. No streak freezes. No points redeemable for anything.**
- **Single source of truth.** `dignitas_history` is the only snapshot table; Overview's `PerformanceRadar`, the pill, and the public profile all read from it.

---

## Original Spec Archive

The original monolithic spec text has been preserved in git history (commit reachable via `git log --all -- .specify/specs/ENG-01-dignitas-living-score/spec.md`). The atomic specs above are the authoritative source going forward.
