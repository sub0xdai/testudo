---
id: 24ec08e3-a247-41ac-86c5-8ab262f39215
tags:
  - ai
  - claude
  - project
alias: []
desc: ""
title: "Master Claude: Enhanced Best Practices 2025"
---
# Master Claude: Enhanced Best Practices 2025

## Executive Summary
This document provides a comprehensive framework for maximizing productivity with Claude Code through strategic setup, proven workflows, and advanced optimization techniques. Claude Code is intentionally low-level and unopinionated, providing close to raw model access without forcing specific workflows, making proper methodology crucial for success.

---

## 🏗️ Strategic Setup & Architecture

### Core Repository Structure (example)
```bash
## Architecture Overview
This is a full-stack customer intelligence platform with:
- **Frontend**: Next.js 14+ App Router with TypeScript
- **Backend**: FastAPI with LlamaIndex for AI/ML capabilities  
- **Database**: Supabase (PostgreSQL + Auth)
- **Deployment**: Frontend on Vercel, Backend on fly.io

## Frontend Structure
The frontend uses Next.js App Router with feature-based organization:
- `app/` - App Router pages and API routes
- `actions/` - Server actions for API calls (e.g., `addBatch.ts`, `runBatchAnswer.ts`)
- `components/` - Feature-based React components
- `types/` - TypeScript type definitions

## Backend Structure
The backend is a FastAPI application with:
- `backend/py` - FastAPI app entry point
- `backend/question_answering/app/` - Main application code
  - `api/routes/` - API endpoints
  - `core/` - Configuration and LlamaIndex setup
  - `services/` - Business logic (answer_service, file_upload_service, etc.)
  - `schemas/` - Pydantic models for API contracts
```

### Enhanced CLAUDE.md Framework

#### Primary CLAUDE.md (Root Level)
```markdown
# Project Architecture & Core Commands

## Essential Commands
- `npm run build` - Build the project
- `npm run typecheck` - Run TypeScript checker
- `npm run test` - Run test suite (prefer single tests for performance)
- `npm run lint` - Run ESLint with auto-fix
- `npm run format` - Run Prettier formatter

## Code Standards
- Use ES modules (import/export) syntax, not CommonJS (require)
- Destructure imports when possible (e.g., `import { foo } from 'bar'`)
- Follow conventional commit format: `type(scope): description`
- Prefer composition over inheritance
- Write tests BEFORE implementation (TDD approach)

## Development Workflow
- Always run typecheck after code changes
- Use pre-commit hooks for quality gates
- Run single tests during development, full suite before commits
- Update this CLAUDE.md when you discover new patterns or solutions

## Quality Gates
- All tests must pass
- TypeScript compilation successful
- Linting rules followed
- Code coverage maintained above 80%

## Architecture Principles
- Feature-based folder organization
- Clear separation of concerns (API, business logic, UI)
- Dependency injection for testability
- Error boundaries and graceful degradation
```

#### Feature-Specific CLAUDE.md Files
Create `claude.md` files for major subfolders:

```markdown
# Frontend Components (components/claude.md)

## Component Patterns
- Use TypeScript interfaces for all prop types
- Implement error boundaries for complex components  
- Follow atomic design principles (atoms → molecules → organisms)
- Use React.memo for performance optimization

## State Management
- Use React Query for server state
- Zustand for client state management
- Context sparingly, only for truly global state

## Testing Approach
- Unit tests with React Testing Library
- Integration tests for user workflows
- Visual regression tests for UI components

## References
When Claude needs component examples, reference:
- `components/ui/` - Base atomic components
- `components/feature/` - Feature-specific components
- `__tests__/` - Test patterns and utilities
```

### Advanced Configuration

#### Permission Management
Use the `/permissions` command after starting Claude Code to add or remove tools from the allowlist. Recommended allowlist:
```json
{
  "allowedTools": [
    "Edit",
    "Bash(git commit:*)",
    "Bash(npm run:*)", 
    "Bash(git push:*)",
    "mcp__puppeteer__puppeteer_navigate"
  ]
}
```

