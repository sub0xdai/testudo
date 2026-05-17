#!/bin/bash
#
# Vox Builder Loop - Autonomous AI Development
# "God spoke and it was."
#
# Usage:
#   ./scripts/vox.sh plan <spec-name>    # Planning mode (no code)
#   ./scripts/vox.sh build <spec-name>   # Building mode (one task per iteration)
#   ./scripts/vox.sh --help              # Show help
#

set -euo pipefail

# --- CONFIGURATION ---
SPECS_DIR=".specify/specs"
PROMPT_PLAN=".specify/PROMPT_plan.md"
PROMPT_BUILD=".specify/PROMPT_build.md"
IMPLEMENTATION_PLAN=".specify/IMPLEMENTATION_PLAN.md"
AGENTS_MD=".specify/AGENTS.md"
CONSTITUTION=".specify/memory/constitution.md"

MAX_ITERATIONS="${MAX_ITERATIONS:-15}"

# Split defaults by mode:
#   - plan is a ONE-SHOT call whose output gates every downstream build
#     iteration; a flawed plan can burn 15 iterations. Run on Opus.
#   - build is mechanical task execution against a fully-specified plan.
#     Run on Sonnet (~3x cheaper input, ~5x cheaper output).
#
# Overrides (in precedence order):
#   AGENT_MODEL_PLAN=<id>    # plan only
#   AGENT_MODEL_BUILD=<id>   # build only
#   AGENT_MODEL=<id>         # legacy — overrides BOTH (e.g. force all-Sonnet)
AGENT_MODEL_PLAN="${AGENT_MODEL_PLAN:-claude-opus-4-7}"
AGENT_MODEL_BUILD="${AGENT_MODEL_BUILD:-claude-sonnet-4-6}"
if [[ -n "${AGENT_MODEL:-}" ]]; then
    AGENT_MODEL_PLAN="$AGENT_MODEL"
    AGENT_MODEL_BUILD="$AGENT_MODEL"
fi

# Effort defaults:
#   - plan: Opus doesn't need the boost; leave unset so it runs at its default.
#   - build: Sonnet benefits from high effort — we're paying cheap input/output
#     rates, the extra thinking tokens lift code quality where it matters.
# Override: AGENT_EFFORT_BUILD=medium  (or empty string to disable)
AGENT_EFFORT_PLAN="${AGENT_EFFORT_PLAN:-}"
AGENT_EFFORT_BUILD="${AGENT_EFFORT_BUILD:-high}"

AGENT_CMD_PLAN="claude --model ${AGENT_MODEL_PLAN}"
[[ -n "$AGENT_EFFORT_PLAN" ]] && AGENT_CMD_PLAN="$AGENT_CMD_PLAN --effort ${AGENT_EFFORT_PLAN}"

AGENT_CMD_BUILD="claude --model ${AGENT_MODEL_BUILD}"
[[ -n "$AGENT_EFFORT_BUILD" ]] && AGENT_CMD_BUILD="$AGENT_CMD_BUILD --effort ${AGENT_EFFORT_BUILD}"

SLEEP_INTERVAL=15

# --- COLORS ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# --- HELPER FUNCTIONS ---

print_help() {
    echo "Vox Builder Loop - Autonomous AI Development"
    echo ""
    echo "Usage:"
    echo "  $0 plan <spec-name>     Run planning mode (gap analysis, no code)"
    echo "  $0 build <spec-name>    Run build mode (one task per iteration)"
    echo "  $0 --help               Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 plan 001-first-feature"
    echo "  $0 build 001-first-feature"
    echo ""
    echo "Options:"
    echo "  --max-iterations N      Set max iterations (default: 15; env MAX_ITERATIONS overrides)"
    echo ""
    echo "Env:"
    echo "  AGENT_MODEL_PLAN=<id>   Plan-mode model (default: claude-opus-4-7)"
    echo "  AGENT_MODEL_BUILD=<id>  Build-mode model (default: claude-sonnet-4-6)"
    echo "  AGENT_MODEL=<id>        Legacy — overrides BOTH plan and build"
    echo "  AGENT_EFFORT_PLAN=<lvl> Plan effort (low/medium/high/xhigh/max; default: unset)"
    echo "  AGENT_EFFORT_BUILD=<lvl> Build effort (default: high — lifts Sonnet code quality)"
    echo "  MAX_ITERATIONS=N        Same as --max-iterations"
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

    echo "# Context for Vox"
    echo ""
    echo "## Mode: ${mode^^}"
    echo "## Spec: $spec"
    echo ""

    # 1. Stable Instructions first (best for caching)
    echo "---"
    echo "## Instructions"
    echo ""
    if [[ "$mode" == "plan" ]]; then
        cat "$PROMPT_PLAN"
    else
        cat "$PROMPT_BUILD"
    fi
    echo ""

    # 2. Constitution (stable per-project)
    if [[ -f "$CONSTITUTION" ]]; then
        echo "---"
        echo "## Constitution"
        echo ""
        cat "$CONSTITUTION"
        echo ""
    fi

    # 3. Specification (stable per-spec)
    if [[ -f "$SPECS_DIR/$spec/spec.md" ]]; then
        echo "---"
        echo "## Specification: $spec"
        echo ""
        cat "$SPECS_DIR/$spec/spec.md"
        echo ""
    fi

    # 4. Cross-cutting operational rules (curated — stays small + stable)
    if [[ -f "$AGENTS_MD" ]]; then
        echo "---"
        echo "## Operational Rules (cross-cutting)"
        echo ""
        cat "$AGENTS_MD"
        echo ""
    fi

    # 4b. Per-spec learnings (append-only by build iterations; scoped to this
    # spec only — avoids cross-spec pollution that bloats context as the
    # global AGENTS.md used to).
    local spec_learnings="$SPECS_DIR/$spec/LEARNINGS.md"
    if [[ -f "$spec_learnings" ]]; then
        echo "---"
        echo "## Spec Learnings: $spec"
        echo ""
        cat "$spec_learnings"
        echo ""
    fi

    # 5. Implementation Plan (most volatile, last)
    if [[ -f "$IMPLEMENTATION_PLAN" ]]; then
        echo "---"
        echo "## Implementation Plan"
        echo ""
        cat "$IMPLEMENTATION_PLAN"
        echo ""
    fi
}

