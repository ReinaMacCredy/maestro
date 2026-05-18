# Witness Levels

Every Evidence row carries a `witness_level` field. The Risk Engine uses this
field when evaluating autopilot policy: a high-risk task may require
`witnessed-by-maestro`, meaning evidence at a weaker level will not clear the
auto-pass threshold and the Verdict downgrades from `PASS` to `HUMAN`.

## Ladder (weakest to strongest)

### `agent-claimed-and-not-reproducible`

Manual notes — something happened that cannot be reproduced mechanically. Used
exclusively for `--kind manual-note` evidence. Does not satisfy any
witness-level gate beyond `low` in default autopilot policies.

```bash
maestro evidence record --task <id> --kind manual-note \
  --note "Verified UI on staging at 1280x800"
```

### `agent-claimed-locally`

The agent self-reported a local run. Maestro did not observe the execution.
This is the **default** for newly recorded Evidence rows unless overridden
with `--witness`.

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
```

### `witnessed-by-ci`

A trusted CI gate ran the check and posted the result back to Maestro.
Treated as authoritative for most `autopilot.yaml` policies at `medium` and
`high` risk classes.

### `witnessed-by-maestro`

Maestro itself ran the command and captured the exit code. The most
trustworthy level — Maestro controls the execution environment. Satisfies all
autopilot policy thresholds, including `high` and `critical` when the policy
permits auto-pass.

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
# Maestro shells out to the command; the level is set automatically
```

## Default autopilot thresholds

Overridable in `policies/autopilot.yaml`:

```yaml
witness_thresholds:
  low:      agent-claimed-locally
  medium:   agent-claimed-locally
  high:     witnessed-by-maestro
  critical: witnessed-by-maestro
```

See `docs/witness-levels.md` for the full ladder, Risk Engine consumption
rules, and policy interaction.

## Evidence kinds added in Phase 1 of the harness pivot

| Kind | Recorded by | Purpose |
|---|---|---|
| `lint-violation` | `task verify` (agent-claimed-locally), `ci verify` (witnessed-by-ci) | One row per architecture-lint finding |
| `session-start`, `session-exit` | preserved kinds — no live emitter, legacy rows still parse | (legacy) anchored session boundaries for arch-lint baselines |
