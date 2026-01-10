#!/bin/bash

# Testudo Exchange - Start Script
# Gracefully starts all exchange services in the correct order

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
EXCHANGE_DIR="$PROJECT_ROOT/testudo-exchange"
WEB_DIR="$PROJECT_ROOT/testudo-web"
PID_DIR="$SCRIPT_DIR/.pids"

# Default options
BACKGROUND=false
SKIP_BUILD=false
RESET_DB=false
VERBOSE=false

# Usage function
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Start all Testudo Exchange services"
    echo ""
    echo "Options:"
    echo "  --background, -b    Run services in background"
    echo "  --skip-build, -s    Skip compilation step"
    echo "  --reset-db, -r      Reset database (drop and recreate)"
    echo "  --verbose, -v       Verbose output"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Services started:"
    echo "  - PostgreSQL (port 5000)"
    echo "  - Redis (port 6380)"
    echo "  - Router API (port 8080)"
    echo "  - Matching Engine"
    echo "  - WebSocket Stream (port 4000)"
    echo "  - Database Processor"
    echo "  - Frontend Dev Server (port 5173)"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --background|-b)
            BACKGROUND=true
            shift
            ;;
        --skip-build|-s)
            SKIP_BUILD=true
            shift
            ;;
        --reset-db|-r)
            RESET_DB=true
            shift
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

verbose_log() {
    if [[ "$VERBOSE" == "true" ]]; then
        echo -e "${BLUE}[VERBOSE]${NC} $1"
    fi
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    local missing_deps=()

    if ! command_exists docker; then
        missing_deps+=("docker")
    fi

    if ! command_exists cargo; then
        missing_deps+=("cargo (Rust)")
    fi

    if ! command_exists node; then
        missing_deps+=("node.js")
    fi

    if ! command_exists yarn; then
        missing_deps+=("yarn")
    fi

    if ! command_exists sqlx; then
        missing_deps+=("sqlx-cli")
    fi

    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        log_info "Please install missing dependencies and try again"
        exit 1
    fi

    log_success "All prerequisites satisfied"
}

# Check if port is available
check_port() {
    local port=$1
    local service=$2

    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null; then
        log_error "Port $port is already in use (required for $service)"
        return 1
    fi
    return 0
}

# Check all required ports
check_ports() {
    log_info "Checking required ports..."

    local ports_services=(
        "5000:PostgreSQL"
        "6380:Redis"
        "8080:Router API"
        "4000:WebSocket"
        "5173:Frontend"
    )

    for port_service in "${ports_services[@]}"; do
        IFS=':' read -r port service <<< "$port_service"
        if ! check_port "$port" "$service"; then
            log_error "Port conflicts detected. Please free the ports and try again."
            exit 1
        fi
    done

    log_success "All required ports are available"
}

# Wait for service to be ready
wait_for_service() {
    local host=$1
    local port=$2
    local service_name=$3
    local timeout=${4:-30}

    verbose_log "Waiting for $service_name to be ready on $host:$port"

    for i in $(seq 1 $timeout); do
        if nc -z "$host" "$port" 2>/dev/null; then
            log_success "$service_name is ready"
            return 0
        fi
        sleep 1
    done

    log_error "$service_name failed to start within $timeout seconds"
    return 1
}

# Start infrastructure services
start_infrastructure() {
    log_info "Starting infrastructure services..."

    cd "$EXCHANGE_DIR/docker"

    # Copy environment file if it doesn't exist
    if [[ ! -f .env ]]; then
        if [[ -f .env.example ]]; then
            cp .env.example .env
            log_info "Created .env from .env.example"
        else
            log_error ".env.example not found in docker directory"
            exit 1
        fi
    fi

    # Start PostgreSQL and Redis
    verbose_log "Starting Docker containers..."
    docker compose up -d

    # Wait for services to be ready
    wait_for_service localhost 5000 "PostgreSQL" 30
    wait_for_service localhost 6380 "Redis" 30

    log_success "Infrastructure services started"
}

# Setup database
setup_database() {
    log_info "Setting up database..."

    cd "$EXCHANGE_DIR"

    # Set database URL
    export DATABASE_URL="postgres://root:root@localhost:5000/exchange-db"

    if [[ "$RESET_DB" == "true" ]]; then
        log_info "Resetting database..."
        sqlx database drop -y || true
    fi

    # Create database if it doesn't exist
    verbose_log "Creating database if not exists..."
    sqlx database create

    # Run migrations
    verbose_log "Running database migrations..."
    sqlx migrate run --source crates/sqlx_postgres/migrations

    log_success "Database setup completed"
}

# Build backend services
build_backend() {
    if [[ "$SKIP_BUILD" == "true" ]]; then
        log_info "Skipping backend build (--skip-build flag)"
        return 0
    fi

    log_info "Building backend services..."

    cd "$EXCHANGE_DIR"
    verbose_log "Running cargo build..."
    cargo build

    log_success "Backend build completed"
}

