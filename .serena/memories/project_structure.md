# Testudo - Project Structure Guide

## Root Directory Structure
```
testudo/
├── crates/              # Roman legion-inspired crate organization
├── docs/               # Technical documentation
├── sop/                # Standard Operating Procedures
├── migrations/         # Database schema migrations
├── config/             # Configuration files
├── scripts/            # Build and deployment scripts
├── src/                # Main binary source
└── examples/           # Usage examples
```

## Core Crates (Roman Military Organization)

### Disciplina (`crates/disciplina/`) - ✅ COMPLETED
**Purpose**: Van Tharp risk calculation engine with formal verification
- `src/calculator.rs` - Position sizing calculator
- `src/types.rs` - Financial types (AccountEquity, RiskPercentage, etc.)
- `src/errors.rs` - Calculation error handling
- `tests/` - Comprehensive test suite with property-based testing

### Prudentia (`crates/prudentia/`) - ✅ SUBSTANTIALLY COMPLETE
**Purpose**: Risk management protocol and exchange integration
- `src/risk/` - Risk assessment and protocol enforcement
  - `assessment.rs` - RiskAssessment and TradeProposal types
  - `rules.rs` - RiskRule trait and MaxTradeRiskRule
  - `protocol.rs` - RiskManagementProtocol orchestrator
  - `portfolio_rules.rs` - Portfolio-level risk rules
  - `engine.rs` - Risk validation engine
- `src/exchange/` - Exchange adapters and integration
  - `adapters.rs` - ExchangeAdapterTrait and core types
  - `mock.rs` - MockExchange for testing (5 passing tests)
  - `binance.rs` - Binance exchange integration (placeholder)
- `src/monitoring/` - Portfolio tracking and metrics
- `src/types/` - Trade proposals and risk assessment types

### Formatio (`crates/formatio/`) - 🚧 IN PROGRESS (Phase 1 Complete)
**Purpose**: OODA loop trading operations and execution logic
- `src/ooda.rs` - Core OodaLoop state machine (7 states, validated transitions)
- `src/types.rs` - TradeIntent, MarketObservation, ExecutionPlan, LoopMetrics
- `src/observer.rs` - Market observation phase (placeholder)
- `src/orientator.rs` - Situation assessment phase (placeholder)
- `src/decider.rs` - Decision making phase (placeholder)
- `src/executor.rs` - Trade execution phase (placeholder)
- `tests/ooda_tests.rs` - State machine tests (7 passing tests)

### Imperium (`crates/imperium/`) - ❌ MINIMAL
**Purpose**: API server and command interface
- Basic structure only

## Important Documentation Files
- `CLAUDE.md` - AI development context (project-wide)
- `crates/*/CLAUDE.md` - Crate-specific development context
- `technical_spec.md` - Complete technical specification
- `architecture.md` - C4 model system architecture
- `prd.md` - Product Requirements Document
- `CHANGELOG.md` - Release history and roadmap
- `sop/*.md` - Standard Operating Procedures

## Configuration
- `Cargo.toml` - Main workspace configuration
- `config/default.toml` - Application configuration template
- Individual `crates/*/Cargo.toml` - Crate-specific dependencies

## Recent Progress (2025-08-31)
- **Formatio OODA Loop Phase 1**: State machine, ExchangeAdapter trait, MockExchange
- **Prudentia Exchange Integration**: Unified adapter interface for exchange operations
- **Test Coverage**: 12 new tests (7 OODA state tests, 5 MockExchange tests)