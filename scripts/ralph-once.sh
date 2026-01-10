#!/bin/bash

# Single-shot sniper mode for interactive debugging
AGENT_CMD="claude"

echo "🎯 Sniper Mode: What task needs doing?"
read -r USER_TASK

PROMPT="
CONTEXT: Read .ralph/context.md for project rules and tech stack.
TASK: $USER_TASK
REQUIREMENT: The code must pass 'cd testudo-exchange && cargo check && cargo test' before you finish.
"

$AGENT_CMD "$PROMPT"
