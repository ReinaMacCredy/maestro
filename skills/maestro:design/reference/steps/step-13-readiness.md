# Step 13: Pre-Implementation Readiness Gate

**Progress: Step 13 of 14** -- Next: Metadata & Commit

## Goal
Validate that the spec and plan are aligned and complete before committing. Quick automated pass/fail check -- not a separate workflow.

## Execution Rules
- You MUST run ALL checks from `reference/readiness-gate.md`
- This is automated validation, not a conversation -- run checks and report results
- If gaps found, present options to user before proceeding
- Do NOT skip this step even when things look complete

## Context Boundaries
- spec.md and plan.md both written to disk
- Readiness gate checklist at `reference/readiness-gate.md`

## Gate Sequence

1. **Read Gate Checklist**
   Read `reference/readiness-gate.md` for the validation checks.

2. **Run Checks**
   Execute each check against spec.md and plan.md:

   a. **FR Coverage**: Parse FR references from spec, parse Addresses lines from plan. Every FR must appear in at least one task.

   b. **Acceptance Criteria Coverage**: Each AC in spec should map to a verifiable task or phase verification in plan.

   c. **Domain Requirements Coverage**: If spec has Domain Requirements section, verify each is addressed.

   d. **Dependency Sanity**: No circular dependencies, infrastructure before consumers.

   e. **Scope Alignment**: Plan tasks only cover MVP items. Growth/Vision items should not appear in plan.

3. **Report Results**
   Use the output format from readiness-gate.md:
   ```
   ## Readiness Gate Results

   [ok] FR Coverage: {n}/{total} FRs addressed
   [ok] Acceptance Criteria: {n}/{total} ACs mapped
   [ok] Domain Requirements: {n}/{total} covered (or N/A)
   [ok] Dependency Sanity: no circular dependencies
   [ok] Scope Alignment: plan covers MVP only

   --> READY: Proceed to metadata and commit.
   ```

   Or if gaps found:
   ```
   [!] FR Coverage: {n}/{total} FRs addressed
       Missing: FR-7, FR-9

   --> GAPS FOUND: Resolve before proceeding.
   ```

4. **Handle Gaps**
   If gaps found, present options:
   - Add missing tasks to plan.md (re-run gate after)
   - Remove orphaned FRs from spec.md (re-run gate after)
   - Accept gaps and proceed anyway (note in metadata)

   If user chooses to fix: update the relevant file, re-run gate.
   If user accepts gaps: note them and proceed.

## Quality Checks
- [ok] All 5 checks executed
- [ok] Results clearly reported with counts
- [ok] Gaps actionable (specific items listed, not vague)
- [ok] Gate re-run after any fixes

## Anti-patterns
- [x] Skipping the gate -- always run it, even when things look complete
- [x] Reporting "all good" without actually parsing the files
- [x] Not offering resolution options when gaps found
- [x] Proceeding past gaps without user acknowledgment

## Next Step
Read and follow `reference/steps/step-14-commit.md`.
