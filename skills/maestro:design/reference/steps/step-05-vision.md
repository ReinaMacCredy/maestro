# Step 5: Vision & Success Criteria

**Progress: Step 5 of 14** -- Next: User Journey Mapping

## Goal
Define the product vision, success criteria (user/business/technical), and negotiate MVP/Growth/Vision scope. Push for measurability in every criterion.

## Execution Rules
- You MUST elicit vision before defining success criteria.
- Success criteria MUST be measurable -- reject vague criteria and help the user make them specific.
- Scope negotiation MUST produce three tiers: MVP, Growth, Vision.
- ALWAYS push back on "everything is MVP" -- force prioritization.

## Context Boundaries
- Product context (product.md) and track description available.
- Classification from step 4 available (type, domain, complexity).
- User journeys are NOT yet defined (that is step 6).

## Discovery Sequence

1. **Vision Elicitation**
   Start with the big picture:
   - "What is the vision for this? When it is done and working perfectly, what does the world look like?"
   - "What makes this approach special? Why build it this way versus alternatives?"
   - "Who benefits most, and what changes for them?"

   Synthesize responses into a concise vision statement and differentiator.

2. **Success Criteria Discovery**
   For each category, guide the user toward measurable outcomes:

   **User Success:**
   - "How will you know users are succeeding with this? What behavior or outcome would you measure?"
   - Push for specifics: "Complete onboarding in under 3 minutes" not "Easy onboarding"

   **Business Success:**
   - "What business needle does this move? Revenue, retention, efficiency, cost?"
   - Push for metrics: "Reduce support tickets by 40%" not "Improve support"

   **Technical Success:**
   - "What quality attributes matter most? Response time, uptime, throughput?"
   - Push for targets: "P95 response time under 200ms" not "Fast responses"

3. **Scope Negotiation**
   Guide the user through three-tier scoping:

   **MVP (Must-Have):**
   - "What is the absolute minimum that delivers value? If you could only ship 3 things, what are they?"
   - Challenge anything that is not essential for initial delivery.

   **Growth (Post-MVP):**
   - "What comes next after MVP proves itself? What would you add in iteration 2-3?"

   **Vision (Future):**
   - "What is the dream? What would you build with unlimited time and resources?"
   - Explicitly defer these: "These are great ideas. Let us capture them as Vision and keep MVP focused."

4. **Present Vision Summary**
   Show the complete vision block:
   ```
   ## Vision & Differentiator
   {synthesized vision statement}

   ## Success Criteria
   ### User Success
   - {measurable criterion 1}
   ### Business Success
   - {measurable criterion 1}
   ### Technical Success
   - {measurable criterion 1}

   ## Product Scope
   ### MVP: {list}
   ### Growth: {list}
   ### Vision: {list}
   ```

## Quality Checks
- [ok] Vision statement is concise (1-3 sentences) and compelling
- [ok] Every success criterion is measurable (has a number or verifiable condition)
- [ok] MVP scope is focused (not everything)
- [ok] Growth and Vision tiers have content (not empty)
- [ok] Clear separation between tiers (no overlap)

## Anti-patterns
- [x] Accepting vague success criteria ("make it better", "improve UX")
- [x] Letting everything be MVP -- force prioritization
- [x] Writing the vision FOR the user without their input
- [x] Skipping business success criteria for technical projects

## A/P/C Menu

Present: `[A] Advanced Elicitation  [P] Party Mode  [C] Continue to User Journey Mapping`

- **[A]**: Read `reference/elicitation-methods.md`. Suggest 3 methods relevant to vision/scope (Constraint Removal is especially strong here). User picks, apply, show improvements, user accepts/rejects. Redisplay menu.
- **[P]**: Read `reference/party-mode.md`. Cycle perspectives on vision and scope. Present findings. User accepts/rejects. Redisplay menu.
- **[C]**: Proceed to next step.

## Next Step
Read and follow `reference/steps/step-06-journeys.md`.
