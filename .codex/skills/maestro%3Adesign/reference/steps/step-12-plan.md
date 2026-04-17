# Step 12: Implementation Plan with Traceability

**Progress: Step 12 of 16** -- Next: Skill Detection

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
   - Small (1-3 files, 3-8 FRs): 1-2 phases, 1-3 tasks/phase
   - Medium (3-8 files, 8-15 FRs): 2-3 phases, 2-4 tasks/phase
   - Large (8-15 files, 15-25 FRs): 3-5 phases, 3-5 tasks/phase
   - XL (15+ files, 25+ FRs): 4-6 phases, 3-6 tasks/phase

   **If classification says "low complexity" but you generated 20+ FRs, something is wrong.** Re-examine: you likely over-specified. Combine FRs or demote to Growth tier.

4. **Map Design Decisions to Tasks**
   Before generating the plan, map each design artifact to its task role:

   | Design artifact | Task role | Example |
   |-----------------|-----------|---------|
   | FR-1: "User can log in with email/password" | Task with test + implementation | Test: `POST /auth/login` returns 200 with valid creds |
   | Domain constraint: "Passwords must be bcrypt-hashed" | `Must NOT do` guardrail | Must NOT store plaintext passwords |
   | NFR: "Login < 200ms p95" | Verify step | `ab -n 100 /auth/login` --> p95 < 200ms |
   | Journey error path: "Invalid password shows error" | Additional test case in the FR's task | Test: `POST /auth/login` returns 401 with wrong password |
   | Codebase pattern: "Auth middleware in src/middleware/" | File path in task | Create: `src/middleware/auth.ts` |
   | Out of scope: "No OAuth in MVP" | `Must NOT do` guardrail | Must NOT implement OAuth providers |

5. **Generate Plan**
   For each phase:
   - Group related FRs into tasks
   - Each task gets an "Addresses: FR-N, FR-M" line listing which FRs it covers
   - Apply TDD or ship-fast pattern from workflow.md
   - Include phase completion verification
   - Follow dependency rules (infrastructure first, no forward references)
   - Carry domain constraints and out-of-scope items into `Must NOT do` sections
   - Derive verification commands from NFRs

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

| Anti-pattern | Why it fails | Fix |
|--------------|-------------|-----|
| Tasks without FR traceability | Worker cannot verify they built the right thing | Every task gets `Addresses: FR-N` |
| Missing coverage matrix | Orphaned FRs slip through silently | Always append matrix after all phases |
| Ignoring codebase patterns | Worker invents new conventions that clash | Use step 11 findings for file paths and code style |
| Oversized phases (6+ tasks) | Phases lose meaning as testable increments | Split into 2-4 tasks per phase |
| Vague verification | "Check it works" is not executable | `bun test -- auth` --> all pass |
| Tasks that modify the same file without dependency | Merge conflicts when tasks run in parallel | Add explicit dependency or restructure |
| Not flagging orphaned FRs | Spec promises something the plan does not deliver | Coverage matrix must show 0 orphans |
| Writing plan to disk before approval | User has no chance to revise | Always present, get approval, then write |

## Next Step
Read and follow `reference/steps/step-13-skills.md`.
