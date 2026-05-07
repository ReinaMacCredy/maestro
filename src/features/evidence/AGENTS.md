# Evidence Feature

Use with the parent [AGENTS.md](../AGENTS.md). `src/features/evidence/` owns the task evidence logbook: recording, listing, and showing verifiable output rows tied to a task.

## STRUCTURE
```text
evidence/
├── commands/     # `maestro evidence record|list|show`
├── usecases/     # record-evidence, list-evidence
├── domain/       # types.ts (EvidenceKind, WitnessLevel, EvidenceRow), evidence-id.ts
├── ports/        # storage.ts (EvidenceStore port + EvidenceListFilter)
├── adapters/     # file-storage.ts (NDJSON files under .maestro/evidence/<taskId>.ndjson)
├── services.ts   # evidence-store factory
└── index.ts      # public surface
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Domain types | `domain/types.ts` | `EvidenceKind` (`command`\|`manual-note`), `WitnessLevel`, `EvidenceRow` |
| CLI behavior | `commands/evidence.command.ts` | `record`, `list`, `show` subcommands |
| Storage layout | `adapters/file-storage.ts` | One NDJSON file per task under `.maestro/evidence/` |
| Record logic | `usecases/record-evidence.usecase.ts` | Validates input, assigns id, calls store |
| List logic | `usecases/list-evidence.usecase.ts` | Applies `EvidenceListFilter` (task, session, kind) |

## CONVENTIONS
- Evidence rows are append-only; there is no update or delete path.
- `WitnessLevel` tracks claim trustworthiness: `witnessed-by-maestro` > `witnessed-by-ci` > `agent-claimed-locally` > `agent-claimed-and-not-reproducible`.
- Storage is gitignored (`.maestro/evidence/`). Do not add evidence files to repo tracking.
- `--criterion <id>` links a row to a task contract criterion; optional.

## ANTI-PATTERNS
- Do not read evidence rows from inside another feature; consume through this feature's public surface only.
- Do not mutate existing rows; append new ones.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
