---
name: plan-maestro
description: "Universal interview-driven planning for Claude Code, Codex, and Amp Code. Uses subagents only -- no Agent Teams required."
triggers:
  - "/plan:maestro"
  - "$plan:maestro"
metadata:
  short-description: "Cross-platform interview-driven planning"
---

# You Are The Plan:Maestro Orchestrator

## Invocation

- Claude Code: `/plan:maestro <request> [--quick]`
- Codex: `$plan:maestro <request> [--quick]`
- Amp Code: `amp skill run plan:maestro <request> [--quick]`

## Codex Tool Mapping

| Claude Code | Codex | Amp |
|-------------|-------|-----|
| `Task(...)` | `spawn_agent(...)` | subagent |
| `Read` | `exec_command (read-only)` | read_file |
| `Write` | `apply_patch` / `exec_command (write)` | write_file |
| `Bash` | `exec_command` | exec |
| `AskUserQuestion` | `request_user_input` | prompt_user |
| `Glob` | `exec_command: find` | list_files |

## Design Request

`$ARGUMENTS`

---

## Core Principle

Spawn subagents for research. Spawn a single planner subagent that interviews the user directly (via AskUserQuestion) and writes the plan. You save the result. No Agent Teams, no message relay.

---

## Workflow

### Step 1: Mode Detection

Detect mode from `$ARGUMENTS`:
- `--quick` flag present → **Quick mode** (1-2 questions, focused)
- Default → **Full mode** (3-5 questions, thorough)

Derive a short topic slug from `$ARGUMENTS` (kebab-case, max 4 words, strip flags).

### Step 2: Write Handoff File

Write `.maestro/handoff/{topic}.json`:

```json
{
  "topic": "{topic}",
  "status": "designing",
  "skill": "plan-maestro",
  "started": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

Create `.maestro/handoff/` if it does not exist.

### Step 3: Load Priority Context

Read `.maestro/notepad.md` if it exists. Extract any `[P0]` or `[P1]` tagged items and any `## Working Memory` entries from the last 7 days. Inject into the planner prompt as `## Priority Context`.

Read `.maestro/wisdom/` — list any files matching the topic slug and include summaries as `## Prior Wisdom`.

### Step 4: Discover Available Skills

Run:
```bash
find .claude/skills -L -name "SKILL.md" -type f 2>/dev/null
find .agents/skills -L -name "SKILL.md" -type f 2>/dev/null
find ~/.claude/skills -L -name "SKILL.md" -type f 2>/dev/null
```

For each SKILL.md found, extract `name` and `description` from YAML frontmatter. Build a skill summary block. If none found, omit the block.

### Step 5: Spawn Background Research Subagents

**Always spawn explore** (read-only codebase search). In full mode, also spawn oracle (strategic analysis).

Spawn explore as a background Task:

```
Task(
  description: "Codebase research for {topic}",
  prompt: "Research the codebase for: {original $ARGUMENTS}

Find and report:
1. Existing patterns and architecture relevant to this request
2. Files likely to need changes
3. Related tests and testing patterns
4. Similar existing implementations
5. Relevant dependencies and imports

Write your complete findings to: .maestro/drafts/{topic}-research.md

Structure the file as:
# Research Log: {topic}

## Codebase Findings (explore)
{your findings}

Use Glob, Grep, and Read tools. Be thorough but concise.",
  run_in_background: true
)
```

In full mode, also spawn oracle as a background Task:

```
Task(
  description: "Strategic analysis for {topic}",
  prompt: "Analyze this design request strategically: {original $ARGUMENTS}

Read the codebase research from: .maestro/drafts/{topic}-research.md
(If the file does not yet exist, wait up to 30 seconds and retry once.)

Provide:
1. Key architectural tradeoffs
2. Potential risks and pitfalls
3. Recommended approach with justification
4. Suggested task breakdown
5. Edge cases and constraints

Append your analysis to .maestro/drafts/{topic}-research.md under:
## Strategic Analysis (oracle)
{your analysis}",
  run_in_background: true
)
```

### Step 6: Collect Research

Wait for background subagents to complete. Poll `.maestro/drafts/{topic}-research.md` with Read every 10 seconds, up to 60 seconds total. After 60 seconds, read whatever exists and continue.

Read `.maestro/drafts/{topic}-research.md` and pass its content inline to the planner prompt.

### Step 7: Spawn Planner Subagent

Read `.claude/skills/plan-maestro/reference/planner-prompt.md`. Substitute all `{placeholder}` values with actual content, then spawn:

```
Task(
  description: "Interview and plan for {topic}",
  prompt: "{substituted planner prompt}"
)
```

Wait for this Task to complete. The planner writes the completed plan to `.maestro/drafts/{topic}-plan-draft.md` and returns "PLAN WRITTEN".

### Step 8: Read and Present Plan

Read `.maestro/drafts/{topic}-plan-draft.md`.

Display a structured summary to the user:
- Title (first `# ` line)
- Objective (first sentence under `## Objective`)
- Task count and agents (count `- [ ] Task N:` lines, group by `**Agent**:` value)
- Dependency chain (from `## Dependency Chain` section)

Then ask:

```
AskUserQuestion(
  questions: [{
    question: "The plan is ready. How would you like to proceed?",
    header: "Plan Review",
    options: [
      { label: "Approve", description: "Save plan to .maestro/plans/{topic}.md" },
      { label: "Revise", description: "Provide feedback for the planner to revise" },
      { label: "Cancel", description: "Discard the draft and exit" }
    ],
    multiSelect: false
  }]
)
```

**On Approve**: Continue to Step 9.

**On Revise**: Ask the user for specific feedback (AskUserQuestion with free-text). Re-spawn the planner Task with the existing draft content prepended as `## Current Draft` and the feedback as `## Revision Request`. Loop up to 2 times, then proceed to Step 9 regardless.

**On Cancel**: Delete `.maestro/drafts/{topic}-plan-draft.md`. Update handoff `status` to `"cancelled"`. Stop.

### Step 9: Save Plan

Write the plan to its final destination:
```
Write(
  file_path: ".maestro/plans/{topic}.md",
  content: {plan content from draft file}
)
```

Auto-capture design decisions: read the `## Notes` section. If it has content, append up to 5 decisions as timestamped entries to `.maestro/notepad.md` under `## Working Memory`:
```
- [{ISO date}] [plan-maestro:{topic}] {decision}
```
Create `.maestro/notepad.md` if it does not exist.

### Step 10: Update Handoff

Update `.maestro/handoff/{topic}.json`:
```json
{
  "topic": "{topic}",
  "status": "complete",
  "skill": "plan-maestro",
  "started": "{original timestamp}",
  "completed": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

### Step 11: Hand Off

Tell the user:
```
Plan saved to: .maestro/plans/{topic}.md

To execute:
  Claude Code / Codex: /work
  Amp Code:            amp skill run work

/work auto-detects this plan and will suggest it for execution.
```

---

## Anti-Patterns

| Anti-Pattern | Do This Instead |
|--------------|-----------------|
| Using TeamCreate or SendMessage | Use Task (subagents) only |
| Interviewing the user yourself | Spawn the planner subagent -- it owns the interview |
| Researching codebase yourself | Spawn explore/oracle background subagents |
| Skipping the handoff file | Always write .maestro/handoff/ before spawning |
| Polling indefinitely for research | Max 60s wait, then proceed with whatever exists |
| Writing plan draft yourself | Planner subagent writes the draft -- you only save it |
