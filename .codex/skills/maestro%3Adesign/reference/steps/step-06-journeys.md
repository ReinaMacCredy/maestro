# Step 6: User Journey Mapping

**Progress: Step 6 of 16** -- Next: Domain & Scoping

## Goal
Map ALL user types that interact with the system using narrative story-based journeys. Minimum 3 journeys. Extract capability hints that feed into functional requirements (step 8).

## Execution Rules
- You MUST identify ALL user types, not just the primary user.
- Every journey MUST use narrative structure (opening scene, rising action, climax, resolution).
- You MUST connect journey discoveries to capability areas.
- Minimum 3 journeys required before proceeding.

## Context Boundaries
- Product context, track description, classification, vision, and scope available.
- Functional requirements are NOT yet synthesized (that is step 8) -- but journeys REVEAL them.
- Domain requirements are NOT yet defined (step 7).

## Discovery Sequence

1. **Identify User Types**
   Start broad:
   - "Who are ALL the people who interact with this system? Think beyond the primary user."
   - Prompt for: admins, moderators, support staff, API consumers, internal ops, other personas.
   - Based on project type from classification, suggest relevant user types:
     - API: developers, API consumers, admin dashboard users
     - Web app: end users, admins, content managers
     - CLI: developers, CI/CD pipelines, power users
     - SaaS: tenant admins, end users, super admins, billing managers

2. **Map Journeys**
   For each significant user type, facilitate a narrative journey:

   **Story Framework:**
   - Name the persona (realistic name, brief context).
   - **Opening Scene**: Where/how do we meet them? What is their current pain?
   - **Rising Action**: What steps do they take? What do they discover?
   - **Climax**: The moment where the product delivers real value.
   - **Resolution**: How their situation improves. The new reality.

   Guide with questions:
   - "Walk me through a typical session for {persona}. What triggers them to use this?"
   - "What is the most critical moment in that journey? Where does value appear?"
   - "What could go wrong? What is the recovery path?"

3. **Extract Capability Hints**
   After each journey, explicitly note:
   - "This journey reveals needs for: {capability area 1}, {capability area 2}"
   - Build a running list of capabilities surfaced across all journeys.
   - These feed directly into FR synthesis in step 8.

4. **Check Coverage**
   Before presenting:
   - Primary user happy path covered?
   - Primary user error/edge case covered?
   - At least one admin/ops journey?
   - API consumer journey (if applicable)?
   - If fewer than 3 journeys, prompt for more.

5. **Present Journey Summary**
   Show all journeys plus the capability hint list:
   ```
   ## User Journeys

   ### Journey 1: {Persona} -- {Scenario}
   {narrative}

   ### Journey 2: {Persona} -- {Scenario}
   {narrative}

   ### Journey Requirements Summary
   Capabilities revealed:
   - {Capability area}: surfaced by Journey 1, 3
   - {Capability area}: surfaced by Journey 2
   ```

## Quality Checks
- [ok] All user types identified (not just primary)
- [ok] Minimum 3 narrative journeys mapped
- [ok] Each journey has full story arc (opening, rising action, climax, resolution)
- [ok] Capability hints extracted from each journey
- [ok] Error/edge case scenarios included in at least one journey

## Anti-patterns
- [x] Only mapping the primary user's happy path
- [x] Bullet-point journeys instead of narratives (story structure matters for empathy)
- [x] Not extracting capability hints -- journeys must feed into requirements
- [x] Treating admin/ops users as an afterthought

## A/P/C Menu

Present: `[A] Advanced Elicitation  [P] Party Mode  [C] Continue to Domain & Scoping`

- **[A]**: Read `reference/elicitation-methods.md`. Suggest methods relevant to journey coverage (Pre-mortem and Inversion work well here). Apply, show improvements, user accepts/rejects. Redisplay menu.
- **[P]**: Read `reference/party-mode.md`. Cycle perspectives on journeys. Present findings. User accepts/rejects. Redisplay menu.
- **[C]**: Proceed to next step.

## Next Step
Read and follow `reference/steps/step-07-domain.md`.
