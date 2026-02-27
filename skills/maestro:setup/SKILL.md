---
name: maestro-setup
description: "Scaffolds project context (product, tech stack, coding guidelines, product guidelines, workflow) and initializes track registry. Use for first-time project onboarding."
argument-hint: "[--reset]"
---

# Maestro Setup -- Project Context Scaffolding

Interview the user to create persistent project context documents. These files are referenced by all `maestro:*` skills for deeper project understanding.

## Arguments

`$ARGUMENTS`

- `--reset`: Delete all existing context files and start fresh.
- Default (no args): Run setup interview. If context already exists, offer to update or skip.

---

## Step 1: Handle --reset

If `$ARGUMENTS` contains `--reset`:
1. Confirm with user before deleting `.maestro/context/`, `.maestro/tracks.md`, `.maestro/setup_state.json`
2. If confirmed: delete and report. Stop.

## Step 2: Check Setup State (Resume Protocol)

See `reference/resume-protocol.md` for full state machine, step name registry, and skip logic.

Check `.maestro/setup_state.json`. If interrupted run found, offer resume or start over.

## Step 3: Check Existing Context

_Skip if resumed past this step._

Search for `.maestro/context/*.md`. If exists, ask: Update / View / Cancel.

## Step 4: Detect Project Maturity

_Skip if resumed past this step._

Classify as **Brownfield** (existing code) or **Greenfield** (new project).

**Brownfield indicators**: package manifest exists, `src/`/`app/`/`lib/` has code, `.git` with 5+ commits.

**Brownfield flow**:
1. Warn if uncommitted changes exist
2. Ask scan permission (read-only codebase analysis)
3. If granted: scan via `git ls-files | head -200`, read README, manifests, CLAUDE.md, linter configs
4. Store inferences for pre-filling interview answers

**Greenfield flow**: Announce new project. Offer `git init` if no `.git/`.

## Step 5: Create Context Directory

```bash
mkdir -p .maestro/context
```

### 5a: Bootstrap Beads Workspace

If `.beads/` does not exist and `br` is available: `br init --prefix maestro --json && br doctor --json`. Skip silently if `br` is not installed.

## Steps 6-10: Interview & File Generation

Each step generates one context file via an interactive or autogenerate flow.
See `reference/interviews.md` for all questions and branching logic.
See `reference/templates.md` for all file formats.

| Step | File Generated | State Key |
|------|---------------|-----------|
| 6 | `.maestro/context/product.md` | `product_definition` |
| 7 | `.maestro/context/tech-stack.md` | `tech_stack` |
| 8 | `.maestro/context/guidelines.md` | `coding_guidelines` |
| 9 | `.maestro/context/product-guidelines.md` | `product_guidelines` |
| 10 | `.maestro/context/workflow.md` (use `reference/workflow-template.md`) | `workflow_config` |

Write state after each step completes.

## Step 11: Initialize Tracks Registry

Create `.maestro/tracks.md` with registry header. See `reference/templates.md`.

## Step 12: Code Style Guides (Optional)

Offer to copy style guides from `reference/styleguides/` to `.maestro/context/code_styleguides/`.
See `reference/interviews.md` for the question format.

## Step 13: Generate Index File

Write `.maestro/context/index.md` linking all context files and the tracks registry. See `reference/templates.md`.

## Step 14: First Track (Optional)

Offer to create the first track. See `reference/interviews.md` for the flow and `reference/templates.md` for file formats.

## Step 15: Summary and Commit

Display summary of all generated files. See `reference/templates.md` for output format and commit messages.

Remove `.maestro/setup_state.json` on successful completion.

## Step 16: Auto-Generate AGENTS.md

After the commit succeeds, check if `AGENTS.md` exists. If it does NOT exist, automatically invoke `/maestro:AGENTS.md` (no arguments, no user prompt). If `AGENTS.md` already exists, skip silently.

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- **You are here.** Scaffold project context (run first)
- `/maestro:AGENTS.md` -- Generate AGENTS.md context file (offered at end of setup)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Setup is the entry point for all maestro workflows. All other commands depend on the context files it creates. Run this once per project, then use `/maestro:new-track` to start building.
