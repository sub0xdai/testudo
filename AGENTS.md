# AGENTS.md - AI Development Guidelines

> This document provides instructions for AI agents working on the Testudo project.

---

## Quick Start

1. **Read the Constitution**: `.specify/memory/constitution.md`
2. **Find Specifications**: `.specify/specs/[NNN]-[name]/spec.md`
3. **Implement**: Follow the Completion Signal protocol in each spec
4. **Verify**: Run all verification commands before committing
5. **Signal**: Output `<promise>DONE</promise>` when complete

---

## Project Overview

Testudo is a cryptocurrency exchange platform with:
- **Backend**: Rust matching engine (`testudo-exchange/`)
- **Frontend**: React trading interface (`testudo-web/`)
- **Infrastructure**: Kubernetes deployment (`testudo-ops/`)

---

## Verification Commands

Always run before marking work complete:

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Frontend
cd testudo-web/apps/web && bun run lint && bun run build
```

---

## Workflow Commands

### Creating Specifications
```
/speckit.specify [feature description]
```
Generates a numbered spec in `.specify/specs/`

### Implementing Specifications
```
/speckit.implement [spec-name]
/ralph-loop [spec-name]
```
Executes autonomous implementation loop

### Running All Specs
```
./scripts/ralph-loop.sh --all
```
Processes all specs in order

---

## Autonomous Operation

AI agents on this project should:
- Make implementation decisions independently
- Commit changes when verification passes
- Iterate until all acceptance criteria are met
- Push to remote when explicitly requested
- Signal completion with `<promise>DONE</promise>`

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

```
type: description

Co-Authored-By: Claude <noreply@anthropic.com>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

---

## Do Not

- Force push to main/master
- Commit files containing secrets
- Skip verification commands
- Mark work done without running tests
- Make breaking changes without documentation

---

*Last updated: 2026-01-15*
