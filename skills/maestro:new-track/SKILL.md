---
name: maestro:new-track
description: "Create a new feature/bug track with spec and implementation plan. Interactive interview generates requirements spec, then phased TDD plan. Use when starting work on a new feature, bug fix, or chore."
argument-hint: "<track description>"
---

# New Track -- Specification & Planning

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

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

Search for files matching `.maestro/context/product.md`.

If no context files exist:
- Report: "Project context not found. Run `/maestro:setup` first."
- Stop.

Check that tracks registry exists:

Read `.maestro/tracks.md`.

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

Ask the user: "What feature, bug fix, or chore would you like to track?"
Options:
- **Feature** -- New functionality to build
- **Bug fix** -- Something broken to fix
- **Chore** -- Refactoring, cleanup, or maintenance

Then ask for the description as a follow-up.

## Step 3: Generate Track ID

Format: `{shortname}_{YYYYMMDD}`

Rules:
- Extract a short name from the description (2-4 words, snake_case)
- Append today's date: `YYYYMMDD`
- Example: `"Add dark mode support"` --> `dark_mode_20260225`

## Step 4: Duplicate Check

Search for files matching `.maestro/tracks/*`.

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

Ask the user: "I inferred this as '{inferred_type}' -- does that look right?"
Options:
- **Feature** -- New functionality or capability
- **Bug** -- Fix for broken behavior
- **Chore** -- Refactoring, maintenance, or tech debt

## Step 7: Specification Interview

Generate a requirements specification through interactive questioning. Batch independent questions into a single interaction if your runtime supports it.

### For Features (batch these into a single interaction if your runtime supports it):

1. Ask the user: "What should this feature do? Describe the core behavior and expected outcomes."

2. Ask the user: "How will users interact with this feature?"
   Options:
   - **UI component** -- Visual element users see and interact with
   - **API endpoint** -- Programmatic interface
   - **CLI command** -- Terminal command or flag
   - **Background process** -- No direct user interaction

3. Ask the user: "Any constraints or non-functional requirements? (performance, security, compatibility)" (select all that apply)
   Options:
   - **No special constraints** -- Standard quality expectations
   - **Performance-critical** -- Must meet specific latency/throughput targets
   - **Security-sensitive** -- Handles auth, PII, or financial data
   - **Let me specify** -- Type your constraints

4. Ask the user: "Any known edge cases or error scenarios to handle?"
   Options:
   - **I'll list them** -- Type known edge cases
   - **Infer from requirements** -- Generate edge cases from the spec

### For Bugs (batch these into a single interaction if your runtime supports it):

1. Ask the user: "What is happening? Provide steps to reproduce."

2. Ask the user: "What should happen instead?"

3. Ask the user: "How critical is this? Which users or flows are affected?"
   Options:
   - **Blocker** -- Core flow broken, no workaround
   - **High** -- Significant degradation, workaround exists
   - **Medium** -- Noticeable issue, limited impact
   - **Low** -- Minor or cosmetic

### For Chores (batch these into a single interaction if your runtime supports it):

1. Ask the user: "What needs to change and why?"

2. Ask the user: "Any backward compatibility requirements?"
   Options:
   - **Must be backward compatible** -- No breaking changes to public API
   - **Breaking changes acceptable** -- Semver major bump is fine
   - **Internal only** -- No public surface affected

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

Ask the user: "Here is the drafted specification -- does it look correct?\n\n---\n{full spec content}\n---"
Options:
- **Approved** -- Spec is ready, generate the plan
- **Needs revision** -- I'll tell you what to change

If revision needed: ask what to change, update, and re-present with the full updated content embedded. Max 3 revision loops.

Write approved spec to `.maestro/tracks/{track_id}/spec.md`.

## Step 9: Generate Implementation Plan

Read project context for informed planning:
- `.maestro/context/workflow.md`
- `.maestro/context/tech-stack.md`
- `.maestro/context/guidelines.md`

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

## Step 9.5: Detect Relevant Skills

After generating the implementation plan, identify installed skills that could provide domain expertise during implementation.

### 9.5.1: Check Learning Cache

Read `.maestro/context/skill-mappings.md` (if exists).

If a mapping row exists where:
- Track type matches, AND
- 2+ keywords from the row appear in this track's description

Then use the cached skill names directly. Mark each with `relevance: "cached"`. Skip to Step 9.5.4.

If no cache hit, proceed to Step 9.5.2.

### 9.5.2: Build Match Corpus

Combine these into a single text corpus for matching:
- Track description (from Step 2)
- Track type (feature/bug/chore)
- Technology keywords from `.maestro/context/tech-stack.md` (already read in Step 9)
- Phase titles and task titles from the generated plan (from Step 9)

### 9.5.3: Match Skills

The runtime injects a list of all installed skills (names + descriptions) into the agent's context at conversation start. Use this list as the skill registry.

For each skill in the runtime skill list:
1. Check if the skill's description keywords overlap with the match corpus
2. Prioritize skills whose description explicitly mentions technologies, frameworks, or patterns present in the match corpus
3. Exclude skills that are clearly irrelevant (e.g., `reset`, `status`, `pipeline` -- workflow/utility skills, not domain expertise)

**Relevance filter**: Only match skills that provide domain expertise (coding patterns, framework guidance, testing strategies). Skip workflow/orchestration skills.

### 9.5.4: Record Matched Skills

If skills were matched, store them for Step 10 (metadata.json).

Print an informational message:

```
[ok] Detected {N} relevant skill(s) for this track:
  --> {skill-1-name}: {one-line description}
  --> {skill-2-name}: {one-line description}
These will be auto-loaded during /maestro:implement.
```

If no skills matched, print nothing. Proceed to Step 10.

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
  "tasks": {task_count},
  "skills": [
    {
      "name": "skill-name",
      "relevance": "matched",
      "matched_on": ["keyword1", "keyword2"]
    }
  ]
}
```

Note: `"skills"` is `[]` if no skills were detected in Step 9.5.

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

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- **You are here.** Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed

A track created here produces `spec.md` and `plan.md` that `/maestro:implement` consumes. The spec also serves as the baseline for `/maestro:review` to validate against. Good specs lead to good implementations -- be thorough in the interview.
