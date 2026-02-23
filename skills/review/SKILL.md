---
name: review
description: "Performs code review with plan-to-implementation comparison and structured diff analysis, including auto-fix support. Use when validating quality and addressing review findings."
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Post-Execution Review

Perform a rigorous, structured comparison of the plan against the actual implementation. Every acceptance criterion gets a verdict. Scope compliance is checked. Issues found during review are automatically fixed when possible; complex issues are logged as TODOs.

## Step 1: Find and Load the Plan

Discover all plans from both active and archive directories:
```
Glob(".maestro/plans/*.md")
Glob(".maestro/archive/*.md")
```

- If **no plans exist in either directory**: Skip to **Planless Review Flow** below.
- If **exactly one plan** (from either directory): Load it.
- If **multiple plans**: Present a combined list to the user via `AskUserQuestion` and ask which plan to review. Include filenames, modification dates, and location context — mark plans from `.maestro/plans/` as **(active)** and plans from `.maestro/archive/` as **(archived)**. Load the selected plan.

Read the chosen plan file in full.

## Step 2: Parse All Plan Sections

Extract these sections from the plan:

1. **Objective** — The top-level goal statement
2. **Scope (In)** — Items explicitly in scope (bulleted list under `**In**:`)
3. **Scope (Out)** — Items explicitly out of scope (bulleted list under `**Out**:`)
4. **Tasks** — Each task with:
   - Task number and title (from `- [ ] Task N: Title` or `- [x] Task N: Title`)
   - Commit SHA (from `<!-- commit: {SHA} -->` annotation on the checkbox line, if present)
   - File path (from `**File**: \`path\``)
   - Description (from `**Description**:`)
   - Acceptance criteria (bulleted list under `**Acceptance criteria**:`)
   - Agent assignment (from `**Agent**:`)
5. **Verification** — Commands/checks listed in the Verification section
6. **Notes** — Any technical decisions, rollback strategy, research findings

If any section is missing, note it as `[SECTION NOT FOUND]` and continue.

## Step 3: Verify Each Task's Acceptance Criteria

For **each task** in the plan, check **every acceptance criterion individually**:

1. **Commit traceability**: If the task has a `<!-- commit: {SHA} -->` annotation, use `git show {SHA} --stat` to identify exactly which files were changed in that commit, and `git show {SHA}` to inspect the diff. This scopes your verification to the actual changes made for this task rather than searching the entire codebase.
2. **File existence**: Does the file referenced in `**File**:` exist? (Glob)
3. **Per-criterion check**: For each acceptance criterion:
   - If a commit SHA is available, prefer checking the diff from `git show {SHA}` for evidence
   - Otherwise, read the target file(s) and use Grep/Read to find evidence
   - Look for the specific patterns, functions, sections, or behaviors described
   - Mark as **PASS** (evidence found) or **FAIL** (no evidence or contradictory evidence)
   - Record the evidence (file path + line number or grep match) or the reason for failure

Do NOT just check file existence — verify the *content* matches each criterion.

## Step 4: Check Scope Compliance (Out-of-Scope)

For each item listed under `**Out**:` in the Scope section:

- Grep/Glob for files, patterns, or changes that suggest work was done on out-of-scope items
- If evidence of out-of-scope work is found, flag it as a **SCOPE VIOLATION**
- Record what was found and where

This catches scope creep — work that was explicitly excluded but got done anyway.

## Step 5: Check Scope Completeness (In-Scope)

For each item listed under `**In**:` in the Scope section:

- Look for implementation evidence: files created, functions written, config changed
- Cross-reference against the task list — is this in-scope item covered by at least one task?
- Mark as **COVERED** or **UNCOVERED**

This catches scope gaps — work that was promised but not delivered.

## Step 6: Run Verification Commands

Execute each verification command from the plan's `## Verification` section:

- Run the command via Bash
- Record the output
- Mark as **PASS** if the command succeeds (exit code 0 and output matches expectations) or **FAIL** if it fails
- For commands that test for content presence (e.g., grep), PASS means the content was found

