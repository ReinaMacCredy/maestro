---
title: Observer mode
description: Read Maestro state without persisting session, lease, or liveness updates.
---

Set `MAESTRO_READ_ONLY=1` for a fail-closed observer process:

```sh
MAESTRO_READ_ONLY=1 maestro status
```

Pure commands remain available, including status, search, recipes, and
read-only list and show operations:

```sh
MAESTRO_READ_ONLY=1 maestro search "release"
MAESTRO_READ_ONLY=1 maestro recipe list
```

## What observer mode protects

Observer mode does not persist session, lease, or liveness updates. Mutating
commands fail with a `READ_ONLY` JSON error envelope, and external plugins are
not loaded. Built-in read paths remain available so a cross-repository brief
can inspect projects without changing their stores.

Search also fails closed. If its index cannot be refreshed in read-only mode,
Maestro reports the stale-index problem instead of returning stale results as
current. Run search once without observer mode to refresh the index, then retry
the read-only query.
