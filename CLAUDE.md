# CLAUDE.md - AI Development Context for Testudo Trading Platform

## 🏛️ Project Philosophy - The Roman Way

**Testudo** embodies the Roman military principles of **Disciplina**, **Formatio**, **Prudentia**, and **Imperium** in systematic crypto trading. Every line of code, every architectural decision, and every user interaction must reflect unwavering discipline and mathematical precision.

**Core Principle**: *"In trading, as in war, discipline separates victory from defeat. Testudo removes human emotion from position sizing decisions through formal mathematical verification."*

---

## 🎯 AI Development Imperatives

### Primary Mission
You are building a **disciplined crypto trading platform** that implements Van Tharp position sizing methodology with **99.9% mathematical accuracy** and **sub-200ms execution latency**. Every decision must serve this mission.

### Sacred Rules (Never Violate)
1. **Monotonic Development**: Add functionality, never modify core risk calculations
2. **Formal Verification**: All financial math must be property-tested with 10,000+ iterations
3. **OODA Loop Discipline**: Every trade follows Observe → Orient → Decide → Act sequence
4. **Zero Financial Errors**: Position sizing calculations have zero tolerance for inaccuracy
5. **Roman Naming**: Use Latin-inspired names for core components (Disciplina, Formatio, etc.)

---

## 🔧 Technical Architecture Context

### Stack Decisions (Immutable)
- **Backend**: Rust monolith (Tokio + Axum) - chosen for performance and safety
- **Database**: PostgreSQL + TimescaleDB - ACID compliance for financial data
- **Cache**: Redis - sub-second market data access
- **Frontend**: Progressive Web App (React/TypeScript) - accessibility over native performance
- **Charts**: TradingView Lightweight Charts - industry standard integration

### Performance Requirements (Non-Negotiable)
- Order execution: **<200ms** from UI to exchange confirmation
- Position calculation: **<50ms** Van Tharp formula execution  
- Market data latency: **<100ms** WebSocket updates
- System uptime: **99.9%** during market hours

---

## 💡 Development Patterns

### Code Organization Philosophy
```
src/
├── core/           # Immutable financial calculations (Van Tharp, risk rules)
├── ooda/          # OODA loop implementation (observe, orient, decide, act)  
├── exchange/      # Exchange adapters (add-only, never modify existing)
├── risk/          # Testudo Protocol enforcement
├── ui/            # Progressive Web App interface
└── sops/          # Standard Operating Procedures
```

### Naming Conventions
- **Core Services**: Latin military terms (Disciplina, Formatio, Prudentia, Imperium)
- **Functions**: Clear English describing exact mathematical operation
- **Types**: Explicit financial types (AccountEquity, RiskPercentage, PositionSize)
- **Errors**: Descriptive error types with recovery guidance

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

### Risk Engine (Core - Never Modify)
```rust
pub struct VanTharpCalculator {
    // Immutable after initialization
    precision: u32,
    verification_enabled: bool,
}

// Formula: Position Size = (Account Equity × Risk %) ÷ (Entry - Stop)
impl PositionSizeCalculator for VanTharpCalculator {
    fn calculate_position_size(
        &self,
        account_equity: Decimal,
        risk_percentage: Decimal,
        entry_price: Decimal,
        stop_loss: Decimal,
    ) -> Result<PositionSize, RiskCalculationError>;
}
```

### OODA Trading Loop
1. **Observe**: Market data ingestion and validation
2. **Orient**: Van Tharp position sizing and risk assessment  
3. **Decide**: Testudo Protocol rule enforcement
4. **Act**: Exchange order execution with confirmation

### Testudo Protocol Rules
- Individual trade risk: **≤6% of account equity**
- Total portfolio risk: **≤10% of account equity**  
- Daily loss limit: **Configurable per user**
- Consecutive loss limit: **3 trades (circuit breaker)**

---

## 📊 Data Architecture

### Database Schema Principles
- **Immutable trade records**: INSERT only, never UPDATE trade history
- **Audit trails**: Every calculation logged with cryptographic hash
- **Time-series optimization**: TimescaleDB for performance analytics
- **Data retention**: 7 years for trades, 3 years for user activity

### Real-time Data Flow
```
Binance WebSocket → Market Data Manager → Redis Cache → WebSocket Handler → Frontend UI
User Drag Action → Position Calculator → Risk Validator → Order Executor → Exchange API
```

---

## 🚨 Error Handling Protocol

### Critical Errors (Halt Trading)
- Risk calculation failures
- Exchange connectivity loss
- Database integrity violations
- Circuit breaker triggers

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

## 🎨 User Experience Principles

### Interface Philosophy
- **Minimalist**: Only essential trading information visible
- **Drag-based**: Intuitive chart-based order placement
- **Roman-inspired**: Clean, disciplined visual design
- **Zero cognitive load**: Automated position sizing removes decision fatigue

