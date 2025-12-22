---
name: conductor
version: "1.2.0"
description: Context-driven development methodology. Understands projects set up with Conductor (via Gemini CLI or Claude Code). Use when working with conductor/ directories, tracks, specs, plans, or when user mentions context-driven development.
license: Apache-2.0
compatibility: Works with Claude Code, Gemini CLI, Amp Code, Codex, and any Agent Skills compatible CLI
metadata:
  author: "Gemini CLI Extensions"
  repository: "https://github.com/gemini-cli-extensions/conductor"
  keywords:
    - context-driven-development
    - specs
    - plans
    - tracks
    - tdd
    - workflow
---

# Conductor: Context-Driven Development

Measure twice, code once.

## Overview

Conductor enables context-driven development by:
1. Establishing project context (product vision, tech stack, workflow)
2. Organizing work into "tracks" (features, bugs, improvements)
3. Creating specs and phased implementation plans
4. Executing with TDD practices and progress tracking

## 4-Phase Framework Mapping

Conductor implements the Knowledge & Vibes 4-phase framework:

| Phase | Purpose | Conductor Equivalent |
|-------|---------|---------------------|
| **Requirements** | Understand problem completely before code | `/conductor-design` → `design.md` → review |
| **Plan** | Create detailed plan BEFORE writing code | `/conductor-newtrack` uses `design.md` → `spec.md` + `plan.md` |
| **Implement** | Build incrementally with TDD | `bd ready` → TDD cycle (one epic per run) |
| **Reflect** | Verify before shipping | `verification-before-completion` + `bd close` |

**Key Questions per Phase:**

1. **Requirements**: "Does the AI actually understand what we're building?"
2. **Plan**: "Does this plan fit our architecture and constraints?"
3. **Implement**: "Can this be tested independently?"
4. **Reflect**: "Would I bet my job on this code?"

**Interoperability:** This skill understands conductor projects created by either:
- Gemini CLI extension (`/conductor:setup`, `/conductor:newTrack`, etc.)
- Claude Code commands (`/conductor-setup`, `/conductor-newtrack`, etc.)

Both tools use the same `conductor/` directory structure.

## When to Use This Skill

Automatically engage when:
- Project has a `conductor/` directory
- User mentions specs, plans, tracks, or context-driven development
- User asks about project status or implementation progress
- Files like `conductor/tracks.md`, `conductor/product.md` exist
- User wants to organize development work

## Slash Commands

Users can invoke these commands directly:

| Command | Description |
|---------|-------------|
| `/conductor-setup` | Initialize project with product.md, tech-stack.md, workflow.md |
| `/conductor-design [desc]` | Design a feature/bug through collaborative dialogue (replaces `bs`) |
| `/conductor-newtrack [id or desc]` | Create spec and plan from design.md (or interactive if no design) |
| `/conductor-implement [id]` | Execute ONE EPIC from track's plan |
| `/conductor-status` | Display progress overview |
| `/conductor-revert` | Git-aware revert of work |
| `/conductor-revise` | Update spec/plan when implementation reveals issues |
| `/conductor-refresh` | Sync context docs with current codebase |

## Intent Mapping

When users express these intents, invoke the corresponding workflow:

| User Intent | Action | Command |
|-------------|--------|---------|
| "Set up this project" / "Initialize conductor" | Run setup workflow | `/conductor-setup` |
| "Design a feature" / "Brainstorm X" | Create design through dialogue | `/conductor-design [desc]` |
| "Create a new feature" / "Add a track for X" | Create track from design | `/conductor-newtrack [id]` |
| "Start working" / "Implement the feature" | Begin implementation | `/conductor-implement` |
| "What's the status?" / "Show progress" | Display status | `/conductor-status` |
| "Undo that" / "Revert the last task" | Revert work | `/conductor-revert` |
| "Check for issues" / "Validate the project" | Run validation | `/conductor-validate` |
| "This is blocked" / "Can't proceed" | Mark as blocked | `/conductor-block` |
| "Skip this task" | Skip current task | `/conductor-skip` |
| "Archive completed tracks" | Archive tracks | `/conductor-archive` |
| "Export project summary" | Generate export | `/conductor-export` |
| "Docs are outdated" / "Sync with codebase" | Refresh context | `/conductor-refresh` |
| "Spec is wrong" / "Plan needs update" | Revise spec/plan | `/conductor-revise` |

## Context Loading

