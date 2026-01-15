#!/bin/bash
#
# Ralph Loop - Autonomous AI Implementation Runner
# Part of the Ralph Wiggum Framework
#
# Usage:
#   ./scripts/ralph-loop.sh <spec-name>        # Single spec
#   ./scripts/ralph-loop.sh --all              # All specs
#   ./scripts/ralph-loop.sh --all --headless   # Non-interactive batch
#

set -e

# --- CONFIGURATION ---
AGENT_CMD="claude"
MAX_ITERATIONS=30
MAX_TOTAL_ITERATIONS=100
SPECS_DIR=".specify/specs"
CONSTITUTION=".specify/memory/constitution.md"

# Verification commands
CHECK_CMD_BACKEND="cd testudo-exchange && cargo clippy --all-targets"
TEST_CMD_BACKEND="cd testudo-exchange && cargo test"
CHECK_CMD_FRONTEND="cd testudo-web/apps/web && bun run lint"
BUILD_CMD_FRONTEND="cd testudo-web/apps/web && bun run build"

# --- ARGUMENT PARSING ---
SPEC_NAME=""
RUN_ALL=false
HEADLESS=false
USE_CODEX=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            RUN_ALL=true
            shift
            ;;
        --headless)
            HEADLESS=true
            shift
            ;;
        --codex)
            USE_CODEX=true
            AGENT_CMD="codex"
            shift
            ;;
        --claude)
            AGENT_CMD="claude"
            shift
            ;;
        --max-iterations)
            MAX_ITERATIONS="$2"
            shift 2
            ;;
        *)
            SPEC_NAME="$1"
            shift
            ;;
    esac
done

# --- HELPER FUNCTIONS ---

list_specs() {
    if [[ -d "$SPECS_DIR" ]]; then
        ls -1 "$SPECS_DIR" 2>/dev/null | sort
    fi
}

spec_exists() {
    [[ -f "$SPECS_DIR/$1/spec.md" ]]
}

run_single_spec() {
    local spec="$1"
    local iteration=0

    echo "=========================================="
    echo "Ralph Wiggum: Implementing $spec"
    echo "=========================================="

    if ! spec_exists "$spec"; then
        echo "ERROR: Spec not found: $SPECS_DIR/$spec/spec.md"
        echo "Available specs:"
        list_specs
        return 1
    fi

    while [[ $iteration -lt $MAX_ITERATIONS ]]; do
        ((iteration++))
        echo ""
        echo "--- Iteration $iteration of $MAX_ITERATIONS ---"

        # Construct the prompt
        PROMPT="
ROLE: You are Ralph Wiggum, an autonomous developer implementing specifications.

CONTEXT:
- Read the constitution: $CONSTITUTION
- Read the specification: $SPECS_DIR/$spec/spec.md

INSTRUCTIONS:
1. ANALYZE: Understand all requirements in the spec
2. IMPLEMENT: Write code to satisfy all functional requirements
3. VERIFY BACKEND: Run '$CHECK_CMD_BACKEND && $TEST_CMD_BACKEND'
4. VERIFY FRONTEND: Run '$CHECK_CMD_FRONTEND && $BUILD_CMD_FRONTEND'
5. FIX: If any verification fails, fix the issue and re-verify
6. COMMIT: Run 'git add . && git commit -m \"feat: implement $spec\"'
7. COMPLETE: When ALL verifications pass, output <promise>DONE</promise>

ITERATION: $iteration of $MAX_ITERATIONS
"

        # Execute agent
        if [[ "$HEADLESS" == true && "$USE_CODEX" == true ]]; then
            $AGENT_CMD --dangerously-bypass-approvals-and-sandbox "$PROMPT"
        else
            $AGENT_CMD "$PROMPT"
        fi

        # Check for completion signal in recent output
        # (Agent should have output DONE if complete)

        # Git safety check
        if [[ -n $(git status --porcelain) ]]; then
            echo "WARNING: Uncommitted changes detected after iteration"
            git status --short
        fi

        echo ""
        echo "Iteration $iteration complete. Checking for DONE signal..."

        # The agent should output DONE when complete
        # In practice, we rely on the agent to manage its own completion

    done

    echo "WARNING: Reached max iterations ($MAX_ITERATIONS) for $spec"
    return 1
}

run_all_specs() {
    local total_iterations=0
    local specs=($(list_specs))

    echo "=========================================="
    echo "Ralph Wiggum: Processing ALL specs"
    echo "Found ${#specs[@]} specs"
    echo "=========================================="

    for spec in "${specs[@]}"; do
        echo ""
        echo ">>> Starting: $spec"

        if run_single_spec "$spec"; then
            echo "<<< Completed: $spec"
        else
            echo "<<< FAILED: $spec"
            return 1
        fi

        ((total_iterations += MAX_ITERATIONS))
        if [[ $total_iterations -ge $MAX_TOTAL_ITERATIONS ]]; then
            echo "WARNING: Reached total iteration limit ($MAX_TOTAL_ITERATIONS)"
            return 1
        fi
    done

    echo ""
    echo "=========================================="
    echo "<promise>ALL_DONE</promise>"
    echo "=========================================="
}

# --- MAIN ---

echo "Ralph Wiggum - Autonomous Implementation Framework"
echo "Agent: $AGENT_CMD"
echo ""

if [[ "$RUN_ALL" == true ]]; then
    run_all_specs
elif [[ -n "$SPEC_NAME" ]]; then
    run_single_spec "$SPEC_NAME"
else
    echo "Usage:"
    echo "  $0 <spec-name>     Run single spec"
    echo "  $0 --all           Run all specs"
    echo ""
    echo "Options:"
    echo "  --headless         Non-interactive mode (codex only)"
    echo "  --codex            Use OpenAI Codex CLI"
    echo "  --claude           Use Claude Code (default)"
    echo "  --max-iterations N Set max iterations (default: 30)"
    echo ""
    echo "Available specs:"
    list_specs
fi
