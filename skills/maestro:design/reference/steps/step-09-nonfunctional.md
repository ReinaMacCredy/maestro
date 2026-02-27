# Step 9: Non-Functional Requirements

**Progress: Step 9 of 16** -- Next: Spec Draft & Approval

## Goal
Define measurable non-functional requirements: performance, security, scalability, compatibility, and compliance. Format: "[System] shall [metric] [condition]".

## Execution Rules
- Every NFR MUST be measurable -- reject subjective statements
- Seed from classification data (domain compliance) and step 7 domain requirements
- Do NOT repeat domain-specific requirements already captured in step 7 -- reference them
- Cover at minimum: performance, security, scalability, compatibility

## Context Boundaries
- All prior discovery available including FRs from step 8
- Classification data provides compliance baseline
- Domain requirements from step 7 (if applicable) already cover domain-specific compliance

## Discovery Sequence

1. **Seed from Classification**
   Pull compliance requirements and key concerns from classification (step 4). These inform the baseline.

2. **Performance Requirements**
   Guide the user:
   - "What response time is acceptable? Think about the most common operation."
   - "What throughput do you need to support? How many concurrent users/requests?"
   - Push for specific targets: "API responses under 200ms at P95" not "fast"

3. **Security Requirements**
   Based on domain and data sensitivity:
   - "What authentication method is required?"
   - "What data is sensitive? How should it be protected at rest and in transit?"
   - "What audit/logging requirements exist?"
   - For regulated domains: reference specific controls from step 7

4. **Scalability Requirements**
   - "What growth do you anticipate? 10x? 100x?"
   - "Which components need to scale independently?"
   - "What's the data growth projection?"

5. **Compatibility Requirements**
   - "What platforms, browsers, or environments must be supported?"
   - "Any minimum version requirements?"
   - "API backward compatibility requirements?"

6. **Present NFR Summary**
   Format each as measurable statement:
   ```
   ### Non-Functional Requirements

   - Performance: API responses shall complete within 200ms at P95 under 1000 concurrent users
   - Security: All PII shall be encrypted at rest (AES-256) and in transit (TLS 1.2+)
   - Scalability: System shall support 10x current load without architecture changes
   - Compatibility: Shall support latest 2 major versions of Chrome, Firefox, Safari, Edge
   ```

   Ask: "Do these targets match your expectations? Any categories missing or targets to adjust?"

## Quality Checks
- [ok] Every NFR is measurable (has a number, condition, or verifiable criterion)
- [ok] Performance, security, scalability, and compatibility all addressed
- [ok] No duplication with step 7 domain requirements
- [ok] NFRs are realistic for MVP scope

## Anti-patterns
- [x] Vague NFRs ("system shall be fast", "system shall be secure")
- [x] Duplicating domain requirements already in step 7
- [x] Gold-plating NFRs beyond what MVP needs
- [x] Forgetting compatibility/platform requirements
- [x] Skipping the measurability check -- every NFR must have a verifiable criterion

## A/P/C Menu

Present: `[A] Advanced Elicitation  [P] Party Mode  [C] Continue to Spec Draft & Approval`

- **[A]**: Read `reference/elicitation-methods.md`. Suggest methods relevant to NFRs (Red Team for security, Inversion for reliability). Apply, show improvements, user accepts/rejects. Redisplay menu.
- **[P]**: Read `reference/party-mode.md`. Cycle perspectives. Architect and Security perspectives especially relevant. Present findings. User accepts/rejects. Redisplay menu.
- **[C]**: Proceed to next step.

## Next Step
Read and follow `reference/steps/step-10-spec-approval.md`.
