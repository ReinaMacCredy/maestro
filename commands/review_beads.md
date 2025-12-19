---
description: Review, proofread, and refine filed Beads epics and issues
argument-hint: [optional: specific epic or issue IDs to focus on]
---

# Review and Refine Beads Issues

You are tasked with thoroughly reviewing, proofreading, and polishing the filed Beads epics and issues.

## Step 1: Initialize and Load Current Issues

**CRITICAL**: Always initialize first!

**CLI (Primary):**
```bash
bd onboard
bd list --json
bd ready --json
```

**MCP (Secondary):**
```
init()
ls(status="open")
ls(status="ready")
```

If specific IDs were provided (`$ARGUMENTS`), focus on those with `bd show <id>` or `show(id="bd-X")`.

## Step 2: Get Insights

**CLI (Primary):**
```bash
bd dep cycles --json
bd dep tree <epic-id> --json
```

**MCP (Secondary):**
```
bv_insights()      # Bottlenecks, keystones, cycles
bv_priority()      # Priority recommendations
bv_plan()          # Parallel execution tracks
```

## Step 3: Systematic Review Checklist

For EACH issue (use `bd show <id>` or `show(id="bd-X")` to get details):

### Clarity
- [ ] Title is action-oriented and specific
- [ ] Description is clear and unambiguous
- [ ] A developer unfamiliar with the codebase could understand the task
- [ ] No jargon without explanation

### Completeness
- [ ] Acceptance criteria are defined and testable
- [ ] Technical implementation hints are provided where helpful
- [ ] Relevant file paths or modules are mentioned
- [ ] Edge cases and error handling are considered

### Dependencies
- [ ] All blocking dependencies are linked
- [ ] No circular dependencies exist
- [ ] Dependencies are minimal (not over-constrained)
- [ ] Ready issues exist for parallel work

### Scope
- [ ] Issue is appropriately sized (not too large)
- [ ] Large issues are broken into subtasks
- [ ] No duplicate or overlapping issues

### Priority
- [ ] Priority reflects actual importance
- [ ] Critical path items are prioritized correctly
- [ ] Dependencies and priorities align

### Role Assignment
- [ ] Tasks have appropriate role tags (`fe`, `be`, `mobile`, `devops`, `qa`)
- [ ] Role tags match the work type

## Step 4: Common Issues to Fix

Watch for and correct:

1. **Vague titles**: "Fix bug" → "Fix null pointer in UserService.getProfile when user not found"
2. **Missing context**: Add relevant file paths, function names, or module references
3. **Implicit knowledge**: Make assumptions explicit
4. **Missing acceptance criteria**: Add "Done when..." statements
5. **Over-coupling**: Break dependencies that aren't strictly necessary
6. **Under-specified**: Add technical notes for complex tasks
7. **Duplicate work**: Merge or link related issues
8. **Missing dependencies**: Link issues that should be sequenced
9. **Wrong priorities**: Adjust based on critical path analysis
10. **Missing role tags**: Add tags for role assignment

## Step 5: Update Issues

**CLI (Primary):**
```bash
bd update <id> --title "Improved title" --json
bd update <id> --priority <new-priority> --json
bd update <id> --description "New description" --json
bd dep add <issue-id> <dependency-id> --json
bd dep remove <issue-id> <dependency-id> --json
```

**MCP (Secondary):**
```
add(title="...", typ="...", pri=..., desc="...", tags=["..."], deps=["..."])
assign(id="bd-X", role="fe|be|mobile|devops|qa")
```

For major rewrites, close and recreate:
```bash
bd close <id> --reason "Replaced by <new-id>" --json
bd create "Better title" -t <type> -p <priority> --deps bd-<id> --json
```

## Step 6: Validate the Graph

**CLI (Primary):**
```bash
bd list --json
bd list --status open --json
bd ready --json
bd dep cycles --json
bd dep tree <epic-id> --json
```

**MCP (Secondary):**
```
ls(status="open")
ls(status="ready")
bv_insights()
bv_plan()
```

Check:
- No orphaned issues (except entry points)
- No circular dependencies
- Critical path is clear
- Parallelization opportunities are preserved

## Step 7: Final Quality Gate

Before completing, ensure:

1. **Readability**: Any developer can pick up any ready issue
2. **Traceability**: Issues link to epics, epics link to the plan
3. **Testability**: Each issue has clear "done" criteria
4. **Parallelism**: Multiple issues can be worked simultaneously
5. **Completeness**: No gaps in the plan coverage
6. **Role Coverage**: All tasks have appropriate role assignments

## Output Format

Provide a review report:

### Summary
- Total issues reviewed: X
- Issues updated: Y
- Issues created: Z
- Issues closed/merged: W

### Changes Made
- List significant updates with rationale

### Graph Analysis
- Bottlenecks identified
- Priority recommendations

### Remaining Concerns
- Any issues that need user input
- Ambiguities that couldn't be resolved

### Ready for Implementation
- List of ready issues
- Parallel tracks available
- Suggested execution order for optimal flow

## Iteration Tracking

You may iterate on refinements up to 5 times if asked. Track iterations:

- Iteration 1: Initial review pass
- Iteration 2-5: Deeper refinements based on feedback

After 5 iterations, respond: "I don't think we can do much better than this. The issues are thoroughly reviewed, well-documented, and ready for workers to implement."
