---
name: maestro:review
description: "Code review for a track against its spec and plan. Verifies implementation matches requirements, checks code quality and security."
argument-hint: "[<track-name>] [--current]"
allowed-tools: Read, Glob, Grep, Bash, AskUserQuestion
disable-model-invocation: true
---

# Review -- Track Code Review

> Adapted from [Conductor](https://github.com/gemini-cli-extensions/conductor) for Claude Code.

Review the implementation of a track against its specification and plan. Verifies intent match, code quality, test coverage, and security.

CRITICAL: You must validate the success of every tool call. If any tool call fails, halt immediately and report the failure before proceeding.

When using AskUserQuestion, immediately call the tool -- do not repeat the question in plain text.

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

```
AskUserQuestion(
  questions: [{
    question: "Which track would you like to review?",
    header: "Review",
    options: [
      { label: "{track_1}", description: "{status} | {task_count} tasks" },
      { label: "{track_2}", description: "{status} | {task_count} tasks" }
    ],
    multiSelect: false
  }]
)
```

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

```
AskUserQuestion(
  questions: [{
    question: "This diff is {N} lines. Use Iterative Review Mode (per-file review)?",
    header: "Review Mode",
    options: [
      { label: "Yes", description: "Review each file separately for more focused feedback" },
      { label: "No", description: "Review the full diff at once" }
    ],
    multiSelect: false
  }]
)
```

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

```
AskUserQuestion(
  questions: [{
    question: "Apply auto-fixes for the suggested changes?",
    header: "Auto-fix",
    options: [
      { label: "Yes, apply fixes", description: "Make the suggested changes automatically" },
      { label: "No, manual only", description: "I'll handle fixes myself" },
      { label: "Show me each fix", description: "Review and approve each fix individually" },
      { label: "Complete Track (ignore warnings)", description: "Mark track complete without fixing warnings" }
    ],
    multiSelect: false
  }]
)
```

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

```
AskUserQuestion(
  questions: [{
    question: "Review complete. What would you like to do with this track?",
    header: "Track Cleanup",
    options: [
      { label: "Archive", description: "Move track to .maestro/archive/" },
      { label: "Delete", description: "Remove track files entirely" },
      { label: "Keep", description: "Leave track as-is for further work" },
      { label: "Skip", description: "Do nothing" }
    ],
    multiSelect: false
  }]
)
```

- **Archive**: Move `.maestro/tracks/{track_id}/` to `.maestro/archive/{track_id}/`
- **Delete**: Remove `.maestro/tracks/{track_id}/` entirely
- **Keep** / **Skip**: No file changes
