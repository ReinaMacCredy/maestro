---
name: maestro:plan-review-loop
description: "Deep-review any plan (maestro, Codex, Claude Code plan mode, or plain markdown) using iterative subagent review loops with BMAD-inspired adversarial edge-case discovery. Spawns reviewer subagents that find issues using pre-mortem, inversion, and red-team techniques, auto-fixes them with structured fix strategies, and re-reviews until the plan passes with zero actionable issues. Use when the user says 'review the plan', 'deep review', 'check the plan thoroughly', 'review loop', 'validate before approving', or wants rigorous plan validation before execution. Also use proactively before plan-approve when the plan is complex or high-risk."
---

# Plan Review Loop

## Overview

Iteratively review and improve a plan through three complementary agents:
1. **Structural reviewer** -- 10 standard dimensions (completeness, feasibility, dependencies, risk, testing, scope, ordering, clarity, FR traceability, edge cases)
2. **Adversarial reviewer** -- BMAD-inspired techniques (pre-mortem, inversion, red-team, first-principles) that systematically surface blind spots the structural pass misses
3. **Final reviewer (gate)** -- holistic coherence check after the specialists pass, catching fix collisions, coherence drift, and severity decay blind spots

The specialist reviewers (1, 2) loop until both pass. The final reviewer (3) runs once as a gate before approval. Each agent is a fresh subagent with no sunk-cost bias.

## When to Use

- Before `maestro plan-approve` on any non-trivial plan
- Before executing a Codex plan or Claude Code plan-mode plan
- When the user explicitly asks for deep review
- When you wrote the plan yourself and want honest validation before proceeding
- When the plan is complex (3+ phases, 5+ tasks, cross-cutting concerns)

## When NOT to Use

- Simple, single-task plans (one file change, obvious fix)
- The user explicitly says they don't want review
- The plan has already been through this loop and approved

## The Loop

```
     +-------------------+
     |  0. Classify       |
     |     (depth/domain) |
     +--------+----------+
              |
     +--------v----------+
     |  1. Read Plan      |
     +--------+----------+
              |
  +--+--------v----------+<--+
  |  |  2a. Structural    |   |
  |  |      Reviewer      |   |
  |  +--------+----------+   |
  |           |               |
  |  +--------v----------+   |
  |  |  2b. Adversarial   |   |
  |  |      Edge-Case     |   |  INFINITE LOOP
  |  |      Discovery     |   |  (auto-adapts at
  |  +--------+----------+   |   rounds 3, 5, 7, 9+)
  |           |               |
  |  +--------v----------+   |
  |  |  3. Merge &        |   |
  |  |     Deduplicate    |   |
  |  +---+----------+----+   |
  |      |          |         |
  |     ANY      BOTH PASS    |
  |     ISSUES      |         |
  |      |          v         |
  | +----v----+ +----------+  |
  | | 4. Fix  | | 5. Final |  |
  | | + Decay | | Reviewer |  |
  | +----+----+ +---+------+  |
  |      |       |      |     |
  |      |    issues?  PASS   |
  |      |       |      |     |
  |      +<------+  +---v---+ |
  |      |          |6. OKAY| |
  +------+----------+-------+-+
```

### Step 0: Classify Plan Depth

Before reviewing, classify the plan to calibrate review intensity. This determines which adversarial techniques to apply and how deep to probe.

**Quick classification (do this inline, not via subagent):**

| Signal | Depth | Adversarial techniques |
|--------|-------|----------------------|
| 1-2 phases, < 5 tasks, single domain | **Light** | Pre-mortem only |
| 3-4 phases, 5-10 tasks, moderate complexity | **Standard** | Pre-mortem + Inversion |
| 5+ phases, 10+ tasks, cross-cutting, regulated domain, or unclear requirements | **Deep** | Pre-mortem + Inversion + Red-team + First-principles |

**Domain detection:** If a spec exists (maestro feature), check for domain markers (healthcare, fintech, govtech, etc.). Regulated domains automatically escalate to Deep regardless of task count.

### Step 1: Read the Plan

Detect the plan source and read it:

| Source | How to detect | How to read |
|--------|--------------|-------------|
| Maestro feature | User says "review the plan for [feature]" | `maestro plan-read --feature <name>` |
| File path | User provides a path (`.md`, `.txt`) | Read the file directly |
| Plan mode | Active plan in `~/.claude/plans/` | Read the plan file |
| Inline | Plan is pasted in the conversation | Save to a temp file first (see below) |
| Codex | Plan in `.codex/` or similar | Read the file directly |

