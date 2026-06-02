# Foundation Core Agent Notes

## OVERVIEW

`src/foundation/core/` provides shared safety primitives used by domains,
operations, and adapters.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Repo paths | `paths.rs`, `managed_path.rs` | Preserve containment and symlink policy. |
| Atomic writes | `safe_write.rs`, `backup.rs` | User files need rollback or recovery stories. |
| Managed blocks | `managed_blocks.rs` | Keep user content outside managed regions. |
| Schema constants | `schema.rs` | Reserved constants need artifact readers before activation. |
| Errors and IO helpers | `error.rs`, `fs.rs` | Library code returns typed/recoverable errors. |
| Hashes, diffs, git snapshots | `hash.rs`, `diff.rs`, `git.rs` | Used by install/update/resource guards. |

## CONVENTIONS

- Foundation stays domain-neutral. It must not depend on domain, operations, or
  interface modules.
- Changes here can affect many safety surfaces; use `srcwalk deps` before
  moving or deleting helpers.
- Path and write helpers should reject unsafe inputs at trust boundaries rather
  than letting callers duplicate checks.

## ANTI-PATTERNS (THIS PROJECT)

- Do not import Task, Harness, Install, or Update from foundation.
- Do not weaken symlink, path traversal, backup, or atomic-write behavior for a
  single caller without proving every dependent contract.
- Do not add schema constants as active versions without migration/reader tests.

## VERIFICATION

Start with `tests/core_paths_fs.rs`, `tests/core_schema_error.rs`,
`tests/core_managed_blocks.rs`, or `tests/core_backup_diff_git.rs`. Broaden to
the owning module tests for any caller whose safety policy changed.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../../AGENTS.md](../../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
