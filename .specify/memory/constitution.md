# Project Constitution

> This document defines the core principles, standards, and governance for autonomous AI development.

---

## 1. Core Principles

### Simplicity & YAGNI
- Start simple. Avoid over-engineering. Build exactly what's needed, nothing more.
- Delete dead code. No backwards-compatibility hacks unless explicitly required.
- Three similar lines of code is better than a premature abstraction.

### Autonomous Agent Development
- Make decisions without asking for approval on implementation details.
- Commit, push, and deploy autonomously when tests pass.
- Iterate until acceptance criteria are met, then signal completion.

### Quality Over Speed
- All code must pass linting and tests before commit.
- No shortcuts that compromise security or reliability.

---

## 2. Technology Stack

<!-- Customize for your project -->

### Languages & Frameworks
- **Backend**: Rust (Actix-web, Tokio, SQLx, PostgreSQL) — `testudo-exchange/`
- **Extension**: TypeScript + Solid.js (Manifest V3, esbuild) — `testudo-extension/`
- **Desk/Journal**: TypeScript + Solid.js (Vite, Tailwind, ECharts) — `testudo-journal/`
- **Landing**: TypeScript + React 18 (Vite, Tailwind) → migrating to Astro — `testudo-web/`
- **CCXT Sidecar**: Node.js + Express — `testudo-ccxt/`
- **Build**: `cargo build` (Rust), `bun run build` (TS/JS)
- **Linting**: `cargo clippy --all-targets` (Rust), `bun run build` (TS — build is the lint gate)
- **Testing**: `cargo test` (Rust), `bun run test` (Extension)

### Path Context
- All paths in this document are relative to the repository root.
- The agent is assumed to be executing commands from the root.

---

## 3. Development Standards

### Code Style
- Rust: `Result<T,E>` everywhere (never `unwrap()` in prod), `rust_decimal::Decimal` for financial math (never `f64`), `BTreeMap` for orderbooks, `DashMap` for concurrent state
- TypeScript: Zod schemas for runtime validation, Solid.js signals for state
- Follow existing patterns in the codebase
- Explicit error handling

### Git Workflow
- Atomic commits with descriptive messages
- Format: `type: description` (feat, fix, refactor, docs, test)
- Always include `Co-Authored-By: Claude <noreply@anthropic.com>` for AI commits

### Testing Requirements
- Tests for all public functions
- Never delete a failing test to make the pipeline pass. Fix the implementation.

### Test-Driven Development (TDD) Protocol
- If a spec references an existing failing test, do not modify the test. Make the implementation satisfy it.
- If no test exists, create unit tests to verify your work.
- Red -> Green -> Refactor: Write failing test, make it pass, then clean up.

---

## 4. Verification Commands

<!-- Customize for your project -->
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

## 5. Completion Protocol

### When Implementing Specs
1. Read the specification completely
2. Implement all functional requirements
3. Run all verification commands
4. Commit and push changes
5. Output `<promise>DONE</promise>` only when ALL criteria pass

### Iteration Workflow
If any check fails:
1. Identify the issue from error output
2. Fix the code
3. Commit the fix
4. Re-run verification
5. Repeat until all checks pass

### Recovery Protocol
If verification fails for >3 iterations on the same error:
1. Stop modifying the implementation.
2. Check if the test expectation itself is incorrect.
3. If truly stuck, document the blocker and signal for human review.

---

## 6. Key Files Reference

<!-- Customize for your project -->

| Purpose | Location |
|---------|----------|
| Rust router entry | `testudo-exchange/crates/router/src/main.rs` |
| Routes | `testudo-exchange/crates/router/src/routes/` |
| Services | `testudo-exchange/crates/router/src/services/` |
| Models | `testudo-exchange/crates/router/src/models/` |
| Repositories | `testudo-exchange/crates/router/src/repositories/` |
| Migrations | `testudo-exchange/crates/sqlx_postgres/migrations/` |
| pg_queue | `testudo-exchange/crates/pg_queue/src/` |
| Journal service | `testudo-exchange/crates/router/src/services/journal_service.rs` |
| HL SDK integration | `testudo-exchange/crates/router/src/services/hyperliquid/` |
| CEX client | `testudo-exchange/crates/router/src/services/cex_client.rs` |
| Exchange accounts | `testudo-exchange/crates/router/src/repositories/exchange_account.rs` |
| Desk/Journal app | `testudo-journal/src/` |
| Extension source | `testudo-extension/src/` |
| Landing page | `testudo-web/src/` |
| Specs | `.specify/specs/` |

---

*Last updated: $(date +%Y-%m-%d)*