When this skill activates, automatically load:
1. `conductor/product.md` - Understand the product
2. `conductor/tech-stack.md` - Know the tech constraints
3. `conductor/workflow.md` - Follow the methodology
4. `conductor/tracks.md` - Current work status

For active tracks, also load:
- `conductor/tracks/<track_id>/design.md` (if exists)
- `conductor/tracks/<track_id>/spec.md`
- `conductor/tracks/<track_id>/plan.md`

## Proactive Behaviors

When skill is active:
1. **On new session**: Check for in-progress tracks, offer to resume
2. **On task completion**: Suggest next task or phase verification
3. **On blocked detection**: Alert user and suggest alternatives
4. **On all tasks complete**: Congratulate and offer archive/cleanup
5. **On stale context**: If setup >2 days old or significant changes detected, suggest `/conductor-refresh`

## After Track Creation

After creating design.md, spec.md and plan.md:

1. Present the plan for review
2. Address any feedback
3. When approved, say: "Plan approved. Say `fb` to convert into beads issues."

## Epic Completion Behavior

When `/conductor-implement` completes an epic:

**Present explicit choice to user (do not auto-continue):**

```
Epic complete. Choose:
1. Say `rb` to review remaining beads (recommended: fewer mistakes, but uses more tokens)
2. Handoff to next epic: Start epic <next-epic-id>
```

This ensures quality gates between epics and prevents error propagation.

## Conductor Directory Structure

When you see this structure, the project uses Conductor:

```
conductor/
├── product.md              # Product vision, users, goals
├── product-guidelines.md   # Brand/style guidelines (optional)
├── tech-stack.md           # Technology choices
├── workflow.md             # Development standards (TDD, commits, coverage)
├── tracks.md               # Master track list with status markers
├── setup_state.json        # Setup progress tracking
├── refresh_state.json      # Context refresh tracking (created by /conductor-refresh)
├── code_styleguides/       # Language-specific style guides
├── archive/                # Archived completed tracks
├── exports/                # Exported summaries
└── tracks/
    └── <track_id>/         # Format: shortname_YYYYMMDD
        ├── metadata.json   # Track type, status, dates
        ├── design.md       # High-level design (created via /conductor-design)
        ├── spec.md         # Requirements and acceptance criteria
        ├── plan.md         # Phased task list with status
        ├── revisions.md    # Revision history log (if any)
        └── implement_state.json  # Implementation resume state (if in progress)
```

## Status Markers

Throughout conductor files:
- `[ ]` - Pending/New
- `[~]` - In Progress
- `[x]` - Completed (often followed by 7-char commit SHA)

## Reading Conductor Context

When working in a Conductor project:

1. **Read `conductor/product.md`** - Understand what we're building and for whom
2. **Read `conductor/tech-stack.md`** - Know the technologies and constraints
3. **Read `conductor/workflow.md`** - Follow the development methodology (usually TDD)
4. **Read `conductor/tracks.md`** - See all work items and their status
5. **For active work:** Read the current track's `spec.md` and `plan.md`

## Workflow Integration

When implementing tasks, follow `conductor/workflow.md` which typically specifies:

1. **TDD Cycle:** Write failing test → Implement → Pass → Refactor
2. **Coverage Target:** Usually >80%
3. **Commit Strategy:** Conventional commits (`feat:`, `fix:`, `test:`, etc.)
4. **Task Updates:** Mark `[~]` when starting, `[x]` when done + commit SHA
5. **Phase Verification:** Manual user confirmation at phase end

## Gemini CLI Compatibility

Projects set up with Gemini CLI's Conductor extension use identical structure.
The only differences are command syntax:

| Gemini CLI | Claude Code |
|------------|-------------|
| `/conductor:setup` | `/conductor-setup` |
| `/conductor:newTrack` | `/conductor-newtrack` |
| `/conductor:implement` | `/conductor-implement` |
| `/conductor:status` | `/conductor-status` |
| `/conductor:revert` | `/conductor-revert` |

Files, workflows, and state management are fully compatible.

## Example: Recognizing Conductor Projects

When you see `conductor/tracks.md` with content like:

```markdown
## [~] Track: Add user authentication
*Link: [conductor/tracks/auth_20241215/](conductor/tracks/auth_20241215/)*
```

You know:
- This is a Conductor project
- There's an in-progress track for authentication
- Spec and plan are in `conductor/tracks/auth_20241215/`
- Follow the workflow in `conductor/workflow.md`

## References

For detailed workflow documentation, see [references/workflows.md](references/workflows.md).
