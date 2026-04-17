# Review Dimensions -- Deep Guidance

Each dimension tells the reviewer what to look for. Read this when you need more detail than the summary in SKILL.md.

---

## 1. Completeness

The plan should address everything the spec or user request asks for. Nothing should be silently dropped.

**What to check:**
- Every functional requirement has at least one task
- Error handling is specified (not just happy path)
- Edge cases are mentioned (empty inputs, max limits, concurrent access)
- Migration path is covered if changing existing behavior
- Rollback strategy exists for risky changes

**Common misses:**
- "Update the API" without specifying backward compatibility
- No task for updating tests after behavior change
- Missing documentation/changelog tasks
- No consideration of existing data migration

**Edge-case probes (BMAD-derived):**
- What happens when the input is empty? null? maximum size? malformed?
- What happens when a dependency is unavailable during this step?
- What happens if this step succeeds but the next step fails -- is the system in a consistent state?
- What happens if two users trigger this simultaneously?

**Red flag:** A plan that only describes what to build, never what could go wrong.

---

## 2. Feasibility

Can this actually be built as described? Are there hidden blockers?

**What to check:**
- Dependencies on external APIs/services are realistic (rate limits, auth, availability)
- Time/complexity estimates match the actual work (if estimates are given)
- The tech stack can support the proposed approach
- Required permissions/access are available
- No assumptions about features that don't exist yet

**Common misses:**
- "Use the existing caching layer" when no caching layer exists
- Assuming a library API works a certain way without checking
- Planning parallel work on files that will have merge conflicts

**Edge-case probes (BMAD-derived):**
- Apply first-principles: which assumptions here are inherited from convention vs verified for this project?
- What is the simplest thing that could work? If the plan is more complex, is there a stated reason?
- Are there any "seems straightforward" tasks that actually hide significant complexity?

**Red flag:** A plan that sounds elegant but skips the messy parts.

---

## 3. Dependencies

Tasks should be ordered correctly. Nothing should start before its prerequisites are done.

**What to check:**
- Task dependency graph has no cycles
- Shared infrastructure tasks come before tasks that use them
- Database migrations before code that uses new schemas
- API changes before consumers of those APIs
- Test infrastructure before tests

**Common misses:**
- Two tasks that modify the same file listed as parallelizable
- "Add feature flag" listed after "deploy feature"
- No dependency between "create interface" and "implement interface"

**Edge-case probes (BMAD-derived):**
- Inversion test: "What if we ran these tasks in reverse order?" -- any task that would break reveals a missing dependency
- What shared state do parallel tasks touch? Shared files, shared DB tables, shared config?
- What happens if a dependency task partially completes? Does the dependent task have a precondition check?

**Red flag:** All tasks listed as independent when they clearly aren't.

---

## 4. Risk

What could go wrong, and does the plan account for it?

**What to check:**
- Fragile assumptions are identified (if X changes, this breaks)
- High-risk tasks have verification steps
- External dependency failures have fallback plans
- Data loss scenarios are considered
- Security implications are addressed

**Common misses:**
- No mention of what happens if the migration fails halfway
- Assuming network calls always succeed
- No rollback plan for database schema changes
- Ignoring rate limits on third-party APIs

**Edge-case probes (BMAD-derived):**
- Pre-mortem: "This plan has failed. What was the single most likely root cause?"
- For each external dependency: what is the plan's behavior when it returns an error? times out? returns unexpected data?
- What is the blast radius of each task? If task N corrupts data, how much is affected and how do you recover?
- Are there any "point of no return" steps? What happens if you need to abort after them?

**Red flag:** Zero risk items in a multi-phase plan.

---

## 5. Testing

Is the testing strategy adequate for the changes being made?

**What to check:**
- Critical paths have explicit test tasks
- Test types match the changes (unit for logic, integration for boundaries, e2e for flows)
- Edge cases from the completeness check have corresponding tests
- Regression tests for existing functionality that might break
- Test data/fixtures are accounted for

**Common misses:**
- "Add tests" as a single task covering everything (too vague)
- No integration tests for new API endpoints
- No tests for error handling paths
- Testing strategy that only covers happy path

**Edge-case probes (BMAD-derived):**
- For each edge case identified in other dimensions: is there a test that would catch it?
- Inversion: "What test, if removed, would let the most bugs through?" -- that test better exist
- Are boundary conditions tested? (0, 1, max, max+1, negative)
- Are concurrent scenarios tested? (two requests at once, interrupted operations)

**Red flag:** Testing is a single bullet point at the end, not woven into each phase.

---

## 6. Scope

Is the plan doing too much or too little?

**What to check:**
- Every task is traceable to a requirement (no gold-plating)
- No "while we're at it" refactoring mixed in with feature work
- Complexity is proportional to the problem (simple problems get simple solutions)
- No premature abstractions or unnecessary configurability
- Non-goals are explicit

**Common misses:**
- Adding a plugin system when a single implementation is needed
- Refactoring adjacent code that isn't broken
- Building admin UI for a feature that 3 people will use
- Over-engineering error handling for impossible scenarios