#### MCP Integration
Create `.mcp.json` for team-wide tool availability:
```json
{
  "servers": {
    "puppeteer": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-puppeteer"]
    },
    "sentry": {
      "command": "mcp-sentry-server",
      "env": {
        "SENTRY_AUTH_TOKEN": "${SENTRY_AUTH_TOKEN}"
      }
    }
  }
}
```

---

## 🔄 Proven Workflows

### 1. Enhanced Explore-Plan-Execute Pattern

#### Exploration Phase
```
# Context Gathering Protocol
1. "Read [specific files] and understand the current architecture. Don't write code yet."
2. "Use subagents to investigate [specific questions] about the codebase structure."
3. "Think hard about potential approaches and trade-offs."
```

#### Planning Phase  
We recommend using the word "think" to trigger extended thinking mode, which gives Claude additional computation time to evaluate alternatives more thoroughly

**Thinking Budget Levels:**
- `think` - Basic analysis (4,000 tokens)
- `think hard` - Moderate complexity
- `think harder` - Complex problems  
- `ultrathink` - Maximum thinking budget (31,999 tokens)

#### Execution Phase
```
# Implementation Protocol
1. "Implement your plan step by step."
2. "Verify each component as you build it."
3. "Run tests continuously during development."
4. "Update documentation as you go."
```

### 2. TDD-First Workflow (Recommended)

The robots LOVE TDD. Seriously. They eat it up. With TDD you have the robot friend build out the test, and the mock. Then your next prompt you build the mock to be real

```bash
# TDD Implementation Steps
1. "Write comprehensive tests for [feature] based on these requirements. Use TDD approach - avoid mock implementations."
2. "Run tests and confirm they fail appropriately."  
3. "Commit the tests with message: 'test: add tests for [feature]'"
4. "Implement code to make tests pass. Don't modify the tests."
5. "Iterate until all tests pass."
6. "Use independent subagents to verify implementation isn't overfitting to tests."
7. "Commit implementation with message: 'feat: implement [feature]'"
```

### 3. Visual-Driven Development
```bash
# Screenshot-Iterate Pattern
1. "Set up screenshot capability with Puppeteer MCP."
2. "Here's the design mock [attach image]."
3. "Implement the UI, take screenshots, and iterate until it matches."
4. "Focus on pixel-perfect implementation and responsive behavior."
```

### 4. Multi-Claude Parallel Workflows

Some of the most powerful applications involve running multiple Claude instances in parallel

#### Git Worktree Strategy
```bash
# Setup Multiple Workstreams  
git worktree add ../project-feature-a feature-a
git worktree add ../project-refactor-b refactor-b
git worktree add ../project-docs-c docs-update

# Launch Claude in each (separate terminals)
cd ../project-feature-a && claude
cd ../project-refactor-b && claude  
cd ../project-docs-c && claude
```

#### Code Review Workflow
```bash
# Writer Claude (Terminal 1)
"Implement the user authentication system following our security guidelines."

# Reviewer Claude (Terminal 2) 
"/clear"
"Review the authentication implementation in auth/ folder. Look for security vulnerabilities, code quality issues, and alignment with our patterns."

# Integration Claude (Terminal 3)
"/clear" 
"Read both the implementation and review feedback. Create final implementation addressing all concerns."
```

---

## 🚀 Advanced Optimization Techniques

### Custom Slash Commands

Create `.claude/commands/` directory with workflow templates:

#### `.claude/commands/feature-complete.md`
```markdown
Please implement a complete feature following our standards: $ARGUMENTS

## Implementation Steps:
1. Analyze existing patterns in the codebase
2. Write comprehensive tests first (TDD approach)
3. Implement the feature following our architecture
4. Ensure all quality gates pass:
   - TypeScript compilation
   - All tests passing  
   - Linting rules followed
   - Code coverage maintained
5. Update documentation and CLAUDE.md if needed
6. Create descriptive commit message following conventional commits
7. Prepare PR description with implementation details

## Quality Checklist:
- [ ] Tests written first and passing
- [ ] TypeScript types properly defined
- [ ] Error handling implemented
- [ ] Performance considerations addressed
- [ ] Security best practices followed
- [ ] Documentation updated
```

