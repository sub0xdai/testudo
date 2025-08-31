# Testudo - Technology Stack & Architecture

## Core Technology Stack
- **Backend**: Rust (Tokio + Axum) monolithic architecture
- **Database**: PostgreSQL 14+ with TimescaleDB extension for time-series data
- **Cache**: Redis 6+ for real-time market data and position state
- **Frontend**: Progressive Web App (React/TypeScript) - NOTE: Leptos also mentioned as option
- **Charts**: TradingView Lightweight Charts integration
- **Monitoring**: Sentry error tracking, Prometheus metrics

## Architecture Pattern
Monolithic Rust Application chosen for:
- Lower latency (<200ms target)
- Simplified deployment
- Better resource utilization for 100-1000 concurrent users
- Easier debugging and monitoring

## Crate Organization (Roman Legion Structure)
- **Disciplina** (`crates/disciplina`): Van Tharp risk calculation engine with formal verification - ✅ COMPLETED
- **Formatio** (`crates/formatio`): OODA loop trading operations and execution logic - ❌ PLANNED ONLY
- **Prudentia** (`crates/prudentia`): Risk management protocol and exchange integration adapters - ✅ SUBSTANTIALLY COMPLETE
- **Imperium** (`crates/imperium`): API server and command interface - ❌ MINIMAL

## Performance Targets
- Order execution: <200ms from UI to exchange confirmation
- Position calculation: <50ms Van Tharp formula execution  
- Market data latency: <100ms WebSocket updates
- System uptime: 99.9% during market hours

## Deployment
Single Rust binary deployment with embedded frontend assets