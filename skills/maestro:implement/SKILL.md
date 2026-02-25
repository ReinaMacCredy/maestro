---
name: maestro-implement
description: "Execute track tasks following TDD workflow. Single-agent by default, --team for parallel Agent Teams, Sub Agent Parallels. Use when ready to implement a planned track."
argument-hint: "[<track-name>] [--team]"
---

# Implement -- Task Execution Engine

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Execute tasks from a track's implementation plan, following the configured workflow methodology (TDD or ship-fast). Supports single-agent mode (default) and team mode (`--team`).

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

## Arguments

`$ARGUMENTS`

- `<track-name>`: Match track by name or ID substring. Optional -- auto-selects if only one track is pending.
- `--team`: Enable team mode with parallel workers (kraken/spark).
- `--resume`: Skip already-completed tasks (marked `[x]`) and continue from next `[ ]` task.

---

## Step 1: Mode Detection

Parse `$ARGUMENTS`:
- If contains `--team` --> team mode (see `reference/team-mode.md`)
- Otherwise --> single-agent mode (default)
- If contains `--resume` --> set resume flag

## Step 2: Track Selection

1. Read `.maestro/tracks.md`. Parse status markers: `[ ]` = new, `[~]` = in-progress, `[x]` = complete. Support both `- [ ] **Track:` and legacy `## [ ] Track:` formats.
2. **If track name given**: Match by exact ID or case-insensitive substring on description. If multiple matches, ask user.
3. **If no track name**: Filter `[ ]`/`[~]` tracks. 0 = error, 1 = auto-select, multiple = ask user.
4. **Confirm selection**: Ask user to start or cancel.

## Step 3: Load Context

1. Read track plan: `.maestro/tracks/{track_id}/plan.md`
2. Read track spec: `.maestro/tracks/{track_id}/spec.md`
3. Read workflow config: `.maestro/context/workflow.md`
4. Read tech stack: `.maestro/context/tech-stack.md`
5. Read guidelines: `.maestro/context/guidelines.md` (if exists)
6. Read code style guides: `.maestro/context/code_styleguides/` (if exists)
7. Load skill guidance from `.maestro/tracks/{track_id}/metadata.json` `"skills"` array. For each skill, load its SKILL.md content. **Graceful degradation**: if missing/empty, proceed without.
8. Read `.maestro/notepad.md` (if exists). Extract `## Priority Context` bullets. These are injected as constraints into task execution context. **Graceful degradation**: if missing or empty, skip.

## Step 4: Update Track Status

Edit `.maestro/tracks.md`: `[ ]` --> `[~]`. Update `metadata.json`: `"status": "in_progress"`.

## Step 4.5: BR Bootstrap Check

If `.beads/` does not exist and `br` is available:

```bash
[ -d ".beads" ] || br init --prefix maestro --json
```

If `br` is not installed, skip silently.

## Step 5: Build Task Queue

Parse `plan.md`: identify phases (`## Phase N`), tasks (`### Task N.M`), sub-tasks (`- [ ] ...`).
If `--resume`: skip tasks already marked `[x]`.

**BR-enhanced path**: If `metadata.json` has `beads_epic_id`:
- Use `bv -robot-plan -label "track:{epic_id}" -format json` to get dependency-respecting execution order
- If `--resume`: use `br list --status open --label "phase:{N}" --json` to identify remaining work (skip closed issues)
- Fall back to plan.md parsing if `bv` is unavailable or the command fails

See `reference/br-integration.md` for full BR/BV usage patterns.

---

## Single-Agent Mode (Default)

### Step 6a: Execute Tasks Sequentially

Follow the TDD or ship-fast methodology for each task.
See `reference/single-agent-execution.md` for the full Red-Green-Refactor cycle (steps 6a.1-6a.9), ship-fast variant, and skill injection protocol.
See `reference/tdd-workflow.md` for TDD best practices and anti-patterns.

### Step 7a: Phase Completion Verification

When the last task in a phase completes, run the Phase Completion Protocol.
See `reference/phase-completion.md` for details (coverage check, full test run, manual verification, user confirmation).

---

## Team Mode (--team)

See `reference/team-mode.md` for full protocol: team creation, task delegation, worker spawning, monitoring, verification, and shutdown.

---

## Step 8: Track Completion

When ALL phases are complete, run the Track Completion Protocol.
See `reference/track-completion.md` for details (mark complete, skill effectiveness recording, doc sync, cleanup, final commit, summary).

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- **You are here.** Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Implementation consumes the `plan.md` created by `/maestro:new-track`. Each task produces atomic commits, which `/maestro:review` can analyze to verify correctness against the spec. Run `/maestro:status` to check progress mid-implementation, or `/maestro:revert` to undo if something goes wrong.
