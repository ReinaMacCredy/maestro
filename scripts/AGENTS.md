# Scripts

Use this file with the repo-root [AGENTS.md](../AGENTS.md). `scripts/` owns build, version, install, release, and TUI helper entrypoints.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Build pipeline | `build.ts`, `build-lib.ts` | Syncs built-in skills before compile |
| Versioning | `bump.ts`, `auto-bump.ts`, `version-file.ts` | Custom `0.x.y` policy |
| Local release/install | `ci.ts`, `install-local.ts`, `install.sh`, `install.ps1` | `ci.ts` is release-prep, not a no-op check |
| Skill syncing | `sync-built-in-skills.ts` | Source is `skills/built-in/` |
| TUI iteration | `tui-dev.ts`, `test-tty.ts` | Dev watcher and raw TTY probes |

## CONVENTIONS
- Prefer Bun TypeScript entrypoints here; shell and PowerShell wrappers are installation edges only.
- `build.ts` is stateful: it syncs skills, injects build metadata, and handles platform-specific build cleanup.
- `ci.ts` assumes a clean tree and may roll back git state on failure.
- `release:local` installs locally; `deploy` is still a local/test flow unless explicitly changed.

## ANTI-PATTERNS
- Do not put product-domain logic here when it belongs in `src/`.
- Do not assume script names match generic npm conventions.
- Do not bypass shared version helpers when changing version semantics.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
