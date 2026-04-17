# Pre-Implementation Readiness Gate

Lightweight validation checklist run after plan generation (step 13). Quick automated pass/fail -- not a separate workflow.

## Checks

### 1. FR Coverage
- Parse all FR-N references from spec.md
- Parse all "Addresses: FR-N" lines from plan.md
- Every FR MUST appear in at least one task
- Output: list of covered FRs, list of orphaned FRs

### 2. Acceptance Criteria Coverage
- Each acceptance criterion in spec.md MUST map to at least one verifiable task or phase verification step in plan.md
- Output: list of covered ACs, list of unaddressed ACs

### 3. Domain Requirements Coverage
- If spec.md has a "Domain Requirements" section, verify each requirement is addressed by a task or NFR
- Output: list of covered domain reqs, list of gaps

### 4. Dependency Sanity
- No circular phase dependencies
- Infrastructure/foundation tasks appear before consumers
- No task references a phase that comes after it
- Output: PASS or list of dependency issues

### 5. Scope Alignment
- Plan tasks MUST only cover MVP scope items
- Growth and Vision items from spec MUST NOT appear in plan tasks
- Output: PASS or list of scope creep items

## Output Format

When all checks pass:

```
## Readiness Gate Results

[ok] FR Coverage: {covered}/{total} FRs addressed
[ok] Acceptance Criteria: {covered}/{total} ACs mapped
[ok] Domain Requirements: {covered}/{total} addressed (or N/A)
[ok] Dependency Sanity: PASS
[ok] Scope Alignment: PASS

--> READY: Proceed to metadata and commit.
```

When gaps are found:

```
## Readiness Gate Results

[ok] FR Coverage: 8/10 FRs addressed
[!] GAPS: FR-7 (password reset flow), FR-9 (audit logging) have no tasks
[ok] Acceptance Criteria: 5/5 ACs mapped
[ok] Domain Requirements: N/A
[ok] Dependency Sanity: PASS
[!] Scope Alignment: Task 3.2 addresses "Growth" scope item "analytics dashboard"

--> GAPS FOUND: Resolve the above before proceeding.
```

## Handling Gaps

Present gaps to user with these options:

1. **Add missing tasks to plan** -- Create new tasks in plan.md to cover orphaned FRs or ACs
2. **Remove orphaned items from spec** -- Delete FRs or ACs that are intentionally deferred
3. **Accept gaps and proceed** -- Note gaps in metadata and continue anyway

If user chooses to fix: update plan.md or spec.md accordingly, then re-run the gate.
If user accepts gaps: record them in metadata.json and proceed to step 14.
