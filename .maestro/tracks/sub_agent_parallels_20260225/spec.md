# Specification: Add Sub Agent Parallels to maestro:implement

## Overview
Add a third execution mode (`--parallel`) to `maestro:implement` alongside the existing single-agent (default) and `--team` modes. In `--parallel` mode, the main session analyzes the plan for independent tasks, spawns Task sub-agents to execute them concurrently, then verifies and merges results before committing. Sub-agents can read, write, and edit files but cannot commit. The main session owns verification, merging, and all git operations.

## Type
feature

## Requirements

### Functional Requirements
1. New `--parallel` flag for `maestro:implement` that activates sub-agent parallel mode
2. Main session analyzes plan.md to identify independent tasks (no dependency conflicts, no overlapping file scopes)
3. Main session spawns Task sub-agents (using the Task tool) for each independent task in a wave
4. Sub-agents execute tasks following TDD or ship-fast methodology (per workflow.md)
5. Sub-agents can read, write, and edit files -- but cannot commit, touch BR state, or modify plan.md
6. Main session collects sub-agent results, verifies correctness (re-reads files, re-runs tests), resolves conflicts, and commits
7. Sequential fallback: tasks with dependencies or overlapping file scopes execute sequentially in the main session
8. Wave-based execution: spawn a batch of independent tasks, wait for completion, verify, commit, then spawn the next batch

### User Interaction
- CLI command: `/maestro:implement [<track-name>] --parallel`
- Mode is transparent to the user -- same progress tracking and status updates as other modes
- Main session reports parallelism decisions (which tasks run in parallel, which are sequential) before executing

### Non-Functional Requirements
- Sub-agents must use worktree isolation to prevent file conflicts between parallel agents
- Main session is the single point of truth for git state -- no parallel commits
- Graceful degradation: if sub-agent spawning fails or rate limits hit, fall back to sequential execution

## Edge Cases & Error Handling
- **File conflicts**: Two sub-agents edit the same file -- detect via git diff after completion, prompt user or auto-merge if non-overlapping hunks
- **Sub-agent failure**: If a sub-agent fails or times out, main session retries the task sequentially
- **Rate limits**: If Task tool hits rate limits, queue remaining tasks and execute sequentially
- **Orphaned sub-agents**: If main session is interrupted, sub-agents in worktrees are cleaned up on next run
- **All tasks dependent**: If no independent tasks found, fall back to single-agent mode with informational message
- **Mixed results**: If some sub-agents succeed and others fail, commit successful work and retry failed tasks

## Out of Scope
- Modifying the existing `--team` mode (remains unchanged)
- Sub-agents spawning their own sub-agents (single level only)
- Automatic dependency analysis beyond plan.md task ordering
- BR integration changes (sub-agents don't touch BR; main session handles it)

## Acceptance Criteria
- [ ] `--parallel` flag is recognized and documented in SKILL.md
- [ ] Independent tasks are correctly identified from plan.md
- [ ] Sub-agents execute in isolated worktrees and produce correct results
- [ ] Main session verifies sub-agent output (re-reads files, runs tests) before committing
- [ ] File conflicts between sub-agents are detected and handled
- [ ] Fallback to sequential execution works when parallelism is not possible
- [ ] Phase completion protocol works correctly after parallel execution
- [ ] Reference doc `reference/parallel-mode.md` is complete and self-contained
