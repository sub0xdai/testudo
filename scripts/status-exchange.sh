#!/bin/bash

# Testudo Exchange - Status Script
# Shows the status of all exchange services

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
EXCHANGE_DIR="$PROJECT_ROOT/testudo-exchange"
PID_DIR="$SCRIPT_DIR/.pids"

# Default options
WATCH=false
COMPACT=false

# Usage function
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Show status of all Testudo Exchange services"
    echo ""
    echo "Options:"
    echo "  --watch, -w         Continuously watch status (refresh every 2s)"
    echo "  --compact, -c       Compact output format"
    echo "  --help, -h          Show this help message"
    echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --watch|-w)
            WATCH=true
            shift
            ;;
        --compact|-c)
            COMPACT=true
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

# Check if port is in use
check_port() {
    local port=$1
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
        return 0  # Port is in use
    else
        return 1  # Port is free
    fi
}

# Get process info for port
get_port_process() {
    local port=$1
    local pid
    pid=$(lsof -Pi :$port -sTCP:LISTEN -t 2>/dev/null | head -1)

    if [[ -n "$pid" ]]; then
        local cmd
        cmd=$(ps -p "$pid" -o comm= 2>/dev/null || echo "unknown")
        echo "$pid ($cmd)"
    else
        echo "-"
    fi
}

# Check if process is running
is_process_running() {
    local pid=$1
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
        return 0
    else
        return 1
    fi
}

# Get service status
get_service_status() {
    local service_name=$1
    local port=${2:-""}
    local pid_file="$PID_DIR/$service_name.pid"

    local status="stopped"
    local pid="-"
    local uptime="-"
    local port_status="-"

    # Check PID file
    if [[ -f "$pid_file" ]]; then
        local file_pid
        file_pid=$(cat "$pid_file" 2>/dev/null || echo "")

        if is_process_running "$file_pid"; then
            status="running"
            pid="$file_pid"

            # Calculate uptime
            if [[ -n "$file_pid" ]]; then
                local start_time
                start_time=$(ps -o lstart= -p "$file_pid" 2>/dev/null | xargs -I {} date -d "{}" +%s 2>/dev/null || echo "")
                if [[ -n "$start_time" ]]; then
                    local current_time
                    current_time=$(date +%s)
                    local uptime_seconds=$((current_time - start_time))

                    if [[ $uptime_seconds -lt 60 ]]; then
                        uptime="${uptime_seconds}s"
                    elif [[ $uptime_seconds -lt 3600 ]]; then
                        uptime="$((uptime_seconds / 60))m"
                    else
                        uptime="$((uptime_seconds / 3600))h $((uptime_seconds % 3600 / 60))m"
                    fi
                fi
            fi
        else
            status="dead"
        fi
    fi

    # Check port if specified
    if [[ -n "$port" ]]; then
        if check_port "$port"; then
            port_status="listening"

            # If we don't have a running status from PID but port is active, mark as running
            if [[ "$status" == "stopped" ]]; then
                status="running"
                pid=$(get_port_process "$port")
            fi
        else
            port_status="closed"

            # If PID says running but port is closed, there might be an issue
            if [[ "$status" == "running" ]]; then
                status="error"
            fi
        fi
    fi

    echo "$status|$pid|$uptime|$port_status"
}

# Get Docker service status
get_docker_status() {
    cd "$EXCHANGE_DIR/docker" 2>/dev/null || return 1

    local postgres_status="stopped"
    local redis_status="stopped"

    if docker compose ps -q db 2>/dev/null | grep -q .; then
        local container_status
        container_status=$(docker compose ps db --format "table {{.State}}" 2>/dev/null | tail -n +2 | tr -d ' ')
        if [[ "$container_status" == "running" ]]; then
            postgres_status="running"
        else
            postgres_status="error"
        fi
    fi

    if docker compose ps -q redis 2>/dev/null | grep -q .; then
        local container_status
        container_status=$(docker compose ps redis --format "table {{.State}}" 2>/dev/null | tail -n +2 | tr -d ' ')
        if [[ "$container_status" == "running" ]]; then
            redis_status="running"
        else
            redis_status="error"
        fi
    fi

    echo "$postgres_status|$redis_status"
}

# Format status for display
format_status() {
    local status=$1
    case $status in
        "running")
            echo -e "${GREEN}●${NC} running"
            ;;
        "stopped")
            echo -e "${GRAY}●${NC} stopped"
            ;;
        "dead")
            echo -e "${RED}●${NC} dead"
            ;;
        "error")
            echo -e "${YELLOW}●${NC} error"
            ;;
        *)
            echo -e "${GRAY}●${NC} unknown"
            ;;
    esac
}

