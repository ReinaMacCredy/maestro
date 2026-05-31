# CLI Agent Notes

## OVERVIEW

`src/interfaces/cli/` owns clap parsing, command dispatch, and terminal output
for the `maestro` binary.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Root command surface | `mod.rs` | `Cli`, `RootCommand`, argument structs, value enums. |
| Task verbs | `task.rs`, `task_id.rs` | Adapter over Task and task verification contracts. |
| Verification command | `verify.rs` | Routes to `operations/task_verify/`. |
| Update command | `update.rs` | Render check/update UX; passive check behavior matters. |
| Query/metrics/improve | `query.rs`, `metrics.rs`, `improve.rs` | Read models and Harness backlog surfaces. |
| MCP/hook/watch adapters | `mcp.rs`, `hook.rs`, `watch.rs` | Thin command entrypoints for non-CLI interfaces. |

## CONVENTIONS

- CLI files parse args, call domain/operation facades, and render output.
- User-visible text changes require command-specific integration tests or
  `tests/cli_help.rs` when help text changes.
- Keep default passive update exclusion logic in `src/main.rs` aligned with
  update/init/MCP/hook/shell behavior.

## ANTI-PATTERNS (THIS PROJECT)

- Do not encode durable artifact layout or lifecycle rules in CLI code when a
  domain facade should own them.
- Do not make CLI output read private files that a read model already exposes.
- Do not make `init --dry-run`, passive update checks, or `update --check`
  mutate Harness files.

## VERIFICATION

For help text, run `tests/cli_help.rs`. For command behavior, run the
command-specific integration test and the owning domain contract test.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../../AGENTS.md](../../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
