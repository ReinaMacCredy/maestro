# Maestro State

Use this directory with the repo-root `AGENTS.md`. `.maestro/` is the repo-owned project state and guidance surface, not a disposable cache.

## STRUCTURE
```text
.maestro/
├── context/     # durable project guidance and workflow docs
├── tasks/       # tracked daily task queue, contracts, and reusable contract templates
├── plans/       # active planning notes
├── drafts/      # in-progress long-form docs
├── wisdom/      # promoted guidance/history
├── archive/     # historical reference material
└── config.yaml  # project-level Maestro config
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Shared operator guidance | `context/`, `MAESTRO.md` | Read before changing workflow assumptions |
| Daily task queue | `tasks/tasks.jsonl` | Tracked durable state; reasons become history |
| Task contracts | `tasks/contracts/` | Per-task intent/scope/verdict files with append-only `index.jsonl` history |
| Contract draft templates | `tasks/contract-templates/` | Repo-local YAML seeds for `maestro task contract new <id> --from <name>` |
| Local planning corpus | `plans/`, `drafts/`, `wisdom/`, `archive/` | Reference material, not automatically current product truth |
| Retrieval/memory data | `retrieval-index.json`, `feedback.jsonl` | Generated/supporting artifacts |
| Repo config | `config.yaml`, `settings.json` | Project-local Maestro settings |

## CONVENTIONS
- Treat `tasks/tasks.jsonl` as shared durable state. Review edits the same way you would review commit messages.
- `context/` and `MAESTRO.md` are operator-facing guidance; keep them aligned with the current CLI behavior.
- `plans/`, `drafts/`, `wisdom/`, and `archive/` hold useful history, but they are not a substitute for current source verification.
- Runtime skill lookup prefers `.maestro/skills/{agentType}/SKILL.md` before `skills/built-in/{agentType}/SKILL.md`.

## RUNTIME STATE (GITIGNORED)
- `.maestro/evidence/` — per-task evidence rows written by `maestro evidence record`. Gitignored; per-machine only.
- `.maestro/runs/` — per-task run records written by Maestro tooling. Gitignored; per-machine only.

Both directories are created on first use by `maestro init` (or `maestro setup`). Do not commit their contents.

## ANTI-PATTERNS
- Do not put secrets or throwaway venting into tasks, notes, or long-lived plan text.
- Do not treat archived guidance as the current contract without checking source or the current root docs.
- Do not hand-edit generated indices unless you are fixing the generator or deliberately repairing broken state.

@MAESTRO.md

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
