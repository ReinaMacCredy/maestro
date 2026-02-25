# Implementation Plan: Add Sub Agent Parallels to maestro:implement

> Track: sub_agent_parallels_20260225
> Type: feature
> Created: 2026-02-25

## Phase 1: Parallel Mode Reference Doc & SKILL.md Updates

### Task 1.1: Create reference/parallel-mode.md
- [x] Write the parallel mode reference document covering: mode activation, task independence analysis, wave-based execution protocol, sub-agent spawning, worktree isolation, result collection, verification and merge, conflict detection, sequential fallback, and error handling
- [x] Include concrete examples of plan.md analysis showing how independent tasks are identified
- [x] Document the sub-agent prompt template (what context sub-agents receive, what they can/cannot do)
- [x] Document the wave execution loop: analyze --> spawn --> wait --> verify --> merge --> commit --> next wave

### Task 1.2: Update SKILL.md with --parallel mode
- [x] Add `--parallel` to the argument-hint in frontmatter
- [x] Add `--parallel` to the Arguments section with description
- [x] Add Step 1 mode detection for `--parallel` flag
- [x] Add new section between single-agent and team mode: `## Parallel Mode (--parallel)` referencing `reference/parallel-mode.md`
- [x] Update the description frontmatter to reflect the three modes

### Phase 1 Completion Verification
- [ ] Verify reference/parallel-mode.md is self-contained and covers all spec requirements
- [ ] Verify SKILL.md correctly routes --parallel to the new reference doc
- [ ] Verify no existing single-agent or team-mode references are broken
- [ ] Run `bash -n scripts/*.sh` to ensure no shell scripts were broken
- [ ] Maestro - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Testing & Validation

### Task 2.1: Update test-hooks.sh with parallel mode test cases
- [x] Add test case verifying --parallel flag is recognized in SKILL.md
- [x] Add test case verifying reference/parallel-mode.md exists and is non-empty
- [x] Add test case verifying SKILL.md frontmatter includes --parallel in argument-hint

### Task 2.2: Shell syntax and version sync validation
- [ ] Run `bash -n scripts/*.sh` to verify no syntax errors
- [ ] Run version sync check between plugin.json and marketplace.json
- [ ] Verify all markdown files have valid structure (no broken links within the skill)

### Phase 2 Completion Verification
- [ ] All test cases pass
- [ ] No shell syntax errors
- [ ] Version metadata is consistent
- [ ] Maestro - User Manual Verification 'Phase 2' (Protocol in workflow.md)
