# AGENTS.md - AI Development Guidelines

> This document provides instructions for AI agents working on the Testudo project.

---

## Quick Start

1. **Read the Constitution**: `.specify/memory/constitution.md`
2. **Find Specifications**: `.specify/specs/[NNN]-[name]/spec.md`
3. **Plan (Architect Mode)**: Generate the execution blueprint.
4. **Implement (Executor Mode)**: Follow the Completion Signal protocol in each spec.
5. **Verify**: Run all verification commands before committing.
6. **Signal**: Output `<promise>DONE</promise>` when complete.

---

## Project Overview

Testudo is a cryptocurrency exchange platform with:
- **Backend**: Rust matching engine (`testudo-exchange/`)
- **Frontend**: React trading interface (`testudo-web/`)
- **Infrastructure**: Kubernetes deployment (`testudo-ops/`)

---

## Agent Workflow

### Plan → Implement → Verify
1. **Plan**: Analyze the specification. Define exact file paths, logic signatures, and test commands.
2. **Implement**: Write code within the planned scope.
3. **Verify**: Run verification commands in a tight loop to fix compiler/lint errors.
4. **Complete**: Commit verified changes and signal completion.

---

## Verification Commands

Run these commands before marking any work complete:

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Extension
cd testudo-extension && bun run build

# Web
cd testudo-web && bun run build
```

---

## Workflow Commands

### Creating Specifications
```text
/speckit.specify [feature description]
```
Generates a numbered spec in `.specify/specs/`

### Implementing Specifications
```text
/speckit.implement [spec-name]
/ralph-loop [spec-name]
```
Executes autonomous implementation loop using the Executor model.

### Running All Specs
```bash
./scripts/ralph-loop.sh --all
```
Processes all specs in order.

---

## Autonomous Operation

AI agents on this project should:
- Make implementation decisions independently within the Architect's blueprint.
- Commit changes only when all verification commands pass.
- Iterate the `/ralph-loop` until all acceptance criteria in the `.specify` doc are met.
- Push to remote when explicitly requested.
- Signal completion with `<promise>DONE</promise>`.

---

## Key Files

| Purpose | Location |
|---------|----------|
| Constitution | `.specify/memory/constitution.md` |
| Spec Template | `templates/spec-template.md` |
| Checklist Template | `templates/checklist-template.md` |
| Ralph Loop Script | `scripts/ralph-loop.sh` |
| Archived Work | `.specify/archive/` |

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
- Skip verification commands (cargo/bun) under any circumstances.
- Mark work done without running tests.
- Make breaking changes to `testudo-ops/` without documentation.
