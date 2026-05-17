# Parallel Vox Orchestrator — Design

> Date: 2026-04-03
> Status: Approved

## Problem

`/vox build` executes tasks serially — one agent, one task at a time. Many specs have tasks that could run in parallel (e.g. T2 and T3 both depend only on T1). This leaves performance on the table.

## Solution

Auto-detect parallelism in spec tasks using `bv --robot-plan`, then dispatch multiple agents into isolated git worktrees for independent tracks. Merge back with verification gates.

## Core Flow

```
/vox build <spec>
    │
    ├─ Phase 1: Plan
    │   └─ Read spec → gap analysis → generate tasks → write IMPLEMENTATION_PLAN.md
    │
    ├─ Phase 2: Graph Analysis (automatic)
    │   ├─ br init + br create per task + br dep for dependencies
    │   ├─ bv --robot-plan → identify parallel execution tracks
    │   └─ Decision gate:
    │       ├─ 1 track → serial vox build (existing behavior, zero overhead)
    │       └─ 2+ tracks → fork into parallel orchestrator
    │
    ├─ Phase 3: Parallel Execution
    │   ├─ Execute serial prefix tasks on main branch (foundation work)
    │   ├─ Verify foundation before forking
    │   ├─ Create git worktree per parallel track
    │   ├─ Dispatch agent per worktree (full spec context, scoped task list)
    │   └─ Wait for all agents to complete
    │
    └─ Phase 4: Merge & Verify
        ├─ Tag pre-parallel-merge rollback point
        ├─ Merge tracks sequentially in dependency order
        ├─ Run full verification after each merge
        ├─ On failure → stop, report, don't continue
        └─ On success → clean up worktrees, update plan, done
```

## Sequential-Then-Parallel Split

```
T1 (foundation) ──→ T2 (independent) ──→ T4 (validate)
                ──→ T3 (independent) ──↗
```

1. **Serial prefix**: Run T1 on main branch via normal vox build. Commit.
2. **Parallel fan-out**: Worktree-A for T2, worktree-B for T3. Both branch from T1's commit. Agents dispatched simultaneously with full spec context.
3. **Serial merge + validate**: Merge tracks back one at a time. Verify after each. T4 runs last on merged result.

## Safety Gates

1. **Pre-fork verification** — Verify foundation passes before forking
2. **Per-agent verification** — Each agent runs verification in its worktree before committing
3. **Sequential merge order** — bv determines order, not random
4. **Post-merge verification** — Full verification suite after each merge
5. **Rollback point** — `pre-parallel-merge` tag enables clean recovery

## What This Does NOT Do

- No force pushes
- No merging past a verification failure
- No deleting branches until everything passes
- No modifying files outside assigned tasks

## Design Decisions

- **Auto-detect**: No `--parallel` flag. If bv finds 1 track, serial. If 2+, parallel. Transparent.
- **Full context per agent**: Each agent gets spec + constitution + AGENTS.md (same as vox build today). Simpler, safer than scoped slices.
- **Fallback**: If br/bv unavailable, falls back to serial vox build. No hard dependency.
- **Agent tool**: Uses Claude Code's Agent tool with `isolation: "worktree"` for proper git isolation.
