# AGENTS.md

Shipped built-in skill source only. Use this file with the repo-root [AGENTS.md](../AGENTS.md).

## Ownership

- `built-in/*/SKILL.md` is the source of truth for repo-shipped skills.
- `src/infra/domain/built-in-skill-templates.ts` is generated from this tree for compiled releases.
- `.factory/skills/` is reference material for authors and reviewers, not the runtime lookup path.

## Workflow

- Edit files under `skills/built-in/`.
- Regenerate embedded templates with `bun scripts/sync-built-in-skills.ts`.
- Check for drift with `bun run check:skills`.
- `bun run build` also syncs this tree before compile.

## Lookup Rules

- Runtime agent prompt lookup resolves `.maestro/skills/{agentType}/SKILL.md` first.
- If no project-local skill exists, runtime falls back to `skills/built-in/{agentType}/SKILL.md`.

## Local Gotchas

- Do not hand-edit `src/infra/domain/built-in-skill-templates.ts`.
- Keep directory names aligned with the decoded skill name expected by `scripts/sync-built-in-skills.ts`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
