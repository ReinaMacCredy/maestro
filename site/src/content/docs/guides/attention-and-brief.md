---
title: Attention and brief
description: Compute project attention at read time and summarize registered repositories.
---

## Scan one repository

```sh
maestro attention
```

Attention is computed from current store state when read. It does not deliver a
mailbox message and does not require a daemon. The detector set is:

- `STALLED_LEASE`
- `REPEATED_FAILURE`
- `DECISION_STALE`
- `SCOPE_COLLISION`
- `DISPATCH_UNRETURNED`
- `HANDBACK_UNREVIEWED`

Threshold flags tune stale leases, draft decisions, and unreturned dispatches.
For a compact machine-readable result, run:

```sh
maestro attention --json
```

## Brief all registered repositories

```sh
maestro brief
```

Brief reads `~/maestro/registry`, opens each registered repository with
`MAESTRO_READ_ONLY=1`, and reports only what needs attention. Missing
repositories are named and skipped. When every repository is running normally,
the brief says so in one line instead of listing ordinary progress.

The Supervisor room's `hm` shell function focuses the `maestro` Herdr workspace
and prints this brief. It returns to the shell and does not start an agent.