#### `.claude/commands/fix-github-issue.md`
```markdown
Please analyze and fix GitHub issue: $ARGUMENTS

## Resolution Protocol:
1. Use `gh issue view $ARGUMENTS` to get issue details
2. Understand the problem and gather relevant context
3. Search codebase for related files and patterns
4. Think hard about the root cause and potential solutions
5. Implement fix following our coding standards
6. Write/update tests to prevent regression
7. Verify all quality gates pass
8. Create descriptive commit message
9. Push changes and update issue with resolution details
```

### Pre-commit Quality Gates

#### `.pre-commit-config.yaml`
```yaml
repos:
  - repo: local
    hooks:
      - id: typescript-check
        name: TypeScript Check
        entry: npm run typecheck
        language: system
        pass_filenames: false
        
      - id: lint-and-format
        name: Lint and Format
        entry: npm run lint:fix && npm run format
        language: system
        pass_filenames: false
        
      - id: test-affected
        name: Test Affected Files
        entry: npm run test:affected
        language: system
        pass_filenames: false
```

### GitHub Integration

#### PR Review Configuration (`.github/claude-code-review.yml`)
```yaml
direct_prompt: |
  Review this pull request focusing on:
  
  ## Critical Issues Only
  - Security vulnerabilities
  - Logic bugs that could cause failures
  - Performance bottlenecks
  - Breaking changes to public APIs
  
  ## Standards Compliance
  - TypeScript type safety
  - Test coverage for new features
  - Adherence to established patterns
  
  Be concise. Only comment on genuine issues, not style preferences.
  Provide specific suggestions for fixes.
```

---

## 🎯 Power User Commands & Shortcuts

### Quick Commands Reference
```bash
# Thinking Commands
"think" | "think hard" | "think harder" | "ultrathink"

# Workflow Commands  
"qplan" - Analyze codebase consistency before planning
"qcode" - Implement plan and ensure tests pass
"qcheck" - Perform skeptical senior engineer review
"qgit" - Create conventional commit message and push

# Development Flow
"prepare to discuss [feature]" - Context gathering mode
"think architecturally first" - Focus on system design
"/clear" - Reset context between tasks
```

### Advanced Context Management

#### Context Priming Strategy
Context Priming by disler provides a systematic approach to priming Claude Code with comprehensive project context through specialized commands

Create specialized context commands:
```bash
# Project Context
/context:full - Load complete project understanding
/context:api - Focus on API layer context  
/context:frontend - Load frontend-specific patterns
/context:testing - Load testing frameworks and patterns
```

### Performance Optimization

#### Token Management
- Use the `/clear` command frequently between tasks to reset the context window
- Use specific file references rather than broad directory scans
- Leverage subagents for focused investigations
- Store frequently referenced patterns in CLAUDE.md files

#### Efficiency Hacks
```bash
# Skip Permission Prompts (Use with caution)
claude --dangerously-skip-permissions

# Headless Automation
claude -p "your prompt here" --output-format stream-json

# MCP Debug Mode  
claude --mcp-debug

# Verbose Mode for Debugging
claude --verbose
```

---

## 📋 Quality Assurance Framework

### Code Review Checklist
```markdown
## CLAUDE.md Checklist: Writing Functions Best Practices
- [ ] Single responsibility principle followed
- [ ] Clear, descriptive function names
- [ ] Proper TypeScript types for all parameters and returns
- [ ] Error handling implemented appropriately
- [ ] Unit tests written and passing
- [ ] Documentation comments for complex logic

## CLAUDE.md Checklist: Writing Tests Best Practices  
- [ ] Tests written before implementation (TDD)
- [ ] All edge cases covered
- [ ] Clear test descriptions (describe/it blocks)
- [ ] Proper setup and teardown
- [ ] No test interdependencies
- [ ] Mock external dependencies appropriately
```

