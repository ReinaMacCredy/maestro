#!/usr/bin/env python3
"""Require exact Stage 5 predecessor and three-engine agreement before publication."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import stat
import tarfile
from pathlib import Path
from typing import Any


EXPECTED_STAGE4_IDENTITY = "sha256:462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077"
EXPECTED_STAGE4_SOURCE_COMMIT = "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2"
EXPECTED_STAGE4_SOURCE_TREE = "2f832a04c7109e17b4b298e40b4827c1ced2d527"
EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH = 16_486_231
EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256 = (
    "347eaf928f81d9ce6e07e3767f0cdaf2cde23cd98d13bad41b745d5fbc359910"
)
EXPECTED_BEHAVIOR_TESTS = 86
EXPECTED_PROOF_HARNESS_TESTS = 66
EXPECTED_BEHAVIOR_MANIFEST_IDENTITY = (
    "sha256:fe5df73a47fb802b0ef87afafab04267c0b8a540931c8a6e667749f3a60131a5"
)
EXPECTED_OBSERVATION_CONTRACT_TABLE_IDENTITY = (
    "sha256:a5f0e9137c091972802cb7084d86070a930091f0570cefcc7df445074478a676"
)
EXPECTED_PROOF_HARNESS_MANIFEST_IDENTITY = (
    "sha256:c5d8562805f5b655447d32f1262d4fc06e91c7a80ce9ccdeab4eb0c77e1188a1"
)
ENGINE_RECEIPT_CONTRACTS = {
    "builder": (
        "maestro.vnext.stage5.python-builder-receipt.v1",
        "builder_sha256",
        "tools/vnext_contracts/stage5/evidence_gates/build.py",
    ),
    "validator": (
        "maestro.vnext.stage5.semantic-validation-receipt.v1",
        "validator_sha256",
        "tools/vnext_contracts/stage5/evidence_gates/validate.py",
    ),
    "ruby": (
        "maestro.vnext.stage5.ruby-verification-receipt.v1",
        "verifier_sha256",
        "tools/vnext_contracts/stage5/evidence_gates/verify.rb",
    ),
}
DIAGNOSTIC_PROOF_CLAIM = "test_adapter_only"
DOMAIN = "maestro.vnext.stage5.evidence-gates.v1"
ARTIFACT_KEYS = {
    "artifact_id",
    "behavior",
    "behavior_manifest_identity",
    "byte_length",
    "cbor_hex",
    "domain",
    "diagnostic_proof_claim",
    "invalidation_reasons",
    "invariants",
    "observation_catalog_manifest_id",
    "observation_contract_table_identity",
    "observation_kinds",
    "predecessors",
    "protocol",
    "publication_state",
    "schema_version",
    "source_closure",
    "stage",
}
EXPECTED_NORMAL_RUNS = (
    ("assessment-kernel", "maestro", 15),
    ("submission-evidence-join", "maestro", 5),
    ("authorized-evidence-store", "maestro", 47),
    ("work-completion-boundary", "vnext_work_lifecycle", 1),
    ("claim-contracts", "vnext_evidence_claims", 5),
    ("submission-claim-carrier", "vnext_submission_claim_set", 4),
    ("evidence-gate-contracts", "vnext_stage5_evidence_gates", 8),
    ("diagnostic-architecture", "architecture_imports", 1),
)
RESULTS = [[1, "Pass"], [2, "Fail"], [3, "Indeterminate"], [4, "Error"]]
INPUT_CLASSES = [[1, "Evidence"], [2, "Authority"], [3, "Mixed"], [4, "Composite"]]
OPERATORS = [
    [1, "Leaf"],
    [2, "All"],
    [3, "Any"],
    [4, "Quorum"],
    [5, "Veto"],
    [6, "DenyOverrides"],
]
ACQUISITION_MODES = [
    [1, "EffectFree", "zero_run"],
    [2, "RunMediated", "exact_execution_attempt_owner"],
    [3, "DeclaredDerivation", "source_observation_closure"],
]
INVALIDATION_REASONS = [
    [1, "WorkGenerationAdvanced"],
    [2, "StepRevisionAdvanced"],
    [3, "GateSnapshotChanged"],
    [4, "EvaluatorChanged"],
    [5, "InputTombstoned"],
    [6, "InputCorrected"],
    [7, "FreshnessExpired"],
    [8, "IntegrityFailure"],
    [9, "AuthorizationReceiptRevoked"],
]
INVARIANTS = [
    "observation_kind_exact_43_dense_closed",
    "observation_catalog_binds_producer_action_routes_and_cma",
    "observation_payload_schemas_are_exact_typed_and_kind_specific",
    "observation_scope_binds_exact_work_step_submission_and_generation",
    "observation_secret_scan_redaction_and_retention_are_typed_and_authenticated",
    "secret_scan_is_deterministically_recomputed_from_exact_payload_bytes",
    "observation_is_immutable_non_bearer",
    "observation_publication_requires_typed_action_authority_and_atomic_store_index",
    "stored_evidence_records_require_canonical_identity_consistent_decoding",
    "payload_identity_distinct_from_observation_identity",
    "effect_free_acquisition_has_zero_run",
    "effecting_acquisition_binds_exact_run_and_attempt_owner",
    "acquisition_identity_is_unique_per_store",
    "declared_derivation_equals_lineage",
    "claim_binds_exactly_one_submission",
    "claim_publication_resolves_exact_observation_records",
    "submission_claim_set_has_exact_three_field_carrier",
    "assessment_evaluates_exactly_one_gate_node",
    "assessment_scope_store_generation_and_evidence_cut_are_exact",
    "assessment_support_binds_pairwise_independent_contributors_and_sources",
    "assessment_uses_trusted_time_freshness_and_pinned_trust_root",
    "empirical_authority_and_composite_inputs_are_nominally_distinct",
    "gate_snapshot_is_complete_content_addressed_and_acyclic",
    "gate_snapshot_has_no_detached_nodes",
    "gate_leaf_cannot_accept_a_proposed_result",
    "gate_composite_evaluation_is_pure_and_pinned",
    "closed_semantic_leaf_evaluators_produce_pass_or_fail_from_exact_inputs",
    "only_pass_derives_satisfaction",
    "fail_indeterminate_and_error_block",
    "equally_applicable_conflict_is_indeterminate",
    "applicability_has_no_newest_selector",
    "invalidation_requires_typed_authority_and_exact_evidence_cut",
    "assessment_and_invalidation_publication_require_complete_store_derived_cut",
    "security_erasure_derives_complete_narrow_invalidation_closure",
    "security_erasure_transitively_invalidates_composite_dependents",
    "security_erasure_publishes_in_doubt_intent_before_physical_absence",
    "security_erasure_receipt_requires_verified_physical_absence_and_exact_resume",
    "security_erasure_revokes_every_secret_bearing_sealed_export_under_one_durable_barrier",
    "security_erasure_restores_exact_insert_only_schema_before_publication_commit",
    "security_erasure_finalization_survives_authority_head_advance",
    "physical_erasure_never_resolves_while_hard_link_or_crash_debt_remains",
    "atomic_publication_builders_reduce_supersets_to_the_exact_generation_closure",
    "raw_atomic_publication_rejects_every_object_outside_the_exact_generation_closure",
    "idempotency_results_remain_durable_replay_horizons",
    "work_completion_atomically_commits_current_claim_gate_and_submission_evidence",
    "work_completion_requires_repository_derived_current_satisfied_submission_closure",
    "persisted_invalidation_rejoins_exact_authorized_action_and_effect_intent",
    "stage3_claim_and_work_submission_v1_bytes_remain_exact",
    "scheduling_and_admission_assessments_are_outside_evidence",
]
WORKSPACE = Path(__file__).resolve().parents[4]
PREDECESSOR_PATHS = (
    "contracts/vnext/stage4/execution/execution-effects.v1.json",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json",
)
ARTIFACT_SOURCE_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "contracts/vnext/catalogs/generated/catalog-01-observation.json",
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
    "src/lib.rs",
    "src/domain/mod.rs",
    "src/domain/vnext/mod.rs",
    "src/domain/vnext/authority/action_basis.rs",
    "src/domain/vnext/authority/downstream_action_basis.rs",
    "src/domain/vnext/authority/facade.rs",
    "src/domain/vnext/authority/facade_tests.rs",
    "src/domain/vnext/authority/facade/repository_admission.rs",
    "src/domain/vnext/authority/facade/repository_leaf_authority.rs",
    "src/domain/vnext/authority/materialization.rs",
    "src/domain/vnext/authority/mod.rs",
    "src/domain/vnext/authority/protected_diagnostic_envelope.rs",
    "src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs",
    "src/domain/vnext/authority/result.rs",
    "src/domain/vnext/contract/runtime.rs",
    "src/domain/vnext/evidence/assessment.rs",
    "src/domain/vnext/evidence/claim.rs",
    "src/domain/vnext/evidence/erasure.rs",
    "src/domain/vnext/evidence/identity.rs",
    "src/domain/vnext/evidence/mod.rs",
    "src/domain/vnext/evidence/observation.rs",
    "src/domain/vnext/evidence/submission_claim.rs",
    "src/domain/vnext/evidence/store.rs",
    "src/domain/vnext/execution/h3_withdrawal_publication.rs",
    "src/domain/vnext/execution/mod.rs",
    "src/domain/vnext/execution/store.rs",
    "src/domain/vnext/execution/runtime.rs",
    "src/domain/vnext/gate/mod.rs",
    "src/domain/vnext/installation/consumer_snapshot.rs",
    "src/domain/vnext/installation/mod.rs",
    "src/domain/vnext/integration/consumer_closure.rs",
    "src/domain/vnext/integration/mod.rs",
    "src/domain/vnext/integration/trusted_host_diagnostic.rs",
    "src/domain/vnext/integration/trusted_host_diagnostic_stage10_seed.rs",
    "src/domain/vnext/persistence/mod.rs",
    "src/domain/vnext/persistence/consumer_snapshot.rs",
    "src/domain/vnext/persistence/idempotency.rs",
    "src/domain/vnext/persistence/metadata.rs",
    "src/domain/vnext/persistence/store.rs",
    "src/domain/vnext/persistence/protected_diagnostic.rs",
    "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs",
    "src/domain/vnext/persistence/tests/atomic_publication.rs",
    "src/domain/vnext/repository/mod.rs",
    "src/domain/vnext/repository/tests.rs",
    "src/domain/vnext/work/lifecycle.rs",
    "src/domain/vnext/work/mod.rs",
    "src/domain/vnext/work/submission.rs",
    "src/foundation/core/secure_fs.rs",
    "tests/vnext_evidence_claims.rs",
    "tests/vnext_submission_claim_set.rs",
    "tests/vnext_stage5_contracts.rs",
    "tests/vnext_stage5_evidence_gates.rs",
    "tests/architecture_imports.rs",
    "tests/vnext_work_lifecycle.rs",
    "tools/vnext_contracts/catalogs/cbor_py.py",
    "tools/vnext_contracts/proof_engine/__init__.py",
    "tools/vnext_contracts/proof_engine/README.md",
    "tools/vnext_contracts/proof_engine/engine.py",
    "tools/vnext_contracts/proof_engine/test_engine.py",
    "tools/vnext_contracts/stage5/evidence_gates/behavior.py",
    "tools/vnext_contracts/stage5/evidence_gates/build.py",
    "tools/vnext_contracts/stage5/evidence_gates/consensus.py",
    "tools/vnext_contracts/stage5/evidence_gates/harness.py",
    "tools/vnext_contracts/stage5/evidence_gates/predecessor.py",
    "tools/vnext_contracts/stage5/evidence_gates/validate.py",
    "tools/vnext_contracts/stage5/evidence_gates/verify.rb",
    "tools/vnext_contracts/stage5/evidence_gates/seal.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_consensus.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_consensus_harness_contract.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_seal.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_toolchain.py",
    "tools/vnext_contracts/stage5/evidence_gates/toolchain.py",
)
EXPECTED_PREDECESSOR_SHA256 = {
    "contracts/vnext/stage4/execution/execution-effects.v1.json": "18b215280ea9aeab3a7bb6edf15214950d35343e6d15be89fef54031c9a51e3b",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor": "462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json": "ead17b652be513d2bbb6cf8460676c38609ffaec9bee9ac1818d83be454cb3ac",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json": "c806b4fe97ecb9374adf1ae7401fb86081230644a444ca4a77ff37c881e04f51",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json": "5fd6437350350691ee7b623fb3a0b8750b43b16fd3a7719cd9d7e8713d3756c4",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json": "e9a9e882decfc91a23ae5d2a47fef5b976b42583ae1b2b565ce7e2f2fab9103b",
}
SNAPSHOT_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src",
    "embedded",
    "tests",
    "tools/vnext_contracts",
    "contracts/vnext/catalogs",
    "contracts/vnext/stage0",
    "contracts/vnext/stage2",
    "contracts/vnext/stage3",
    "contracts/vnext/stage4/execution",
    "predecessors/stage4-source.tar.gz",
)


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def pretty_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode("ascii")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path) -> tuple[dict[str, Any], bytes]:
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"consensus input is absent or unsafe: {path}")
    data = path.read_bytes()
    value = json.loads(data)
    if not isinstance(value, dict):
        raise RuntimeError(f"consensus input is not an object: {path}")
    return value, data


def read_regular(path: Path) -> tuple[bytes, bool]:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise RuntimeError(f"consensus input closure contains an unsafe file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        opened = os.fstat(descriptor)
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    binding = (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns)
    if binding != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise RuntimeError(f"consensus input closure changed while read: {path}")
    data = b"".join(chunks)
    if len(data) != opened.st_size:
        raise RuntimeError(f"consensus input closure length changed while read: {path}")
    return data, bool(opened.st_mode & 0o111)


def source_rows(root: Path) -> list[list[object]]:
    rows: list[list[object]] = []
    for relative in SNAPSHOT_PATHS:
        path = root / relative
        if path.is_symlink() or not path.exists():
            raise RuntimeError(f"snapshot source is absent or unsafe: {path}")
        children = [path] if path.is_file() else sorted(path.rglob("*"))
        for child in children:
            if child.is_symlink():
                raise RuntimeError(f"snapshot source contains a symlink: {child}")
            if child.is_dir() or "__pycache__" in child.parts or child.suffix == ".pyc":
                continue
            data, executable = read_regular(child)
            rows.append(
                [child.relative_to(root).as_posix(), len(data), sha256(data), executable]
            )
    rows.sort(key=lambda row: str(row[0]))
    return rows


def snapshot_rows(root: Path) -> list[list[object]]:
    rows: list[list[object]] = []
    for child in sorted(root.rglob("*")):
        if child.is_symlink():
            raise RuntimeError(f"immutable snapshot contains a symlink: {child}")
        if child.is_dir() or child.name == "snapshot-manifest.v1.json":
            continue
        data, executable = read_regular(child)
        rows.append(
            [child.relative_to(root).as_posix(), len(data), sha256(data), executable]
        )
    return rows


def historical_predecessor_rows(source_archive: bytes) -> list[list[object]] | None:
    rows: list[list[object]] = []
    seen: set[str] = set()
    try:
        with tarfile.open(fileobj=io.BytesIO(source_archive), mode="r:gz") as archive:
            for member in archive.getmembers():
                if member.name not in PREDECESSOR_PATHS:
                    continue
                if member.name in seen or not member.isfile():
                    return None
                stream = archive.extractfile(member)
                if stream is None:
                    return None
                data = stream.read()
                digest = sha256(data)
                if EXPECTED_PREDECESSOR_SHA256.get(member.name) != digest:
                    return None
                seen.add(member.name)
                rows.append([member.name, len(data), digest])
    except (OSError, tarfile.TarError):
        return None
    if seen != set(PREDECESSOR_PATHS):
        return None
    rows.sort(key=lambda row: PREDECESSOR_PATHS.index(str(row[0])))
    return rows


def validate_predecessor(predecessor: dict[str, Any], source_archive: bytes) -> bool:
    historical_rows = historical_predecessor_rows(source_archive)
    if historical_rows is None:
        return False
    current_rows = artifact_file_rows(PREDECESSOR_PATHS)
    historical = predecessor.get("historical_receipt_validation")
    return (
        predecessor.get("files") == historical_rows
        and predecessor.get("current_dependency_files") == current_rows
        and predecessor.get("current_dependency_differs_from_history")
        == (current_rows != historical_rows)
        and predecessor.get("identity") == EXPECTED_STAGE4_IDENTITY
        and predecessor.get("source_commit") == EXPECTED_STAGE4_SOURCE_COMMIT
        and predecessor.get("source_tree") == EXPECTED_STAGE4_SOURCE_TREE
        and predecessor.get("source_archive_byte_length")
        == EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH
        and predecessor.get("source_archive_sha256")
        == EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256
        and len(source_archive) == EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH
        and sha256(source_archive) == EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256
        and historical
        == {
            "archive_matches_source_commit": True,
            "current_dependency_rows_bound_separately": True,
            "mode": "read_only_commit_tree_content_and_receipt_equality",
            "receipt_count": 4,
            "receipts_report_pass": True,
            "source_commit": EXPECTED_STAGE4_SOURCE_COMMIT,
            "source_tree": EXPECTED_STAGE4_SOURCE_TREE,
        }
    )


def validate_snapshot_manifest(
    manifest: dict[str, Any], manifest_bytes: bytes
) -> bool:
    return (
        set(manifest) == {"schema_version", "snapshot_identity", "source_identity", "source_rows"}
        and manifest.get("schema_version")
        == "maestro.vnext.stage5.immutable-workspace-snapshot.v1"
        and manifest_bytes == canonical_json(manifest)
        and manifest.get("source_rows") == source_rows(WORKSPACE)
        and manifest.get("source_identity")
        == f"sha256:{sha256(canonical_json(manifest.get('source_rows')))}"
        and manifest.get("snapshot_identity")
        == f"sha256:{sha256(canonical_json(snapshot_rows(WORKSPACE)))}"
    )


def validate_toolchain(toolchain: dict[str, Any], toolchain_path: Path, target: str) -> bool:
    rows = toolchain.get("files")
    if not isinstance(rows, list) or not rows:
        return False
    root = toolchain_path.parent.resolve(strict=True)
    toolchain_root = root / "toolchain"
    if toolchain_root.is_symlink() or not toolchain_root.is_dir():
        return False
    actual = []
    for path in sorted(toolchain_root.rglob("*")):
        if path.is_symlink():
            return False
        if path.is_dir():
            continue
        data, executable = read_regular(path)
        actual.append(
            [path.relative_to(root).as_posix(), len(data), sha256(data), executable]
        )
    for row in rows:
        if (
            not isinstance(row, list)
            or len(row) != 4
            or not isinstance(row[0], str)
            or not isinstance(row[1], int)
            or not isinstance(row[2], str)
            or not isinstance(row[3], bool)
        ):
            return False
        relative = Path(row[0])
        if (
            relative.is_absolute()
            or ".." in relative.parts
            or relative.parts[:1] != ("toolchain",)
        ):
            return False
    return (
        rows == sorted(actual, key=lambda row: str(row[0]))
        and len({str(row[0]) for row in rows}) == len(rows)
        and toolchain.get("schema_version")
        == "maestro.vnext.stage5.rust-toolchain-closure.v1"
        and toolchain.get("target") == target
        and toolchain.get("identity") == f"sha256:{sha256(canonical_json(rows))}"
    )


def validate_receipt_identity(receipt: dict[str, Any]) -> bool:
    value = {key: item for key, item in receipt.items() if key != "receipt_identity"}
    return receipt.get("receipt_identity") == f"sha256:{sha256(canonical_json(value))}"


def has_exact_diagnostic_proof_claim(value: dict[str, Any]) -> bool:
    return value.get("diagnostic_proof_claim") == DIAGNOSTIC_PROOF_CLAIM


def artifact_file_rows(paths: tuple[str, ...]) -> list[list[object]]:
    rows = []
    for relative in paths:
        data, _ = read_regular(WORKSPACE / relative)
        rows.append([relative, len(data), sha256(data)])
    return rows


def validate_artifact_grammar(artifact: object, *, require_full: bool) -> bool:
    if not isinstance(artifact, dict):
        return False
    try:
        encoded = bytes.fromhex(artifact.get("cbor_hex", ""))
        catalog_bytes, _ = read_regular(
            WORKSPACE / "contracts/vnext/catalogs/generated/catalog-01-observation.json"
        )
        catalog = json.loads(catalog_bytes)
        behavior = artifact.get("behavior")
        if require_full:
            if (
                not isinstance(behavior, dict)
                or set(behavior) != {"passed", "runs"}
                or behavior.get("passed") != EXPECTED_BEHAVIOR_TESTS
            ):
                return False
            semantic_behavior_runs(behavior.get("runs"))
        elif behavior != {"mode": "preflight", "passed": 0}:
            return False
    except (OSError, RuntimeError, TypeError, ValueError, json.JSONDecodeError):
        return False
    expected_protocol = {
        "acquisition_modes": ACQUISITION_MODES,
        "gate_input_classes": INPUT_CLASSES,
        "gate_operators": OPERATORS,
        "gate_results": RESULTS,
    }
    return (
        set(artifact) == ARTIFACT_KEYS
        and artifact.get("schema_version") == DOMAIN
        and artifact.get("domain") == DOMAIN
        and artifact.get("publication_state") == "inactive_candidate"
        and artifact.get("diagnostic_proof_claim") == DIAGNOSTIC_PROOF_CLAIM
        and artifact.get("stage") == 5
        and artifact.get("behavior_manifest_identity")
        == EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
        and artifact.get("observation_contract_table_identity")
        == EXPECTED_OBSERVATION_CONTRACT_TABLE_IDENTITY
        and artifact.get("observation_catalog_manifest_id") == catalog.get("manifest_id")
        and artifact.get("observation_kinds") == catalog.get("manifest_rows")
        and artifact.get("source_closure")
        == artifact_file_rows(tuple(sorted(ARTIFACT_SOURCE_PATHS)))
        and artifact.get("predecessors") == artifact_file_rows(PREDECESSOR_PATHS)
        and artifact.get("protocol") == expected_protocol
        and artifact.get("invalidation_reasons") == INVALIDATION_REASONS
        and artifact.get("invariants") == INVARIANTS
        and type(artifact.get("byte_length")) is int
        and artifact.get("byte_length") == len(encoded)
        and isinstance(artifact.get("artifact_id"), str)
        and artifact.get("artifact_id") == sha256(encoded)
    )


def validate_engine_receipt(
    name: str, receipt: object, artifact: dict[str, Any]
) -> bool:
    contract = ENGINE_RECEIPT_CONTRACTS.get(name)
    sources = artifact.get("source_closure")
    if contract is None or not isinstance(receipt, dict) or not isinstance(sources, list):
        return False
    schema_version, engine_hash_key, engine_path = contract
    try:
        semantic_behavior_runs(receipt.get("behavior_runs"))
    except RuntimeError:
        return False
    expected_keys = {
        "artifact_id",
        "artifact_sha256",
        "behavior_manifest_identity",
        "behavior_passed",
        "behavior_runs",
        "diagnostic_proof_claim",
        engine_hash_key,
        "publication_state",
        "receipt_identity",
        "schema_version",
        "source_closure_sha256",
    }
    source_hashes = {
        row[0]: row[2]
        for row in sources
        if isinstance(row, list)
        and len(row) == 3
        and isinstance(row[0], str)
        and isinstance(row[2], str)
    }
    return (
        set(receipt) == expected_keys
        and set(artifact) == ARTIFACT_KEYS
        and has_exact_diagnostic_proof_claim(artifact)
        and has_exact_diagnostic_proof_claim(receipt)
        and receipt.get("artifact_id") == artifact.get("artifact_id")
        and receipt.get("artifact_sha256") == sha256(pretty_json(artifact))
        and receipt.get("behavior_manifest_identity")
        == EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
        and receipt.get("behavior_passed") == EXPECTED_BEHAVIOR_TESTS
        and receipt.get("publication_state") == "inactive_candidate"
        and receipt.get("schema_version") == schema_version
        and receipt.get(engine_hash_key) == source_hashes.get(engine_path)
        and receipt.get("source_closure_sha256")
        == sha256(canonical_json(sources))
        and validate_receipt_identity(receipt)
    )


def validate_harness_receipt(harness: object) -> bool:
    if not isinstance(harness, dict):
        return False
    tests = harness.get("tests")
    return (
        harness.get("schema_version")
        == "maestro.vnext.stage5.proof-harness-receipt.v1"
        and harness.get("passed") == EXPECTED_PROOF_HARNESS_TESTS
        and set(harness)
        == {
            "diagnostic_proof_claim",
            "manifest_identity",
            "passed",
            "schema_version",
            "tests",
        }
        and has_exact_diagnostic_proof_claim(harness)
        and harness.get("manifest_identity") == EXPECTED_PROOF_HARNESS_MANIFEST_IDENTITY
        and isinstance(tests, list)
        and all(isinstance(test, str) for test in tests)
        and len(tests) == EXPECTED_PROOF_HARNESS_TESTS
        and len(set(tests)) == EXPECTED_PROOF_HARNESS_TESTS
        and harness.get("manifest_identity")
        == f"sha256:{sha256(canonical_json(tests))}"
    )


def behavior_manifest_rows(runs: object) -> list[list[str]]:
    if not isinstance(runs, list) or len(runs) < 2:
        raise RuntimeError("Stage 5 behavior runs are malformed")
    rows: list[list[str]] = []
    for run in runs[:-1]:
        if not isinstance(run, dict) or not isinstance(run.get("tests"), list):
            raise RuntimeError("Stage 5 behavior run is malformed")
        for test in run["tests"]:
            if not isinstance(test, dict):
                raise RuntimeError("Stage 5 behavior test receipt is malformed")
            command = test.get("command")
            name = test.get("name")
            if (
                set(test) != {"command", "name", "result"}
                or
                not isinstance(command, list)
                or len(command) != 4
                or not isinstance(command[0], str)
                or not isinstance(name, str)
                or command != [command[0], name, "--exact", "--nocapture"]
                or test.get("result") != "pass"
            ):
                raise RuntimeError("Stage 5 behavior test receipt is not exact")
            rows.append([command[0], name])
    if len(rows) != EXPECTED_BEHAVIOR_TESTS or len({tuple(row) for row in rows}) != len(rows):
        raise RuntimeError("Stage 5 behavior manifest count or uniqueness differs")
    return rows


def semantic_behavior_runs(runs: object) -> list[dict[str, Any]]:
    manifest_rows = behavior_manifest_rows(runs)
    if not isinstance(runs, list) or len(runs) < 2:
        raise RuntimeError("Stage 5 behavior runs are malformed")
    binary_by_target: dict[str, str] = {}
    semantic_runs: list[dict[str, Any]] = []
    labels: set[str] = set()
    total_passed = 0

    def bind_binary(run: dict[str, Any], target: str) -> None:
        binary_sha256 = run.get("binary_sha256")
        if not isinstance(binary_sha256, str) or re.fullmatch(
            r"[0-9a-f]{64}", binary_sha256
        ) is None:
            raise RuntimeError("Stage 5 engine-local binary identity is malformed")
        previous = binary_by_target.setdefault(target, binary_sha256)
        if previous != binary_sha256:
            raise RuntimeError("Stage 5 engine-local binary identity is inconsistent")

    if len(runs) != len(EXPECTED_NORMAL_RUNS) + 1:
        raise RuntimeError("Stage 5 behavior run count differs")
    for run, (expected_label, expected_target, expected_count) in zip(
        runs[:-1], EXPECTED_NORMAL_RUNS, strict=True
    ):
        if not isinstance(run, dict):
            raise RuntimeError("Stage 5 behavior run is malformed")
        label = run.get("label")
        tests = run.get("tests")
        passed = run.get("passed")
        if (
            not isinstance(label, str)
            or re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", label) is None
            or label != expected_label
            or label in labels
            or not isinstance(tests, list)
            or not tests
            or type(passed) is not int
            or passed != expected_count
            or passed != len(tests)
            or set(run) != {"binary_sha256", "label", "passed", "tests"}
        ):
            raise RuntimeError("Stage 5 normal behavior run is malformed")
        labels.add(label)
        total_passed += passed
        targets = {test["command"][0] for test in tests}
        if len(targets) != 1:
            raise RuntimeError("Stage 5 behavior run target is malformed or ambiguous")
        target = next(iter(targets))
        if target != expected_target:
            raise RuntimeError("Stage 5 behavior run target differs")
        bind_binary(run, target)
        semantic_runs.append(
            {key: value for key, value in run.items() if key != "binary_sha256"}
        )

    if total_passed != EXPECTED_BEHAVIOR_TESTS:
        raise RuntimeError("Stage 5 declared behavior pass total differs")

    mutant = runs[-1]
    if not isinstance(mutant, dict):
        raise RuntimeError("Stage 5 behavior mutant is malformed")
    first_target, first_exact_name = manifest_rows[0]
    if (
        set(mutant)
        != {
            "binary_sha256",
            "command",
            "label",
            "passed",
            "rejected",
            "result",
            "substituted_for",
        }
        or
        mutant.get("label") != "same-count-substitution-mutant"
        or type(mutant.get("passed")) is not int
        or mutant.get("passed") != 0
        or mutant.get("rejected") is not True
        or mutant.get("result") != "rejected"
        or mutant.get("substituted_for") != first_exact_name
        or mutant.get("command")
        != [
            first_target,
            f"{first_exact_name}_same_count_substitution_mutant",
            "--exact",
            "--nocapture",
        ]
    ):
        raise RuntimeError("Stage 5 same-count substitution mutant is malformed")
    bind_binary(mutant, first_target)
    semantic_runs.append(
        {key: value for key, value in mutant.items() if key != "binary_sha256"}
    )
    return semantic_runs


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--builder", type=Path, required=True)
    parser.add_argument("--validator", type=Path, required=True)
    parser.add_argument("--ruby", type=Path, required=True)
    parser.add_argument("--predecessor", type=Path, required=True)
    parser.add_argument("--predecessor-source", type=Path, required=True)
    parser.add_argument("--harness", type=Path, required=True)
    parser.add_argument("--snapshot-manifest", type=Path, required=True)
    parser.add_argument("--toolchain", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()

    artifact, artifact_bytes = read_json(args.artifact)
    predecessor, predecessor_bytes = read_json(args.predecessor)
    predecessor_source_bytes, _ = read_regular(args.predecessor_source)
    harness, harness_bytes = read_json(args.harness)
    snapshot_manifest, snapshot_manifest_bytes = read_json(args.snapshot_manifest)
    toolchain, toolchain_bytes = read_json(args.toolchain)
    named_receipts = {
        "builder": read_json(args.builder),
        "validator": read_json(args.validator),
        "ruby": read_json(args.ruby),
    }
    artifact_id = artifact.get("artifact_id")
    artifact_sha256 = sha256(artifact_bytes)
    if (
        not isinstance(artifact_id, str)
        or not validate_artifact_grammar(artifact, require_full=True)
        or not validate_predecessor(predecessor, predecessor_source_bytes)
        or not validate_harness_receipt(harness)
        or harness.get("manifest_identity")
        != EXPECTED_PROOF_HARNESS_MANIFEST_IDENTITY
        or harness.get("manifest_identity")
        != f"sha256:{sha256(canonical_json(harness['tests']))}"
        or harness_bytes != canonical_json(harness)
        or not validate_snapshot_manifest(snapshot_manifest, snapshot_manifest_bytes)
        or not validate_toolchain(toolchain, args.toolchain, args.target)
    ):
        raise RuntimeError("Stage 5 artifact or exact Stage 4 predecessor differs")
    behavior_runs = None
    input_rows = []
    for name, (receipt, receipt_bytes) in named_receipts.items():
        runs = receipt.get("behavior_runs")
        if (
            receipt.get("artifact_id") != artifact_id
            or receipt.get("artifact_sha256") != artifact_sha256
            or receipt.get("behavior_manifest_identity")
            != EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
            or receipt.get("behavior_passed") != EXPECTED_BEHAVIOR_TESTS
            or receipt.get("publication_state") != "inactive_candidate"
            or not has_exact_diagnostic_proof_claim(receipt)
            or not validate_engine_receipt(name, receipt, artifact)
            or not isinstance(runs, list)
            or not runs
            or runs[-1].get("label") != "same-count-substitution-mutant"
            or runs[-1].get("rejected") is not True
            or runs[-1].get("result") != "rejected"
            or any(
                "--exact" not in test.get("command", [])
                for run in runs[:-1]
                for test in run.get("tests", [])
            )
        ):
            raise RuntimeError(f"{name} Stage 5 receipt is incomplete or disagrees")
        manifest_rows = behavior_manifest_rows(runs)
        if (
            f"sha256:{sha256(canonical_json(manifest_rows))}"
            != EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
        ):
            raise RuntimeError(f"{name} Stage 5 behavior manifest differs")
        semantic_runs = semantic_behavior_runs(runs)
        if behavior_runs is None:
            behavior_runs = semantic_runs
        elif semantic_runs != behavior_runs:
            raise RuntimeError("Stage 5 engines disagree on exact behavioral receipts")
        input_rows.append([name, len(receipt_bytes), sha256(receipt_bytes)])
    input_rows.extend(
        [
            ["artifact", len(artifact_bytes), artifact_sha256],
            ["harness", len(harness_bytes), sha256(harness_bytes)],
            ["predecessor", len(predecessor_bytes), sha256(predecessor_bytes)],
            [
                "predecessor-source",
                len(predecessor_source_bytes),
                sha256(predecessor_source_bytes),
            ],
            ["snapshot-manifest", len(snapshot_manifest_bytes), sha256(snapshot_manifest_bytes)],
            ["toolchain", len(toolchain_bytes), sha256(toolchain_bytes)],
        ]
    )
    input_rows.sort()
    value = {
        "artifact_id": artifact_id,
        "behavior_passed": EXPECTED_BEHAVIOR_TESTS,
        "behavior_manifest_identity": EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
        "diagnostic_proof_claim": DIAGNOSTIC_PROOF_CLAIM,
        "exact_behavior_receipt_sha256": sha256(canonical_json(behavior_runs)),
        "inputs": input_rows,
        "predecessor_identity": EXPECTED_STAGE4_IDENTITY,
        "proof_harness_passed": EXPECTED_PROOF_HARNESS_TESTS,
        "publication_state": "inactive_candidate",
        "schema_version": "maestro.vnext.stage5.three-engine-consensus.v1",
    }
    receipt = {**value, "consensus_identity": f"sha256:{sha256(canonical_json(value))}"}
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "three-engine-consensus-receipt.v1.json").write_bytes(
        pretty_json(receipt)
    )
    (args.output_root / "workspace-snapshot-manifest.v1.json").write_bytes(
        snapshot_manifest_bytes
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
