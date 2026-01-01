---
name: conductor
description: Context-driven development methodology. Understands projects set up with Conductor (via Gemini CLI, Claude Code, Amp Code, Codex, or any Agent Skills compatible CLI). Use when working with conductor/ directories, tracks, specs, plans, or when user mentions context-driven development.
license: Apache-2.0
compatibility: Works with Claude Code, Gemini CLI, Amp Code, Codex, and any Agent Skills compatible CLI
metadata:
  version: "1.6.0"
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

## Prerequisites

Routing and fallback policies are defined in [AGENTS.md](../../AGENTS.md).

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

| Phase            | Purpose                                   | Conductor Equivalent                                           |
| ---------------- | ----------------------------------------- | -------------------------------------------------------------- |
| **Requirements** | Understand problem completely before code | `/conductor-design` → `design.md` → review                     |
| **Plan**         | Create detailed plan BEFORE writing code  | `/conductor-newtrack` uses `design.md` → `spec.md` + `plan.md` |
| **Implement**    | Build incrementally with TDD              | `bd ready` → TDD cycle (one epic per run)                      |
| **Reflect**      | Verify before shipping                    | `verification-before-completion` + `bd close`                  |

**Key Questions per Phase:**

1. **Requirements**: "Does the AI actually understand what we're building?"
2. **Plan**: "Does this plan fit our architecture and constraints?"
3. **Implement**: "Can this be tested independently?"
4. **Reflect**: "Would I bet my job on this code?"

## Double Diamond → Conductor Mapping

The `/conductor-design` command uses Double Diamond methodology with four phases:

```
Double Diamond         Conductor Phase       Output
─────────────────────────────────────────────────────────
DISCOVER (Diverge)  →  Requirements        Problem space explored
DEFINE (Converge)   →  Requirements        Problem statement defined
DEVELOP (Diverge)   →  Plan               Solutions explored
DELIVER (Converge)  →  Plan               design.md finalized
```

### Phase Details

| DD Phase     | Purpose               | Activities                                             | Exit Criteria                               |
| ------------ | --------------------- | ------------------------------------------------------ | ------------------------------------------- |
| **DISCOVER** | Explore problem space | Ask about pain, users, impact, constraints             | Problem articulated, users identified       |
| **DEFINE**   | Frame the problem     | Problem statement, success criteria, scope, approaches | Statement agreed, approach selected         |
| **DEVELOP**  | Explore solutions     | Architecture, components, data model, user flow        | Architecture understood, interfaces defined |
| **DELIVER**  | Finalize design       | Full research verification, acceptance criteria, risks | Design verified and approved                |

### A/P/C Checkpoints

At the end of each phase, users choose:

- **[A] Advanced** - Deeper analysis, assumption audit
- **[P] Party** - Multi-agent feedback (see `../design/references/bmad/`)
- **[C] Continue** - Proceed to next phase
- **[↩ Back]** - Return to previous phase

### Research-Based Verification System

> **NEW:** Replaces tiered grounding with parallel research agents for faster, more comprehensive verification.

Verification is **automatic** at phase transitions using parallel sub-agents:

| Mode | Phase Transition | Agents | Enforcement |
|------|------------------|--------|-------------|
| SPEED | Any | 1 (Locator) | Advisory |
| FULL | DISCOVER→DEFINE | 2 (Locator + Pattern) | Advisory |
| FULL | DEFINE→DEVELOP | 2 (Locator + Pattern) | Advisory |
| FULL | DEVELOP→DELIVER | 4 (Locator + Analyzer + Pattern + Web) | Gatekeeper |
| FULL | DELIVER→Complete | 5 (All agents + Impact) | Mandatory |

**Enforcement levels:**
- **Advisory** - Log skip, warn, proceed
- **Gatekeeper** - Block if verification not run
- **Mandatory** - Block if fails or low confidence

