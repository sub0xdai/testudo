# Testudo Trading Platform - Technical Specification

## 🏛️ Executive Summary

Testudo is a disciplined crypto trading platform that enforces Van Tharp position sizing methodology through a drag-based interface. Built with Rust for performance and safety, it embodies Roman military principles of discipline, precision, and systematic execution.

**Core Innovation**: Complete automation of position sizing decisions, removing emotional trading while maintaining Robinhood-level simplicity with FxBlue-style analytics.

---

## 🎯 System Architecture Philosophy

### Roman Military Principles in Code
- **Disciplina**: Strict type safety, immutable risk rules
- **Formatio**: Systematic OODA loop architecture
- **Prudentia**: Conservative defaults, formal verification
- **Imperium**: Clear command structure, monolithic deployment

### OODA Loop Trading Architecture
```
Observe (Market Data) → Orient (Position Sizing) → Decide (Risk Validation) → Act (Order Execution)
```

---

## 🏗️ Technical Architecture

### Architecture Pattern: Monolithic Rust Application
**Rationale**: For 100-1000 concurrent users, monolithic architecture provides:
- Lower latency (<200ms target)
- Simplified deployment
- Better resource utilization
- Easier debugging and monitoring

### Core Stack
```
┌─────────────────────────────────────────┐
│             Frontend (PWA)              │
│      TradingView + React/Leptos         │
└─────────────────────────────────────────┘
                    │ WebSocket
┌─────────────────────────────────────────┐
│          Axum Web Server (Rust)         │
│     ┌─────────────┬─────────────────┐   │
│     │   API       │   WebSocket     │   │
│     │   Routes    │   Handler       │   │
│     └─────────────┴─────────────────┘   │
└─────────────────────────────────────────┘
                    │
┌─────────────────────────────────────────┐
│            Trading Core                 │
│  ┌──────────┬──────────┬──────────────┐ │
│  │ OODA     │ Risk     │ Exchange     │ │
│  │ Loop     │ Engine   │ Adapter      │ │
│  └──────────┴──────────┴──────────────┘ │
└─────────────────────────────────────────┘
                    │
┌─────────────────────────────────────────┐
│      Data Layer (PostgreSQL + Redis)   │
│  ┌──────────────┬──────────────────────┐ │
│  │ TimescaleDB  │      Redis           │ │
│  │ (Journal)    │  (Real-time State)   │ │
│  └──────────────┴──────────────────────┘ │
└─────────────────────────────────────────┘
```

---

## 🧮 Core Components

### 1. Monotonic Risk Engine (COMPLETED & VERIFIED)
**Philosophy**: Add-only development - risk rules never modified, only extended.
**Crates**: `disciplina`, `prudentia`

```rust
// crates/disciplina/src/calculator.rs
pub struct VanTharpCalculator;

impl PositionSizeCalculator for VanTharpCalculator {
    // Position Size = (Account Risk $) / (Entry - Stop)
    fn calculate_position_size(
        &self,
        account_equity: Decimal,
        risk_percentage: Decimal,
        entry_price: Decimal,
        stop_loss: Decimal,
    ) -> Result<PositionSize, RiskError> {
        // Formally verified with property-based testing to be bulletproof
    }
}
```

**Key Features**:
- **Completed**: The core engine is fully implemented and tested.
- **Verified**: Passed a suite of 60+ unit tests and 8 property-based tests with 10,000+ iterations each.
- **Type-Safe**: Compile-time validation for all financial types using `rust_decimal`.
- **Immutable Rules**: The Testudo Protocol's core rules (6% trade risk, 10% portfolio risk) are immutable.

### 2. OODA Trading Loop (PLANNED)
**Implementation**: Each phase as separate module following "one declaration per file" principle.

```rust
// trading/ooda/observe.rs - Market data ingestion
pub struct MarketObserver;

// trading/ooda/orient.rs - Position sizing analysis  
pub struct PositionOrientator;

// trading/ooda/decide.rs - Risk validation
pub struct RiskDecider;

// trading/ooda/act.rs - Order execution
pub struct OrderExecutor;
```