check_done_signal() {
    local output="$1"
    if echo "$output" | grep -q "<promise>DONE</promise>"; then
        return 0
    fi
    return 1
}

check_rate_limit_signal() {
    local output="$1"
    if echo "$output" | grep -qiE "you('ve| have) hit your limit|usage limit reached|rate limit"; then
        return 0
    fi
    return 1
}

extract_rate_limit_reset() {
    local output="$1"
    echo "$output" | grep -oE "resets [^[:space:]]+ \([^)]+\)" | head -1
}

run_planning() {
    local spec="$1"

    echo -e "${BLUE}=========================================="
    echo "Vox: PLANNING MODE"
    echo "Spec: $spec"
    echo -e "==========================================${NC}"

    if ! spec_exists "$spec"; then
        echo -e "${RED}ERROR: Spec not found: $SPECS_DIR/$spec/spec.md${NC}"
        list_specs
        return 1
    fi

    echo ""
    echo -e "${YELLOW}Vox speaks...${NC}"
    echo ""

    local output
    output=$(build_context "$spec" "plan" | $AGENT_CMD_PLAN -p --dangerously-skip-permissions 2>&1) || true

    echo "$output"

    echo ""
    echo -e "${GREEN}Planning complete. Review IMPLEMENTATION_PLAN.md${NC}"
    echo "Next: ./scripts/vox.sh build $spec"
}

run_building() {
    local spec="$1"
    local iteration=0

    echo -e "${BLUE}=========================================="
    echo "Vox: BUILD MODE"
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
        echo -e "${YELLOW}Vox speaks...${NC}"

        local output
        output=$(build_context "$spec" "build" | $AGENT_CMD_BUILD -p --dangerously-skip-permissions 2>&1) || true

        echo "$output"

        if check_done_signal "$output"; then
            echo ""
            echo -e "${GREEN}=========================================="
            echo "SPEC COMPLETE: $spec"
            echo -e "==========================================${NC}"
            return 0
        fi

        if check_rate_limit_signal "$output"; then
            local reset
            reset=$(extract_rate_limit_reset "$output")
            echo ""
            echo -e "${RED}=========================================="
            echo "RATE LIMIT HIT — halting build loop"
            if [[ -n "$reset" ]]; then
                echo "Limit ${reset}"
            fi
            echo "Completed $((iteration - 1)) of $MAX_ITERATIONS iterations before stall."
            echo "Resume after reset: $0 build $spec"
            echo -e "==========================================${NC}"
            return 2
        fi

        if [[ -n $(git status --porcelain 2>/dev/null || true) ]]; then
            echo ""
            echo -e "${YELLOW}Uncommitted changes after iteration:${NC}"
            git status --short
        fi

        echo ""
        echo -e "${BLUE}Iteration $iteration complete. Continuing...${NC}"

        sleep "$SLEEP_INTERVAL"

    done

    echo ""
    echo -e "${RED}WARNING: Reached max iterations ($MAX_ITERATIONS) for $spec${NC}"
    return 1
}

# --- MAIN ---

MODE=""
SPEC_NAME=""

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

if [[ ! -f "$PROMPT_PLAN" ]]; then
    echo -e "${RED}ERROR: Missing $PROMPT_PLAN${NC}"
    exit 1
fi

if [[ ! -f "$PROMPT_BUILD" ]]; then
    echo -e "${RED}ERROR: Missing $PROMPT_BUILD${NC}"
    exit 1
fi

echo "Vox Builder Loop"
echo "Plan agent:  $AGENT_CMD_PLAN"
echo "Build agent: $AGENT_CMD_BUILD"
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