If a verification command references tools or packages that aren't installed, mark as **SKIP** with explanation.

## Step 7: Check for Regressions

Look for and run the project's standard validation checks:

1. Check `package.json` for `test`, `build`, `lint`, `typecheck` scripts — run them if they exist
2. Check for `Makefile` — run relevant targets if present
3. Check for CI config (`.github/workflows/`, `.gitlab-ci.yml`) — note what CI would run
4. Check for project verification scripts and documented quality gates — run them when available

Record each result as **PASS**, **FAIL**, or **SKIP** (with reason).

## Step 8: Check Wisdom Extraction

Check if `/work` created a wisdom file for this execution:

```
Glob(".maestro/wisdom/*.md")
```

- Look for a wisdom file that corresponds to this plan (by name or timestamp)
- If found: **PASS** — note the file path
- If not found: **FAIL** — wisdom extraction was missed

## Step 9: Produce Structured Report

Generate the final report in this exact format:

```
## Review: {Plan Name}

**Plan**: `{plan file path}`
**Reviewed**: {current date}

---

### Objective
{State the objective from the plan}
**Met**: YES / PARTIAL / NO

---

### Task Completion

#### Task {N}: {Title}
**File**: `{path}`
**Commit**: `{SHA or "none"}`
**Status**: COMPLETE / PARTIAL / MISSING

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | {criterion text} | PASS/FAIL | {file:line or reason} |
| 2 | {criterion text} | PASS/FAIL | {file:line or reason} |

{Repeat for each task}

---

### Scope Compliance

**In-Scope Coverage:**
| Item | Status | Evidence |
|------|--------|----------|
| {in-scope item} | COVERED/UNCOVERED | {where implemented} |

**Out-of-Scope Violations:**
| Item | Status | Evidence |
|------|--------|----------|
| {out-scope item} | CLEAN/VIOLATION | {what was found} |

---

### Verification Results
| # | Check | Result | Output |
|---|-------|--------|--------|
| 1 | {verification command} | PASS/FAIL/SKIP | {summary} |

---

### Regression Check
| Check | Result | Output |
|-------|--------|--------|
| {check name} | PASS/FAIL/SKIP | {summary} |

---

### Wisdom Extraction
**Status**: PASS / FAIL
**File**: `{path or "none found"}`

---

### Remediation
{Only include this section if there are FAILs}

For each failure, provide a specific fix suggestion:
- **Task {N}, Criterion {M}**: {What needs to be done to fix it}
- **Scope violation — {item}**: {What to revert or how to address}
- **Verification — {check}**: {How to fix the failing check}

---

### Auto-Fix Results
{Populated by Step 9.5 — see below}

---

### Verdict: COMPLETE / NEEDS WORK / FAILED

**Summary**: {1-2 sentence summary}

- **COMPLETE**: All criteria PASS, scope is clean, verifications pass, wisdom extracted
- **NEEDS WORK**: Most criteria pass but some failures need attention
- **FAILED**: Majority of criteria fail or critical tasks are missing

{If NEEDS WORK or FAILED, list the specific items that need attention}
```

Populate every section. Do not skip sections — use "N/A" if a section has no applicable items. Be precise with evidence: include file paths and line numbers, not vague descriptions.

## Step 9.5: Auto-Fix

After generating the report, automatically fix issues that can be resolved with mechanical edits.

### Classify Findings

Review every FAIL finding from the report (Task Completion criteria, Scope violations, Verification failures). Classify each as:

- **Fixable**: Missing exports, wrong function/variable names, missing imports, incorrect config values, missing sections in markdown files, wrong file paths in references, minor logic errors with obvious corrections. The fix is unambiguous and localized to a few lines.
- **Complex (TODO)**: Missing feature implementations, architectural changes, new test files needed, design decisions required, multi-file refactors with unclear scope. The fix requires judgment or significant new code.

### Apply Fixes

