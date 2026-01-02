# Implementation Plan: Pre-Orchestration Token Optimization

## Phase 1: Harden Trust plan.md (LOW RISK)

### 1.1 Update Phase 2b fast-path in implement.md
- [ ] **Task 1.1.1**: Skip `group_by_file_scope()` when Track Assignments exist
  - File: `skills/conductor/references/workflows/implement.md`
  - Change: In Phase 2b, when `## Track Assignments` detected, parse table directly instead of calling grouping algorithm
  - AC: Track Assignments path skips all file scope analysis

- [ ] **Task 1.1.2**: Use parsed tracks for confirmation prompt
  - File: `skills/conductor/references/workflows/implement.md`
  - Change: Confirmation prompt uses `parse_track_assignments()` output, not `group_by_file_scope()` output
  - AC: Confirmation shows track info from plan.md without re-analysis

### 1.2 Add light validation
- [ ] **Task 1.2.1**: Add sanity check against beads.planTasks
  - File: `skills/conductor/references/workflows/implement.md`
  - Change: After parsing Track Assignments, verify task IDs exist in `metadata.json.beads.planTasks`
  - AC: Warning shown if Track Assignments references unknown tasks

---

## Phase 2: Preflight/Handoff Optimization (LOW RISK)

### 2.1 Conditional preflight
- [ ] **Task 2.1.1**: Skip `bv --robot-triage` when beads already filed
  - File: `skills/conductor/references/preflight-beads.md`
  - Change: Check `metadata.beads.status == "complete"` before running triage
  - AC: Preflight skipped when beads already filed and status is complete

- [ ] **Task 2.1.2**: Cache bead state in metadata.json
  - File: `skills/conductor/references/preflight-beads.md`
  - Change: Store triage results in `metadata.beads.triageCache`
  - AC: Re-runs use cached state instead of calling `bv`

### 2.2 Conditional handoff
- [ ] **Task 2.2.1**: Skip handoff load for fresh sessions
  - File: `skills/conductor/references/workflows/implement.md`
  - Change: Check if handoffs exist before attempting load
  - AC: No handoff load attempted when `conductor/handoffs/<track>/` is empty

- [ ] **Task 2.2.2**: Add 7-day freshness check
  - File: `skills/conductor/references/workflows/handoff.md`
  - Change: Skip handoffs older than 7 days
  - AC: Stale handoffs ignored with info message

---

## Phase 3: Normalize Agent Mail Protocol (MEDIUM RISK)

### 3.1 Update orchestrator registration
- [ ] **Task 3.1.1**: Use macro_start_session for orchestrator
  - File: `skills/orchestrator/references/workflow.md`
  - Change: Replace `ensure_project` + `register_agent` with single `macro_start_session`
  - AC: Orchestrator registers with 1 MCP call

- [ ] **Task 3.1.2**: Remove worker pre-registration
  - File: `skills/orchestrator/references/workflow.md`
  - Change: Remove loop that pre-registers all workers in Phase 3
  - AC: No `register_agent` calls for workers in orchestrator context

### 3.2 Update EPIC START message
- [ ] **Task 3.2.1**: Send to self instead of worker names
  - File: `skills/orchestrator/references/workflow.md`
  - Change: EPIC START message `to=[ORCHESTRATOR_NAME]` instead of `to=[all_workers]`
  - AC: Message succeeds without pre-registered recipients

- [ ] **Task 3.2.2**: Update worker join protocol
  - File: `skills/orchestrator/references/worker-prompt.md`
  - Change: Workers call `macro_start_session`, then `fetch_inbox` to find epic thread
  - AC: Workers successfully join epic thread after self-registration

### 3.3 Update documentation consistency
- [ ] **Task 3.3.1**: Update agent-mail.md pre-registration section
  - File: `skills/orchestrator/references/agent-mail.md`
  - Change: "MUST pre-register" → "workers self-register via macro_start_session"
  - AC: Documentation consistent with new protocol

- [ ] **Task 3.3.2**: Remove "already registered" assumption from worker-prompt
  - File: `skills/orchestrator/references/worker-prompt.md`
  - Change: Remove text "Orchestrator has ALREADY registered you"
  - AC: Worker prompt doesn't assume pre-registration

---

## Phase 4: Lazy Skill Loading (MEDIUM RISK)

### 4.1 Add lazy references section
- [ ] **Task 4.1.1**: Add `## Lazy References` to orchestrator SKILL.md
  - File: `skills/orchestrator/SKILL.md`
  - Change: Replace `## References` with `## Lazy References`, add trigger table
  - AC: SKILL.md defines when each reference should load

- [ ] **Task 4.1.2**: Add inline summaries for critical flows
  - File: `skills/orchestrator/SKILL.md`
  - Change: Add compressed 8-phase summary and worker 4-step protocol inline
  - AC: Critical info available without loading workflow.md

### 4.2 Define trigger conditions
- [ ] **Task 4.2.1**: Map phases to reference files
  - File: `skills/orchestrator/SKILL.md`
  - Create mapping:
    - Phase 3 (Initialize) → agent-mail.md
    - Phase 4 (Spawn) → worker-prompt.md
    - Phase 6 (Handle Issues) → agent-coordination.md
  - AC: Each reference has clear trigger condition

### 4.3 Document loader requirements
- [ ] **Task 4.3.1**: Document loader changes needed
  - File: `skills/orchestrator/references/architecture.md`
  - Change: Add section "Lazy Loading Requirements" with host-side changes
  - AC: Requirements documented for Maestro loader update

---

## Phase 5: Verification

### 5.1 Manual testing
- [ ] **Task 5.1.1**: Test Track Assignments fast-path
  - Run `ci` on track with explicit Track Assignments
  - Verify no file scope analysis in output
  - AC: Pre-spawn completes in <4k tokens

- [ ] **Task 5.1.2**: Test preflight skip
  - Run `ci` twice on same track
  - Verify second run skips preflight
  - AC: "Using cached bead state" message shown

- [ ] **Task 5.1.3**: Test worker self-registration
  - Run parallel execution
  - Verify workers join epic thread
  - AC: All workers send completion summary

### 5.2 Regression testing
- [ ] **Task 5.2.1**: Test sequential execution
  - Run `ci` on track without Track Assignments
  - Verify sequential path still works
  - AC: No regression in sequential mode

---

## Track Assignments

| Track | Tasks | File Scope | Depends On |
|-------|-------|------------|------------|
| A | 1.1.*, 1.2.* | `conductor/references/workflows/implement.md` | - |
| B | 2.1.*, 2.2.* | `conductor/references/preflight-beads.md`, `conductor/references/workflows/handoff.md` | - |
| C | 3.1.*, 3.2.*, 3.3.* | `orchestrator/references/workflow.md`, `orchestrator/references/agent-mail.md`, `orchestrator/references/worker-prompt.md` | - |
| D | 4.1.*, 4.2.*, 4.3.* | `orchestrator/SKILL.md`, `orchestrator/references/architecture.md` | C |
| E | 5.1.*, 5.2.* | (testing) | A, B, C, D |

---

## Automated Verification

```bash
# After each phase, run diagnostics
echo "Phase complete - checking token usage..."

# Verify no new lint errors
./scripts/validate-links.sh .

# Verify skill structure
python3 skills/skill-creator/scripts/quick_validate.py skills/orchestrator
python3 skills/skill-creator/scripts/quick_validate.py skills/conductor
```
