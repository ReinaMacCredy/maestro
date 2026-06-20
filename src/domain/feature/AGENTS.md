# Feature Domain Agent Notes

## OVERVIEW

`src/domain/feature/` owns feature lifecycle, feature-side prose, acceptance
coverage, QA gates, archive behavior, and feature read models.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Lifecycle and feature views | `registry.rs` | Gated transitions, strict/tolerant scans, notes/spec writes. |
| Feature status and schema | `schema.rs` | Normalize acceptance ids consistently. |
| Task counts and child lookups | `query.rs` | Counts are computed on read, not persisted. |
| QA baseline and slices | `qa.rs` | Accept/close gates fail closed on missing or empty baseline data. |
| Feature verification sweep | `verification.rs` | Acceptance coverage and proof summaries. |
| Archive/unarchive | `archive.rs` | Keep card-store and sidecar movement consistent. |

## CONVENTIONS

- Feature records are the product contract; task counts and coverage are read
  models.
- `accept` and `close` layer preconditions over legal lifecycle transitions.
- QA slices count only when they cite scenarios and carry non-empty evidence.
- Keep feature-to-card/task boundaries explicit through facades.

## ANTI-PATTERNS (THIS PROJECT)

- Do not persist derived task counts.
- Do not bypass QA gate helpers from CLI or operations code.
- Do not silently normalize malformed feature records on read.

## VERIFICATION

Start with `tests/feature_domain.rs`,
`tests/feature_decision_commands_integration.rs`,
`tests/feature_qa_gate_integration.rs`, and
`tests/feature_close_suite_integration.rs`. Broaden to card query and task tests
when parent/child behavior changes.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
