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
# Frontend: lint + Playwright E2E tests (use npx for Playwright compatibility)
CHECK_CMD_FRONTEND="cd testudo-web/apps/web && bun run lint && npx playwright test"
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

    # Initialize feedback variables for the loop
    local VERIFY_OUTPUT=""
    local VERIFY_EXIT_CODE=0
    local FEEDBACK=""

    while [[ $iteration -lt $MAX_ITERATIONS ]]; do
        iteration=$((iteration + 1))
        echo ""
        echo "--- Iteration $iteration of $MAX_ITERATIONS ---"

        # 1. Capture verification output from previous iteration (or initial state)
        #    On first iteration, we run verification to establish baseline
        echo "Running verification checks..."
        # Capture both output and exit code (without triggering set -e)
        if VERIFY_OUTPUT=$(eval "$CHECK_CMD_BACKEND && $TEST_CMD_BACKEND" 2>&1); then
            VERIFY_EXIT_CODE=0
        else
            VERIFY_EXIT_CODE=$?
        fi

        # 2. Determine feedback based on verification results
        if [[ $VERIFY_EXIT_CODE -eq 0 ]]; then
            echo "✓ Verification passed!"
            FEEDBACK="Verification PASSED. Please perform final cleanup and output <promise>DONE</promise> if all requirements are met."
        else
            echo "✗ Verification failed (exit code: $VERIFY_EXIT_CODE)"
            FEEDBACK="Verification FAILED. Fix the errors shown below."
        fi

        # 3. Construct the prompt with feedback
        PROMPT="
ROLE: You are Ralph Wiggum, an autonomous developer implementing specifications.

CONTEXT:
- Read the constitution: $CONSTITUTION
- Read the specification: $SPECS_DIR/$spec/spec.md

STATUS:
- Iteration: $iteration of $MAX_ITERATIONS
- Previous Verification Exit Code: $VERIFY_EXIT_CODE
- $FEEDBACK

VERIFICATION OUTPUT:
----------------------------------------
$VERIFY_OUTPUT
----------------------------------------

INSTRUCTIONS:
1. ANALYZE: Understand all requirements in the spec
2. IMPLEMENT: Write code to satisfy all functional requirements
3. FIX: If verification failed above, fix the reported errors
4. VERIFY BACKEND: Run '$CHECK_CMD_BACKEND && $TEST_CMD_BACKEND'
5. VERIFY FRONTEND: Run '$CHECK_CMD_FRONTEND && $BUILD_CMD_FRONTEND'
6. COMMIT: Run 'git add . && git commit -m \"feat: implement $spec\"'
7. COMPLETE: When ALL verifications pass, output <promise>DONE</promise>
"

        # 4. Execute agent in non-interactive mode
        # -p forces print mode (run once and exit, no chat window)
        # --dangerously-skip-permissions prevents "Can I edit this file?" prompts
        echo "🤖 Waking up Ralph..."

        if [[ "$AGENT_CMD" == "claude" ]]; then
            $AGENT_CMD -p "$PROMPT" --dangerously-skip-permissions
        elif [[ "$HEADLESS" == true && "$USE_CODEX" == true ]]; then
            $AGENT_CMD --dangerously-bypass-approvals-and-sandbox "$PROMPT"
        else
            # Fallback for other agents
            $AGENT_CMD "$PROMPT"
        fi

        # Git safety check
        if [[ -n $(git status --porcelain) ]]; then
            echo "WARNING: Uncommitted changes detected after iteration"
            git status --short
        fi

        echo ""
        echo "Iteration $iteration complete. Checking for DONE signal..."

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

        total_iterations=$((total_iterations + MAX_ITERATIONS))
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
