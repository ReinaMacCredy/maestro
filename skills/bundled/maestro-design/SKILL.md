---
name: maestro-design
description: Interview-driven product-spec authoring for maestro v2. Runs the grill protocol from ADR-0016 — walks the decision tree one branch at a time, challenges user language against CONTEXT.md and committed ADRs, cross-references claims against code when verifiable, updates CONTEXT.md inline as terms resolve, and offers ADRs sparingly. Produces a committed product-spec markdown at `.maestro/specs/<slug>.md` ready for `maestro task from-spec`. Use when the user wants to author or refine a spec before planning or implementation.
parity-skip-verbs:
  - principle promote
---

# Maestro Design

Use this skill to author or refine a product-spec via the grill protocol. The output is a committed `.maestro/specs/<slug>.md` with YAML frontmatter that `maestro task from-spec` will consume.

The grill protocol is the design skill's interview loop. It is the same pattern named by ADR-0016. Entering this skill IS running the grill — there is no separate `spec grill` verb.

## Hard rules

1. One question per turn. Wait for the answer before asking the next.
2. Provide a recommended answer with every question.
3. Challenge user language against the committed glossary in `CONTEXT.md`. If they reuse a defined term in a new way, name the conflict before refining the spec.
4. Cross-reference claims against the codebase when verifiable. If the user says "X already does Y" and the code disagrees, surface the contradiction in the next turn.
5. Update `CONTEXT.md` inline when a term resolves. Don't batch glossary edits to the end.
6. Offer an ADR only when the decision is (a) hard to reverse, (b) surprising without context, AND (c) the result of a real trade-off. If any of the three is missing, skip the ADR.
7. End by writing `.maestro/specs/<slug>.md` and confirming the spec validates (`maestro spec validate <path>`).

## Workflow

### 1. Ground in committed context

Before asking the first question, read:

- `CONTEXT.md` (if present) — the canonical glossary and domain model.
- `docs/adr/` — committed architectural decisions. Recent ADRs frame what the user can and can't change without contradiction.
- `docs/architecture.yaml` — the layered architecture rules that any code change will be linted against.
- The relevant feature directory under `src/` if the spec touches existing code.

If `CONTEXT.md` does not exist, create one lazily as the first term resolves — do not stub a blank file up front.

### 2. Open with the framing question

Ask one open question that surfaces the user's intent in their own words. Examples:

- "What problem is this spec solving — what's the symptom you're seeing today?"
- "Who's the consumer of this change, and what do they do after it lands?"
- "What's the smallest version of this that's still valuable?"

Don't ask multiple framing questions in one turn. Pick the one that most narrows the design tree.

### 3. Walk the decision tree branch by branch

For each branch — acceptance criteria, non-goals, risk class, mode, work-type, dependencies — ask one question with a recommended answer. Resolve it before moving to the next.

For each question:

- **State the question** clearly.
- **State your recommendation** with a one-line reason.
- **Quote the constraint** (CONTEXT.md term, ADR rule, lint rule) that motivates the recommendation if one applies.

Branches to walk, in roughly this order:

1. **Acceptance criteria.** What must be true for this spec to be considered shipped? Push for falsifiable criteria (a test exists, a verb behaves a specific way, a file is present), not vibes.
2. **Non-goals.** What is explicitly out of scope? Name the things a reader might assume are in scope.
3. **Risk class.** `low`, `medium`, `high`, `critical`. Default by what the change touches (auth/secrets/payments/policies/migrations → critical or high; src under a single feature → medium; docs-only → low).
4. **Mode.** `light` (single agent, in-tree) or `heavy` (worktree-per-task, ADR-0008). Default light unless the change spans multiple features or contracts.
5. **Work-type.** Classification used by harness routing. Walk the decision tree in `.maestro/docs/FEATURE_INTAKE.md` (six values: `new-spec`, `spec-slice`, `change-request`, `initiative`, `maintenance`, `harness-improvement`).
6. **Dependencies / blocked_by.** What other specs or tasks must complete first?

