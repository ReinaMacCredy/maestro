# tests/fixtures/v1-maestro

Representative v1 `.maestro/` tree used by PR 32+ to exercise `setup migrate-v2`.

Layout:

```
.maestro/
├── evidence/
│   └── 2026-04-01.jsonl
├── memory/
│   └── corrections/
│       └── legacy-rule-1.json
├── missions/
│   └── mis-001/
│       └── mission.json
├── policies/
│   └── owners.yaml
└── tasks/
    └── tasks.jsonl
```

The fixture is intentionally minimal: enough rows for the migration steps in
PR 33 to assert against, but small enough to inspect by hand.

Note: `handoffs/` and `plans/` are not present on disk -- earlier README
versions listed them incorrectly. Phase 6 brownfield scenarios (5-8) consume
this fixture; `setup migrate-v2` creates `plans/` during migration.