### 3. Exchange Integration (PLANNED)
**Primary**: Binance (high volume, good documentation)
**Pattern**: Adapter pattern for future exchange additions

```rust
// trading/exchange/binance_adapter.rs
pub struct BinanceAdapter {
    websocket_client: BinanceWebSocket,
    rest_client: BinanceRest,
}

impl ExchangeAdapter for BinanceAdapter {
    async fn place_order(&self, order: Order) -> Result<OrderResult, ExchangeError>;
    async fn get_market_data(&self) -> Result<MarketData, ExchangeError>;
}
```

### 4. Trade Journal System (PLANNED)
**Database**: PostgreSQL + TimescaleDB for time-series analysis

```sql
-- Schema for comprehensive trade analytics
CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    side VARCHAR(4) NOT NULL, -- LONG/SHORT
    entry_price DECIMAL(18,8) NOT NULL,
    exit_price DECIMAL(18,8),
    position_size DECIMAL(18,8) NOT NULL,
    stop_loss DECIMAL(18,8) NOT NULL,
    take_profit DECIMAL(18,8),
    r_multiple DECIMAL(10,4), -- Van Tharp R-multiple
    pnl DECIMAL(18,8),
    status VARCHAR(20) NOT NULL -- OPEN/CLOSED/STOPPED
);

-- TimescaleDB hypertable for performance
SELECT create_hypertable('trades', 'timestamp');
```

---

## 🎨 Frontend Architecture (PLANNED)

### Progressive Web App (PWA)
**Rationale**: Balances performance with accessibility requirements.

**Technology Stack**:
- **Charts**: TradingView Lightweight Charts (not full library)
- **Framework**: React with TypeScript or Leptos (Rust WASM)
- **Styling**: Tailwind CSS with Roman military design system
- **State**: Real-time via WebSocket connection

### Drag-to-Trade Interface
```typescript
// Simplified conceptual interface
interface TradeSetup {
    symbol: string;
    side: 'LONG' | 'SHORT';
    entry_price: number;
    stop_loss: number;
    take_profit: number;
    // Position size calculated automatically by backend
}
```

### Roman Military Theme
- **Color Palette**: Deep reds, golds, bronze
- **Typography**: Clean, authoritative fonts
- **Iconography**: Subtle shield/spear elements
- **Terminology**: "Command Center", "Battle Plan", "Formation"

---

## 🔐 Security & Risk Management

### Testudo Protocol (IMPLEMENTED & ENFORCED)
1. **Maximum 6% risk per trade** (user configurable 0.5-6%)
2. **Maximum 10% total portfolio risk** across all open positions
3. **Daily loss limits** following prop firm standards:
   - 5% daily drawdown limit
   - 10% maximum overall drawdown
   - Trading halt after 3 consecutive losses
4. **Position size validation** before order placement
5. **Automatic stop-loss enforcement** (no trades without stops)

### Security Implementation
```rust
// crates/prudentia/src/protocol.rs
pub struct TestudoProtocol {
    rules: Vec<Box<dyn RiskRule>>,
}

impl RiskValidator for TestudoProtocol {
    fn validate_trade(&self, trade: &ProposedTrade) -> Result<(), RiskViolation> {
        for rule in &self.rules {
            rule.validate(trade)?;
        }
        Ok(())
    }
}
```

---

## 📊 Performance Requirements

### Latency Targets
- **Order execution**: <200ms from click to exchange acknowledgment
- **Market data updates**: <100ms WebSocket latency
- **Position size calculation**: <50ms (Achieved and Benchmarked)
- **UI responsiveness**: 60fps chart rendering

### Scalability Targets
- **Concurrent users**: 100-1000 (MVP → Growth)
- **Orders per second**: 1000 peak
- **Database queries**: <10ms 99th percentile
- **Memory usage**: <512MB per process

### Reliability Targets
- **Uptime**: 99.9% (8.7 hours downtime per year)
- **Data accuracy**: 100% for position sizing calculations (Verified)
- **Order success rate**: >99.5%

---

## 🗄️ Data Architecture

