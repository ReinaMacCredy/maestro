# Step 12: Implementation Plan with Traceability

**Progress: Step 12 of 14** -- Next: Readiness Gate

## Goal
Generate the implementation plan using enriched template with FR traceability. Each task references which FRs it addresses. Append Requirements Coverage Matrix. Present for approval.

## Execution Rules
- You MUST use `reference/plan-template.md` as the structure
- Every task MUST have an "Addresses: FR-N, FR-M" line
- You MUST generate a Requirements Coverage Matrix at the end
- Flag any orphaned FRs (in spec but not addressed by any task)
- Read project context files for informed planning
- Max 3 revision loops for plan approval
- Do NOT proceed until user explicitly approves

## Context Boundaries
- Approved spec at `.maestro/tracks/{track_id}/spec.md`
- Codebase patterns from step 11
- Project context: `.maestro/context/workflow.md`, `.maestro/context/tech-stack.md`, `.maestro/context/guidelines.md`
- Plan template at `reference/plan-template.md`

## Plan Generation Sequence

1. **Read Context**
   Read these files if they exist (skip gracefully if missing):
   - `.maestro/context/workflow.md` -- determines TDD vs ship-fast pattern
   - `.maestro/context/tech-stack.md` -- informs technology choices in tasks
   - `.maestro/context/guidelines.md` -- coding standards and conventions

   Combine with codebase patterns from step 11.

2. **Read Template**
   Read `reference/plan-template.md` for structure, TDD pattern injection, sizing guidelines, and dependency rules.

3. **Determine Sizing**
   Estimate scope based on FR count and complexity:
   - Medium (3-8 files, 10-15 FRs): 2-3 phases, 2-4 tasks/phase
   - Large (8-15 files, 15-25 FRs): 3-5 phases, 3-5 tasks/phase
   - XL (15+ files, 25+ FRs): 4-6 phases, 3-6 tasks/phase

4. **Generate Plan**
   For each phase:
   - Group related FRs into tasks
   - Each task gets an "Addresses: FR-N, FR-M" line listing which FRs it covers
   - Apply TDD or ship-fast pattern from workflow.md
   - Include phase completion verification
   - Follow dependency rules (infrastructure first, no forward references)

5. **Generate Coverage Matrix**
   After all phases, append:
   ```
   ## Requirements Coverage Matrix

   | FR | Description | Task(s) | Status |
   |----|------------|---------|--------|
   | FR-1 | {desc} | 1.1, 2.3 | Covered |
   | FR-2 | {desc} | 1.2 | Covered |
   | FR-3 | {desc} | -- | [!] ORPHANED |
   ```

   Check every FR from spec against all task Addresses lines. Flag orphans.

6. **Present Plan**
   Show the complete plan including coverage matrix.
   Ask: "Review this implementation plan. Does the phasing and task breakdown make sense?"

   Options:
   - **Approved** -- Plan is ready
   - **Needs revision** -- I'll tell you what to change

   If orphaned FRs exist, call them out explicitly: "Note: FR-{N} is not addressed by any task. Should I add a task or remove the FR from spec?"

7. **Handle Revisions**
   If revision requested:
   - Ask what specifically needs to change
   - Make targeted updates (do not regenerate the whole plan)
   - Re-present updated sections plus coverage matrix
   - Max 3 revision loops. After 3, ask user to approve current version or provide final edits.

8. **Write Plan**
   Once approved, write to `.maestro/tracks/{track_id}/plan.md`.
   Confirm: "Plan written to `.maestro/tracks/{track_id}/plan.md`."

## Quality Checks
- [ok] Every task has "Addresses: FR-N" line
- [ok] Coverage matrix included and accurate
- [ok] No orphaned FRs (or orphans explicitly acknowledged by user)
- [ok] Phases follow dependency rules (infrastructure first, no forward references)
- [ok] Sizing appropriate for FR count
- [ok] TDD/ship-fast pattern applied per workflow.md
- [ok] User explicitly approved

## Anti-patterns
- [x] Tasks without FR traceability -- every task must reference at least one FR
- [x] Missing coverage matrix -- must always be appended
- [x] Ignoring codebase patterns from step 11 -- plan should build on existing conventions
- [x] Oversized phases (6+ tasks per phase) -- split into smaller phases
- [x] Not flagging orphaned FRs
- [x] Writing plan to disk before user approval

## Next Step
Read and follow `reference/steps/step-13-readiness.md`.
