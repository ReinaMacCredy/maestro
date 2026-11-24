---
title: Read-only mode
description: Read Maestro state without persisting session, lease, liveness, or application records.
---

Read-only mode is a setting for one process. It is not a team seat and starts
no background process.

Set `MAESTRO_READ_ONLY=1` for a process that must fail closed on mutation:

```sh
MAESTRO_READ_ONLY=1 maestro work list
MAESTRO_READ_ONLY=1 maestro search "<query>"
```

## What it protects

The process opens the Maestro store without persisting session, lease or
liveness updates. Mutating commands fail with `READ_ONLY` rather than partially
writing.

Use it for diagnostics, reporting and inspection where even normal session
bookkeeping would be an unwanted side effect.

## What it does not do

- It does not read raw Herdr pane transcript.
- It does not make stale state current.
- It does not grant access to another project store. `maestro search` still
  reads the Hub room at `~/maestro` by default (pass `--local` to skip it);
  that read is itself read-only.
- It admits `term list|show` and `memory list|show` but refuses
  `memory render --check`, which is registered as a writing command.
  `maestro help <verb>` marks the admitted verbs with a trailing `*`.
- An unknown verb under `MAESTRO_READ_ONLY=1` is reported as not admitted,
  which does not mean the verb exists.
