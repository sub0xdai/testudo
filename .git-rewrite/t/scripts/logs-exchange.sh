#!/bin/bash

# Testudo Exchange - Logs Script
# View and manage logs from all exchange services

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
EXCHANGE_DIR="$PROJECT_ROOT/testudo-exchange"
PID_DIR="$SCRIPT_DIR/.pids"

# Default options
FOLLOW=false
SERVICE=""
LINES=50
SINCE=""
SHOW_ALL=false

# Available services
SERVICES=("router" "engine" "ws-stream" "db-processor" "frontend")

# Usage function
usage() {
    echo "Usage: $0 [OPTIONS] [SERVICE]"
    echo "View logs from Testudo Exchange services"
    echo ""
    echo "Services:"
    echo "  router        API Gateway service"
    echo "  engine        Matching engine service"
    echo "  ws-stream     WebSocket service"
    echo "  db-processor  Database processor service"
    echo "  frontend      Frontend development server"
    echo "  docker        Docker infrastructure logs"
    echo "  all           All services (default)"
    echo ""
    echo "Options:"
    echo "  --follow, -f          Follow logs in real-time"
    echo "  --lines, -n LINES     Number of lines to show (default: 50)"
    echo "  --since, -s TIME      Show logs since timestamp (e.g., '1h ago', '2023-01-01')"
    echo "  --all, -a             Show all available logs"
    echo "  --help, -h            Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    Show recent logs from all services"
    echo "  $0 router             Show logs from router service only"
    echo "  $0 -f                 Follow logs from all services"
    echo "  $0 -f router          Follow logs from router service"
    echo "  $0 --lines 100        Show last 100 lines from all services"
    echo "  $0 --since '1h ago'   Show logs from last hour"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --follow|-f)
            FOLLOW=true
            shift
            ;;
        --lines|-n)
            LINES="$2"
            shift 2
            ;;
        --since|-s)
            SINCE="$2"
            shift 2
            ;;
        --all|-a)
            SHOW_ALL=true
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        router|engine|ws-stream|db-processor|frontend|docker|all)
            SERVICE="$1"
            shift
            ;;
        *)
            echo "Unknown option or service: $1"
            usage
            exit 1
            ;;
    esac
done

# Set default service if none specified
if [[ -z "$SERVICE" ]]; then
    SERVICE="all"
fi

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Get color for service
get_service_color() {
    local service=$1
    case $service in
        "router")
            echo "${GREEN}"
            ;;
        "engine")
            echo "${BLUE}"
            ;;
        "ws-stream")
            echo "${CYAN}"
            ;;
        "db-processor")
            echo "${YELLOW}"
            ;;
        "frontend")
            echo "${MAGENTA}"
            ;;
        "docker")
            echo "${RED}"
            ;;
        *)
            echo "${GRAY}"
            ;;
    esac
}

# Format log line with service prefix
format_log_line() {
    local service=$1
    local line="$2"
    local color
    color=$(get_service_color "$service")
    local timestamp
    timestamp=$(date '+%H:%M:%S')

    echo -e "${color}[${service}]${NC} ${GRAY}${timestamp}${NC} $line"
}

# Check if service log file exists
check_log_file() {
    local service=$1
    local log_file="$PID_DIR/$service.log"

    if [[ ! -f "$log_file" ]]; then
        return 1
    fi
    return 0
}

# Show service log
show_service_log() {
    local service=$1
    local log_file="$PID_DIR/$service.log"

    if ! check_log_file "$service"; then
        log_error "No log file found for $service"
        log_info "Service may not be running or may not have been started with the start script"
        return 1
    fi

    local tail_cmd="tail"

    if [[ "$FOLLOW" == "true" ]]; then
        tail_cmd="tail -f"
    fi

    if [[ -n "$LINES" ]]; then
        tail_cmd="$tail_cmd -n $LINES"
    fi

    log_info "Showing logs for $service ($log_file)"
    echo ""

    if [[ "$FOLLOW" == "true" ]]; then
        $tail_cmd "$log_file" | while IFS= read -r line; do
            format_log_line "$service" "$line"
        done
    else
        $tail_cmd "$log_file" | while IFS= read -r line; do
            format_log_line "$service" "$line"
        done
    fi
}

# Show Docker logs
show_docker_logs() {
    cd "$EXCHANGE_DIR/docker" 2>/dev/null || {
        log_error "Cannot access Docker directory"
        return 1
    }

    local docker_cmd="docker compose logs"

    if [[ "$FOLLOW" == "true" ]]; then
        docker_cmd="$docker_cmd --follow"
    fi

    if [[ -n "$LINES" ]]; then
        docker_cmd="$docker_cmd --tail $LINES"
    fi

    if [[ -n "$SINCE" ]]; then
        docker_cmd="$docker_cmd --since $SINCE"
    fi

    log_info "Showing Docker infrastructure logs"
    echo ""

    $docker_cmd | while IFS= read -r line; do
        format_log_line "docker" "$line"
    done
}

