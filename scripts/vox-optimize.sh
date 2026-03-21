#!/bin/bash
#
# Vox Optimizer Loop - Iterative experimentation with batched human review
# "God spoke and it was."
#
# Usage:
#   ./scripts/vox-optimize.sh <target>              # Run with default batch size (5)
#   ./scripts/vox-optimize.sh <target> --batch 3    # Custom batch size
#   ./scripts/vox-optimize.sh --help                # Show help
#

set -euo pipefail

# --- CONFIGURATION ---
OPTIMIZE_DIR=".specify/optimize"
CONSTITUTION=".specify/memory/constitution.md"
AGENTS_MD=".specify/AGENTS.md"
PROMPT_OPTIMIZE=".specify/PROMPT_optimize.md"

BATCH_SIZE=5
AGENT_CMD="claude"

# --- COLORS ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# --- HELPER FUNCTIONS ---

print_help() {
    echo "Vox Optimizer Loop - Iterative Experimentation"
    echo ""
    echo "Usage:"
    echo "  $0 <target>              Run optimization (default batch: 5)"
    echo "  $0 <target> --batch N    Set experiments per checkpoint"
    echo "  $0 --help                Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 matching-engine"
    echo "  $0 matching-engine --batch 3"
    echo ""
    echo "Available targets:"
    list_targets
    echo ""
    echo "Create a new target:"
    echo "  mkdir -p .specify/optimize/<target>"
    echo "  Write .specify/optimize/<target>/program.md"
    echo "  Optionally add .specify/optimize/<target>/benchmark.sh"
}

