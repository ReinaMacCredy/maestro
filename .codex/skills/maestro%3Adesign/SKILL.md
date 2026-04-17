---
name: maestro:design
description: "Deep discovery and specification for ambitious features. Full BMAD-inspired interview with classification, vision, journeys, domain analysis, and FR synthesis. Same output contract (spec.md + plan.md) as new-track but far richer. Use for multi-component systems, regulated domains, or unclear requirements."
argument-hint: "<track description>"
---

# Design -- Deep Discovery & Specification

Full-ceremony specification process for ambitious features. Produces the same `spec.md` + `plan.md` output as `/maestro:new-track` but through deep, multi-step discovery inspired by BMAD methodology.

**When to use this instead of `/maestro:new-track`:**
- Multi-component systems with many moving parts
- Regulated domains (healthcare, fintech, govtech)
- Unclear or complex requirements that need thorough discovery
- Features where getting the spec wrong is expensive

## Arguments

`$ARGUMENTS`

The track description. Examples: `"Add user authentication with OAuth and RBAC"`, `"Build real-time collaboration engine"`, `"Implement HIPAA-compliant patient portal"`

---

## Step Sequence

This skill uses a step-file architecture. Each step is a self-contained file in `reference/steps/`. You MUST load ONE step at a time, execute it fully, then load the NEXT. Previous step files get dropped from context.

**Rules:**
- NEVER load multiple step files simultaneously
- ALWAYS read the entire step file before executing
- NEVER skip steps or reorder the sequence
- Steps 4-9 include an A/P/C menu -- the user MUST select [C] before you proceed

### Step 1: Validate Prerequisites
Check product.md and tracks.md exist.
--> Read and follow `reference/steps/step-01-init.md`

### Step 2: Parse Input & Generate Track ID
Extract description, infer type, generate `{shortname}_{YYYYMMDD}` ID.
--> Read and follow `reference/steps/step-02-parse-input.md`

### Step 3: Create Track Directory
Create `.maestro/tracks/{track_id}/`.
--> Read and follow `reference/steps/step-03-create-dir.md`

### Step 4: Project Classification
Classify project type, domain, and complexity using `reference/classification-data.md`.
--> Read and follow `reference/steps/step-04-classification.md`

### Step 5: Vision & Success Criteria
Define vision, measurable success criteria, and MVP/Growth/Vision scope.
--> Read and follow `reference/steps/step-05-vision.md`

### Step 6: User Journey Mapping
Map ALL user types with narrative journeys. Minimum 3 journeys. Extract capability hints.
--> Read and follow `reference/steps/step-06-journeys.md`

### Step 7: Domain & Scoping
Domain-specific requirements and risk analysis. Skipped if domain=general AND complexity=low.
--> Read and follow `reference/steps/step-07-domain.md`

### Step 8: Functional Requirements Synthesis
Synthesize FRs from all discovery. Grouped by capability area. Each FR testable and implementation-agnostic.
--> Read and follow `reference/steps/step-08-functional.md`

### Step 9: Non-Functional Requirements
Performance, security, scalability, compatibility. Measurable format.
--> Read and follow `reference/steps/step-09-nonfunctional.md`

### Step 10: Spec Draft & Approval
Compose enriched spec from all discovery. Present for approval. Write spec.md.
--> Read and follow `reference/steps/step-10-spec-approval.md`

### Step 11: Codebase Pattern Scan
Scan codebase for existing patterns relevant to this track. Feed into plan context.
--> Read and follow `reference/steps/step-11-codebase-scan.md`

### Step 12: Implementation Plan with Traceability
Generate plan with FR traceability and coverage matrix. Present for approval. Write plan.md.
--> Read and follow `reference/steps/step-12-plan.md`

### Step 13: Detect Relevant Skills
Scan runtime skill list for skills matching this track's domain/tech. Store matches in metadata.json.
--> Read and follow `reference/steps/step-13-skills.md`

### Step 14: Plan-to-BR Sync
If Beads (BR) is available and initialized, sync plan into BR epics/issues. Skip if not available.
--> Read and follow `reference/steps/step-14-br-sync.md`

### Step 15: Pre-Implementation Readiness Gate
Validate FR coverage, AC coverage, dependency sanity, scope alignment. Pass/fail gate.
--> Read and follow `reference/steps/step-15-readiness.md`

### Step 16: Metadata, Registry, Commit & Summary
Write metadata.json, update tracks.md, commit, display summary.
--> Read and follow `reference/steps/step-16-commit.md`

---

## When Design Is "Done Enough"

