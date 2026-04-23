# Skills

Shipped skill source tree. Use this file with the repo-root [AGENTS.md](../AGENTS.md).

## Ownership

- `built-in/*/SKILL.md` is the source of truth for repo-shipped built-in skills.
- `bundled/*/SKILL.md` is the source of truth for the global installed Maestro skill bundle.
- `src/infra/domain/built-in-skill-templates.ts` and `src/infra/domain/bundled-skill-templates.ts` are generated from this tree.
- `.factory/skills/` is reference material for authors and reviewers, not the runtime lookup path.

## Workflow

- Edit repo-shipped skills under `skills/built-in/`.
- Edit installed global-skill content under `skills/bundled/`.
- Regenerate built-in templates with `bun scripts/sync-built-in-skills.ts`.
- Regenerate bundled templates with `bun scripts/sync-bundled-skills.ts`.
- Check for drift with `bun run check:skills` and `bun run check:bundled-skills`.
- `bun run build` syncs built-in templates before compile; bundled templates still need their dedicated sync/check flow.

## Lookup Rules

- Runtime agent prompt lookup resolves `.maestro/skills/{agentType}/SKILL.md` first.
- If no project-local skill exists, runtime falls back to `skills/built-in/{agentType}/SKILL.md`.
- `maestro install` publishes the bundled skill set from `skills/bundled/` into user-level skill directories.

## Local Gotchas

- Do not hand-edit `src/infra/domain/built-in-skill-templates.ts` or `src/infra/domain/bundled-skill-templates.ts`.
- Keep directory names aligned with the decoded skill name expected by the corresponding sync script.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
