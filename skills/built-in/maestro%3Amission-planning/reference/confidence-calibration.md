# Confidence Calibration: CS-work and CS-summary

This is the fifth step of mission planning. You have a decomposed plan, worker assignments, and boundaries from Steps 2-4. You need to rate your confidence in the plan itself and in the summary you will attach to the handoff.

The output of this step is two numbers, `CS-work` and `CS-summary`, each between 0.0 and 1.0. They go into the `CS` slot of the UKI handoff as `CS-work_0.xx~summary_0.yy`. External workers and downstream reviewers use these to decide how carefully to treat the plan.

## The two scores

**`CS-work` (0.0-1.0)** — confidence in plan correctness and executability. Would a worker who picks this plan up actually be able to execute it end-to-end? Are the dependencies right? Are the worker type assignments realistic? Are the verification steps actually verifiable?

**`CS-summary` (0.0-1.0)** — confidence that the summary captures the full intent of the work. If a downstream reviewer reads only the `SUMMARY` slot, will they understand what is happening, or will they form a wrong mental model?

The two are independent. A plan can be excellent with a misleading summary, or a solid summary can describe a half-baked plan.

## Calibration table

| Range | Meaning |
|---|---|
| 0.95-1.00 | Verified. You would bet money on this. Every step has been thought through, every assumption has been named, every worker-type choice is defensible. |
| 0.85-0.94 | Solid. 1-2 unverified assumptions remain, but they are small and the plan survives if they are wrong. |
| 0.70-0.84 | Reasonable but several unknowns. The worker should expect surprises. Plan is directionally right but expect mid-execution corrections. |
| 0.50-0.69 | Sketch. Call this a spike, not an implementation plan. The handoff is useful as a starting point but not as a contract. |
| Below 0.50 | Do not ship. Go back to Step 1 (brainstorm opening) and re-scope. |

Most real planning lands between 0.80 and 0.92. Above 0.95 is earned, not assumed. Below 0.80 is a signal to narrow scope before shipping the handoff.

## The honesty rule

CS scores lie upward by default. Humans and models both over-rate their own confidence, especially when tired or invested in a plan. The corrective is:

1. Rate the plan your gut's number.
2. Ask: "what would make this fail?"
3. For every realistic failure mode you can name, drop 0.1 (up to 0.2 total).
4. Post the adjusted number.

If you cannot name any failure modes at all, you are not being honest with yourself — re-read the plan and find at least one. Every real plan has at least one "what if this assumption is wrong" hiding in it.

## Divergent scores

When `CS-work` and `CS-summary` differ by more than 0.1, the difference is telling you something:

- **`work 0.95 / summary 0.70`** — the plan is solid but your summary does not capture it. Revise the summary. Probably you compressed something load-bearing out of the `SUMMARY` slot and it reads as something smaller than it actually is.
- **`work 0.70 / summary 0.95`** — the summary is crisp but the plan underneath is sketchy. Revise the plan. The summary reads confident because it elides the unknowns — that is how you ended up with a 0.95 summary in the first place.
- **Both below 0.80** — back to Step 1. The plan is not ready to hand off to anyone. Re-scope or re-decompose before continuing.

## Worked examples

**Example 1: The clean refactor plan.**
- Plan: split `authMiddleware` into validation and permission halves. Four features, clear boundaries, worker type is `codex-cli` for the mechanical splits and `claude-code` for the review.
- Gut rating: 0.95 / 0.92.
- Failure modes: (1) the 14 consumers might have undocumented assumptions about internal state order. Drop 0.1.
- Final: `CS-work_0.85~summary_0.92`. The summary survives because the framing is still honest even with the failure mode — it just expands execution time, not direction.

**Example 2: The ambiguous feature addition.**
- Plan: add a command palette to the TUI. Five milestones, worker types split across `human`, `claude-code`, and `codex-cli`.
- Gut rating: 0.90 / 0.90.
- Failure modes: (1) the palette's interaction with existing modals is not fully mapped; (2) the wireframe is not approved yet; (3) the keybinding conflicts with an existing shortcut the inventory did not catch.
- Three failure modes, but the cap is -0.2. Drop to 0.70 / 0.70.
- Final: `CS-work_0.70~summary_0.70`. Both below 0.80 — back to Step 1 to narrow the scope or do the inventory first. The handoff ships only after the wireframe is approved and the keybinding audit is done.

**Example 3: The bug fix.**
- Plan: fix the memory leak in the TUI that came from multiple `root.render()` calls. Single milestone, single feature, bug reproduced with a heap-growth test.
- Gut rating: 0.98 / 0.95.
- Failure modes: (1) the leak might have a second source the first fix does not cover. Drop 0.1.
- Final: `CS-work_0.88~summary_0.95`. Work drops because the root cause might be compound; summary holds because the framing ("fix the root.render leak") remains honest even if a second source is found later.
