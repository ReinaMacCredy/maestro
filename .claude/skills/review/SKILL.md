---
name: review
description: Post-execution review — rigorous plan-vs-implementation comparison with per-criterion verdicts.
allowed-tools: Read, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Post-Execution Review

Perform a rigorous, structured comparison of the plan against the actual implementation. Every acceptance criterion gets a verdict. Scope compliance is checked. Remediation is suggested for failures.

## Step 1: Find and Load the Plan

Discover all plans from both active and archive directories:
```
Glob(".maestro/plans/*.md")
Glob(".maestro/archive/*.md")
```

- If **no plans exist in either directory**: Report "No plans found in `.maestro/plans/` or `.maestro/archive/`. Run /design first." and stop.
- If **exactly one plan** (from either directory): Load it.
- If **multiple plans**: Present a combined list to the user via `AskUserQuestion` and ask which plan to review. Include filenames, modification dates, and location context — mark plans from `.maestro/plans/` as **(active)** and plans from `.maestro/archive/` as **(archived)**. Load the selected plan.

Read the chosen plan file in full.

## Step 2: Parse All Plan Sections

Extract these sections from the plan:

1. **Objective** — The top-level goal statement
2. **Scope (In)** — Items explicitly in scope (bulleted list under `**In**:`)
3. **Scope (Out)** — Items explicitly out of scope (bulleted list under `**Out**:`)
4. **Tasks** — Each task with:
   - Task number and title (from `- [ ] Task N: Title`)
   - File path (from `**File**: \`path\``)
   - Description (from `**Description**:`)
   - Acceptance criteria (bulleted list under `**Acceptance criteria**:`)
   - Agent assignment (from `**Agent**:`)
5. **Verification** — Commands/checks listed in the Verification section
6. **Notes** — Any technical decisions, rollback strategy, research findings

If any section is missing, note it as `[SECTION NOT FOUND]` and continue.

## Step 3: Verify Each Task's Acceptance Criteria

For **each task** in the plan, check **every acceptance criterion individually**:

1. **File existence**: Does the file referenced in `**File**:` exist? (Glob)
2. **Per-criterion check**: For each acceptance criterion:
   - Read the target file(s)
   - Use Grep/Read to find evidence that the criterion is satisfied
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
4. Check for validation scripts (e.g., `scripts/validate-*.sh`) — run them

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

### Verdict: COMPLETE / NEEDS WORK / FAILED

**Summary**: {1-2 sentence summary}

- **COMPLETE**: All criteria PASS, scope is clean, verifications pass, wisdom extracted
- **NEEDS WORK**: Most criteria pass but some failures need attention
- **FAILED**: Majority of criteria fail or critical tasks are missing

{If NEEDS WORK or FAILED, list the specific items that need attention}
```

Populate every section. Do not skip sections — use "N/A" if a section has no applicable items. Be precise with evidence: include file paths and line numbers, not vague descriptions.

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