For each **fixable** finding:

1. Read the target file using the path and line number from the report evidence
2. Apply the fix using `Edit(file_path, old_string, new_string)` — use the smallest possible edit
3. Record what was changed: `{file}:{line} — {description of fix}`

For each **complex** finding:

1. Do NOT attempt a fix
2. Record as TODO: `TODO: {Task N, Criterion M} — {what needs to be done}`

### Re-Run Verification

After all fixes are applied, re-run the verification commands from Step 6 and the regression checks from Step 7:

```
# Re-run plan verification commands
Bash("{each verification command from the plan}")

# Re-run project validation
Bash("{test/build/lint commands from Step 7}")
```

Record updated results.

### Update Report

Populate the **Auto-Fix Results** section in the report (the placeholder added in Step 9):

```
### Auto-Fix Results

**Fixed ({N} of {total FAIL count}):**
| # | Finding | File | Fix Applied |
|---|---------|------|-------------|
| 1 | {Task N, Criterion M or finding description} | `{file}:{line}` | {what was changed} |

**Unfixed — TODO ({M} remaining):**
| # | Finding | Reason |
|---|---------|--------|
| 1 | {Task N, Criterion M or finding description} | {why it couldn't be fixed inline} |

**Re-Verification:**
| # | Check | Before Fix | After Fix |
|---|-------|------------|-----------|
| 1 | {command or check name} | FAIL | PASS/FAIL |
```

If no FAILs were found in the report, populate the section with: `No issues to fix.`

### Recalculate Verdict

After fixes, update the verdict based on **remaining unfixed issues only**:

- **COMPLETE**: All FAILs were fixed (or none existed), re-verification passes
- **NEEDS WORK**: Some FAILs were fixed but TODOs remain
- **FAILED**: Critical FAILs could not be fixed, or re-verification still fails

Update the Verdict section at the bottom of the report to reflect post-fix state. Append a note:
`Auto-fix applied: {N} issues fixed, {M} remaining as TODO.`

#### Fix Escalation Loop

If re-verification still shows FAIL results after auto-fix:

1. **Cycle 1**: Re-read the failing files, apply a second round of targeted Edit fixes based on the error output
2. **Cycle 2**: If still failing, use Bash to run the failing command, capture full error output, and apply fixes based on the complete error context
3. **Cycle 3**: If still failing, stop and report remaining failures as TODOs

**Exit conditions**:
- All FAILs resolved → update verdict to COMPLETE/NEEDS WORK
- Same failure 3 cycles → stop, mark as TODO
- Max 3 cycles total (including the initial auto-fix attempt)

Update the final verdict and append: `Auto-fix escalation: {N} additional issues fixed in {M} cycles, {R} remaining as TODO.`

## Step 10: Post-Review Archival

If the verdict is **COMPLETE** and the plan was loaded from `.maestro/plans/` (not already in `.maestro/archive/`):

1. Create the archive directory if it doesn't exist:
   ```
   mkdir -p .maestro/archive/
   ```
2. Move the plan to archive:
   ```
   mv .maestro/plans/{name}.md .maestro/archive/{name}.md
   ```
3. Report: "Plan archived to `.maestro/archive/{name}.md`"

If the plan is already in `.maestro/archive/`, or the verdict is not COMPLETE, do nothing.

### Post-Review Knowledge Capture (Auto)

**Trigger**: Review found >= 2 FAIL findings (skip for clean reviews).

Auto-extract review insights as learned skills:
1. Collect all FAIL findings from the review report
2. Group by pattern (e.g., "missing exports", "untested edge case", "hardcoded secret")
3. For each pattern that appeared 2+ times, apply learner quality gates (non-Googleable, context-specific, actionable, hard-won)
4. Passing patterns → save to `.claude/skills/learned/{slug}.md` with triggers matching the pattern keywords
5. Single-occurrence FAILs → skip (not yet a pattern)

This ensures recurring review failures get codified as skills that future agents will see, breaking the cycle of repeated mistakes.

