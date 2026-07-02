# Test Optimization Static Inventory

Feature: `optimize-test-runtime-and-architecture`
SPEC: ./SPEC-test-optimization.md
VERIFY: ./VERIFY-test-optimization.md

Scope: static scan of `tests/*.rs` only. This is literal/source evidence, not runtime proof; `cargo test --workspace -- --list` is currently blocked by compile errors recorded in the SPEC.

## Summary

- Rust test files scanned: 73
- Files with local `maestro` binary helpers or helper functions: 49
- Files using `Command::new`: 47
- Files using temp/repo/card setup markers: 59
- Files setting notable `HOME`/`MAESTRO_*` env vars: 28
- Files with stdin/spawn/timeout/sleep/try_wait markers: 9

Behavior label counts: CLI integration=45, architecture=6, install/global=7, unit/domain=15, workflow/e2e=32

Cost/risk label counts: fast=7, filesystem=64, process=47, serial/global=29, slow/flake-risk=9

Most common local helper names: maestro=42, stdout=23, assert_success=12, maestro_with_env=12, run=11, assert_failure=9, stderr=8, git=7, run_failure=2, run_success=2, maestro_clean_env_with=2, maestro_record=2, run_with_env=2, run_log=1, run_output=1, run_domain_facade_does_not_publish_leaf_modules=1, run_domain_may_expose_task_ids_but_must_not_import_task=1, maestro_in_session=1, run_as=1, run_err=1

## Harness pattern buckets

### A. Direct maestro binary process helpers

