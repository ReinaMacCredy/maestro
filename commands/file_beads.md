---
description: File detailed Beads epics and issues from a plan
argument-hint: <plan-description-or-context>
---

# File Beads Epics and Issues from Plan

You are tasked with converting a plan into a comprehensive set of Beads epics and issues.

## Step 1: Initialize

**CRITICAL**: Always initialize first!

**CLI (Primary):**
```bash
bd onboard
```

**MCP (Secondary):**
```
init(leader=true)
```

## Step 2: Understand the Plan

Review the plan context provided: `$ARGUMENTS`

If no specific plan is provided, ask the user to share the plan or point to a planning document (check `history/` directory for recent plans).

## Step 3: Analyze and Structure

Before filing any issues, analyze the plan for:

1. **Major workstreams** - These become epics
2. **Individual tasks** - These become issues under epics
3. **Dependencies** - What must complete before other work can start?
4. **Parallelization opportunities** - What can be worked on simultaneously?
5. **Technical risks** - What needs spikes or investigation first?

## Step 4: File Epics First

Create epics for major workstreams:

**CLI (Primary):**
```bash
bd create "Epic: <title>" -t epic -p <priority> -d "<acceptance criteria>" --json
```

**MCP (Secondary):**
```
add(title="Epic: <title>", typ="epic", pri=<priority>, desc="<acceptance criteria>")
```

Epics should:
- Have clear, descriptive titles
- Include acceptance criteria in the description
- Be scoped to deliverable milestones

## Step 5: File Detailed Issues

For each epic, create child issues:

**CLI (Primary):**
```bash
bd create "<task title>" -t <type> -p <priority> -d "<description>" --tags fe,be --deps bd-<id> --json
```

**MCP (Secondary):**
```
add(
    title="<action-oriented title>",
    typ="task|feature|bug",
    pri=<priority>,
    desc="<detailed description with acceptance criteria>",
    tags=["fe"|"be"|"mobile"|"devops"|"qa"],
    deps=["bd-<parent-epic-id>"]
)
```

Each issue MUST include:
- **Clear title** - Action-oriented (e.g., "Implement X", "Add Y", "Configure Z")
- **Detailed description** - What exactly needs to be done + acceptance criteria
- **Tags** - Role assignment (`fe`, `be`, `mobile`, `devops`, `qa`)
- **Dependencies** - Link to blocking issues

## Step 6: Issue Types

| Type | Use For | Example |
|------|---------|---------|
| `task` | General work (default) | "Refactor auth module" |
| `bug` | Something broken | "Login fails on Safari" |
| `feature` | New functionality | "Add OAuth2 support" |
| `epic` | Large work with sub-tasks | "User authentication system" |
| `chore` | Maintenance | "Update dependencies" |

## Step 7: Set Priorities

| Priority | Meaning |
|----------|---------|
| **0** | Critical - Production down, security breach |
| **1** | High - Blocking other work |
| **2** | Medium - Normal priority (default) |
| **3** | Low - Nice to have |
| **4** | Backlog - Future consideration |

## Step 8: Verify the Graph

After filing all issues, verify:

**CLI (Primary):**
```bash
bd list --json
bd ready --json
bd dep cycles --json
```

**MCP (Secondary):**
```
ls(status="open")
ls(status="ready")
bv_plan()
bv_insights()
```

Verify:
- All epics have child issues
- Dependencies form a valid DAG (no cycles)
- Ready work exists (some issues have no blockers)
- Priorities align with execution order

## Output Format

After completing, provide:

1. Summary of epics created
2. Summary of issues per epic with role assignments
3. Dependency graph overview (what unblocks what)
4. Suggested starting points (ready issues)
5. Parallelization opportunities
