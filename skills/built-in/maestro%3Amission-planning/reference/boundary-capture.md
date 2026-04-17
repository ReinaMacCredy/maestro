# Boundary Capture: What Workers Must Not Touch

This is the fourth step of mission planning. You have workers assigned to features from Step 3. You need to capture the things those workers must not touch, and the reasons, so they do not drift outside scope.

The output of this step is a short list of boundaries per feature and a `BOUNDARY_STATE` token list for the final handoff. Without explicit boundaries, workers follow the most expansive reading of the prompt and touch things you did not intend.

## What a boundary actually is

A boundary is a thing not to touch, plus the reason. The reason is load-bearing — without it, the boundary cannot be enforced at edge cases. A worker that hits an unexpected obstacle and sees "do not modify X.ts" with no reason will either violate the boundary (because the reason "must not be that serious") or halt the mission asking for clarification. Both are failures.

A boundary with a reason lets the worker reason about its own edge case: "the rule says do not modify X.ts because Y, and in my situation Y still applies, so I should not modify X.ts." Workers that can reason about boundaries execute further without needing intervention.

## Four categories

Every boundary is one of:

1. **Files or paths** — "do not modify `src/legacy/session-store.ts` because the codex-cli port is still in flight and a concurrent change would produce a merge conflict nobody can resolve cleanly."
2. **APIs or interfaces** — "do not change the signature of `HandoffEmitter.emit()` because three other consumers read its return shape today and would break silently."
3. **Patterns or idioms** — "do not use async iterators in the supervisor loop because the OpenTUI runtime does not poll them correctly and the effect only shows up under load."
4. **Out-of-scope** — "do not add caching to the handoff lookup, out of scope for this sprint, deferred to the perf milestone in the next mission."

If a boundary does not fit one of these four, it is usually a goal in disguise. See the Common mistakes section below.

## Token format for `BOUNDARY_STATE`

Boundaries are captured in two places: long-form in the feature description or `KEY_DECISIONS`, and short-form as `BOUNDARY_STATE` tokens in the UKI handoff. The short-form rules:

- Max 4 words per `_`-link
- Lowercase, underscores between words
- No `-` inside a token (reserved for slot-name separators in UKI v5.2)
- The token names the constraint; the reason lives in the long-form

Examples:
- `no_caching_outside_scope` — paired with "deferred to perf milestone in next mission"
- `preserve_auth_middleware_signature` — paired with "three consumers depend on return shape"
- `src_legacy_do_not_edit` — paired with "codex-cli port in flight, concurrent edits conflict"
- `no_async_iterators_supervisor` — paired with "OpenTUI runtime does not poll correctly under load"

A `BOUNDARY_STATE` list of 1-4 tokens per feature is typical. If you cannot fit the constraint in 4 words, the underlying rule is probably two boundaries, not one — split it.

## The "why" requirement

Every boundary in `BOUNDARY_STATE` must have a corresponding reason somewhere the worker can read it. Two options:

- Short reasons go in `KEY_DECISIONS` tokens: `kept_emitter_signature_load_bearing`
- Long reasons go in the feature description, which the worker receives alongside the handoff

If a boundary has no reason written down, it is not a real boundary — it is a preference the planner forgot to justify. Remove it or write the reason.

## Common mistakes

**Goals disguised as boundaries.** "Do not ship bugs" is a goal, not a boundary. "Do not break the existing auth tests" is a goal. Boundaries name specific things not to touch; goals name outcomes to achieve. Goals belong in `verificationSteps`.

**Boundaries without reasons.** "Do not modify the logger" with no reason is worthless — the worker cannot decide what to do when a test genuinely needs logging changes. Always pair with the why.

**More than 5 boundaries per feature.** If a feature has 6+ things it cannot touch, the feature is too large or the scope is not well understood. Split the feature or re-scope the mission.

**Whole-repo boundaries on a small feature.** "Do not touch any file outside `src/tui/`" is usually laziness. Name the specific files or subsystems that matter.

## Worked example

Feature: "Refactor the auth middleware to split session validation from permission checking."

Boundaries:

1. `preserve_middleware_signature` — the exported `authMiddleware(req, res, next)` signature is consumed by 14 route files and cannot change in this feature. The internal split is invisible to callers. Reason in `KEY_DECISIONS`: `auth_signature_load_bearing_14_callers`.

2. `no_session_store_changes` — session storage lives in `src/runtime/session-store.ts` and is being ported to a new backend in a parallel mission. Do not touch it here; the port owner will re-integrate once both are done. Reason in the feature description: "concurrent port in flight, edits will conflict."

3. `no_permission_semantics_changes` — splitting validation from permission checking must be a pure refactor. Do not change what counts as "permitted" — that is a separate product decision scheduled for the next mission. Reason in `KEY_DECISIONS`: `permission_semantics_product_decision_deferred`.

Three boundaries, each in a different category (API, files, out-of-scope), each with a reason, each under the 4-word-per-token limit. A worker hitting an edge case during the refactor can reason about each constraint without asking for human intervention.