list_targets() {
    if [[ -d "$OPTIMIZE_DIR" ]]; then
        for d in "$OPTIMIZE_DIR"/*/; do
            [[ -d "$d" ]] || continue
            if [[ -f "${d}program.md" ]]; then
                basename "$d"
            fi
        done | sort
    fi
}

# Run benchmark N times, return median
run_benchmark_median() {
    local benchmark_script="$1"
    local runs="${2:-3}"
    local values=()

    for ((i=1; i<=runs; i++)); do
        local result
        result=$("$benchmark_script" 2>/dev/null | grep -oP 'METRIC=\K[0-9.]+' || echo "")
        if [[ -n "$result" ]]; then
            values+=("$result")
        fi
    done

    if [[ ${#values[@]} -eq 0 ]]; then
        echo ""
        return 1
    fi

    # Sort and pick median
    IFS=$'\n' sorted=($(sort -g <<<"${values[*]}")); unset IFS
    local mid=$(( ${#sorted[@]} / 2 ))
    echo "${sorted[$mid]}"
}

# Parse benchmark runs from program.md
get_benchmark_runs() {
    local program="$1"
    grep -oP 'BENCHMARK_RUNS=\K[0-9]+' "$program" 2>/dev/null || echo "3"
}

# Parse metric direction from program.md
get_metric_direction() {
    local program="$1"
    grep -oP 'METRIC_DIRECTION=\K\w+' "$program" 2>/dev/null || echo "MINIMIZE"
}

# Extract experiment description from last commit message
get_experiment_description() {
    git log -1 --format='%s' | sed 's/^experiment: //'
}

# Append a row to results.tsv (bash-owned, never written by Claude)
append_tsv() {
    local tsv_file="$1"
    local batch="$2"
    local exp_id="$3"
    local commit_hash="$4"
    local description="$5"
    local metric="$6"
    local baseline="$7"
    local status="$8"
    local timestamp
    timestamp=$(date -Iseconds)

    # Create header if file doesn't exist
    if [[ ! -f "$tsv_file" ]]; then
        printf "timestamp\tbatch\texp_id\tcommit_hash\tdescription\tmetric\tbaseline\tstatus\n" > "$tsv_file"
    fi

    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
        "$timestamp" "$batch" "$exp_id" "$commit_hash" "$description" "$metric" "$baseline" "$status" \
        >> "$tsv_file"
}

# Print the scoreboard at checkpoint
print_scoreboard() {
    local tsv_file="$1"
    local batch_num="$2"
    local baseline_metric="$3"
    local has_metric="$4"
    local direction="$5"

    echo ""
    echo -e "${BOLD}${CYAN}=======================================================${NC}"
    echo -e "${BOLD}${CYAN}  CHECKPOINT — Batch $batch_num Results${NC}"
    echo -e "${BOLD}${CYAN}=======================================================${NC}"
    echo ""

    if [[ "$has_metric" == "true" ]]; then
        echo -e "  ${BOLD}Baseline: $baseline_metric${NC}"
        echo ""
    fi

    while IFS=$'\t' read -r ts batch eid hash desc metric bl status; do
        [[ "$batch" == "$batch_num" ]] || continue
        [[ "$status" == "failed" ]] && {
            printf "  ${RED}#%-2s  %-40s  FAILED${NC}\n" "$eid" "$desc"
            continue
        }

        if [[ "$has_metric" == "true" && -n "$metric" && "$metric" != "-" && -n "$bl" && "$bl" != "-" ]]; then
            local delta
            delta=$(awk "BEGIN { printf \"%.1f\", (($metric - $bl) / $bl) * 100 }")

            # Color based on direction: for MINIMIZE, negative delta = good
            local sign="" color="$GREEN" marker=""
            if [[ "$direction" == "MINIMIZE" ]]; then
                if (( $(awk "BEGIN { print ($delta > 0) }") )); then
                    sign="+"
                    color="$RED"
                    if (( $(awk "BEGIN { print ($delta > 5) }") )); then
                        marker="  <- regression"
                    fi
                fi
            else
                # MAXIMIZE: positive delta = good
                if (( $(awk "BEGIN { print ($delta < 0) }") )); then
                    color="$RED"
                    if (( $(awk "BEGIN { print ($delta < -5) }") )); then
                        marker="  <- regression"
                    fi
                else
                    sign="+"
                fi
            fi
            printf "  ${color}#%-2s  %-40s  %-10s  (%s%s%%)  %s%s${NC}\n" \
                "$eid" "$desc" "$metric" "$sign" "$delta" "$hash" "$marker"
        else
            printf "  #%-2s  %-40s  %s\n" "$eid" "$desc" "$hash"
        fi
    done < <(tail -n +2 "$tsv_file")

    echo ""
    echo -e "${BOLD}${CYAN}=======================================================${NC}"
}

# Build context for Claude in optimize mode
build_optimize_context() {
    local target="$1"
    local target_dir="$OPTIMIZE_DIR/$target"

    echo "# Context for Vox"
    echo ""
    echo "## Mode: OPTIMIZE"
    echo "## Target: $target"
    echo ""

    if [[ -f "$CONSTITUTION" ]]; then
        echo "---"
        echo "## Constitution"
        echo ""
        cat "$CONSTITUTION"
        echo ""
    fi

    if [[ -f "$target_dir/program.md" ]]; then
        echo "---"
        echo "## Optimization Program"
        echo ""
        cat "$target_dir/program.md"
        echo ""
    fi

    if [[ -f "$AGENTS_MD" ]]; then
        echo "---"
        echo "## Operational Learnings"
        echo ""
        cat "$AGENTS_MD"
        echo ""
    fi

    if [[ -f "$target_dir/results.tsv" ]]; then
        echo "---"
        echo "## Experiment History"
        echo ""
        cat "$target_dir/results.tsv"
        echo ""
    fi

    echo "---"
    echo "## Instructions"
    echo ""
    sed "s/{TARGET}/$target/g" "$PROMPT_OPTIMIZE"
}

# --- MAIN LOGIC ---

TARGET=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --batch)
            BATCH_SIZE="$2"
            shift 2
            ;;
        --help|-h)
            print_help
            exit 0
            ;;
        *)
            if [[ -z "$TARGET" ]]; then
                TARGET="$1"
            fi
            shift
            ;;
    esac
done

if [[ -z "$TARGET" ]]; then
    echo -e "${RED}ERROR: Target name required${NC}"
    echo ""
    print_help
    exit 1
fi

TARGET_DIR_PATH="$OPTIMIZE_DIR/$TARGET"
PROGRAM_FILE="$TARGET_DIR_PATH/program.md"
BENCHMARK_SCRIPT="$TARGET_DIR_PATH/benchmark.sh"
RESULTS_FILE="$TARGET_DIR_PATH/results.tsv"

if [[ ! -f "$PROGRAM_FILE" ]]; then
    echo -e "${RED}ERROR: program.md not found at $PROGRAM_FILE${NC}"
    echo ""
    echo "Create the target first:"
    echo "  mkdir -p $TARGET_DIR_PATH"
    echo "  Write $PROGRAM_FILE"
    exit 1
fi

if [[ ! -f "$PROMPT_OPTIMIZE" ]]; then
    echo -e "${RED}ERROR: Missing $PROMPT_OPTIMIZE${NC}"
    exit 1
fi

# --- DETERMINE BENCHMARK SETTINGS ---
HAS_BENCHMARK=false
BENCHMARK_RUNS=3
METRIC_DIRECTION="MINIMIZE"

if [[ -f "$BENCHMARK_SCRIPT" && -x "$BENCHMARK_SCRIPT" ]]; then
    HAS_BENCHMARK=true
    BENCHMARK_RUNS=$(get_benchmark_runs "$PROGRAM_FILE")
    METRIC_DIRECTION=$(get_metric_direction "$PROGRAM_FILE")
fi

# --- SETUP WORKTREE ---
REPO_ROOT=$(git rev-parse --show-toplevel)
REPO_NAME=$(basename "$REPO_ROOT")
WORKTREE_PATH="$(dirname "$REPO_ROOT")/${REPO_NAME}-optimize-${TARGET}"
BRANCH_NAME="optimize/$TARGET"

echo -e "${BLUE}=========================================="
echo "Vox: OPTIMIZE MODE"
echo "Target: $TARGET"
echo "Batch size: $BATCH_SIZE"
echo "Benchmark: $(if $HAS_BENCHMARK; then echo "yes ($METRIC_DIRECTION, ${BENCHMARK_RUNS} runs)"; else echo "none (qualitative)"; fi)"
echo -e "==========================================${NC}"
echo ""

# Check if worktree already exists
if [[ -d "$WORKTREE_PATH" ]]; then
    echo -e "${YELLOW}Worktree already exists at $WORKTREE_PATH${NC}"
    echo -e "${YELLOW}Resuming optimization session...${NC}"
else
    echo -e "${YELLOW}Creating worktree at $WORKTREE_PATH${NC}"

    if git show-ref --verify --quiet "refs/heads/$BRANCH_NAME" 2>/dev/null; then
        git worktree add "$WORKTREE_PATH" "$BRANCH_NAME"
    else
        git worktree add "$WORKTREE_PATH" -b "$BRANCH_NAME"
    fi
fi

# Work in the worktree
cd "$WORKTREE_PATH"

echo -e "${GREEN}Working in: $WORKTREE_PATH${NC}"
echo ""

# --- ESTABLISH BASELINE ---
BASELINE_METRIC="-"
BASELINE_COMMIT=$(git rev-parse --short HEAD)

if $HAS_BENCHMARK; then
    echo -e "${YELLOW}Running baseline benchmark ($BENCHMARK_RUNS runs)...${NC}"
    BASELINE_METRIC=$(run_benchmark_median "$BENCHMARK_SCRIPT" "$BENCHMARK_RUNS")
    if [[ -n "$BASELINE_METRIC" ]]; then
        echo -e "${GREEN}Baseline metric: $BASELINE_METRIC${NC}"
    else
        echo -e "${RED}WARNING: Benchmark produced no output. Continuing without metrics.${NC}"
        HAS_BENCHMARK=false
    fi
fi

# --- OPTIMIZATION LOOP ---
BATCH_NUM=1
TOTAL_EXPERIMENTS=0
TOTAL_KEPT=0

while true; do
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  Starting Batch $BATCH_NUM ($BATCH_SIZE experiments)${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    BATCH_BASELINE_COMMIT=$(git rev-parse --short HEAD)
    CURRENT_BASELINE="$BASELINE_METRIC"

    for ((exp=1; exp<=BATCH_SIZE; exp++)); do
        TOTAL_EXPERIMENTS=$((TOTAL_EXPERIMENTS + 1))

        echo ""
        echo -e "${YELLOW}--- Experiment $exp of $BATCH_SIZE (total: $TOTAL_EXPERIMENTS) ---${NC}"
        echo -e "${YELLOW}Vox speaks...${NC}"

        # Snapshot commit before Claude runs
        PRE_COMMIT=$(git rev-parse HEAD)

        # Run Claude with optimization context
        local_output=$(build_optimize_context "$TARGET" | $AGENT_CMD -p --dangerously-skip-permissions 2>&1) || true

        echo "$local_output"

        # Check if Claude made a new commit
        POST_COMMIT=$(git rev-parse HEAD)
        if [[ "$PRE_COMMIT" == "$POST_COMMIT" ]]; then
            echo -e "${RED}No commit detected — Claude may have failed. Logging as failed.${NC}"
            append_tsv "$RESULTS_FILE" "$BATCH_NUM" "$exp" "-" "no commit produced" "-" "$CURRENT_BASELINE" "failed"
            continue
        fi

        LATEST_COMMIT=$(git rev-parse --short HEAD)
        DESCRIPTION=$(get_experiment_description)

        # Run benchmark if available
        EXP_METRIC="-"
        if $HAS_BENCHMARK; then
            echo -e "${YELLOW}Running benchmark ($BENCHMARK_RUNS runs)...${NC}"
            EXP_METRIC=$(run_benchmark_median "$BENCHMARK_SCRIPT" "$BENCHMARK_RUNS")
            if [[ -z "$EXP_METRIC" ]]; then
                EXP_METRIC="-"
            fi
            echo -e "${CYAN}Metric: $EXP_METRIC (baseline: $CURRENT_BASELINE)${NC}"
        fi

        append_tsv "$RESULTS_FILE" "$BATCH_NUM" "$exp" "$LATEST_COMMIT" "$DESCRIPTION" "$EXP_METRIC" "$CURRENT_BASELINE" "pending"

        echo -e "${GREEN}Experiment $exp committed: $LATEST_COMMIT${NC}"

        sleep 2
    done

    # --- CHECKPOINT ---
    print_scoreboard "$RESULTS_FILE" "$BATCH_NUM" "$BASELINE_METRIC" "$HAS_BENCHMARK" "$METRIC_DIRECTION"

    echo ""
    echo -n -e "  ${BOLD}Keep through which experiment? (#, 'all', or 'none'): ${NC}"
    read -r choice < /dev/tty

    case "$choice" in
        none)
            echo -e "${YELLOW}Reverting all experiments in batch $BATCH_NUM...${NC}"
            git reset --hard "$BATCH_BASELINE_COMMIT" >/dev/null 2>&1
            # Mark all pending as discarded
            if [[ -f "$RESULTS_FILE" ]]; then
                awk -F'\t' -v OFS='\t' -v batch="$BATCH_NUM" '
                    NR == 1 { print; next }
                    $2 == batch && $8 == "pending" { $8 = "discarded"; print; next }
                    { print }
                ' "$RESULTS_FILE" > "${RESULTS_FILE}.tmp" && mv "${RESULTS_FILE}.tmp" "$RESULTS_FILE"
            fi
            echo -e "${GREEN}Reverted to baseline.${NC}"
            ;;
        all)
            echo -e "${GREEN}Keeping all experiments.${NC}"
            if [[ -f "$RESULTS_FILE" ]]; then
                awk -F'\t' -v OFS='\t' -v batch="$BATCH_NUM" '
                    NR == 1 { print; next }
                    $2 == batch && $8 == "pending" { $8 = "kept"; print; next }
                    { print }
                ' "$RESULTS_FILE" > "${RESULTS_FILE}.tmp" && mv "${RESULTS_FILE}.tmp" "$RESULTS_FILE"
            fi
            if $HAS_BENCHMARK; then
                LAST_METRIC=$(tail -1 "$RESULTS_FILE" | cut -f6)
                if [[ "$LAST_METRIC" != "-" ]]; then
                    BASELINE_METRIC="$LAST_METRIC"
                fi
            fi
            TOTAL_KEPT=$((TOTAL_KEPT + BATCH_SIZE))
            ;;
        [0-9]*)
            KEEP_THROUGH=$choice
            echo -e "${YELLOW}Keeping through experiment #$KEEP_THROUGH, discarding rest...${NC}"

            KEEP_HASH=$(awk -F'\t' -v batch="$BATCH_NUM" -v eid="$KEEP_THROUGH" \
                '$2 == batch && $3 == eid { print $4 }' "$RESULTS_FILE")

            if [[ -n "$KEEP_HASH" && "$KEEP_HASH" != "-" ]]; then
                git reset --hard "$KEEP_HASH" >/dev/null 2>&1

                awk -F'\t' -v OFS='\t' -v batch="$BATCH_NUM" -v keep="$KEEP_THROUGH" '
                    NR == 1 { print; next }
                    $2 == batch && $3+0 <= keep+0 && $8 == "pending" { $8 = "kept"; print; next }
                    $2 == batch && $3+0 > keep+0 && $8 == "pending" { $8 = "discarded"; print; next }
                    { print }
                ' "$RESULTS_FILE" > "${RESULTS_FILE}.tmp" && mv "${RESULTS_FILE}.tmp" "$RESULTS_FILE"

                if $HAS_BENCHMARK; then
                    KEPT_METRIC=$(awk -F'\t' -v batch="$BATCH_NUM" -v eid="$KEEP_THROUGH" \
                        '$2 == batch && $3 == eid { print $6 }' "$RESULTS_FILE")
                    if [[ "$KEPT_METRIC" != "-" ]]; then
                        BASELINE_METRIC="$KEPT_METRIC"
                    fi
                fi
                TOTAL_KEPT=$((TOTAL_KEPT + KEEP_THROUGH))
            else
                echo -e "${RED}Could not find commit for experiment #$KEEP_THROUGH${NC}"
            fi

            echo -e "${GREEN}Done. New baseline: $(git rev-parse --short HEAD)${NC}"
            ;;
        *)
            echo -e "${RED}Invalid choice. Keeping all by default.${NC}"
            if [[ -f "$RESULTS_FILE" ]]; then
                awk -F'\t' -v OFS='\t' -v batch="$BATCH_NUM" '
                    NR == 1 { print; next }
                    $2 == batch && $8 == "pending" { $8 = "kept"; print; next }
                    { print }
                ' "$RESULTS_FILE" > "${RESULTS_FILE}.tmp" && mv "${RESULTS_FILE}.tmp" "$RESULTS_FILE"
            fi
            ;;
    esac

    echo ""
    echo -n -e "  ${BOLD}Continue optimizing? (y/n): ${NC}"
    read -r continue_choice < /dev/tty

    if [[ "$continue_choice" != "y" && "$continue_choice" != "Y" ]]; then
        break
    fi

    BATCH_NUM=$((BATCH_NUM + 1))
done

# --- COMPLETION ---
echo ""
echo -e "${GREEN}=========================================="
echo "  Vox Optimization Complete"
echo "==========================================${NC}"
echo ""
echo "  Target:      $TARGET"
echo "  Batches:     $BATCH_NUM"
echo "  Experiments: $TOTAL_EXPERIMENTS"
echo "  Kept:        $TOTAL_KEPT"
if $HAS_BENCHMARK; then
    echo "  Final metric: $BASELINE_METRIC"
fi
echo "  Worktree:    $WORKTREE_PATH"
echo "  Branch:      $BRANCH_NAME"
echo ""

echo -n -e "  ${BOLD}Merge worktree branch to main? (y/n): ${NC}"
read -r merge_choice < /dev/tty

if [[ "$merge_choice" == "y" || "$merge_choice" == "Y" ]]; then
    cd "$REPO_ROOT"
    echo -e "${YELLOW}Merging $BRANCH_NAME...${NC}"
    git merge "$BRANCH_NAME" --no-ff -m "feat: merge optimization results for $TARGET

Experiments: $TOTAL_EXPERIMENTS, Kept: $TOTAL_KEPT
$(if $HAS_BENCHMARK; then echo "Final metric: $BASELINE_METRIC"; fi)

Co-Authored-By: Claude <noreply@anthropic.com>"

    echo -e "${YELLOW}Cleaning up worktree...${NC}"
    git worktree remove "$WORKTREE_PATH"
    echo -e "${GREEN}Merged and cleaned up.${NC}"
else
    echo ""
    echo "  Worktree preserved at: $WORKTREE_PATH"
    echo "  To merge later:  cd $REPO_ROOT && git merge $BRANCH_NAME"
    echo "  To discard:      git worktree remove $WORKTREE_PATH && git branch -D $BRANCH_NAME"
fi
