# Step 7: Domain & Scoping

**Progress: Step 7 of 14** -- Next: Functional Requirements Synthesis

## Goal
Define domain-specific requirements (compliance, regulatory, industry-specific) and finalize MVP scope boundaries. Identify risks specific to the classified domain.

## Execution Rules
- Check classification from step 4: if domain=`general` AND complexity=`low`, SKIP this step entirely and proceed to step 8.
- If NOT skipped, domain requirements MUST be specific to the classified domain (not generic security/performance -- those go in step 9).
- Risk analysis MUST produce actionable items, not generic concerns.

## Context Boundaries
- All previous step outputs available (product context, classification, vision, scope, journeys).
- Functional requirements are NOT yet synthesized (step 8).
- NFRs are NOT yet defined (step 9).
- Classification data provides the compliance requirements and key concerns for this domain.

## Conditional Execution
- If domain is `general` AND complexity is `low`: announce "Domain is general with low complexity -- skipping domain-specific requirements. Proceeding to functional requirements." Go directly to step 8.
- Otherwise: execute the full discovery sequence below.

## Discovery Sequence

1. **Load Domain Context**
   Recall classification from step 4:
   - Domain: {domain}
   - Complexity: {complexity}
   - Key concerns from classification data
   - Compliance requirements from classification data

2. **Domain Requirements Discovery**
   Guided by the domain's key concerns and compliance requirements:

   - "Based on the {domain} domain, there are specific requirements we need to address. Let me walk through them."
   - For each compliance requirement listed in classification data:
     - Explain what it means in practical terms.
     - Ask if it applies to this specific track.
     - If yes, define concrete requirements (not just "comply with X" -- specify what that means for implementation).

   - For each key concern:
     - Discuss how it applies to this track.
     - Define specific requirements or constraints.

3. **Risk Analysis**
   Domain-specific risks:
   - "What could go wrong specifically because this is a {domain} project?"
   - Generate 3-5 domain-specific risks (not generic project risks).
   - For each risk: likelihood, impact, mitigation strategy.
   - Ask user to validate and add their own.

4. **MVP Scope Refinement**
   Re-examine MVP scope from step 5 through domain lens:
   - Do any domain requirements force items INTO MVP? (e.g., HIPAA compliance cannot be post-MVP)
   - Do any domain constraints force items OUT of MVP? (e.g., certain integrations require certification first)
   - Adjust scope tiers if needed, with user approval.

5. **Present Domain Summary**
   ```
   ## Domain Requirements

   ### Compliance
   - {Requirement 1}: {what it means for implementation}
   - {Requirement 2}: {what it means for implementation}

   ### Domain-Specific Constraints
   - {Constraint 1}: {impact on design}

   ### Risk Register
   | Risk | Likelihood | Impact | Mitigation |
   |------|-----------|--------|------------|
   | {risk} | {L/M/H} | {L/M/H} | {strategy} |

   ### Scope Adjustments
   {Any changes to MVP/Growth/Vision from domain analysis, or "None"}
   ```

## Quality Checks
- [ok] Step skipped correctly if general/low
- [ok] Every compliance requirement from classification data addressed
- [ok] Domain risks are specific (not generic project risks)
- [ok] Scope adjustments justified by domain analysis
- [ok] Requirements are concrete (not "comply with HIPAA" but "encrypt PHI at rest with AES-256")

## Anti-patterns
- [x] Running this step for general/low complexity projects -- it wastes time
- [x] Generic requirements that apply to any project (those go in step 9 NFRs)
- [x] Listing compliance acronyms without explaining implementation impact
- [x] Risk analysis with only vague/generic risks

## A/P/C Menu

Present: `[A] Advanced Elicitation  [P] Party Mode  [C] Continue to Functional Requirements`

- **[A]**: Read `reference/elicitation-methods.md`. Suggest methods relevant to domain (Red Team is especially strong for compliance and security domains). Apply, show improvements, user accepts/rejects. Redisplay menu.
- **[P]**: Read `reference/party-mode.md`. Cycle perspectives on domain requirements. Security and Architect perspectives especially relevant. Present findings. User accepts/rejects. Redisplay menu.
- **[C]**: Proceed to next step.

## Next Step
Read and follow `reference/steps/step-08-functional.md`.
