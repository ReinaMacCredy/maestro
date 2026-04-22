# Shared Utilities

Use with the parent [AGENTS.md](../AGENTS.md). `src/shared/` holds generic utilities with no product-domain knowledge.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Filesystem helpers | `lib/fs.ts` | `ensureDir`, `readText`, `writeText`, `writeJson`, `appendText`, `removeIfExists` |
| YAML utilities | `lib/yaml.ts` | `parseYaml`, `stringifyYaml`, `deepMerge` |
| Shell execution | `lib/shell.ts` | Cross-platform shell runners |
| Path safety | `lib/path-safety.ts`, `lib/path-normalize.ts` | Validation + normalization |
| Output formatting | `lib/output.ts`, `lib/output-capture.ts` | Stdout/stderr helpers |
| Error base class | `errors.ts` | `MaestroError` with hints array |
| Domain primitives | `domain/id.ts`, `domain/defaults.ts`, `domain/ui-config.ts` | IDs, constants, UI config |

## CONVENTIONS
- No product-domain logic here. Generic I/O, shell, YAML, path, and output helpers only.
- `lib/fs.ts` includes `renameForInPlaceReplace` for atomic file writes.
- `errors.ts` provides `MaestroError` with a `hints` array for actionable guidance.

## ANTI-PATTERNS
- Moving feature-owned domain logic into `shared/`.
- Using `shared/` as a dumping ground for code that belongs in a feature.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