---

## Planless Review Flow

When no plan is found, perform a structured code review based on git changes.

### Step P1: Determine Diff Scope

Identify what changes to review by checking the git state:

1. **Check if on a feature branch** (not `main` or `master`):
   ```
   Bash("git rev-parse --abbrev-ref HEAD")
   ```
   - If on a feature branch: diff against the base branch (`main` or `master`)
     ```
     Bash("git diff main...HEAD --name-only")
     ```
   - If on `main`/`master`: diff the most recent commit(s) with uncommitted changes
     ```
     Bash("git diff HEAD --name-only")
     ```
     If no uncommitted changes, diff the last commit:
     ```
     Bash("git diff HEAD~1 --name-only")
     ```

2. **If no changes are found at all**: Report "No changes detected. Nothing to review." and stop.

3. **Collect the full diff** for the determined scope:
   ```
   Bash("git diff main...HEAD")  # or the appropriate diff command from above
   ```

4. **List changed files** with their change type (added/modified/deleted):
   ```
   Bash("git diff main...HEAD --name-status")
   ```

Record the diff scope (branch name, commit range, or "uncommitted") for the report header.

### Step P2: Code Quality Review

For each changed file (skip deleted files), read the file and the relevant diff hunks. Check for:

1. **Naming** — Are variables, functions, classes named clearly and consistently with the project's conventions?
2. **Structure** — Is the code well-organized? Are functions/methods a reasonable size? Is there unnecessary nesting?
3. **Readability** — Would another developer understand this code without excessive comments? Are complex sections documented?
4. **Duplication** — Is there copy-pasted code that should be extracted? Use Grep to check for similar patterns in the codebase.
5. **Dead code** — Are there unused imports, variables, or functions introduced in the diff?

For each file, read it:
```
Read("{file_path}")
```

Record findings as a list of `{file, line, dimension, severity, description}` tuples. Severity levels:
- **FAIL** — Must fix: bugs, broken logic, clear violations
- **WARN** — Should fix: poor naming, unnecessary complexity, mild duplication
- **INFO** — Consider: style suggestions, minor improvements

### Step P3: Security Review (Auto-Delegated)

Spawn the `security-reviewer` agent for deep security analysis on the diff:

```
Task(
  description: "Security review for code review",
  subagent_type: "security-reviewer",
  model: "sonnet",
  prompt: |
    Analyze the following diff for security vulnerabilities:

    {diff from Step P1}

    Check: auth/authz, input validation, secrets exposure, injection risks (SQL, XSS, command), dependency security, sensitive data logging, error message leaking.

    Run ecosystem audit if applicable (bun audit / pip-audit / govulncheck).

    Report each finding with: severity (Critical/High/Medium/Low), file:line, description, recommendation.
)
```

Map security-reviewer findings to review dimensions:
- **Critical/High** → FAIL severity in review report
- **Medium** → WARN severity
- **Low** → INFO severity

All findings go into the review's "Findings by File" table with Dimension = "Security".

### Step P4: Test Coverage Review

For each changed file that contains implementation code (not test files, configs, or docs):

1. **Identify the expected test file** — Based on project conventions, determine where tests should live:
   ```
   Glob("**/*test*{file_basename}*")
   Glob("**/*{file_basename}*test*")
   Glob("**/__tests__/{file_basename}*")
   Glob("**/test_*{file_basename_no_ext}*")
   ```

2. **Check test existence** — Does a corresponding test file exist?
   - If yes: Read it and check if the changed/new functions are covered
   - If no: Flag as WARN ("No test file found for {file}")

3. **Check for test changes in the diff** — Were test files modified as part of this change?
   - New functions/methods added without corresponding test additions = WARN
   - Bug fixes without regression tests = WARN
   - Pure refactors with passing existing tests = INFO (acceptable)

Record findings:
- **FAIL** — New public API or critical logic with zero test coverage
- **WARN** — Changed logic without updated tests, or missing test file
- **INFO** — Test coverage exists but could be more thorough