**Research Protocol:**
- [Protocol overview](references/research/protocol.md)
- [Research agents](../orchestrator/agents/research/) (all research agents)
- [Integration hooks](references/research/hooks/)

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

## Beads-Conductor Integration

Conductor integrates with Beads issue tracking to achieve **zero manual bd commands** in the happy path.

### Dual-Mode Architecture

```
Session Start
     │
     ▼
┌─────────────┐
│  PREFLIGHT  │ ─── Mode detect ───┬─► SA Mode (bd CLI)
└─────────────┘                    │
                                   └─► MA Mode (Village MCP)
```

| Mode | Description | When Used |
|------|-------------|-----------|
| **SA** | Single-Agent: Direct `bd` CLI | Default, one agent per codebase |
| **MA** | Multi-Agent: Village MCP | Multiple agents coordinating |

### Preflight Behavior

Every Beads-integrated command runs preflight first:

1. **Check bd availability** → HALT if unavailable (no silent skip)
2. **Detect mode** (SA or MA) and lock for session
3. **Update `metadata.json.last_activity`** timestamp
4. **Recover pending operations** from crashed sessions
5. **Detect concurrent sessions** via heartbeat protocol

### Session Lifecycle

| Phase | SA Mode | MA Mode |
|-------|---------|---------|
| **Claim** | `bd update <id> --status in_progress` | `init()` → `claim()` (atomic) |
| **Reserve** | N/A | `reserve(path, ttl)` before file edits |
| **Close** | `bd close <id> --reason <reason>` | `done(id, reason)` |
| **Sync** | `bd sync` at session end | Automatic via Village |

### Close Reasons

- `completed` - Task finished successfully
- `skipped` - Task skipped (not needed)
- `blocked` - Task blocked, cannot proceed

### TDD Checkpoints (Default On)

TDD is enabled by default. Use `--no-tdd` flag to disable:

```
RED → GREEN → REFACTOR
```

Each phase updates bead notes: `IN_PROGRESS: <phase> phase`

### Subagent Rules

When dispatching subagents via Task tool:

- ✅ **Read-only**: `bd show`, `bd ready`, `bd list`
- ❌ **Blocked**: `bd update`, `bd close`, `bd create`

Subagents return structured results; main agent centralizes writes.

### References

- [Beads Facade](references/beads-facade.md) - API contract
- [Beads Integration](references/beads-integration.md) - All 13 integration points
- [Preflight Workflow](references/preflight-beads.md) - Preflight details

## Command Routing

> **Routing is handled by AGENTS.md.** See [AGENTS.md](../../AGENTS.md) for fallback policies.

This skill only executes - it does not route. Available commands:

| Command | Description |
|---------|-------------|
| `/conductor-setup` | Initialize project with product.md, tech-stack.md, workflow.md |
| `/conductor-design` | Design a feature through Double Diamond dialogue |
| `/conductor-newtrack` | Create spec + plan from design.md, file beads |
| `/conductor-implement` | Execute track (auto-routes to orchestrator if parallel) |
| `/conductor-status` | Display progress overview |
| `/conductor-revert` | Git-aware revert of work |
| `/conductor-revise` | Update spec/plan when issues arise |
| `/conductor-finish` | Complete track: extract learnings, archive |
| `/create_handoff` | Create handoff file for session context |
| `/resume_handoff` | Load handoff and resume session context |

> **Note:** Session continuity in this codebase is **Conductor-only**. The handoff system (`/create_handoff`, `/resume_handoff`) replaces the standalone `continuity` skill from the marketplace plugin. Do not use legacy `continuity save/load` commands.

### `/conductor-implement` Auto-Routing

**CRITICAL:** When `ci` or `/conductor-implement` is triggered, BEFORE executing:

1. **Read the track's metadata.json**
2. **Check if `orchestrated=true`** → If so, skip orchestration (already done), continue sequential
3. **Read the track's plan.md**
4. **Check for `## Track Assignments` section**
5. **If found AND `orchestrated=false` → LOAD orchestrator skill and hand off execution**

