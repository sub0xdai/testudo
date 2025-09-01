# Src: Testudo Backend Integration Layer

This is the integration layer that unifies all Testudo crates into a cohesive trading platform. It initializes all services, defines the API, and orchestrates the flow of data between the core components.

## Core Architecture: Service Integration
The application uses a dependency injection pattern to manage and share core services. All services are initialized at startup and made available to the API handlers.

### Service Container
The `TestudoServices` struct acts as the central container for all shared application state.
`
pub struct TestudoServices {
    pub disciplina: Arc<VanTharpCalculator>,
    pub prudentia: Arc<RiskEngine>,
    pub formatio: Arc<OODAEngine>,
    pub database: Arc<PostgresPool>,
    pub cache: Arc<RedisClient>,
    pub exchange: Arc<BinanceAdapter>,
}
`
### Application Startup (`main.rs`)
The main entry point initializes services and configures the `axum` router with essential middleware.
`
// Simplified main function logic
async fn main() {
    // 1. Initialize all services within TestudoServices.
    let services = Arc::new(TestudoServices::new().await?);

    // 2. Build the API router.
    let app = build_router()
        // 3. Inject services and apply middleware.
        .layer(Extension(services))
        .layer(middleware::from_fn(request_timing_middleware))
        .layer(middleware::from_fn(risk_validation_middleware));

    // 4. Start the server.
    axum::serve(listener, app).await?;
}
`
---

## API & Real-Time Endpoints
The backend exposes a RESTful API for core operations and WebSockets for real-time data streaming.

### API Route Map
The `build_router` function provides a clear map of all available API endpoints.
`
pub fn build_router() -> Router {
    Router::new()
        .nest("/api/v1/trades", trade_routes())
        .nest("/api/v1/positions", position_routes())
        .nest("/api/v1/portfolio", portfolio_routes())
        .nest("/ws", websocket_routes())
        .route("/health", get(health_check))
}
`
### WebSocket Endpoints
-   **/ws/market-data**: Streams real-time market data (e.g., ticker prices) to the client.
-   **/ws/trade-execution**: Provides real-time status updates on trade execution requests sent through the OODA loop.

---

## Key Architectural Patterns

### Middleware
Cross-cutting concerns are handled via `axum` middleware:
-   **Request Timing**: Logs the latency of all API requests and warns if they exceed performance targets.
-   **Risk Validation**: Intercepts trading operations to ensure they comply with the Testudo Protocol before execution (though primary validation is in the OODA loop).

### Database Access (Repository Pattern)
Data is accessed through a repository pattern that abstracts the SQL queries.
`
pub struct TradeRepository {
    pool: PgPool,
}

impl TradeRepository {
    pub async fn create_trade(&self, trade: &NewTrade) -> Result<TradeRecord, DatabaseError>;
    pub async fn get_user_trades_in_period(
        &self, user_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>
    ) -> Result<Vec<TradeRecord>, DatabaseError>;
}
`
---

## Integration Testing Mandate
The integrity of the entire system must be verified with end-to-end integration tests that simulate a complete user workflow, from API request to database verification.
`
#[tokio::test]
async fn test_complete_trade_workflow() {
    let app = create_test_app().await;

    // 1. Send a request to the `/calculate-position` endpoint.
    // 2. Assert the response is successful and the calculation is correct.
    // 3. Send a request to the `/execute` endpoint using the calculated size.
    // 4. Assert the trade execution was successful.
    // 5. Verify that the correct trade record was created in the database.
}
```

---

## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
```
cargo nextest run | tdd-guard-rust --passthrough
```

### Additional Commands
- **Run integration tests**: `cargo test --package testudo integration`
- **Run all services tests**: `cargo test --workspace`
- **Start development server**: `cargo run --bin testudo`
- **Database migrations**: `sqlx migrate run`
`
