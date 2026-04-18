# Testudo — Project Router

## Initial Action
Before any implementation: re-read the task plan and the relevant rule file below.

## Project Structure
- **testudo-exchange/** — Rust backend (Actix-web, Tokio, PostgreSQL, 7 crates)
- **testudo-extension/** — Browser extension (Solid.js, Manifest V3, esbuild)
- **testudo-ccxt/** — CCXT sidecar (Node.js, Express, port 3100)
- **testudo-web/** — Landing site + auth (React 18, Vite, Tailwind)
- **testudo-ops/** — Kubernetes infrastructure (GKE, ArgoCD)

## Routing

### When working on Rust backend
Read `.claude/rules/rust-backend.md` — crate map, patterns, key files.
```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

### When working on browser extension
Read `.claude/rules/extension.md` — architecture, message types, key files.
```bash
cd testudo-extension && bun run build
```

### When working on trading/exchange logic
Read `.claude/rules/trading.md` — order types, sizing, CCXT endpoints, exchange quirks.

### When working on web frontend
```bash
cd testudo-web && bun run build
```

### When implementing a specification
Use `/vox build [spec-name]`.
Specs live in `.specify/specs/`. Constitution: `.specify/memory/constitution.md`.

### When creating a new feature
Use `/vox plan [description]` or `/speckit.specify [description]` first.

### When brainstorming or planning
Read `.claude/rules/workflow.md` — research-first, vertical slicing.

## Verification — Always Before Commit
```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Extension
cd testudo-extension && bun run build

# Web
cd testudo-web && bun run build
```