```
ci / /conductor-implement
        ↓
  Read metadata.json
        ↓
  orchestrated = true?
        ↓
  ┌─────┴─────┐
  YES         NO
  ↓           ↓
  Continue    Read plan.md
  sequential  Contains "## Track Assignments"?
  (Phase 3)   ┌─────┴─────┐
              YES         NO
              ↓           ↓
              Load        Continue
              orchestrator sequential
              skill       (Phase 3)
```

**When Track Assignments detected (and not already orchestrated):**
```text
I'll load the orchestrator skill for parallel execution.
[Load skill: orchestrator]
```

This ensures parallel execution only happens once when the plan specifies Track Assignments.

### Handoff System

Conductor uses a HumanLayer-inspired handoff system for cross-session context preservation.

| Trigger | When | Automatic |
|---------|------|-----------|
| `design-end` | After `/conductor-newtrack` completes | ✅ |
| `epic-start` | Before each epic in `/conductor-implement` | ✅ |
| `epic-end` | After each epic closes | ✅ |
| `pre-finish` | At start of `/conductor-finish` | ✅ |
| `manual` | User runs `/create_handoff` | ❌ |
| `idle` | 30min inactivity gap detected | ✅ (prompted) |

Handoffs are stored in `conductor/handoffs/<track-id>/` (git-committed, shareable).

See [references/handoff/](references/handoff/) for full documentation.

## Context Loading

When this skill activates, automatically load:

1. `conductor/product.md` - Understand the product
2. `conductor/tech-stack.md` - Know the tech constraints
3. `conductor/workflow.md` - Follow the methodology
4. `conductor/tracks.md` - Current work status
5. `conductor/AGENTS.md` - Learnings from completed tracks
6. `conductor/CODEMAPS/` - Architecture documentation for codebase orientation

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
5. **On stale context**: If setup >2 days old or significant changes detected, suggest running `/conductor-finish` when track completes (which includes Context Refresh)

## After Track Creation

After creating design.md, spec.md and plan.md:

1. Present the plan for review
2. Address any feedback
3. When approved, say: "Track created. Beads filed and reviewed. Run `bd ready` to see available work, or `/conductor-implement` to start."

## Epic Completion Behavior

When `/conductor-implement` completes an epic:

**Present explicit choice to user (do not auto-continue):**

```
Epic complete. Choose:
1. Say `rb` to review remaining beads (recommended: fewer mistakes, but uses more tokens)
2. Handoff to next epic: Start epic <next-epic-id>
```

This ensures quality gates between epics and prevents error propagation.

## Track Completion

When all epics are closed and all beads resolved, the track is ready to finish.

### Auto-Trigger

After closing the last epic, prompt:

```
Track ready. Run `/conductor-finish`?
```

### /conductor-finish Workflow

Runs 6 phases (see [references/finish-workflow.md](references/finish-workflow.md)):

0. **Pre-Flight Validation** - Check for stale state, validate track integrity
1. **Thread Compaction** - Extract learnings from work threads → `LEARNINGS.md`
2. **Beads Compaction** - Generate AI summaries for closed issues
3. **Knowledge Merge** - Dedupe and merge to `conductor/AGENTS.md`
4. **Context Refresh** - Update product.md, tech-stack.md, tracks.md, workflow.md
5. **Archive** - A/K choice (Archive/Keep), single commit, cleanup beads
6. **CODEMAPS Regeneration** - Update architecture documentation

**Flags:**

- `--with-pr` - Chain to finish-branch skill after Phase 6
- `--skip-codemaps` - Skip CODEMAPS regeneration (Phase 6)
- `--skip-refresh` - Skip Context Refresh (Phase 4)

### ready_to_finish Status

Set `metadata.json` status to `"ready_to_finish"` when:

- All child beads are closed
- All epics in plan.md are marked complete

