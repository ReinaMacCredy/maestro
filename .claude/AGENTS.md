# .claude Agent Notes

## OVERVIEW

This directory holds local Claude-facing workflow and skill assets for this
checkout. Keep it aligned with the Rust Maestro repo, not the older TypeScript
layout.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Local Claude settings | `.claude/settings.local.json` | Treat as local state; avoid unrelated edits. |
| Workflow scripts | `.claude/workflows/` | JavaScript helpers for local agent workflows. |
| Bundled local skills | `.claude/skills/` | Installed or mirrored skill content; check ownership before editing. |
| Repo-wide rules | `../AGENTS.md` | Root rules remain authoritative. |

## CONVENTIONS

- Keep `CLAUDE.md` in this directory as a tiny shim to `@AGENTS.md`.
- Do not reintroduce paths from the pre-Rust TypeScript Maestro layout.
- Treat settings and generated skill mirrors as local state unless the user
  explicitly asks to edit them.

## ANTI-PATTERNS (THIS PROJECT)

- Do not copy root AGENTS content here.
- Do not update `.claude/settings.local.json` as drive-by cleanup.
- Do not use old commands such as `bun test` or `dist/maestro` for this Rust
  branch unless a legacy task explicitly requires them.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
