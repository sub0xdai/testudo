# Src: Testudo Backend Integration Layer

## 🏛️ Mission: Unified Command Center for Trading Operations

The **src** directory serves as the integration layer that unifies all Testudo crates into a cohesive trading platform. This is where Disciplina, Formatio, Prudentia, and Imperium work together as a disciplined Roman legion.

---

## 🎯 Architecture Integration

### Rust Tokio + Axum Backend
```rust
// Main application structure
use axum::{Router, Extension, middleware};
use tokio::net::TcpListener;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();
    
    // Initialize core services
    let disciplina = Arc::new(VanTharpCalculator::new());
    let prudentia = Arc::new(RiskEngine::new(disciplina.clone()));
    let formatio = Arc::new(OODAEngine::new(prudentia.clone()));
    
    // Build application router
    let app = build_router()
        .layer(ServiceBuilder::new()
            .layer(Extension(disciplina))
            .layer(Extension(prudentia))
            .layer(Extension(formatio))
            .layer(middleware::from_fn(request_timing_middleware))
            .layer(middleware::from_fn(risk_validation_middleware))
        );
    
    // Start server
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Testudo server running on port 3000");
    
    axum::serve(listener, app).await?;
    Ok(())
}
```

### Service Layer Architecture
```rust
// Integration between crates
pub struct TestudoServices {
    pub disciplina: Arc<VanTharpCalculator>,
    pub prudentia: Arc<RiskEngine>,
    pub formatio: Arc<OODAEngine>,
    pub database: Arc<PostgresPool>,
    pub cache: Arc<RedisClient>,
    pub exchange: Arc<BinanceAdapter>,
}

impl TestudoServices {
    pub async fn new() -> Result<Self, ServiceInitError> {
        // Initialize database connections
        let database = Arc::new(create_db_pool().await?);
        let cache = Arc::new(create_redis_client().await?);
        
        // Initialize exchange adapter
        let exchange = Arc::new(BinanceAdapter::new().await?);
        
        // Initialize core calculation engine
        let disciplina = Arc::new(VanTharpCalculator::new());
        
        // Initialize risk management with disciplina
        let prudentia = Arc::new(RiskEngine::new(disciplina.clone()));
        
        // Initialize OODA loop with all dependencies
        let formatio = Arc::new(OODAEngine::new(
            prudentia.clone(),
            exchange.clone(),
            database.clone(),
            cache.clone(),
        ));
        
        Ok(Self {
            disciplina,
            prudentia,
            formatio,
            database,
            cache,
            exchange,
        })
    }
}
```

---

## 🛣️ API Routes Architecture

### RESTful API Design
```rust
// routes/mod.rs
pub fn build_router() -> Router {
    Router::new()
        // Authentication routes
        .nest("/auth", auth_routes())
        
        // Trading operations
        .nest("/api/v1/trades", trade_routes())
        .nest("/api/v1/positions", position_routes())
        .nest("/api/v1/portfolio", portfolio_routes())
        
        // Market data
        .nest("/api/v1/market", market_routes())
        
        // Risk management
        .nest("/api/v1/risk", risk_routes())
        
        // WebSocket endpoints
        .nest("/ws", websocket_routes())
        
        // Health and monitoring
        .route("/health", get(health_check))
        .route("/metrics", get(prometheus_metrics))
}
```

### Trading API Endpoints
```rust
// routes/trades.rs
use axum::{Json, Extension, extract::Path};
use crate::schemas::{TradeSetupRequest, TradeExecutionResponse};

pub fn trade_routes() -> Router {
    Router::new()
        .route("/calculate-position", post(calculate_position_size))
        .route("/validate-setup", post(validate_trade_setup))
        .route("/execute", post(execute_trade))
        .route("/history", get(get_trade_history))
        .route("/:trade_id", get(get_trade_by_id))
}

// Position size calculation endpoint
async fn calculate_position_size(
    Extension(disciplina): Extension<Arc<VanTharpCalculator>>,
    Extension(prudentia): Extension<Arc<RiskEngine>>,
    Json(request): Json<TradeSetupRequest>,
) -> Result<Json<PositionCalculationResponse>, ApiError> {
    // Validate inputs using Prudentia
    let validation = prudentia.validate_trade_setup(&request)?;
    if !validation.is_valid() {
        return Err(ApiError::InvalidTradeSetup(validation.errors()));
    }
    
    // Calculate position size using Disciplina
    let position_size = disciplina.calculate_position_size(
        request.account_equity,
        request.risk_percentage,
        request.entry_price,
        request.stop_loss,
    )?;
    
    // Return calculation with risk metrics
    Ok(Json(PositionCalculationResponse {
        position_size: position_size.shares,
        risk_amount: position_size.risk_amount,
        reward_potential: position_size.reward_potential,
        r_multiple: position_size.r_multiple,
        protocol_compliance: validation,
    }))
}
```

---

## 🔄 Real-Time WebSocket Integration