### Continuous Improvement Protocol

#### Documentation Iteration
1. Use `#` key during development to add insights to CLAUDE.md
2. Regular CLAUDE.md reviews and refinements
3. Run CLAUDE.md files through the prompt improver and often tune instructions (e.g. adding emphasis with "IMPORTANT" or "YOU MUST")

#### Team Knowledge Sharing
```bash
# Development Notes Structure
changelog.md - Track significant changes and decisions
plan.md - Current development roadmap and priorities  
lessons-learned.md - Document solutions to complex problems
patterns.md - Reusable code patterns and examples
```

---

## 🔧 Tool Integration Excellence

### GitHub Actions Integration
```yaml
# .github/workflows/claude-review.yml
name: Claude Code Review
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  claude-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Claude Code Review
        uses: anthropics/claude-code-action@v1
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Development Environment Setup
```bash
# Essential Tool Installation
npm install -g @anthropic-ai/claude-code
brew install gh  # GitHub CLI
npm install -g pre-commit
uv tools install pre-commit  # Alternative Python package manager

# IDE Extensions
# - Claude Code VS Code extension
# - Enable multiple terminal panes for parallel workflows
```

---

## 🎓 Advanced Techniques

### Headless Automation Patterns

#### Batch Processing
```bash
# Fan-out Pattern for Large Migrations
claude -p "migrate foo.py from React to Vue. Return 'OK' if succeeded, 'FAIL' if failed." \
  --allowedTools Edit Bash(git commit:*)
```

#### Pipeline Integration
```bash
# Data Processing Pipeline
claude -p "analyze sentiment in this log data" --json | \
  your_processing_command | \
  claude -p "generate report from analysis results"
```

### Expert Consultation Patterns

#### Domain Expert Mode
Transform Claude Code custom agents from formal tools into engaging collaborators. Create custom agents like ಠ_ಠ Security Analyst with text-faces and nicknames

```bash
# Security Analysis Agent
"Act as ಠ_ಠ Security Analyst. Review this authentication code for vulnerabilities."

# Performance Optimization Agent  
"Act as ⚡ Performance Expert. Analyze this database query for optimization opportunities."

# Architecture Review Agent
"Act as 🏗️ Systems Architect. Evaluate this microservice design for scalability."
```

---

## 🚨 Risk Management & Safety

### Safe Automation Guidelines
- Use `--dangerously-skip-permissions` only in sandboxed environments
- Implement proper backup strategies before large refactoring
- Test automation scripts thoroughly before production use
- Monitor token usage and costs regularly

### Security Best Practices
- Never commit sensitive data to CLAUDE.md files
- Use `.gitignore` for local CLAUDE.local.md files with sensitive info
- Regular security audits of automated workflows
- Principle of least privilege for tool permissions

---

## 📊 Success Metrics & Optimization

### Performance Indicators
- Reduced development time for feature implementation
- Decreased code review cycles
- Improved test coverage and code quality
- Faster onboarding for new team members

### Continuous Optimization
- Weekly review of CLAUDE.md effectiveness
- Monthly analysis of workflow patterns
- Quarterly tool and process improvements
- Regular team retrospectives on AI-assisted development

---

## 🔮 Future-Proofing Strategies

### Emerging Patterns
- Integration with more specialized MCP servers
- Enhanced multi-modal capabilities (voice, video)
- Deeper IDE integration and workflow automation
- Advanced reasoning modes for complex architectural decisions

### Preparation Guidelines
- Keep CLAUDE.md files version-controlled and documented
- Maintain flexibility in workflow definitions
- Regular evaluation of new Claude Code features
- Community engagement for best practice sharing

---

*This document represents the culmination of best practices from the Claude Code community, Anthropic engineering teams, and real-world production usage. Continue iterating and improving based on your specific needs and discoveries.* "Create this pull request. Once you have completed it, look at the next pull request in this project. Someone else will be working on it. Give the next developer some advice to help tghem accomplish the next PR in the  markdown file"
