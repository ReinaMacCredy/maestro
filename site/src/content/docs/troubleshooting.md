---
title: Troubleshooting
description: Read JSON failures, repair lifecycle and lease errors, and resolve split-brain sessions.
---

## Start with doctor

```sh
maestro doctor
```

Doctor is read-only. It checks the installed shim and stamp, recorded source,
repository wiring, permissions, and store access. A mechanical failure names
the next repair command.

## JSON error envelope

Failed commands write one compact JSON object to stderr and exit nonzero:

```json
{
  "ok": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "actionable description",
    "detail": "error-specific fields may follow"
  }
}
```

Use `error.code` for automation and read `error.message` before retrying. Blank
required arguments are rejected instead of becoming identities or targets.

## `UPDATE_SOURCE_DIRTY`

The recorded source checkout has tracked local changes. Commit or stash those
changes, then run `maestro update`. Untracked installer wiring is ignored by
the dirty check; tracked changes are not.

## `UPDATE_SOURCE_UNPUBLISHED`

The recorded source checkout has a remote but sits on a branch no remote has
published, so an update would install code nobody else can see. Lanes branch
inside the same checkout the runtime is installed from, so this is the ordinary
way an unreviewed branch becomes the runtime. Check out the branch the runtime
follows, then run `maestro update`; to install the current branch on purpose,
run `maestro install` from that checkout. A checkout with no remote at all is
not affected: a branch there has nothing to be unpublished against.

The drift line names both halves of the same fact, so the branch is visible
before the update rather than after it:

```
[update] runtime 24002076 differs from source 8cc27daa on main (2 commits no remote holds); run maestro update
```

The count is every commit reachable from `HEAD` that no remote-tracking ref
holds. A tracking branch that is only ahead of its upstream still updates, so
on that path the count is the only signal that the runtime would be built from
unpublished commits.

## `LEASE_REQUIRED`

The verb acts on a lease that does not exist. `maestro work release` raises it
when no session holds the item, and `maestro work reclaim` raises it in the
same case, naming the exact `maestro work start <id>` command; reclaiming a
lease another session holds is that verb's normal path.

Completion does not raise this. `maestro work done` takes an unheld lease
itself and says so when a previous holder lost one, naming that holder and the
PID or TTL liveness reason. `work start`, `work done`, and `work release`
refuse a lease another live session holds with `LEASE_HELD`.

Read the work and live session state first:

```sh
maestro work show <work-id>
maestro status --live
```

## Team is not `OPERABLE`

`maestro team status <team> --json` is the last Room-ledger snapshot. Refresh
the evidence before acting:

```sh
maestro team health <team> \
  --operation <new-stable-id> \
  --requested-by supervisor-<team> \
  --json
```

- `STARTING` or `CLOSED` means readiness was never proved for an active
  generation. Read the receipt's `missing` array.
- `DRAINING` means fresh inspection found a missing, dead, mismatched, or
  duplicate required resource. Health inspection never repairs it; run
  `team reconcile` only for explicitly authorized resource names.
- `REVIEW_HOLD` means Observer submitted a valid packet-bound finding. The team
  Supervisor must clear or escalate it with rationale; reconcile does not
  erase review state.
- `STOPPING` means drain or absence proof is incomplete. Read the stop receipt
  before retrying. Force stop is separate possible-loss authorization.

`TEAM_OVERRIDE_DENIED` with `supervisorReachable: true` is expected when the
Room tries emergency authority while `supervisor-<team>` remains reachable.
Do not retry it as an override; use routine Supervisor authority. See
[Supervised teams](/guides/supervised-teams/) for repair, owner intervention,
and shutdown commands.

## Split-brain notice

Two live sessions holding sibling or parent work in the same repository can be
a split-brain topology. `maestro attention` reports `SCOPE_COLLISION` with the
holders and common parent. The later session should stop, read `maestro status`,
and let the Lead keep one write owner per moving scope.