This triggers the auto-prompt for `/conductor-finish`.

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
├── finish-state.json       # Finish workflow tracking (created by /conductor-finish)
├── AGENTS.md               # Learnings hub (auto-updated by /conductor-finish)
├── CODEMAPS/               # Architecture documentation
│   ├── .meta.json          # Generation metadata
│   ├── overview.md         # Project-level architecture (always generated)
│   └── [module].md         # Per-module codemaps (skills.md, api.md, etc.)
├── handoffs/               # Session handoffs (git-committed)
│   ├── general/            # Non-track handoffs
│   │   └── index.md        # Handoff log
│   └── <track_id>/         # Per-track handoffs
│       ├── index.md        # Handoff log
│       └── *.md            # Individual handoff files
├── code_styleguides/       # Language-specific style guides
├── archive/                # Archived completed tracks
├── exports/                # Exported summaries
└── tracks/
    └── <track_id>/         # Format: shortname_YYYYMMDD
        ├── metadata.json   # Track type, status, dates, validation state
        ├── design.md       # High-level design (created via /conductor-design)
        ├── spec.md         # Requirements and acceptance criteria
        ├── plan.md         # Phased task list with status
        ├── revisions.md    # Revision history log (if any)
        ├── implement_state.json  # Implementation resume state (if in progress)
        ├── LEARNINGS.md    # Extracted learnings (created by /conductor-finish)
        └── finish-state.json  # Finish resume state (if interrupted)
```

## CODEMAPS Integration

Conductor generates and maintains architecture documentation in `conductor/CODEMAPS/`:

### Trigger Points

| Command             | CODEMAPS Action                         |
| ------------------- | --------------------------------------- |
| `/conductor-setup`  | Generate initial codemaps               |
| `/conductor-finish` | Auto-regenerate (Phase 6)               |
| `ds`                | Load for context during design sessions |

### Generated Files

- `overview.md` - Project summary, directory structure, key files, data flow (always generated)
- `[module].md` - Per-module codemaps for significant areas (skills.md, api.md, database.md, etc.)
- `.meta.json` - Generation metadata including timestamps and user modification tracking

### .meta.json Structure

```json
{
  "generated": "2025-12-24T10:30:00Z",
  "generator": "/conductor-setup",
  "project_type": "plugin",
  "files": {
    "overview.md": { "generated": true, "user_modified": false },
    "skills.md": { "generated": true, "user_modified": true }
  }
}
```

### User Modification Tracking

Before overwriting during `/conductor-finish`:

1. Compare file mtime to `.meta.json` generated timestamp
2. If `user_modified: true`, warn user before overwriting

### Scale Limits

- Directory scan depth: Top 2 levels only
- Key files per codemap: Max 50 files
- Module codemaps: Max 10 files

### Monorepo Support

Detects `packages/`, `apps/`, or workspaces in package.json and generates per-package codemaps.

### Skipping Regeneration

Use `--skip-codemaps` with `/conductor-finish` to skip CODEMAPS regeneration (useful for batch finishing).

See [references/CODEMAPS_TEMPLATE.md](references/CODEMAPS_TEMPLATE.md) for codemap templates.

## Track Integrity Validation

For detailed validation logic, see:

- `skills/conductor/references/validation/track/checks.md` - Core validation logic
- `skills/conductor/references/validation/track/snippets.md` - State file templates
- `skills/conductor/references/validation/track/recovery.md` - Troubleshooting guide

### Required Files Per Track

| File                   | Required           | Created By                      | Purpose              |
| ---------------------- | ------------------ | ------------------------------- | -------------------- |
| `design.md`            | Optional           | `/conductor-design`             | High-level design    |
| `spec.md`              | Yes (with plan.md) | `/conductor-newtrack`           | Requirements         |
| `plan.md`              | Yes (with spec.md) | `/conductor-newtrack`           | Implementation tasks |
| `metadata.json`        | Yes                | `/conductor-newtrack` Phase 1.3 | Track metadata + generation + beads |

**CRITICAL:** The `metadata.json` file is created at the START of `/conductor-newtrack` in Phase 1.3, BEFORE spec/plan generation. It contains `generation` and `beads` sections for tracking workflow state.

### State File Creation Timeline

```
/conductor-newtrack Phase 1.3 (FIRST):
└── Create metadata.json (status: new, generation.status: initializing, beads.status: pending)

