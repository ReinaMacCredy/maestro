# Readiness Check: Is the Plan Ready to Launch?

This is the final checkpoint before you draft the first `maestro handoff` command. You already have a decomposed plan, worker assignments, and explicit constraints. Now you need to decide whether the plan is actually launchable.

The output of this step is binary:

- **ready** — the mission can be persisted and you can draft the first handoff command
- **not ready** — go back and fix the plan before you hand anything to an external worker

## The launchability checklist

Every item below must be true before you draft the command.

1. **The mission has a clear done sentence.**
   If you cannot state "when this mission is done, X is true," the worker will inherit that ambiguity.

2. **The first launchable feature is explicit.**
   You should be able to point at one specific feature and say "this is the next fresh Codex or Claude run."

3. **That feature maps to a real provider.**
   - `codex-cli` -> `--provider codex`
   - `claude-code` -> `--provider claude`
   - `subagent` or `human` -> not launchable through `maestro handoff`

4. **Dependencies are satisfied or named.**
   If the first feature depends on earlier work that is not done, the handoff is premature. Pick a different first feature or fix the plan.

5. **Verification is concrete.**
   The first feature has 2-5 observable verification steps. "Run the relevant tests" is not specific enough.

6. **Constraints are visible.**
   The worker can read what not to touch and why from `preconditions`, the feature description, or another plain-language field that will survive into the handoff prompt.

## Fast failure signals

Stop and revise the plan if any of these are true:

- The first candidate feature is assigned to `subagent` or `human`
- The feature title describes a whole milestone instead of one sprint-sized outcome
- Verification says only "tests pass" or "works"
- Constraints exist only in your head
- The handoff command would need a paragraph of caveats because the plan is still muddy

## Worked example

Mission: split auth middleware into validation and permission halves.

- First launchable feature: `auth-impl`
- `agentType`: `codex-cli`
- Provider mapping: `--provider codex`
- Verification: build, targeted tests, manual login flow
- Constraints: preserve middleware signature, do not touch the session store, no permission semantics changes

Result: **ready**

Counterexample:

- First feature is "Explore how auth works"
- `agentType`: `subagent`
- No concrete verification beyond "write notes"

Result: **not ready for `maestro handoff`**. Either add a later implementation feature as the first external worker launch, or re-decompose the mission.
