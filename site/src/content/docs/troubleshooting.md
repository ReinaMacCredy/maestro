---
title: Troubleshooting
description: Read JSON failures, repair SLP setup and role errors, and resolve development lease problems.
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
published, so an update would install code nobody else can see. Development branches
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

## SLP setup and pack errors

`SLP_PACK_MISSING` means Hub does not have its canonical `~/maestro/SLP.md`.
Repair the installation from the Maestro source checkout; do not promote a
project snapshot into the Hub by hand.

`SLP_SNAPSHOT_MISSING` or `SLP_SNAPSHOT_CHANGED` means the project copy no
longer matches the generation digest. A running generation is immutable. Read
`maestro status`, stop or emergency-stop through the authorized Supervisor,
then start a new generation from the canonical Hub pack.

`TEAM_RUNNING` means the requested project already has a running generation
with a different objective or model configuration. Stop it before starting the
changed configuration. An identical start is safe: it verifies the generation
and restores a missing required role without creating duplicates.

`INVALID_SLP_PACK` naming a model marker means the canonical pack predates that
role. Install and update preserve owner edits to `~/maestro/SLP.md`, so a pack
seeded before the Observer seat lacks its marker. Add the line the error names
next to the other model markers, for the Observer
`<!-- slp:model:observer=codex:gpt-5.6-luna -->`, then start again.

See [SLP setup and storage](/getting-started/slp-setup/) for every managed path.

## SLP role and work errors

- `NO_ACTIVE_TEAM`: this project store has no running team binding.
- `ROLE_UNPROVEN`: the command did not run from a Herdr pane registered to the
  current generation. Maestro reads `HERDR_PANE_ID` from the command's
  environment or, when the agent's shell dropped it, from the nearest ancestor
  process that still carries it; a command run outside Herdr, or from a pane
  that is not a role, stays unproven.
- `sentinel off` in `status` while the generation is RUNNING: the sentinel tab
  exited or was closed. Repeat the identical `team start` to relaunch it.
- `ROLE_FORBIDDEN`: the proven role does not own that operation. Check the
  [role authority table](/concepts/roles/#authority).
- `INVALID_STATE`: the requested transition does not follow `OPEN → ACTIVE →
  RETURNED → DONE`, or the caller is not the current owner.
- `TEAM_UNFINISHED`: normal stop found work that is not `DONE`; the error lists
  every work ID and state.
- `SLP_BINDING_MISSING`: Hub knows the generation but its project-side binding
  is absent; preserve both stores and diagnose before attempting recovery.

Use the two read-only forms before retrying:

```sh
maestro status
maestro status <work-id>
```

Normal `team stop` refuses while unfinished work remains and lists it. Return
and accept that work, or use the separately authorized Hub emergency stop:

```sh
cd ~/maestro
maestro team stop <team-id> --emergency --reason "<why work is abandoned>"
```

This keeps the four public work states and records explicit abandonment
metadata on every unfinished item.

## Removed SLP commands

SLP v2 is a hard cut. A removed verb fails with the corresponding new
operation and performs no legacy action. The full compact mapping is in the
[CLI reference](/reference/cli/#hard-cut-mapping).

## Split-brain notice

Two live sessions holding sibling or parent work in the same repository can be
a split-brain topology. `maestro attention` reports `SCOPE_COLLISION` with the
holders and common parent. The later session should stop, read `maestro status`,
and let the Lead keep one write owner per moving scope.
