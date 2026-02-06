# /design Command Validation Test Plan

## Objective
Validate that the `/design` command workflow functions correctly end-to-end, from team creation through plan approval and cleanup.

## Scope
**In**:
- Team creation via `Teammate(spawnTeam)`
- Prometheus spawning in plan mode
- Interview flow (AskUserQuestion tool usage)
- Plan approval/rejection flow (plan_approval_request/response)
- Plan persistence to `.maestro/plans/`
- Handoff file lifecycle (designing → complete)
- Cleanup (shutdown + team cleanup)

**Out**:
- `/work` command execution
- Explore/oracle subagent internals (tested via Prometheus behavior)
- Wisdom injection (optional feature)

## Tasks

### Phase 1: Pre-flight Setup
- [ ] Task 1: Clean existing state with `/reset` to ensure fresh environment
- [ ] Task 2: Verify `.maestro/` directory structure exists (drafts, plans, handoff, wisdom)
- [ ] Task 3: Confirm Agent Teams is enabled (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: "1"` in settings)

### Phase 2: Team Creation (Step 1-2 of design.md)
- [ ] Task 4: Run `/design test-validation` and observe team creation
- [ ] Task 5: Verify handoff file created at `.maestro/handoff/test-validation.json` with `status: "designing"`
- [ ] Task 6: Verify team directory exists at `~/.claude/teams/design-test-validation/`

### Phase 3: Prometheus Spawn & Interview (Steps 3-4)
- [ ] Task 7: Confirm Prometheus spawned (check for teammate message in orchestrator)
- [ ] Task 8: Verify Prometheus uses `AskUserQuestion` tool (not plain text questions)
- [ ] Task 9: Respond to interview questions and confirm responses are received

### Phase 4: Plan Generation & Approval (Steps 5-7)
- [ ] Task 10: Wait for `plan_approval_request` message from Prometheus
- [ ] Task 11: Read generated plan from plan-mode file
- [ ] Task 12: Test REJECT flow: send rejection with feedback, verify Prometheus revises
- [ ] Task 13: Test APPROVE flow: send approval, verify plan saved to `.maestro/plans/test-validation.md`

### Phase 5: Cleanup & Handoff (Steps 8-10)
- [ ] Task 14: Verify handoff file updated to `status: "complete"`
- [ ] Task 15: Confirm shutdown_request sent to Prometheus
- [ ] Task 16: Confirm team cleanup executed
- [ ] Task 17: Verify no orphaned team directories remain

### Phase 6: Edge Cases
- [ ] Task 18: Test quick mode (`/design --quick simple-task`)
- [ ] Task 19: Test error case: run `/design` when team already exists (should suggest `/reset`)
- [ ] Task 20: Test cancel flow: reject plan and cancel, verify cleanup without save

## Verification

Each phase has explicit checkpoints:

| Phase | Verification Method | Pass Criteria |
|-------|---------------------|---------------|
| 1 | `ls .maestro/` | All subdirectories exist, no stale handoffs |
| 2 | `cat .maestro/handoff/test-validation.json` | File exists, status = "designing" |
| 3 | Observe Prometheus messages | Uses AskUserQuestion, not plain text |
| 4 | `cat .maestro/plans/test-validation.md` | Plan file exists with correct format |
| 5 | `ls ~/.claude/teams/` | No orphaned team directories |
| 6 | Run both modes | Quick mode completes faster with fewer questions |

## Notes

**Technical Decisions:**
- This is a manual test plan (no automation) because the workflow involves interactive user prompts
- Each phase builds on the previous; if Phase 2 fails, skip to debugging before proceeding
- The test uses a distinct topic slug (`test-validation`) to avoid conflicts with real plans

**Research Findings:**
- Design workflow has 10 steps defined in `/Users/reinamaccredy/Code/maestro/.claude/commands/design.md`
- Prometheus agent at `/Users/reinamaccredy/Code/maestro/.claude/agents/prometheus.md` has a Stop hook to enforce AskUserQuestion usage
- Reset command at `/Users/reinamaccredy/Code/maestro/.claude/commands/reset.md` cleans drafts, handoffs, teams, and tasks
- Current handoff file exists at `.maestro/handoff/test.json` with `status: "designing"` (from this session)

**Known Constraints:**
- Agent Teams must be enabled in settings
- Teammate tool must be available
- Plan mode requires ExitPlanMode to trigger approval flow
