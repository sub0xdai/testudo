# Operational Learnings

> Ralph's accumulated knowledge. Loaded each iteration.

---

## Codebase Patterns

### Rust Backend (testudo-exchange)

- **Error handling**: Use `Result<T, E>` with custom error enums, not `.unwrap()`
- **Async**: All services use `tokio` async runtime
- **Decimal math**: Use `rust_decimal` for financial calculations, not f64
- **Lock pattern**: Use `lock_or_recover!` macro for mutex access
- **Testing**: Tests live in same file or `tests/` directory

### File Locations

- Execution types: `crates/common_utils/src/adapters/execution_types.rs`
- Risk calculations: `crates/common_utils/src/risk/`
- Router services: `crates/router/src/services/`
- Shadow engine: `crates/engine/src/shadow/`

### Import Patterns

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Instant;
```

---

## Anti-Patterns (Don't Do This)

- Don't use `.unwrap()` on fallible operations
- Don't use `f64` for money/prices
- Don't hold locks across await points
- Don't modify tests to make them pass - fix the implementation

---

## Signs (Discoverable Patterns)

### When you see "clippy warning"
→ Fix the warning, don't suppress with `#[allow(...)]` unless truly necessary

### When you see "test failed"
→ Read the assertion message, trace back to find root cause, don't guess

### When you see "lock poisoned"
→ Use `lock_or_recover!` macro or handle with `.unwrap_or_else()`

### When latency is too high
→ Check for: unnecessary clones, allocations in hot path, await in loops

---

## Discoveries Log

<!-- Ralph adds discoveries here during implementation -->

### 2026-01-26
- Clippy warnings cleaned up across all crates
- `rust_decimal` already in dependencies

---

*This file grows as Ralph learns. Never delete entries.*
