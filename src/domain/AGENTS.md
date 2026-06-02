# Domain Agent Notes

## OVERVIEW

`src/domain/` owns Maestro's durable concepts and their repo-local artifact
contracts.

## STRUCTURE

```text
domain/
├── harness/     # canonical .maestro/harness artifacts
├── task/        # task aggregate, lifecycle, blockers, display, lookup
├── feature/     # feature registry and task-count read models
├── decisions/   # decision records and lookup
├── run/         # hook events, event logs, run evidence
├── proof/       # verification reports, freshness, evidence claims
├── install/     # mirrors, hooks, install lock
├── skills/      # bundled skill catalog, extraction, symlink safety
└── extraction/  # shared resource extraction primitives
```

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Harness templates/config/backlog | `harness/` | User-owned artifacts only change through explicit mutation paths. |
| Task state or blockers | `task/` | Keep lifecycle and optimistic concurrency in Task. |
| Run events or evidence | `run/` | Preserve normalized append and reader tolerance. |
| Proof verification | `proof/` | Proof writes reports; Task applies outcome state. |
| Agent install mirrors | `install/` | Mirrors reference Harness rather than copying it. |
| Skill extraction | `skills/`, `extraction/` | Preserve directory-package resources and rollback behavior. |

## CONVENTIONS

- Each domain module owns data model, path contract, invariants, and write
  policy for its concept.
- Cross-concept interaction goes through narrow facades, not private file
  choreography.
- If changing a public domain contract, update `ARCHITECTURE.md`, `TESTING.md`,
  and the adapter/runtime-flow tests that exercise callers.

## ANTI-PATTERNS (THIS PROJECT)

- Do not let Feature, Run, or Proof mutate Task lifecycle directly.
- Do not make Harness own install, proof, update, or skill extraction behavior.
- Do not add a new public child module unless the parent facade intentionally
  exposes that contract.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [proof/AGENTS.md](proof/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