/conductor-newtrack Phase 2.4 (After spec/plan):
└── Update metadata.json (status: planned, generation.status: plan_done, artifacts.spec/plan: true)

beads skill (fb):
├── Validate metadata.json exists (HALT if missing)
└── Update metadata.json.beads (status: in_progress → complete)
```

### Validation Rules

| Rule                                     | Enforcement                      |
| ---------------------------------------- | -------------------------------- |
| spec.md and plan.md must exist together  | HALT if one without other        |
| metadata.json must exist before fb       | HALT if missing                  |
| track_id mismatch                        | Auto-fix to match directory name |
| Corrupted JSON                           | HALT (do not auto-repair)        |

### Auto-Fix Behaviors

| Issue                                            | Action                                             |
| ------------------------------------------------ | -------------------------------------------------- |
| `metadata.json.track_id` != directory name       | Auto-fix: update to directory name                 |
| Missing metadata.json                            | HALT with message: "Run /conductor-newtrack first" |

### /conductor-newtrack Creates All State Files

When `/conductor-newtrack` runs on an EXISTING track (with design.md, spec.md, plan.md but missing metadata.json), it will:

1. **Phase 1.3:** Create metadata.json with generation and beads sections
2. **Phase 2:** Skip spec/plan generation if files exist
3. **Phase 3:** Proceed to file beads

This enables auto-repair of tracks created before this fix.

### Repair Template: metadata.json.beads

```json
{
  "beads": {
    "status": "pending",
    "epicId": null,
    "epics": [],
    "issues": [],
    "planTasks": {},
    "beadToTask": {},
    "crossTrackDeps": [],
    "reviewStatus": null,
    "reviewedAt": null
  }
}
```

### /conductor-status Integration

When `/conductor-status` runs, include integrity check:

```
Track: codemaps-integration_20251223
Status: new (repaired: missing metadata.json, implement_state.json)
Files: design.md ✓, spec.md ✓, plan.md ✓
Beads: Not filed (run `fb` to file)
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

| Gemini CLI             | Claude Code            |
| ---------------------- | ---------------------- |
| `/conductor:setup`     | `/conductor-setup`     |
| `/conductor:design`    | `/conductor-design`    |
| `/conductor:newTrack`  | `/conductor-newtrack`  |
| `/conductor:implement` | `/conductor-implement` |
| `/conductor:status`    | `/conductor-status`    |
| `/conductor:revert`    | `/conductor-revert`    |
| `/conductor:revise`    | `/conductor-revise`    |
| `/conductor:finish`    | `/conductor-finish`    |

Files, workflows, and state management are fully compatible.

## Example: Recognizing Conductor Projects

When you see `conductor/tracks.md` with content like:

```markdown
## [~] Track: Add user authentication

_Link: [conductor/tracks/auth_20251215/](../../conductor/tracks/auth_20251215/)_
```

You know:

- This is a Conductor project
- There's an in-progress track for authentication
- Spec and plan are in `conductor/tracks/auth_20251215/`
- Follow the workflow in `conductor/workflow.md`

## References

For detailed workflow documentation, see [references/workflows.md](references/workflows.md) (index) and the authoritative per-command docs in [references/workflows/](references/workflows/).

**Key workflow files:**
- [workflows/newtrack.md](references/workflows/newtrack.md) - Track creation with validation gates
- [workflows/implement.md](references/workflows/implement.md) - Task execution
- [finish-workflow.md](references/finish-workflow.md) - Track completion
- [tdd/cycle.md](references/tdd/cycle.md) - TDD cycle with validation gate
