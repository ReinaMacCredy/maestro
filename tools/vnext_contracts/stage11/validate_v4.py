#!/usr/bin/env python3
"""Validate the effect-inert Stage-11 V4 loss and root-universe closure."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Callable, Mapping, cast


ROOT = Path(__file__).resolve().parents[3]
CONTRACT = ROOT / "tests/fixtures/vnext/stage11/live_set_v4_contract.v1.json"
ROOT_UNIVERSE = ROOT / "tests/fixtures/vnext/stage11/root-universe.v1.json"
CONTRACT_SCHEMA = "maestro.test-only.vnext-stage11-live-set-v4-contract.v1"
ROOT_SCHEMA = "maestro.test-only.vnext-stage11-root-universe.v1"
PACKET_SHA256 = "d0953ac33f361ccad2fe0c7844294324b7b33cb974e16a11639ad3aad19e40e2"
DESIGN = {
    "commit": "bb7b1ee0e51fa591b21943e8c7d50844cb4d0b05",
    "parent": "1685b39138a045bcd5e87744860d95eb589999d2",
    "tree": "cb6b62cc187abdecebef8f621206289029fb590b",
}
REPOSITORY_ROLES = {"RepositoryStore"}
INSTALLATION_ROLES = {
    "Active",
    "Inactive",
    "Snapshot",
    "Cache",
    "Archive",
    "Host",
    "Legacy",
}
EVIDENCE_FIELDS = {
    "history_snapshot_identity",
    "history_owner",
    "root_source_provenance",
    "lossless_locator_commitment",
    "object_kind",
    "object_identity",
    "byte_length",
    "content_sha256",
    "metadata_commitment",
    "store_generation",
    "store_head",
    "namespace_epoch",
    "trust_root",
    "release",
    "provider",
    "mount",
    "anchor",
    "fence",
    "currentness",
    "revocation",
}
CONTRACT_FIELDS = {
    "current_types",
    "design",
    "forbidden_source_tokens",
    "historical_current_authority_forbidden",
    "historical_evidence_fields",
    "historical_immutable_files",
    "packet_sha256",
    "public_delta",
    "required_source_tokens",
    "schema_version",
    "state",
    "unchanged_shape_types",
}
ROOT_FIELDS = {
    "alias_relations",
    "caller_roots",
    "census_header_scope_authority",
    "census_schema_delta",
    "expected_sources",
    "final_recheck",
    "header_references",
    "installation",
    "observation_sequence",
    "operation_attempt",
    "pass_a_present_rows",
    "pass_b_present_rows",
    "protected_primary_journal",
    "repository",
    "schema_version",
}
PUBLIC_DELTA_FIELDS = {
    "installation_census_header_bytes_changed",
    "installation_census_bytes_changed",
    "mcp_recipe_resource_bundle_release_catalog_wire_changed",
    "packet_operation_result_action_scope_ceremony_changed",
}
UNCHANGED_SHAPE_TYPES = [
    "MembershipKeyV3",
    "SourceCaseV3",
    "Stage12SightingManifestV2",
    "MigrationClassificationManifestV3",
    "DeclaredOverlapManifestV2",
    "SealedQuarantineManifestV3",
]
HISTORICAL_IMMUTABLE_FILES = {
    "tests/fixtures/vnext/stage11/live_set_v3_contract.v1.json": (
        "3ce733b5b13473c92b5aa78b507db51fc06507c258d0b20e7dccdda228b22c35"
    ),
    "tools/vnext_contracts/stage11/validate_v3.py": (
        "4b865ea300c79cfb6e95f554b2392443d91d12601d5109b4df3c04cd67df7c8f"
    ),
    "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v2.json": (
        "3a38959643425a8413c55edf899f046f147514375dead85ce3096f16c11f52ec"
    ),
    "tools/vnext_contracts/stage12/coordinator.py": (
        "0210ad3a2982ad6cf8b36f0f86bbeb52dbe59750863f0d0be8dbc849b4ce5a39"
    ),
}
REQUIRED_SOURCE_TOKENS = {
    "src/domain/repository/legacy_source_history.rs": [
        "LegacySourceHistorySnapshotV1",
        "FoundationOwnerEvidenceMintV1",
        "issue_bound_absent_sources",
    ],
    "src/domain/repository/root_universe.rs": [
        "RepositoryDeclaredRootUniverseLeaseV1",
    ],
    "src/domain/installation/legacy_source_history.rs": [
        "LegacySourceHistorySnapshotV1",
        "FoundationOwnerEvidenceMintV1",
        "issue_bound_absent_sources",
    ],
    "src/domain/installation/root_universe.rs": [
        "InstallationDeclaredRootUniverseLeaseV1",
        "COMPLETE_INSTALLATION_ROOT_ROLES_V1",
        "census_comparison_identity",
        "IncompleteRoleCoverage",
    ],
    "src/domain/persistence/legacy_source_history.rs": [
        "ProtectedPrimaryHistoryJournalV1",
        "FoundationOwnerEvidenceMintV1",
        "record_unavailable_preexisting_loss",
        "StoreLegacySourceProviderObservationV1",
    ],
    "src/foundation/core/legacy_loss_evidence.rs": [
        "FoundationOwnerEvidenceMintV1",
        "record_unavailable_preexisting_loss",
        "pub(in crate::foundation::core) fn finish(",
        "PhantomData<Rc<()>>",
        "FoundationValidatedUnavailablePreexistingLossReceiptV1",
    ],
    "src/foundation/core/legacy_quarantine.rs": [
        "FoundationLegacyQuarantineLeaseV2",
        "FoundationLegacyQuarantineClosureV2",
    ],
    "src/domain/migration/runtime/live_set_v3.rs": [
        "UnavailablePreexistingLossV4",
        "UnavailablePreexistingLossManifestV4",
        "UnavailablePreexistingLossAuditCurrentnessV4",
        "encode_canonical_audit",
        "decode_canonical_audit",
        "LegacyRollbackAssessmentV4",
        "LegacyQuarantineEpochV4",
    ],
    "src/operations/migration/live_set_v3.rs": [
        "FoundationSourceCopyContinuationV2",
        "UnavailablePreexistingLossAuditPersistencePortV1",
        "for FoundationSourceCopyContinuationV2<P, Q>",
        "persist_unavailable_preexisting_loss_audits_v4",
        "&mut self.physical",
        "UnavailablePreexistingLossAuditGateErrorV1",
        "LossAuditRollbackFailed",
        "audit_failure_after_rollback",
        ".finish(self.quarantine.identity().into_bytes())?",
    ],
    "src/domain/persistence/legacy_quarantine.rs": [
        "QuarantineCustodyLeaseV1",
        "create_loss_audit_if_absent",
        "read_loss_audit",
        "recheck_loss_audit_custody",
        "loss_audit_path",
        "recovery/legacy-loss-audit-v4",
        "create_file_if_absent",
        "read_immutable",
    ],
    "src/domain/authority/legacy_removal_guard.rs": [
        "LegacyRemovalGuardV3",
        "LegacyRemovalConsumerBindingV3",
        "expected_old_state",
        "consume_bound_with_linearization",
        "with_serialized_active_view",
    ],
}
FORBIDDEN_SOURCE_TOKENS = {
    "src/domain/repository/legacy_source_history.rs": [
        "OwnerIssuedUnavailablePreexistingLossEvidenceSetV1",
    ],
    "src/domain/installation/legacy_source_history.rs": [
        "OwnerIssuedUnavailablePreexistingLossEvidenceSetV1",
    ],
    "src/domain/persistence/legacy_source_history.rs": [
        "OwnerIssuedUnavailablePreexistingLossEvidenceSetV1",
    ],
    "src/domain/persistence/mod.rs": [
        "ImmutableRecoveryAuditPersistenceV1",
        "IMMUTABLE_RECOVERY_AUDIT_DIRECTORY_V1",
    ],
    "src/operations/migration/mod.rs": [
        "persistence_store",
        "immutable_recovery_audit_persistence_v1",
        "ImmutableRecoveryAuditPersistenceV1",
    ],
    "src/domain/migration/runtime/live_set_v3.rs": [
        "PathBuf",
        "LegacySourceHistorySnapshotV1",
        "OwnerIssuedUnavailablePreexistingLossEvidenceSetV1",
        "FoundationValidatedUnavailablePreexistingLossReceiptV1::new",
    ],
    "src/domain/authority/legacy_removal_guard.rs": [
        "LegacyRemovalGuardV2::",
        "UnavailablePreexistingLossManifestV3",
        "LegacyQuarantineEpochV3",
    ],
}
ROW_FIELDS = {
    "declaration_identity",
    "declaration_revision",
    "role",
    "required",
    "provider_revision",
    "locator_commitment",
    "owner_realm",
    "operation_attempt",
    "currentness",
    "fence",
    "revocation",
    "disposition",
    "retained_root_capability",
    "absence_fence",
}
HEADER_STRUCT = """pub struct InstallationCensusHeaderV1 {
    pub domain: DistributionDomainRefV1,
    pub inspection_request_ref: DistributionScopedObjectRefV1,
    pub declared_root_set_ref: DistributionScopedObjectRefV1,
    pub host_adapter_set_ref: DistributionScopedObjectRefV1,
    pub legacy_locator_set_ref: DistributionScopedObjectRefV1,
    pub observed_state_ref: DistributionScopedObjectRefV1,
    pub proof_profile_id: CommitmentV1,
}"""
CENSUS_STRUCT = """pub struct InstallationCensusV1 {
    pub header: InstallationCensusHeaderV1,
    pub rows: Vec<(u64, CommitmentV1, InstallationCensusEntryV1)>,
}"""
HEX64 = re.compile(r"^[0-9a-f]{64}$")


class ValidationError(RuntimeError):
    """A V4 proof input violates one exact default-deny rule."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(f"{code}: {detail}" if detail else code)
        self.code = code