### 4. Challenge against committed knowledge

When the user uses a defined term in a new way, stop and name the conflict:

> "Your `CONTEXT.md` defines `<term>` as X. You just used it to mean Y. Which is right — should I update the glossary, or do you mean the existing X?"

When the user states how something works, verify against the code:

> "You said `task verify` re-runs all lints. The code in `src/service/task-verify.usecase.ts` only runs architecture lints. Did you mean that, or do you want to broaden the verifier?"

Surface contradictions in the next turn. Don't hide confusion behind plausible spec language.

### 5. Update CONTEXT.md inline

When a new term is resolved, append it to `CONTEXT.md` in the same turn it was decided. Keep entries short — a definition and one example.

Do not couple `CONTEXT.md` to implementation details. Only include terms that are meaningful to domain experts.

### 6. Offer ADRs sparingly

Only offer to author an ADR when all three are true:

1. The decision is hard to reverse (changing it later would require migration, deprecation, or coordinated rollout).
2. It would surprise a future reader without context (the choice is non-obvious).
3. There were genuine alternatives and you picked one for specific reasons.

If any of the three is missing, skip the ADR. The protocol is the deliberate human-driven counterpart to memory-auto-promotion (ADR-0015); ADRs land as committed artifacts the agent commits, not as background captures.

### 7. Write the spec

When the decision tree is resolved, write the product-spec markdown to `.maestro/specs/<slug>.md`. Use the YAML frontmatter that `maestro spec validate` expects.

Slug rules:

- kebab-case derived from the spec subject (2–4 words).
- If `.maestro/specs/<slug>.md` already exists, append a numeric suffix (`<slug>-2.md`).

Example frontmatter (refer to `src/types/product-spec.ts` for the authoritative shape):

```markdown
---
slug: <kebab-case>
title: <short title>
status: draft
acceptance:
  - <falsifiable criterion>
  - <falsifiable criterion>
non_goals:
  - <out-of-scope item>
risk_class: medium
mode: light
work_type: feature
blocked_by: []
---

# <title>

## Why
<2–3 sentences on the problem and the consumer.>

## What
<the change, grounded in committed context — link to CONTEXT.md terms and ADRs where relevant.>

## How (optional)
<the rough shape if a particular implementation is required; otherwise leave to plan.>

## Notes
<risks, dependencies, cut lines, references.>
```

### 8. Validate before handoff

Run `maestro spec validate .maestro/specs/<slug>.md`. Fix any frontmatter errors before declaring the spec ready.

### 9. Hand off cleanly

When the spec validates, the design phase is done. The next step is `maestro task from-spec .maestro/specs/<slug>.md` to create the v2 task in `draft`. Tell the user the slug, the spec path, and the next verb to run.

Do not start planning or implementation inside this skill. The handoff is to `maestro-plan` (heavy mode) or directly to `maestro task claim <id>` followed by `maestro-task` (light mode).

## What the grill is not

- It is not a sixth skill. ADR-0012 commits maestro to a 5-skill bundle plus design/plan grills as protocols, not new skills.
- It does not introduce new CLI verbs. There is no `maestro spec grill`. Entering this skill IS running the grill.
- It does not auto-promote corrections into principles. That stays human-driven via `maestro principle promote` (Phase 1.5).

## Anti-patterns

- Asking three questions in one turn. The user can't answer all of them, and the design tree stays tangled.
- Skipping the glossary check. Drift in `CONTEXT.md` compounds; one missed term becomes ten over a quarter.
- Offering an ADR for every decision. ADRs are sparse by design; over-offering trains the user to ignore them.
- Writing the spec before the user confirms the acceptance criteria. Backing out edits is more expensive than asking the question.
- Treating the grill output as ephemeral. Refined specs, updated `CONTEXT.md`, and new ADRs are committed artifacts — not session logs.