### Key User Flows
1. **Login**: Roman shield button → Clerk authentication
2. **Trade Setup**: Drag entry/stop/target on chart → Automatic position size calculation
3. **Risk Confirmation**: Visual risk display → One-click execution
4. **Portfolio Monitoring**: Real-time P/L updates with R-multiple analysis

---

## 📋 Standard Operating Procedures

### Before Coding Any Feature
1. Read relevant SOP document in `/sop/` directory
2. Understand impact on Van Tharp calculations
3. Verify no modification to existing risk rules
4. Write property-based tests first
5. Implement with formal verification mindset

### Code Review Requirements
- [ ] Mathematical accuracy verified through multiple methods
- [ ] No modification to existing financial calculations
- [ ] Property-based tests with 10,000+ iterations
- [ ] Performance benchmarks meet latency requirements
- [ ] Error handling covers all failure scenarios

---

## 🔐 Security Imperatives

### Data Protection
- **API Keys**: AES-256 encrypted storage
- **User Data**: Database encryption at rest
- **Network**: TLS 1.3 for all communications
- **Audit Logging**: Immutable record of all financial operations

### Risk Controls
- **Position Limits**: Enforced at multiple layers (UI, API, Risk Engine)
- **Circuit Breakers**: Automatic trading halts on loss limits
- **Protocol Enforcement**: Immutable risk rules prevent override

---

## 📚 Implementation Priorities

### Phase 1: Core Risk Engine
1. Van Tharp position sizing calculator with formal verification
2. Testudo Protocol rule enforcement
3. Property-based testing suite
4. Database schema with audit trails

### Phase 2: OODA Trading Loop
1. Market data observation layer
2. Position orientation and analysis
3. Risk-based decision making
4. Order execution with exchange integration

### Phase 3: User Interface
1. Progressive Web App foundation
2. TradingView chart integration
3. Drag-based trade setup interface
4. Real-time portfolio monitoring

---

## 🎯 Success Metrics

### Technical KPIs
- **Latency**: 99th percentile order execution <200ms
- **Accuracy**: Zero position sizing calculation errors
- **Uptime**: 99.9% system availability
- **Performance**: <50ms Van Tharp calculation time

### Business KPIs
- **User Growth**: 1000 active traders within 12 months
- **Engagement**: Average 30-second trade setup time
- **Retention**: 80% user retention after 3 months
- **Revenue**: Sustainable subscription model

---

## 🔄 Continuous Improvement

### Monthly Reviews
- [ ] Position sizing accuracy audit
- [ ] Performance benchmark analysis
- [ ] User feedback incorporation
- [ ] Security vulnerability assessment

### Quarterly Updates
- [ ] SOP effectiveness review
- [ ] Architecture optimization opportunities
- [ ] Exchange integration expansion
- [ ] Feature roadmap alignment

---

## 🔄 Enhanced Workflow Patterns

### 1. Explore-Plan-Execute Pattern (MANDATORY)

#### Exploration Phase
```
# Context Gathering Protocol
1. "Read [specific files] and understand the current architecture. Don't write code yet."
2. "Use subagents to investigate [specific questions] about the codebase structure."
3. "Think hard about potential approaches and trade-offs."
```

#### Planning Phase (Use Thinking Budget)
We recommend using the word "think" to trigger extended thinking mode for complex trading logic:

**Thinking Budget Levels:**
- `think` - Basic analysis (4,000 tokens)
- `think hard` - Moderate complexity (Van Tharp calculations)
- `think harder` - Complex problems (OODA loop integration)  
- `ultrathink` - Maximum thinking budget (Risk engine design)

#### Execution Phase
```
# Implementation Protocol
1. "Implement your plan step by step."
2. "Verify each component as you build it."
3. "Run tests continuously during development."
4. "Update documentation as you go."
```

### 2. TDD-First Workflow (REQUIRED for Financial Calculations)

Property-based testing is essential for Testudo's mathematical accuracy:

```bash
# TDD Implementation Steps
1. "Write comprehensive property-based tests for [Van Tharp calculation] based on mathematical requirements. Use TDD approach."
2. "Run tests and confirm they fail appropriately."  
3. "Commit the tests with message: 'test: add property tests for [feature]'"
4. "Implement code to make tests pass. Don't modify the tests."
5. "Iterate until all 10,000+ property test iterations pass."
6. "Commit implementation with message: 'feat: implement [feature] with formal verification'"
```

### 3. Multi-Claude Development Strategy

#### Git Worktree for Parallel Development
```bash
# Setup Multiple Workstreams for different Testudo components
git worktree add ../testudo-disciplina disciplina-implementation
git worktree add ../testudo-formatio formatio-ooda-loop
git worktree add ../testudo-prudentia prudentia-risk-engine

# Launch Claude in each (separate terminals)
cd ../testudo-disciplina && claude
cd ../testudo-formatio && claude  
cd ../testudo-prudentia && claude
```