Design is not a goal -- it is a gate. You are done when an autonomous agent can execute the plan without asking questions.

**The Handoff Test**: Read every task in the plan. For each one, ask: "Could a worker agent execute this with zero clarification?" If the answer is no for any task, the design is not done.

**Done criteria:**
- [ ] Every task has exact file paths, complete code snippets, and executable verification commands
- [ ] No task says "add validation" or "implement error handling" -- it says what, where, and how
- [ ] Dependencies form a DAG (no cycles, infrastructure before consumers)
- [ ] Coverage matrix shows 0 orphaned FRs
- [ ] Readiness gate passes (step 15)
- [ ] Each design decision has a stated reason (not just a choice)

**Not done criteria (keep going):**
- A task requires domain knowledge that is not in the spec or context files
- Verification steps say "manually check" or "visually confirm"
- Two tasks modify the same file with no dependency between them
- A task's "What to do" section requires inference about the codebase

---

## Design-to-Plan Bridge

Design produces `spec.md`. Plan-write consumes it. The bridge between them is how design decisions become task structure.

### How Design Decisions Map to Tasks

| Design artifact | Becomes in plan |
|-----------------|-----------------|
| FR (functional requirement) | One or more tasks with `Addresses: FR-N` |
| User journey | Sequence of tasks ordered by journey flow |
| Domain constraint | Task guardrail in `Must NOT do` section |
| NFR (performance, security) | Verification command in task's `Verify` section |
| Codebase pattern (step 11) | Code snippets that follow the existing convention |
| Out-of-scope item | Explicit exclusion in task's `Must NOT do` |

### The plan-write Connection

After design completes, the approved spec feeds directly into `maestro plan-write --feature <name>`. The plan-write command expects:

1. **A `## Discovery` section** (min 100 chars) -- summarize what design discovered
2. **FR references** -- plan tasks trace back to spec FRs via `Addresses: FR-N`
3. **Non-Goals / Ghost Diffs** -- derived from the spec's Out of Scope section
4. **Context files** -- design should save key findings via `maestro context-write` so plan-write can reference them

**Output contract**: Design produces `spec.md` + `plan.md`. The `maestro:implement` skill consumes both. The spec is the "what", the plan is the "how and in what order."

---

## Task Structure & Handoff

When the plan reaches the task-writing stage, apply these conventions to every task.

### Bite-Sized Task Granularity

**Each step is one action (2-5 minutes):**
- "Write the failing test" - step
- "Run it to make sure it fails" - step
- "Implement the minimal code to make the test pass" - step
- "Run the tests and make sure they pass" - step
- "Commit" - step

### Task Dependencies

The **Depends on** annotation declares task execution order:
- **Depends on**: none -- No dependencies; can run immediately or in parallel
- **Depends on**: 1 -- Depends on task 1
- **Depends on**: 1, 3 -- Depends on tasks 1 and 3

Always include **Depends on** for each task. Use `none` to enable parallel starts.

Maximize parallelism. If two tasks touch different files and have no data dependency, they should both be `Depends on: none` or depend only on shared infrastructure tasks.

### Task Template

````markdown
### N. Task Name

**Depends on**: none
**Addresses**: FR-1, FR-3

**Files:**
- Create: `exact/path/to/file.py`
- Modify: `exact/path/to/existing.py:123-145`
- Test: `tests/exact/path/to/test.py`

**What to do**:
- Step 1: Write the failing test
  ```python
  def test_specific_behavior():
      result = function(input)
      assert result == expected
  ```
- Step 2: Run test to verify it fails
  - Run: `pytest tests/path/test.py::test_name -v`
  - Expected: FAIL with "function not defined"
- Step 3: Write minimal implementation
  ```python
  def function(input):
      return expected
  ```
- Step 4: Run test to verify it passes
  - Run: `pytest tests/path/test.py::test_name -v`
  - Expected: PASS
- Step 5: Commit
  ```bash
  git add tests/path/test.py src/path/file.py
  git commit -m "feat: add specific feature"
  ```

**Must NOT do**:
- {Task guardrail -- derived from domain constraints and out-of-scope items}

**References**:
- `{file:lines}` -- {Why this reference matters}

**Verify**:
- [ ] Run: `{command}` --> {expected}
- [ ] {Additional acceptance criteria}

All verification MUST be agent-executable (no human intervention):
  `bun test` --> all pass
  `curl -X POST /api/x` --> 201
NOT: "User manually tests..."
NOT: "Visually confirm..."
````

### Remember

