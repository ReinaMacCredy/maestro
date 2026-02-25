---
name: maestro-review
description: "Code review for a track against its spec and plan. Verifies implementation matches requirements, checks code quality and security."
argument-hint: "[<track-name>] [--current]"
---

# Review -- Track Code Review

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Review the implementation of a track against its specification and plan. Verifies intent match, code quality, test coverage, and security.

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

## Arguments

`$ARGUMENTS`

- `<track-name>`: Match track by name or ID substring
- `--current`: Auto-select the in-progress (`[~]`) track
- No args: ask user which track to review, or fall back to uncommitted/staged changes if no tracks exist

---

## Step 1: Select Track

1. **If `--current`**: Find the `[~]` track in `.maestro/tracks.md`
2. **If track name given**: Match by ID or description substring
3. **If no args and tracks exist**: List completed and in-progress tracks, ask user
4. **If no args and no tracks**: Fall back to reviewing uncommitted changes via `git diff HEAD`

## Step 2: Load Track Context

Read all track files:
- `.maestro/tracks/{track_id}/spec.md` -- requirements to verify against
- `.maestro/tracks/{track_id}/plan.md` -- task SHAs and completion status
- `.maestro/tracks/{track_id}/metadata.json` -- track metadata
- `.maestro/context/code_styleguides/` -- code style references (if exist)
- `.maestro/context/product-guidelines.md` -- product/brand/UX guidelines (if exists)

## Step 3: Collect Commits

Parse `plan.md` for all `[x] {sha}` markers. If no SHAs found (and a track was selected): "Nothing to review." Stop.

If operating in arbitrary scope (no track), skip -- diff collected in Step 4.

## Step 4: Aggregate Diffs

```bash
# Track mode
git diff {first_sha}^..{last_sha}

# Arbitrary scope (no track)
git diff HEAD
```

If diff > 300 lines, offer Iterative Review Mode (per-file review).

## Step 5: Run Automated Checks

```bash
CI=true {test_command}
{lint_command}
{typecheck_command}
```

Report pass/fail for each check.

## Step 6: Review Dimensions

Analyze the diff against 5 dimensions: intent match, code quality, test coverage, security, product guidelines.
See `reference/review-dimensions.md` for full criteria per dimension.

## Step 7: Generate Report

Format findings with severity ratings and checkbox verification.
See `reference/report-template.md` for the full report format and verdict options (PASS / PASS WITH NOTES / NEEDS CHANGES).

## Step 8: Auto-fix Option

If verdict is not PASS, offer auto-fix, manual-only, per-fix review, or complete-track-anyway.
See `reference/report-template.md` for the auto-fix protocol.

## Step 9: Post-Review Cleanup

Offer archive, delete, keep, or skip for the track.
See `reference/report-template.md` for cleanup options.

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- **You are here.** Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Review works best after commits are made, as it analyzes git history to understand what was implemented. It compares the implementation against the spec from `/maestro:new-track` and the plan from `/maestro:implement`. If issues are found, use `/maestro:revert` to undo and re-implement, or apply fixes directly.

Remember: Good validation catches issues before they reach production. Be constructive but thorough in identifying gaps or improvements.