**Inline plans must be materialized to a file.** When the user pastes a plan directly in conversation, save it to a working file before starting the loop. This is critical -- the loop edits the plan in-place across rounds, and conversation context is not a durable place to track changes.

```
# Save inline plan to a working file
Write the plan to: .maestro/plans/review-draft.md (or /tmp/plan-review-<timestamp>.md if no .maestro/)
```

Tell the user: "I've saved your plan to `<path>` so I can track changes across review rounds. The final version will be at the same path."

**Also read the spec if available.** For maestro features, read `spec.md` alongside `plan.md`. The spec enables FR traceability checking -- a high-value review dimension that catches orphaned requirements.

If the source is ambiguous, ask: "Where is the plan? Give me a feature name, file path, or paste it."

### Step 2a: Spawn Structural Reviewer

Launch a subagent with the plan content and standard review criteria. The reviewer is a fresh agent with no context from prior work -- this is the point.

```
Agent({
  prompt: `You are a plan reviewer. Read and deeply review the following plan.

PLAN:
<plan>
{plan content here}
</plan>

SPEC (if available):
<spec>
{spec content here, or "No spec available -- skip FR traceability checks"}
</spec>

CONTEXT (if available):
- Project: {project description}
- Tech stack: {languages, frameworks}
- Constraints: {any known constraints}
- Review depth: {Light/Standard/Deep from Step 0}

Review the plan against ALL of these dimensions:
1. COMPLETENESS -- Are all requirements addressed? Missing edge cases? Missing error handling?
2. FEASIBILITY -- Can this actually be built as described? Are estimates realistic?
3. DEPENDENCIES -- Are task dependencies correct? Any circular deps? Missing prerequisites?
4. RISK -- What could go wrong? Are there fragile assumptions? Missing fallback strategies?
5. TESTING -- Is the testing strategy adequate? Are critical paths covered?
6. SCOPE -- Is it YAGNI-compliant? Over-engineered? Under-specified?
7. ORDERING -- Is the phase/task order logical? Could anything be parallelized?
8. CLARITY -- Could a worker agent execute each task without ambiguity?
9. FR TRACEABILITY -- (Only if spec provided) Does every FR in the spec have at least one task that addresses it? Are there orphaned FRs? Are there tasks that don't trace to any FR?
10. EDGE CASES -- Are boundary conditions, empty inputs, max limits, concurrent access, and failure modes explicitly handled? Look for what the plan assumes will "just work."

For each issue found, report:
- SEVERITY: [blocker] [major] [minor] [nit]
- DIMENSION: which of the 10 above
- LOCATION: which section/task/phase
- ISSUE: what's wrong
- FIX: specific, actionable fix (not "add error handling" -- say exactly what to add and where)

If you find ZERO actionable issues (blocker/major/minor), respond with exactly:
VERDICT: PASS

If you find ANY actionable issues, respond with exactly:
VERDICT: FAIL
Then list all issues.

Be rigorous. A plan that "seems fine" is not a PASS. Look for what's missing, not just what's wrong.`,
  mode: "bypassPermissions"
})
```

See `reference/review-dimensions.md` for detailed guidance on each review dimension.

### Step 2b: Spawn Adversarial Edge-Case Reviewer

Launch a **second** subagent that attacks the plan using BMAD-inspired adversarial techniques. This runs in parallel with step 2a when possible.

Which techniques to apply depends on the depth classification from Step 0:

```
Agent({
  prompt: `You are an adversarial plan reviewer. Your job is NOT to check formatting or completeness -- a separate reviewer handles that. Your job is to find the edge cases, blind spots, and hidden failure modes that a standard review misses.

PLAN:
<plan>
{plan content here}
</plan>

SPEC (if available):
<spec>
{spec content here, or "No spec available"}
</spec>

CONTEXT:
- Project: {project description}
- Tech stack: {languages, frameworks}
- Domain: {domain from classification, or "general"}
- Review depth: {Light/Standard/Deep}

Apply these adversarial techniques:

{Include techniques based on depth classification:}

## PRE-MORTEM (all depths)
Assume this plan has already been executed and FAILED in production. Work backward:
1. Generate 3-5 specific, concrete failure scenarios (not vague "it didn't scale")
2. For each scenario, trace the root cause to a gap in the plan
3. For each gap, propose a specific fix (new task, modified task, or new verification step)

Example failure scenario: "The migration ran in staging but deadlocked in prod because the plan didn't account for concurrent writes to the users table during the 2-hour migration window."
NOT: "The migration might fail." (too vague to be actionable)