# Format port status
format_port_status() {
    local status=$1
    case $status in
        "listening")
            echo -e "${GREEN}open${NC}"
            ;;
        "closed")
            echo -e "${RED}closed${NC}"
            ;;
        "-")
            echo -e "${GRAY}n/a${NC}"
            ;;
        *)
            echo -e "${GRAY}unknown${NC}"
            ;;
    esac
}

# Show compact status
show_compact_status() {
    # Get all service statuses
    local router_status
    router_status=$(get_service_status "router" "8080")
    local engine_status
    engine_status=$(get_service_status "engine")
    local ws_status
    ws_status=$(get_service_status "ws-stream" "4000")
    local db_proc_status
    db_proc_status=$(get_service_status "db-processor")
    local frontend_status
    frontend_status=$(get_service_status "frontend" "5173")
    local docker_status
    docker_status=$(get_docker_status)

    # Parse statuses
    IFS='|' read -r router_st router_pid router_up router_port <<< "$router_status"
    IFS='|' read -r engine_st engine_pid engine_up engine_port <<< "$engine_status"
    IFS='|' read -r ws_st ws_pid ws_up ws_port <<< "$ws_status"
    IFS='|' read -r db_st db_pid db_up db_port <<< "$db_proc_status"
    IFS='|' read -r fe_st fe_pid fe_up fe_port <<< "$frontend_status"
    IFS='|' read -r pg_st redis_st <<< "$docker_status"

    # Count running services
    local running_count=0
    for status in "$router_st" "$engine_st" "$ws_st" "$db_st" "$fe_st" "$pg_st" "$redis_st"; do
        if [[ "$status" == "running" ]]; then
            ((running_count++))
        fi
    done

    echo "Exchange Status: $running_count/7 services running"

    printf "%-12s %-12s %-12s %-12s %-12s %-12s %-12s\n" \
        "$(format_status "$router_st")" \
        "$(format_status "$engine_st")" \
        "$(format_status "$ws_st")" \
        "$(format_status "$db_st")" \
        "$(format_status "$fe_st")" \
        "$(format_status "$pg_st")" \
        "$(format_status "$redis_st")"

    printf "%-12s %-12s %-12s %-12s %-12s %-12s %-12s\n" \
        "router" "engine" "websocket" "db-proc" "frontend" "postgres" "redis"
}

