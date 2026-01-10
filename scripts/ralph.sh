#!/bin/bash

# --- CONFIGURATION ---
AGENT_CMD="claude"
MAX_LOOPS=15

# Verification commands for testudo-exchange (Rust)
CHECK_CMD="cd testudo-exchange && cargo check"
TEST_CMD="cd testudo-exchange && cargo test"

# --- THE LOOP ---
for i in $(seq 1 $MAX_LOOPS); do
  echo "🤖 Ralph Iteration: $i"

  # 1. Check for Pending Work
  if ! grep -q '"status": "pending"' .ralph/prd.json; then
    echo "✅ All tasks complete. Ralph is sleeping."
    exit 0
  fi

  # 2. Construct the Prompt
  PROMPT="
  ROLE: You are an autonomous developer working on testudo-exchange (Rust).
  GOAL: Complete the first task in '.ralph/prd.json' where status is 'pending'.

  INSTRUCTIONS:
  1. ANALYZE: Read '.ralph/prd.json' and focus ONLY on the first pending task.
  2. CONTEXT: Read '.ralph/context.md' for style guides and tech stack.
  3. IMPLEMENT: Write the code changes following TDD (write test first, then code).
  4. VERIFY: Run '$CHECK_CMD'. You MUST fix all errors.
  5. TEST: Run '$TEST_CMD'. You MUST fix all failures.
  6. COMMIT: Run 'git add . && git commit -m \"feat: <task_id> <description>\"'.
  7. UPDATE STATE: Edit '.ralph/prd.json' changing that task's status to 'done'.
  8. LOG: Append a brief summary of what you did to '.ralph/progress.md'.
  "

  # 3. Execute Agent
  $AGENT_CMD "$PROMPT"

  # 4. Git Safety Check
  if [[ -n $(git status --porcelain) ]]; then
    echo "⚠️ Agent left uncommitted changes. Stashing and pausing."
    git stash
    exit 1
  fi
done