### Market Data Streaming
```rust
// WebSocket handler for real-time market data
use axum::extract::ws::{WebSocket, Message};
use futures::{stream::SplitSink, SinkExt, StreamExt};

pub async fn market_data_websocket(
    ws: WebSocketUpgrade,
    Extension(services): Extension<Arc<TestudoServices>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_market_data_socket(socket, services))
}

async fn handle_market_data_socket(
    socket: WebSocket,
    services: Arc<TestudoServices>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to Binance WebSocket
    let mut binance_stream = services.exchange.subscribe_to_ticker("BTCUSDT").await;
    
    // Handle incoming client messages
    let client_receiver = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle client subscription requests
                    handle_client_message(text).await;
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });
    
    // Forward market data to client
    let market_data_forwarder = tokio::spawn(async move {
        while let Some(ticker) = binance_stream.next().await {
            let market_update = MarketUpdate {
                symbol: ticker.symbol,
                price: ticker.price,
                volume: ticker.volume,
                timestamp: ticker.timestamp,
            };
            
            let json = serde_json::to_string(&market_update).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = client_receiver => {},
        _ = market_data_forwarder => {},
    }
}
```

### Trade Execution WebSocket
```rust
// Real-time trade execution updates
pub async fn trade_execution_websocket(
    ws: WebSocketUpgrade,
    Extension(formatio): Extension<Arc<OODAEngine>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_trade_execution_socket(socket, formatio))
}

async fn handle_trade_execution_socket(
    socket: WebSocket,
    formatio: Arc<OODAEngine>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // Listen for trade execution requests
    while let Some(msg) = receiver.next().await {
        if let Ok(Message::Text(text)) = msg {
            match serde_json::from_str::<TradeExecutionRequest>(&text) {
                Ok(trade_request) => {
                    // Execute trade through OODA loop
                    let result = formatio.execute_trade(trade_request).await;
                    
                    // Send real-time updates back to client
                    let response = TradeExecutionResponse {
                        trade_id: result.trade_id,
                        status: result.status,
                        execution_price: result.execution_price,
                        slippage: result.slippage,
                        timestamp: result.timestamp,
                    };
                    
                    let json = serde_json::to_string(&response).unwrap();
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let error_response = json!({
                        "error": "Invalid trade request",
                        "details": e.to_string()
                    });
                    
                    if sender.send(Message::Text(error_response.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}
```

---

## 🔒 Middleware & Security

### Request Timing Middleware
```rust
// Middleware to track API latency
use axum::middleware::Next;
use std::time::Instant;

pub async fn request_timing_middleware(
    request: Request<Body>,
    next: Next<Body>,
) -> impl IntoResponse {
    let start = Instant::now();
    let path = request.uri().path().to_string();
    
    let response = next.run(request).await;
    
    let duration = start.elapsed();
    
    // Log slow requests (>100ms for trading operations)
    if path.starts_with("/api/v1/trades") && duration.as_millis() > 100 {
        tracing::warn!(
            path = %path,
            duration_ms = duration.as_millis(),
            "Slow trading API request"
        );
    }
    
    // Add timing header
    let mut response = response;
    response.headers_mut().insert(
        "X-Response-Time",
        HeaderValue::from_str(&format!("{}ms", duration.as_millis())).unwrap(),
    );
    
    response
}
```

### Risk Validation Middleware
```rust
// Middleware to enforce Testudo Protocol on all trading operations
pub async fn risk_validation_middleware(
    request: Request<Body>,
    next: Next<Body>,
) -> impl IntoResponse {
    // Only validate trading-related endpoints
    if !request.uri().path().starts_with("/api/v1/trades") {
        return next.run(request).await;
    }
    
    // Extract user context for risk limits
    let user_id = extract_user_id(&request).await;
    if user_id.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }
    
    let response = next.run(request).await;
    
    // Log all trading operations for audit trail
    tracing::info!(
        user_id = %user_id.unwrap(),
        path = %request.uri().path(),
        "Trading operation executed"
    );
    
    response
}
```

---

## 📊 Database Integration