# Show detailed status
show_detailed_status() {
    local current_time
    current_time=$(date '+%Y-%m-%d %H:%M:%S')

    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}                           ${BLUE}Testudo Exchange Status${NC}                              ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}                              $current_time                              ${CYAN}║${NC}"
    echo -e "${CYAN}╠════════════════════════════════════════════════════════════════════════════════╣${NC}"
    echo ""

    # Infrastructure Services
    echo -e "${BLUE}📦 Infrastructure Services${NC}"
    echo -e "${CYAN}┌─────────────────┬──────────┬──────────┬─────────────┬──────────────┐${NC}"
    echo -e "${CYAN}│${NC} Service         ${CYAN}│${NC} Status   ${CYAN}│${NC} Port     ${CYAN}│${NC} PID         ${CYAN}│${NC} Port Status  ${CYAN}│${NC}"
    echo -e "${CYAN}├─────────────────┼──────────┼──────────┼─────────────┼──────────────┤${NC}"

    # PostgreSQL
    local docker_status
    docker_status=$(get_docker_status)
    IFS='|' read -r pg_status redis_status <<< "$docker_status"

    local pg_port_process="-"
    local pg_port_status="closed"
    if check_port 5000; then
        pg_port_process=$(get_port_process 5000)
        pg_port_status="listening"
    fi

    printf "${CYAN}│${NC} %-15s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-11s ${CYAN}│${NC} %-12s ${CYAN}│${NC}\n" \
        "PostgreSQL" "$(format_status "$pg_status")" "5000" "$pg_port_process" "$(format_port_status "$pg_port_status")"

    # Redis
    local redis_port_process="-"
    local redis_port_status="closed"
    if check_port 6380; then
        redis_port_process=$(get_port_process 6380)
        redis_port_status="listening"
    fi

    printf "${CYAN}│${NC} %-15s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-11s ${CYAN}│${NC} %-12s ${CYAN}│${NC}\n" \
        "Redis" "$(format_status "$redis_status")" "6380" "$redis_port_process" "$(format_port_status "$redis_port_status")"

    echo -e "${CYAN}└─────────────────┴──────────┴──────────┴─────────────┴──────────────┘${NC}"
    echo ""

    # Backend Services
    echo -e "${BLUE}⚙️  Backend Services${NC}"
    echo -e "${CYAN}┌─────────────────┬──────────┬──────────┬─────────────┬──────────────┬──────────┐${NC}"
    echo -e "${CYAN}│${NC} Service         ${CYAN}│${NC} Status   ${CYAN}│${NC} Port     ${CYAN}│${NC} PID         ${CYAN}│${NC} Port Status  ${CYAN}│${NC} Uptime   ${CYAN}│${NC}"
    echo -e "${CYAN}├─────────────────┼──────────┼──────────┼─────────────┼──────────────┼──────────┤${NC}"

    local services=(
        "router|Router API|8080"
        "engine|Engine|"
        "ws-stream|WebSocket|4000"
        "db-processor|DB Processor|"
    )

    for service_info in "${services[@]}"; do
        IFS='|' read -r service_name display_name port <<< "$service_info"

        local status_info
        status_info=$(get_service_status "$service_name" "$port")
        IFS='|' read -r status pid uptime port_status <<< "$status_info"

        printf "${CYAN}│${NC} %-15s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-11s ${CYAN}│${NC} %-12s ${CYAN}│${NC} %-8s ${CYAN}│${NC}\n" \
            "$display_name" "$(format_status "$status")" "${port:-n/a}" "$pid" "$(format_port_status "$port_status")" "$uptime"
    done

    echo -e "${CYAN}└─────────────────┴──────────┴──────────┴─────────────┴──────────────┴──────────┘${NC}"
    echo ""

    # Frontend Services
    echo -e "${BLUE}🖥️  Frontend Services${NC}"
    echo -e "${CYAN}┌─────────────────┬──────────┬──────────┬─────────────┬──────────────┬──────────┐${NC}"
    echo -e "${CYAN}│${NC} Service         ${CYAN}│${NC} Status   ${CYAN}│${NC} Port     ${CYAN}│${NC} PID         ${CYAN}│${NC} Port Status  ${CYAN}│${NC} Uptime   ${CYAN}│${NC}"
    echo -e "${CYAN}├─────────────────┼──────────┼──────────┼─────────────┼──────────────┼──────────┤${NC}"

    local frontend_status
    frontend_status=$(get_service_status "frontend" "5173")
    IFS='|' read -r fe_status fe_pid fe_uptime fe_port_status <<< "$frontend_status"

    printf "${CYAN}│${NC} %-15s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-8s ${CYAN}│${NC} %-11s ${CYAN}│${NC} %-12s ${CYAN}│${NC} %-8s ${CYAN}│${NC}\n" \
        "Dev Server" "$(format_status "$fe_status")" "5173" "$fe_pid" "$(format_port_status "$fe_port_status")" "$fe_uptime"

    echo -e "${CYAN}└─────────────────┴──────────┴──────────┴─────────────┴──────────────┴──────────┘${NC}"
    echo ""

    # Quick Access URLs
    echo -e "${BLUE}🌐 Quick Access${NC}"
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────┐${NC}"

    local urls=(
        "Trading Interface|http://localhost:5173"
        "API Documentation|http://localhost:8080/api/v1"
        "WebSocket Endpoint|ws://localhost:4000"
    )

    for url_info in "${urls[@]}"; do
        IFS='|' read -r name url <<< "$url_info"
        printf "${CYAN}│${NC} %-20s: %-50s ${CYAN}│${NC}\n" "$name" "$url"
    done

    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────┘${NC}"
    echo ""

    # Management Commands
    echo -e "${BLUE}🛠️  Management Commands${NC}"
    echo "  Start services:  $SCRIPT_DIR/start-exchange.sh"
    echo "  Stop services:   $SCRIPT_DIR/stop-exchange.sh"
    echo "  View logs:       $SCRIPT_DIR/logs-exchange.sh"
    echo ""
}

# Watch mode
watch_status() {
    while true; do
        clear
        if [[ "$COMPACT" == "true" ]]; then
            show_compact_status
        else
            show_detailed_status
        fi
        echo -e "${GRAY}Press Ctrl+C to exit watch mode${NC}"
        sleep 2
    done
}

# Main function
main() {
    # Create PID directory if it doesn't exist
    mkdir -p "$PID_DIR"

    if [[ "$WATCH" == "true" ]]; then
        watch_status
    else
        if [[ "$COMPACT" == "true" ]]; then
            show_compact_status
        else
            show_detailed_status
        fi
    fi
}

# Handle Ctrl+C in watch mode
trap 'echo -e "\n${BLUE}[INFO]${NC} Exiting watch mode..."; exit 0' SIGINT

# Run main function
main "$@"