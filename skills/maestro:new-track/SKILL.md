---
name: maestro:new-track
description: "Create a new feature/bug track with spec and implementation plan. Interactive interview generates requirements spec, then phased TDD plan. Use when starting work on a new feature, bug fix, or chore."
argument-hint: "<track description>"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# New Track -- Specification & Planning

> Adapted from [Conductor](https://github.com/gemini-cli-extensions/conductor) for Claude Code.

CRITICAL: You must validate the success of every tool call. If any tool call fails, halt immediately, announce the failure, and await instructions.

When using AskUserQuestion, immediately call the tool -- do not repeat the question in plain text before the tool call.

Create a new development track with a requirements specification and phased implementation plan. Every feature, bug fix, or chore gets its own track.

## Arguments

`$ARGUMENTS`

The track description. Examples:
- `"Add dark mode support"`
- `"Fix login timeout on slow connections"`
- `"Refactor database connection pooling"`

---

## Step 1: Validate Prerequisites

Check that `/maestro:setup` has been run:

```
Glob(pattern: ".maestro/context/product.md")
```

If no context files exist:
- Report: "Project context not found. Run `/maestro:setup` first."
- Stop.

Check that tracks registry exists:
```
Read(file_path: ".maestro/tracks.md")
```

If missing, create it:
```markdown
# Tracks Registry

> Managed by Maestro. Do not edit manually.
> Status markers: `[ ]` New | `[~]` In Progress | `[x]` Complete

---
```

## Step 2: Parse Input

Extract the track description from `$ARGUMENTS`.

If no arguments provided, ask:
```
AskUserQuestion(
  questions: [{
    question: "What feature, bug fix, or chore would you like to track?",
    header: "Track",
    options: [
      { label: "Feature", description: "New functionality to build" },
      { label: "Bug fix", description: "Something broken to fix" },
      { label: "Chore", description: "Refactoring, cleanup, or maintenance" }
    ],
    multiSelect: false
  }]
)
```

Then ask for the description as a follow-up.

## Step 3: Generate Track ID

Format: `{shortname}_{YYYYMMDD}`

Rules:
- Extract a short name from the description (2-4 words, snake_case)
- Append today's date: `YYYYMMDD`
- Example: `"Add dark mode support"` --> `dark_mode_20260225`

## Step 4: Duplicate Check

```
Glob(pattern: ".maestro/tracks/*")
```

Scan existing track directories. If any directory starts with the same short name prefix:
- Warn: "A track with a similar name already exists: `{existing_track_id}`"
- Ask: Continue anyway or choose a different name?

## Step 5: Create Track Directory

```bash
mkdir -p .maestro/tracks/{track_id}
```

## Step 6: Auto-Infer Track Type

Analyze the description to determine if it is a `feature`, `bug`, or `chore`. Do NOT ask the user to classify it.

Inference rules:
- **feature**: adds new behavior, capability, or user-visible functionality (keywords: add, build, create, implement, support, introduce)
- **bug**: fixes incorrect, broken, or unexpected behavior (keywords: fix, broken, error, crash, incorrect, regression, timeout, fail)
- **chore**: improves internals without changing external behavior (keywords: refactor, cleanup, migrate, upgrade, rename, reorganize, extract)

If the description is ambiguous (matches multiple types or no clear keywords), confirm with:
```
AskUserQuestion(
  questions: [{
    question: "I inferred this as '{inferred_type}' -- does that look right?",
    header: "Confirm Type",
    options: [
      { label: "Feature", description: "New functionality or capability" },
      { label: "Bug", description: "Fix for broken behavior" },
      { label: "Chore", description: "Refactoring, maintenance, or tech debt" }
    ],
    multiSelect: false
  }]
)
```

## Step 7: Specification Interview

Generate a requirements specification through interactive questioning. Batch up to 4 questions in a single AskUserQuestion call where they are independent of each other.

### For Features (one batched call, 4 questions):

```
AskUserQuestion(
  questions: [
    {
      question: "What should this feature do? Describe the core behavior and expected outcomes.",
      header: "Requirements"
    },
    {
      question: "How will users interact with this feature?",
      header: "Interaction",
      options: [
        { label: "UI component", description: "Visual element users see and interact with" },
        { label: "API endpoint", description: "Programmatic interface" },
        { label: "CLI command", description: "Terminal command or flag" },
        { label: "Background process", description: "No direct user interaction" }
      ],
      multiSelect: false
    },
    {
      question: "Any constraints or non-functional requirements? (performance, security, compatibility)",
      header: "Constraints",
      options: [
        { label: "No special constraints", description: "Standard quality expectations" },
        { label: "Performance-critical", description: "Must meet specific latency/throughput targets" },
        { label: "Security-sensitive", description: "Handles auth, PII, or financial data" },
        { label: "Let me specify", description: "Type your constraints" }
      ],
      multiSelect: true
    },
    {
      question: "Any known edge cases or error scenarios to handle?",
      header: "Edge Cases",
      options: [
        { label: "I'll list them", description: "Type known edge cases" },
        { label: "Infer from requirements", description: "Generate edge cases from the spec" }
      ],
      multiSelect: false
    }
  ]
)
```

### For Bugs (one batched call, 3 questions):

```
AskUserQuestion(
  questions: [
    {
      question: "What is happening? Provide steps to reproduce.",
      header: "Observed Behavior"
    },
    {
      question: "What should happen instead?",
      header: "Expected Behavior"
    },
    {
      question: "How critical is this? Which users or flows are affected?",
      header: "Impact",
      options: [
        { label: "Blocker", description: "Core flow broken, no workaround" },
        { label: "High", description: "Significant degradation, workaround exists" },
        { label: "Medium", description: "Noticeable issue, limited impact" },
        { label: "Low", description: "Minor or cosmetic" }
      ],
      multiSelect: false
    }
  ]
)
```

### For Chores (one batched call, 2 questions):

```
AskUserQuestion(
  questions: [
    {
      question: "What needs to change and why?",
      header: "Scope"
    },
    {
      question: "Any backward compatibility requirements?",
      header: "Constraints",
      options: [
        { label: "Must be backward compatible", description: "No breaking changes to public API" },
        { label: "Breaking changes acceptable", description: "Semver major bump is fine" },
        { label: "Internal only", description: "No public surface affected" }
      ],
      multiSelect: false
    }
  ]
)
```

## Step 8: Draft Specification

Compose a spec document from the interview answers.

```markdown
# Specification: {track description}

## Overview
{one-paragraph summary}

## Type
{feature | bug | chore}

## Requirements

### Functional Requirements
{from Q1}

### User Interaction
{from Q2}

### Non-Functional Requirements
{from Q3}

## Edge Cases & Error Handling
{from Q4 or inferred}

## Out of Scope
{explicitly list what this track does NOT cover}

## Acceptance Criteria
- [ ] {criterion 1}
- [ ] {criterion 2}
- [ ] {criterion 3}
```

Present the full draft to the user for approval by embedding the entire spec content directly in the question field:

```
AskUserQuestion(
  questions: [{
    question: "Here is the drafted specification -- does it look correct?\n\n---\n{full spec content}\n---",
    header: "Approve Spec",
    options: [
      { label: "Approved", description: "Spec is ready, generate the plan" },
      { label: "Needs revision", description: "I'll tell you what to change" }
    ],
    multiSelect: false
  }]
)
```

If revision needed: ask what to change, update, and re-present with the full updated content embedded. Max 3 revision loops.

Write approved spec to `.maestro/tracks/{track_id}/spec.md`.

## Step 9: Generate Implementation Plan

Read project context for informed planning:
```
Read(file_path: ".maestro/context/workflow.md")
Read(file_path: ".maestro/context/tech-stack.md")
Read(file_path: ".maestro/context/guidelines.md")
```

Use the plan template from `reference/plan-template.md`.

### Plan Structure

```markdown
# Implementation Plan: {track description}

> Track: {track_id}
> Type: {feature | bug | chore}
> Created: {YYYY-MM-DD}

## Phase 1: {phase title}

### Task 1.1: {task title}
- [ ] **Sub-task**: {description}
- [ ] **Sub-task**: {description}

{For TDD methodology, inject per task:}
- [ ] Write failing tests for {task}
- [ ] Implement {task} to pass tests
- [ ] Refactor {task} (if applicable)

### Task 1.2: {next task}
...

### Phase 1 Completion Verification
- [ ] Run test suite for Phase 1 scope
- [ ] Verify coverage meets threshold ({from workflow.md})
- [ ] Manual verification: {user-facing check}
- [ ] Maestro - User Manual Verification 'Phase 1: {phase title}' (Protocol in workflow.md)

## Phase 2: {next phase}
...
```

### Plan Rules

1. **Phases** group related tasks into logical milestones (1-4 phases typical)
2. **Tasks** are atomic work items completable in one session
3. **Sub-tasks** are granular steps within a task
4. **TDD injection**: If workflow.md specifies TDD, every implementation task gets:
   - `Write failing tests for {task}` (Red)
   - `Implement {task} to pass tests` (Green)
   - `Refactor {task}` (optional cleanup)
5. **Phase Completion Verification**: Every phase ends with a verification block containing:
   - Test coverage check
   - Test suite execution
   - Manual verification steps
   - `- [ ] Maestro - User Manual Verification '{Phase Name}' (Protocol in workflow.md)`
6. **All items** get `[ ]` checkboxes for progress tracking
7. Tasks MUST be ordered by dependency (no forward references)

Present the full plan for approval by embedding the entire plan content directly in the question field (same pattern as spec approval -- no "I'll show it above"). Max 3 revision loops.

Write approved plan to `.maestro/tracks/{track_id}/plan.md`.

## Step 10: Write Metadata

```json
{
  "track_id": "{track_id}",
  "type": "{feature | bug | chore}",
  "status": "new",
  "description": "{track description}",
  "created_at": "{ISO 8601 timestamp}",
  "updated_at": "{ISO 8601 timestamp}",
  "phases": {phase_count},
  "tasks": {task_count}
}
```

Write to `.maestro/tracks/{track_id}/metadata.json`.

## Step 11: Write Track Index

```markdown
# Track: {track description}

> ID: {track_id}
> Type: {type}
> Status: New
> Created: {date}

## Files
- [Specification](./spec.md)
- [Implementation Plan](./plan.md)
- [Metadata](./metadata.json)

## Quick Links
- Registry: [tracks.md](../../tracks.md)
- Implement: `/maestro:implement {track_id}`
- Status: `/maestro:status`
```

Write to `.maestro/tracks/{track_id}/index.md`.

## Step 12: Update Tracks Registry

Append to `.maestro/tracks.md`:

```markdown
---
- [ ] **Track: {track description}**
  *Type: {type} | ID: [{track_id}](./tracks/{track_id}/)*
```

## Step 13: Commit

```bash
git add .maestro/tracks/{track_id} .maestro/tracks.md
git commit -m "chore(maestro:new-track): add track {track_id}"
```

## Step 14: Summary

```
## Track Created

**{track description}**
- ID: `{track_id}`
- Type: {type}
- Phases: {count}
- Tasks: {count}

**Files**:
- `.maestro/tracks/{track_id}/spec.md`
- `.maestro/tracks/{track_id}/plan.md`

**Next**: `/maestro:implement {track_id}`
```
