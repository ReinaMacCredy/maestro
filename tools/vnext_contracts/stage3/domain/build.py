#!/usr/bin/env python3
"""Build the additive, inactive Stage 3 domain-kernel contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
OUTPUT = WORKSPACE / "contracts/vnext/stage3/domain"
STAGE0_ENCODER_RECEIPT = "contracts/vnext/stage0/effect-home/encoder-receipt.json"
STAGE0_FINALIZATION_RECEIPT = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
STAGE2_MANIFEST = "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json"
STAGE2_PROOF_RECEIPTS = [
    "contracts/vnext/stage2/authority/python-encoder-receipt.v1.json",
    "contracts/vnext/stage2/authority/semantic-validation-receipt.v1.json",
    "contracts/vnext/stage2/authority/ruby-verification-receipt.v1.json",
]
sys.dont_write_bytecode = True
sys.path.insert(0, str(WORKSPACE / "tools/vnext_contracts/catalogs"))
import cbor_py  # noqa: E402


DOMAIN = "maestro.vnext.stage3.domain-kernel.v1"
PUBLICATION_STATE = "inactive_candidate"
OWNERS = [
    ["Work", "work", ["identity", "revision", "lifecycle", "submission", "requirement", "relation"]],
    ["Contract", "contract", ["revision", "generation", "semantic-publication-request", "current-root"]],
    ["Step", "step", ["identity", "revision", "binding", "dag", "lifecycle", "submission", "amendment"]],
    ["Design", "design", ["source-binding", "revision", "slot-manifest", "reconciliation"]],
    ["Decision", "design", ["revision", "alternative", "resolution", "materialization", "lineage", "batch"]],
    [
        "Evidence",
        "evidence",
        ["claim", "observation-reference", "submission-reference", "claim-subject", "no-lifecycle"],
    ],
    [
        "Repository Store",
        "repository",
        [
            "publication-boundary",
            "authority-admission",
            "atomic-owner-joined-commit",
            "idempotent-replay",
            "stale-basis-refusal",
        ],
    ],
]
ACTION_CATALOG = [
    "maestro.vnext.stage3.repository-action-catalog-closure.v1",
    "7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970",
    "b7ef635dcd29af4fc41f20cd670b726e5627c2f7210344d058e7c188ace69647",
    1,
    [
        ["implemented", 1, "CreateDraftWork", "56ded201d62fbb94486581d13cc6a086b3e114ad889aa1a954841f7f646afc40", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 1],
        ["implemented", 2, "CancelWork", "b58d2fecb0f1b27146884f85847cb1b22575b32d8d6e92efe6608cf582420615", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 2],
        ["deferred_stage5", 3, "CompleteWork", "163de9814514910c9ca1d5b1f76ac982e0788bd8a81025eff5c551a0a923b5d2", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 3],
        ["catalogued_deferred_stage4_fail_closed", 4, "AbsorbWork", "4fb2d35f4bd7c2169bec6bc51af840f325c419c28de0041d60b69bc4691125ea", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 4],
        ["deferred_stage5", 5, "SubmitWorkCompletion", "7d8083d10f75348f805e89e8fcd5f27d81face12d2d8ae1047a01c53fbfb2803", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 5],
        ["deferred_stage5", 6, "RejectWorkCompletion", "d03d0a753eb0821b43f002de3eab1afb32a7ede75ff0a3588b0718292c0d7b3f", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 6],
        ["deferred_stage5", 7, "ReturnWorkForRepair", "2fbc3d51f0b750cb9a1292d404ada520c17fac6c9fd960350188d97f3b6acc0b", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 7],
        ["deferred_stage4", 8, "SubmitStep", "c5a7079af2dafa9acc956477b3004b5fb21dd688b4022677416164c4485f96d6", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 1],
        ["deferred_stage5", 9, "SatisfyStep", "a5887f9b87af5f6a5f466df222b3a76b2eca5762b33a5694b4ebcb61b3db127e", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 2],
        ["deferred_stage5", 10, "RejectStepSubmission", "b75460c72c4907e2893bb48559b2c19d99ecb4d27ab43a10adda4ee95dfbc62a", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 3],
        ["deferred_stage5", 11, "RecoverStepSubmission", "130cb3ecfd8146ba869de9a1198b3c3b6b67b2c06101cecf62a955db5b587e13", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 4],
        ["implemented", 12, "PublishInitialContract", "5c3bf7e45cc2e8348bb5a6ce403cf6d14f718c7ef4514ca01e60370174387234", 3, "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab", 1],
        ["implemented", 13, "AmendContract", "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4", 3, "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab", 2],
        ["implemented", 15, "AppendDesignRevision", "4235a07743d0fa3557f612b0d4dd499afcd02adadcc92facfcbc806645a99e83", 4, "85aad446bae62f47851f719f74296bd2576f30894b95ccb4b3b0c59790a80dc5", 2],
        ["implemented", 20, "ResolveDecision", "4e05d1c7d9314a843d43538c399ece7df7da52793062c7ca43805e8f763f75ac", 5, "a3d6c9c0dcd9b5e3447cf4dc45edf5d1b338c99dfc27a61df23966b7514ae9dc", 3],
        ["deferred_stage4", 23, "AcquireStepExecution", "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2", 6, "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292", 1],
    ],
]
STORE_SCHEMAS = [
    "maestro.vnext.stage3.repository-store-schema-closure.v1",
    [
        [1, "ActionRequest", "maestro.vnext.repository-action-request.v1", "c512faedbf87869f531be18baad9674652c5d83ab4ab76bc555330604e5791cd"],
        [2, "WorkRecord", "maestro.vnext.repository-work-record.v1", "03e178d89683e4c93528e1d6ed7c900d0da8957024e729b3661350b6c397cf37"],
        [3, "DesignStream", "maestro.vnext.repository-design-stream.v1", "8686ba5cc854ff7cb408e322817f173d6db93e99d492bea9bac3aa344b9f5537"],
        [4, "ContractRevision", "maestro.vnext.repository-contract-revision.v1", "221f327f2b29497599f943dd4c6fdea0d328916b7f2e7269c1cda6f81d4a12c0"],
        [5, "ContractGeneration", "maestro.vnext.repository-contract-generation.v1", "d5208061bd3aa95917b36ab2e55439365987286bc594e09ba3a327b3601a6cc7"],
        [6, "DesignFinalizationManifest", "maestro.vnext.repository-design-finalization-manifest.v1", "5ac8298b026b63f4548121ffcf054fa68c30314cb491e67f3150a3f929550226"],
        [7, "ContractRoot", "maestro.vnext.repository-contract-root.v1", "b6950c9de50712ae010468d1a1375ad2635967036972d07cdee0130286c42337"],
        [8, "Decision", "maestro.vnext.repository-decision.v1", "604471128e0c03afa1ac53f72f34c2287f1f975b98effbd2a78569e3490c7fb3"],
        [9, "StepGraph", "maestro.vnext.repository-step-graph.v1", "57bc1aca32b395c1fb43b6c960b38d5828aa71d3d63eac0f7cecf1887a1ed005"],
        [10, "StepState", "maestro.vnext.repository-step-state.v1", "90158ef1e261ea84260ad5fd98deb8461b0655d3216c4993d0d949df1cfc6596"],
        [11, "StepAmendmentAudit", "maestro.vnext.repository-step-amendment-audit.v1", "cf16f6af5205d093c58c63cc27fcd8c7314109ae2604e2baafe7473e74770bcf"],
        [12, "DecisionMaterializationAudit", "maestro.vnext.repository-decision-materialization-audit.v1", "19f3f998c8e408e621bc46aafb0716b4fb0d7b61b721b1c9d177addc4c9a88c4"],
        [13, "ExactEquivalenceReceipt", "maestro.vnext.repository-exact-equivalence-receipt.v1", "8108071f0749a887669d2537f9f7a95d7570e7588410a488e22c95dd73754fa8"],
        [14, "ComponentInvalidationReceipt", "maestro.vnext.repository-component-invalidation-receipt.v1", "ebc6630651d0379159b7369f70b4570440254418dae527043e64cf252128ad23"],
    ],
]
WORK_STATES = ["draft", "ready", "active", "awaiting_acceptance", "completed", "cancelled", "superseded"]
STEP_STATES = ["open", "submitted", "satisfied", "cancelled", "superseded"]
DECISION_STATES = ["open", "resolved", "withdrawn", "superseded"]
WORK_TRANSITIONS = [
    ["publish", "draft", "ready"],
    ["start", "ready", "active"],
    ["submit", "active", "awaiting_acceptance"],
    ["accept", "awaiting_acceptance", "completed"],
    ["reject", "awaiting_acceptance", "active"],
    ["repair", "awaiting_acceptance", "active"],
    ["amend", "awaiting_acceptance", "active"],
    ["cancel", "draft|ready|active|awaiting_acceptance", "cancelled"],
    ["supersede", "draft|ready|active|awaiting_acceptance", "superseded"],
]
STEP_TRANSITIONS = [
    ["submit", "open", "submitted"],
    ["satisfy", "submitted", "satisfied"],
    ["reject", "submitted", "open"],
    ["recover", "submitted", "open"],
    ["cancel", "open|submitted", "cancelled"],
    ["supersede", "open|submitted", "superseded"],
]
RELATIONS = [
    ["requirement", "before_execution|before_step|before_completion", "acyclic", "same_repository"],
    ["superseded_by", "lineage", "acyclic", "same_repository"],
    ["corrects", "lineage", "acyclic", "same_repository"],
    ["continues", "lineage", "acyclic", "same_repository"],
    ["reference", "informational", "cycles_allowed", "cross_repository_allowed"],
]
AMENDMENTS = [
    ["retain_exact", "open_fresh_stage3", "satisfaction_carry_requires_stage5_canonical_evidence_gate_material", "no_lease_attempt_run_transfer"],
    ["replace", "successor_binding_required", "old_cancelled_or_superseded"],
    ["remove", "old_cancelled_or_superseded", "obligations_conserved"],
    ["add", "new_open_binding", "complete_required_dag"],
]
INVARIANTS = [
    "one_owner_per_concept",
    "all_mutations_require_typed_authorized_action_request",
    "current_generation_and_root_exact",
    "semantic_no_op_detected_before_authority",
    "terminal_work_rejects_design_and_decision_writes",
    "claim_binds_exactly_one_submission",
    "claim_subject_matches_full_submission_subject",
    "work_claim_subject_matches_exact_step_submission_closure",
    "submission_claim_cardinality_1_to_n_without_second_count_cap",
    "nonauthoritative_claim_carrier_refused",
    "step_binding_generation_scoped",
    "step_binding_commits_contract_generation",
    "contract_generation_identity_excludes_runtime_authority_and_is_predictable",
    "dag_complete_finite_acyclic",
    "decision_resolution_has_no_direct_contract_effect",
    "candidate_root_derived_only_from_typed_consequence_plan",
    "equal_root_detected_before_authority_and_requires_none",
    "exactly_equivalent_distinct_root_validated_before_authority_and_writes_nothing",
    "materialization_candidate_only_and_joined_only_by_contract_publication",
    "no_standalone_materialize_decision_action",
    "repository_store_is_only_publication_boundary",
    "closed_owner_handlers_have_no_generic_lifecycle_bypass",
    "repository_actions_use_exact_nominal_authority_leaves",
    "deferred_execution_evidence_and_gate_publication_surfaces_absent",
    "ordinary_grant_has_canonical_parent_delegation_reachability",
    "same_store_atomic_owner_joined_publication",
    "initial_contract_publication_roots_complete_step_dag_and_open_fresh_states",
    "contract_amendment_consumes_total_step_plan_and_publishes_all_dispositions",
    "stage3_satisfaction_carry_unavailable_until_canonical_evidence_gate_material",
    "authority_is_store_loaded_and_action_is_admitted_before_commit",
    "replay_returns_original_committed_result",
    "stale_store_basis_refused_before_commit",
    "failed_replayed_or_stale_publication_leaves_no_orphan_objects",
]
AUTHORITY_SUCCESSOR = [
    "maestro.vnext.stage3.authority-successor.v1",
    "additive_over_frozen_stage2_and_public_predecessors",
    [
        [23, "OrdinaryBoundedGrantV1", ["complete_grant_definition", "exact_parent_and_delegation", "bounded_capacity_root", "immutable"]],
        [24, "OrdinaryGrantDelegationV1", ["exact_parent_child", "same_context", "same_capacity_root", "immutable"]],
    ],
    [
        ["AllocateGovernedCapacitySlot", "BootstrapControlG0"],
        ["EstablishConsumptionCellRoot", "BootstrapControlG0"],
        ["IssueRootAttachedBoundedGrant", "BootstrapControlG0"],
        ["ReissueRootAttachedGrantOneToOne", "OrdinaryLiveRuntime"],
        ["RevokeGrant", "OrdinaryLiveRuntime"],
    ],
    [
        "parentless_bounded_grant_refused",
        "unknown_or_cma_grant_action_basis_refused",
        "candidate_or_target_self_authorization_refused",
        "g0_issue_has_no_ordinary_capacity_debit",
        "reissue_and_revoke_require_separate_live_admin_grant",
        "reissue_and_revoke_spend_exactly_one_admin_capacity_unit",
    ],
]
SOURCE_PATHS = [
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
    "contracts/vnext/public/setup_operation_compatibility.v1.json",
    "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json",
    "src/lib.rs",
    "src/domain/mod.rs",
    "src/domain/authority/action_basis.rs",
    "src/domain/authority/bootstrap_catalog.rs",
    "src/domain/authority/capacity.rs",
    "src/domain/authority/closed.rs",
    "src/domain/authority/context.rs",
    "src/domain/authority/continuity.rs",
    "src/domain/authority/continuity/allocation.rs",
    "src/domain/authority/continuity/catalog.rs",
    "src/domain/authority/continuity/closure.rs",
    "src/domain/authority/continuity/state.rs",
    "src/domain/authority/continuity/totality.rs",
    "src/domain/authority/continuity/trusted_time.rs",
    "src/domain/authority/downstream_action_basis.rs",
    "src/domain/authority/evaluator.rs",
    "src/domain/authority/facade.rs",
    "src/domain/authority/facade/repository_admission.rs",
    "src/domain/authority/facade/repository_leaf_authority.rs",
    "src/domain/authority/facade_tests.rs",
    "src/domain/authority/grant.rs",
    "src/domain/authority/identity.rs",
    "src/domain/authority/mandate.rs",
    "src/domain/authority/mod.rs",
    "src/domain/authority/post_cut.rs",
    "src/domain/authority/principal.rs",
    "src/domain/authority/protected_diagnostic_envelope.rs",
    "src/domain/authority/protected_diagnostic_envelope_stage8_seed.rs",
    "src/domain/authority/publication.rs",
    "src/domain/authority/result.rs",
    "src/domain/authority/transition.rs",
    "src/domain/contract/assembly.rs",
    "src/domain/contract/component.rs",
    "src/domain/contract/component_kind.rs",
    "src/domain/contract/decision_closure.rs",
    "src/domain/contract/finalization.rs",
    "src/domain/contract/handoff.rs",
    "src/domain/contract/materialization.rs",
    "src/domain/contract/mod.rs",
    "src/domain/contract/proof.rs",
    "src/domain/contract/provenance.rs",
    "src/domain/contract/root.rs",
    "src/domain/contract/runtime.rs",
    "src/domain/evidence/submission_claim.rs",
    "src/domain/design/batch.rs",
    "src/domain/design/closure.rs",
    "src/domain/design/common.rs",
    "src/domain/design/decision.rs",
    "src/domain/design/materialization.rs",
    "src/domain/design/mod.rs",
    "src/domain/design/revision.rs",
    "src/domain/evidence/assessment.rs",
    "src/domain/evidence/claim.rs",
    "src/domain/evidence/diagnostics/mod.rs",
    "src/domain/evidence/erasure.rs",
    "src/domain/evidence/identity.rs",
    "src/domain/evidence/mod.rs",
    "src/domain/evidence/observation.rs",
    "src/domain/evidence/store.rs",
    "src/domain/identity/digest.rs",
    "src/domain/identity/manifest.rs",
    "src/domain/identity/mod.rs",
    "src/domain/identity/schema.rs",
    "src/domain/mod.rs",
    "src/domain/persistence/export.rs",
    "src/domain/persistence/generation.rs",
    "src/domain/persistence/idempotency.rs",
    "src/domain/persistence/metadata.rs",
    "src/domain/persistence/mod.rs",
    "src/domain/persistence/object.rs",
    "src/domain/persistence/protected_diagnostic.rs",
    "src/domain/persistence/protected_diagnostic_stage9_seed.rs",
    "src/domain/persistence/retention.rs",
    "src/domain/persistence/snapshot.rs",
    "src/domain/persistence/snapshot_blocks.rs",
    "src/domain/persistence/snapshot_export.rs",
    "src/domain/persistence/snapshot_restore.rs",
    "src/domain/persistence/snapshot_rows.rs",
    "src/domain/persistence/store.rs",
    "src/domain/persistence/tests/atomic_publication.rs",
    "src/domain/persistence/tests/canonical_store.rs",
    "src/domain/persistence/tests/mod.rs",
    "src/domain/persistence/tests/store_full_export.rs",
    "src/domain/persistence/tests/store_safety.rs",
    "src/domain/persistence/types.rs",
    "src/domain/repository/mod.rs",
    "src/domain/repository/tests.rs",
    "src/domain/step/amendment.rs",
    "src/domain/step/graph.rs",
    "src/domain/step/identity.rs",
    "src/domain/step/lifecycle.rs",
    "src/domain/step/mod.rs",
    "src/domain/step/revision.rs",
    "src/domain/step/submission.rs",
    "src/domain/work/identity.rs",
    "src/domain/work/lifecycle.rs",
    "src/domain/work/mod.rs",
    "src/domain/work/relation.rs",
    "src/domain/work/submission.rs",
    "src/foundation/mod.rs",
    "src/foundation/core/mod.rs",
    "src/foundation/core/deterministic_cbor.rs",
    "src/foundation/core/secure_fs.rs",
    "tests/vnext_work_identity.rs",
    "tests/vnext_work_lifecycle.rs",
    "tests/vnext_work_relations.rs",
    "tests/vnext_step_graph.rs",
    "tests/vnext_step_amendment.rs",
    "tests/vnext_step_amendment_application.rs",
    "tests/vnext_contract_step_publication.rs",
    "tests/vnext_design_revisions.rs",
    "tests/vnext_decision_kernel.rs",
    "tests/vnext_decision_closure.rs",
    "tests/vnext_decision_materialization_plan.rs",
    "tests/vnext_evidence_claims.rs",
    "tests/vnext_submission_claim_set.rs",
    "tests/vnext_stage3_contracts.rs",
    "tools/vnext_contracts/public/build_public_literals.py",
    "tools/vnext_contracts/catalogs/cbor_py.py",
    "tools/vnext_contracts/stage2/authority/build.py",
    "tools/vnext_contracts/stage2/authority/validate.py",
    "tools/vnext_contracts/stage2/authority/verify.rb",
    "tools/vnext_contracts/stage3/domain/build.py",
    "tools/vnext_contracts/stage3/domain/validate.py",
    "tools/vnext_contracts/stage3/domain/verify.rb",
]
RUST_SOURCE_ROOTS = [
    "src/domain/authority",
    "src/domain/contract",
    "src/domain/design",
    "src/domain/evidence",
    "src/domain/identity",
    "src/domain/persistence",
    "src/domain/repository",
    "src/domain/step",
    "src/domain/work",
]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_object(relative: str) -> dict[str, object]:
    value = json.loads((WORKSPACE / relative).read_text(encoding="ascii"))
    if not isinstance(value, dict):
        raise ValueError(f"predecessor artifact must contain one object: {relative}")
    return value


def proof_receipt_rows(paths: list[str]) -> list[dict[str, object]]:
    rows = []
    for relative in paths:
        data = (WORKSPACE / relative).read_bytes()
        rows.append({"byte_length": len(data), "path": relative, "sha256": digest(data)})
    return rows


def predecessor_chain_binding() -> dict[str, object]:
    stage0_encoder = load_object(STAGE0_ENCODER_RECEIPT)
    stage0_finalization = load_object(STAGE0_FINALIZATION_RECEIPT)
    if (
        stage0_encoder.get("schema_version")
        != "maestro.vnext.stage0.effect-home-encoder-receipt.v1"
        or stage0_finalization.get("schema_version")
        != "maestro.vnext.stage0.effect-home-finalization-receipt.v1"
        or stage0_finalization.get("finalization_state") != "final"
        or stage0_finalization.get("candidate_only") is not True
        or stage0_finalization.get("runtime_activation") is not False
    ):
        raise ValueError("Stage 0 proof receipt is not a final inactive-candidate certification")
    stage0_encoder_sha256 = digest((WORKSPACE / STAGE0_ENCODER_RECEIPT).read_bytes())
    if stage0_finalization.get("encoder_receipt_sha256") != stage0_encoder_sha256:
        raise ValueError("Stage 0 finalization receipt does not bind the exact encoder receipt")
    stage0_semantic_root = stage0_finalization.get("identity")
    if not isinstance(stage0_semantic_root, str) or not stage0_semantic_root.startswith("sha256:"):
        raise ValueError("Stage 0 finalization receipt has no semantic root")

    stage2_manifest = load_object(STAGE2_MANIFEST)
    stage2_root_id = stage2_manifest.get("root_id")
    stage0_tree_sha256 = stage2_manifest.get("stage0_tree_sha256")
    if (
        stage2_manifest.get("schema_version")
        != "maestro.vnext.stage2.authority.root-manifest.v1"
        or not isinstance(stage2_root_id, str)
        or len(stage2_root_id) != 64
        or not isinstance(stage0_tree_sha256, str)
        or len(stage0_tree_sha256) != 64
    ):
        raise ValueError("Stage 2 manifest has no exact Stage 0 tree and Stage 2 semantic roots")
    for relative in STAGE2_PROOF_RECEIPTS:
        receipt = load_object(relative)
        if receipt.get("root_id") != stage2_root_id:
            raise ValueError(f"Stage 2 proof receipt does not bind the exact root: {relative}")

    return {
        "mode": "full_chain",
        "stage0": {
            "proof_receipts": proof_receipt_rows(
                [STAGE0_ENCODER_RECEIPT, STAGE0_FINALIZATION_RECEIPT]
            ),
            "semantic_root": stage0_semantic_root,
            "source_tree_root": f"sha256:{stage0_tree_sha256}",
        },
        "stage2": {
            "proof_receipts": proof_receipt_rows(STAGE2_PROOF_RECEIPTS),
            "semantic_root": f"sha256:{stage2_root_id}",
        },
    }


def source_rows() -> list[list[object]]:
    rows: list[list[object]] = []
    for relative in SOURCE_PATHS:
        path = WORKSPACE / relative
        if not path.is_file():
            raise SystemExit(f"missing Stage 3 semantic source: {relative}")
        data = path.read_bytes()
        rows.append([relative, len(data), digest(data)])
    return rows


def validate_source_closure() -> None:
    actual = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for relative in RUST_SOURCE_ROOTS
        for path in (WORKSPACE / relative).rglob("*.rs")
    )
    declared = sorted(
        relative
        for relative in SOURCE_PATHS
        if relative.endswith(".rs")
        and any(relative.startswith(f"{root}/") for root in RUST_SOURCE_ROOTS)
    )
    if declared != actual:
        missing = sorted(set(actual) - set(declared))
        unexpected = sorted(set(declared) - set(actual))
        raise ValueError(
            f"Stage 3 transitive Rust source closure drifted: missing={missing}, unexpected={unexpected}"
        )
    for relative in [
        "src/lib.rs",
        "src/domain/mod.rs",
        "src/domain/mod.rs",
        "src/foundation/mod.rs",
        "src/foundation/core/mod.rs",
        "src/foundation/core/deterministic_cbor.rs",
        "src/foundation/core/secure_fs.rs",
    ]:
        if relative not in SOURCE_PATHS:
            raise ValueError(f"Stage 3 transitive semantic dependency is undeclared: {relative}")


def canonical_value() -> list[object]:
    validate_predecessor_chain()
    validate_action_catalog()
    validate_store_schemas()
    validate_source_closure()
    return [
        DOMAIN,
        1,
        PUBLICATION_STATE,
        OWNERS,
        [WORK_STATES, STEP_STATES, DECISION_STATES],
        [WORK_TRANSITIONS, STEP_TRANSITIONS],
        RELATIONS,
        AMENDMENTS,
        INVARIANTS,
        ACTION_CATALOG,
        STORE_SCHEMAS,
        source_rows(),
        AUTHORITY_SUCCESSOR,
    ]


def validate_predecessor_chain() -> None:
    commands = [
        [sys.executable, "tools/vnext_contracts/stage0/effect_home/build.py", "--check"],
        [sys.executable, "tools/vnext_contracts/stage0/effect_home/validate.py"],
        [sys.executable, "tools/vnext_contracts/stage2/authority/build.py", "--check"],
        [sys.executable, "tools/vnext_contracts/stage2/authority/validate.py"],
        ["ruby", "tools/vnext_contracts/stage2/authority/verify.rb"],
    ]
    for command in commands:
        result = subprocess.run(
            command,
            cwd=WORKSPACE,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip()
            raise ValueError(f"Stage 3 predecessor validation failed: {' '.join(command)}: {detail}")


def validate_action_catalog() -> None:
    setup = json.loads((WORKSPACE / "contracts/vnext/public/setup_operation_compatibility.v1.json").read_text())
    generated_path = WORKSPACE / "contracts/vnext/catalogs/generated/catalog-09-action-spec.json"
    generated_bytes = generated_path.read_bytes()
    generated = json.loads(generated_bytes)
    bindings = setup["catalog_bindings"]
    if (
        generated["manifest_id"] != ACTION_CATALOG[1]
        or generated["grammar_id"] != ACTION_CATALOG[2]
        or bindings["action_spec_manifest_id"] != ACTION_CATALOG[1]
        or bindings["catalog_profile_grammar_id"] != ACTION_CATALOG[2]
        or bindings["action_spec_file_sha256"] != digest(generated_bytes)
    ):
        raise ValueError("the frozen ActionSpec manifest or grammar identity drifted")
    manifest_envelope = generated["manifest_identity_envelope"]
    manifest_bytes = cbor_py.encode(manifest_envelope)
    if (
        generated["cbor_hex"] != manifest_bytes.hex()
        or generated["byte_length"] != len(manifest_bytes)
        or digest(manifest_bytes) != ACTION_CATALOG[1]
        or manifest_envelope[3] != generated["manifest_header"]
        or manifest_envelope[4] != generated["manifest_rows"]
        or generated["manifest_header"][1] != ACTION_CATALOG[3]
        or generated["manifest_header"][3]["bytes"] != ACTION_CATALOG[2]
    ):
        raise ValueError("the frozen ActionSpec manifest envelope is not self-authenticating")
    setup_by_tag = {row["catalog_tag"]: row for row in setup["action_rows"]}
    generated_by_tag = {row["value"][0]: row for row in generated["descriptors"]}
    manifest_by_tag = {row[0]: row for row in generated["manifest_rows"]}
    for _, tag, name, descriptor_id, owner_tag, owner_id, local_tag in ACTION_CATALOG[4]:
        setup_row = setup_by_tag.get(tag)
        generated_row = generated_by_tag.get(tag)
        manifest_row = manifest_by_tag.get(tag)
        if setup_row is None or generated_row is None or manifest_row is None:
            raise ValueError(f"missing frozen ActionSpec row {tag}:{name}")
        expected_setup = [name, descriptor_id, owner_tag, owner_id, local_tag]
        actual_setup = [
            setup_row["name"], setup_row["descriptor_id"], setup_row["primary_owner_tag"],
            setup_row["primary_owner_descriptor_id"], setup_row["family_local_tag"],
        ]
        value = generated_row["value"]
        actual_generated = [value[1], generated_row["descriptor_id"], value[2][0], value[2][1]["bytes"], value[4]]
        descriptor_envelope = generated_row["identity_envelope"]
        descriptor_bytes = cbor_py.encode(descriptor_envelope)
        if (
            actual_setup != expected_setup
            or actual_generated != expected_setup
            or value[3] != setup_row["family_tag"]
            or descriptor_envelope[2] != value
            or generated_row["cbor_hex"] != descriptor_bytes.hex()
            or generated_row["byte_length"] != len(descriptor_bytes)
            or digest(descriptor_bytes) != descriptor_id
            or manifest_row != [tag, {"bytes": descriptor_id}, value]
        ):
            raise ValueError(f"frozen ActionSpec row drifted for {tag}:{name}")


def validate_store_schemas() -> None:
    for expected_ordinal, (ordinal, _, domain, schema_id) in enumerate(STORE_SCHEMAS[1], start=1):
        expected = digest(cbor_py.encode(["maestro.vnext.repository-runtime-schema.v1", domain]))
        if ordinal != expected_ordinal or schema_id != expected:
            raise ValueError(f"Repository Store schema identity drifted for {domain}")


def write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode("ascii")


def build_to(root: Path) -> str:
    value = canonical_value()
    encoded = cbor_py.encode(value)
    identity = f"sha256:{digest(encoded)}"
    manifest = {
        "canonical_value": value,
        "identity": identity,
        "publication_state": PUBLICATION_STATE,
        "schema_version": DOMAIN,
    }
    manifest_bytes = json_bytes(manifest)
    write_atomic(root / "domain-kernel.v1.cbor", encoded)
    write_atomic(root / "domain-kernel.v1.json", manifest_bytes)
    receipt = {
        "artifacts": [
            ["domain-kernel.v1.cbor", len(encoded), digest(encoded)],
            ["domain-kernel.v1.json", len(manifest_bytes), digest(manifest_bytes)],
        ],
        "encoder": "python-stdlib-plus-frozen-cbor-subset",
        "identity": identity,
        "predecessor_chain": predecessor_chain_binding(),
        "schema_version": "maestro.vnext.stage3.domain-kernel.encoder-receipt.v1",
        "validation_mode": "full_chain",
    }
    write_atomic(root / "python-encoder-receipt.v1.json", json_bytes(receipt))
    return identity


def certify(root: Path) -> None:
    commands = [
        [sys.executable, str(TOOLS / "validate.py"), "--root", str(root)],
        ["ruby", str(TOOLS / "verify.rb"), "--root", str(root)],
    ]
    for command in commands:
        result = subprocess.run(
            command,
            cwd=WORKSPACE,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip()
            raise ValueError(f"Stage 3 certification failed: {' '.join(command)}: {detail}")


def compare_trees(expected: Path, actual: Path) -> None:
    expected_paths = sorted(
        path.relative_to(expected).as_posix() for path in expected.rglob("*") if path.is_file()
    )
    actual_paths = sorted(
        path.relative_to(actual).as_posix() for path in actual.rglob("*") if path.is_file()
    )
    if expected_paths != actual_paths:
        raise ValueError("Stage 3 domain artifact file set is stale")
    for relative in expected_paths:
        if (expected / relative).read_bytes() != (actual / relative).read_bytes():
            raise ValueError(f"Stage 3 domain artifact is stale: {relative}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog-only", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--root", type=Path, default=OUTPUT)
    args = parser.parse_args()
    if args.catalog_only:
        validate_action_catalog()
        print("Stage 3 frozen ActionSpec catalog validated")
        return 0
    if args.check:
        with tempfile.TemporaryDirectory(prefix="maestro-stage3-domain-") as temporary:
            generated = Path(temporary) / "domain"
            identity = build_to(generated)
            certify(generated)
            compare_trees(generated, args.root)
    else:
        identity = build_to(args.root)
        certify(args.root)
    print(identity)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