## INVERSION (Standard and Deep)
Ask: "How would we GUARANTEE this plan fails?" Generate 5-7 sabotage strategies:
1. List concrete ways to ensure failure (e.g., "skip all error handling," "assume the database never goes down," "ignore auth on internal endpoints")
2. For each sabotage strategy, check whether the plan has a countermeasure
3. Flag undefended sabotage strategies as plan gaps with proposed fixes

## RED-TEAM (Deep only)
Attack the plan from an adversarial stance:
1. Security: What inputs are unsanitized? What endpoints lack auth? What data is unencrypted?
2. Resilience: What happens when external services are down? What has no retry/fallback?
3. Data integrity: Where can data be corrupted, lost, or duplicated? What has no idempotency?
4. Concurrency: Where can race conditions occur? What shared state lacks synchronization?

## FIRST-PRINCIPLES (Deep only)
Strip assumptions. For each major design decision in the plan:
1. List the assumption it rests on
2. Ask: "Is this actually true for THIS project, or inherited from convention?"
3. Flag unfounded assumptions as risks

For each issue found, report:
- SEVERITY: [blocker] [major] [minor]
- TECHNIQUE: which adversarial technique surfaced this (PRE-MORTEM / INVERSION / RED-TEAM / FIRST-PRINCIPLES)
- LOCATION: which section/task/phase
- ISSUE: the specific edge case or blind spot
- SCENARIO: a concrete example of how this causes failure
- FIX: specific, actionable fix

If you find ZERO issues, respond with:
VERDICT: PASS

Otherwise:
VERDICT: FAIL
Then list all issues.