def _reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValidationError("duplicate_json_key", key)
        value[key] = item
    return value


def load_json(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ValidationError("unsafe_or_missing_fixture", str(path))
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise ValidationError("noncanonical_fixture_bytes", str(path))
    try:
        value = json.loads(raw, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValidationError("invalid_fixture_json", str(error)) from error
    if not isinstance(value, dict):
        raise ValidationError("fixture_not_object", str(path))
    return cast(dict[str, Any], value)


def _mapping(value: object, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ValidationError("invalid_object", label)
    return cast(Mapping[str, Any], value)


def _list(value: object, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise ValidationError("invalid_list", label)
    return cast(list[Any], value)


def _identity(value: object, label: str) -> str:
    if not isinstance(value, str) or HEX64.fullmatch(value) is None or set(value) == {"0"}:
        raise ValidationError("invalid_identity", label)
    return value


def _positive(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        raise ValidationError("invalid_positive_integer", label)
    return value


def validate_contract(contract: Mapping[str, Any], repository: Path = ROOT) -> None:
    if set(contract) != CONTRACT_FIELDS:
        raise ValidationError("contract_fields_differ")
    if contract.get("schema_version") != CONTRACT_SCHEMA:
        raise ValidationError("contract_schema_differs")
    if contract.get("state") != "candidate_preparation_effect_inert_unintegrated":
        raise ValidationError("contract_state_differs")
    if contract.get("design") != DESIGN or contract.get("packet_sha256") != PACKET_SHA256:
        raise ValidationError("design_or_packet_binding_differs")
    public_delta = _mapping(contract.get("public_delta"), "public_delta")
    if set(public_delta) != PUBLIC_DELTA_FIELDS or set(public_delta.values()) != {False}:
        raise ValidationError("public_or_persisted_delta_claimed")
    if contract.get("unchanged_shape_types") != UNCHANGED_SHAPE_TYPES:
        raise ValidationError("unchanged_shape_type_closure_differs")
    if (
        contract.get("required_source_tokens") != REQUIRED_SOURCE_TOKENS
        or contract.get("forbidden_source_tokens") != FORBIDDEN_SOURCE_TOKENS
    ):
        raise ValidationError("source_token_closure_differs")
    if set(contract.get("historical_evidence_fields", [])) != EVIDENCE_FIELDS:
        raise ValidationError("historical_evidence_field_closure_differs")
    current = _mapping(contract.get("current_types"), "current_types")
    expected_current = {
        "foundation_closure": "FoundationLegacyQuarantineClosureV2",
        "foundation_lease": "FoundationLegacyQuarantineLeaseV2",
        "foundation_owner_evidence_mint": "FoundationOwnerEvidenceMintV1",
        "loss": "UnavailablePreexistingLossV4",
        "loss_manifest": "UnavailablePreexistingLossManifestV4",
        "loss_audit_currentness": "UnavailablePreexistingLossAuditCurrentnessV4",
        "loss_audit_gate": "UnavailablePreexistingLossAuditGateErrorV1",
        "loss_audit_custody": "QuarantineCustodyLeaseV1",
        "rollback": "LegacyRollbackAssessmentV4",
        "epoch": "LegacyQuarantineEpochV4",
        "guard": "LegacyRemovalGuardV3",
        "guard_consumer_binding": "LegacyRemovalConsumerBindingV3",
        "coordinator": "Stage12LegacyCutCoordinatorV3",
    }
    if dict(current) != expected_current:
        raise ValidationError("current_type_closure_differs")
    historical = set(contract.get("historical_current_authority_forbidden", []))
    if historical != {
        "UnavailablePreexistingLossV3",
        "UnavailablePreexistingLossManifestV3",
        "LegacyRollbackAssessmentV3",
        "LegacyQuarantineEpochV3",
        "LegacyRemovalGuardV2",
        "Stage12LegacyCutCoordinatorV2",
    }:
        raise ValidationError("historical_authority_denylist_differs")
    immutable = _mapping(contract.get("historical_immutable_files"), "historical files")
    if dict(immutable) != HISTORICAL_IMMUTABLE_FILES:
        raise ValidationError("historical_artifact_binding_differs")
    for relative, expected in immutable.items():
        path = repository / str(relative)
        if path.is_symlink() or not path.is_file():
            raise ValidationError("historical_artifact_missing", str(relative))
        observed = hashlib.sha256(path.read_bytes()).hexdigest()
        if observed != expected:
            raise ValidationError("historical_artifact_drift", str(relative))


def _validate_row(
    row_value: object,
    *,
    owner: str,
    operation_attempt: str,
    provider_revision: int,
    currentness: str,
    revocation: int,
) -> Mapping[str, Any]:
    row = _mapping(row_value, f"{owner} row")
    if set(row) != ROW_FIELDS:
        raise ValidationError("root_row_fields_differ", owner)
    for field in (
        "declaration_identity",
        "locator_commitment",
        "owner_realm",
        "operation_attempt",
        "currentness",
        "fence",
    ):
        _identity(row[field], f"{owner}.{field}")
    _positive(row["declaration_revision"], f"{owner}.declaration_revision")
    _positive(row["provider_revision"], f"{owner}.provider_revision")
    _positive(row["revocation"], f"{owner}.revocation")
    if (
        row["operation_attempt"] != operation_attempt
        or row["provider_revision"] != provider_revision
        or row["currentness"] != currentness
        or row["revocation"] != revocation
    ):
        raise ValidationError("owner_row_currentness_differs", owner)
    disposition = row["disposition"]
    if disposition == "Present":
        _identity(row["retained_root_capability"], f"{owner}.retained_root_capability")
        if row["absence_fence"] is not None:
            raise ValidationError("present_row_has_absence_fence", owner)
    elif disposition == "DeclaredAbsent":
        if row["required"] is not False:
            raise ValidationError("required_declaration_absent", owner)
        if row["retained_root_capability"] is not None:
            raise ValidationError("absent_row_has_retained_root", owner)
        if row["absence_fence"] != row["fence"]:
            raise ValidationError("stale_or_foreign_absence_fence", owner)
    elif disposition == "Unsupported":
        raise ValidationError("unsupported_production_row", owner)
    else:
        raise ValidationError("unknown_root_disposition", owner)
    return row


def _validate_owner(
    owner_value: object,
    *,
    owner: str,
    roles: set[str],
    operation_attempt: str,
) -> tuple[list[Mapping[str, Any]], set[str]]:
    owner_value = _mapping(owner_value, owner)
    if set(owner_value) != {
        "declaration_set_revision",
        "provider_revision",
        "currentness",
        "revocation",
        "rows",
    }:
        raise ValidationError("owner_universe_fields_differ", owner)
    _positive(owner_value["declaration_set_revision"], f"{owner}.declaration_set_revision")
    provider_revision = _positive(
        owner_value["provider_revision"], f"{owner}.provider_revision"
    )
    currentness = _identity(owner_value["currentness"], f"{owner}.currentness")
    revocation = _positive(owner_value["revocation"], f"{owner}.revocation")
    rows = [
        _validate_row(
            row,
            owner=owner,
            operation_attempt=operation_attempt,
            provider_revision=provider_revision,
            currentness=currentness,
            revocation=revocation,
        )
        for row in _list(owner_value["rows"], f"{owner}.rows")
    ]
    identities = [str(row["declaration_identity"]) for row in rows]
    if identities != sorted(identities):
        raise ValidationError("root_rows_not_canonical_order", owner)
    if len(identities) != len(set(identities)):
        raise ValidationError("duplicate_root_declaration", owner)
    observed_roles = {str(row["role"]) for row in rows}
    if observed_roles != roles:
        raise ValidationError("root_universe_incomplete_or_extra", owner)
    if len(rows) != len(roles):
        raise ValidationError("root_role_substitution_or_duplicate", owner)
    return rows, set(identities)


def validate_root_universe(value: Mapping[str, Any]) -> None:
    if set(value) != ROOT_FIELDS:
        raise ValidationError("root_fixture_fields_differ")
    if value.get("schema_version") != ROOT_SCHEMA:
        raise ValidationError("root_fixture_schema_differs")
    if value.get("census_schema_delta") is not False:
        raise ValidationError("installation_census_schema_delta")
    if value.get("census_header_scope_authority") is not False:
        raise ValidationError("census_header_became_scope_authority")
    if value.get("caller_roots") != []:
        raise ValidationError("caller_root_admission")
    operation_attempt = _identity(value.get("operation_attempt"), "operation_attempt")
    repository_rows, repository_ids = _validate_owner(
        value.get("repository"),
        owner="Repository",
        roles=REPOSITORY_ROLES,
        operation_attempt=operation_attempt,
    )
    installation_rows, installation_ids = _validate_owner(
        value.get("installation"),
        owner="Installation",
        roles=INSTALLATION_ROLES,
        operation_attempt=operation_attempt,
    )
    if repository_ids & installation_ids:
        raise ValidationError("cross_owner_duplicate_declaration")
    rows = {
        str(row["declaration_identity"]): (owner, row)
        for owner, owner_rows in (
            ("Repository", repository_rows),
            ("Installation", installation_rows),
        )
        for row in owner_rows
    }
    locators = [str(row["locator_commitment"]) for _owner, row in rows.values()]
    if len(locators) != len(set(locators)):
        raise ValidationError("cross_root_alias")
    if value.get("alias_relations") != []:
        raise ValidationError("ancestor_descendant_or_substitution_alias")
    header = _mapping(value.get("header_references"), "header_references")
    if set(header) != {"declared_root_set", "host_adapter_set", "legacy_locator_set"}:
        raise ValidationError("census_header_reference_closure_differs")
    if any(item != "comparison-only" for item in header.values()):
        raise ValidationError("census_header_became_scope_authority")
    present = {
        identity
        for identity, (_owner, row) in rows.items()
        if row["disposition"] == "Present"
    }
    for field in ("pass_a_present_rows", "pass_b_present_rows"):
        observed = _list(value.get(field), field)
        if len(observed) != len(set(observed)) or set(observed) != present:
            raise ValidationError("two_pass_present_set_differs", field)
    observations = _list(value.get("observation_sequence"), "observation_sequence")
    if len(observations) != 2 or observations[0] != observations[1]:
        raise ValidationError("a_to_b_to_a_or_physical_drift")
    for observation in observations:
        _identity(observation, "observation_sequence")
    expected_sources = _list(value.get("expected_sources"), "expected_sources")
    seen_sources: set[str] = set()
    for source_value in expected_sources:
        source = _mapping(source_value, "expected_source")
        source_identity = _identity(source.get("source_identity"), "source_identity")
        if source_identity in seen_sources:
            raise ValidationError("duplicate_expected_source")
        seen_sources.add(source_identity)
        row_identity = source.get("row")
        if row_identity not in rows:
            raise ValidationError("expected_source_has_no_owner_row")
        owner, row = rows[str(row_identity)]
        if source.get("owner") != owner:
            raise ValidationError("expected_source_owner_mismatch")
        if row["disposition"] == "DeclaredAbsent":
            raise ValidationError("expected_source_under_declared_absent")
        if row["disposition"] == "Unsupported":
            raise ValidationError("expected_source_under_unsupported")
        if row["disposition"] != "Present":
            raise ValidationError("expected_source_not_under_present")
        if source.get("present_in_pass_a") is not False or source.get(
            "present_in_pass_b"
        ) is not False:
            raise ValidationError("unavailable_source_is_present")
        if source.get("post_admission_disappearance") is not False:
            raise ValidationError("post_admission_disappearance_is_not_preexisting_loss")
        if (
            source.get("history_reachable") is not True
            or source.get("history_current") is not True
            or source.get("source_present_at_capture") is not True
        ):
            raise ValidationError("history_not_reachable_current_or_pre_loss")
        if source.get("history_owner") != owner:
            raise ValidationError("history_owner_mismatch")
        evidence = _mapping(source.get("evidence"), "historical_evidence")
        if set(evidence) != EVIDENCE_FIELDS:
            raise ValidationError("historical_evidence_fields_differ")
        for field in EVIDENCE_FIELDS - {
            "history_owner",
            "object_kind",
            "byte_length",
            "store_generation",
            "namespace_epoch",
            "revocation",
        }:
            _identity(evidence[field], f"evidence.{field}")
        if evidence["history_owner"] != owner:
            raise ValidationError("evidence_owner_mismatch")
        if evidence["object_kind"] not in {"file", "directory", "symlink"}:
            raise ValidationError("evidence_object_kind_invalid")
        for field in ("byte_length", "store_generation", "namespace_epoch", "revocation"):
            _positive(evidence[field], f"evidence.{field}")
        if (
            evidence["lossless_locator_commitment"] != row["locator_commitment"]
            or evidence["fence"] != row["fence"]
            or evidence["currentness"] != row["currentness"]
            or evidence["revocation"] != row["revocation"]
        ):
            raise ValidationError("evidence_root_or_currentness_mismatch")
    finality = _mapping(value.get("final_recheck"), "final_recheck")
    repository = _mapping(value.get("repository"), "repository")
    installation = _mapping(value.get("installation"), "installation")
    expected_finality = {
        "repository_declaration_set_revision": repository["declaration_set_revision"],
        "repository_provider_revision": repository["provider_revision"],
        "repository_currentness": repository["currentness"],
        "repository_revocation": repository["revocation"],
        "installation_declaration_set_revision": installation["declaration_set_revision"],
        "installation_provider_revision": installation["provider_revision"],
        "installation_currentness": installation["currentness"],
        "installation_revocation": installation["revocation"],
        "custody_sealed": True,
        "outcome": "closed",
    }
    if dict(finality) != expected_finality:
        raise ValidationError("post_seal_finality_drift")
    journal = _mapping(value.get("protected_primary_journal"), "protected journal")
    if journal != {
        "reachable_from_current_head": True,
        "recorded_while_present": True,
        "backend_current": True,
        "reader_authority": False,
        "locator_authority": False,
        "mutation_authority": False,
    }:
        raise ValidationError("protected_primary_journal_not_current_non_bearer")


def validate_sources(contract: Mapping[str, Any], source_root: Path) -> None:
    required = _mapping(contract.get("required_source_tokens"), "required sources")
    texts: dict[str, str] = {}
    for relative, needles_value in required.items():
        path = source_root / str(relative)
        if path.is_symlink() or not path.is_file():
            raise ValidationError("required_product_source_missing", str(relative))
        text = path.read_text(encoding="utf-8")
        texts[str(relative)] = text
        missing = [
            needle
            for needle in _list(needles_value, f"tokens for {relative}")
            if not isinstance(needle, str) or needle not in text
        ]
        if missing:
            raise ValidationError("required_product_token_missing", f"{relative}: {missing}")
    forbidden = _mapping(contract.get("forbidden_source_tokens"), "forbidden sources")
    for relative, needles_value in forbidden.items():
        text = texts.get(str(relative))
        if text is None:
            path = source_root / str(relative)
            if path.is_symlink() or not path.is_file():
                raise ValidationError("required_product_source_missing", str(relative))
            text = path.read_text(encoding="utf-8")
        present = [
            needle
            for needle in _list(needles_value, f"forbidden tokens for {relative}")
            if isinstance(needle, str) and needle in text
        ]
        if present:
            raise ValidationError("historical_or_locator_adapter_reachable", f"{relative}: {present}")
    census = (source_root / "src/domain/installation/census.rs").read_text(
        encoding="utf-8"
    )
    if HEADER_STRUCT not in census or CENSUS_STRUCT not in census:
        raise ValidationError("installation_census_struct_bytes_changed")
    migration = texts["src/operations/migration/live_set_v3.rs"]
    audit = migration.find("persist_unavailable_preexisting_loss_audits_v4")
    rollback = migration.find("audit_failure_after_rollback", audit)
    finality = migration.find(
        ".finish(self.quarantine.identity().into_bytes())?", rollback
    )
    if min(audit, rollback, finality) < 0 or not audit < rollback < finality:
        raise ValidationError("loss_audit_finality_order_differs")
    custody = texts["src/domain/persistence/legacy_quarantine.rs"]
    if custody.count("self.recheck_loss_audit_custody()?") < 5:
        raise ValidationError("loss_audit_custody_currentness_rechecks_missing")


Mutation = tuple[str, Callable[[dict[str, Any]], None], str]


def mutant_cases(contract: Mapping[str, Any], universe: Mapping[str, Any]) -> list[Mutation]:
    cases: list[Mutation] = []

    def add(name: str, mutate: Callable[[dict[str, Any]], None], code: str) -> None:
        cases.append((name, mutate, code))

    for field in sorted(EVIDENCE_FIELDS):
        add(
            f"missing_evidence_{field}",
            lambda value, field=field: value["expected_sources"][0]["evidence"].pop(field),
            "historical_evidence_fields_differ",
        )
    add(
        "post_admission_disappearance",
        lambda value: value["expected_sources"][0].__setitem__(
            "post_admission_disappearance", True
        ),
        "post_admission_disappearance_is_not_preexisting_loss",
    )
    add(
        "orphan_history",
        lambda value: value["expected_sources"][0].__setitem__("history_reachable", False),
        "history_not_reachable_current_or_pre_loss",
    )
    add(
        "current_absence_only",
        lambda value: value["expected_sources"][0].__setitem__(
            "source_present_at_capture", False
        ),
        "history_not_reachable_current_or_pre_loss",
    )
    add(
        "wrong_owner",
        lambda value: value["expected_sources"][0].__setitem__("history_owner", "Repository"),
        "history_owner_mismatch",
    )
    add(
        "provider_race",
        lambda value: value["installation"]["rows"][0].__setitem__(
            "provider_revision", 18
        ),
        "owner_row_currentness_differs",
    )
    add(
        "revocation_race",
        lambda value: value["installation"]["rows"][0].__setitem__("revocation", 5),
        "owner_row_currentness_differs",
    )
    add(
        "omitted_role",
        lambda value: value["installation"]["rows"].pop(),
        "root_universe_incomplete_or_extra",
    )
    add(
        "duplicate_declaration",
        lambda value: value["installation"]["rows"].append(
            copy.deepcopy(value["installation"]["rows"][0])
        ),
        "root_rows_not_canonical_order",
    )
    add(
        "role_substitution",
        lambda value: value["installation"]["rows"][0].__setitem__("role", "Legacy"),
        "root_universe_incomplete_or_extra",
    )
    add(
        "unsupported",
        lambda value: value["installation"]["rows"][0].__setitem__(
            "disposition", "Unsupported"
        ),
        "unsupported_production_row",
    )
    add(
        "header_without_rows",
        lambda value: value["installation"].__setitem__("rows", []),
        "root_universe_incomplete_or_extra",
    )
    add(
        "expected_under_declared_absent",
        lambda value: value["expected_sources"][0].__setitem__(
            "row", value["installation"]["rows"][1]["declaration_identity"]
        ),
        "expected_source_under_declared_absent",
    )
    add(
        "requiredness_flip",
        lambda value: value["installation"]["rows"][1].__setitem__("required", True),
        "required_declaration_absent",
    )
    add(
        "foreign_absence_fence",
        lambda value: value["installation"]["rows"][1].__setitem__(
            "absence_fence", "f" * 64
        ),
        "stale_or_foreign_absence_fence",
    )
    add(
        "caller_root",
        lambda value: value["caller_roots"].append("/caller/chosen/root"),
        "caller_root_admission",
    )
    add(
        "cross_root_alias",
        lambda value: value["installation"]["rows"][2].__setitem__(
            "locator_commitment", value["repository"]["rows"][0]["locator_commitment"]
        ),
        "cross_root_alias",
    )
    add(
        "a_to_b_to_a",
        lambda value: value.__setitem__(
            "observation_sequence",
            [value["observation_sequence"][0], "f" * 64, value["observation_sequence"][0]],
        ),
        "a_to_b_to_a_or_physical_drift",
    )
    add(
        "final_recheck_drift",
        lambda value: value["final_recheck"].__setitem__(
            "installation_provider_revision", 18
        ),
        "post_seal_finality_drift",
    )
    add(
        "journal_unreachable",
        lambda value: value["protected_primary_journal"].__setitem__(
            "reachable_from_current_head", False
        ),
        "protected_primary_journal_not_current_non_bearer",
    )
    return cases


def run_mutants(contract: Mapping[str, Any], universe: Mapping[str, Any]) -> dict[str, int]:
    accepted = 0
    rejected = 0
    for name, mutate, expected_code in mutant_cases(contract, universe):
        candidate = copy.deepcopy(dict(universe))
        mutate(candidate)
        try:
            validate_root_universe(candidate)
        except ValidationError as error:
            if error.code != expected_code:
                raise ValidationError(
                    "mutant_wrong_refusal",
                    f"{name}: expected {expected_code}, observed {error.code}",
                ) from error
            rejected += 1
        else:
            accepted += 1
    return {"accepted_mutants": accepted, "rejected_mutants": rejected}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, default=CONTRACT)
    parser.add_argument("--root-universe", type=Path, default=ROOT_UNIVERSE)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--mutant-suite", action="store_true")
    args = parser.parse_args()
    try:
        contract = load_json(args.contract)
        universe = load_json(args.root_universe)
        validate_contract(contract)
        validate_root_universe(universe)
        if args.source_root is not None:
            validate_sources(contract, args.source_root.resolve(strict=True))
        mutants = (
            run_mutants(contract, universe)
            if args.mutant_suite
            else {"accepted_mutants": 0, "rejected_mutants": 0}
        )
    except (OSError, ValidationError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    print(
        json.dumps(
            {
                "authority_state": "none",
                "effect_state": "read_only",
                "status": "pass",
                **mutants,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.dont_write_bytecode = True
    raise SystemExit(main())