- `tests/active_integration.rs`: bin=1; helpers=maestro,run,run_failure,run_log,run_output; cwd=1; env=MAESTRO_AGENT:1,MAESTRO_SESSION_ID:37; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/card_commands_integration.rs`: bin=4; helpers=git,maestro,maestro_in_session,run,run_as,run_err,run_lease_as_json; cwd=5; env=MAESTRO_AGENT:4,MAESTRO_SESSION:3,MAESTRO_SESSION_ID:1,MAESTRO_RUN_ID:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/card_namespace_integration.rs`: bin=1; helpers=maestro,stdout; cwd=1; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/card_query_e2e.rs`: bin=2; helpers=maestro,maestro_claim,run; cwd=2; env=MAESTRO_AGENT:2,MAESTRO_SESSION:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/cli_help.rs`: bin=2; helpers=maestro; cwd=1; behavior=CLI integration; cost=process/filesystem
- `tests/core_paths_fs.rs`: helpers=maestro_paths_construct_expected_artifact_dirs; behavior=unit/domain; cost=filesystem/serial/global
- `tests/design_integration.rs`: bin=1; helpers=maestro; cwd=1; env=HOME:1; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/did_you_mean_integration.rs`: bin=1; helpers=maestro,stdout; cwd=1; behavior=CLI integration; cost=process/filesystem
- `tests/doctor_query_integration.rs`: bin=2; helpers=assert_failure,assert_success,maestro,maestro_files,maestro_with_env,run_success,stderr,stdout; cwd=2; env=HOME:2,MAESTRO_AGENT:2,MAESTRO_CURRENT_TASK:3; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/feature_close_suite_integration.rs`: bin=1; helpers=assert_failure,maestro,stdout; cwd=1; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/feature_decision_commands_integration.rs`: bin=6; helpers=assert_failure,maestro,maestro_owned,maestro_with_env,maestro_with_timeout,stdout; cwd=6; io/async=stdout:1,stderr:1,spawn:3,timeout:7,duration:2,sleep:1,try_wait:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/slow/flake-risk
- `tests/feature_qa_gate_integration.rs`: bin=2; helpers=assert_failure,maestro,maestro_with_stdin,stdout; cwd=2; io/async=stdin:1,stdout:1,stderr:1,spawn:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/slow/flake-risk
- `tests/feature_verify_autoclose_integration.rs`: bin=1; helpers=maestro,stdout; cwd=1; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/global_skills_integration.rs`: bin=1; helpers=assert_failure,assert_success,maestro; cwd=1; env=HOME:1; behavior=install/global/CLI integration; cost=process/filesystem/serial/global
- `tests/grep_memory_integration.rs`: bin=1; helpers=maestro,stdout; cwd=1; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_output_freshness_integration.rs`: bin=2; helpers=git,maestro,maestro_with_env,stdout; cwd=3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_ranking_integration.rs`: bin=1; helpers=git,maestro,stdout; cwd=2; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_source_integration.rs`: bin=2; helpers=git,maestro,maestro_with_env,stdout; cwd=3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_transcript_query_integration.rs`: bin=1; helpers=git,maestro,stdout; cwd=2; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_transcript_wiring_integration.rs`: bin=2; helpers=git,maestro,maestro_with_env,stdout; cwd=3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/harness_complete_readout_integration.rs`: bin=1; helpers=maestro,maestro_with_extra_env,run,run_failure,run_failure_with_home,run_with_home; cwd=1; env=HOME:2,MAESTRO_AGENT:1,MAESTRO_SESSION_ID:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/harness_integration.rs`: bin=3; helpers=assert_success,maestro,maestro_with_env,run_mcp_bytes,run_mcp_bytes_raw,run_mcp_requests,run_success,stderr,stdout; cwd=3; env=MAESTRO_SESSION_ID:4; io/async=stdin:1,stdout:1,stderr:1,spawn:1,duration:1,sleep:1,try_wait:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/harness_templates.rs`: helpers=maestro_paths_include_phase_2_artifact_locations; behavior=workflow/e2e; cost=filesystem
- `tests/hook_record_integration.rs`: bin=6; helpers=maestro,maestro_clean_env_with,maestro_record,maestro_record_args_clean_env_with,maestro_record_clean_env_with,maestro_with_env,maestro_without_session_env; cwd=6; env=MAESTRO_AGENT:5,MAESTRO_SESSION_ID:3,MAESTRO_RUN_ID:1,MAESTRO_CURRENT_TASK:2; io/async=stdin:2,stdout:2,stderr:2,spawn:2; behavior=CLI integration; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/id_only_integration.rs`: bin=1; helpers=maestro,stdout; cwd=1; behavior=CLI integration; cost=process/filesystem
- `tests/init_integration.rs`: bin=2; helpers=maestro,maestro_with_clean_agent_env; cwd=2; env=MAESTRO_AGENT:3; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/install_uninstall_integration.rs`: bin=1; helpers=maestro; cwd=1; env=HOME:1; behavior=install/global/CLI integration; cost=process/filesystem/serial/global
- `tests/loop_recipes_integration.rs`: bin=2; helpers=maestro,maestro_with_env,stderr,stdout; cwd=2; env=MAESTRO_SESSION_ID:1; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/memory_commands_integration.rs`: bin=1; helpers=maestro,run; cwd=1; env=MAESTRO_AGENT:1,MAESTRO_SESSION:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/mission_control_integration.rs`: bin=1; helpers=maestro,run; cwd=1; env=MAESTRO_AGENT:1,MAESTRO_SESSION:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/msg_codex_delivery_integration.rs`: bin=1; helpers=maestro,run; cwd=1; env=MAESTRO_AGENT:1,MAESTRO_SESSION_ID:1,MAESTRO_RUN_ID:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/phase3_core_verbs_e2e.rs`: bin=2; helpers=assert_success,maestro,run,run_with_env,stdout; cwd=2; env=MAESTRO_AGENT:1; behavior=unit/domain/CLI integration; cost=process/filesystem/serial/global
- `tests/project_scope_read_surface.rs`: bin=1; helpers=maestro,run; cwd=1; env=MAESTRO_AGENT:1,MAESTRO_SESSION:1; behavior=unit/domain; cost=process/serial/global
- `tests/projects_lifecycle.rs`: bin=1; helpers=maestro; cwd=1; env=HOME:1; behavior=unit/domain; cost=process/filesystem/serial/global
- `tests/resources_version_guard.rs`: helpers=maestro_card_skill_keeps_explicit_unattended_loop_triggers; behavior=architecture; cost=fast
- `tests/run_evidence_integration.rs`: bin=1; helpers=maestro_record,run_evidence; cwd=1; io/async=stdin:1,stdout:1,stderr:1,spawn:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/slow/flake-risk
- `tests/schema_fixture_harness.rs`: bin=1; helpers=assert_success,maestro,run_event_fixture_identity_is_runtime_neutral; cwd=1; behavior=unit/domain; cost=process/filesystem
- `tests/session_show_integration.rs`: bin=2; helpers=maestro,run; cwd=2; env=MAESTRO_AGENT:1,MAESTRO_SESSION_ID:1; io/async=stdin:1,stdout:1,stderr:1,spawn:1; behavior=CLI integration; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/shell_init_integration.rs`: bin=2; helpers=maestro_shell_init; behavior=install/global/CLI integration; cost=process/filesystem
- `tests/skills_symlink_integration.rs`: bin=1; cwd=1; env=HOME:1; behavior=install/global/CLI integration; cost=process/filesystem/serial/global
- `tests/status_next_integration.rs`: bin=2; helpers=assert_failure,assert_success,maestro,maestro_with_env,run,stderr,stdout; cwd=2; env=MAESTRO_SESSION_ID:3,MAESTRO_CURRENT_TASK:7; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/sync_integration.rs`: bin=1; helpers=maestro; cwd=1; env=HOME:1; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/task_commands_integration.rs`: bin=2; helpers=assert_failure,assert_success,maestro,maestro_with_env,stderr,stdout; cwd=2; env=MAESTRO_CURRENT_TASK:3; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/task_verify_integration.rs`: bin=3; helpers=assert_failure,assert_success,maestro,maestro_clean_env_with,stderr,stdout; cwd=3; env=MAESTRO_AGENT:3,MAESTRO_SESSION_ID:2,MAESTRO_RUN_ID:1; io/async=stdin:1,stdout:1,stderr:1,spawn:1; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/universal_commands.rs`: bin=1; helpers=assert_success,maestro,stderr,stdout; cwd=1; env=HOME:1; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/unknown_subcommand_integration.rs`: bin=1; helpers=maestro; behavior=CLI integration; cost=process
- `tests/update_integration.rs`: bin=9; helpers=assert_success,maestro; cwd=11; env=HOME:9; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/v1_demo.rs`: bin=2; helpers=assert_success,maestro_with_env,run_with_env,stdout; cwd=2; env=HOME:1; io/async=stdin:1,stdout:1,stderr:1,spawn:1; behavior=unit/domain; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/worktree_ledger_integration.rs`: bin=2; helpers=assert_failure,assert_success,git,maestro,maestro_with_env,stderr,stdout; cwd=4; env=MAESTRO_SESSION_ID:2; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global

