# Formatio: OODA Loop Trading Context

## 🏛️ Mission: Systematic Trading Through Formation Discipline

**Formatio** implements the OODA loop (Observe → Orient → Decide → Act) for systematic crypto trading. Like a Roman military formation, every component works in precise coordination to execute disciplined trading decisions.

---

## 🎯 OODA Loop Architecture

### The Four Pillars of Formatio

#### 1. **OBSERVE** - Market Data Ingestion
```rust
pub trait MarketObserver {
    async fn observe_market_data(&self) -> Result<MarketSnapshot, ObservationError>;
    async fn validate_data_integrity(&self, snapshot: &MarketSnapshot) -> bool;
    fn get_latency_metrics(&self) -> LatencyMetrics;
}

// Target: <100ms WebSocket update latency
```

#### 2. **ORIENT** - Situation Assessment  
```rust
pub trait SituationAssessment {
    fn analyze_market_conditions(&self, data: MarketSnapshot) -> MarketConditions;
    fn evaluate_portfolio_status(&self, positions: &[Position]) -> PortfolioHealth;
    fn calculate_optimal_position_size(&self, setup: TradeSetup) -> PositionRecommendation;
}
```

#### 3. **DECIDE** - Testudo Protocol Enforcement
```rust
pub trait DecisionEngine {
    fn validate_against_protocol(&self, trade: &ProposedTrade) -> ProtocolValidation;
    fn apply_risk_filters(&self, trade: ProposedTrade) -> FilteredTrade;
    fn generate_execution_plan(&self, trade: FilteredTrade) -> ExecutionPlan;
}
```

#### 4. **ACT** - Order Execution
```rust
pub trait OrderExecutor {
    async fn execute_trade(&self, plan: ExecutionPlan) -> Result<TradeResult, ExecutionError>;
    async fn confirm_execution(&self, trade_id: TradeId) -> ExecutionConfirmation;
    fn track_slippage(&self, expected: Price, actual: Price) -> SlippageMetrics;
}
```

---

## 🔧 Implementation Patterns

### Real-Time Data Pipeline
```rust
// WebSocket → Validation → Redis Cache → Decision Engine
pub struct MarketDataPipeline {
    websocket_manager: BinanceWebSocketManager,
    data_validator: MarketDataValidator,
    cache_layer: RedisCache,
    decision_engine: Arc<DecisionEngine>,
}

impl MarketDataPipeline {
    // Target: <100ms end-to-end processing
    pub async fn process_market_update(&self, raw_data: RawMarketData) -> ProcessingResult {
        let validated = self.data_validator.validate(raw_data)?;
        self.cache_layer.update(validated.clone()).await?;
        self.decision_engine.notify_market_change(validated).await
    }
}
```

### Decision Engine Architecture
```rust
pub struct TestudoDecisionEngine {
    risk_calculator: Arc<VanTharpCalculator>, // From Disciplina crate
    protocol_enforcer: ProtocolEnforcer,      // From Prudentia crate  
    portfolio_tracker: PortfolioTracker,
    circuit_breakers: CircuitBreakerManager,
}

// Every decision follows the Roman principle: 
// "Better to preserve than to risk unnecessary loss"
impl DecisionEngine for TestudoDecisionEngine {
    fn validate_against_protocol(&self, trade: &ProposedTrade) -> ProtocolValidation {
        // 1. Individual trade risk ≤ 6% account equity
        // 2. Total portfolio risk ≤ 10% account equity  
        // 3. No more than 3 consecutive losses
        // 4. Daily loss limit enforcement
    }
}
```

---

## ⚡ Performance Requirements

### Latency Targets (CRITICAL)
- **Market Observation**: <100ms WebSocket to cache
- **Situation Assessment**: <50ms analysis completion
- **Decision Making**: <25ms protocol validation
- **Order Execution**: <200ms UI to exchange confirmation

### Throughput Requirements
- Process 1000+ market updates per second
- Handle 10+ concurrent trade decisions  
- Support 100+ active portfolio positions
- Maintain <1% system resource utilization

---

## 🚨 Circuit Breakers & Risk Controls

### Automated Trading Halts
```rust
pub enum CircuitBreakerTrigger {
    ConsecutiveLosses(u32),           // Default: 3 losses
    DailyLossLimit(Decimal),          // Configurable per user
    PortfolioRiskExceeded(Decimal),   // >10% total risk
    SystemLatencyExceeded(Duration),  // >500ms average
    ExchangeConnectivityLoss,         // WebSocket disconnection
}

impl CircuitBreakerManager {
    pub fn should_halt_trading(&self) -> Option<CircuitBreakerTrigger> {
        // Roman principle: "When in doubt, preserve the legion"
        // Better to miss opportunity than risk destruction
    }
}
```

