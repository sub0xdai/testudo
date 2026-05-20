#!/bin/bash
#
# Vox Builder Loop - Autonomous AI Development
# "God spoke and it was."
#
# Usage:
#   ./scripts/vox.sh plan <spec-name>    # Planning mode (gap analysis, no code)
#   ./scripts/vox.sh build <spec-name>   # Building mode (one task per iteration)
#   ./scripts/vox.sh --help              # Show help
#
# Two Vox modalities — pick the one that fits your workflow:
#
#   scripts/vox.sh        Autonomous. Fire and walk away. Loops until DONE or
#                          max iterations. Best when you trust the spec and
#                          want unattended execution.
#
#   /skill:vox (in pi)    Attended. One checkpoint per invocation, you stay in
#                          the pi session. Review each checkpoint. Intervene if
#                          needed. Best for complex specs or tight control.
#
# Agent detection (precedence):
#   1. --agent <name>             Explicit override
#   2. $AGENT_CMD                 Env override
#   3. Auto-detect: pi if available, else claude (fallback)

set -euo pipefail

# --- CONFIGURATION ---
SPECS_DIR=".specify/specs"
PROMPT_PLAN=".specify/PROMPT_plan.md"
PROMPT_BUILD=".specify/PROMPT_build.md"
IMPLEMENTATION_PLAN=".specify/IMPLEMENTATION_PLAN.md"
AGENTS_MD=".specify/AGENTS.md"
CONSTITUTION=".specify/memory/constitution.md"

MAX_ITERATIONS="${MAX_ITERATIONS:-15}"
SLEEP_INTERVAL=15

# --- AGENT DETECTION ---
# Order: --agent flag > $AGENT_CMD env > auto-detect

auto_detect_agent() {
    if command -v pi &>/dev/null && [[ -n "${PI_CODING_AGENT_DIR:-}" ]]; then
        echo "pi"
    elif command -v claude &>/dev/null; then
        echo "claude"
    else
        echo ""
    fi
}

detected_agent=$(auto_detect_agent)

AGENT="${AGENT_CMD:-${AGENT_EXPLICIT:-$detected_agent}}"

# Validate
if [[ -z "$AGENT" ]]; then
    echo "ERROR: No agent found. Install pi (npm i -g @earendil-works/pi) or claude CLI."
    exit 1
fi

# --- MODEL SELECTION ---
# Models are set via env vars, not hardcoded.
# Plan and build can use different models.
# Sensible defaults per agent but never forced.
#
#   AGENT_MODEL_PLAN=<id>     Plan-only model (default per agent below)
#   AGENT_MODEL_BUILD=<id>    Build-only model (default per agent below)
#   AGENT_MODEL=<id>          Legacy — overrides BOTH

case "$AGENT" in
    pi)
        # pi uses its configured model by default (no --model flag needed).
        # To override: AGENT_MODEL_PLAN="google/gemini-2.5-pro"
        AGENT_BIN="pi"
        AGENT_FLAGS=""                         # pi reads from config
        DEFAULT_MODEL_PLAN=""                  # uses pi's configured model
        DEFAULT_MODEL_BUILD=""                 # uses pi's configured model
        ;;
    claude|claude-cli)
        # claude CLI needs explicit model flags.
        # Override: AGENT_MODEL_PLAN="claude-sonnet-4-6"
        AGENT_BIN="claude"
        AGENT_FLAGS="-p --dangerously-skip-permissions"
        DEFAULT_MODEL_PLAN="${AGENT_MODEL_PLAN:-claude-opus-4-7}"
        DEFAULT_MODEL_BUILD="${AGENT_MODEL_BUILD:-claude-sonnet-4-6}"
        ;;
    *)
        echo "ERROR: Unknown agent '$AGENT'. Supported: pi, claude"
        exit 1
        ;;
esac

# Apply model selection (env overrides or defaults)
MODEL_PLAN="${AGENT_MODEL_PLAN:-$DEFAULT_MODEL_PLAN}"
MODEL_BUILD="${AGENT_MODEL_BUILD:-$DEFAULT_MODEL_BUILD}"

