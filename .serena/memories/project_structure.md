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
- `src/exchange/` - Exchange adapters and integration
- `src/monitoring/` - Portfolio tracking and metrics
- `src/types/` - Trade proposals and risk assessment types

### Formatio (`crates/formatio/`) - ❌ PLANNED ONLY
**Purpose**: OODA loop trading operations and execution logic
- Not yet implemented

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