Your value is in finding what the plan author didn't think of. Be creative and adversarial. Generic observations like "consider error handling" are worthless -- give concrete scenarios.`,
  mode: "bypassPermissions"
})
```

See `reference/edge-case-discovery.md` for detailed guidance on each adversarial technique.

### Step 3: Merge and Deduplicate Issues

Both reviewers return independently. Merge their findings:

1. **Collect all issues** from both the structural and adversarial reviewers
2. **Deduplicate** -- if both reviewers flagged the same gap (e.g., both noticed missing error handling on the same endpoint), keep the more specific version
3. **Severity reconciliation** -- if one reviewer says [major] and another says [blocker] for the same issue, use the higher severity
4. **Categorize for fix strategy** (see Step 4)

If BOTH reviewers return VERDICT: PASS --> proceed to Step 5 (Final Reviewer).

Track the review history:
- Log which iteration you're on
- Log how many issues were found per round, broken down by reviewer (structural vs adversarial)
- Track issue trend (decreasing = healthy, flat/oscillating = stalled, increasing = diverging)
- The loop NEVER stops on its own -- it adapts strategy automatically (see Convergence Safety)

### Step 4: Auto-Fix Issues

For each merged issue, apply the appropriate fix strategy based on category:

#### Fix Categories

| Category | Pattern | Fix Strategy |
|----------|---------|-------------|
| **Missing task** | Gap in FR coverage, missing error handling path, missing migration step | Add a new task to the appropriate phase with proper dependencies, acceptance criteria, and FR tracing |
| **Incomplete task** | Task lacks verification, missing edge case handling, vague acceptance criteria | Expand the existing task with specific details -- exact commands, expected outputs, boundary conditions |
| **Wrong ordering** | Dependency missing or incorrect, task in wrong phase | Add/fix dependency annotations, move task to correct phase |
| **Scope violation** | Gold-plating, Growth items in MVP, premature abstraction | Remove the over-engineered part; replace with simplest-thing-that-works |
| **Missing verification** | No way to prove the task worked, "manually check" instructions | Add executable verification commands with expected output |
| **Undefended edge case** | Adversarial reviewer found unhandled failure mode | Add error handling, fallback, retry, or validation -- with a concrete test for the scenario |
| **Assumption risk** | First-principles reviewer flagged unfounded assumption | Add a verification task early in the plan that validates the assumption before depending on it |

#### Severity-based action

| Severity | Action |
|----------|--------|
| `[blocker]` | Must fix. Fundamental design flaw -- may require restructuring. |
| `[major]` | Must fix. Missing requirement, incorrect dependency, undefended failure mode. |
| `[minor]` | Fix if straightforward. Unclear wording, missing detail, small gaps. |
| `[nit]` | Skip unless trivial. Style, formatting, preference. |

#### How to fix depends on plan source

| Source | Fix method |
|--------|-----------|
| Maestro | `maestro plan-write --feature <name>` with updated content |
| File | Edit the plan file directly |
| Plan mode | Edit the plan file |
| Inline (saved to file) | Edit the working file created in Step 1 |

**Fix discipline:**
- Apply fixes atomically -- one issue at a time, verify the fix doesn't break something else
- When adding new tasks, ensure they have: dependencies, FR tracing (if spec exists), executable verification, and "Must NOT do" guardrails
- When fixing edge cases surfaced by adversarial review, include the concrete failure scenario as a comment so future readers understand WHY the handling exists
- Never fix a nit if it risks introducing a new issue

After fixing all actionable issues:
1. Apply severity decay to any recurring issues (see Convergence Safety)
2. Check if auto-adaptation triggers apply for this round number (3, 5, 7, 9+)
3. Go back to Step 2a with the updated plan. The loop continues until PASS.

### Step 5: Spawn Final Reviewer (gate)

When both specialist reviewers return VERDICT: PASS, spawn a **third** agent that reads the final plan holistically. This agent has no knowledge of the fix history -- it sees only the finished product. It runs exactly **once**.

The final reviewer's job is different from the specialists: it checks whether the plan is coherent as a whole after N rounds of incremental edits.

```
Agent({
  prompt: `You are the final reviewer for a plan that has already passed structural and adversarial review. Two specialist reviewers found it acceptable. Your job is to read the plan with completely fresh eyes and check for problems that incremental review misses.

PLAN:
<plan>
{final plan content here}
</plan>

SPEC (if available):
<spec>
{spec content here, or "No spec available"}
</spec>

FIX HISTORY:
<history>
{Summary of all issues found and fixed across N rounds, including:
- Round N: X structural issues, Y adversarial edge cases
- Issues fixed: [brief list]
- Issues demoted via severity decay: [brief list]
- Auto-adaptations applied: [e.g., "nit purge at round 3", "depth downgrade at round 5"]}
</history>

You are checking for THREE things only:

1. COHERENCE -- Do the plan's parts still fit together after multiple rounds of fixes? Look for:
   - Tasks that reference other tasks which were removed or restructured
   - Dependencies that became circular after reordering
   - Acceptance criteria that contradict each other across tasks
   - Phases that no longer flow logically

2. FIX COLLISIONS -- Did fixes from different rounds or different reviewers conflict? Look for:
   - Two tasks that now cover the same work (duplicated by separate fixes)
   - Error handling added by one fix that contradicts a retry strategy added by another
   - Scope that crept back in after a scope-reduction fix

3. DECAY BLIND SPOTS -- Were real issues incorrectly demoted away? Look at the fix history:
   - Issues demoted via severity decay that still represent genuine gaps
   - Patterns where the same underlying problem was flagged differently each round (symptom of an unfixed root cause)

Do NOT re-review the plan against structural or adversarial dimensions -- that work is done. Only check the three things above.

For each issue found, report:
- SEVERITY: [blocker] [major] [minor]
- CATEGORY: COHERENCE / FIX_COLLISION / DECAY_BLIND_SPOT
- LOCATION: which section/task/phase
- ISSUE: what's wrong
- FIX: specific, actionable fix

If you find ZERO issues, respond with:
VERDICT: PASS

Otherwise:
VERDICT: FAIL
Then list all issues.

Be precise. You are the last gate before approval. Only flag issues that would cause real problems during execution -- not stylistic preferences.`,
  mode: "bypassPermissions"
})
```

**If the final reviewer returns PASS** --> proceed to Step 6.

**If the final reviewer returns FAIL** --> apply fixes (same as Step 4), then go back to Step 2a. The specialists re-validate the fixed plan. The final reviewer does NOT run again -- once the specialists next return PASS, the plan goes directly to Step 6. The final reviewer gets exactly one shot; this prevents ping-pong between the gate and the loop.

### Step 6: Plan is OKAY

When the plan has passed all reviewers:

1. Report to the user: "Plan passed review after N rounds (including final gate review). Here's what was fixed:"
   - Summarize the issues found and fixed across all rounds
   - Separate structural fixes from edge-case discoveries from final-gate findings so the user sees all three categories
   - Note any nits that were intentionally skipped
   - Highlight the most interesting edge cases the adversarial reviewer caught (these are the ones that would have been bugs)
   - If the final reviewer caught anything, highlight it -- these are coherence issues that would have slipped through without the gate
2. If this is a maestro plan, suggest: `maestro plan-approve --feature <name>`
3. If this is a file-based plan, confirm the file has been updated

**Final traceability check (if spec exists):** Before declaring PASS, do one quick inline verification:
- Count FRs in spec, count "Addresses: FR-N" references in plan
- If any FR is unaddressed, flag it even if reviewers missed it
- This is a mechanical check, not a judgment call -- it catches drift between fix rounds

## Convergence Safety

The loop runs until BOTH reviewers return VERDICT: PASS. There is no hard iteration limit and no automatic pause. The loop self-corrects through automatic strategy adaptation.

**Healthy convergence:** Issue count drops each round (e.g., 8 --> 3 --> 0). This is normal. No intervention needed.

**Severity decay (automatic, every round):** After each fix round, before re-reviewing, apply severity decay to recurring issues:
- An issue that was flagged in the previous round AND was already fixed --> demote one severity level (blocker --> major --> minor --> nit)
- An issue demoted to [nit] is automatically skipped in the next round
- This prevents the loop from stalling on subjective or stylistic disagreements between reviewer rounds
- Track demotions explicitly: "Issue X demoted from [major] to [minor] (recurred after fix in round N)"

**Stalled convergence (auto-adapt at round 3, 5, 7, ...):** If issue count is not decreasing after an odd-numbered round:

| Round | Automatic adaptation |
|-------|---------------------|
| 3 | **Nit purge**: Skip ALL remaining [nit] and [minor] issues. Only fix [blocker] and [major]. Report: "Narrowing focus to blockers and majors." |
| 5 | **Depth downgrade**: Reduce adversarial depth by one level (Deep --> Standard --> Light). Report: "Reducing adversarial depth to break oscillation." |
| 7 | **Adversarial freeze**: Lock in the adversarial reviewer's last PASS (or skip adversarial entirely if it never passed). Only run structural reviewer. Report: "Freezing adversarial pass -- structural issues only." |
| 9+ | **Reviewer sharpening**: Add to the structural reviewer prompt: "The following issues have been fixed in prior rounds. Do NOT re-flag them unless the fix was reverted: [list of fixed issues with round numbers]." This prevents reviewer amnesia from re-discovering already-addressed concerns. |

**Divergence (issue count increasing for 2+ consecutive rounds):** Do NOT stop. Instead:
1. Revert plan to the last-known-good version (the version from the round with fewest issues)
2. Re-apply only [blocker] fixes from subsequent rounds
3. Resume the loop from the reverted+blocker-fixed version
4. Report: "Divergence detected. Reverted to round N baseline, re-applying only blockers."

**The loop never asks the user for guidance on convergence.** It adapts automatically. The user can interrupt at any time by their own choice, but the loop will not prompt them to.

## Progress Reporting

Keep the user informed during the loop:

- After classification: "Plan classified as [depth]. Will apply [techniques]."
- After each review round: "Round N: structural reviewer found X issues, adversarial reviewer found Y edge cases (Z blocker, W major). Fixing..."
- After each fix round: "Fixed N issues (M structural, K edge cases). Sending back for review..."
- After final reviewer: "Final gate review: [PASS / found X issues]. {details if issues found}"
- On completion: "Plan passed after N rounds (including final gate). Summary of changes: ..."
- Highlight adversarial findings: "Edge cases caught: [list the interesting ones briefly]"

## Key Principles

- **Fresh eyes every round** -- Each reviewer subagent starts clean. No accumulated context bias.
- **Three lenses, one loop** -- Structural review catches what's wrong with the plan as written. Adversarial review catches what the plan doesn't address. Final review catches what the fix process broke. All three are necessary.
- **Final reviewer runs once** -- The gate agent gets exactly one shot after both specialists pass. If it finds issues, fixes go back through the specialist loop. This prevents ping-pong between the gate and the specialists.
- **Fix before re-review** -- Never send the same plan back without changes. That's a waste.
- **Severity discipline** -- Don't let nits block convergence. Fix blockers and majors, skip nits if they're stalling progress.
- **Concrete over generic** -- "Add error handling" is not a fix. "Add try/catch around the API call in task 3 that returns a 503 fallback response with retry-after header" is a fix.
- **Adversarial value is in specificity** -- A pre-mortem that says "the migration might fail" is worthless. One that says "the migration deadlocks because task 4 holds a write lock while task 5 reads the same table" is gold.
- **User stays in control** -- Report progress at each round. The user can interrupt anytime, but the loop never prompts them to.
- **No iteration limit, no pause** -- The loop runs until PASS. It self-corrects through severity decay, depth downgrade, and reviewer sharpening. Healthy loops converge in 2-4 rounds; stalled loops auto-adapt and keep going.
- **Calibrate depth to risk** -- A 3-task bugfix doesn't need red-team analysis. A multi-phase regulated-domain feature does. Step 0 classification prevents over-reviewing simple plans.