# Show logs from multiple services
show_all_logs() {
    local available_services=()

    # Check which services have log files
    for service in "${SERVICES[@]}"; do
        if check_log_file "$service"; then
            available_services+=("$service")
        fi
    done

    if [[ ${#available_services[@]} -eq 0 ]]; then
        log_error "No service log files found"
        log_info "Services may not be running or may not have been started with the start script"
        return 1
    fi

    log_info "Available services: ${available_services[*]}"
    echo ""

    if [[ "$FOLLOW" == "true" ]]; then
        # For follow mode, we need to tail all files simultaneously
        local tail_files=()
        for service in "${available_services[@]}"; do
            tail_files+=("$PID_DIR/$service.log")
        done

        if [[ ${#tail_files[@]} -gt 0 ]]; then
            tail -f "${tail_files[@]}" | while IFS= read -r line; do
                # Try to determine which service the line came from
                local source_service="unknown"
                for service in "${available_services[@]}"; do
                    if [[ "$line" == *"$PID_DIR/$service.log"* ]]; then
                        source_service="$service"
                        # Remove the file path prefix from tail -f output
                        line=$(echo "$line" | sed "s|==> $PID_DIR/$service.log <==||" | sed 's/^[[:space:]]*//')
                        if [[ -n "$line" ]]; then
                            format_log_line "$service" "$line"
                        fi
                        break
                    fi
                done

                # If we couldn't determine the service, just show the line
                if [[ "$source_service" == "unknown" && -n "$line" ]]; then
                    echo "$line"
                fi
            done
        fi
    else
        # For non-follow mode, show logs from each service
        for service in "${available_services[@]}"; do
            echo -e "${CYAN}=== ${service} logs ===${NC}"
            show_service_log "$service"
            echo ""
        done
    fi
}

# Show summary of available logs
show_log_summary() {
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}                        ${BLUE}Testudo Exchange Logs${NC}                          ${CYAN}║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    echo -e "${BLUE}📋 Available Logs:${NC}"
    echo ""

    # Check service logs
    local service_logs_available=false
    for service in "${SERVICES[@]}"; do
        local log_file="$PID_DIR/$service.log"
        local status="${RED}✗ Not available${NC}"
        local size=""
        local last_modified=""

        if [[ -f "$log_file" ]]; then
            service_logs_available=true
            status="${GREEN}✓ Available${NC}"

            # Get file size
            if command -v du >/dev/null 2>&1; then
                size=$(du -h "$log_file" 2>/dev/null | cut -f1 || echo "unknown")
            fi

            # Get last modified time
            if command -v stat >/dev/null 2>&1; then
                last_modified=$(stat -c %y "$log_file" 2>/dev/null | cut -d. -f1 || echo "unknown")
            fi
        fi

        printf "  %-12s: %s" "$service" "$status"
        if [[ -n "$size" && -n "$last_modified" ]]; then
            printf " (${size}, modified: ${last_modified})"
        fi
        echo ""
    done

    # Check Docker logs
    echo ""
    cd "$EXCHANGE_DIR/docker" 2>/dev/null && {
        if docker compose ps -q | grep -q .; then
            echo -e "  docker      : ${GREEN}✓ Available${NC} (Docker infrastructure)"
        else
            echo -e "  docker      : ${RED}✗ No containers running${NC}"
        fi
    } || {
        echo -e "  docker      : ${RED}✗ Docker directory not accessible${NC}"
    }

    echo ""

    if [[ "$service_logs_available" == "true" ]]; then
        echo -e "${BLUE}📖 Usage Examples:${NC}"
        echo "  View all logs:           $0"
        echo "  Follow all logs:         $0 --follow"
        echo "  View specific service:   $0 router"
        echo "  Follow specific service: $0 --follow engine"
        echo "  Last 100 lines:         $0 --lines 100"
        echo ""
    else
        echo -e "${YELLOW}⚠️  No service logs found.${NC}"
        echo "   Services must be started with the start script to generate logs."
        echo "   Use: $SCRIPT_DIR/start-exchange.sh --background"
        echo ""
    fi
}

# Main function
main() {
    # Create PID directory if it doesn't exist
    mkdir -p "$PID_DIR"

    if [[ "$SHOW_ALL" == "true" ]]; then
        show_log_summary
        return 0
    fi

    case $SERVICE in
        "all")
            if [[ "$FOLLOW" == "true" ]]; then
                log_info "Following logs from all services (press Ctrl+C to exit)"
                echo ""
            fi
            show_all_logs
            ;;
        "docker")
            show_docker_logs
            ;;
        *)
            if [[ ! " ${SERVICES[*]} " =~ " ${SERVICE} " ]]; then
                log_error "Unknown service: $SERVICE"
                log_info "Available services: ${SERVICES[*]} docker all"
                exit 1
            fi

            if [[ "$FOLLOW" == "true" ]]; then
                log_info "Following logs from $SERVICE (press Ctrl+C to exit)"
                echo ""
            fi
            show_service_log "$SERVICE"
            ;;
    esac
}

# Handle Ctrl+C gracefully
trap 'echo -e "\n${BLUE}[INFO]${NC} Stopping log viewing..."; exit 0' SIGINT

# Run main function
main "$@"