# Unattended

Use this recipe when an external driver is authorized to keep advancing local
accepted work while the user is away. Maestro records state; it does not own a
hidden scheduler or a second lifecycle. The exit predicate and the
side-discovery rule come from pstack's autonomous-run playbook (Lauren Tan,
MIT; notice in THIRD_PARTY.md).

## Exit predicate

Before the first iteration, write the run's exit condition as a checkable
predicate, such as tests green, a repro fixed, every listed item done, or a
pixel diff of zero. Record it with the base commit as the first work note on
the item that holds the run:
`maestro work note <id> "start: unattended | predicate <condition> | base <commit>"`.

- The run ends when the predicate holds, with the report under Continue.
- A plateau is not a stop. When verified iterations stop moving the
  predicate, pivot the approach and keep going. Return blocked on a genuine
  dead end instead of spinning.
- Never relax the predicate to declare victory.

## Loop anatomy

### Perceive

Reconstruct state from `maestro work list`, `maestro work show`, and decisions.
Read the predicate note first. Do not rely on conversational memory.

### Choose

Select exactly one ready item whose authority and acceptance are already
settled. If none is safe, return dry or blocked instead of inventing work.

### Act

Run that one item through its playbook (`maestro recipe show <name>`, matched by
`~/maestro/WORKFLOW.md#routing`). Reversible external actions
proceed (`~/maestro/WORKFLOW.md#authorization-boundaries`); an irreversible one
happens only when the unattended grant names it.
Make the smallest change the evidence justifies and verify it before the next
change, per **Sequence Work into Verifiable Units** (PRINCIPLES.md). Keep a
change only when it advanced the predicate. A change that "might help" is
reverted, not left to ride.

An unattended run fixes only what blocks its goal, as its own change, and then
returns to the predicate (`~/maestro/WORKFLOW.md#autonomy`). Every other
discovery is a finding for the owner: a related bug, a flaky check, review
noise, a tooling failure, drift. File a doctrine correction with
`maestro lesson file`. Any other finding goes in the run's report or a
handback.

### Observe

Verify the item through its real consumer path and record completion evidence.
Add one work note per iteration on the item that holds the run, naming what
changed and whether the predicate moved.
Stop on approval prompts, secrets, destructive git, or exhausted failure
budget.

### Learn

Record only sourced corrections that will help a later session resume safely.

### Continue

The external driver may select the next ready item after the current unit is
verified, while the predicate does not hold. Return one of: next item, dry,
blocked, or hard stop.

The driver wakes on the signal the run waits for. An event to watch (CI, a
merge, a ref advancing) wakes it on the event, with a long time-based
heartbeat as a fallback. With no event to watch, a fixed interval matches how
often the result is worth checking again.

When the predicate holds, the run ends with its report. The report names the
exit predicate, the iterations run, what landed, what was discarded, the
findings for the owner, and the predicate's final state.