# Start backend service
start_backend_service() {
    local service=$1
    local port=${2:-""}
    local env_vars=${3:-""}

    cd "$EXCHANGE_DIR"

    local cmd="cargo run --bin $service"
    if [[ -n "$env_vars" ]]; then
        cmd="$env_vars $cmd"
    fi

    if [[ "$BACKGROUND" == "true" ]]; then
        verbose_log "Starting $service in background..."
        nohup bash -c "$cmd" > "$PID_DIR/$service.log" 2>&1 &
        echo $! > "$PID_DIR/$service.pid"

        if [[ -n "$port" ]]; then
            wait_for_service localhost "$port" "$service" 15
        else
            sleep 2  # Give service time to start
        fi
    else
        log_info "Starting $service (press Ctrl+C to stop all services)..."
        echo "Starting $service..." > "$PID_DIR/$service.log"
        bash -c "$cmd" &
        echo $! > "$PID_DIR/$service.pid"

        if [[ -n "$port" ]]; then
            wait_for_service localhost "$port" "$service" 15
        else
            sleep 2
        fi
    fi
}

# Start backend services
start_backend() {
    log_info "Starting backend services..."

    # Environment variables for services
    local db_env="DATABASE_URL=postgres://root:root@localhost:5000/exchange-db"
    local redis_env="REDIS_URL=redis://localhost:6380"
    local ws_env="WS_STREAM_URL=127.0.0.1:4000"

    # Start services in order
    start_backend_service "router" "8080" "$db_env $redis_env"
    start_backend_service "engine" "" "$db_env $redis_env"
    start_backend_service "ws-stream" "4000" "$ws_env $redis_env"
    start_backend_service "db-processor" "" "$db_env $redis_env"

    log_success "Backend services started"
}

# Setup frontend
setup_frontend() {
    log_info "Setting up frontend..."

    cd "$WEB_DIR"

    if [[ ! -d node_modules ]] || [[ "$RESET_DB" == "true" ]]; then
        verbose_log "Installing frontend dependencies..."
        yarn install
    fi

    log_success "Frontend setup completed"
}

# Start frontend
start_frontend() {
    log_info "Starting frontend development server..."

    cd "$WEB_DIR"

    if [[ "$BACKGROUND" == "true" ]]; then
        verbose_log "Starting frontend in background..."
        nohup yarn dev > "$PID_DIR/frontend.log" 2>&1 &
        echo $! > "$PID_DIR/frontend.pid"

        wait_for_service localhost 5173 "Frontend" 20
    else
        log_info "Starting frontend (press Ctrl+C to stop all services)..."
        echo "Starting frontend..." > "$PID_DIR/frontend.log"
        yarn dev &
        echo $! > "$PID_DIR/frontend.pid"

        wait_for_service localhost 5173 "Frontend" 20
    fi

    log_success "Frontend started"
}

# Cleanup function for graceful shutdown
cleanup() {
    log_info "Shutting down services..."
    "$SCRIPT_DIR/stop-exchange.sh" --quiet
    exit 0
}

# Show status
show_status() {
    echo ""
    log_success "🚀 Testudo Exchange is now running!"
    echo ""
    echo "📊 Service Status:"
    echo "  • PostgreSQL:        localhost:5000"
    echo "  • Redis:             localhost:6380"
    echo "  • API Gateway:       http://localhost:8080"
    echo "  • WebSocket:         ws://localhost:4000"
    echo "  • Trading Interface: http://localhost:5173"
    echo ""
    echo "🛠️  Management:"
    echo "  • Stop services:     $SCRIPT_DIR/stop-exchange.sh"
    echo "  • Check status:      $SCRIPT_DIR/status-exchange.sh"
    echo "  • View logs:         $SCRIPT_DIR/logs-exchange.sh"
    echo ""

    if [[ "$BACKGROUND" == "true" ]]; then
        echo "ℹ️  Services are running in background"
        echo "   Use '$SCRIPT_DIR/logs-exchange.sh' to view logs"
    else
        echo "ℹ️  Press Ctrl+C to stop all services"
    fi
    echo ""
}

# Main function
main() {
    # Create PID directory
    mkdir -p "$PID_DIR"

    # Clear old PID files
    rm -f "$PID_DIR"/*.pid

    echo "🔄 Starting Testudo Exchange..."
    echo ""

    # Set up signal handlers for graceful shutdown
    if [[ "$BACKGROUND" != "true" ]]; then
        trap cleanup SIGINT SIGTERM
    fi

    # Run all setup steps
    check_prerequisites
    check_ports
    start_infrastructure
    setup_database
    build_backend
    start_backend
    setup_frontend
    start_frontend

    show_status

    # If not running in background, wait for user input
    if [[ "$BACKGROUND" != "true" ]]; then
        wait
    fi
}

# Run main function
main "$@"