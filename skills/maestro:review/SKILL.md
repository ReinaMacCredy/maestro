---
name: maestro:review
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
4. **If no args and no tracks exist**: Fall back to arbitrary scope -- review uncommitted/staged changes via `git diff HEAD` (or `git diff --staged` for staged-only). Notify user: "No tracks found. Reviewing uncommitted changes."

Ask the user: "Which track would you like to review?"
Options:
- **{track_1}** -- {status} | {task_count} tasks
- **{track_2}** -- {status} | {task_count} tasks

## Step 2: Load Track Context

Read all track files:
- `.maestro/tracks/{track_id}/spec.md` -- requirements to verify against
- `.maestro/tracks/{track_id}/plan.md` -- task SHAs and completion status
- `.maestro/tracks/{track_id}/metadata.json` -- track metadata
- `.maestro/context/code_styleguides/` -- code style references (if exist)
- `.maestro/context/product-guidelines.md` -- product/brand/UX guidelines (if exists)

## Step 3: Collect Commits

Parse `plan.md` for all `[x] {sha}` markers. Collect the list of commit SHAs.

If no SHAs found and a track was selected:
- Report: "No completed tasks found in this track. Nothing to review."
- Stop.

If operating in arbitrary scope (no track), skip this step -- diff is collected in Step 4.

## Step 4: Aggregate Diffs

```bash
# Track mode: get combined diff for all track commits
git diff {first_sha}^..{last_sha}

# Arbitrary scope (no track): get uncommitted/staged changes
git diff HEAD
# or for staged-only:
git diff --staged
```

If the diff is larger than 300 lines, ask the user before chunking:

Ask the user: "This diff is {N} lines. Use Iterative Review Mode (per-file review)?"
Options:
- **Yes** -- Review each file separately for more focused feedback
- **No** -- Review the full diff at once

If Yes: chunk the diff by file and process each file in sequence, accumulating findings.

## Step 5: Run Automated Checks

Before manual review, run automated checks:

```bash
# Run test suite
CI=true {test_command}

# Run linter (if configured)
{lint_command}

# Run type checker (if configured)
{typecheck_command}
```

Report results: pass/fail for each check.

## Step 6: Review Dimensions

Analyze the diff against 5 dimensions:

### 6.1: Intent Match

Compare implementation against spec.md:
- For each acceptance criterion in spec, verify it's addressed in the code
- Flag any spec requirements that appear unimplemented
- Flag any implemented behavior not in the spec (scope creep)

### 6.2: Code Quality

Review against code style guides and general quality:
- Naming conventions consistent with project style
- Function/method size and complexity
- Code duplication
- Error handling patterns
- Proper use of language idioms

**Code style guides are the Law. Violations are High severity by default. Only downgrade severity with explicit written justification in the finding.**

When reporting a style violation, include an explicit diff block showing the required change:

```diff
- non_compliant_code_here
+ compliant_code_here
```

### 6.3: Test Coverage

Assess test quality:
- Are all acceptance criteria covered by tests?
- Do tests verify behavior (not implementation details)?
- Are edge cases from spec tested?
- Are error scenarios tested?

### 6.4: Security

Basic security review:
- Input validation at boundaries
- No hardcoded secrets or credentials
- SQL/command injection prevention
- XSS prevention (for web code)
- Auth/authz checks where appropriate

### 6.5: Product Guidelines Compliance

Only run this dimension if `.maestro/context/product-guidelines.md` was loaded in Step 2.

Check the implementation against product guidelines:
- Branding rules (naming, logos, terminology)
- Voice and tone (copy strings, error messages, UI text)
- UX principles (interaction patterns, accessibility, flow expectations)

Flag any deviation as a finding with severity appropriate to the impact.

## Step 7: Generate Report

Format findings with severity ratings and checkbox verification:

```
## Review Report: {track_description}

**Track**: {track_id}
**Commits**: {sha_list}
**Files changed**: {count}

### Summary
{1-2 sentence overall assessment}

## Verification Checks
- [ ] **Intent Match**: [Yes/No/Partial] - {comment}
- [ ] **Style Compliance**: [Pass/Fail] - {comment}
- [ ] **Test Coverage**: [Yes/No/Partial] - {comment}
- [ ] **Test Results**: [Passed/Failed] - {summary}
- [ ] **Security**: [Pass/Fail] - {comment}
- [ ] **Product Guidelines**: [Pass/Fail/N/A] - {comment}

### Intent Match
- [ok] {criterion met}
- [!] {criterion not fully met}: {explanation}

### Code Quality
{findings with severity: [!] critical, [?] suggestion}

For each violation include a diff block:
```diff
- old_code
+ new_code
```

### Test Coverage
{findings}

### Security
{findings}

### Product Guidelines
{findings, or "N/A -- no product-guidelines.md found"}

### Suggested Fixes
1. {fix description} -- {file}:{line}
   ```diff
   - old_code
   + new_code
   ```
2. {fix description} -- {file}:{line}

### Verdict
{PASS | PASS WITH NOTES | NEEDS CHANGES}
```

## Step 8: Auto-fix Option

If the verdict is PASS WITH NOTES or NEEDS CHANGES:

Ask the user: "Apply auto-fixes for the suggested changes?"
Options:
- **Yes, apply fixes** -- Make the suggested changes automatically
- **No, manual only** -- I'll handle fixes myself
- **Show me each fix** -- Review and approve each fix individually
- **Complete Track (ignore warnings)** -- Mark track complete without fixing warnings

If auto-fix: apply changes, run tests, commit:
```bash
git add {changed_files}
git commit -m "fix(review): apply review fixes for track {track_id}"
```

After committing, capture the new commit SHA and update `plan.md` with a new section:

```markdown
## Review Fixes

| Fix | Commit |
|-----|--------|
| {fix_description} | {commit_sha} |
```

Write this section to `.maestro/tracks/{track_id}/plan.md` appended after the existing content.

## Step 9: Post-Review Cleanup

After the review is complete (verdict delivered and any fixes applied), offer cleanup options:

Ask the user: "Review complete. What would you like to do with this track?"
Options:
- **Archive** -- Move track to .maestro/archive/
- **Delete** -- Remove track files entirely
- **Keep** -- Leave track as-is for further work
- **Skip** -- Do nothing

- **Archive**: Move `.maestro/tracks/{track_id}/` to `.maestro/archive/{track_id}/`
- **Delete**: Remove `.maestro/tracks/{track_id}/` entirely
- **Keep** / **Skip**: No file changes

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- **You are here.** Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed

Review works best after commits are made, as it analyzes git history to understand what was implemented. It compares the implementation against the spec from `/maestro:new-track` and the plan from `/maestro:implement`. If issues are found, use `/maestro:revert` to undo and re-implement, or apply fixes directly.

Remember: Good validation catches issues before they reach production. Be constructive but thorough in identifying gaps or improvements.