### PostgreSQL Schema Design
```sql
-- User accounts with risk profiles
CREATE TABLE user_accounts (
    id BIGSERIAL PRIMARY KEY,
    user_id VARCHAR(50) NOT NULL, -- Clerk user ID
    exchange VARCHAR(20) NOT NULL,
    api_key_encrypted BYTEA NOT NULL,
    risk_percentage DECIMAL(5,4) DEFAULT 0.02, -- 2% default
    daily_loss_limit DECIMAL(18,8),
    max_positions INTEGER DEFAULT 5,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Position tracking
CREATE TABLE positions (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES user_accounts(id),
    symbol VARCHAR(20) NOT NULL,
    side VARCHAR(4) NOT NULL,
    size DECIMAL(18,8) NOT NULL,
    entry_price DECIMAL(18,8) NOT NULL,
    current_price DECIMAL(18,8),
    unrealized_pnl DECIMAL(18,8),
    stop_loss DECIMAL(18,8),
    take_profit DECIMAL(18,8),
    status VARCHAR(20) DEFAULT 'OPEN',
    opened_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ
);
```

### Redis Cache Strategy
```rust
// Real-time data caching
pub enum CacheKey {
    AccountEquity(String),    // 30s TTL
    MarketPrice(String),      // 5s TTL  
    PositionSizes(String),    // Real-time updates
    RiskExposure(String),     // Real-time updates
}
```

---

## 🔄 Development Workflow

### Test-Driven Development (TDD)
1. **Write failing test** for position sizing logic
2. **Implement minimal code** to pass test
3. **Refactor** while maintaining test coverage
4. **Document** with comprehensive examples

### Quality Gates
- **Unit tests**: 90%+ coverage
- **Integration tests**: 80%+ coverage  
- **Property-based tests** for financial calculations
- **Formal verification** of critical risk logic
- **Performance benchmarks** must pass

### Standard Operating Procedures (SOPs)
Following GEMINI.md principles:
1. **Risk Calculation SOP**: Formal verification process
2. **Order Execution SOP**: Multi-stage validation
3. **System Recovery SOP**: Disaster recovery procedures
4. **Trade Journal SOP**: Data integrity standards

---

## 🚀 Deployment Architecture

### Single Binary Deployment
```rust
// Embedded frontend assets in Rust binary
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

// Complete application in single executable
pub struct TestudoApp {
    web_server: AxumServer,
    trading_engine: TradingCore,
    database: DatabasePool,
}
```

### Infrastructure Requirements
- **Compute**: 4 vCPU, 8GB RAM
- **Database**: PostgreSQL 14+ with TimescaleDB
- **Cache**: Redis 6+  
- **Network**: Low-latency connection to exchange APIs
- **Storage**: 100GB SSD for trade history

---

## 📈 Success Metrics

### Business Metrics
- **User acquisition**: 1000 users within 6 months
- **Revenue**: $49/month subscription after 1-week trial
- **User retention**: >80% monthly retention
- **Trading volume**: $10M+ monthly across platform

### Technical Metrics
- **System uptime**: 99.9%+
- **Order execution latency**: <200ms average
- **Position sizing accuracy**: 100% (zero errors)
- **User satisfaction**: >4.5/5 rating

### Risk Management Metrics
- **Risk rule violations**: Zero
- **Account blowups prevented**: Track saves via position sizing
- **Average R-multiple**: Positive across user base
- **Drawdown protection**: Maximum 10% enforcement

---

## 🔮 Future Roadmap

### Phase 2 (Months 6-12)
- Additional exchanges (Bybit, dYdX)
- Multiple position sizing strategies (Kelly Criterion, Fixed %)
- Advanced trade management rules
- Mobile-optimized interface

### Phase 3 (Year 2)
- Solana token integration
- DeFi protocol trading
- Copy trading functionality
- Institutional features

### Long-term Vision
- AI-powered trade analysis
- Formal verification of entire system
- Cross-asset trading (forex, stocks)
- Regulatory compliance expansion

---

*"In the discipline of the legion, we find the precision of the mathematics. In the formation of the testudo, we discover the safety of systematic risk management."*

**Document Version**: 1.1
**Last Updated**: 2025-08-31
**Architecture Review**: Completed for Risk Engine
