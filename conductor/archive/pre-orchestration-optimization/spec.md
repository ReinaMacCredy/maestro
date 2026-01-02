# Pre-Orchestration Token Optimization

## Overview

Reduce token consumption before parallel worker execution by 60-75%. Currently, `/conductor-implement` consumes ~8-12k tokens before spawning the first worker due to eager skill loading, redundant analysis, and verbose Agent Mail setup.

## Problem Statement

Token sinks identified (Oracle-audited):

| Sink | Current Cost |
|------|--------------|
| Orchestrator SKILL + 16 references | ~5-7k tokens |
| Conductor/Maestro-core skills | ~1-2k tokens |
| Phase 0/0.5: Preflight + Handoff | ~1-2k tokens |
| Phase 2b: Routing analysis | ~300-500 tokens |
| Agent Mail setup (7+ calls) | ~500 tokens |

## Functional Requirements

### FR-1: Lazy Skill Reference Loading
- Orchestrator SKILL.md loads without references initially
- References load on-demand when workflow phase requires them
- Loader supports both `## References` (eager) and `## Lazy References` (deferred)
- Inline summaries in SKILL.md for critical flows

### FR-2: Harden Trust plan.md Fast-Path
- Skip `group_by_file_scope()` when `## Track Assignments` exists in plan.md
- Use parsed Track Assignments for confirmation prompt
- Add light validation against `metadata.json.beads.planTasks`

### FR-3: Conditional Preflight/Handoff
- Skip `bv --robot-triage` when `metadata.beads.status == "complete"`
- Skip handoff load when no recent handoffs exist (>7 days)
- Cache bead state in metadata.json for faster re-runs

### FR-4: Normalize Agent Mail Protocol
- Decide single model: workers self-register OR orchestrator pre-registers
- Orchestrator uses `macro_start_session` (1 call vs 3+)
- EPIC START message sends to self, workers join thread later
- Update `workflow.md`, `agent-mail.md`, `worker-prompt.md` consistently

## Non-Functional Requirements

### NFR-1: Performance
- Pre-spawn tokens: ~3-4k typical (down from ~8-12k)
- Best-case pre-spawn: ~1.5-2.5k
- Time to first worker: ~15s (down from ~30s)

### NFR-2: Compatibility
- Support both `## References` and `## Lazy References` in loader
- Fallback to eager loading if lazy triggers fail
- No breaking changes to existing workflows

### NFR-3: Maintainability
- Lazy reference trigger conditions documented in SKILL.md
- Lint/tests to validate reference coverage

## Acceptance Criteria

1. ✅ Running `ci <track>` with Track Assignments consumes <4k tokens before first worker
2. ✅ Preflight is skipped when `metadata.beads.status == "complete"`
3. ✅ Orchestrator registers itself with single `macro_start_session` call
4. ✅ Workers successfully self-register and join epic thread
5. ✅ All existing parallel execution tests pass
6. ✅ No regression in sequential execution path

## Out of Scope

- Changes to beads CLI (`bd`)
- Changes to MCP Agent Mail server implementation
- Optimization of worker-side token consumption
- Changes to design session (`ds`) token usage
