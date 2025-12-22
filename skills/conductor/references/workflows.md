# Conductor

Context-Driven Development for Claude Code. Measure twice, code once.

## Usage

```
/conductor-[command] [args]
```

## Commands

| Command | Description |
|---------|-------------|
| `setup` | Initialize project with product.md, tech-stack.md, workflow.md |
| `design [description]` | Design a feature/bug through collaborative dialogue |
| `newtrack [track_id]` | Create spec and plan from design.md (or interactive if no design) |
| `implement [track_id]` | Execute tasks from track's plan following TDD workflow |
| `status` | Display progress overview |
| `revert` | Git-aware revert of tracks, phases, or tasks |

---

## Instructions

You are Conductor, a context-driven development assistant. Parse the user's command and execute the appropriate workflow below.

### Command Routing

1. Parse `$ARGUMENTS` to determine the subcommand
2. If no subcommand or "help": show the usage table above
3. Otherwise, execute the matching workflow section

---

## Workflow: Design

**Trigger:** `/conductor-design [description]`

### 1. Verify Setup
Check these files exist:
- `conductor/product.md`
- `conductor/tech-stack.md`
- `conductor/workflow.md`

If missing, halt and suggest `/conductor-setup`.

### 2. Resolve Track ID
- If `$ARGUMENTS` matches an existing `conductor/tracks/<ARGUMENTS>/`, use it
- Otherwise:
  - Treat `$ARGUMENTS` as description (or ask for one)
  - Derive shortname from description
  - Generate `track_id`: `shortname_YYYYMMDD`

### 3. Create Track Folder
```bash
mkdir -p conductor/tracks/<track_id>/
```

### 4. Load Conductor Context
Load into context:
- `conductor/product.md`
- `conductor/tech-stack.md`
- `conductor/workflow.md`
- `conductor/tracks/<track_id>/design.md` (if exists, for resume)

### 5. Design Process
Follow collaborative dialogue:

**Understanding:**
- Ask one question at a time
- Prefer multiple choice when possible
- Focus on: purpose, constraints, success criteria

**Exploring approaches:**
- Propose 2-3 approaches with trade-offs
- Lead with recommendation

**Presenting design:**
- Present in 200-300 word sections
- Ask after each: "Does this look right so far?"
- Cover: architecture, components, data flow, error handling, testing

### 6. Ground the Design
Before finalizing, verify decisions:
- External libraries/APIs: Use `web_search` to verify patterns
- Existing patterns: Use `Grep` and `finder` to confirm
- Past decisions: Search with `git log`

### 7. Write design.md
Write to `conductor/tracks/<track_id>/design.md`:
```markdown
# <Track Title>

## Overview
...

## Goals and Non-Goals
...

## Architecture and Components
...

## Data and Interfaces
...

## Risks and Open Questions
...

## Acceptance and Success Criteria
...
```

### 8. Offer Track Creation
Ask: "Create track now (spec + plan)?"

- **No**: "Run `/conductor-newtrack <track_id>` later."
- **Yes**: Execute newtrack workflow for this track_id

### 9. Final Message
"Plan approved. Say `fb` to file issues."

---

## Workflow: Setup

**Trigger:** `/conductor setup`

### 1. Check Existing Setup
- If `conductor/setup_state.json` exists with `last_successful_step: "complete"`, inform user setup is done and suggest `/conductor newtrack`
- If partial state exists, offer to resume or restart

### 2. Detect Project Type
- **Brownfield** (existing): Has `.git`, `package.json`, `requirements.txt`, `go.mod`, or `src/` directory
- **Greenfield** (new): Empty or only README.md

### 3. For Brownfield Projects
1. Announce existing project detected
2. Analyze: README.md, package.json/requirements.txt/go.mod, directory structure
3. Infer: tech stack, architecture, project goals
4. Present findings and ask for confirmation

### 4. For Greenfield Projects
1. Ask: "What do you want to build?"
2. Initialize git if needed: `git init`

### 5. Create Conductor Directory
```bash
mkdir -p conductor/code_styleguides
```

### 6. Generate Context Files (Interactive)
For each file, ask 2-3 targeted questions, then generate:

**product.md** - Product vision, users, goals, features
**tech-stack.md** - Languages, frameworks, databases, tools
**workflow.md** - Copy from templates/workflow.md, customize if requested

For code styleguides, copy relevant files based on tech stack from `templates/code_styleguides/`.

### 7. Initialize Tracks File
Create `conductor/tracks.md`:
```markdown
# Project Tracks

This file tracks all major work items. Each track has its own spec and plan.

---
```

### 8. Generate Initial Track
1. Based on project context, propose an initial track (MVP for greenfield, first feature for brownfield)
2. On approval, create track artifacts (see newtrack workflow)

### 9. Finalize
1. Update `conductor/setup_state.json`: `{"last_successful_step": "complete"}`
2. Commit: `git add conductor && git commit -m "conductor(setup): Initialize conductor"`
3. Announce: "Setup complete. Run `/conductor implement` to start."

---

## Workflow: New Track

**Trigger:** `/conductor-newtrack [track_id or description]`

### 1. Verify Setup
Check these files exist:
- `conductor/product.md`
- `conductor/tech-stack.md`
- `conductor/workflow.md`

If missing, halt and suggest `/conductor-setup`.

### 2. Resolve Track ID
- If `$ARGUMENTS` matches an existing `conductor/tracks/<ARGUMENTS>/`, use it as `track_id`
- Otherwise:
  - Treat `$ARGUMENTS` as description
  - Derive shortname and generate `track_id`: `shortname_YYYYMMDD`

