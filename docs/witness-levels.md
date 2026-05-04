# Witness Levels

Every Evidence row carries a `witness_level` field indicating how trustworthy
the claim is. The Risk Engine uses this field when evaluating autopilot policy:
for example, `autopilot.yaml` can require `witnessed-by-maestro` for risk class
`high`, meaning evidence at a weaker level does not clear the auto-pass threshold
and the verdict downgrades from `PASS` to `HUMAN`.

## The ladder (strongest to weakest)

### `witnessed-by-maestro`

The claim was produced by Maestro itself running a command and capturing the
exit code and output. This is the most trustworthy level because Maestro
controls the execution environment.

What produces it: `maestro evidence record --task <id> --command "bun test" --exit 0`
where Maestro shells out to the given command and records the actual exit code.
Any `evidence record` call that goes through the Maestro command runner receives
this level automatically.

How the Risk Engine consumes it: satisfies all autopilot policy thresholds,
including `high` and `critical` risk classes when the policy permits auto-pass.

Examples:
- `maestro evidence record --task tsk-aaa --command "bun test" --exit 0`
- `maestro evidence record --task tsk-aaa --command "bun run build" --exit 0 --duration 8200`

### `witnessed-by-ci`

The claim was produced by a trusted CI run — for example, a GitHub Actions check
that executed the same command and posted the result back to Maestro.

What produces it: a CI integration that writes an evidence row with
`witness_level: witnessed-by-ci`. Full CI wiring is partial in v0.67; complete
round-trip integration lands at L4.

How the Risk Engine consumes it: treated as authoritative for any risk class
that does not explicitly require `witnessed-by-maestro`. In practice,
`witnessed-by-ci` satisfies most `autopilot.yaml` policies for `medium` and
`high` risk classes.

Examples:
- A GitHub Actions workflow that runs `bun test` and calls `maestro evidence record`
  with `--witness witnessed-by-ci` after a successful run.

### `agent-claimed-locally`

The agent says the command ran with some result, but Maestro did not observe
the execution. The evidence row was written by the agent directly rather than
by the Maestro command runner.

What produces it: any evidence row created without Maestro observing the
execution. This is the **default** for schema v1 evidence rows that lack an
explicit `witness_level` field (via the v1-to-v3 reader-tolerant upgrade path
introduced in L3.3).

How the Risk Engine consumes it: sufficient for `low` risk class auto-pass.
For `medium` risk class, autopilot policy determines whether this level clears;
for `high` and `critical`, it does not by default.

Examples:
- An agent that ran `bun test` in its own shell and then called
  `maestro evidence record --task tsk-aaa --command "bun test" --exit 0`
  where Maestro did not shell out itself.

### `agent-claimed-and-not-reproducible`

The agent says something happened that cannot be reproduced mechanically — for
example, "I manually verified the UI looked correct on staging." This is the
weakest level and is used exclusively for `manual-note` evidence.

What produces it: `maestro evidence record --task <id> --kind manual-note --note "..."`.
Manual notes always receive this level; there is no mechanism to promote them.

How the Risk Engine consumes it: does not satisfy any witness-level threshold
beyond `low` in most default autopilot policies. Manual notes contribute to
ProofMap criterion coverage counts but do not clear witness-level gates for
`medium`, `high`, or `critical` risk classes.

Examples:
- `maestro evidence record --task tsk-aaa --kind manual-note --note "Verified UI on staging"`
- `maestro evidence record --task tsk-aaa --kind manual-note --note "Confirmed no regressions in dev environment"`

## How the Risk Engine consumes witness levels

`autopilot.yaml` defines the minimum required witness level per risk class:

```yaml
# Example autopilot.yaml
witness_thresholds:
  low: agent-claimed-locally
  medium: agent-claimed-locally
  high: witnessed-by-maestro
  critical: witnessed-by-maestro
```

When the Risk Engine evaluates a verdict request, it finds the effective risk
class for the task (taking the higher of agent-proposed and Maestro-derived),
then checks every evidence row linked to acceptance criteria. If any row's
witness level is below the threshold for the effective risk class, the verdict
downgrades from `PASS` to `HUMAN` (Rule 12 + autopilot policy).

The ProofMap builder (`src/features/verify/usecases/proof-map.ts`) produces a
per-criterion coverage map that shows which evidence rows satisfy each acceptance
criterion and at what witness level, making the gap visible before a verdict is
requested.

## See also

- `docs/risk-class-derivation.md` — how the effective risk class is computed from diff signals.
- `docs/policy-format.md` — full schema for `autopilot.yaml` and the other policy files.
