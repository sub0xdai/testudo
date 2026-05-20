# AGENTS.md - AI Development Guidelines

> This document provides instructions for AI agents working on the Testudo project.

---

## Quick Start

1. **Read the Constitution**: `.specify/memory/constitution.md`
2. **Find Specifications**: `.specify/specs/[NNN]-[name]/spec.md`
3. **Plan**: `/skill:vox plan <spec-name>` — gap analysis → task decomposition → `IMPLEMENTATION_PLAN.md`
4. **Build**: `/skill:vox build <spec-name>` — one checkpoint per invocation, TDD (red → green → refactor)
5. **Verify**: Run all verification commands before committing.
6. **Signal**: Output `<promise>DONE</promise>` when complete.

---

## Project Overview

Testudo is a cryptocurrency exchange platform with:
- **Backend**: Rust matching engine (`testudo-exchange/`)
- **Frontend**: Solid.js desk/journal (`testudo-journal/`), Astro landing (`testudo-web/`)
- **Extension**: Chrome/Firefox Manifest V3 (`testudo-extension/`)
- **Infrastructure**: Kubernetes deployment (`testudo-ops/`)

---

## Agent Workflow

### Vox: Plan → Build → Verify

The project uses the **vox** spec-driven development methodology. Every spec goes through two phases:

#### Plan (`/skill:vox plan <spec-name>`)
1. Load the spec, constitution, and affected code.
2. Perform gap analysis: for each FR, does the code already do this? What's missing?
3. Decompose gaps into **vertical checkpoints** — each independently testable and committable.
4. Write `.specify/specs/<spec-name>/IMPLEMENTATION_PLAN.md` with checkpoint contracts.
5. No code is written during planning.

#### Build (`/skill:vox build <spec-name>`)
1. Pick the next incomplete checkpoint from `IMPLEMENTATION_PLAN.md`.
2. **Red**: Write the failing test first. Quote the failure output.
3. **Green**: Minimum code to pass. Quote the passing output.
4. **Refactor**: Clean up only if needed, keeping tests green.
5. **Verify**: Run the checkpoint's verification command. If it fails, loop back to step 4.
6. Run repo-wide gates (`cargo clippy --all-targets && cargo test`).
7. Mark the checkpoint complete in `IMPLEMENTATION_PLAN.md`.
8. **Stop**. Do not chain into the next checkpoint. Summarize and wait for the next invocation.

### TDD Protocol
- Write the failing test first. Never implement before the test exists.
- If a spec references an existing failing test, do not modify the test. Make the implementation satisfy it.
- Red → Green → Refactor: always in this order.

---

## Verification Commands

Run these commands before marking any work complete:

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Extension
cd testudo-extension && bun run build

# Journal/Desk
cd testudo-journal && bun run build

# Web/Landing
cd testudo-web && bun run build
```

All commands must exit with code 0.

---

## Working with Specifications

### Creating a Spec
Write a spec following the template in `templates/spec-template.md` (or `.specify/specs/TEMPLATE.md`). Place it in `.specify/specs/<SERIES>-<NN>-<slug>/spec.md`.

### Planning a Spec
```
/skill:vox plan <spec-name>
```
Produces `IMPLEMENTATION_PLAN.md` with vertical checkpoints, touched files, and verification commands.

### Building a Spec
```
/skill:vox build <spec-name>
```
Executes one checkpoint per invocation. Repeat until all checkpoints are complete.

### Active Spec Series
Check `.specify/specs/notes.md` for the current spec series and status.

---

## Autonomous Operation

AI agents on this project should:
- Make implementation decisions independently within the plan's checkpoint contracts.
- Commit changes only when all verification commands pass.
- Follow the TDD protocol strictly — red before green.
- Push to remote when explicitly requested.
- Signal completion with `<promise>DONE</promise>`.

---

## Key Files

| Purpose | Location |
|---------|----------|
| Constitution | `.specify/memory/constitution.md` |
| Spec Template | `templates/spec-template.md` or `.specify/specs/TEMPLATE.md` |
| Specs Index | `.specify/specs/notes.md` |
| Implementation Plans | `.specify/specs/<spec-name>/IMPLEMENTATION_PLAN.md` |
| Archived Work | `.specify/archive/` |
| Agent Blueprint | `agent-integration-blueprint.md` (project root) |

---

## Commit Message Format

```text
type: description

Co-Authored-By: Claude <noreply@anthropic.com>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

---

## Do Not

- Force push to main/master.
- Commit files containing secrets.
- Skip verification commands (`cargo clippy --all-targets && cargo test` / `bun run build`) under any circumstances.
- Mark work done without running tests.
- Make breaking changes to `testudo-ops/` without documentation.
- Chain multiple vox build checkpoints in a single invocation.
- Write implementation code during `vox plan` — planning only.
