---

# The Ralph Wiggum Flow: Master Protocol

## 1. User Instructions (Bootstrap Prompt)

*Copy and paste the following block into your AI CLI (Claude Code, Aider, etc.) to initialize a new project:*

```text
I want to initialize this project using the "Ralph Flow" methodology. 

1. READ the Master Protocol below (or read the file at ~/1-projects/claude_plans/ralph-guide.md if available).
2. SCAFFOLD the directory structure and files exactly as defined in the Protocol.
3. INTERVIEW ME to populate the dynamic configuration:
   - Ask for the "Tech Stack" to populate '.ralph/context.md'.
   - Ask for the "First Feature" to create atomic tasks in '.ralph/prd.json'.
   - Ask for the "Verification Commands" (build/test) to configure 'scripts/ralph.sh'.
4. DETECT my current environment (OS, Shell) to ensure scripts are executable.
5. CONFIGURE the 'scripts/ralph.sh' to use the correct CLI command for YOU (e.g., if you are 'claude', set AGENT_CMD="claude").

Do not start the loop yet. Just set up the environment, run 'chmod +x scripts/*.sh', and confirm when ready.

```
- Step 1: Write a detailed PRD 
- Step 2: Convert it to small, atomic user stories
- Step 3: Add clear acceptance criteria for each
- Step 4: Loop your AI agent through each story
- Step 5: It logs learnings so it doesn't repeat mistakes
- Step 6: Wake up, test, fix edge cases

---

## 2. Agent Reference: The Master Protocol

### Phase 1: Directory Architecture

The system requires this exact structure. Create these folders and files relative to the project root.

```text
/project-root
├── .ralph/                 # Agent State & Memory
│   ├── prd.json            # The Task Queue
│   ├── progress.md         # The Memory Log
│   └── context.md          # Tech Stack Rules
├── scripts/
│   ├── ralph.sh            # The Autonomous Controller
│   └── ralph-once.sh       # The Interactive Debugger
└── src/                    # (User's Source Code)

```

### Phase 2: Configuration Templates

#### A. The Queue (`.ralph/prd.json`)

*Instruction: Create this file. Initialize it with the user's "First Feature" broken down into atomic steps.*

```json
[
  {
    "id": "INIT-01",
    "description": "[Agent: Insert first atomic task from interview]",
    "file_context": ["src/"],
    "acceptance_criteria": "Code compiles and passes verification",
    "status": "pending"
  }
]

```

#### B. The Rulebook (`.ralph/context.md`)

*Instruction: Create this file. Populate it based on the Tech Stack interview.*

```markdown
# Project Context

## Tech Stack
- Language: [e.g., Rust, Go, TypeScript]
- Build Tool: [e.g., Cargo, Go modules, PNPM]
- Testing Framework: [e.g., Nextest, Pytest]

## Coding Standards
- [Agent: Insert idiomatic best practices for the chosen language]
- [Agent: Insert specific linter rules or strictness levels]

```

#### C. The Memory (`.ralph/progress.md`)

*Instruction: Create an empty file.*

---

### Phase 3: The Controller Scripts

#### A. The Loop (`scripts/ralph.sh`)

*Instruction: Create this script. Replace `[CHECK_CMD]` and `[TEST_CMD]` with the specific commands gathered during the interview.*

```bash
#!/bin/bash

# --- CONFIGURATION ---
# Agent: Set AGENT_CMD to the command used to invoke you (e.g., "claude", "aider --message")
AGENT_CMD="claude"
MAX_LOOPS=15

# Agent: Populate these during setup interview
CHECK_CMD="[INSERT_FAST_CHECK_CMD]"  # e.g., cargo check, go vet
TEST_CMD="[INSERT_TEST_CMD]"        # e.g., cargo test, go test ./...

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
  ROLE: You are an autonomous developer.
  GOAL: Complete the first task in '.ralph/prd.json' where status is 'pending'.
  
  INSTRUCTIONS:
  1. ANALYZE: Read '.ralph/prd.json' and focus ONLY on the first pending task.
  2. CONTEXT: Read '.ralph/context.md' for style guides and tech stack.
  3. IMPLEMENT: Write the code changes.
  4. VERIFY: Run '$CHECK_CMD'. You MUST fix all errors.
  5. TEST: Run '$TEST_CMD'. You MUST fix all failures.
  6. COMMIT: Run 'git add . && git commit -m \"feat: <task_id> <description>\"'.
  7. UPDATE STATE: Edit '.ralph/prd.json' changing that task's status to 'done'.
  8. LOG: Append a brief summary of what you did to '.ralph/progress.md'.
  "

  # 3. Execute Agent
  # Note: Adjust flags if using a different agent (e.g., Aider uses --file)
  $AGENT_CMD "$PROMPT" \
    --file .ralph/prd.json \
    --file .ralph/progress.md \
    --file .ralph/context.md

  # 4. Git Safety Check
  if [[ -n $(git status --porcelain) ]]; then
    echo "⚠️ Agent left uncommitted changes. Stashing and pausing."
    git stash
    exit 1
  fi
done

```

#### B. The Sniper (`scripts/ralph-once.sh`)

*Instruction: Create this script for interactive debugging.*

```bash
#!/bin/bash
# Agent: Ensure this matches the AGENT_CMD in ralph.sh
AGENT_CMD="claude"

echo "🎯 Sniper Mode: What task needs doing?"
read -r USER_TASK

PROMPT="
CONTEXT: Read .ralph/context.md
TASK: $USER_TASK
REQUIREMENT: The code must pass validation commands before you finish.
"

$AGENT_CMD "$PROMPT" --file .ralph/context.md

```

### Phase 4: Permissions

*Instruction: Finalize setup by making scripts executable.*
`chmod +x scripts/*.sh`

---