### PostgreSQL + TimescaleDB Schema
```rust
// Database models using SQLx
use sqlx::{PgPool, Row};
use chrono::{DateTime, Utc};

#[derive(sqlx::FromRow, Debug)]
pub struct TradeRecord {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub symbol: String,
    pub entry_price: rust_decimal::Decimal,
    pub stop_loss: rust_decimal::Decimal,
    pub target_price: Option<rust_decimal::Decimal>,
    pub position_size: rust_decimal::Decimal,
    pub risk_amount: rust_decimal::Decimal,
    pub status: TradeStatus,
    pub executed_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub realized_pnl: Option<rust_decimal::Decimal>,
}

// Repository pattern for data access
pub struct TradeRepository {
    pool: PgPool,
}

impl TradeRepository {
    pub async fn create_trade(&self, trade: &NewTrade) -> Result<TradeRecord, DatabaseError> {
        let record = sqlx::query_as::<_, TradeRecord>(
            r#"
            INSERT INTO trades (
                user_id, symbol, entry_price, stop_loss, target_price,
                position_size, risk_amount, status, executed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(trade.user_id)
        .bind(&trade.symbol)
        .bind(trade.entry_price)
        .bind(trade.stop_loss)
        .bind(trade.target_price)
        .bind(trade.position_size)
        .bind(trade.risk_amount)
        .bind(TradeStatus::Pending)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;
        
        Ok(record)
    }
    
    // Get trading history with time-series optimization
    pub async fn get_user_trades_in_period(
        &self,
        user_id: uuid::Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TradeRecord>, DatabaseError> {
        let trades = sqlx::query_as::<_, TradeRecord>(
            r#"
            SELECT * FROM trades 
            WHERE user_id = $1 
            AND executed_at >= $2 
            AND executed_at <= $3
            ORDER BY executed_at DESC
            "#,
        )
        .bind(user_id)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(trades)
    }
}
```

---

## ⚡ Configuration & Environment

### Application Configuration
```rust
// config/mod.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub binance: BinanceConfig,
    pub risk: RiskConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RiskConfig {
    pub max_individual_trade_risk: rust_decimal::Decimal,
    pub max_portfolio_risk: rust_decimal::Decimal,
    pub max_consecutive_losses: u32,
    pub circuit_breaker_cooldown_minutes: u32,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let config = config::Config::builder()
            .add_source(config::Environment::with_prefix("TESTUDO"))
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name("config/production").required(false))
            .build()?;
        
        config.try_deserialize()
    }
}
```

### Environment Variables
```bash
# .env.example
TESTUDO_SERVER_HOST=0.0.0.0
TESTUDO_SERVER_PORT=3000
TESTUDO_DATABASE_URL=postgresql://user:pass@localhost:5432/testudo
TESTUDO_REDIS_URL=redis://localhost:6379
TESTUDO_BINANCE_API_KEY=your_api_key_here
TESTUDO_BINANCE_SECRET_KEY=your_secret_key_here

# Risk configuration
TESTUDO_RISK_MAX_INDIVIDUAL_TRADE_RISK=0.06
TESTUDO_RISK_MAX_PORTFOLIO_RISK=0.10
TESTUDO_RISK_MAX_CONSECUTIVE_LOSSES=3
```

---

## 🧪 Integration Testing

### API Testing Strategy
```rust
// Integration tests for complete API workflows
#[tokio::test]
async fn test_complete_trade_workflow() {
    let app = create_test_app().await;
    
    // 1. Calculate position size
    let calc_request = json!({
        "account_equity": "10000.00",
        "risk_percentage": "0.02",
        "entry_price": "50000.00",
        "stop_loss": "49000.00"
    });
    
    let response = app
        .oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri("/api/v1/trades/calculate-position")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(calc_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // 2. Execute trade
    let exec_request = json!({
        "symbol": "BTCUSDT",
        "position_size": "0.2",
        "entry_price": "50000.00",
        "stop_loss": "49000.00",
        "target_price": "52000.00"
    });
    
    let response = app
        .oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri("/api/v1/trades/execute")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(exec_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify trade was recorded in database
    // Verify risk limits were enforced
    // Verify audit trail was created
}
```

---

## 📋 Development Checklist

### Backend Integration Requirements
- [ ] All crates properly integrated through dependency injection
- [ ] Database migrations match schema requirements
- [ ] WebSocket connections handle reconnection gracefully
- [ ] API endpoints meet <200ms response time targets
- [ ] Risk validation middleware cannot be bypassed
- [ ] Comprehensive error handling with proper HTTP status codes
- [ ] Audit logging for all trading operations
- [ ] Integration tests cover complete workflows

### Performance Requirements
- [ ] Database queries optimized with proper indexing
- [ ] Redis caching reduces database load
- [ ] WebSocket connections scale to 1000+ concurrent users
- [ ] API latency <100ms for position calculations
- [ ] Memory usage <2GB for main application
- [ ] Graceful handling of high-frequency market data

---

## 🏛️ The Integration Way

*"Divide et impera, unitas et victoria" - Divide to conquer, unite for victory.*

The src directory serves as the Roman headquarters where all specialized legions (crates) coordinate their efforts. Like a well-organized military campaign, each component has its role, but victory comes through disciplined coordination and unified command.

Every API endpoint, every WebSocket message, every database transaction serves the greater mission: providing systematic, disciplined, mathematically verified position sizing for crypto traders. The backend doesn't just process requests—it enforces the Testudo Protocol with unwavering discipline.

---

**Directory Version**: 0.1.0  
**API Response Target**: <200ms for all trading operations  
**WebSocket Capacity**: 1000+ concurrent connections  
**Database**: PostgreSQL + TimescaleDB for time-series optimization  
**Caching**: Redis for sub-second market data access