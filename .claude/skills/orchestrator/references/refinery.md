# Refinery Role

Post-completion review and quality verification before merge.

## Overview

The Refinery is a specialized review phase that runs after all workers complete their tracks. It ensures code quality, validates acceptance criteria, and gates merges.

```
Workers Complete → Refinery Review → Quality Gate → Merge
```

## Post-Completion Review Flow

After Phase 7 epic completion:

```python
# 1. Orchestrator spawns Refinery agent
Task(
  description="Refinery review for epic <epic-id>",
  prompt="""
You are the Refinery agent for epic <epic-id>.

## Your Mission
1. Run `rb` to review all completed beads
2. Validate acceptance criteria from spec.md
3. Check code quality and consistency
4. Identify any gaps or issues
5. Gate the merge

## Protocol
1. Call macro_start_session() first
2. Execute review steps
3. Send review summary to orchestrator
4. Release reservations
"""
)
```

## Integration with rb (Review Beads)

Refinery uses `rb` command for systematic review:

```bash
# Review all beads in epic
rb --epic <epic-id>

# Output structure:
# ✓ my-workflow:3-zyci.1.1 - Completed, verified
# ✓ my-workflow:3-zyci.1.2 - Completed, verified
# ⚠ my-workflow:3-zyci.2.1 - Completed, needs attention
# ✓ my-workflow:3-zyci.3.1 - Completed, verified
```

### rb Review Criteria

| Check | Description |
|-------|-------------|
| Bead closed | All beads have `completed` status |
| Notes present | Completion notes document what was done |
| Files exist | Referenced files were created/modified |
| Tests pass | Related tests are passing |
| Spec match | Implementation matches spec acceptance criteria |

## Quality Gate Checks

Refinery enforces quality gates before merge:

### Gate 1: Bead Completion
```python
# Verify all beads closed
open_beads = bash(f"bd list --parent={epic_id} --status=open --json | jq 'length'")
assert open_beads == "0", f"Lingering beads: {open_beads}"
```

### Gate 2: Spec Validation
```python
# Load spec and verify acceptance criteria
spec = Read(f"conductor/tracks/{track_id}/spec.md")
acceptance_criteria = parse_acceptance_criteria(spec)

for criterion in acceptance_criteria:
    if not verify_criterion(criterion):
        report_gap(criterion)
```

### Gate 3: Code Quality
```python
# Run linters and type checks
lint_result = bash("pnpm run lint")
type_result = bash("pnpm run typecheck")

if lint_result.exit_code != 0 or type_result.exit_code != 0:
    report_quality_issue(lint_result, type_result)
```

### Gate 4: Test Coverage
```python
# Verify tests pass
test_result = bash("pnpm run test")

if test_result.exit_code != 0:
    report_test_failures(test_result)
```

## Refinery Report Format

```python
send_message(
  project_key="<path>",
  sender_name="<refinery_agent>",
  to=["<orchestrator>"],
  thread_id="<epic-id>",
  subject="[REFINERY] Review Complete",
  body_md="""
## Refinery Review: <epic-id>

### Summary
- **Beads Reviewed**: 26
- **Quality Score**: 94%
- **Gate Status**: PASSED ✓

### Detailed Review

#### Gate 1: Bead Completion ✓
All 26 beads properly closed with notes.

#### Gate 2: Spec Validation ✓
All 12 acceptance criteria verified.

#### Gate 3: Code Quality ✓
- Lint: 0 errors, 3 warnings (acceptable)
- Types: 0 errors

#### Gate 4: Tests ✓
- 47 tests passed
- 0 failed
- Coverage: 82%

### Recommendations
1. Consider adding tests for edge case X
2. Minor: Inconsistent naming in file Y

### Merge Decision
**APPROVED** for merge to main.
"""
)
```

## Merge Integration

After Refinery approval:

```python
# 1. Orchestrator receives approval
approval = fetch_inbox(filter="[REFINERY]")[0]

if "APPROVED" in approval.body:
    # 2. Proceed with merge
    print("✓ Refinery approved - proceeding with merge")
    
    # 3. Create PR or merge directly
    if requires_pr:
        bash("gh pr create --title 'Epic: <title>' --body '<summary>'")
    else:
        bash("git checkout main && git merge <branch>")
    
    # 4. Archive track
    bash(f"bd close {epic_id} --reason completed")
else:
    # 5. Handle rejection
    print("⚠️ Refinery flagged issues - review required")
    issues = parse_issues(approval.body)
    display_issues(issues)
```

## Rejection Handling

When Refinery rejects:

```python
# 1. Parse rejection reasons
issues = parse_issues(refinery_report)

# 2. Categorize by severity
blockers = [i for i in issues if i.severity == "blocker"]
warnings = [i for i in issues if i.severity == "warning"]

# 3. For blockers: spawn fix agents
for blocker in blockers:
    Task(
      description=f"Fix: {blocker.description}",
      prompt=f"""
Fix the following issue flagged by Refinery:

{blocker.details}

File: {blocker.file}
Line: {blocker.line}
"""
    )

# 4. Re-run Refinery after fixes
```

## Anti-Patterns

| ❌ Don't | ✅ Do |
|----------|-------|
| Skip Refinery for "simple" changes | Always run Refinery for epics |
| Ignore warnings | Document reason for accepted warnings |
| Merge without test verification | Require passing tests |
| Override gates manually | Fix issues and re-run |

## Related

- [workflow.md](workflow.md) - Phase 7 completion and rb spawn
- [summary-protocol.md](summary-protocol.md) - Report format standards
- [../beads/SKILL.md](../../beads/SKILL.md) - `rb` command documentation
