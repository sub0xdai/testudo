# TigerBeetle Engineering Principles vs. Testudo Codebase

> Comparative analysis of [TigerBeetleDB](https://github.com/tigerbeetle/tigerbeetle) engineering principles against the Testudo codebase.
> Generated: 2026-05-06

---

## 1. Safety

### TigerBeetle: Safety First (above performance and DX)
TigerBeetle enforces strict serializability, single-core execution, static memory allocation, explicit bounds on everything, and an assertion density of **≥2 assertions per function**. No dynamic allocation after startup. `Result<T,E>` is insufficient — they pair-assert pre/postconditions at least twice.

### Testudo: Safety is Present but Inconsistently Applied

**What's good:**
- Constitution mandates `Result<T,E>` everywhere, `rust_decimal::Decimal` for financial math (no `f64`), `BTreeMap` for orderbooks
- Risk validation gate: `risk_validated: bool` on all shadow orders — orders can't execute without passing Decision Loop risk checks
- Dedicated error types in `crates/pg_queue/src/errors.rs` and `crates/sqlx_postgres/src/repositories/errors.rs`

**What's weaker:**
- **Assertion density is low.** `engine.rs` has exactly **0 assertions** in 489 lines. `orderbook.rs` has 0 assertions in 420 lines. `shadow/orders.rs` has assertions mostly in test blocks. The core matching engine has no invariant checks.
- `init_engine` calls `.unwrap()` directly: `let trade_id: i64 = get_latest_trade_id_from_db(pool, market).await.unwrap()`
- Error handling uses string errors (`Result<(), &str>`) everywhere in the engine instead of typed errors — losing pattern matching
- `check_and_lock_funds` mutates balances directly across multiple hashmap lookups without atomicity guarantees
- The `error.rs` file is entirely *commented out* — a planned custom error type that was abandoned
- **2 tests currently FAIL** in `cargo test`

| Principle | TigerBeetle | Testudo |
|---|---|---|
| Assertions per function | ≥2 (mandatory) | ~0 (engine), ~5 (shadow/orders in tests) |
| Error types | Zig error unions + crash on corruption | Result<(), &str> — strings, not typed |
| Financial math | Fixed-point custom types | `rust_decimal::Decimal` (correct choice) |
| Unwrap in prod | Banned (all paths handled) | Present in `init_engine` |
| Pair assertion pattern | Assert before write AND after read | Not present |

---

## 2. Performance

### TigerBeetle: Mechanical Sympathy from Day One
- Back-of-the-envelope sketches for all 4 resources (network, disk, memory, CPU)
- Single-threaded execution, batched prepares (up to 8190), Direct I/O, io_uring, LSM trees
- Control plane / data plane separation — O(1) decisions outside inner O(N) loops
- Explicit static allocation: all memory computed at startup from CLI args, zero post-init allocation

### Testudo: Good Patterns but Mixed Execution

**What's good:**
- `BTreeMap` for orderbooks with range matching in `match_asks` / `match_bids` — O(log n + k), architecturally sound
- User order index (`user_orders: HashMap<String, HashSet<String>>`) and order location index for O(1) lookups — matches TigerBeetle's "index everything" approach
- Lazy serialization on hot path: notification publishing uses `tracing::error!` for failures instead of panicking
- Read-Compute-Write pattern documented in shadow orders for minimizing lock contention

**What's weaker:**
- **No batching.** Each order is processed individually. Compare to TigerBeetle's batch of 8190 transfers per prepare. This means network round-trips and database writes per order.
- **tokio::sync::Mutex wrapping the entire engine** (`Arc<Mutex<Engine>>` in `order.rs`) — the whole engine is a single critical section. TigerBeetle achieves concurrency through pipeline stages (prefetch I/O parallel, execution sequential).
- `engine.rs` stores balances in `HashMap<String, Mutex<UserBalances>>` — a Mutex *per user* inside an Arc<Mutex<Engine>>. This is double-locking for no gain since the outer Mutex already serializes everything.
- No memory budgeting or upper-bound calculations anywhere
- PostgreSQL as the intermediary queue for all operations — adds network + serialization overhead per message vs TigerBeetle's direct client-to-database protocol

| Principle | TigerBeetle | Testudo |
|---|---|---|
| Batching | 8190 transfers per prepare | No batching (1 order at a time) |
| Execution model | Single-threaded, no locks | Arc<Mutex<Engine>> + per-user Mutex |
| Storage engine | Custom LSM (Direct I/O, io_uring) | PostgreSQL (via SQLx) |
| Control/data plane | Explicit separation | Not separated |
| Resource budgeting | Full static allocation at startup | Unbounded HashMap growth |

---

## 3. Developer Experience / Code Quality

### TigerBeetle: Rigid Discipline
- **70-line function limit** (hard). 100-column line limit.
- Zero dependencies (except Zig stdlib + Linux kernel)
- All memory statically allocated — forces design discipline
- Snake_case everywhere, units in variable names (`latency_ms_max`), descriptive commit messages
- Zero technical debt policy

### Testudo: Modern Rust but Lax on Discipline

**What's good:**
- Clean module structure: `routes/`, `services/`, `repositories/`, `models/` — standard Rust web pattern
- `rust_decimal::Decimal` consistently used, no `f64` in financial paths
- Solid test coverage: **740 passing tests** across the workspace
- Good doc comments on shadow engine (`/// # Fill Logic (from PRD)`) with architectural rationale
- Spec-driven development with completion protocol

**What's weaker:**
- `engine.rs` is 489 lines — exceeds TigerBeetle's 70-line limit by 7x
- `shadow/actor.rs` is **2029 lines** — a single file monolith. This is a critical architectural risk.
- `main.rs` is 1421 lines — router startup mixed with route definitions, config, middleware setup all in one file
- Code comments are sparse in critical paths: `engine.rs` has almost no comments explaining *why* balance updates happen in a specific order
- Multiple deprecation patterns: `#[deprecated]` on `create_order_pg`, deprecated `check_fills` function — suggests code churn without cleanup
- `eprintln!` used in production code paths instead of structured `tracing` — mixing stdout/stderr with proper observability
- clippy warnings exist (useless conversions, `iter().any()` vs `contains()`)
- Node.js sidecar (`testudo-cex/`), separate extension, separate journal — multi-language operational complexity

| Principle | TigerBeetle | Testudo |
|---|---|---|
| Function length limit | 70 lines (hard) | 489 (engine), 2029 (actor) |
| Dependencies | 0 (stdlib only) | ~20 Rust crates + npm ecosystem |
| Line-length limit | 100 columns | Not enforced |
| Variable naming | Units/qualifiers in names | Mixed — `fills`, `asks`, `trade_id` |
| Deprecation handling | Zero tolerance | Deprecated code left in tree |
| Observability | Explicit logging | Mix of `eprintln!` + `tracing` |

---

## 4. Testing & Correctness

### TigerBeetle: Deterministic Simulation Testing (DST)
- VOPR simulator runs the **actual production code** with seeded randomness, storage fault injection, and time dilation
- Any bug is perfectly reproducible from seed + git commit
- Exhaustive testing: valid data, invalid data, and valid-becoming-invalid data

### Testudo: Unit + Integration Tests
- **740 passing tests** — strong coverage for a project this size
- Tests exist for orders (fill logic, leverage, cancel), risk validation, pruning, WebSocket serialization
- But: tests are standard Rust `#[test]` — no simulation, no fault injection, no determinism guarantees
- Test failures suggest tests may depend on external state (database) — `test_me_returns_user_info` panics on Null vs expected UUID

| Principle | TigerBeetle | Testudo |
|---|---|---|
| Testing strategy | DST (VOPR simulator) | Standard unit + integration |
| Fault injection | Storage faults, network partitions, crashes | None |
| Reproducibility | Seeded, deterministic | May depend on DB state |
| Test count | N/A (continuous fuzzing) | 740 + 22 ignored |

---

## 5. Architecture Philosophy

### TigerBeetle: First-Principles Co-Design
Consensus protocol (VSR) + storage engine (LSM) + memory management (static) are co-designed from the ground up. Every component knows the limits of every other component. The database is the **system of record** — it doesn't delegate correctness to external systems.

### Testudo: Service-Oriented Assembly
A pragmatic assembly of proven components: PostgreSQL (SQLx) + Actix-web + Tokio. The engine is a layer *on top of* a general-purpose database. This is a legitimate architectural choice (most systems work this way), but it means:
- Consistency depends on PostgreSQL's guarantees, not co-designed invariants
- Order execution goes through a `PgQueueManager` (NOTIFY/LISTEN) rather than a direct protocol
- Multiple sidecars: CCXT bridge, TypeScript CEX service, extension

---

## Summary: The 80/20 Gap

Testudo applies many TigerBeetle principles at the *constitutional* level (Decimal for financial math, Result types, spec-driven development) but falls short in the *execution*:

| Dimension | Grade | Key Gap |
|---|---|---|
| Safety | 🟡 | No assertions in core engine; string errors; two failing tests |
| Performance | 🟡 | No batching; global Mutex serializes everything; PG as queue |
| Code discipline | 🔴 | 2029-line files; `eprintln!` in prod; zombie deprecated code |
| Testing | 🟢 | 740 tests is strong, but no simulation/fault injection |
| Architecture | 🟡 | Good module layout but no co-design of storage/execution/consensus |

### Highest-Impact Changes

1. **Assertion discipline**: 2+ assertions per function in the engine and orderbook, catching invariant violations before they become production bugs
2. **Split `shadow/actor.rs` (2029 lines)** into focused modules of ≤200 lines each — this alone would reduce cognitive load dramatically
3. **Replace string errors** with typed error enums in the engine (`Result<(), &str>` → `Result<(), EngineError>`)
4. **Eliminate `.unwrap()`** from all production code paths
5. **Remove deprecated code** and standardize on `tracing` crate instead of `eprintln!`
