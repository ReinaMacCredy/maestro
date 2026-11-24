---
title: Troubleshooting
description: Read JSON failures, resolve lease problems, and repair SLP setup and seat errors.
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

The drift line names two different quantities, so the branch and the size of
the gap are both visible before the update rather than after it:

```
[update] runtime 24002076 is 1 commit behind source 8cc27daa on main; separately, the source holds 2 commits no remote has; run maestro update
```

The first number is the drift: how far the installed runtime is behind the
source head. If the installed commit is not an ancestor of that head the line
says so instead (`has diverged from ... 1 commit behind and 2 ahead`), and if
the range cannot be measured at all it falls back to `differs from` with no
number rather than an invented one.

The second number is a different quantity, which is why it sits in its own
clause: every commit reachable from `HEAD` that no remote-tracking ref holds.
A tracking branch that is only ahead of its upstream still updates, so on that
path the count is the only signal that the runtime would be built from
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

## `LEASE_HELD`

Another live session holds the item: `work start`, `work done`, and `work
release` refuse it and name the holder. Read the holder first:

```sh
maestro work list
maestro work show <work-id>
```

`work list` prints each active item as `held by <session> (live|dead)`. A dead
holder's lease is taken over with `maestro work reclaim <id> --reason
"<why>"`; a live one keeps the lease until it finishes. `maestro work release
<id> --force` drops a live session's lease and records who forced it.

## SLP plugin off

The team verbs (`team`, `slp`, `decide`, `status`, `work take`,
`work return`, `work accept`, and `work add --to`) belong to the `slp` plugin.
While it is off, each is refused with a line that names the enable command.
Run `maestro plugin enable slp` in the Hub room and in the project, then
repeat the command. See [SLP setup and storage](/getting-started/slp-setup/).

## `HERDR_UNAVAILABLE`

An SLP operation could not reach Herdr's socket; the message names the path
(`HERDR_SOCKET_PATH`, or `~/.config/herdr/herdr.sock`) and the failing call.
Start Herdr, or run the command from a Herdr pane so the socket path is in
its environment, then repeat the operation. Nothing was recorded.

## `RUNTIME_PANE_FAILED`

The generation is RUNNING but its runtime pane did not open, usually because
the Herdr plugin is not linked. Run `maestro install` with Herdr running (it
renders `~/maestro/herdr-plugin.toml` and links the Hub room as the plugin),
then repeat the same `team start`: it reopens only what is missing.

## Unreachable wakes

`maestro slp status` from a team pane lists the wakes the runtime still holds
for working seats and the ones it dropped as `unreachable <subject>:
<reason>`. A wake is unreachable when no target resolves: a Lead's line to
the Hub with no recorded Hub pane and no Herdr agent named `supervisor`, or a
seat whose pane is gone. The store already holds the fact the wake pointed at;
read `maestro status` there and, for the Hub case, start the next generation
from the Hub agent's own pane so `team start` records it.

## SLP setup and pack errors

`SLP_PACK_MISSING` means the Hub does not have its `~/maestro/SLP.md`. Repair
the installation from the Maestro source checkout; do not copy a project's
pack into the Hub by hand.

`INVALID_SLP_PACK` means `~/maestro/SLP.md` is missing its version marker or a
required section. Install and update keep your edits to that file, so compare
it with the shipped copy at
`~/.maestro/runtime/src/plugins/resources/SLP.md` and restore what is missing.

`SLP_SNAPSHOT_MISSING` or `SLP_SNAPSHOT_CHANGED` means the project copy of the
pack, or a seat file the generation pinned, does not match the generation
digest. A running generation is immutable. Put the bytes back, or stop the
team from the Hub and start a new generation.

`TEAM_RUNNING` means the project already has a running generation with a
different objective or seat model. Stop it before starting the changed
configuration. An identical start is safe: it verifies the generation and
restores a missing seat without creating duplicates.

`SEAT_NOT_INSTALLED` means a seat's rendered file is missing (for a Claude
seat, under `~/.maestro/claude/<seat>/agents/`); run `maestro install`.

`SEAT_TOKEN_MISSING` means a Claude seat was about to open without the
`claude setup-token` token at `~/.maestro/claude/oauth-token`. Run
`claude setup-token | maestro install --seat-token` and check
`maestro doctor` for `seat token: present`.

`TRUST_DIALOG` means a Codex pane met its directory trust dialog. Open that
directory once with Codex, accept the dialog, and rerun.

`INVALID_OPTION` on `--lead-model`, `--peer-model` or `work add --model`
means an empty model name, `--model` on an item with no `--to`, or `--model`
on a Peer pane that is already running. Inside a team, `work add` also refuses
`--acceptance` and `--blocked-by` this way; write both into the objective.

`PEER_ACTIVE` and `PEER_RESET_FAILED` come from `work add --to <peer> --fresh`:
the Peer still holds ACTIVE work, or its harness reset did not take. Nothing
was added.

See [SLP setup and storage](/getting-started/slp-setup/) for every managed path.

## SLP role and work errors

- `NO_ACTIVE_TEAM`: this project store has no running team binding.
- `ROLE_UNPROVEN`: the command did not run from a Herdr pane registered to the
  current generation. Maestro reads `HERDR_PANE_ID` from the command's
  environment or, when the agent's shell dropped it, from the nearest ancestor
  process that still carries it; a command run outside Herdr, or from a pane
  that is not a seat, stays unproven.
- `ROLE_FORBIDDEN`: the proven role does not own that operation. Check the
  [role authority table](/concepts/roles/#authority).
- `INVALID_STATE`: the requested transition does not follow `OPEN → ACTIVE →
  RETURNED → DONE`, or the caller is not the current owner.
- `TEAM_UNFINISHED`: normal stop found work that is not `DONE`; the error lists
  every work ID and state.
- `SLP_BINDING_MISSING`: the Hub knows the generation but its project-side
  binding is absent; keep both stores and diagnose before attempting recovery.
- `SLP_V2_CUTOVER`: inside a team pane, a classic verb that an SLP operation
  covers is refused and the message names the operation to use.

Use the two read-only forms before retrying:

```sh
maestro status
maestro status <work-id>
```

Normal `team stop` refuses while unfinished work remains and lists it. Return
and accept that work, or use the Hub's emergency stop:

```sh
cd ~/maestro
maestro team stop <team-id> --emergency --reason "<why work is abandoned>"
```

This keeps the four public work states and records explicit abandonment
metadata on every unfinished item.
