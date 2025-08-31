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
**Purpose**: Risk management protocol enforcement (no longer handles exchange integration)
- `src/risk/` - Risk assessment and protocol enforcement
  - `assessment.rs` - RiskAssessment and TradeProposal types
  - `rules.rs` - RiskRule trait and MaxTradeRiskRule
  - `protocol.rs` - RiskManagementProtocol orchestrator
  - `portfolio_rules.rs` - Portfolio-level risk rules
  - `engine.rs` - Risk validation engine
- `src/monitoring/` - Portfolio tracking and metrics
- `src/types/` - Trade proposals and risk assessment types

### Formatio (`crates/formatio/`) - 🚧 IN PROGRESS (Phase 2: Observer Complete)
**Purpose**: OODA loop trading operations and execution logic
- `src/ooda.rs` - Core OodaLoop state machine (7 states, validated transitions)
- `src/types.rs` - TradeIntent, MarketObservation, ExecutionPlan, LoopMetrics
- `src/observer.rs` - ✅ **NEW** Market observation phase (COMPLETED)
  - MarketObserver struct with configurable data age thresholds
  - observe_symbol() method with automatic state transitions
  - ObservationResult for comprehensive success/failure tracking
  - Integration with exchange MarketData conversion
- `src/exchange.rs` - ✅ **NEW** Exchange integration module (COMPLETED)
  - ExchangeAdapterTrait for unified exchange abstraction
  - MockExchange implementation for testing
  - MarketData, TradeOrder, OrderResult, AccountBalance types
  - ExchangeError enum with detailed error classification
- `src/orientator.rs` - Situation assessment phase (placeholder)
- `src/decider.rs` - Decision making phase (placeholder)
- `src/executor.rs` - Trade execution phase (placeholder)
- `tests/ooda_tests.rs` - Comprehensive test suite (14 passing tests)
  - 8 OODA state machine tests
  - 6 Observer integration tests

### Imperium (`crates/imperium/`) - ❌ MINIMAL
**Purpose**: API server and command interface
- Basic structure only

## Important Documentation Files
- `CLAUDE.md` - AI development context (project-wide)
- `crates/*/CLAUDE.md` - Crate-specific development context
- `technical_spec.md` - Complete technical specification
- `architecture.md` - C4 model system architecture
- `prd.md` - Product Requirements Document
- `CHANGELOG.md` - Release history and roadmap (recently updated for Observer)
- `sop/*.md` - Standard Operating Procedures

## Configuration
- `Cargo.toml` - Main workspace configuration
- `config/default.toml` - Application configuration template
- Individual `crates/*/Cargo.toml` - Crate-specific dependencies

## Architecture Changes (2025-08-31)
- **Exchange Integration Moved**: From Prudentia to Formatio for better OODA loop integration
- **Circular Dependency Resolved**: Formatio no longer depends on Prudentia
- **Observer Component Complete**: First functional OODA phase with market data ingestion

## Recent Progress (2025-08-31)
- **Formatio OODA Loop Phase 2**: Observer component with market data observation
- **Exchange Integration**: Complete ExchangeAdapterTrait and MockExchange in Formatio
- **Observer Testing**: 6 comprehensive integration tests covering all scenarios
- **CHANGELOG Updated**: Documented Observer implementation as major Phase 2 update
- **Test Coverage**: 14 total tests (8 OODA state tests, 6 Observer integration tests)

## Current Development Status
- **Disciplina**: Production ready with formal verification
- **Prudentia**: Production ready risk management system
- **Formatio**: Observer phase complete, Orient/Decide/Act phases remain
- **Imperium**: Minimal implementation, requires API development

## Next Development Targets
1. Formatio Orient phase - Position sizing integration with Disciplina
2. Formatio Decide phase - Risk validation integration with Prudentia
3. Formatio Act phase - Order execution via exchange adapters
4. Imperium API development - REST endpoints and WebSocket integration