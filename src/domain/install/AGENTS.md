# Install Domain Agent Notes

## OVERVIEW

`src/domain/install/` owns repo-local agent install state, mirror planning,
managed hook/settings edits, lock state, and uninstall rollback.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Install/uninstall orchestration | `mod.rs` | Pending/committed/removing lock transitions and rollback. |
| Lock schema and ownership | `lock.rs` | File ownership, mirror kind, symlink ownership, agent entries. |
| Mirror writes/removal | `mirrors.rs` | Managed blocks, JSON keys, backups, diffs, rollback. |
| Hook settings | `hooks.rs` | Agent-specific managed hook configuration. |

## CONVENTIONS

- Write pending lock state before mirror writes; restore prior lock state on
  completed rollback.
- User-owned mirror files need visible diffs, backups, and managed-block/key
  boundaries.
- Symlink mirrors are ownership-sensitive; do not remove paths another agent
  still owns.
- Install refresh must not duplicate Harness protocol into mirrors.

## ANTI-PATTERNS (THIS PROJECT)

- Do not overwrite unmanaged user content.
- Do not make `sync` or `update` mutate install mirrors through ad hoc writes.
- Do not skip rollback/error wording when adding a mirror target.

## VERIFICATION

Start with `tests/install_mirrors.rs`,
`tests/install_uninstall_integration.rs`, and
`tests/skills_symlink_integration.rs`. Broaden to update/sync and resource
version guards when mirror content or bundled resources change.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
