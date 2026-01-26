#!/bin/bash
#
# Ralph Loop - Autonomous AI Development Loop
# Based on the Ralph Wiggum Technique: https://github.com/ghuntley/how-to-ralph-wiggum
#
# Usage:
#   ./scripts/ralph-loop.sh plan <spec-name>    # Planning mode (no code)
#   ./scripts/ralph-loop.sh build <spec-name>   # Building mode (one task per iteration)
#   ./scripts/ralph-loop.sh --help              # Show help
#

set -euo pipefail

# --- CONFIGURATION ---
SPECS_DIR=".specify/specs"
MEMORY_DIR=".specify/memory"
PROMPT_PLAN=".specify/PROMPT_plan.md"
PROMPT_BUILD=".specify/PROMPT_build.md"
IMPLEMENTATION_PLAN=".specify/IMPLEMENTATION_PLAN.md"
AGENTS_MD=".specify/AGENTS.md"
CONSTITUTION=".specify/memory/constitution.md"

MAX_ITERATIONS=30
AGENT_CMD="claude"

# --- COLORS ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# --- HELPER FUNCTIONS ---

print_help() {
    echo "Ralph Loop - Autonomous AI Development"
    echo ""
    echo "Usage:"
    echo "  $0 plan <spec-name>     Run planning mode (gap analysis, no code)"
    echo "  $0 build <spec-name>    Run build mode (one task per iteration)"
    echo "  $0 --help               Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 plan 006-execution-latency"
    echo "  $0 build 006-execution-latency"
    echo ""
    echo "Options:"
    echo "  --max-iterations N      Set max iterations (default: 30)"
    echo ""
    echo "Available specs:"
    list_specs
}

list_specs() {
    if [[ -d "$SPECS_DIR" ]]; then
        ls -1 "$SPECS_DIR" 2>/dev/null | sort
    fi
}

spec_exists() {
    [[ -f "$SPECS_DIR/$1/spec.md" ]]
}

build_context() {
    local spec="$1"
    local mode="$2"

    echo "# Context for Ralph"
    echo ""
    echo "## Mode: ${mode^^}"
    echo "## Spec: $spec"
    echo ""

    # Include constitution
    if [[ -f "$CONSTITUTION" ]]; then
        echo "---"
        echo "## Constitution"
        echo ""
        cat "$CONSTITUTION"
        echo ""
    fi

    # Include spec
    if [[ -f "$SPECS_DIR/$spec/spec.md" ]]; then
        echo "---"
        echo "## Specification: $spec"
        echo ""
        cat "$SPECS_DIR/$spec/spec.md"
        echo ""
    fi

    # Include operational learnings
    if [[ -f "$AGENTS_MD" ]]; then
        echo "---"
        echo "## Operational Learnings"
        echo ""
        cat "$AGENTS_MD"
        echo ""
    fi

    # Include implementation plan
    if [[ -f "$IMPLEMENTATION_PLAN" ]]; then
        echo "---"
        echo "## Implementation Plan"
        echo ""
        cat "$IMPLEMENTATION_PLAN"
        echo ""
    fi

    # Include mode-specific prompt
    echo "---"
    echo "## Instructions"
    echo ""
    if [[ "$mode" == "plan" ]]; then
        cat "$PROMPT_PLAN"
    else
        cat "$PROMPT_BUILD"
    fi
}

check_done_signal() {
    local output="$1"
    if echo "$output" | grep -q "<promise>DONE</promise>"; then
        return 0
    fi
    return 1
}

