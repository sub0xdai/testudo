# GEMINI.md - AI Development Context for Testudo Trading Platform

## 🏛️ Project Philosophy - The Roman Way

**Testudo** embodies the Roman military principles of **Disciplina**, **Formatio**, **Prudentia**, and **Imperium** in systematic crypto trading. Every line of code, every architectural decision, and every user interaction must reflect unwavering discipline and mathematical precision.

**Core Principle**: *"In trading, as in war, discipline separates victory from defeat. Testudo removes human emotion from position sizing decisions through formal mathematical verification."*

---

## 🎯 AI Development Imperatives

### Primary Mission
You are building a **disciplined crypto trading platform** that implements Van Tharp position sizing methodology with **99.9% mathematical accuracy** and **sub-200ms execution latency**. Every decision must serve this mission.

### Sacred Rules (Never Violate)
1.  **Monotonic Development**: Add functionality, never modify core risk calculations.
2.  **Formal Verification**: All financial math must be property-tested with 10,000+ iterations.
3.  **OODA Loop Discipline**: Every trade follows the Observe → Orient → Decide → Act sequence.
4.  **Zero Financial Errors**: Position sizing calculations have zero tolerance for inaccuracy.
5.  **Roman Naming**: Use Latin-inspired names for core components (`disciplina`, `formatio`, etc.).

---

## 🔧 Technical Architecture Context

### Stack Decisions (Immutable)
-   **Backend**: Rust monolith (Tokio + Axum) - chosen for performance and safety.
-   **Database**: PostgreSQL + TimescaleDB - ACID compliance for financial data.
-   **Cache**: Redis - sub-second market data access.
-   **Frontend**: Progressive Web App (React/TypeScript) - accessibility over native performance.
-   **Charts**: TradingView Lightweight Charts - industry standard integration.

### Performance Requirements (Non-Negotiable)
-   Order execution: **<200ms** from UI to exchange confirmation.
-   Position calculation: **<50ms** Van Tharp formula execution.
-   Market data latency: **<100ms** WebSocket updates.
-   System uptime: **99.9%** during market hours.

---

## 💡 Development Patterns

### Code Organization Philosophy (Crates)
The project uses a multi-crate workspace to enforce modularity and clear separation of concerns.
```
/
├── crates/
│   ├── disciplina/   # Core financial calculations (Van Tharp, risk rules)
│   ├── formatio/     # OODA loop implementation (observe, orient, decide, act)
│   ├── prudentia/    # Testudo Protocol enforcement and exchange adapters
│   ├── imperium/     # API Server and Command Interface (Axum)
│   └── testudo-types # Shared types to prevent circular dependencies
└── sop/              # Standard Operating Procedures
```

### Naming Conventions
-   **Crates**: Latin military terms (disciplina, formatio, prudentia, imperium).
-   **Functions**: Clear English describing exact mathematical operation.
-   **Types**: Explicit financial types (AccountEquity, RiskPercentage, PositionSize).
-   **Errors**: Descriptive error types with recovery guidance.

### Testing Philosophy
```rust
// Every financial calculation requires property-based verification
proptest! {
    #[test]
    fn van_tharp_position_sizing_properties(
        account_equity in 1000.0..100000.0,
        risk_pct in 0.005..0.06,
        entry_price in 1.0..100.0,
    ) {
        // Position size must decrease as stop gets closer to entry
        // Position size must scale linearly with account equity
        // Position size must never exceed account balance
    }
}
```

---

## 🏗️ Component Architecture

### Risk Engine (`disciplina` & `prudentia`)
-   **`disciplina`**: Contains the `VanTharpCalculator`.
-   **`prudentia`**: Contains the `RiskManagementProtocol` and `RiskRule` implementations. Enforces the Testudo Protocol.

### OODA Trading Loop (`formatio`)
1.  **Observe**: `MarketObserver` for market data ingestion and validation.
2.  **Orient**: `PositionOrientator` for Van Tharp position sizing and risk assessment.
3.  **Decide**: `RiskDecider` for Testudo Protocol rule enforcement.
4.  **Act**: `OrderExecutor` for exchange order execution.

### Testudo Protocol Rules (Enforced by `prudentia`)
-   Individual trade risk: **≤6% of account equity**
-   Total portfolio risk: **≤10% of account equity**
-   Daily loss limit: **Configurable per user**
-   Consecutive loss limit: **3 trades (circuit breaker)**

---

## 🚨 Error Handling Protocol

### Critical Errors (Halt Trading)
-   Risk calculation failures
-   Exchange connectivity loss
-   Database integrity violations
-   Circuit breaker triggers

### Error Recovery Pattern
```rust
match calculation_result {
    Ok(position) => execute_trade(position),
    Err(RiskCalculationError::InvalidInputs) => request_user_correction(),
    Err(RiskCalculationError::SystemFailure) => halt_trading_immediately(),
    Err(RiskCalculationError::ProtocolViolation) => block_trade_with_explanation(),
}
```

---

## 📋 Standard Operating Procedures

### Before Coding Any Feature
1.  Read relevant SOP document in `/sop/` directory.
2.  Understand impact on Van Tharp calculations.
3.  Verify no modification to existing risk rules.
4.  Write property-based tests first.
5.  Implement with formal verification mindset.

### Code Review Requirements
-   [ ] Mathematical accuracy verified through multiple methods.
-   [ ] No modification to existing financial calculations.
-   [ ] Property-based tests with 10,000+ iterations.
-   [ ] Performance benchmarks meet latency targets.
-   [ ] Error handling covers all failure scenarios.

---

## ⚡ Development Commands

### Essential Commands
```bash
# Build and validation (run after every change)
cargo build --release
cargo test --all-features
cargo clippy -- -D warnings
cargo fmt --check

# Financial calculation testing (MANDATORY)
cargo test --package disciplina -- --test-threads=1

# Property-based testing (minimum 10,000 iterations)
cargo test --package prudentia -- --ignored --release

# Performance benchmarks (must meet latency targets)
cargo bench --package disciplina

# Security audit (zero vulnerabilities allowed)
cargo audit
```