### Step P5: Regression Check

Run the project's standard validation checks (same as plan-based Step 7):

1. Check `package.json` for `test`, `build`, `lint`, `typecheck` scripts — run them if they exist
2. Check for `Makefile` — run relevant targets if present
3. Check for CI config (`.github/workflows/`, `.gitlab-ci.yml`) — note what CI would run
4. Check for project verification scripts and documented quality gates — run them when available

Record each result as **PASS**, **FAIL**, or **SKIP** (with reason).

### Step P6: Commit Hygiene Review

If reviewing a branch with multiple commits, examine commit quality:

```
Bash("git log main..HEAD --oneline")
```

Check for:

1. **Atomic commits** — Does each commit represent a single logical change? Flag commits that mix unrelated changes (e.g., feature + formatting)
2. **Commit messages** — Are messages descriptive? Flag generic messages like "fix", "update", "wip", "asdf"
3. **Debug artifacts** — Check the diff for leftover `console.log`, `debugger`, `print()`, `TODO/FIXME` that appear to be temporary:
   ```
   Grep("(console\\.log|debugger|print\\(|TODO|FIXME|HACK|XXX)", changed_files)
   ```
4. **Large files** — Flag any single file change exceeding 500 lines (may need splitting)
5. **Sensitive files** — Flag changes to `.env`, credentials, or config files that might contain secrets

Record findings:
- **FAIL** — Debug artifacts in production code, secrets committed
- **WARN** — Poor commit messages, non-atomic commits, large changes
- **INFO** — Minor style observations

### Step P7: Produce Planless Review Report

Generate the report in this exact format:

````
## Code Review: {branch name or "uncommitted changes"}

**Scope**: `{diff description, e.g., "feature-branch vs. main (12 commits, 8 files)"}`
**Reviewed**: {current date}

---

### Changed Files

| # | File | Status | Findings |
|---|------|--------|----------|
| 1 | `{path}` | Added/Modified/Deleted | {count} FAIL, {count} WARN, {count} INFO |

---

### Findings by File

#### `{file_path}`

| # | Line | Dimension | Severity | Finding |
|---|------|-----------|----------|---------|
| 1 | {line} | Quality/Security/Tests/Hygiene | FAIL/WARN/INFO | {description} |

{Repeat for each file with findings}

---

### Regression Check

| Check | Result | Output |
|-------|--------|--------|
| {check name} | PASS/FAIL/SKIP | {summary} |

---

### Summary

| Dimension | FAIL | WARN | INFO |
|-----------|------|------|------|
| Code Quality | {n} | {n} | {n} |
| Security | {n} | {n} | {n} |
| Test Coverage | {n} | {n} | {n} |
| Commit Hygiene | {n} | {n} | {n} |
| **Total** | **{n}** | **{n}** | **{n}** |

---

### Auto-Fix Results
{Populated by Step P7.5 — see below}

---

### Verdict: CLEAN / NEEDS WORK / FAILED

**Summary**: {1-2 sentence summary}

- **CLEAN**: No FAILs, few or no WARNs, regressions pass
- **NEEDS WORK**: No FAILs but multiple WARNs that should be addressed
- **FAILED**: One or more FAILs that must be fixed before merging

{If NEEDS WORK or FAILED, list the specific items that need attention}
````

Populate every section. If a file has no findings, omit it from "Findings by File" but keep it in the "Changed Files" table with "0 findings". Be precise with line numbers.

### Step P7.5: Auto-Fix

After generating the report, automatically fix issues that can be resolved with mechanical edits.

#### Classify Findings

Review every FAIL finding from the report (Code Quality, Security, Test Coverage, Commit Hygiene dimensions). Classify each as:

- **Fixable**: Unused imports, missing exports, dead code flagged for removal, debug artifacts (`console.log`, `debugger`, `print()`), trivial naming fixes, missing type annotations with obvious types. The fix is unambiguous and localized to a few lines.
- **Complex (TODO)**: Missing test files, architectural issues, security vulnerabilities requiring design changes, large refactors. The fix requires judgment or significant new code.