# AGENT_MODEL overrides both
if [[ -n "${AGENT_MODEL:-}" ]]; then
    MODEL_PLAN="$AGENT_MODEL"
    MODEL_BUILD="$AGENT_MODEL"
fi

# Build the command strings
if [[ -n "$MODEL_PLAN" ]]; then
    AGENT_CMD_PLAN="$AGENT_BIN --model $MODEL_PLAN $AGENT_FLAGS"
else
    AGENT_CMD_PLAN="$AGENT_BIN $AGENT_FLAGS"
fi

if [[ -n "$MODEL_BUILD" ]]; then
    AGENT_CMD_BUILD="$AGENT_BIN --model $MODEL_BUILD $AGENT_FLAGS"
else
    AGENT_CMD_BUILD="$AGENT_BIN $AGENT_FLAGS"
fi

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
    echo "  $0 plan <spec-name>      Run planning mode (gap analysis, no code)"
    echo "  $0 build <spec-name>     Run build mode (one task per iteration)"
    echo "  $0 --help                Show this help"
    echo ""
    echo "Agent: $AGENT (auto-detected | override with --agent or \$AGENT_CMD)"
    if [[ "$AGENT" == "claude" ]]; then
        echo "  Plan model:  $MODEL_PLAN  (override: AGENT_MODEL_PLAN=<id>)"
        echo "  Build model: $MODEL_BUILD (override: AGENT_MODEL_BUILD=<id>)"
    else
        echo "  Model: uses pi's configured model  (override: AGENT_MODEL=<id>)"
    fi
    echo ""
    echo "Env:"
    echo "  AGENT_CMD=<name>         Agent binary (pi, claude)"
    echo "  AGENT_MODEL_PLAN=<id>    Plan-mode model"
    echo "  AGENT_MODEL_BUILD=<id>   Build-mode model"
    echo "  AGENT_MODEL=<id>         Legacy — overrides BOTH plan and build"
    echo "  MAX_ITERATIONS=N         Max iterations (default: 15)"
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

    # 1. Instructions (stable, cache-friendly)
    echo "---"
    echo "## Instructions"
    echo ""
    if [[ "$mode" == "plan" ]]; then
        cat "$PROMPT_PLAN"
    else
        cat "$PROMPT_BUILD"
    fi
    echo ""

    # 2. Constitution
    if [[ -f "$CONSTITUTION" ]]; then
        echo "---"
        echo "## Constitution"
        echo ""
        cat "$CONSTITUTION"
        echo ""
    fi

    # 3. Specification
    if [[ -f "$SPECS_DIR/$spec/spec.md" ]]; then
        echo "---"
        echo "## Specification: $spec"
        echo ""
        cat "$SPECS_DIR/$spec/spec.md"
        echo ""
    fi

    # 4. Operational rules
    if [[ -f "$AGENTS_MD" ]]; then
        echo "---"
        echo "## Operational Rules"
        echo ""
        cat "$AGENTS_MD"
        echo ""
    fi

    # 5. Per-spec learnings
    local spec_learnings="$SPECS_DIR/$spec/LEARNINGS.md"
    if [[ -f "$spec_learnings" ]]; then
        echo "---"
        echo "## Spec Learnings: $spec"
        echo ""
        cat "$spec_learnings"
        echo ""
    fi

    # 6. Implementation plan (most volatile, last)
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
    echo "Agent: $AGENT"
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
    output=$(build_context "$spec" "plan" | $AGENT_CMD_PLAN 2>&1) || true

    echo "$output"

    echo ""
    echo -e "${GREEN}Planning complete. Review IMPLEMENTATION_PLAN.md${NC}"
    echo "Next: $0 build $spec"
}

run_building() {
    local spec="$1"
    local iteration=0

    echo -e "${BLUE}=========================================="
    echo "Vox: BUILD MODE"
    echo "Spec: $spec"
    echo "Agent: $AGENT"
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
        output=$(build_context "$spec" "build" | $AGENT_CMD_BUILD 2>&1) || true

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
        --agent)
            AGENT_EXPLICIT="$2"
            AGENT="$2"
            shift 2
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
echo "Agent:  $AGENT"
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
