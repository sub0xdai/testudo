# Testudo Project Constitution

> This document defines the core principles, standards, and governance for autonomous AI development on the Testudo cryptocurrency exchange platform.

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
- Performance matters: <16ms frame time for UI, <100ms API responses.

---

## 2. Technology Stack

### Backend (testudo-exchange/)
- **Language**: Rust (stable toolchain)
- **Framework**: Axum, Tokio async runtime
- **Database**: PostgreSQL (persistent), Redis (cache/pub-sub)
- **Build**: Cargo
- **Linting**: `cargo clippy --all-targets`
- **Testing**: `cargo test`
- **Formatting**: `cargo fmt`

### Frontend (testudo-web/)
- **Language**: TypeScript
- **Framework**: React 18, Vite
- **Styling**: Tailwind CSS (industrial/brutalist - no rounded corners)
- **Charts**: lightweight-charts v5.x
- **Package Manager**: Bun
- **Linting**: `bun run lint`
- **Building**: `bun run build`

### Infrastructure (testudo-ops/)
- **Orchestration**: Kubernetes (GKE)
- **GitOps**: ArgoCD
- **Ingress**: NGINX with TLS
- **Monitoring**: Prometheus + Grafana

---

## 3. Development Standards

### Code Style

#### Rust
- Use `Result<T, E>` for fallible operations, not panics
- Prefer `impl Trait` over dynamic dispatch when possible
- Document public APIs with rustdoc comments

#### TypeScript/React
- Functional components with hooks only
- Use existing UI patterns from `components/ui/`
- `font-mono` for numbers, `font-display` for labels
- No `any` types - explicit typing required

### Git Workflow
- Atomic commits with descriptive messages
- Format: `type: description` (feat, fix, refactor, docs, test)
- Always include `Co-Authored-By: Claude <noreply@anthropic.com>` for AI commits
- Never force push to main/master

### Testing Requirements
- Backend: Unit tests for all public functions
- Frontend: Component tests for user-facing features
- Integration: API endpoints must have request/response validation

---

## 4. Verification Commands

Before marking any task complete, run:

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Frontend
cd testudo-web/apps/web && bun run lint && bun run build
```

All commands must exit with code 0.

---

## 5. Completion Protocol

### When Implementing Specs
1. Read the specification completely
2. Implement all functional requirements
3. Complete the Completion Signal checklist
4. Run all verification commands
5. Commit and push changes
6. Output `<promise>DONE</promise>` only when ALL criteria pass

### Iteration Workflow
If any check fails:
1. Identify the issue from error output
2. Fix the code
3. Commit the fix
4. Re-run verification
5. Repeat until all checks pass

---

## 6. Governance

### Exceptions
When deviating from these principles:
- Document the reason in the commit message
- Explain why the deviation was necessary
- Create a follow-up task to address technical debt if applicable

### Updates
This constitution may be updated when:
- New technologies are adopted
- Existing practices prove inadequate
- Team consensus requires change

All updates should be committed with `docs: update constitution` message.

---

## 7. Key Files Reference

| Purpose | Location |
|---------|----------|
| Order matching engine | `testudo-exchange/crates/engine/src/engine/orderbook.rs` |
| API routes | `testudo-exchange/crates/router/src/routes/` |
| Main trading UI | `testudo-web/apps/web/src/pages/Trade.tsx` |
| Chart manager | `testudo-web/apps/web/src/utils/chart_manager.ts` |
| Position primitives | `testudo-web/apps/web/src/primitives/` |

---

*Last updated: 2026-01-15*
