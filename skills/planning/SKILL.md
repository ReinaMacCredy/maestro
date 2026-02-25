---
name: planning
description: "Generate comprehensive implementation plans through systematic discovery, synthesis, verification, and decomposition into beads. Use when asked to plan a feature, create a roadmap, design an implementation approach, or decompose work into trackable issues. Do NOT use for simple one-step tasks, quick fixes, or when the user just wants to execute an existing plan — use the work skill instead."
---

# Feature Planning Pipeline

Transform a feature request into an execution-ready plan through structured discovery, risk-based verification, and parallel track decomposition. Produce plans that the orchestrator can execute immediately via `/work`.

## Quick Reference

| Task                    | Tool / Command                                              |
| ----------------------- | ----------------------------------------------------------- |
| Codebase structure      | `finder` + `shell_command` (`rg --files`, targeted `rg`)   |
| Find definitions        | `finder` + `rg "symbolName"`                               |
| Find usages             | `finder` or `rg` depending on scope                        |
| External patterns       | `librarian`                                                 |
| Library docs            | `web_search` + `read_web_page`                              |
| Gap analysis            | Main-thread synthesis; optional `handoff` reviewer thread   |
| Create beads            | `br create` (optional `skill("file-beads")`)               |
| Validate graph          | `bv -robot-*` via `shell_command`                           |
| Parallel spike workers  | `handoff` threads                                           |

## Pipeline Overview

```
USER REQUEST → 0. Session Setup → 1. Discovery → 2. Synthesis → 3. Verification → 4. Decomposition → 5. Validation → 6. Track Planning → 7. Approve/Save → Ready Plan
```

| Phase             | Tool                                                                        | Output                              |
| ----------------- | --------------------------------------------------------------------------- | ----------------------------------- |
| 0. Session Setup  | `shell_command` or file write tool                                          | Handoff state initialized           |
| 1. Discovery      | `multi_tool_use.parallel`, `finder`, `shell_command`, `librarian`, `web_*`  | Discovery Report                    |
| 2. Synthesis      | Main-thread synthesis + optional `handoff` reviewer thread                  | Approach + Risk Map                 |
| 3. Verification   | Spikes via `handoff` workers + beads CLI (`br`/`bv`)                        | Validated Approach + Learnings      |
| 4. Decomposition  | beads CLI (`br create`), optional `skill("file-beads")` if available        | `.beads/*.md` files                 |
| 5. Validation     | beads CLI (`bv`) + second-pass synthesis/review                             | Validated dependency graph          |
| 6. Track Planning | `bv -robot-plan` + `shell_command`/`jq`                                     | Execution plan with parallel tracks |
| 7. Approve/Save   | User approval + file write + handoff update                                 | `.maestro/plans/{topic}.md`         |

## Phase Summaries

### 0. Session Setup

Derive a topic slug from the request (kebab-case, max 4 words). Create `.maestro/handoff/{topic}.json` with `status: "designing"` and ensure `.maestro/handoff/` and `.maestro/plans/` directories exist. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 1. Discovery

Launch parallel exploration with `finder`, `shell_command`, `librarian`, and `web_search`. Capture architecture, existing patterns, constraints, and external references in `.maestro/drafts/{topic}-discovery.md`. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 2. Synthesis

Synthesize the discovery report into a gap analysis, 1-3 approach options with tradeoffs, and a risk assessment (LOW / MEDIUM / HIGH). Save to `.maestro/drafts/{topic}-approach.md`. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 3. Verification

Create spike beads for HIGH-risk items. Execute spikes in parallel `handoff` workers with 30-minute time-boxes. Aggregate findings back into the approach document. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 4. Decomposition

Create beads with `br create`, embedding spike learnings and acceptance criteria in each. Assign file scopes for track planning. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 5. Validation

Run `bv -robot-suggest`, `bv -robot-insights`, and `bv -robot-priority` to find missing dependencies, detect cycles, and validate priorities. Fix issues with `br dep add`, `br dep remove`, and `br update`. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 6. Track Planning

Generate an execution-ready plan with parallel tracks, file scopes, and agent names. Verify no cycles and all beads are assigned. Save to `.maestro/drafts/{topic}-execution-plan.md`. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

### 7. Approve and Save

Present a plan summary. Gate on user approval (Approve / Revise / Cancel). Save final plan to `.maestro/plans/{topic}.md`, capture key decisions to notepad, update handoff status, and report the `/work` command for execution. Read [reference/phases.md](reference/phases.md) for detailed phase instructions.

Before writing `.maestro/plans/{topic}.md`, enforce the `/work` plan contract:

- `## Objective` section is present (required)
- At least one unchecked task checkbox `- [ ] ...` exists (required)
- `## Verification` section is present (required)
- `## Scope` section is present (recommended; warn if missing)

If required sections are missing, keep output in draft state and revise before save.

## Amp Tool Mapping

Use this mapping when older docs mention non-Amp tools:

| Legacy reference           | Amp-native replacement                                                        |
| -------------------------- | ----------------------------------------------------------------------------- |
| `gkg repo_map`             | `finder` + `shell_command` (`rg --files`, targeted `rg`)                     |
| `gkg definitions/references` | `finder` (behavior search) + `rg` exact lookups                           |
| `exa docs`                 | `web_search` then `read_web_page`                                             |
| `oracle`                   | Main-agent synthesis; optionally run `handoff` for isolated critique/review   |
| `Task()` workers           | `handoff` threads (parallel where independent)                                |

## Output Artifacts

| Artifact          | Location                                        | Purpose                            |
| ----------------- | ----------------------------------------------- | ---------------------------------- |
| Handoff State     | `.maestro/handoff/{topic}.json`                 | Session recovery + plan status     |
| Discovery Report  | `.maestro/drafts/{topic}-discovery.md`          | Codebase snapshot                  |
| Approach Document | `.maestro/drafts/{topic}-approach.md`           | Strategy + risks                   |
| Spike Code        | `.spikes/<feature>/`                            | Reference implementations          |
| Spike Learnings   | Embedded in beads                               | Context for workers                |
| Beads             | `.beads/*.md`                                   | Executable work items              |
| Execution Plan    | `.maestro/drafts/{topic}-execution-plan.md`     | Track assignments for orchestrator |
| Final Plan        | `.maestro/plans/{topic}.md`                     | `/work`-ready execution plan       |

## Common Mistakes

**CRITICAL — Skipping discovery**
- x Jumping straight to decomposition without exploring the codebase
- ✓ Always run Phase 1 — plans that miss existing patterns cause rework

**CRITICAL — No spikes for HIGH risk items**
- x Assuming external integrations or novel patterns will work
- ✓ Create spike beads, execute in `handoff` workers, embed learnings in beads

**CRITICAL — Missing `bv` validation**
- x Skipping graph validation before finalizing the plan
- ✓ Run `bv -robot-suggest`, `bv -robot-insights`, `bv -robot-priority` in Phase 5

**CRITICAL — Overlapping file scopes across tracks**
- x Assigning the same files to multiple parallel tracks
- ✓ Merge overlapping tracks or split files into non-overlapping scopes

- Missing learnings in beads → Workers re-discover the same issues
- No risk assessment → Surprises during execution
- Skipping the approval gate → Plans saved without user consent

## Templates

Read [reference/templates.md](reference/templates.md) for all document templates: Discovery Report, Approach Document, Spike Bead, Bead with Learnings, and Execution Plan.