Also review WARN findings — apply the same classification. Fix WARNs that are mechanical (e.g., removing a `console.log`), skip WARNs that require judgment.

#### Apply Fixes

For each **fixable** finding:

1. Read the target file using the path and line number from the report evidence
2. Apply the fix using `Edit(file_path, old_string, new_string)` — use the smallest possible edit
3. Record what was changed: `{file}:{line} — {description of fix}`

For each **complex** finding:

1. Do NOT attempt a fix
2. Record as TODO: `TODO: {Dimension, file:line} — {what needs to be done}`

#### Re-Run Regression Check

After all fixes are applied, re-run the regression checks from Step P5:

```
# Re-run project validation
Bash("{test/build/lint commands from Step P5}")
```

If a regression check that previously passed now fails after a fix:

1. **Revert the fix** that caused the regression using `Edit` to restore the original code
2. Reclassify that finding as **Complex (TODO)** with note: `Reverted — caused regression in {check name}`
3. Re-run the failing check to confirm the revert resolved it

Record updated results.

#### Update Report

Populate the **Auto-Fix Results** section in the report (the placeholder added in Step P7):

```
### Auto-Fix Results

**Fixed ({N} of {total FAIL+WARN count}):**
| # | Finding | File | Fix Applied |
|---|---------|------|-------------|
| 1 | {Dimension: description} | `{file}:{line}` | {what was changed} |

**Unfixed — TODO ({M} remaining):**
| # | Finding | Reason |
|---|---------|--------|
| 1 | {Dimension: description} | {why it couldn't be fixed inline} |

**Re-Verification:**
| # | Check | Before Fix | After Fix |
|---|-------|------------|-----------|
| 1 | {check name} | PASS/FAIL | PASS/FAIL |
```

If no FAILs or fixable WARNs were found in the report, populate the section with: `No issues to fix.`

#### Recalculate Verdict

After fixes, update the verdict based on **remaining unfixed issues only**:

- **CLEAN**: All FAILs were fixed (or none existed), no WARNs remain, regressions pass
- **NEEDS WORK**: All FAILs were fixed but WARNs or TODOs remain
- **FAILED**: FAILs could not be fixed, or re-verification still fails

Update the Verdict section at the bottom of the report to reflect post-fix state. Append a note:
`Auto-fix applied: {N} issues fixed, {M} remaining as TODO.`

#### Fix Escalation Loop

If re-verification still shows FAIL results after auto-fix:

1. **Cycle 1**: Re-read the failing files, apply a second round of targeted Edit fixes based on the error output
2. **Cycle 2**: If still failing, use Bash to run the failing command, capture full error output, and apply fixes based on the complete error context
3. **Cycle 3**: If still failing, stop and report remaining failures as TODOs

**Exit conditions**:
- All FAILs resolved → update verdict to CLEAN/NEEDS WORK
- Same failure 3 cycles → stop, mark as TODO
- Max 3 cycles total (including the initial auto-fix attempt)

Update the final verdict and append: `Auto-fix escalation: {N} additional issues fixed in {M} cycles, {R} remaining as TODO.`

### Post-Review Knowledge Capture (Auto)

**Trigger**: Review found >= 2 FAIL findings (skip for clean reviews).

Auto-extract review insights as learned skills:
1. Collect all FAIL findings from the review report
2. Group by pattern (e.g., "missing exports", "untested edge case", "hardcoded secret")
3. For each pattern that appeared 2+ times, apply learner quality gates (non-Googleable, context-specific, actionable, hard-won)
4. Passing patterns → save to `.claude/skills/learned/{slug}.md` with triggers matching the pattern keywords
5. Single-occurrence FAILs → skip (not yet a pattern)

This ensures recurring review failures get codified as skills that future agents will see, breaking the cycle of repeated mistakes.
