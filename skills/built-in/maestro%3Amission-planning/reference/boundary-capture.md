# Constraint Capture: What Agents Must Not Touch

This is the fourth step of mission planning. You have agents assigned to features from Step 3. You need to capture the things those agents must not touch, and the reasons, so they do not drift outside scope.

The output of this step is a short list of constraints per feature, written in plain language where the agent will later see them. Without explicit constraints, agents follow the most expansive reading of the prompt and touch things you did not intend.

## What a boundary actually is

A constraint is a thing not to touch, plus the reason. The reason is load-bearing — without it, the rule cannot be enforced at edge cases. An agent that hits an unexpected obstacle and sees "do not modify X.ts" with no reason will either violate the rule or halt the mission asking for clarification. Both are failures.

A constraint with a reason lets the agent reason about its own edge case: "the rule says do not modify X.ts because Y, and in my situation Y still applies, so I should not modify X.ts." Agents that can reason about constraints execute further without needing intervention.

## Four categories

Every constraint is one of:

1. **Files or paths** — "do not modify `src/legacy/session-store.ts` because the codex-cli port is still in flight and a concurrent change would produce a merge conflict nobody can resolve cleanly."
2. **APIs or interfaces** — "do not change the signature of `buildHandoffPrompt()` because multiple tests and launch flows depend on its current shape."
3. **Patterns or idioms** — "do not use async iterators in the supervisor loop because the OpenTUI runtime does not poll them correctly and the effect only shows up under load."
4. **Out-of-scope** — "do not add caching to the launch store lookup, out of scope for this sprint, deferred to the perf milestone in the next mission."

If a constraint does not fit one of these four, it is usually a goal in disguise. See the Common mistakes section below.

## Where constraints live

Constraints should be written in plain language where the agent will later read them:

- `preconditions` when the rule is load-bearing before work starts
- feature description when the rule shapes the whole implementation
- verification notes when the rule affects how the agent proves they are done

Good plain-language examples:
- "Preserve the exported `authMiddleware(req, res, next)` signature because 14 route files depend on it."
- "Do not modify `src/runtime/session-store.ts` because a parallel port is already in flight."
- "Keep permission semantics unchanged in this feature; only separate validation from authorization."

A list of 1-4 real constraints per feature is typical. If you need 5 or more, the feature is usually too large and should be split.

## The "why" requirement

Every constraint must have a corresponding reason somewhere the agent can read it. Two options:

- Short reasons go in `preconditions` when they gate the work
- Long reasons go in the feature description, which the agent receives alongside the handoff

If a constraint has no reason written down, it is not a real constraint — it is a preference the planner forgot to justify. Remove it or write the reason.

## Common mistakes

**Goals disguised as constraints.** "Do not ship bugs" is a goal, not a constraint. "Do not break the existing auth tests" is a goal. Constraints name specific things not to touch; goals name outcomes to achieve. Goals belong in `verificationSteps`.

**Constraints without reasons.** "Do not modify the logger" with no reason is worthless — the agent cannot decide what to do when a test genuinely needs logging changes. Always pair with the why.

**More than 4 constraints per feature.** If a feature has 5+ things it cannot touch, the feature is too large or the scope is not well understood. Split the feature or re-scope the mission.

**Whole-repo constraints on a small feature.** "Do not touch any file outside `src/tui/`" is usually laziness. Name the specific files or subsystems that matter.

## Worked example

Feature: "Refactor the auth middleware to split session validation from permission checking."

Constraints:

1. Preserve the exported `authMiddleware(req, res, next)` signature because 14 route files already depend on it. The internal split is invisible to callers. Good location: `preconditions`.

2. Do not modify `src/runtime/session-store.ts` because that backend is already being ported in a parallel mission. Good location: feature description.

3. Keep permission semantics unchanged because this feature is a pure refactor, not a product behavior change. Good location: `preconditions`.

Three constraints, each in a different category (API, files, out-of-scope), each with a reason, each visible to the agent in normal Maestro context. An agent hitting an edge case during the refactor can reason about each rule without asking for human intervention.
