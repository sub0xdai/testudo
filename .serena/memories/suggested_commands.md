# Testudo - Essential Development Commands

## Build and Test Commands
```bash
# Build and validation (run after every change)
cargo build --release
cargo test --all-features
cargo clippy -- -D warnings
cargo fmt --check

# Financial calculation testing (MANDATORY)
cargo test --package disciplina -- --test-threads=1
cargo test --package prudentia -- --test-threads=1

# Property-based testing (minimum 10,000 iterations)
cargo test prop_ -- --ignored --release

# Performance benchmarks (must meet latency targets)
cargo bench position_sizing
cargo bench --package disciplina
cargo bench --package prudentia

# Security audit (zero vulnerabilities allowed)
cargo audit
```

## Development Workflow
```bash
# Install development dependencies
cargo install sqlx-cli
cargo install cargo-watch

# Start development server with auto-reload
cargo watch -x "run --bin testudo"

# Database operations
createdb testudo
psql testudo -c "CREATE EXTENSION IF NOT EXISTS timescaledb;"
cargo run --bin migrate

# Complete quality check before commits
./scripts/quality-gate.sh  # If available
```

## Testing Specific Components
```bash
# Test core risk engine
cargo test --package disciplina

# Test risk management system
cargo test --package prudentia

# Run integration tests
cargo test --test integration

# Run specific property tests
cargo test --package disciplina prop_van_tharp_properties -- --ignored
```

## System Commands (Linux)
Standard Linux utilities available:
- `git` - Version control
- `ls`, `cd`, `find`, `grep` - File system navigation
- `rg` (ripgrep) - Fast searching
- `curl` - HTTP requests for API testing