#!/bin/bash

# Testudo Exchange - Stop Script
# Gracefully stops all exchange services

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
PID_DIR="$SCRIPT_DIR/.pids"

# Default options
KEEP_INFRA=false
FORCE=false
QUIET=false

# Usage function
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Stop all Testudo Exchange services"
    echo ""
    echo "Options:"
    echo "  --keep-infra, -k    Keep infrastructure (PostgreSQL, Redis) running"
    echo "  --force, -f         Force shutdown (SIGKILL) if graceful shutdown fails"
    echo "  --quiet, -q         Suppress output messages"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Services stopped:"
    echo "  - Frontend Development Server"
    echo "  - Backend Services (router, engine, ws-stream, db-processor)"
    echo "  - Infrastructure (PostgreSQL, Redis) [unless --keep-infra]"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --keep-infra|-k)
            KEEP_INFRA=true
            shift
            ;;
        --force|-f)
            FORCE=true
            shift
            ;;
        --quiet|-q)
            QUIET=true
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
    if [[ "$QUIET" != "true" ]]; then
        echo -e "${BLUE}[INFO]${NC} $1"
    fi
}

log_success() {
    if [[ "$QUIET" != "true" ]]; then
        echo -e "${GREEN}[SUCCESS]${NC} $1"
    fi
}

log_warning() {
    if [[ "$QUIET" != "true" ]]; then
        echo -e "${YELLOW}[WARNING]${NC} $1"
    fi
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Check if process is running
is_process_running() {
    local pid=$1
    if kill -0 "$pid" 2>/dev/null; then
        return 0
    else
        return 1
    fi
}

# Stop process gracefully with timeout
stop_process() {
    local service_name=$1
    local pid_file="$PID_DIR/$service_name.pid"
    local timeout=${2:-10}

    if [[ ! -f "$pid_file" ]]; then
        log_warning "No PID file found for $service_name"
        return 0
    fi

    local pid
    pid=$(cat "$pid_file")

    if ! is_process_running "$pid"; then
        log_info "$service_name was not running"
        rm -f "$pid_file"
        return 0
    fi

    log_info "Stopping $service_name (PID: $pid)..."

    # Send SIGTERM for graceful shutdown
    kill -TERM "$pid" 2>/dev/null || {
        log_warning "Failed to send SIGTERM to $service_name"
        rm -f "$pid_file"
        return 1
    }

    # Wait for graceful shutdown
    for i in $(seq 1 $timeout); do
        if ! is_process_running "$pid"; then
            log_success "$service_name stopped gracefully"
            rm -f "$pid_file"
            return 0
        fi
        sleep 1
    done

    # Force kill if graceful shutdown failed
    if [[ "$FORCE" == "true" ]]; then
        log_warning "Forcefully killing $service_name (PID: $pid)"
        kill -KILL "$pid" 2>/dev/null || true
        sleep 1

        if ! is_process_running "$pid"; then
            log_success "$service_name force-stopped"
            rm -f "$pid_file"
            return 0
        else
            log_error "Failed to stop $service_name"
            return 1
        fi
    else
        log_error "$service_name did not stop gracefully within $timeout seconds"
        log_info "Use --force flag to force shutdown"
        return 1
    fi
}

# Stop all processes by name (fallback)
stop_processes_by_name() {
    local process_patterns=(
        "cargo run --bin router"
        "cargo run --bin engine"
        "cargo run --bin ws-stream"
        "cargo run --bin db-processor"
        "yarn dev"
        "vite"
        "turbo dev"
    )

    log_info "Searching for remaining exchange processes..."

    local found_processes=false
    for pattern in "${process_patterns[@]}"; do
        local pids
        pids=$(pgrep -f "$pattern" 2>/dev/null || true)

        if [[ -n "$pids" ]]; then
            found_processes=true
            log_warning "Found processes matching '$pattern': $pids"

            if [[ "$FORCE" == "true" ]]; then
                echo "$pids" | xargs -r kill -TERM 2>/dev/null || true
                sleep 2
                echo "$pids" | xargs -r kill -KILL 2>/dev/null || true
                log_info "Terminated processes matching '$pattern'"
            else
                log_info "Use --force flag to terminate these processes"
            fi
        fi
    done

    if [[ "$found_processes" == "false" ]]; then
        log_success "No remaining exchange processes found"
    fi
}

# Stop frontend service
stop_frontend() {
    log_info "Stopping frontend service..."
    stop_process "frontend" 10
}

# Stop backend services
stop_backend() {
    log_info "Stopping backend services..."

    local services=("db-processor" "ws-stream" "engine" "router")

    for service in "${services[@]}"; do
        stop_process "$service" 10
    done
}

# Stop infrastructure services
stop_infrastructure() {
    if [[ "$KEEP_INFRA" == "true" ]]; then
        log_info "Keeping infrastructure services running (--keep-infra flag)"
        return 0
    fi

    log_info "Stopping infrastructure services..."

    cd "$EXCHANGE_DIR/docker"

    if docker compose ps -q | grep -q .; then
        docker compose down
        log_success "Infrastructure services stopped"
    else
        log_info "Infrastructure services were not running"
    fi
}

# Clean up temporary files
cleanup_files() {
    log_info "Cleaning up temporary files..."

    # Remove log files older than 7 days
    find "$PID_DIR" -name "*.log" -mtime +7 -delete 2>/dev/null || true

    # Remove empty PID files
    find "$PID_DIR" -name "*.pid" -size 0 -delete 2>/dev/null || true

    log_success "Cleanup completed"
}

# Show final status
show_status() {
    if [[ "$QUIET" == "true" ]]; then
        return 0
    fi

    echo ""
    log_success "🛑 Testudo Exchange services stopped"
    echo ""

    # Check for any remaining processes
    local remaining_count=0

    # Check specific ports
    local ports=(5173 8080 4000)
    for port in "${ports[@]}"; do
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            log_warning "Port $port is still in use"
            ((remaining_count++))
        fi
    done

    # Check Docker containers
    if [[ "$KEEP_INFRA" == "false" ]]; then
        cd "$EXCHANGE_DIR/docker"
        if docker compose ps -q | grep -q .; then
            log_warning "Some Docker containers are still running"
            ((remaining_count++))
        fi
    fi

    if [[ $remaining_count -eq 0 ]]; then
        echo "✅ All services stopped cleanly"
    else
        echo "⚠️  Some services may still be running"
        echo "   Use --force flag for forceful shutdown"
    fi

    if [[ "$KEEP_INFRA" == "true" ]]; then
        echo ""
        echo "ℹ️  Infrastructure services kept running:"
        echo "   • PostgreSQL: localhost:5000"
        echo "   • Redis: localhost:6380"
    fi

    echo ""
}

# Main function
main() {
    if [[ "$QUIET" != "true" ]]; then
        echo "🔄 Stopping Testudo Exchange..."
        echo ""
    fi

    # Create PID directory if it doesn't exist
    mkdir -p "$PID_DIR"

    # Stop services in reverse order
    stop_frontend
    stop_backend
    stop_infrastructure

    # Clean up any remaining processes
    stop_processes_by_name

    # Clean up files
    cleanup_files

    show_status
}

# Handle script interruption
cleanup_on_exit() {
    log_info "Script interrupted, cleaning up..."
    exit 1
}

trap cleanup_on_exit SIGINT SIGTERM

# Run main function
main "$@"