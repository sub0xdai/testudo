# Project Context

## Tech Stack
- Language: Rust (2021 edition)
- Build Tool: Cargo (workspace)
- Testing Framework: cargo test (standard)
- Async Runtime: Tokio
- Web Framework: Actix-web 4
- Database: SQLx with PostgreSQL
- Cache: Redis (fred crate)

## Coding Standards
- Follow Rust idioms: prefer `?` over unwrap, use Result for fallible operations
- Use `rust_decimal::Decimal` for all financial calculations (never f64)
- Traits for abstraction (SOLID Dependency Inversion)
- `Arc<Mutex<>>` or `Arc<RwLock<>>` for shared state
- Async/await for all I/O operations

## TDD Cycle (Mandatory)
1. **RED**: Write failing test first
2. **GREEN**: Write minimal code to pass
3. **REFACTOR**: Improve while keeping tests green

## Error Handling
- Use thiserror for custom error types
- Propagate errors with `?` operator
- Log errors at appropriate levels (RUST_LOG)

## Project Principles (Numogrammatic Codex)
- **KISS**: Reduce complexity, minimal viable solution
- **DRY**: Single source of truth
- **SOLID**: Single responsibility, dependency inversion
- **SoC**: Separate concerns into crates/modules

## File Locations
- Adapters: `testudo-exchange/crates/common_utils/src/adapters/`
- API Routes: `testudo-exchange/crates/router/src/routes/`
- Engine: `testudo-exchange/crates/engine/src/`

## Verification Commands
- Fast check: `cd testudo-exchange && cargo check`
- Full test: `cd testudo-exchange && cargo test`
- Lint: `cd testudo-exchange && cargo clippy`