run_planning() {
    local spec="$1"

    echo -e "${BLUE}=========================================="
    echo "Ralph Wiggum: PLANNING MODE"
    echo "Spec: $spec"
    echo -e "==========================================${NC}"

    if ! spec_exists "$spec"; then
        echo -e "${RED}ERROR: Spec not found: $SPECS_DIR/$spec/spec.md${NC}"
        list_specs
        return 1
    fi

    echo ""
    echo -e "${YELLOW}🤖 Waking up Ralph for planning...${NC}"
    echo ""

    # Build context and pipe to claude
    local output
    output=$(build_context "$spec" "plan" | $AGENT_CMD -p --dangerously-skip-permissions 2>&1) || true

    echo "$output"

    echo ""
    echo -e "${GREEN}Planning complete. Review IMPLEMENTATION_PLAN.md${NC}"
    echo "Next: ./scripts/ralph-loop.sh build $spec"
}

run_building() {
    local spec="$1"
    local iteration=0

    echo -e "${BLUE}=========================================="
    echo "Ralph Wiggum: BUILD MODE"
    echo "Spec: $spec"
    echo "Max iterations: $MAX_ITERATIONS"
    echo -e "==========================================${NC}"

    if ! spec_exists "$spec"; then
        echo -e "${RED}ERROR: Spec not found: $SPECS_DIR/$spec/spec.md${NC}"
        list_specs
        return 1
    fi

    while [[ $iteration -lt $MAX_ITERATIONS ]]; do
        iteration=$((iteration + 1))

        echo ""
        echo -e "${YELLOW}--- Iteration $iteration of $MAX_ITERATIONS ---${NC}"
        echo ""
        echo -e "${YELLOW}🤖 Waking up Ralph...${NC}"

        # Build context and pipe to claude
        local output
        output=$(build_context "$spec" "build" | $AGENT_CMD -p --dangerously-skip-permissions 2>&1) || true

        echo "$output"

        # Check for DONE signal
        if check_done_signal "$output"; then
            echo ""
            echo -e "${GREEN}=========================================="
            echo "🎉 SPEC COMPLETE: $spec"
            echo -e "==========================================${NC}"
            return 0
        fi

        # Git safety check
        if [[ -n $(git status --porcelain 2>/dev/null || true) ]]; then
            echo ""
            echo -e "${YELLOW}Uncommitted changes after iteration:${NC}"
            git status --short
        fi

        echo ""
        echo -e "${BLUE}Iteration $iteration complete. Continuing...${NC}"

        # Small delay to avoid rate limiting
        sleep 2

    done

    echo ""
    echo -e "${RED}WARNING: Reached max iterations ($MAX_ITERATIONS) for $spec${NC}"
    return 1
}

# --- MAIN ---

MODE=""
SPEC_NAME=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        plan|build)
            MODE="$1"
            shift
            ;;
        --max-iterations)
            MAX_ITERATIONS="$2"
            shift 2
            ;;
        --help|-h)
            print_help
            exit 0
            ;;
        *)
            if [[ -z "$SPEC_NAME" ]]; then
                SPEC_NAME="$1"
            fi
            shift
            ;;
    esac
done

# Validate arguments
if [[ -z "$MODE" ]]; then
    echo -e "${RED}ERROR: Mode required (plan or build)${NC}"
    echo ""
    print_help
    exit 1
fi

if [[ -z "$SPEC_NAME" ]]; then
    echo -e "${RED}ERROR: Spec name required${NC}"
    echo ""
    print_help
    exit 1
fi

# Check required files exist
if [[ ! -f "$PROMPT_PLAN" ]]; then
    echo -e "${RED}ERROR: Missing $PROMPT_PLAN${NC}"
    exit 1
fi

if [[ ! -f "$PROMPT_BUILD" ]]; then
    echo -e "${RED}ERROR: Missing $PROMPT_BUILD${NC}"
    exit 1
fi

# Run appropriate mode
echo "Ralph Loop - Autonomous AI Development"
echo "Agent: $AGENT_CMD"
echo ""

if [[ "$MODE" == "plan" ]]; then
    run_planning "$SPEC_NAME"
elif [[ "$MODE" == "build" ]]; then
    run_building "$SPEC_NAME"
else
    echo -e "${RED}ERROR: Unknown mode: $MODE${NC}"
    print_help
    exit 1
fi
