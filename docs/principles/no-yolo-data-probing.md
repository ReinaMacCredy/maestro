# no-yolo-data-probing

## Rule

Do not run ad-hoc shell pipelines (`cat`, `head`, `awk`, `sed`) against `.maestro/` JSONL stores from inside Maestro source code. Read through the typed store port instead (`TaskStorePort`, `EvidenceStorePort`, `ExecPlanStorePort`).

## Rationale

The JSONL files are an append-only journal; reading them via shell skips schema validation, mutation queueing, and the v1/v2 split, which has caused real corruption incidents (see ADR-0011). Routing every read through the port keeps the storage layer swappable and the harness invariant-safe.

## Scan Command

rg -n "(cat|head|tail|awk|sed)\s+[^\"']*\.maestro/(tasks|plans|evidence)" --glob 'src/**' --glob 'scripts/**' --glob '!**/*.test.ts'

## Fix Recipe

1. Replace the shell read with the matching store call (e.g. `await services.taskStore.list()`).
2. If the data isn't reachable via the existing port, add the missing method to the port and adapter rather than parsing JSONL inline.
3. Run `bun test` to confirm behavior is unchanged.
