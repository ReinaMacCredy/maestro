# Step 10: Spec Draft & Approval

**Progress: Step 10 of 16** -- Next: Codebase Pattern Scan

## Goal
Compose the complete specification from all discovery steps (4-9), present for approval, and write to disk.

## Execution Rules
- You MUST use `reference/spec-template.md` as the structure
- Compose by assembling content from steps 4-9 -- do NOT regenerate from scratch
- Present the FULL spec for review (not a summary)
- Max 3 revision loops
- Do NOT proceed until the user explicitly approves

## Context Boundaries
- All discovery content from steps 4-9 is available
- This step assembles and presents -- it does not generate new content
- Spec template at `reference/spec-template.md`

## Composition Sequence

1. **Read Template**
   Read `reference/spec-template.md` for the target structure.

2. **Assemble Spec**
   Map discovery outputs to template sections:
   - Overview: synthesize from track description and vision
   - Type: from step 2
   - Vision & Differentiator: from step 5
   - Success Criteria: from step 5
   - Product Scope: from step 5
   - User Journeys: from step 6
   - Domain Requirements: from step 7 (omit section if step was skipped)
   - Functional Requirements: from step 8
   - User Interaction: synthesize from journeys and FRs
   - Non-Functional Requirements: from step 9
   - Edge Cases: synthesize from journeys (error paths) and domain risks
   - Out of Scope: from Growth/Vision items explicitly deferred
   - Acceptance Criteria: derive from FRs and success criteria, reference FR-N numbers

3. **Present Full Spec**
   Show the complete spec. Ask: "Review this specification. Does it accurately capture what we discussed?"

   Options:
   - **Approved** -- Spec is ready
   - **Needs revision** -- I'll tell you what to change
   - **[A] Advanced Elicitation** -- stress-test the full spec
   - **[P] Party Mode** -- multi-perspective review of full spec

4. **Handle Revisions**
   If revision requested:
   - Ask what specifically needs to change
   - Make targeted updates (do not regenerate the whole spec)
   - Re-present the updated sections plus surrounding context
   - Max 3 revision loops. After 3, ask user to approve current version or provide final edits.

5. **Handle A/P**
   - **[A]**: Read `reference/elicitation-methods.md`. Suggest 3-5 methods for the full spec. User picks, apply, show improvements, user accepts/rejects. Return to approval prompt.
   - **[P]**: Read `reference/party-mode.md`. Run full 5-perspective review on complete spec. Present consolidated findings. User accepts/rejects. Return to approval prompt.

6. **Write Spec**
   Once approved, write the spec to `.maestro/tracks/{track_id}/spec.md`.
   Confirm: "Spec written to `.maestro/tracks/{track_id}/spec.md`."

## Quality Checks
- [ok] All template sections populated from discovery
- [ok] Full spec presented (not summary)
- [ok] User explicitly approved
- [ok] Acceptance criteria reference FR-N numbers
- [ok] File written to correct path

## Self-Review Before Presenting

Before showing the spec to the user, verify these internally:

| Check | How to verify |
|-------|---------------|
| Every FR is testable | FR says what happens, not "handles" or "supports" |
| No FR is a design decision | FRs say WHAT, not HOW. "User can log in" not "Use JWT for auth" |
| Success criteria have numbers | "< 200ms p95" not "fast". "99.9% uptime" not "reliable" |
| MVP scope is small | 3-8 FRs for simple tracks, 8-15 for complex. More than 15 MVP FRs = over-specification |
| Edge cases come from journeys | Error paths from step 6, not invented in isolation |
| Out of scope is explicit | At least 2-3 items. If nothing is out of scope, the scope is too vague |

If any check fails, fix the spec before presenting. Do not ask the user to fix your assembly errors.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|--------------|-------------|-----|
| Regenerating from scratch | Ignores the nuance from discovery steps 4-9 | Assemble from existing outputs |
| Presenting a summary | User cannot review what they cannot see | Show the full spec, every section |
| Skipping AC derivation | Acceptance criteria are the bridge to plan tasks | Derive ACs from FRs + success criteria |
| Not offering A/P options | User misses the stress-test opportunity | Always show `[A] [P] [C]` menu |
| Writing before approval | User cannot revise what is already committed | Present, get explicit approval, then write |
| Over-specified FRs | 25 FRs for a CRUD feature signals over-engineering | Match FR count to classification complexity |

## Next Step
Read and follow `reference/steps/step-11-codebase-scan.md`.
