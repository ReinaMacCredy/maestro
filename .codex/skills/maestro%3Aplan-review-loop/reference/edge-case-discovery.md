# Edge-Case Discovery -- Adversarial Techniques for Plans

These techniques are adapted from maestro:design's BMAD-inspired elicitation methods, reframed for plan review rather than spec creation. Each technique probes a different blind-spot category.

The adversarial reviewer subagent applies these techniques. This reference provides the full methodology so the reviewer can go deep.

---

## 1. Pre-Mortem Analysis

**Purpose:** Surface gaps that optimistic forward-planning misses by working backward from failure.

**The setup:** Assume the plan has been fully executed, deployed to production, and FAILED. Users are filing bugs. The team is in an incident postmortem. Work backward to find what the plan should have addressed.

**How to apply to a plan:**

1. **Generate failure scenarios.** For each phase, imagine the worst realistic outcome:
   - "Phase 2 deployed but the migration corrupted 12% of user records because..."
   - "The feature shipped but 30% of API calls timeout because..."
   - "Users can bypass the authorization check by..."

   Each scenario must be CONCRETE -- include specifics like percentages, data types, user actions. "It might not work" is not a scenario.

2. **Trace root causes.** For each scenario, identify the specific plan gap:
   - Missing task (no migration verification step)
   - Incomplete task (API task doesn't specify timeout handling)
   - Missing dependency (auth task doesn't depend on role-setup task)
   - Wrong assumption (assumes single-tenant when multi-tenant)

3. **Propose fixes.** Each fix must be specific enough to be a plan edit:
   - BAD: "Add error handling"
   - GOOD: "Add task 2.5: 'Verify migration rollback by running rollback script against staging copy. Expected: all records restored to pre-migration state within 5 minutes.'"

**Minimum output:** 3 failure scenarios for Light depth, 5 for Standard/Deep.

**Quality check:** Every scenario should make the plan author say "oh, I didn't think of that." If the scenarios are obvious, dig deeper.

---

## 2. Inversion

**Purpose:** Reveal assumptions and undefended paths by thinking about how to guarantee failure.

**The setup:** You are actively trying to sabotage this plan. Your goal is to ensure the implementation fails. What would you do?

**How to apply to a plan:**

1. **Generate sabotage strategies.** Think like a chaos engineer:
   - "Skip the database migration and go straight to the code changes"
   - "Deploy without running any tests"
   - "Assume the external payment API never returns errors"
   - "Let any authenticated user access admin endpoints"
   - "Never validate input size -- accept 100MB payloads"
   - "Ignore timezone differences between servers"
   - "Don't handle the case where the queue is full"

2. **Check for countermeasures.** For each sabotage strategy, search the plan:
   - Is there a task that explicitly prevents this?
   - Is there a verification step that would catch this?
   - Is there a dependency that enforces correct ordering?

3. **Flag undefended strategies.** Any sabotage that the plan doesn't prevent is a gap:
   - The sabotage becomes the ISSUE
   - The missing countermeasure becomes the FIX
   - The severity depends on how likely the failure mode is in practice

**Minimum output:** 5 sabotage strategies for Standard, 7 for Deep.

**Quality check:** Focus on sabotage strategies that could happen ACCIDENTALLY, not just maliciously. "Skip tests" is interesting because a rushed developer might do it. "Delete the database" is not interesting because no one does that accidentally.

---

## 3. Red-Team

**Purpose:** Attack the plan from multiple adversarial perspectives to find security, resilience, and integrity gaps.

**The setup:** You are an external attacker, a disgruntled insider, a chaotic load balancer, and a cosmic ray all at once. Find every way the system described by this plan can be broken.

**How to apply to a plan:**

### Security Attack Surface
For every user input, API endpoint, and data flow in the plan:
- What if the input contains SQL injection? XSS? Path traversal?
- What if authentication is bypassed? (missing auth check, expired token accepted)
- What if authorization is circumvented? (horizontal privilege escalation -- user A accesses user B's data)
- What if sensitive data is logged, cached, or exposed in error messages?
- What if the API is called with a valid token but invalid permissions?

### Resilience Stress Test
For every external dependency (database, API, queue, cache):
- What happens when it's down? Slow (10x latency)? Returns garbage?
- What happens when it's at capacity? (connection pool exhausted, queue full)
- What happens during a deployment? (rolling restart, version skew)
- What happens when the clock skews? (NTP drift, timezone mismatch)

### Data Integrity Probe
For every write operation:
- What happens if it's executed twice? (idempotency)
- What happens if it partially succeeds? (write to DB succeeds, event publish fails)
- What happens during concurrent writes? (last-write-wins, merge conflicts)
- What happens if the data grows 100x? (pagination, indexing, archival)

### Concurrency Hazards
For every shared resource (database table, file, cache key, config):
- What happens with 2 concurrent readers?
- What happens with 2 concurrent writers?
- What happens with 1 reader and 1 writer?
- Is there a lock? What happens if the lock holder crashes?

**Minimum output:** 3 findings per attack category for Deep depth.

---

## 4. First-Principles Assumption Check

**Purpose:** Identify assumptions inherited from convention that may not hold for this specific project.

**The setup:** Strip away everything you think you know about "how these things are usually done." What is actually true vs assumed?

**How to apply to a plan:**

1. **List assumptions.** For each major design decision in the plan, extract the assumption:
   - "We'll use a relational database" --> assumes data is relational
   - "Authentication via JWT" --> assumes stateless auth is sufficient
   - "Deploy as a single service" --> assumes traffic doesn't need isolated scaling
   - "Use the existing ORM" --> assumes the ORM supports the query patterns needed
   - "API responses are JSON" --> assumes all clients can parse JSON

2. **Challenge each assumption:**
   - Is this actually verified for THIS project? Or copied from a template?
   - What evidence supports this assumption? (not "it's standard practice" -- that's not evidence)
   - What would change if this assumption were false?

3. **Flag unfounded assumptions as risks:**
   - If the assumption is unverified and the impact of being wrong is high --> [blocker]
   - If the assumption is unverified but the impact is low --> [minor]
   - Propose a verification step: a task early in the plan that validates the assumption

**Minimum output:** 3 assumptions checked for Deep depth.

---

## 5. Journey Gap Analysis

**Purpose:** Verify the plan covers all user journeys, not just the main happy path.

**When to apply:** Only when a spec with user journeys exists. Skip otherwise.

**How to apply to a plan:**

1. **Extract journeys from spec.** Each user journey describes a sequence of user actions and expected system responses.

2. **Trace each journey through the plan.** For every step in the journey:
   - Is there a task that implements this step?
   - Is there a task that handles what happens when this step fails?
   - Is there a test that verifies this step works?

3. **Flag gaps:**
   - Journey step with no implementing task --> [major] missing task
   - Journey step with no error handling --> [major] missing edge case
   - Journey step with no test --> [minor] missing verification

**Common journey gaps:**
- The "new user" journey (empty state, onboarding) is planned but the "returning user" journey (existing data, migration) is not
- The "happy path" journey is fully covered but the "error recovery" journey (user retries after failure) has no tasks
- Admin journeys are described in the spec but have no plan tasks
- The "deactivation/deletion" journey is completely absent

---

## Technique Selection by Depth

| Depth | Pre-mortem | Inversion | Red-team | First-principles | Journey gaps |
|-------|-----------|-----------|----------|-----------------|-------------|
| Light | 3 scenarios | -- | -- | -- | -- |
| Standard | 5 scenarios | 5 strategies | -- | -- | If spec exists |
| Deep | 5 scenarios | 7 strategies | All 4 categories | 3+ assumptions | If spec exists |

---

## Output Format for Adversarial Reviewer

Each finding follows this format:

```
- SEVERITY: [blocker/major/minor]
- TECHNIQUE: PRE-MORTEM / INVERSION / RED-TEAM / FIRST-PRINCIPLES / JOURNEY-GAP
- LOCATION: Phase N, Task M (or "Plan-wide" for structural issues)
- ISSUE: {The specific edge case or blind spot}
- SCENARIO: {Concrete example of how this causes failure -- include specifics}
- FIX: {Specific, actionable plan edit -- new task, modified task, or new verification step}
```

**Good finding:**
```
- SEVERITY: [major]
- TECHNIQUE: PRE-MORTEM
- LOCATION: Phase 2, Task 4 (Database migration)
- ISSUE: No verification that migration is reversible before running on production data
- SCENARIO: Migration adds a NOT NULL column with a default. Rollback script drops the column, but loses any data written to it during the migration window. 3 hours of user data lost.
- FIX: Add task 2.3: "Test migration rollback on staging clone. Verify: data written during migration window survives rollback. Run: ./scripts/migrate-rollback-test.sh --> 'All records preserved'"
```

**Bad finding:**
```
- SEVERITY: [major]
- TECHNIQUE: PRE-MORTEM
- LOCATION: Phase 2
- ISSUE: Migration might fail
- SCENARIO: Something goes wrong
- FIX: Add error handling
```

The difference: the good finding tells you exactly what breaks, how, and what to do about it. The bad finding could apply to any plan ever written and is therefore useless.
