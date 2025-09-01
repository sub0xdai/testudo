# Testudo Trading Platform

A high-performance, systematic crypto trading platform built in Rust. Following the `TDD` framework, using `DRY` and `KISS`. Its core is the Van Tharp position sizing methodology, engineered for mathematical precision and low-latency execution.

## Tech Stack
- **Backend**: Rust (Tokio + Axum)
- **Database**: PostgreSQL + TimescaleDB
- **Cache**: Redis
- **Frontend**: React/TypeScript (PWA)
- **Charts**: TradingView Lightweight Charts

## Project Structure (Crates)
The project uses a multi-crate workspace to enforce modularity.
`
/
├── crates/
│   ├── disciplina/      # Core financial calculations (Van Tharp, risk rules)
│   │   ├── src/
│   │   └── tests/
│   ├── formatio/        # OODA loop implementation (observe, orient, decide, act)
│   │   ├── src/
│   │   └── tests/
│   ├── prudentia/       # Protocol enforcement and exchange adapters
│   │   ├── src/
│   │   │   ├── exchange/
│   │   │   ├── monitoring/
│   │   │   └── risk/
│   │   └── tests/
│   ├── imperium/        # API Server and Command Interface (Axum)
│   │   ├── src/
│   │   └── tests/
│   └── testudo-types/   # Shared types
│       └── src/
├── docs/                # Core, fundamental .md files shaping architecture (prd.md, etc)
├── sop/                 # Standard Operating Procedures documentation
├── migrations/          # Database migration scripts
├── config/              # Configuration files
└── src/                 # Main application source (if any, might be just a workspace)
`

## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
`
cargo nextest run | tdd-guard-rust --passthrough
`
### Validation & Formatting
Run these frequently after changes.
`
cargo build --release
cargo clippy -- -D warnings
cargo fmt --check
`

### Testing
- **Run all tests**: `cargo test --all-features`
- **Run only financial math tests**: `cargo test financial -- --test-threads=1`
- **Run property-based tests**: `cargo test prop_ -- --ignored --release`

### Quality Gate
### Note on Quality Gate
A GitHub workflow automatically runs all tests, lints, and format checks on every push. All checks must pass.