- Exact file paths always
- Complete code in plan (not "add validation")
- Exact commands with expected output
- Reference relevant skills with @ syntax
- DRY, YAGNI, TDD, frequent commits
- All acceptance criteria must be agent-executable (zero human intervention)
- Every task must have `Addresses: FR-N` tracing back to the spec

---

## Design Review Checklist

Before approving the spec (step 10) or plan (step 12), run this checklist mentally.

### Spec Review

| Check | Pass | Fail |
|-------|------|------|
| Every FR is testable | "System returns 401 for unauthorized requests" | "System handles auth" |
| Success criteria are measurable | "Response time < 200ms p95" | "System is fast" |
| MVP scope is ruthlessly small | 3-8 FRs for MVP | 15+ FRs all marked MVP |
| Out of scope is explicit | "No admin dashboard in MVP" | Silence on boundaries |
| Edge cases derived from journeys | Error paths from step 6 journey mapping | "Handle errors appropriately" |
| Domain constraints are specific | "PHI must be encrypted at rest (AES-256)" | "Follow HIPAA" |

### Plan Review

| Check | Pass | Fail |
|-------|------|------|
| Coverage matrix has 0 orphans | Every FR mapped to a task | FR-7 shows `--` |
| Tasks are self-contained | Worker can execute without asking questions | "Implement the auth layer" |
| Dependencies are minimal | Max 2-3 deps per task | Task 8 depends on 1,2,3,4,5 |
| Verification is executable | `bun test -- auth` --> all pass | "Check it works" |
| No implicit ordering | Explicit `Depends on` for every task | "Do this after the DB task" |
| Phase boundaries are meaningful | Each phase produces a testable increment | Phases split arbitrarily |

---

## Over-Engineering Traps

Design invites over-engineering. Recognize these patterns and cut them.

| Trap | Signal | Fix |
|------|--------|-----|
| **Abstraction astronautics** | "Let's build a plugin system so we can..." | Build the concrete thing. Abstract when the second use case appears. |
| **Premature generalization** | "This should work for any provider, not just Stripe" | Build for Stripe. Add the interface when provider #2 arrives. |
| **Spec-driven scope creep** | "While we're at it, we should also..." | Check the MVP tier. If it's not MVP, it goes in Growth. |
| **Architecture tourism** | "We should use event sourcing / CQRS / microservices" | Does the simplest architecture fail? If not, use the simplest architecture. |
| **Defensive over-specification** | 25+ FRs for a CRUD feature | If classification says "low complexity", the spec should be short. Match depth to complexity. |
| **Gold-plated NFRs** | "99.99% uptime" for an internal tool | Match NFRs to actual user expectations. Internal tools do not need carrier-grade SLAs. |

**The test**: For every design decision, ask: "What is the simplest thing that could work?" If your design is more complex than that, you need a stated reason why.

---

## Execution Handoff

After saving the plan, the design phase is complete. The handoff artifact is:

```
.maestro/tracks/{track_id}/
  spec.md    -- What to build (from step 10)
  plan.md    -- How to build it, in what order (from step 12)
```

These two files are the complete handoff. The `maestro:implement` skill consumes them.

**Transition options:**

1. **maestro plan-write** (recommended for hive workflows) -- Run `maestro plan-write --feature <name>` with the plan content. This enters the standard hive workflow: plan-approve, tasks-sync, worktree-start.

2. **maestro:implement** (direct execution) -- Open new session with `maestro:implement`. The skill reads spec.md and plan.md, then executes tasks sequentially with checkpoints.

3. **Subagent-driven** (this session) -- Stay in this session. Dispatch a fresh subagent per task. Review between tasks. Fast iteration but no worktree isolation.

**Before handing off**, verify:
- Readiness gate passed (step 15)
- All context files saved via `maestro context-write`
- Track metadata committed (step 16)

---

## Relationship to Other Commands

- `maestro init` -- Initialize maestro for the project (run first)
- `maestro feature-create` -- Create a feature to work on
- `/maestro:design` -- **You are here.** Deep discovery for ambitious features
- `maestro plan-write` -- Write the plan from design output (feeds into task-sync)
- `maestro plan-approve` -- Approve the plan for execution
- `maestro task-sync` -- Generate tasks from approved plan
- `/maestro:implement` -- Execute the implementation plan
- `/maestro:review` -- Verify implementation against spec
- `maestro status` -- Check progress across all features

A track created here produces `spec.md` and `plan.md` that `maestro:implement` or `maestro plan-write` consumes. The enriched spec serves as the baseline for `maestro:review`. Deep specs lead to better implementations -- invest in the discovery.
