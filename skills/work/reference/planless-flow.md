# Planless Work Flow

When `/work` is invoked with a description instead of a plan name (detected in Step 1), follow this flow instead of the plan-based workflow.

---

## Step P1: Analyze description

Parse the user's description to understand intent:

1. Extract the core action (what to do)
2. Identify target files, components, or modules (if mentioned)
3. Determine scope and complexity

## Step P2: Generate task breakdown

Generate 1-5 atomic tasks from the description. For each task, determine:

- **Subject**: Short, imperative title
- **Agent**: `kraken` (TDD, new features, multi-file changes) or `spark` (quick fixes, single-file)
- **Acceptance criteria**: Objectively verifiable outcomes
- **Files**: Target file paths (search the codebase if not specified)

Keep the breakdown minimal — fewer well-scoped tasks over many granular ones.

## Step P3: Confirm with user

Present the task breakdown with agent assignments and acceptance criteria.

Ask: **"Execute these tasks?"** with options:
- **Execute** → proceed to Step P4
- **Revise** → ask for a new description, repeat from P1
- **Cancel** → stop

## Step P4: Join main workflow

After confirmation, rejoin the plan-based workflow:

1. Discover skills (see `reference/skill-injection.md`)
2. Write handoff file with `"status": "executing"`
3. Execute tasks using Step 4 from the main workflow (delegate → verify → commit)
4. Continue through Steps 5-7 (quality gates → wrap up → report)

## Skipped steps in planless mode

| Step | Reason |
|------|--------|
| Plan validation (Step 2) | No plan file to validate |
| Worktree isolation (Step 3) | Too heavyweight for ad-hoc work |
| Plan archival (Step 6) | No plan file to archive |
| Plan checkbox annotation | No plan file to update |

All other steps (execution, verification, wisdom extraction, reporting) proceed normally.