#### Code Review Workflow
```bash
# Writer Claude (Terminal 1) - Implements Van Tharp calculator
"Implement the Van Tharp position sizing calculator following our mathematical precision requirements."

# Reviewer Claude (Terminal 2) - Reviews for accuracy
"/clear"
"Review the position sizing implementation in disciplina/ folder. Look for mathematical errors, precision issues, and property test coverage."

# Integration Claude (Terminal 3) - Ensures system harmony
"/clear" 
"Read both the implementation and review feedback. Ensure integration with Testudo Protocol and OODA loop requirements."
```

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
cargo test financial -- --test-threads=1

# Property-based testing (minimum 10,000 iterations)
cargo test prop_ -- --ignored --release

# Performance benchmarks (must meet latency targets)
cargo bench position_sizing

# Security audit (zero vulnerabilities allowed)
cargo audit
```

### Quality Gates (ALL MUST PASS)
```bash
# Complete quality check before commits
./scripts/quality-gate.sh

# What this script runs:
# 1. cargo test --all-features
# 2. cargo clippy -- -D warnings  
# 3. cargo fmt --check
# 4. cargo audit
# 5. cargo bench --no-run  # Verify benchmarks compile
```

### Deployment
```bash
# Build production binary
cargo build --release --bin testudo

# Database migrations
sqlx migrate run

# Start services with production config
./target/release/testudo --config production.toml

# Health check
curl http://localhost:3000/health
```

---

## 🎯 Custom Testudo Commands & Shortcuts

### Quick Commands Reference
```bash
# Thinking Commands
"think" | "think hard" | "think harder" | "ultrathink"

# Testudo-Specific Workflow Commands  
"qplan" - Analyze Van Tharp implementation consistency before planning
"qcode" - Implement plan and ensure all property tests pass
"qcheck" - Perform mathematical accuracy review like a senior quant
"qgit" - Create conventional commit message and push

# Development Flow
"prepare to discuss [Van Tharp feature]" - Context gathering mode
"think architecturally first about [OODA component]" - Focus on system design
"/clear" - Reset context between different crates/components
```

### Context Management for Testudo

#### Context Priming Strategy
Create specialized context commands for different architectural layers:

```bash
# Testudo Context Commands
/context:disciplina - Load Van Tharp calculation context and mathematical requirements
/context:formatio - Focus on OODA loop and trading system patterns  
/context:prudentia - Load risk management and protocol enforcement context
/context:imperium - Load Progressive Web App and UI patterns
/context:full - Load complete Testudo project understanding
```

#### Performance Optimization
- Use `/clear` command frequently between different crate contexts
- Use specific file references rather than broad directory scans
- Leverage subagents for focused investigations on specific components
- Store frequently referenced patterns in crate-specific CLAUDE.md files

---

## 📋 Quality Assurance Framework

### Code Review Checklist (Testudo-Specific)
```markdown
## Van Tharp Implementation Checklist
- [ ] All calculations use Decimal types (never f64 for money)
- [ ] Property-based tests with 10,000+ iterations
- [ ] Mathematical properties verified (linearity, bounds, precision)
- [ ] Performance meets <50ms calculation target
- [ ] No modification to existing Van Tharp implementations
- [ ] Formal verification reasoning documented

## OODA Loop Integration Checklist  
- [ ] All async operations have timeout handling
- [ ] State transitions logged for audit trail
- [ ] Error recovery paths clearly defined
- [ ] Performance benchmarks meet <200ms execution target
- [ ] Integration with Disciplina and Prudentia verified
- [ ] Circuit breaker logic tested under failure conditions

## Risk Management Checklist
- [ ] Protocol limits enforcement cannot be bypassed
- [ ] All edge cases in risk calculation covered
- [ ] Circuit breaker recovery protocols tested
- [ ] Audit trail integrity verified
- [ ] Integration with monitoring systems complete
```

### Continuous Improvement Protocol

#### Documentation Iteration
1. Use `#` key during development to add insights to crate-specific CLAUDE.md files
2. Monthly CLAUDE.md reviews and refinements across all crates
3. Update root CLAUDE.md when discovering patterns that apply project-wide
4. Run CLAUDE.md files through prompt improvement when adding emphasis

#### Team Knowledge Sharing
```bash
# Testudo-Specific Documentation Structure
CHANGELOG.md - Track Roman-inspired release progress
/sops/ - Standard Operating Procedures for trading operations
/crates/*/CLAUDE.md - Component-specific development context
/docs/architecture.md - System design decisions and trade-offs
```

---

## 🏛️ The Testudo Way

*"Just as the Roman testudo formation protected soldiers through disciplined coordination, our trading platform protects capital through systematic risk management. Every component works in harmony, every calculation verified, every decision disciplined."*

**Remember**: You are not just writing code—you are crafting a systematic approach to wealth preservation and growth through the marriage of ancient military discipline and modern financial mathematics.

**Imperium**: Command your code with the same precision a Roman general commanded his legions.

---

**Document Version**: 1.0  
**Last Updated**: 2025-08-30  
**Review Cycle**: Monthly  
**Owner**: AI Development Context