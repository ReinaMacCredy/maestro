# Unused Test And Code Cleanup Matrix

Feature: `safe-unused-test-and-code-cleanup-rollout`
DecisionSet: `decset-safe-unused-test-and-code-cleanup-rollout-ab72d1`

This matrix is the gate before deletion. It uses the static inventory in
`TEST-INVENTORY-test-optimization.md` and the ownership rows in `TESTING.md`.
No test or production code is classified as safe to delete in this first slice.

## Current Ruling

| Category | Files | Ruling | Evidence |
| --- | --- | --- | --- |
| Consolidate now | `tests/cli_help.rs`, `tests/id_only_integration.rs`, `tests/universal_commands.rs`, `tests/card_namespace_integration.rs`, `tests/did_you_mean_integration.rs`, `tests/unknown_subcommand_integration.rs`, `tests/design_integration.rs`, `tests/feature_qa_gate_integration.rs` | Migrate local `Command::new(env!("CARGO_BIN_EXE_maestro"))` helpers to `tests/common/cli_harness.rs`. | The live first slice covers six straight-through CLI helpers, one explicit `HOME` env helper, and one stdin helper after the straight-through path proved stable. No timeout, MCP, session, or domain fixture semantics moved into the generic harness. |
| Keep | `tests/phase3_core_verbs_e2e.rs`, `tests/v1_demo.rs` | Keep until a replacement preserves their unique end-to-end demo coverage. | `TESTING.md` names these as end-to-end demos for broad architecture, schema, or workflow changes. |
| Keep | `tests/architecture_imports.rs`, `tests/architecture_style.rs`, `tests/architecture_write_safety.rs` | Keep as architecture/safety guard coverage. | These tests enforce module boundaries, style constraints, and write-safety contracts rather than ordinary runtime behavior. |
| Keep | `tests/resources_version_guard.rs`, `tests/cli_reference_freshness.rs`, `tests/schema_contracts_validation.rs` | Keep as generated-resource and schema drift guards. | They protect shipped resources and schema/reference freshness. |
| Consolidate later | Env/session-heavy CLI tests such as `tests/active_integration.rs`, `tests/status_next_integration.rs`, `tests/task_commands_integration.rs`, and `tests/doctor_query_integration.rs` | Defer until common env/session patterns are grouped. | Inventory marks these as serial/global or env-sensitive, so generic harness defaults must stay explicit. |
| Consolidate last | Stdin, timeout, spawn, or MCP-heavy tests such as `tests/feature_decision_commands_integration.rs`, `tests/harness_integration.rs`, `tests/hook_record_integration.rs`, `tests/task_verify_integration.rs`, `tests/run_evidence_integration.rs`, and `tests/session_show_integration.rs` | Defer until the simple harness migration proves stable. | Inventory marks these as slow/flake-risk or async-sensitive. |
| Delete review | None confirmed. | No deletion in this slice. | A deletion candidate still needs proof of no unique behavior or replacement coverage first. |

## First Slice Checks

- Do not delete tests.
- Do not edit production Rust.
- Do not move `TestTempDir`, `cards_repo`, `init_repo`, `setup_repo`, or git helpers into the generic process harness.
- Verify every migrated file with targeted commands:
  - `cargo test --test cli_help`
  - `cargo test --test id_only_integration`
  - `cargo test --test universal_commands`
  - `cargo test --test card_namespace_integration`
  - `cargo test --test did_you_mean_integration`
  - `cargo test --test unknown_subcommand_integration`
  - `cargo test --test design_integration`
  - `cargo test --test feature_qa_gate_integration`
