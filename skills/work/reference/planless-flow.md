# Planless Work Flow

When `/work` is invoked with a description instead of a plan name (detected in Step 1), follow this flow instead of the plan-based workflow.

### Step P1: Analyze Description

Parse the user's description to understand intent:

1. Extract the core action (what to do)
2. Identify target files, components, or modules (if mentioned)
3. Determine scope and complexity

Store the description for use in subsequent steps.

### Step P2: Generate Task Breakdown

Generate 1-5 atomic tasks from the description. For each task, determine:

- **Subject**: Short, imperative title
- **Agent**: `kraken` (TDD, new features, multi-file changes) or `spark` (quick fixes, single-file changes)
- **Acceptance criteria**: Objectively verifiable outcomes
- **Files**: Target file paths (use Glob/Grep to find if not specified in the description)

Use the same task format as plan-based tasks. Keep the breakdown minimal — prefer fewer, well-scoped tasks over many granular ones.

### Step P3: Confirm with User

Present the generated task breakdown for user approval. Show each task with its agent assignment and acceptance criteria before asking.

```
DECIDE(
  question: "Here's the task breakdown. How would you like to proceed?",
  options: [
    {label: "Execute", description: "Proceed with these tasks"},
    {label: "Revise", description: "Let me re-describe what I want"},
    {label: "Cancel", description: "Stop without executing"}
  ],
  blocking: true,
  default: "Cancel"
)
```

**On Execute** → Proceed to Step P4.
**On Revise** → Use `prompt.chat` to ask the user for a new description, then repeat from Step P1.
**On Cancel** → Stop execution.

### Step P4: Join Main Workflow

After user confirms, rejoin the plan-based workflow:

1. **Create tasks** (same as Step 3) — convert the generated breakdown into shared tasks with dependencies
2. **Discover skills** (same as Step 3.5) — scan for skills that can provide guidance
3. **Proceed to Step 2** (Create Team) and continue through Steps 2 → 4 → 5 → 6 → 7 → 8 → 9

### Skipped Steps in Planless Mode

The following plan-based steps are skipped when running in planless mode:

| Step | Reason |
|------|--------|
| Step 1.5 (Validate & Confirm) | No plan file to validate |
| Step 1.7 (Worktree Isolation) | Too heavyweight for ad-hoc work |
| Step 8.5 (Archive Plan) | No plan file to archive |

All other steps (team creation, task execution, verification, wisdom extraction, cleanup, reporting) proceed normally.

### Wisdom File Naming in Planless Mode

When extracting wisdom (Step 7), derive the file slug from the first 5 words of the user's description:

- `/work add retry logic to api client` → `.maestro/wisdom/add-retry-logic-to-api.md`
- `/work fix login page redirect bug` → `.maestro/wisdom/fix-login-page-redirect-bug.md`

Strip articles ("a", "an", "the") and limit to 5 significant words. Use hyphens as separators.
