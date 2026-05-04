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
├── contracts/   # versioned contract snapshots (L2; repo-tracked)
├── policies/    # policy files: owners.yaml, sensitive-paths.yaml (repo-tracked)
├── specs/       # per-mission Spec files (acceptance criteria, non-goals) (repo-tracked)
└── config.yaml  # project-level Maestro config
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Shared operator guidance | `context/`, `MAESTRO.md` | Read before changing workflow assumptions |
| Daily task queue | `tasks/tasks.jsonl` | Tracked durable state; reasons become history |
| Task contracts | `tasks/contracts/` | Per-task intent/scope/verdict files with append-only `index.jsonl` history |
| Contract draft templates | `tasks/contract-templates/` | Repo-local YAML seeds for `maestro task contract new <id> --from <name>` |
| Versioned contract store (L2) | `contracts/` | One subdirectory per task id; each version stored as `v<n>.json`; `current.json` symlink or copy |
| Policy files (L2 + L3) | `policies/` | `owners.yaml`, `sensitive-paths.yaml`, `risk.yaml`, `autopilot.yaml`, `release.yaml`; all repo-tracked. `risk.yaml` absent means ROADMAP-default risk policy applies. |
| Mission specs (L2) | `specs/` | Per-mission `spec.json` files; managed via `maestro spec show/edit --mission <id>` |
| Verdict store (L3) | `verdicts/` | Gitignored. One folder per task id (`verdicts/<taskId>/`), one JSON file per verdict version. Derived; not source of truth. |
| Local planning corpus | `plans/`, `drafts/`, `wisdom/`, `archive/` | Reference material, not automatically current product truth |
| Retrieval/memory data | `retrieval-index.json`, `feedback.jsonl` | Generated/supporting artifacts |
| Repo config | `config.yaml`, `settings.json` | Project-local Maestro settings |

## CONVENTIONS
- Treat `tasks/tasks.jsonl` as shared durable state. Review edits the same way you would review commit messages.
- `context/` and `MAESTRO.md` are operator-facing guidance; keep them aligned with the current CLI behavior.
- `plans/`, `drafts/`, `wisdom/`, and `archive/` hold useful history, but they are not a substitute for current source verification.
- Runtime skill lookup prefers `.maestro/skills/{agentType}/SKILL.md` before `skills/built-in/{agentType}/SKILL.md`.
- `contracts/`, `policies/`, and `specs/` are committed. They are durable policy artifacts, not ephemeral state.
- `evidence/` and `runs/` are gitignored and per-machine. Do not commit their contents.

## POLICIES — owners.yaml
`.maestro/policies/owners.yaml` defines decision-authority roles. Schema (snake_case keys, lists of GitHub usernames or team handles):

```yaml
policy_approver: []      # approves changes to .maestro/policies/ (L3+)
ratchet_approver: []     # approves ratchet promotions (L7+)
sensitive_waiver: []     # signs off on changes to sensitive paths (L5+)
```

At L2, the file must be present and parseable. Role lists may be empty (defaults to "any maintainer"). The `gh` CLI is used at higher levels to resolve role membership; at L2, role lookup is raw list comparison only. See `docs/owners-yaml-format.md` for the full schema reference.

## RUNTIME STATE (GITIGNORED)
- `.maestro/evidence/` — per-task evidence rows written by `maestro evidence record`. Gitignored; per-machine only.
- `.maestro/runs/` — per-task run records written by Maestro tooling. Gitignored; per-machine only.
- `.maestro/verdicts/` — per-task verdict history written by `maestro verdict request`. Gitignored; derived state, not source of truth. One subfolder per task id; one JSON per verdict version.
- `.maestro/policies/.pending-loosenings.json` — gitignored derived cache of in-soak policy loosenings. Written by `maestro policy check` and read by `maestro policy pending`.

Both `evidence/` and `runs/` are created on first use by `maestro init` (or `maestro setup`). Do not commit their contents or any gitignored policy cache.

All other directories under `.maestro/` (including `contracts/`, `policies/`, `specs/`, `tasks/`) are committed and repo-tracked.

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