### B. Fixture/setup-heavy files

- `tests/active_integration.rs`: cards_repo:22; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/card_commands_integration.rs`: TestTempDir:2, cards_repo:65; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/card_namespace_integration.rs`: TestTempDir:3, init_repo:3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/card_query_e2e.rs`: TestTempDir:17; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/cli_help.rs`: TestTempDir:2; behavior=CLI integration; cost=process/filesystem
- `tests/core_backup_diff_git.rs`: TestTempDir:11; behavior=unit/domain; cost=filesystem
- `tests/core_paths_fs.rs`: TestTempDir:10; behavior=unit/domain; cost=filesystem/serial/global
- `tests/decision_domain.rs`: cards_repo:6; behavior=unit/domain; cost=filesystem
- `tests/design_integration.rs`: TestTempDir:3, init_repo:7; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/did_you_mean_integration.rs`: TestTempDir:3, init_repo:4; behavior=CLI integration; cost=process/filesystem
- `tests/doctor_query_integration.rs`: TestTempDir:8, setup_repo:27; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/extraction_rollback.rs`: TestTempDir:4; behavior=unit/domain; cost=filesystem
- `tests/feature_close_suite_integration.rs`: TestTempDir:7; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/feature_decision_artifacts.rs`: TestTempDir:3; behavior=workflow/e2e; cost=filesystem
- `tests/feature_decision_commands_integration.rs`: TestTempDir:68; behavior=CLI integration/workflow/e2e; cost=process/filesystem/slow/flake-risk
- `tests/feature_domain.rs`: TestTempDir:33; behavior=workflow/e2e; cost=filesystem
- `tests/feature_qa_gate_integration.rs`: TestTempDir:14; behavior=CLI integration/workflow/e2e; cost=process/filesystem/slow/flake-risk
- `tests/feature_verify_autoclose_integration.rs`: TestTempDir:9; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/global_skills_integration.rs`: TestTempDir:5, init_repo:4; behavior=install/global/CLI integration; cost=process/filesystem/serial/global
- `tests/grep_memory_integration.rs`: TestTempDir:3, cards_repo:7; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_output_freshness_integration.rs`: TestTempDir:3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_ranking_integration.rs`: TestTempDir:3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_source_integration.rs`: TestTempDir:6; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_transcript_query_integration.rs`: TestTempDir:3; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/grep_transcript_store_integration.rs`: TestTempDir:2; behavior=CLI integration/workflow/e2e; cost=filesystem
- `tests/grep_transcript_wiring_integration.rs`: TestTempDir:6; behavior=CLI integration/workflow/e2e; cost=process/filesystem
- `tests/harness_backlog.rs`: TestTempDir:11; behavior=workflow/e2e; cost=filesystem
- `tests/harness_complete_readout_integration.rs`: TestTempDir:6, cards_repo:2; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/harness_extract.rs`: TestTempDir:6; behavior=workflow/e2e; cost=filesystem
- `tests/harness_integration.rs`: TestTempDir:7, setup_repo:56; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/harness_templates.rs`: TestTempDir:8; behavior=workflow/e2e; cost=filesystem
- `tests/hook_extract.rs`: TestTempDir:6; behavior=unit/domain; cost=filesystem
- `tests/hook_record_integration.rs`: TestTempDir:6, init_repo:36; behavior=CLI integration; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/id_only_integration.rs`: TestTempDir:3, init_repo:7; behavior=CLI integration; cost=process/filesystem
- `tests/init_integration.rs`: TestTempDir:19; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/install_mirrors.rs`: TestTempDir:12, init_repo:8; behavior=install/global; cost=filesystem/slow/flake-risk
- `tests/install_uninstall_integration.rs`: TestTempDir:49, init_repo:43; behavior=install/global/CLI integration; cost=process/filesystem/serial/global
- `tests/local_install_script.rs`: TestTempDir:2; behavior=install/global; cost=process/filesystem
- `tests/loop_recipes_integration.rs`: TestTempDir:26; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/memory_commands_integration.rs`: cards_repo:2; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/mission_control_integration.rs`: cards_repo:2; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/msg_codex_delivery_integration.rs`: cards_repo:3; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/phase3_core_verbs_e2e.rs`: TestTempDir:2; behavior=unit/domain/CLI integration; cost=process/filesystem/serial/global
- `tests/project_scope_read_surface.rs`: cards_repo:9; behavior=unit/domain; cost=process/serial/global
- `tests/projects_lifecycle.rs`: TestTempDir:5; behavior=unit/domain; cost=process/filesystem/serial/global
- `tests/run_evidence_integration.rs`: TestTempDir:4, init_repo:10; behavior=CLI integration/workflow/e2e; cost=process/filesystem/slow/flake-risk
- `tests/schema_fixture_harness.rs`: TestTempDir:6; behavior=unit/domain; cost=process/filesystem
- `tests/session_show_integration.rs`: cards_repo:5; behavior=CLI integration; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/shell_init_integration.rs`: TestTempDir:4; behavior=install/global/CLI integration; cost=process/filesystem
- `tests/skills_symlink_integration.rs`: TestTempDir:5, init_repo:5; behavior=install/global/CLI integration; cost=process/filesystem/serial/global
- `tests/status_next_integration.rs`: TestTempDir:6, setup_repo:40; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/support.rs`: TestTempDir:3; behavior=unit/domain; cost=filesystem
- `tests/sync_integration.rs`: TestTempDir:9; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/task_commands_integration.rs`: TestTempDir:4, setup_repo:49; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global
- `tests/task_verify_integration.rs`: TestTempDir:8, setup_repo:49; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/universal_commands.rs`: TestTempDir:3; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/update_integration.rs`: TestTempDir:24; behavior=CLI integration; cost=process/filesystem/serial/global
- `tests/v1_demo.rs`: TestTempDir:3; behavior=unit/domain; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/worktree_ledger_integration.rs`: TestTempDir:3, setup_repo:5; behavior=CLI integration/workflow/e2e; cost=process/filesystem/serial/global

### C. Env/session/global-state files

- `tests/active_integration.rs`: MAESTRO_AGENT:1, MAESTRO_SESSION_ID:37; cost=process/filesystem/serial/global
- `tests/card_commands_integration.rs`: MAESTRO_AGENT:4, MAESTRO_SESSION:3, MAESTRO_SESSION_ID:1, MAESTRO_RUN_ID:1; cost=process/filesystem/serial/global
- `tests/card_query_e2e.rs`: MAESTRO_AGENT:2, MAESTRO_SESSION:1; cost=process/filesystem/serial/global
- `tests/design_integration.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/doctor_query_integration.rs`: HOME:2, MAESTRO_AGENT:2, MAESTRO_CURRENT_TASK:3; cost=process/filesystem/serial/global
- `tests/global_skills_integration.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/harness_complete_readout_integration.rs`: HOME:2, MAESTRO_AGENT:1, MAESTRO_SESSION_ID:1; cost=process/filesystem/serial/global
- `tests/harness_integration.rs`: MAESTRO_SESSION_ID:4; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/hook_record_integration.rs`: MAESTRO_AGENT:5, MAESTRO_SESSION_ID:3, MAESTRO_RUN_ID:1, MAESTRO_CURRENT_TASK:2; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/init_integration.rs`: MAESTRO_AGENT:3; cost=process/filesystem/serial/global
- `tests/install_uninstall_integration.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/loop_recipes_integration.rs`: MAESTRO_SESSION_ID:1; cost=process/filesystem/serial/global
- `tests/memory_commands_integration.rs`: MAESTRO_AGENT:1, MAESTRO_SESSION:1; cost=process/filesystem/serial/global
- `tests/mission_control_integration.rs`: MAESTRO_AGENT:1, MAESTRO_SESSION:1; cost=process/filesystem/serial/global
- `tests/msg_codex_delivery_integration.rs`: MAESTRO_AGENT:1, MAESTRO_SESSION_ID:1, MAESTRO_RUN_ID:1; cost=process/filesystem/serial/global
- `tests/phase3_core_verbs_e2e.rs`: MAESTRO_AGENT:1; cost=process/filesystem/serial/global
- `tests/project_scope_read_surface.rs`: MAESTRO_AGENT:1, MAESTRO_SESSION:1; cost=process/serial/global
- `tests/projects_lifecycle.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/session_show_integration.rs`: MAESTRO_AGENT:1, MAESTRO_SESSION_ID:1; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/skills_symlink_integration.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/status_next_integration.rs`: MAESTRO_SESSION_ID:3, MAESTRO_CURRENT_TASK:7; cost=process/filesystem/serial/global
- `tests/sync_integration.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/task_commands_integration.rs`: MAESTRO_CURRENT_TASK:3; cost=process/filesystem/serial/global
- `tests/task_verify_integration.rs`: MAESTRO_AGENT:3, MAESTRO_SESSION_ID:2, MAESTRO_RUN_ID:1; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/universal_commands.rs`: HOME:1; cost=process/filesystem/serial/global
- `tests/update_integration.rs`: HOME:9; cost=process/filesystem/serial/global
- `tests/v1_demo.rs`: HOME:1; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/worktree_ledger_integration.rs`: MAESTRO_SESSION_ID:2; cost=process/filesystem/serial/global

