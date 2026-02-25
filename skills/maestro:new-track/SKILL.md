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

The track description. Examples: `"Add dark mode support"`, `"Fix login timeout"`, `"Refactor connection pooling"`

---

## Step 1: Validate Prerequisites

Check `.maestro/context/product.md` exists. If not: "Run `/maestro:setup` first." Stop.

Check `.maestro/tracks.md` exists. If missing, create it with registry header.

## Step 2: Parse Input

Extract track description from `$ARGUMENTS`. If empty, ask user for type (feature/bug/chore) and description.

## Step 3: Generate Track ID

Format: `{shortname}_{YYYYMMDD}` (2-4 words, snake_case + date). Example: `dark_mode_20260225`

## Step 4: Duplicate Check

Scan `.maestro/tracks/*` directories. Warn if any starts with the same short name prefix.

## Step 5: Create Track Directory

```bash
mkdir -p .maestro/tracks/{track_id}
```

## Step 6: Auto-Infer Track Type

Analyze description keywords to classify as `feature`, `bug`, or `chore`. Only confirm with user if ambiguous.

Inference rules:
- **feature**: add, build, create, implement, support, introduce
- **bug**: fix, broken, error, crash, incorrect, regression, timeout, fail
- **chore**: refactor, cleanup, migrate, upgrade, rename, reorganize, extract

## Step 7: Specification Interview

Run the type-specific interview to gather requirements.
See `reference/interview-questions.md` for all questions per type (feature/bug/chore).

## Step 8: Draft Specification

Compose spec from interview answers. See `reference/interview-questions.md` for the spec template and approval loop.

Present full draft for approval. Max 3 revision loops. Write to `.maestro/tracks/{track_id}/spec.md`.

## Step 9: Generate Implementation Plan

Read context: `workflow.md`, `tech-stack.md`, `guidelines.md`.
Use `reference/plan-template.md` for structure.
See `reference/interview-questions.md` for plan rules and TDD injection.

Present full plan for approval. Max 3 revision loops. Write to `.maestro/tracks/{track_id}/plan.md`.

## Step 9.5: Detect Relevant Skills

Match installed skills against track context for auto-loading during implementation.
See `reference/skill-detection.md` for the full detection protocol (cache check, corpus build, matching, recording).

## Step 10-12: Write Metadata, Index, and Registry

Write `metadata.json`, `index.md`, update `tracks.md`.
See `reference/metadata-and-registry.md` for all schemas, templates, commit message, and summary format.

## Step 13: Commit

```bash
git add .maestro/tracks/{track_id} .maestro/tracks.md
git commit -m "chore(maestro:new-track): add track {track_id}"
```

## Step 14: Summary

Display track creation summary with ID, type, phase/task counts, file paths, and next step (`/maestro:implement`).

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