**Edge-case probes (BMAD-derived):**
- First-principles: "Does the simplest architecture fail? If not, why are we using something more complex?"
- For each abstraction: is there a second use case, or is this premature generalization?
- Are there Growth/Vision scope items that leaked into the MVP plan?
- Constraint removal test: which constraints would change the plan the most if removed? Are those constraints real?

**Red flag:** The plan is 3x longer than the problem description.

---

## 7. Ordering

Is the sequence logical? Could things be done more efficiently?

**What to check:**
- High-risk/unknown items are tackled early (fail fast)
- Foundation tasks before feature tasks
- Independent tasks identified for parallel execution
- No phase has too many tasks (3-5 per phase is ideal)
- Verification gates between phases

**Common misses:**
- UI work in phase 1, backend in phase 2 (should be opposite)
- All tasks in one giant phase with no checkpoints
- Easy wins buried in late phases (front-load quick confidence builders)

**Edge-case probes (BMAD-derived):**
- Pre-mortem: "If we have to abandon this plan at phase N, what do we have?" Each phase should produce a testable increment.
- What is the longest sequential chain? Can any of it be parallelized?
- Are the highest-risk assumptions validated in the earliest possible phase?
- If an external dependency blocks a task, does the plan have an alternative path forward?

**Red flag:** 15 tasks in a single phase with no verification step.

---

## 8. Clarity

Could a worker agent execute each task without asking clarifying questions?

**What to check:**
- Each task has clear acceptance criteria
- File paths and module names are specific (not "update the relevant files")
- API contracts are defined (request/response shapes)
- Behavior is specified, not just structure ("button submits form and shows success toast", not "add button")
- Ambiguous terms are defined

**Common misses:**
- "Implement the feature" as a task description
- "Handle errors appropriately" without specifying how
- "Update config" without specifying which fields and values
- Tasks that say "if needed" (decide now, not during execution)

**Edge-case probes (BMAD-derived):**
- The Handoff Test: read each task and ask "Could a worker agent execute this with zero clarification?"
- Are verification commands exact? (`bun test -- auth` vs "run the tests")
- Are expected outputs specified? ("returns 201 with { id, createdAt }" vs "returns success")
- Do any tasks contain hidden decisions? ("choose the best approach" is a design decision, not a task)

**Red flag:** More than 2 tasks that start with "Implement" and nothing else.

---

## 9. FR Traceability

Does every requirement from the spec map to a task in the plan? Does every task trace back to a requirement?

**When to apply:** Only when a spec is available (maestro features with spec.md, or user-provided requirements document). Skip this dimension entirely if no spec exists.

**What to check:**
- Parse all FR-N references from spec
- Parse all "Addresses: FR-N" annotations from plan tasks
- Every FR appears in at least one task
- Every task addresses at least one FR (no orphan tasks doing unspecified work)
- Acceptance criteria in spec map to verification steps in plan

**Common misses:**
- FR added during spec revision but no corresponding task added to plan
- Task addresses an FR but doesn't actually implement all aspects of it
- Non-functional requirements with no verification task
- Domain-specific requirements (compliance, security) without explicit tasks

**Edge-case probes (BMAD-derived):**
- Coverage matrix: build a quick FR --> task mapping. Any gaps are immediate blockers.
- For multi-aspect FRs (e.g., "User can create, edit, and delete records"): does the plan have tasks for ALL aspects, or just the first?
- Do domain requirements from the spec have corresponding tasks? (HIPAA compliance --> encryption task, audit log task, etc.)

**Red flag:** More than 2 FRs with no task addressing them, or tasks with no FR tracing.

---

## 10. Edge Cases

Are boundary conditions, failure modes, and unusual inputs explicitly handled?

This is the dimension where adversarial techniques have the highest impact. Rather than checking what the plan says, check what it DOESN'T say.

**What to check:**
- Empty/null/zero inputs are handled
- Maximum size/count limits are defined and enforced
- Concurrent access scenarios are addressed
- Partial failure states are recoverable
- Timeout and retry behavior is specified for external calls
- Error messages are actionable (not silent failures)

**Common misses:**
- API accepts a list but no max-length enforcement (memory bomb)
- File upload with no size limit or type validation
- Database query with no pagination (full table scan on growth)
- Background job with no idempotency (duplicate processing on retry)
- Cache invalidation not addressed (stale data after writes)
- Timezone handling not specified (UTC assumption not documented)

**Edge-case probes (BMAD-derived):**
- Pre-mortem: "A user reports data corruption. What's the most likely cause in this plan?"
- Inversion: "How would I cause this system to lose data?" -- every answer should have a countermeasure in the plan
- Red-team: "What happens if I send 10,000 requests in 1 second to this endpoint?"
- For each data write: what happens if the write succeeds but the subsequent notification/event fails?
- For each user input: what if the input is valid but adversarial? (SQL injection, XSS, path traversal)

**Red flag:** No mention of error handling, timeouts, or limits in a plan that involves external services or user input.
