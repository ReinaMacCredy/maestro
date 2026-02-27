# Step 8: Functional Requirements Synthesis

**Progress: Step 8 of 14** -- Next: Non-Functional Requirements

## Goal
Synthesize functional requirements from ALL prior discovery (vision, journeys, domain). Group by capability area. Each FR must be implementation-agnostic and testable. Typical range: 10-40 FRs.

## Execution Rules
- You MUST synthesize from prior steps, not generate from scratch
- Every FR MUST follow the format: "FR-N: {Actor} can {capability}"
- Every FR MUST be testable (if you can't write a test for it, rewrite it)
- FRs MUST be implementation-agnostic (no technology choices, no "use React", no "store in PostgreSQL")
- Group FRs by capability area (authentication, data management, reporting, etc.)
- Number sequentially across all groups: FR-1, FR-2, ... FR-N (not per-group)

## Context Boundaries
- All prior discovery available: vision, success criteria, scope, journeys (with capability hints), domain requirements
- Journey Requirements Summary from step 6 is the primary input -- it lists capabilities surfaced by user journeys
- NFRs are NOT yet defined (step 9)

## Synthesis Sequence

1. **Gather Capability Hints**
   Review the Journey Requirements Summary from step 6. This lists capability areas surfaced by user journeys. These are your starting groups.

2. **Cross-Reference All Sources**
   For each capability area, pull requirements from:
   - User journeys (what users need to do)
   - Vision & scope (what's in MVP vs deferred)
   - Domain requirements (compliance-driven features)
   - Track description (original intent)

   Only include FRs that fall within MVP scope. Tag Growth/Vision items separately if user wants to capture them.

3. **Draft FR List**
   Group by capability area. Sequential numbering across groups.

   Example:
   ```
   ### Functional Requirements

   #### Authentication & Authorization
   - FR-1: User can register with email and password
   - FR-2: User can authenticate via OAuth provider
   - FR-3: Admin can assign role-based permissions

   #### Data Management
   - FR-4: User can create and edit records
   - FR-5: User can export records as CSV
   - FR-6: System can validate data against schema on save
   ```

4. **Testability Check**
   For each FR, verify: "Can I write a test that proves this works?" If not, rewrite.

   Bad: "FR-7: System handles errors gracefully" (untestable)
   Good: "FR-7: System returns structured error response with code and message when validation fails" (testable)

5. **Scope Check**
   If the user expressed interest in capturing Growth/Vision items, list them in a separate block after the MVP FRs:
   ```
   #### Growth/Vision (not in MVP)
   - GV-1: User can configure custom workflow automations (Growth)
   - GV-2: System can federate across multiple tenants (Vision)
   ```
   These are informational only -- they do NOT get FR numbers and are NOT included in the coverage matrix later.

6. **Present FR Summary**
   Show complete FR list grouped by capability area. Include total count.
   Ask: "Are there missing capabilities? Any FRs that should be split, merged, or removed?"

## Quality Checks
- [ok] FRs sourced from prior discovery (not invented from thin air)
- [ok] All capability areas from journey summary covered
- [ok] Every FR follows "{Actor} can {capability}" format
- [ok] Every FR is testable
- [ok] No implementation details in FRs
- [ok] Sequential numbering: FR-1 through FR-N
- [ok] Only MVP scope items (Growth/Vision tagged separately if included)

## Anti-patterns
- [x] Generating FRs without referencing journey capability hints
- [x] Vague FRs ("handle data properly") -- must be specific and testable
- [x] Including implementation details ("use JWT for auth") -- implementation-agnostic only
- [x] Per-group numbering (FR-Auth-1, FR-Data-1) -- use flat sequential numbers
- [x] Including Growth/Vision scope items as MVP FRs without flagging them
- [x] Inventing capability areas not grounded in any prior discovery step

## A/P/C Menu

Present: `[A] Advanced Elicitation  [P] Party Mode  [C] Continue to Non-Functional Requirements`

- **[A]**: Read `reference/elicitation-methods.md`. Suggest methods relevant to FR completeness (Pre-mortem and Inversion are strong here). Apply, show improvements, user accepts/rejects. Redisplay menu.
- **[P]**: Read `reference/party-mode.md`. Cycle perspectives on FRs. Dev and QA perspectives especially relevant (are FRs implementable and testable?). Present findings. User accepts/rejects. Redisplay menu.
- **[C]**: Proceed to next step.

## Next Step
Read and follow `reference/steps/step-09-nonfunctional.md`.
