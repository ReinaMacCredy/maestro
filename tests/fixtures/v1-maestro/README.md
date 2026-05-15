# tests/fixtures/v1-maestro

Representative v1 `.maestro/` tree used by PR 32+ to exercise `setup migrate-v2`.

Layout:

```
.maestro/
├── evidence/
│   └── 2026-04-01.jsonl
├── handoffs/
├── memory/
│   └── corrections/
│       └── legacy-rule-1.json
├── missions/
├── plans/
├── policies/
│   └── owners.yaml
└── tasks/
    └── tasks.jsonl
```

The fixture is intentionally minimal: enough rows for the migration steps in
PR 33 to assert against, but small enough to inspect by hand.
