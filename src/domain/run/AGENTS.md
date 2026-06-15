# Run Domain Agent Notes

## OVERVIEW

`src/domain/run/` owns hook/manual event normalization, JSONL append safety,
run evidence, active-session read models, and managed run discovery.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Append safety | `append.rs` | Shared hardened append path for hook/manual events and channels. |
| Hook event contract | `event.rs` | Accepted event names, session id normalization, schema fields. |
| Event readers | `reader.rs` | Tolerant reads over managed event logs. |
| Managed discovery | `discovery.rs` | Constrain traversal to expected run files. |
| Evidence records | `evidence.rs` | Proof-facing run evidence load/write behavior. |
| Active sessions | `active.rs` | Current bound-card/session projections. |
| Hook recorder | `record.rs` | Normalize external hook payloads before append. |

## CONVENTIONS

- Hook and manual events share the same append path.
- Append repairs partial trailing lines and writes complete JSONL records.
- Path handling rejects symlink and traversal escapes under `.maestro/runs/`.
- Readers tolerate bad records only where the read model is explicitly tolerant.

## ANTI-PATTERNS (THIS PROJECT)

- Do not add a second append implementation for a new run-like log.
- Do not let Run import Task lifecycle rules.
- Do not turn tolerant readers into mutating repair paths.

## VERIFICATION

Start with `tests/hook_record_integration.rs`,
`tests/run_evidence_integration.rs`, and `tests/active_integration.rs`. Broaden
to Proof and Harness tests when event evidence or intervention capture changes.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