### Recovery Protocols
```rust
pub enum RecoveryAction {
    AutomaticResume,      // After connectivity restoration
    ManualReview,         // After loss limits triggered  
    SystemReboot,         // After critical errors
    EmergencyShutdown,    // Unrecoverable failures
}
```

---

## 📊 Data Flow Architecture

### Market Data Sources
```rust
// Primary: Binance WebSocket API
// Backup: REST API polling (fallback)
// Validation: Cross-reference multiple sources
pub struct DataSourceManager {
    primary_source: BinanceWebSocket,
    backup_sources: Vec<Box<dyn MarketDataSource>>,
    validator: MultiSourceValidator,
}
```

### State Management
```rust
// Immutable state transitions for auditability
pub struct TradingState {
    portfolio: PortfolioSnapshot,
    market_conditions: MarketSnapshot,  
    active_positions: Vec<Position>,
    risk_metrics: RiskMetrics,
    timestamp: SystemTime,
}

// Every state change logged for audit trail
impl StateMachine for TradingState {
    type Event = TradingEvent;
    type Error = StateTransitionError;
    
    fn transition(self, event: Self::Event) -> Result<Self, Self::Error>;
}
```

---

## 🧪 Testing Strategy

### Integration Testing (MANDATORY)
```rust
// Test complete OODA loop with simulated market data
#[tokio::test]
async fn complete_ooda_cycle_integration_test() {
    let market_sim = MarketSimulator::new();
    let ooda_engine = OODAEngine::new();
    
    // Observe: Inject market data
    market_sim.send_price_update(btc_price_change(-5.0)).await;
    
    // Orient: Verify situation assessment  
    let assessment = ooda_engine.assess_situation().await?;
    assert!(assessment.suggests_action());
    
    // Decide: Check protocol compliance
    let decision = ooda_engine.make_decision(assessment).await?;
    assert!(decision.complies_with_protocol());
    
    // Act: Execute and verify (using mock exchange)
    let result = ooda_engine.execute_decision(decision).await?;
    assert!(result.completed_within_latency_target());
}
```

### Performance Testing
```bash
# Latency benchmarks
cargo bench formatio::ooda_cycle_latency

# Throughput stress testing  
cargo test formatio::stress_tests --ignored --release

# Memory usage profiling
cargo test formatio::memory_profile --test-threads=1
```

---

## 🔄 Monitoring & Observability

### Key Metrics
```rust
pub struct OODAMetrics {
    observe_latency: Histogram,      // WebSocket update timing
    orient_duration: Histogram,      // Analysis completion time
    decide_latency: Histogram,       // Decision making speed
    act_execution_time: Histogram,   // Order execution timing
    
    circuit_breaker_triggers: Counter,
    failed_executions: Counter,
    slippage_measurements: Histogram,
}
```

### Alerting Thresholds
- **Latency Alert**: Any stage >2x target latency
- **Error Rate Alert**: >1% execution failures
- **Circuit Breaker Alert**: Any trigger activation
- **Slippage Alert**: >0.5% average slippage

---

## 📋 Development Checklist

### Before Implementing New OODA Components
- [ ] Understand impact on overall cycle timing
- [ ] Design for fail-fast behavior
- [ ] Implement comprehensive error handling
- [ ] Add circuit breaker integration points
- [ ] Write integration tests covering full cycle
- [ ] Benchmark against latency targets
- [ ] Document state transition logic

### Code Review Requirements  
- [ ] All async operations have timeout handling
- [ ] State transitions are logged for audit trail
- [ ] Error recovery paths are clearly defined
- [ ] Performance benchmarks meet targets
- [ ] Integration with Disciplina and Prudentia verified
- [ ] Circuit breaker logic tested under failure conditions

---

## 🏛️ The Formatio Way

*"Formatio perfecta, victoria assecrata" - Perfect formation ensures victory.*

Like Roman legions in battle formation, every component of the OODA loop must work in perfect coordination. Speed without accuracy leads to defeat; accuracy without speed misses opportunity. Formatio achieves both through systematic discipline and relentless practice.

The OODA loop is not just a process—it's a mindset. Observe with the eyes of a scout, orient with the wisdom of a general, decide with the courage of a centurion, and act with the precision of a siege engine.

---

**Crate Version**: 0.1.0  
**Primary Latency Target**: <200ms complete OODA cycle  
**Reliability Target**: 99.9% successful decision execution  
**Integration Points**: Disciplina, Prudentia, Imperium crates