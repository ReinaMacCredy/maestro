# Tests Agent Notes

## OVERVIEW

`tests/` proves Maestro contracts across domain modules, adapters,
runtime flows, safety surfaces, resources, and architecture import boundaries.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Import boundaries | `architecture_imports.rs` | Required for module moves and facade policy changes. |
| Task behavior | `task_lifecycle.rs`, `task_artifacts.rs`, `task_commands_integration.rs` | Pair domain and CLI checks as needed. |
| Verification | `task_verify_integration.rs` | Main Proof-to-Task workflow coverage. |
| Install and skills | `install_mirrors.rs`, `install_uninstall_integration.rs`, `skills_*` | Mirrors, locks, symlinks, extraction, rollback. |
| Harness | `harness_templates.rs`, `harness_backlog.rs`, `init_integration.rs` | Template, config, backlog, init output. |
| Run/hooks | `hook_record_integration.rs`, `run_evidence_integration.rs` | Event append and evidence derivation. |
| Update/schema drift | `update_integration.rs` | User-owned artifact safety and compatibility reporting. |
| CLI help | `cli_help.rs` | Root and command help text expectations. |

## CONVENTIONS

- Prefer the smallest test that can falsify the touched contract, then broaden
  when public contracts, path layout, schemas, or safety behavior changed.
- Test fixture writes should go through the same domain/operation path as the
  production behavior unless the test is specifically about malformed input.
- `tests/support.rs` is shared support, not a standalone suite.

## ANTI-PATTERNS (THIS PROJECT)

- Do not prove domain behavior only through CLI output when a contract test is
  available.
- Do not delete edge-case coverage during refactors unless the same behavior is
  covered through the accepted facade.
- Do not skip safety tests for symlink, path containment, rollback, backup, or
  optimistic-concurrency behavior when those policies are touched.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
