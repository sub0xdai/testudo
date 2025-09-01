# 🏛️ Testudo Trading Platform

> *"In trading, as in war, discipline separates victory from defeat."*

**Testudo** is a disciplined crypto trading platform that implements Van Tharp position sizing methodology with Roman military precision and systematic risk management. Built in Rust for performance, safety, and mathematical accuracy.

## 🎯 Core Mission

Remove human emotion from position sizing decisions through formal mathematical verification, enabling traders to focus on market analysis while the platform handles all risk calculations with 99.9% accuracy and sub-200ms execution latency.

## 🏛️ Roman Military Principles

- **Disciplina**: Mathematical precision without deviation in all financial calculations
- **Formatio**: Systematic execution following the OODA Loop under all market conditions  
- **Prudentia**: Risk-aware decision making in all trading operations
- **Imperium**: Clear command structure and decisive action under pressure

## ⚡ Key Features

- **Automated Van Tharp Position Sizing**: `Position Size = (Account Equity × Risk %) ÷ (Entry - Stop)`
- **Testudo Protocol Risk Management**: A complete, verified risk engine enforces individual trade limits, portfolio exposure caps, and circuit breakers for consecutive losses.
- **OODA Loop Trading**: Systematic Observe → Orient → Decide → Act execution cycle
- **Drag-to-Trade Interface**: Intuitive chart-based order placement with automatic sizing
- **Sub-200ms Latency**: High-performance order execution and market data processing
- **Comprehensive Trade Journal**: Automatic logging with R-multiple analysis

## 🔧 Technical Architecture

### Core Components (Roman Military Organization)

- **Disciplina** (`crates/disciplina`): Van Tharp risk calculation engine with formal verification
- **Formatio** (`crates/formatio`): OODA loop trading operations and execution logic
- **Prudentia** (`crates/prudentia`): Risk management protocol and exchange integration adapters
- **Imperium** (`crates/imperium`): API server and command interface

### Technology Stack

- **Backend**: Rust (Tokio + Axum) monolithic architecture
- **Database**: PostgreSQL + TimescaleDB for time-series trade data
- **Cache**: Redis for real-time market data and position state - This has changed
- **Frontend**: Progressive Web App (Rust Framework)
- **Charts**: TradingView Lightweight Charts integration
- **Monitoring**: Sentry error tracking, Prometheus metrics

## 🚀 Quick Start

### Prerequisites

- Rust 1.75+ with Cargo
- PostgreSQL 14+ with TimescaleDB extension
- Redis 6+
- Binance API credentials (for exchange integration)

### Installation

```bash
# Clone the repository
git clone git@github.com:sub0xdai/testudo.git
cd testudo

# Install Rust dependencies
cargo build --release

# Set up database
createdb testudo
psql testudo -c "CREATE EXTENSION IF NOT EXISTS timescaledb;"

# Run database migrations
cargo run --bin migrate

# Copy configuration template
cp config/default.toml config/local.toml
# Edit config/local.toml with your settings

# Start the platform
cargo run --bin testudo --config config/local.toml
```

### Development Setup

```bash
# Install development dependencies
cargo install sqlx-cli
cargo install cargo-watch

# Start development server with auto-reload
cargo watch -x "run --bin testudo"

# Run tests (including property-based verification)
cargo test

# Run benchmarks
cargo bench
```

## 📊 Performance Targets

- **Order Execution**: <200ms from UI interaction to exchange confirmation
- **Position Calculation**: <50ms Van Tharp formula execution  
- **Market Data Latency**: <100ms WebSocket price updates
- **System Uptime**: 99.9% availability during market hours
- **Calculation Accuracy**: Zero tolerance for mathematical errors

## 🔒 Security Model

- **API Key Storage**: AES-256 encrypted exchange credentials
- **Network Security**: TLS 1.3 for all external communications
- **Database Protection**: Transparent Data Encryption (TDE)
- **Access Control**: JWT-based authentication with role-based permissions
- **Audit Logging**: Comprehensive audit trail for all trading operations

## 📋 Configuration

Key configuration sections in `config/default.toml`:

```toml
[risk_management]
default_risk_percentage = 0.02  # 2% default risk per trade
max_risk_percentage = 0.06     # 6% maximum risk allowed
max_portfolio_risk = 0.10      # 10% total portfolio risk limit

[ooda]
max_loop_duration = "200ms"    # Complete OODA cycle target
max_market_data_age = "5s"     # Market data freshness limit

[exchanges.binance]
api_key = ""  # Set via environment variable
api_secret = ""  # Set via environment variable  
```

## 🧪 Testing

The platform includes comprehensive testing with formal verification:

```bash
# Unit tests
cargo test

# Property-based testing (10,000+ iterations)
cargo test prop_ -- --ignored

# Performance benchmarks
cargo bench position_sizing

# Integration tests
cargo test --test integration

# Security audit
cargo audit
```

## 📚 Documentation

- [`technical_spec.md`](technical_spec.md) - Complete technical specification
- [`architecture.md`](architecture.md) - C4 model system architecture  
- [`CLAUDE.md`](CLAUDE.md) - AI development context and guidelines
- [`sop/`](sop/) - Standard Operating Procedures for all operations
- [`prd.md`](prd.md) - Product Requirements Document

## 🔄 Development Workflow

Following GEMINI principles for monotonic development:

1. **Add-Only Philosophy**: Never modify existing risk calculation rules
2. **Property-Based Testing**: All financial math verified with 10,000+ test iterations
3. **Formal Verification**: Mathematical proofs required for core algorithms
4. **SOP-Driven**: Every operation follows documented Standard Operating Procedures

## 📈 Roadmap

### Phase 1: Core Risk Engine ✅ **COMPLETED**
- **Status**: Implemented, tested, and verified.
- **Components**: `disciplina` and `prudentia` crates.
- **Features**:
    - [x] Van Tharp position sizing calculator.
    - [x] Testudo Protocol rule enforcement (individual, portfolio, and circuit-breaker limits).
    - [x] Comprehensive property-based and integration testing suite.
    - [x] Database schema with audit trails.

### Phase 2: OODA Trading Loop ✅ **COMPLETED**
- **Status**: Implemented, tested, and verified.
- **Components**: `formatio` crate.
- **Features**:
    - [x] Complete OODA loop implementation (Observe, Orient, Decide, Act).
    - [x] Market data observation layer with freshness checks.
    - [x] Position orientation with Van Tharp integration.
    - [x] Risk-based decision making with protocol enforcement.
    - [x] Order execution with exchange adapter.

### Phase 3: User Interface (Planned)
- [ ] Progressive Web App foundation
- [ ] TradingView chart integration
- [ ] Drag-based trade setup interface
- [ ] Real-time portfolio monitoring

## 🤝 Contributing

1. Read [`CLAUDE.md`](CLAUDE.md) for development context
2. Follow Roman naming conventions and architectural patterns
3. All financial calculations require property-based test verification
4. Submit pull requests with comprehensive test coverage

## 📜 License

MIT License - See [LICENSE](LICENSE) file for details.

## ⚖️ Disclaimer

This software is for educational and research purposes. Trading cryptocurrencies involves substantial risk of loss. Past performance does not guarantee future results. Users are responsible for their own trading decisions and risk management.

---

*"Just as the Roman testudo formation protected soldiers through disciplined coordination, our trading platform protects capital through systematic risk management."*

**Imperium**: Command your trades with the precision of a Roman general.