### D. Async/stdin/timeout-sensitive files

- `tests/feature_decision_commands_integration.rs`: spawn:3, timeout:7, Duration::from_secs:2, thread::sleep:1, try_wait:1; cost=process/filesystem/slow/flake-risk
- `tests/feature_qa_gate_integration.rs`: stdin:1, spawn:1; cost=process/filesystem/slow/flake-risk
- `tests/harness_integration.rs`: stdin:1, spawn:1, Duration::from_secs:1, thread::sleep:1, try_wait:1; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/hook_record_integration.rs`: stdin:2, spawn:2; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/install_mirrors.rs`: timeout:6; cost=filesystem/slow/flake-risk
- `tests/run_evidence_integration.rs`: stdin:1, spawn:1; cost=process/filesystem/slow/flake-risk
- `tests/session_show_integration.rs`: stdin:1, spawn:1; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/task_verify_integration.rs`: stdin:1, spawn:1; cost=process/filesystem/serial/global/slow/flake-risk
- `tests/v1_demo.rs`: stdin:1, spawn:1; cost=process/filesystem/serial/global/slow/flake-risk

## Per-file dual-axis map

| File | Behavior labels | Cost/risk labels | Helper/process notes |
|---|---|---|---|
| `tests/active_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: maestro, run, run_failure, run_log, run_output; maestro_bin=1; Command::new=1; cwd=1; cards_repo=22; env |
| `tests/architecture_imports.rs` | architecture | filesystem | helpers: run_domain_facade_does_not_publish_leaf_modules, run_domain_may_expose_task_ids_but_must_not_import_task |
| `tests/architecture_style.rs` | architecture | filesystem | none |
| `tests/architecture_write_safety.rs` | architecture | filesystem | none |
| `tests/card_commands_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: git, maestro, maestro_in_session, run, run_as, run_err; maestro_bin=4; Command::new=5; cwd=5; TestTempDir=2; cards_repo=65; env |
| `tests/card_namespace_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: maestro, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=3 |
| `tests/card_query_e2e.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: maestro, maestro_claim, run; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=17; env |
| `tests/cli_help.rs` | CLI integration | process, filesystem | helpers: maestro; maestro_bin=2; Command::new=2; cwd=1; TestTempDir=2 |
| `tests/cli_reference_freshness.rs` | architecture | filesystem | none |
| `tests/core_backup_diff_git.rs` | unit/domain | filesystem | TestTempDir=11 |
| `tests/core_managed_blocks.rs` | unit/domain | filesystem | none |
| `tests/core_paths_fs.rs` | unit/domain | filesystem, serial/global | helpers: maestro_paths_construct_expected_artifact_dirs; TestTempDir=10 |
| `tests/core_schema_error.rs` | unit/domain | fast | none |
| `tests/decision_domain.rs` | unit/domain | filesystem | cards_repo=6 |
| `tests/design_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: maestro; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=3; env |
| `tests/did_you_mean_integration.rs` | CLI integration | process, filesystem | helpers: maestro, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=3 |
| `tests/doctor_query_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: assert_failure, assert_success, maestro, maestro_files, maestro_with_env, run_success; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=8; env |
| `tests/extraction_rollback.rs` | unit/domain | filesystem | TestTempDir=4 |
| `tests/feature_close_suite_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: assert_failure, maestro, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=7 |
| `tests/feature_decision_artifacts.rs` | workflow/e2e | filesystem | TestTempDir=3 |
| `tests/feature_decision_commands_integration.rs` | CLI integration, workflow/e2e | process, filesystem, slow/flake-risk | helpers: assert_failure, maestro, maestro_owned, maestro_with_env, maestro_with_timeout, stdout; maestro_bin=6; Command::new=6; cwd=6; TestTempDir=68; async/io |
| `tests/feature_domain.rs` | workflow/e2e | filesystem | TestTempDir=33 |
| `tests/feature_qa_gate_integration.rs` | CLI integration, workflow/e2e | process, filesystem, slow/flake-risk | helpers: assert_failure, maestro, maestro_with_stdin, stdout; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=14; async/io |
| `tests/feature_verify_autoclose_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: maestro, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=9 |
| `tests/global_skills_integration.rs` | install/global, CLI integration | process, filesystem, serial/global | helpers: assert_failure, assert_success, maestro; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=5; env |
| `tests/grep_memory_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: maestro, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=3; cards_repo=7 |
| `tests/grep_output_freshness_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: git, maestro, maestro_with_env, stdout; maestro_bin=2; Command::new=3; cwd=3; TestTempDir=3 |
| `tests/grep_ranking_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: git, maestro, stdout; maestro_bin=1; Command::new=2; cwd=2; TestTempDir=3 |
| `tests/grep_source_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: git, maestro, maestro_with_env, stdout; maestro_bin=2; Command::new=3; cwd=3; TestTempDir=6 |
| `tests/grep_transcript_codex_provider_integration.rs` | CLI integration, workflow/e2e | fast | none |
| `tests/grep_transcript_provider_integration.rs` | CLI integration, workflow/e2e | fast | none |
| `tests/grep_transcript_query_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: git, maestro, stdout; maestro_bin=1; Command::new=2; cwd=2; TestTempDir=3 |
| `tests/grep_transcript_store_integration.rs` | CLI integration, workflow/e2e | filesystem | TestTempDir=2 |
| `tests/grep_transcript_wiring_integration.rs` | CLI integration, workflow/e2e | process, filesystem | helpers: git, maestro, maestro_with_env, stdout; maestro_bin=2; Command::new=3; cwd=3; TestTempDir=6 |
| `tests/harness_backlog.rs` | workflow/e2e | filesystem | TestTempDir=11 |
| `tests/harness_complete_readout_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: maestro, maestro_with_extra_env, run, run_failure, run_failure_with_home, run_with_home; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=6; cards_repo=2; env |
| `tests/harness_extract.rs` | workflow/e2e | filesystem | TestTempDir=6 |
| `tests/harness_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global, slow/flake-risk | helpers: assert_success, maestro, maestro_with_env, run_mcp_bytes, run_mcp_bytes_raw, run_mcp_requests; maestro_bin=3; Command::new=3; cwd=3; TestTempDir=7; env; async/io |
| `tests/harness_templates.rs` | workflow/e2e | filesystem | helpers: maestro_paths_include_phase_2_artifact_locations; TestTempDir=8 |
| `tests/hook_extract.rs` | unit/domain | filesystem | TestTempDir=6 |
| `tests/hook_record_integration.rs` | CLI integration | process, filesystem, serial/global, slow/flake-risk | helpers: maestro, maestro_clean_env_with, maestro_record, maestro_record_args_clean_env_with, maestro_record_clean_env_with, maestro_with_env; maestro_bin=6; Command::new=6; cwd=6; TestTempDir=6; env; async/io |
| `tests/id_only_integration.rs` | CLI integration | process, filesystem | helpers: maestro, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=3 |
| `tests/init_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: maestro, maestro_with_clean_agent_env; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=19; env |
| `tests/install_mirrors.rs` | install/global | filesystem, slow/flake-risk | TestTempDir=12; async/io |
| `tests/install_uninstall_integration.rs` | install/global, CLI integration | process, filesystem, serial/global | helpers: maestro; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=49; env |
| `tests/local_install_script.rs` | install/global | process, filesystem | Command::new=1; cwd=1; TestTempDir=2 |
| `tests/loop_recipes_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: maestro, maestro_with_env, stderr, stdout; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=26; env |
| `tests/memory_commands_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: maestro, run; maestro_bin=1; Command::new=1; cwd=1; cards_repo=2; env |
| `tests/mission_control_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: maestro, run; maestro_bin=1; Command::new=1; cwd=1; cards_repo=2; env |
| `tests/msg_codex_delivery_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: maestro, run; maestro_bin=1; Command::new=1; cwd=1; cards_repo=3; env |
| `tests/phase3_core_verbs_e2e.rs` | unit/domain, CLI integration | process, filesystem, serial/global | helpers: assert_success, maestro, run, run_with_env, stdout; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=2; env |
| `tests/project_scope_read_surface.rs` | unit/domain | process, serial/global | helpers: maestro, run; maestro_bin=1; Command::new=1; cwd=1; cards_repo=9; env |
| `tests/projects_lifecycle.rs` | unit/domain | process, filesystem, serial/global | helpers: maestro; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=5; env |
| `tests/resources_version_guard.rs` | architecture | fast | helpers: maestro_card_skill_keeps_explicit_unattended_loop_triggers |
| `tests/run_evidence_integration.rs` | CLI integration, workflow/e2e | process, filesystem, slow/flake-risk | helpers: maestro_record, run_evidence; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=4; async/io |
| `tests/schema_contracts_validation.rs` | architecture | fast | none |
| `tests/schema_fixture_harness.rs` | unit/domain | process, filesystem | helpers: assert_success, maestro, run_event_fixture_identity_is_runtime_neutral; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=6 |
| `tests/session_show_integration.rs` | CLI integration | process, filesystem, serial/global, slow/flake-risk | helpers: maestro, run; maestro_bin=2; Command::new=2; cwd=2; cards_repo=5; env; async/io |
| `tests/setup_skill_context_readin.rs` | install/global | fast | none |
| `tests/shell_init_integration.rs` | install/global, CLI integration | process, filesystem | helpers: maestro_shell_init; maestro_bin=2; Command::new=6; TestTempDir=4 |
| `tests/skills_symlink_integration.rs` | install/global, CLI integration | process, filesystem, serial/global | maestro_bin=1; Command::new=1; cwd=1; TestTempDir=5; env |
| `tests/status_next_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: assert_failure, assert_success, maestro, maestro_with_env, run, stderr; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=6; env |
| `tests/support.rs` | unit/domain | filesystem | TestTempDir=3 |
| `tests/sync_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: maestro; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=9; env |
| `tests/task_artifacts.rs` | unit/domain, workflow/e2e | fast | none |
| `tests/task_commands_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: assert_failure, assert_success, maestro, maestro_with_env, stderr, stdout; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=4; env |
| `tests/task_verify_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global, slow/flake-risk | helpers: assert_failure, assert_success, maestro, maestro_clean_env_with, stderr, stdout; maestro_bin=3; Command::new=3; cwd=3; TestTempDir=8; env; async/io |
| `tests/universal_commands.rs` | CLI integration | process, filesystem, serial/global | helpers: assert_success, maestro, stderr, stdout; maestro_bin=1; Command::new=1; cwd=1; TestTempDir=3; env |
| `tests/unknown_subcommand_integration.rs` | CLI integration | process | helpers: maestro; maestro_bin=1; Command::new=1 |
| `tests/update_integration.rs` | CLI integration | process, filesystem, serial/global | helpers: assert_success, maestro; maestro_bin=9; Command::new=11; cwd=11; TestTempDir=24; env |
| `tests/v1_demo.rs` | unit/domain | process, filesystem, serial/global, slow/flake-risk | helpers: assert_success, maestro_with_env, run_with_env, stdout; maestro_bin=2; Command::new=2; cwd=2; TestTempDir=3; env; async/io |
| `tests/worktree_ledger_integration.rs` | CLI integration, workflow/e2e | process, filesystem, serial/global | helpers: assert_failure, assert_success, git, maestro, maestro_with_env, stderr; maestro_bin=2; Command::new=4; cwd=4; TestTempDir=3; env |
| `tests/write_path_boundedness.rs` | unit/domain | filesystem | none |
