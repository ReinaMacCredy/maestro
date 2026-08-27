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

## `LEASE_REQUIRED`

Completion requires the current session to hold the work lease. The error names
the exact `maestro work start <id>` command. If a previous holder expired, the
message also names that holder and the PID or TTL liveness reason.

Do not bypass the lease. Read the work and live session state first:

```sh
maestro work show <work-id>
maestro status --live
```

## Split-brain notice

Two live sessions holding sibling or parent work in the same repository can be
a split-brain topology. `maestro attention` reports `SCOPE_COLLISION` with the
holders and common parent. The later session should stop, read `maestro status`,
and let the Lead keep one write owner per moving scope.
