#!/usr/bin/env python3
"""Run the exact proof-engine and snapshot adversarial closure without timing output."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import sys
import unittest
from pathlib import Path


WORKSPACE = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(WORKSPACE))


EXPECTED_TESTS = (
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_child_phase_cannot_mutate_completed_dependency_output",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_command_cannot_reference_an_input_omitted_from_phase_identity",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_command_reads_immutable_input_snapshot_during_live_path_aba",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_completed_run_token_cannot_replay_but_new_seal_reexecutes",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_concurrent_same_identity_different_results_fail_closed",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_content_cache_hit_and_performance_data_is_non_canonical",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_corrupt_checkpoint_fails_closed",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_failure_leaves_existing_publication_pointer_untouched",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_independent_phases_run_in_parallel_with_disjoint_roots",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_input_mutation_is_rejected_before_checkpoint_seal",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_internal_symlink_tree_is_content_bound_and_copied_into_the_frozen_run",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_interrupted_run_cannot_redirect_publication_destination",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_interrupted_run_resumes_only_its_completed_checkpoints",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_old_post_pointer_crash_cannot_regress_a_newer_pointer",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_overlapping_publication_destinations_are_rejected",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_performance_log_cannot_overlap_run_or_publication_paths",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_phase_temp_is_isolated_and_excluded_from_sealed_output",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_post_pointer_crash_resumes_without_republishing",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_publication_rejects_output_changed_after_phase_completion",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_resealed_checkpoint_payload_is_rejected",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_resource_class_is_noncanonical_execution_policy",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_resource_limits_cap_compile_work_without_starving_light_phases",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_resource_limits_reject_invalid_capacity",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_run_root_placeholder_is_rejected_to_preserve_phase_isolation",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_same_plan_and_run_token_execute_exactly_once",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_source_command_and_literal_changes_each_invalidate_cache",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_substituted_checkpoint_binding_fails_closed",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_success_publishes_content_addressed_release_with_one_atomic_pointer",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_symlink_tree_rejects_a_link_that_escapes_its_bound_root",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_target_profile_and_mutant_are_independent_cache_key_inputs",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_tool_aba_substitution_cannot_change_the_executed_bytes",
    "tools.vnext_contracts.proof_engine.test_engine.ProofEngineTests.test_tool_byte_change_invalidates_cache",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_engine_local_binary_hashes_are_validated_before_semantic_consensus",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_frozen_behavior_manifest_rejects_a_real_passing_test_substitution",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_predecessor_rows_are_recomputed_instead_of_trusted",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_receipt_identity_rejects_a_self_consistent_payload_mutation",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_semantic_behavior_receipt_excludes_duration_only_diagnostics",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_semantic_consensus_excludes_only_engine_local_binary_hashes",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_snapshot_identity_is_recomputed_from_exact_tree_bytes",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_toolchain_identity_rejects_extra_file_and_duplicate_row",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus.Stage5ConsensusTests.test_toolchain_identity_rejects_file_mutation",
    "tools.vnext_contracts.stage5.evidence_gates.test_consensus_harness_contract.Stage5ConsensusHarnessContractTests.test_consensus_pins_the_exact_frozen_harness_manifest",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_driver_name_literal_is_identity_only_while_static_argument_executes",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_reconstruction_does_not_readmit_a_mutable_snapshot_object",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_ruby_verifier_executes_its_exact_test_output_parser",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_seven_phase_adapter_resumes_exact_topology_after_interruption",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_snapshot_bootstrap_executes_the_immutable_seal_copy",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_snapshot_cache_ignores_substituted_source_pointer_and_reconstructs_closure",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_snapshot_cache_reuses_a_frozen_content_bound_snapshot_without_revendoring",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_snapshot_copy_rejects_source_change",
    "tools.vnext_contracts.stage5.evidence_gates.test_seal.Stage5SnapshotTests.test_snapshot_source_rejects_symlinked_directory",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_exact_macos_developer_tools_freeze_and_execute_after_relocation",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_materialized_macos_developer_tools_execute_after_relocation",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_materializes_exact_minimal_compiler_closure",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_clang_resource_mutation_during_copy",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_clang_resource_tree_growth_during_copy",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_driver_substitution_during_materialization",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_invalid_driver_basename",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_linker_mutation_during_copy",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_linker_outside_exact_clang_toolchain",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_mutated_cached_clang_resource",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_nested_tool_aba_during_materialization",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_substituted_clang_resource_directory",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_symlinked_clang_resource_entry",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_rejects_symlinked_target_library_entry",
    "tools.vnext_contracts.stage5.evidence_gates.test_toolchain.Stage5ToolchainClosureTests.test_relocated_clang_links_concurrently_with_empty_path",
)
EXPECTED_TEST_MANIFEST_IDENTITY = (
    "sha256:c5d8562805f5b655447d32f1262d4fc06e91c7a80ce9ccdeab4eb0c77e1188a1"
)


def flatten(suite: unittest.TestSuite) -> list[unittest.TestCase]:
    tests: list[unittest.TestCase] = []
    for candidate in suite:
        if isinstance(candidate, unittest.TestSuite):
            tests.extend(flatten(candidate))
        else:
            tests.append(candidate)
    return tests


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()
    loader = unittest.defaultTestLoader
    suite = loader.loadTestsFromNames(
        [
            "tools.vnext_contracts.proof_engine.test_engine",
            "tools.vnext_contracts.stage5.evidence_gates.test_consensus",
            "tools.vnext_contracts.stage5.evidence_gates.test_consensus_harness_contract",
            "tools.vnext_contracts.stage5.evidence_gates.test_seal",
            "tools.vnext_contracts.stage5.evidence_gates.test_toolchain",
        ]
    )
    tests = flatten(suite)
    names = tuple(test.id() for test in tests)
    manifest_identity = f"sha256:{hashlib.sha256(canonical_json(list(names))).hexdigest()}"
    if names != EXPECTED_TESTS or manifest_identity != EXPECTED_TEST_MANIFEST_IDENTITY:
        raise RuntimeError("Stage 5 proof-harness test closure differs from its exact manifest")
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream, verbosity=0).run(suite)
    if not result.wasSuccessful() or result.testsRun != len(EXPECTED_TESTS):
        sys.stderr.write(stream.getvalue())
        return 1
    receipt = {
        "diagnostic_proof_claim": "test_adapter_only",
        "manifest_identity": EXPECTED_TEST_MANIFEST_IDENTITY,
        "passed": result.testsRun,
        "schema_version": "maestro.vnext.stage5.proof-harness-receipt.v1",
        "tests": list(EXPECTED_TESTS),
    }
    if set(receipt) != {
        "diagnostic_proof_claim",
        "manifest_identity",
        "passed",
        "schema_version",
        "tests",
    } or receipt["diagnostic_proof_claim"] != "test_adapter_only":
        raise RuntimeError("Stage 5 harness proof claim schema differs")
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "proof-harness-receipt.v1.json").write_bytes(canonical_json(receipt))
    print(json.dumps({"passed": result.testsRun}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