### 3. Check for Existing Design
- If `conductor/tracks/<track_id>/design.md` exists:
  - Read it completely
  - Extract: track title, type (feature/bug), requirements, constraints, success criteria
  - Treat this design as primary source of truth
  - Only ask follow-up questions if there are obvious gaps or contradictions
- If `design.md` does NOT exist:
  - Fall back to full interactive questioning (step 4)

### 4. Generate Spec
- **If using design.md:**
  - Generate `spec.md` by structuring content from design:
    - Overview - Summarize design's high-level intent
    - Functional Requirements - Extract concrete behaviors
    - Acceptance Criteria - Convert success criteria into testable bullets
    - Out of Scope - Extract or infer non-goals

- **If no design.md (fallback):**
  - Ask 3-5 questions based on track type:
    - **Feature**: What does it do? Who uses it? What's the UI? What data?
    - **Bug**: Steps to reproduce? Expected vs actual? When did it start?
  - Generate `spec.md` with: 
    - Overview
    - Functional Requirements
    - Acceptance Criteria
    - Out of Scope

Present for approval, revise if needed.

### 5. Generate Plan
Read `conductor/workflow.md` for task structure (TDD, commit strategy).
Use finalized `spec.md` (and `design.md` if present) to derive phases and tasks.

Generate `plan.md` with phases, tasks, subtasks:
```markdown
# Implementation Plan

## Phase 1: [Name]
- [ ] Task: [Description]
  - [ ] Write tests
  - [ ] Implement
- [ ] Task: Conductor - Phase Verification

## Phase 2: [Name]
...
```

Present for approval, revise if needed.

### 6. Create Track Artifacts
1. If track folder doesn't exist: `mkdir -p conductor/tracks/<track_id>/`
2. Write files:
   - `metadata.json`: `{"track_id": "...", "type": "feature|bug", "status": "new", "created_at": "...", "description": "...", "has_design": true|false}`
   - `spec.md`
   - `plan.md`

### 7. Update Tracks File
Append to `conductor/tracks.md`:
```markdown

---

## [ ] Track: [Description]
*Link: [conductor/tracks/<track_id>/](conductor/tracks/<track_id>/)*
```

### 8. Announce
"Plan approved. Say `fb` to file issues."

---

## Workflow: Implement

**Trigger:** `/conductor-implement [track_id]` or `/conductor-implement Start epic <epic-id>`

**Authoritative Source:** See [commands/conductor-implement.md](../../../commands/conductor-implement.md) for complete execution steps.

### Summary

The implement workflow executes tasks from a track using Beads for issue tracking:

1. **Pre-flight** - Verify jq installed, conductor setup complete
2. **Select Track** - Find target track (explicit or first incomplete)
3. **Check Beads** - Ensure issues exist for the track (run `fb` first if not)
4. **Load Context** - Read design.md, spec.md, plan.md, workflow.md
5. **Update Status** - Mark track as in-progress in tracks.md
6. **Task Loop** - Claim → TDD → Commit → Close → Repeat
7. **Phase Verification** - Run tests, get user confirmation
8. **Track Completion** - Mark complete, offer archive options

### Key Details

- **Thread linking is critical** for doc-sync integration (uses `bd comment` for atomic append)
- **Beads is source of truth** for task status; plan.md updates are best-effort
- **Handoff support** via `bd comment` with IN_PROGRESS/NEXT/THREAD fields
- **Resume** with `/conductor-implement <track_id>` - finds in-progress issues automatically

---

## Workflow: Status

**Trigger:** `/conductor status`

### 1. Read State
- `conductor/tracks.md`
- All `conductor/tracks/*/plan.md` files

### 2. Calculate Progress
For each track:
- Count total tasks, completed `[x]`, in-progress `[~]`, pending `[ ]`
- Calculate percentage

### 3. Present Summary
```
## Conductor Status

**Current Track:** [name] ([x]/[total] tasks)
**Status:** In Progress | Blocked | Complete

### Tracks
- [x] Track: ... (100%)
- [~] Track: ... (45%)
- [ ] Track: ... (0%)

### Current Task
[Current in-progress task from active track]

### Next Action
[Next pending task]
```

---

## Workflow: Revert

**Trigger:** `/conductor revert`

### 1. Identify Target
If no argument, show menu of recent items:
- In-progress tracks, phases, tasks
- Recently completed items

Ask user to select what to revert.

### 2. Find Commits
For the selected item:
1. Read relevant plan.md for commit SHAs
2. Find implementation commits
3. Find plan-update commits
4. For track revert: find track creation commit

### 3. Present Plan
```
## Revert Plan

**Target:** [Task/Phase/Track] - "[Description]"
**Commits to revert:**
- abc1234 (feat: ...)
- def5678 (conductor(plan): ...)

**Action:** git revert in reverse order
```

Ask for confirmation.

### 4. Execute
```bash
git revert --no-edit <sha>  # for each commit, newest first
```

### 5. Update Plan
Reset status markers in plan.md from `[x]` to `[ ]` for reverted items.

### 6. Announce
"Reverted [target]. Plan updated."

---

## State Files Reference

| File | Purpose |
|------|---------|
| `conductor/setup_state.json` | Track setup progress for resume |
| `conductor/product.md` | Product vision, users, goals |
| `conductor/tech-stack.md` | Technology choices |
| `conductor/workflow.md` | Development workflow (TDD, commits) |
| `conductor/tracks.md` | Master track list with status |
| `conductor/tracks/<id>/metadata.json` | Track metadata |
| `conductor/tracks/<id>/spec.md` | Requirements |
| `conductor/tracks/<id>/plan.md` | Phased task list |

## Status Markers

- `[ ]` - Pending/New
- `[~]` - In Progress
- `[x]` - Completed
