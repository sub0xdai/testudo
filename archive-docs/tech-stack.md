

# Testudo Exchange - Technology Stack

## Overview
Testudo is a high-performance centralized cryptocurrency exchange (CEX) built with modern technologies for scalability, real-time trading, and efficient order matching.

## Backend Architecture (Rust)

### Core Technologies
- **Language**: Rust (stable toolchain)
- **Build System**: Cargo
- **Architecture**: Microservices with in-memory order matching

### Backend Services
- **Router** (`router`): HTTP API Gateway
  - REST API endpoints for trading operations
  - User management and authentication
  - Order placement and management
  - Port: 8080

- **Engine** (`engine`): Order Matching Engine
  - In-memory order books with price-time priority
  - Real-time order matching
  - Shadow balance management & position tracking
  - Communicates via PostgreSQL (pg_queue)

- **WebSocket Stream** (`ws-stream`): Real-time Data
  - WebSocket server for live market data
  - Order book updates
  - Trade execution notifications
  - Port: 4000

- **Database Processor** (`db-processor`): Persistent Storage
  - Asynchronous database operations
  - Trade history persistence
  - Background data processing

### Key Rust Dependencies
- **tokio**: Async runtime
- **sqlx**: PostgreSQL database operations
- **pg_queue**: PostgreSQL-based high-performance messaging & caching
- **serde**: JSON serialization/deserialization
- **actix-web**: Web framework for HTTP APIs
- **tokio-tungstenite**: WebSocket implementation
- **uuid**: Unique identifier generation
- **chrono**: Date/time handling

## Frontend Architecture (TypeScript/React)

### Core Technologies
- **Language**: TypeScript
- **Framework**: React 18
- **Build Tool**: Vite
- **Monorepo**: Turbo
- **Package Manager**: Bun

### Frontend Structure
```
testudo-web/
├── apps/web/           # Main trading application
├── packages/ui/        # Shared UI components
└── packages/config/    # Shared configurations
```

### Key Frontend Dependencies
- **React Router**: Client-side routing
- **Lightweight Charts**: TradingView-style charts
- **Axios**: HTTP client for API communication
- **Tailwind CSS**: Utility-first CSS framework
- **TypeScript**: Type safety and developer experience

### Development Tools
- **ESLint**: Code linting
- **Prettier**: Code formatting
- **Turbo**: Monorepo task runner
- **Vite**: Fast development server and bundler

## Data Layer

### PostgreSQL (Unified Storage & Messaging)
- **Version**: 16-alpine
- **Port**: 5000
- **Purpose**:
  - **Persistent Storage**: Trade history, user accounts, exchange credentials
  - **Message Queuing**: `pg_queue` with SKIP LOCKED for inter-service jobs
  - **Pub/Sub**: `pg_notify` (LISTEN/NOTIFY) for real-time WebSocket updates
  - **Caching**: UNLOGGED tables with TTL checks for market data
- **Connection**: SQLx with connection pooling

> **Note**: Redis was previously used but has been deprecated and replaced by PostgreSQL to consolidate the infrastructure stack.

## Infrastructure

### Development Environment
- **Containerization**: Docker Compose
  - PostgreSQL container
  - Redis container
- **Process Management**: Custom bash scripts
  - Service startup/shutdown
  - Health monitoring
  - Log management

### Production Deployment (Kubernetes)
- **Platform**: Google Kubernetes Engine (GKE)
- **GitOps**: ArgoCD for continuous deployment
- **Ingress**: NGINX with TLS certificates
- **Monitoring**: Prometheus + Grafana
- **Secrets**: Sealed Secrets for credential management

## Development Workflow

### Backend Development
```bash
# Build and test
cargo build
cargo test
cargo fmt
cargo clippy

# Run individual services
cargo run --bin router
cargo run --bin engine
cargo run --bin ws-stream
cargo run --bin db-processor
```

### Frontend Development
```bash
# Install dependencies
bun install

# Start development server
bun run dev

# Build for production
bun run build

# Linting and formatting
bun run lint
bun run format
```

### Service Management
```bash
# Start all services
./scripts/start-exchange.sh

# Check status
./scripts/status-exchange.sh

# View logs
./scripts/logs-exchange.sh

# Stop all services
./scripts/stop-exchange.sh
```

## Environment Configuration

### Required Environment Variables
```bash
# Database
DATABASE_URL=postgres://root:root@localhost:5000/exchange-db
PG__USER=root
PG__PASSWORD=root
PG__HOST=localhost
PG__PORT=5000
PG__DBNAME=exchange-db

# Services
SERVER_ADDR=127.0.0.1:3001
WS_STREAM_URL=127.0.0.1:4000
```

## Key Architectural Decisions

### Why Rust for Backend?
- **Performance**: Zero-cost abstractions for high-frequency trading
- **Safety**: Memory safety without garbage collection
- **Concurrency**: Excellent async/await support with Tokio
- **Type System**: Prevents many runtime errors at compile time

### Why In-Memory Order Books?
- **Speed**: Microsecond-level order matching
- **Simplicity**: Direct memory access without database overhead
- **Scalability**: Easy to distribute across multiple instances

### Why Microservices?
- **Separation of Concerns**: Each service has a single responsibility
- **Scalability**: Scale services independently based on load
- **Reliability**: Service isolation prevents cascade failures
- **Development**: Teams can work on different services independently

### Why PostgreSQL for Messaging & Caching?
- **Infrastructure Consolidation**: Reduces operational complexity by using a single, battle-tested database.
- **Transactional Atomicity**: Enables atomic updates to both business state and queues (transactional outbox pattern).
- **Latency**: `pg_notify` (LISTEN/NOTIFY) provides sub-millisecond wake-up times for queue consumers.
- **Reliability**: Mature persistence and backup tools ensure zero message loss for non-ephemeral queues.
- **Performance**: `UNLOGGED` tables for caching achieve high write throughput for volatile data.

## Performance Characteristics

### Order Matching
- **Latency**: Sub-millisecond order matching
- **Throughput**: Thousands of orders per second
- **Algorithm**: Price-time priority matching

### Real-time Data
- **WebSocket**: Low-latency market data streaming
- **Updates**: Order book changes pushed immediately
- **Scalability**: Multiple concurrent WebSocket connections

### Database Operations
- **Async**: Non-blocking database writes
- **Connection Pooling**: Efficient database connection management
- **Persistence**: Trade history stored reliably

## Security Considerations

### Development
- **Type Safety**: Rust's type system prevents many vulnerabilities
- **Input Validation**: All API inputs validated
- **Error Handling**: Comprehensive error handling prevents crashes

### Production
- **TLS**: All external communications encrypted
- **Secrets Management**: Kubernetes Sealed Secrets
- **Network Isolation**: Service-to-service communication within cluster
- **Monitoring**: Comprehensive logging and metrics collection
