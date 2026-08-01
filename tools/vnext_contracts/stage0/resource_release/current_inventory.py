#!/usr/bin/env python3
"""Independent, read-only census of current shipped Resource candidates.

This module deliberately does not import the Stage-0 builder or validator.  It
starts from the nine-family Rust registry, exact declared repository roots and
a closed direct-reader registry.  Historical E204 rows may annotate provenance
but can never create Resource membership.
"""

from __future__ import annotations

import ast
import hashlib
import json
import os
import re
import stat
from dataclasses import dataclass, replace
from enum import Enum
from pathlib import Path, PurePosixPath
from typing import Iterable, Mapping, Sequence


AUTHORITATIVE_REGISTRY_LOCATOR = "src/domain/resource_contracts.rs"
E204_LEDGER_LOCATOR = "contracts/vnext/public/embedded_resources.e204.v1.json"
E204_EXPECTED_COUNT = 204
E204_EXPECTED_DIGEST = "c8fc4c6cd53d81272d19c3b402e99a0ca3f69ebd18cf9464539db1d1ecf85388"

EXPECTED_FAMILY_ROOTS: tuple[tuple[str, str], ...] = (
    ("schemas", "embedded/schemas"),
    ("loop-recipes", "embedded/loop-recipes"),
    ("skills", "embedded/skills"),
    ("harness", "embedded/harness"),
    ("hooks", "embedded/hooks"),
    ("shell", "embedded/shell"),
    ("playbook", "embedded/playbook"),
    ("design", "embedded/design"),
    ("cli-mcp-references", "src/interfaces"),
)

HISTORICAL_ONLY_LOCATORS = frozenset(
    {
        "contracts/vnext/stage0/input-bindings.json",
        "contracts/vnext/public/census_admission_report.v1.json",
        "contracts/vnext/public/direct_consumers.c325.v1.json",
        "contracts/vnext/public/embedded_resources.e204.v1.json",
        "contracts/vnext/public/historical_source_coverage_inputs.v1.json",
        "contracts/vnext/public/physical_census.commitment.v1.json",
        "contracts/vnext/public/physical_census.historical-output.txt",
    }
)
DOCUMENTATION_ONLY_LOCATORS = frozenset({"embedded/vnext/README.md"})
RESOURCE_RELEASE_ROOT = "contracts/vnext/stage0/resource-release"
POST_RELEASE_GENERATED_OUTPUT_ROOTS = (
    "contracts/vnext/stage0/proof-matrix",
    "contracts/vnext/stage0/candidate-root",
    "contracts/vnext/stage2/authority",
    "contracts/vnext/stage3/domain",
    "contracts/vnext/stage4/execution",
    "contracts/vnext/stage5/evidence-gates",
)
POST_RELEASE_GENERATED_OUTPUT_LOCATORS = frozenset(
    {"contracts/vnext/stage0/effect-home/stage2-semantic-consumer-delta-v1.json"}
)
RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS = frozenset(
    {
        "contracts/vnext/stage0/resource-release/c868-successor.v1.cbor",
        "contracts/vnext/stage0/resource-release/c868-successor.v1.json",
        "contracts/vnext/stage0/resource-release/capability-evaluator.v1.json",
        "contracts/vnext/stage0/resource-release/capability-relations.v1.json",
        "contracts/vnext/stage0/resource-release/vendor-reference-pack.v1.json",
        "contracts/vnext/stage0/resource-release/writer-compatibility-successor.v1.cbor",
        "contracts/vnext/stage0/resource-release/writer-compatibility-successor.v1.json",
    }
)
EMBEDDED_RELEASE_ROOT_ADMISSION_LOCATORS: frozenset[str] = frozenset()
EXPLICIT_ROOT_INSTRUCTION_LOCATORS = (
    "embedded/AGENTS.md",
    "embedded/CLAUDE.md",
)
LEGACY_TUI_ROOT = "src/tui"
LEGACY_TUI_ENTRY_LOCATOR = "src/tui/sidecar.ts"
LEGACY_TUI_LAUNCHER_LOCATOR = "src/interfaces/cli/mission_control.rs#run_opentui"
LEGACY_TUI_ROOT_DEPENDENCY_LOCATORS = (
    "bun.lock",
    "package.json",
    "tsconfig.json",
)
LEGACY_TUI_EXPECTED_LOCATORS = frozenset(
    {
        *LEGACY_TUI_ROOT_DEPENDENCY_LOCATORS,
        "src/tui/AGENTS.md",
        "src/tui/CLAUDE.md",
        "src/tui/README.md",
        "src/tui/app/input-dispatch.ts",
        "src/tui/app/interactive-shared.ts",
        "src/tui/app/modal-builders.ts",
        "src/tui/app/preview-contract.ts",
        "src/tui/app/preview-state.ts",
        "src/tui/app/render-check-contract.ts",
        "src/tui/current-snapshot.ts",
        "src/tui/input.ts",
        "src/tui/opentui/ansi.ts",
        "src/tui/opentui/app/interactive.tsx",
        "src/tui/opentui/app/mission-control-app.tsx",
        "src/tui/opentui/app/preview.ts",
        "src/tui/opentui/app/render-check.ts",
        "src/tui/opentui/components/builders.ts",
        "src/tui/opentui/components/mission-control-screen.tsx",
        "src/tui/opentui/index.ts",
        "src/tui/opentui/testing/frame-capture.tsx",
        "src/tui/shared/format.ts",
        "src/tui/shared/header-animation.ts",
        "src/tui/shared/modal-model.ts",
        "src/tui/shared/session-id.ts",
        "src/tui/shared/theme.ts",
        "src/tui/shared/ui-config.ts",
        "src/tui/sidecar.ts",
        "src/tui/state/autopilot-screen.ts",
        "src/tui/state/config-inspector.ts",
        "src/tui/state/environment-projection.ts",
        "src/tui/state/events.ts",
        "src/tui/state/memory-projection.ts",
        "src/tui/state/mission-control-commands.ts",
        "src/tui/state/projection.ts",
        "src/tui/state/reducer.ts",
        "src/tui/state/reply-projection.ts",
        "src/tui/state/screen-types.ts",
        "src/tui/state/snapshot-demand.ts",
        "src/tui/state/snapshot-poll-cache.ts",
        "src/tui/state/task-board.ts",
        "src/tui/state/types.ts",
    }
)
LEGACY_TUI_EXPECTED_RUNTIME_REACHABLE_COUNT = 29
LEGACY_TUI_EXPECTED_TYPESCRIPT_PROJECT_ONLY_COUNT = 1
LEGACY_TUI_EXPECTED_MIGRATION_CENSUS_ONLY_COUNT = 11
EXPECTED_RESOURCE_COUNT = 412
EXPECTED_GENERATED_REFERENCE_PRODUCER_COUNT = 62
EXPECTED_BUNDLE_COUNTS: Mapping["BundleKind", int]
ROOT_INSTRUCTION_DISPOSITIONS: Mapping[str, "ResourceDisposition"]


class InventoryError(ValueError):
    """The declared inventory is incomplete, ambiguous or unsafe to read."""


class SourceKind(str, Enum):
    V1_SHIPPED_RESOURCE = "v1_shipped_resource"
    GENERATED_REFERENCE_PRODUCER = "generated_reference_producer"
    VNEXT_TARGET_RESOURCE = "vnext_target_resource"
    VNEXT_CONTRACT_ARTIFACT = "vnext_contract_artifact"
    HISTORICAL_NON_PROMOTING_EVIDENCE = "historical_non_promoting_evidence"
    DOCUMENTATION_NOT_RESOURCE = "documentation_not_resource"
    GENERATED_PROOF_OUTPUT = "generated_proof_output"

    @property
    def tag(self) -> int:
        return {
            self.V1_SHIPPED_RESOURCE: 1,
            self.GENERATED_REFERENCE_PRODUCER: 2,
            self.VNEXT_TARGET_RESOURCE: 3,
            self.VNEXT_CONTRACT_ARTIFACT: 4,
            self.HISTORICAL_NON_PROMOTING_EVIDENCE: 5,
            self.DOCUMENTATION_NOT_RESOURCE: 6,
            self.GENERATED_PROOF_OUTPUT: 7,
        }[self]


class ContentEncoding(str, Enum):
    UTF8 = "utf-8"
    BINARY = "binary"


class C868ContentEncoding(str, Enum):
    OPAQUE_BYTES = "OpaqueBytes"
    UTF8_TEXT = "Utf8Text"

    @property
    def tag(self) -> int:
        return {self.OPAQUE_BYTES: 1, self.UTF8_TEXT: 2}[self]


class ResourceProvenanceKind(str, Enum):
    FIRST_PARTY = "FirstParty"
    THIRD_PARTY = "ThirdParty"

    @property
    def tag(self) -> int:
        return {self.FIRST_PARTY: 1, self.THIRD_PARTY: 2}[self]


class ResourceReachability(str, Enum):
    NOT_APPLICABLE = "NotApplicable"
    RUNTIME_ENTRY = "RuntimeEntry"
    RUNTIME_REACHABLE = "RuntimeReachable"
    TYPESCRIPT_PROJECT_ONLY = "TypeScriptProjectOnly"
    BUN_PROJECT_INPUT = "BunProjectInput"
    MIGRATION_CENSUS_ONLY = "MigrationCensusOnly"


class FrozenResourceKind(str, Enum):
    EXECUTABLE = "Executable"
    SIGNATURE = "Signature"
    BILL_OF_MATERIALS = "BillOfMaterials"
    AGENT_INSTRUCTION = "AgentInstruction"
    ORCHESTRATION_DEFINITION = "OrchestrationDefinition"
    PUBLIC_CONTRACT = "PublicContract"
    ADAPTER_ARTIFACT = "AdapterArtifact"
    EXTERNAL_PATTERN = "ExternalPattern"
    MIGRATION_ARTIFACT = "MigrationArtifact"
    LICENSE = "License"
    PROVENANCE_MANIFEST = "ProvenanceManifest"

    @property
    def tag(self) -> int:
        return {
            self.EXECUTABLE: 1,
            self.SIGNATURE: 2,
            self.BILL_OF_MATERIALS: 3,
            self.AGENT_INSTRUCTION: 4,
            self.ORCHESTRATION_DEFINITION: 5,
            self.PUBLIC_CONTRACT: 6,
            self.ADAPTER_ARTIFACT: 7,
            self.EXTERNAL_PATTERN: 8,
            self.MIGRATION_ARTIFACT: 9,
            self.LICENSE: 10,
            self.PROVENANCE_MANIFEST: 11,
        }[self]


class ResourceDisposition(str, Enum):
    RETAIN = "Retain"
    REWRITE = "Rewrite"
    REPLACE = "Replace"
    MIGRATION_ONLY = "MigrationOnly"
    REMOVE = "Remove"

    @property
    def tag(self) -> int:
        return {
            self.RETAIN: 1,
            self.REWRITE: 2,
            self.REPLACE: 3,
            self.MIGRATION_ONLY: 4,
            self.REMOVE: 5,
        }[self]


class BundleKind(str, Enum):
    RELEASE = "Release"
    AGENT_BOOTSTRAP = "AgentBootstrap"
    CAPABILITY = "Capability"
    ORCHESTRATION = "Orchestration"
    SHARED_CONTRACT = "SharedContract"
    ADAPTER = "Adapter"
    EXTERNAL_PATTERN = "ExternalPattern"
    MIGRATION = "Migration"

    @property
    def tag(self) -> int:
        return {
            self.RELEASE: 1,
            self.AGENT_BOOTSTRAP: 2,
            self.CAPABILITY: 3,
            self.ORCHESTRATION: 4,
            self.SHARED_CONTRACT: 5,
            self.ADAPTER: 6,
            self.EXTERNAL_PATTERN: 7,
            self.MIGRATION: 8,
        }[self]


class SemanticOwner(str, Enum):
    DISTRIBUTION = "Distribution"
    AGENT_BOOTSTRAP = "AgentBootstrap"
    INTEGRATION = "Integration"
    CAPABILITY = "Capability"
    ORCHESTRATION = "Orchestration"
    SHARED_CONTRACT = "SharedContract"
    ADAPTER = "Adapter"
    DESIGN = "Design"
    MIGRATION = "Migration"
    EXECUTION = "Execution"
    CONTRACT_CLOSURE = "ContractClosure"
    SUBMISSION = "Submission"

    @property
    def frozen_tag(self) -> int:
        tags = {
            self.DISTRIBUTION: 1,
            self.AGENT_BOOTSTRAP: 2,
            self.CAPABILITY: 3,
            self.ORCHESTRATION: 4,
            self.SHARED_CONTRACT: 5,
            self.ADAPTER: 6,
            self.DESIGN: 7,
            self.MIGRATION: 8,
            self.CONTRACT_CLOSURE: 9,
            self.SUBMISSION: 10,
        }
        if self not in tags:
            raise InventoryError(f"{self.value} is a reader owner, not a frozen Resource owner")
        return tags[self]


class DirectConsumerKind(str, Enum):
    BUILD = "Build"
    RUNTIME = "Runtime"
    INSTALL = "Install"
    MIGRATION = "Migration"
    PROOF = "Proof"
    DOCUMENTATION = "Documentation"
    REMOVAL_READER = "RemovalReader"

    @property
    def tag(self) -> int:
        return {
            self.BUILD: 1,
            self.RUNTIME: 2,
            self.INSTALL: 3,
            self.MIGRATION: 4,
            self.PROOF: 5,
            self.DOCUMENTATION: 6,
            self.REMOVAL_READER: 7,
        }[self]


class ReaderEvidenceKind(str, Enum):
    INCLUDE_STR = "include_str"
    INCLUDE_BYTES = "include_bytes"
    INCLUDE_DIR_TYPED_EXTRACTOR = "include_dir_typed_extractor"
    TYPED_PARSER = "typed_parser"
    TYPED_VALIDATOR = "typed_validator"
    INSTALLER = "installer"
    REFERENCE_GENERATOR = "reference_generator"
    TEST_FIXTURE_READER = "test_fixture_reader"
    ARCHIVE_READER = "archive_reader"
    SEALED_MIGRATION_READER = "sealed_migration_reader"
    REMOVAL_PROOF = "removal_proof"
    BUN_LAUNCHER = "bun_launcher"
    TYPESCRIPT_IMPORT = "typescript_import"
    TYPESCRIPT_PROJECT_INPUT = "typescript_project_input"
    BUN_PROJECT_INPUT = "bun_project_input"
    MIGRATION_CENSUS = "migration_census"


class ReaderRole(str, Enum):
    LIVE_READER = "live_reader"
    SEALED_READER = "sealed_reader"
    REMOVAL_PROOF = "removal_proof"


@dataclass(frozen=True, order=True)
class ResourceFamilyDeclaration:
    family_id: str
    source_root: str
    ownership_mode: str
    parser_owner: str
    validator_owner: str
    registry_locator: str


@dataclass(frozen=True, order=True)
class HistoricalEvidence:
    locator: str
    recorded_sha256: str
    family: str
    current_bytes_equal: bool


@dataclass(frozen=True, order=True)
class Provenance:
    source_locator: str
    current_content_sha256: str
    registry_locator: str | None
    kind: ResourceProvenanceKind
    license_locator: str | None
    applicability: str
    historical_evidence: tuple[HistoricalEvidence, ...]
    statement: str


@dataclass(frozen=True)
class PhysicalSource:
    stable_locator: str
    family: str
    source_kind: SourceKind
    ownership_mode: str
    content_bytes: bytes
    content_sha256: str
    encoding: ContentEncoding
    media_type: str


@dataclass(frozen=True)
class ResourceCandidate:
    inventory_ordinal: int
    stable_key: str
    physical_candidate_id: str
    stable_locator: str
    family: str
    source_kind: SourceKind
    semantic_owner: SemanticOwner
    target_bundle_kind: BundleKind
    target_bundle_group: str
    disposition: ResourceDisposition
    content_bytes: bytes
    content_sha256: str
    encoding: ContentEncoding
    media_type: str
    c868_content_encoding: C868ContentEncoding
    frozen_resource_kind: FrozenResourceKind
    source_reachability: ResourceReachability
    provenance: Provenance


@dataclass(frozen=True, order=True)
class DirectReaderEvidence:
    reader_locator: str
    reader_content_sha256: str
    semantic_owner: SemanticOwner
    kind: DirectConsumerKind
    evidence_kind: ReaderEvidenceKind
    role: ReaderRole
    resource_stable_key: str
    resource_candidate_id: str
    resource_locator: str
    disposition: ResourceDisposition
    evidence: str
    explicit_dual_role_contract: bool = False


@dataclass(frozen=True)
class HistoricalE204Ledger:
    locator: str
    content_sha256: str
    expected_digest: str
    rows: tuple[HistoricalEvidence, ...]


@dataclass(frozen=True, order=True)
class ClassifiedExclusion:
    stable_locator: str
    source_kind: SourceKind
    reason: str


@dataclass(frozen=True)
class CurrentInventory:
    families: tuple[ResourceFamilyDeclaration, ...]
    authoritative_sources: tuple[PhysicalSource, ...]
    vnext_sources: tuple[PhysicalSource, ...]
    resources: tuple[ResourceCandidate, ...]
    direct_readers: tuple[DirectReaderEvidence, ...]
    historical_e204: HistoricalE204Ledger
    exclusions: tuple[ClassifiedExclusion, ...]
    unclassified_paths: tuple[str, ...]


@dataclass(frozen=True)
class InventoryValidation:
    family_count: int
    authoritative_source_count: int
    generated_reference_producer_count: int
    vnext_source_count: int
    resource_count: int
    direct_reader_edge_count: int
    historical_e204_count: int
    exclusion_count: int
    generated_output_audit_count: int
    external_pattern_bundle_group_count: int
    legacy_tui_source_count: int
    legacy_tui_runtime_reachable_count: int
    legacy_tui_typescript_project_only_count: int
    legacy_tui_migration_census_only_count: int
    unclassified_paths: tuple[str, ...]
    inventory_sha256: str


@dataclass(frozen=True)
class _ResourcePolicy:
    owner: SemanticOwner
    bundle: BundleKind
    disposition: ResourceDisposition


@dataclass(frozen=True)
class _CandidateGroup:
    family: str
    prefix: str
    source_kind: SourceKind
    policy: _ResourcePolicy
    reader_locator: str
    reader_owner: SemanticOwner
    reader_kind: ReaderEvidenceKind
    reader_role: ReaderRole
    reader_evidence: str


@dataclass(frozen=True)
class _MacroReader:
    reader_locator: str
    resource_root: str
    exact_resource_locator: str | None
    kind: ReaderEvidenceKind
    owner: SemanticOwner
    evidence: str


@dataclass(frozen=True, order=True)
class _TuiImportEdge:
    importer_locator: str
    imported_locator: str
    line: int
    specifier: str


@dataclass(frozen=True)
class _TuiClosure:
    locators: tuple[str, ...]
    runtime_reachable: frozenset[str]
    runtime_parent_edges: Mapping[str, _TuiImportEdge]
    typescript_project_inputs: frozenset[str]
    unresolved_imports: tuple[tuple[str, int, str], ...]


ROOT_INSTRUCTION_DISPOSITIONS = {
    "embedded/AGENTS.md": ResourceDisposition.REWRITE,
    "embedded/CLAUDE.md": ResourceDisposition.REMOVE,
}
EXPECTED_BUNDLE_COUNTS = {
    BundleKind.RELEASE: 0,
    BundleKind.AGENT_BOOTSTRAP: 2,
    BundleKind.CAPABILITY: 34,
    BundleKind.ORCHESTRATION: 13,
    BundleKind.SHARED_CONTRACT: 90,
    BundleKind.ADAPTER: 7,
    BundleKind.EXTERNAL_PATTERN: 82,
    BundleKind.MIGRATION: 184,
}


FAMILY_POLICIES: Mapping[str, _ResourcePolicy] = {
    "schemas": _ResourcePolicy(
        SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REPLACE
    ),
    "loop-recipes": _ResourcePolicy(
        SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.MIGRATION_ONLY
    ),
    "skills": _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REWRITE),
    "harness": _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.MIGRATION_ONLY),
    "hooks": _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REPLACE),
    "shell": _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REPLACE),
    "playbook": _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REPLACE),
    "design": _ResourcePolicy(SemanticOwner.DESIGN, BundleKind.EXTERNAL_PATTERN, ResourceDisposition.REPLACE),
    "root-agent-instruction": _ResourcePolicy(
        SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REWRITE
    ),
    "legacy-tui": _ResourcePolicy(
        SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.REPLACE
    ),
}


CANDIDATE_GROUPS: tuple[_CandidateGroup, ...] = (
    _CandidateGroup(
        "vnext-adapter",
        "embedded/vnext/adapter",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.ADAPTER, BundleKind.ADAPTER, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/public/validate_public_contracts.py#validate_sources",
        SemanticOwner.INTEGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact adapter Resource source selected by the public contract closure",
    ),
    _CandidateGroup(
        "vnext-bootstrap",
        "embedded/vnext/bootstrap",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.AGENT_BOOTSTRAP, BundleKind.AGENT_BOOTSTRAP, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/resource_release/validate.py#validate_current_surface",
        SemanticOwner.INTEGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact current AgentBootstrap surface row",
    ),
    _CandidateGroup(
        "vnext-connectors",
        "embedded/vnext/connectors",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.ADAPTER, BundleKind.ADAPTER, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage10/validate.py#main",
        SemanticOwner.INTEGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Stage-10 connector descriptor closure",
    ),
    _CandidateGroup(
        "vnext-hosts",
        "embedded/vnext/hosts",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.ADAPTER, BundleKind.ADAPTER, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage10/validate.py#main",
        SemanticOwner.INTEGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Stage-10 host descriptor closure",
    ),
    _CandidateGroup(
        "vnext-patterns",
        "embedded/vnext/patterns",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.DESIGN, BundleKind.EXTERNAL_PATTERN, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage10/validate.py#main",
        SemanticOwner.INTEGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Stage-10 external pattern closure",
    ),
    _CandidateGroup(
        "vnext-schemas",
        "embedded/vnext/schemas",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(
            SemanticOwner.SHARED_CONTRACT,
            BundleKind.SHARED_CONTRACT,
            ResourceDisposition.RETAIN,
        ),
        "tools/vnext_contracts/stage10/validate.py#main",
        SemanticOwner.INTEGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Stage-10 adapter and host schema closure",
    ),
    _CandidateGroup(
        "vnext-capability",
        "embedded/vnext/capability",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.CAPABILITY, BundleKind.CAPABILITY, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/public/validate_public_contracts.py#validate_sources",
        SemanticOwner.CAPABILITY,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates instruction, context-budget and capability source Resources by exact declared root",
    ),
    _CandidateGroup(
        "vnext-orchestration",
        "embedded/vnext/orchestration",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.ORCHESTRATION, BundleKind.ORCHESTRATION, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/public/validate_public_contracts.py#validate_sources",
        SemanticOwner.ORCHESTRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates exact Recipe, profile and catalog Resource sources",
    ),
    _CandidateGroup(
        "vnext-release-policy",
        "embedded/vnext/release",
        SourceKind.VNEXT_TARGET_RESOURCE,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/resource_release/validate.py#validate_preidentity_artifacts",
        SemanticOwner.DISTRIBUTION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Release policy source",
    ),
    _CandidateGroup(
        "vnext-catalog-evidence",
        "contracts/vnext/catalogs/evidence",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.MIGRATION_ONLY),
        "tools/vnext_contracts/catalogs/verify_predecessors.py#main",
        SemanticOwner.MIGRATION,
        ReaderEvidenceKind.SEALED_MIGRATION_READER,
        ReaderRole.SEALED_READER,
        "sealed predecessor evidence is read only by the exact catalog verifier",
    ),
    _CandidateGroup(
        "vnext-catalog-predecessor",
        "contracts/vnext/catalogs/predecessor",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.MIGRATION_ONLY),
        "tools/vnext_contracts/catalogs/verify_predecessors.py#main",
        SemanticOwner.MIGRATION,
        ReaderEvidenceKind.SEALED_MIGRATION_READER,
        ReaderRole.SEALED_READER,
        "sealed predecessor reproduction is read only by the exact catalog verifier",
    ),
    _CandidateGroup(
        "vnext-catalog-generated",
        "contracts/vnext/catalogs/generated",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/catalogs/validate.py#main",
        SemanticOwner.SHARED_CONTRACT,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact generated catalog artifact closure",
    ),
    _CandidateGroup(
        "vnext-public",
        "contracts/vnext/public",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/public/validate_public_contracts.py#validate_data",
        SemanticOwner.SHARED_CONTRACT,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates exact public contract artifacts; historical-only locators are excluded before binding",
    ),
    _CandidateGroup(
        "vnext-public-identity",
        "contracts/vnext/stage0/public-identity",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/public_identity/validate.py#main",
        SemanticOwner.SHARED_CONTRACT,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates exact public identity closure artifacts",
    ),
    _CandidateGroup(
        "vnext-decision-closure",
        "contracts/vnext/stage0/decision-closure",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/decision_closure/validate.py#main",
        SemanticOwner.CONTRACT_CLOSURE,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates exact Decision and authority closure artifacts",
    ),
    _CandidateGroup(
        "vnext-submission-claim",
        "contracts/vnext/stage0/submission-claim",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/submission_claim/build.py#main",
        SemanticOwner.SUBMISSION,
        ReaderEvidenceKind.TYPED_PARSER,
        ReaderRole.LIVE_READER,
        "reads the exact SubmissionClaimSet artifact closure",
    ),
    _CandidateGroup(
        "vnext-dispatch-cutover",
        "contracts/vnext/stage0/dispatch-cutover",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.MIGRATION, BundleKind.MIGRATION, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/dispatch_cutover/validate.py#main",
        SemanticOwner.MIGRATION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact dispatch and migration-cutover artifacts",
    ),
    _CandidateGroup(
        "vnext-effect-home",
        "contracts/vnext/stage0/effect-home",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/effect_home/validate.py#main",
        SemanticOwner.EXECUTION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Effect Home, control and withdrawal artifact closure",
    ),
    _CandidateGroup(
        "vnext-resource-release",
        "contracts/vnext/stage0/resource-release",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/resource_release/validate.py#validate_preidentity_artifacts",
        SemanticOwner.DISTRIBUTION,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact seven admitted preidentity Resource inputs before Resource construction",
    ),
    _CandidateGroup(
        "vnext-stage0-input-verification-contract",
        "contracts/vnext/stage0/input-verification-contract.v1.json",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(SemanticOwner.SHARED_CONTRACT, BundleKind.SHARED_CONTRACT, ResourceDisposition.RETAIN),
        "tools/vnext_contracts/stage0/verify_input_bindings.py#main",
        SemanticOwner.CONTRACT_CLOSURE,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "defines the non-promoting Stage-0 input verification boundary without packaging any approval instance",
    ),
    _CandidateGroup(
        "vnext-final-chain",
        "contracts/vnext/final-chain",
        SourceKind.VNEXT_CONTRACT_ARTIFACT,
        _ResourcePolicy(
            SemanticOwner.SHARED_CONTRACT,
            BundleKind.SHARED_CONTRACT,
            ResourceDisposition.RETAIN,
        ),
        "tools/vnext_contracts/stage12/validate.py#main",
        SemanticOwner.CONTRACT_CLOSURE,
        ReaderEvidenceKind.TYPED_VALIDATOR,
        ReaderRole.LIVE_READER,
        "validates the exact Stage-12 final-chain schema and proof-input closure",
    ),
)


MACRO_READERS: tuple[_MacroReader, ...] = (
    _MacroReader(
        "src/domain/schema_contracts/catalog.rs",
        "embedded/schemas",
        None,
        ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR,
        SemanticOwner.DISTRIBUTION,
        "include_dir embeds and typed-parses every schema descriptor and fixture",
    ),
    _MacroReader(
        "src/domain/loop_recipes.rs",
        "embedded/loop-recipes",
        None,
        ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR,
        SemanticOwner.ORCHESTRATION,
        "include_dir embeds and typed-parses the shipped Recipe tree",
    ),
    _MacroReader(
        "src/domain/skills/catalog.rs",
        "embedded/skills",
        None,
        ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR,
        SemanticOwner.CAPABILITY,
        "include_dir embeds every exact Skill tree byte",
    ),
    _MacroReader(
        "src/domain/harness/templates.rs",
        "embedded/harness",
        "embedded/harness/HARNESS.md",
        ReaderEvidenceKind.INCLUDE_STR,
        SemanticOwner.MIGRATION,
        "include_str embeds the exact Harness source",
    ),
    _MacroReader(
        "src/domain/harness/templates.rs",
        "embedded/harness",
        "embedded/harness/RECOVERY.md",
        ReaderEvidenceKind.INCLUDE_STR,
        SemanticOwner.MIGRATION,
        "include_str embeds the exact Recovery source",
    ),
    _MacroReader(
        "src/domain/extraction/hook_script.rs",
        "embedded/hooks",
        "embedded/hooks/record.sh",
        ReaderEvidenceKind.INCLUDE_STR,
        SemanticOwner.DISTRIBUTION,
        "include_str embeds the exact installed hook script",
    ),
    _MacroReader(
        "src/domain/run/event.rs",
        "embedded/hooks",
        "embedded/hooks/events.yaml",
        ReaderEvidenceKind.INCLUDE_STR,
        SemanticOwner.DISTRIBUTION,
        "include_str embeds the exact hook event configuration",
    ),
    _MacroReader(
        "src/interfaces/shell/mod.rs",
        "embedded/shell",
        "embedded/shell/posix.sh",
        ReaderEvidenceKind.INCLUDE_STR,
        SemanticOwner.DISTRIBUTION,
        "include_str embeds the exact POSIX shell integration",
    ),
    _MacroReader(
        "src/interfaces/shell/mod.rs",
        "embedded/shell",
        "embedded/shell/fish.fish",
        ReaderEvidenceKind.INCLUDE_STR,
        SemanticOwner.DISTRIBUTION,
        "include_str embeds the exact Fish shell integration",
    ),
    _MacroReader(
        "src/domain/playbook.rs",
        "embedded/playbook",
        None,
        ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR,
        SemanticOwner.DISTRIBUTION,
        "include_dir embeds every playbook Resource and the typed server enumerates it",
    ),
    _MacroReader(
        "src/domain/design/legacy.rs",
        "embedded/design/styles",
        None,
        ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR,
        SemanticOwner.DISTRIBUTION,
        "include_dir embeds exact first-party Design Resources",
    ),
    _MacroReader(
        "src/domain/design/legacy.rs",
        "embedded/design/vendor/awesome-design-md",
        None,
        ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR,
        SemanticOwner.DISTRIBUTION,
        "include_dir embeds the exact licensed vendor reference pack",
    ),
)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _repo_root(value: Path | str) -> Path:
    root = Path(value).resolve()
    if not root.is_dir():
        raise InventoryError(f"repository root is not a directory: {root}")
    return root


def _normal_locator(locator: str) -> str:
    if not locator or "\\" in locator:
        raise InventoryError(f"invalid repository locator: {locator!r}")
    path = PurePosixPath(locator)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise InventoryError(f"non-canonical repository locator: {locator!r}")
    canonical = path.as_posix()
    if canonical != locator:
        raise InventoryError(f"non-canonical repository locator: {locator!r}")
    return canonical


def _read_regular_file(root: Path, locator: str) -> bytes:
    locator = _normal_locator(locator)
    path = root.joinpath(*PurePosixPath(locator).parts)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise InventoryError(f"declared file is missing: {locator}") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise InventoryError(f"declared file is a symlink: {locator}")
    if not stat.S_ISREG(metadata.st_mode):
        raise InventoryError(f"declared file is not regular: {locator}")
    return path.read_bytes()


def _enumerate_regular_files(root: Path, declared_root: str) -> tuple[str, ...]:
    declared_root = _normal_locator(declared_root)
    start = root.joinpath(*PurePosixPath(declared_root).parts)
    try:
        metadata = start.lstat()
    except FileNotFoundError as error:
        raise InventoryError(f"declared root is missing: {declared_root}") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise InventoryError(f"declared root is a symlink: {declared_root}")
    if stat.S_ISREG(metadata.st_mode):
        return (declared_root,)
    if not stat.S_ISDIR(metadata.st_mode):
        raise InventoryError(f"declared root is not a directory or regular file: {declared_root}")

    result: list[str] = []

    def visit(directory: Path) -> None:
        with os.scandir(directory) as entries:
            ordered = sorted(entries, key=lambda entry: entry.name)
        for entry in ordered:
            relative = Path(entry.path).relative_to(root).as_posix()
            if entry.is_symlink():
                raise InventoryError(f"symlink beneath declared root: {relative}")
            if entry.is_dir(follow_symlinks=False):
                visit(Path(entry.path))
            elif entry.is_file(follow_symlinks=False):
                result.append(_normal_locator(relative))
            else:
                raise InventoryError(f"non-regular node beneath declared root: {relative}")

    visit(start)
    return tuple(sorted(result))


def _media(locator: str, data: bytes) -> tuple[ContentEncoding, str]:
    suffix = PurePosixPath(locator).suffix.lower()
    if suffix == ".cbor":
        return ContentEncoding.BINARY, "application/cbor"
    media = {
        ".json": "application/json",
        ".jsonl": "application/x-ndjson",
        ".yaml": "application/yaml",
        ".yml": "application/yaml",
        ".md": "text/markdown",
        ".rs": "text/x-rust",
        ".ts": "text/typescript",
        ".tsx": "text/typescript-jsx",
        ".py": "text/x-python",
        ".rb": "text/x-ruby",
        ".sh": "text/x-shellscript",
        ".fish": "text/x-fish",
        ".txt": "text/plain",
        ".lock": "application/vnd.bun.lock",
    }.get(suffix, "application/octet-stream")
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return ContentEncoding.BINARY, media
    return ContentEncoding.UTF8, media


def _extract_balanced_blocks(source: str, marker: str) -> tuple[str, ...]:
    blocks: list[str] = []
    cursor = 0
    while True:
        start = source.find(marker, cursor)
        if start < 0:
            return tuple(blocks)
        brace = source.find("{", start + len(marker))
        if brace < 0:
            raise InventoryError(f"unclosed {marker.strip()} declaration")
        depth = 0
        in_string = False
        escaped = False
        for index in range(brace, len(source)):
            char = source[index]
            if in_string:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    in_string = False
                continue
            if char == '"':
                in_string = True
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    blocks.append(source[start : index + 1])
                    cursor = index + 1
                    break
        else:
            raise InventoryError(f"unclosed {marker.strip()} declaration")


def _field_string(block: str, field: str) -> str:
    match = re.search(rf"\b{re.escape(field)}\s*:\s*\"([^\"]+)\"", block)
    if match is None:
        raise InventoryError(f"ResourceFamily block is missing literal {field}")
    return match.group(1)


def _kernel_owner(block: str, field: str) -> str:
    match = re.search(
        rf"\b{re.escape(field)}\s*:\s*SemanticKernel\s*\{{(?P<body>.*?)\n\s*\}}",
        block,
        flags=re.DOTALL,
    )
    if match is None:
        raise InventoryError(f"ResourceFamily block is missing {field} SemanticKernel")
    return _field_string(match.group("body"), "owner")


def parse_authoritative_families(repo_root: Path | str) -> tuple[ResourceFamilyDeclaration, ...]:
    """Parse and strictly validate the independent nine-family Rust registry."""

    root = _repo_root(repo_root)
    source = _read_regular_file(root, AUTHORITATIVE_REGISTRY_LOCATOR).decode("utf-8")
    constant_at = source.find("const SHIPPED_RESOURCE_FAMILIES")
    if constant_at < 0:
        raise InventoryError("SHIPPED_RESOURCE_FAMILIES constant is missing")
    blocks = _extract_balanced_blocks(source[constant_at:], "ResourceFamily ")
    rows: list[ResourceFamilyDeclaration] = []
    for block in blocks:
        family_id = _field_string(block, "id")
        source_root = _normal_locator(_field_string(block, "source_path"))
        ownership = re.search(r"\bownership_mode\s*:\s*ResourceOwnershipMode::([A-Za-z0-9_]+)", block)
        if ownership is None:
            raise InventoryError(f"{family_id}: ownership_mode is not a literal enum")
        rows.append(
            ResourceFamilyDeclaration(
                family_id=family_id,
                source_root=source_root,
                ownership_mode=ownership.group(1),
                parser_owner=_normal_locator(_kernel_owner(block, "parser")),
                validator_owner=_normal_locator(_kernel_owner(block, "validator")),
                registry_locator=AUTHORITATIVE_REGISTRY_LOCATOR,
            )
        )
    actual = tuple((row.family_id, row.source_root) for row in rows)
    if actual != EXPECTED_FAMILY_ROOTS:
        raise InventoryError(
            "authoritative family registry changed; expected exact nine-family closure "
            f"{EXPECTED_FAMILY_ROOTS!r}, got {actual!r}"
        )
    if len({row.family_id for row in rows}) != len(rows):
        raise InventoryError("duplicate authoritative family id")
    if len({row.source_root for row in rows}) != len(rows):
        raise InventoryError("duplicate authoritative family root")
    for left in rows:
        for right in rows:
            if left is right:
                continue
            if right.source_root.startswith(f"{left.source_root}/"):
                raise InventoryError(f"overlapping authoritative roots: {left.source_root} and {right.source_root}")
    return tuple(rows)


def _physical_source(
    root: Path,
    locator: str,
    family: str,
    source_kind: SourceKind,
    ownership_mode: str,
) -> PhysicalSource:
    data = _read_regular_file(root, locator)
    encoding, media_type = _media(locator, data)
    return PhysicalSource(
        stable_locator=locator,
        family=family,
        source_kind=source_kind,
        ownership_mode=ownership_mode,
        content_bytes=data,
        content_sha256=_sha256(data),
        encoding=encoding,
        media_type=media_type,
    )


def enumerate_authoritative_family_sources(
    repo_root: Path | str,
    families: Sequence[ResourceFamilyDeclaration] | None = None,
) -> tuple[PhysicalSource, ...]:
    """Enumerate only exact regular paths beneath registry-declared roots."""

    root = _repo_root(repo_root)
    declarations = tuple(families) if families is not None else parse_authoritative_families(root)
    known = {family_id for family_id, _ in EXPECTED_FAMILY_ROOTS}
    if {row.family_id for row in declarations} != known:
        raise InventoryError("unknown or missing family in authoritative source enumeration")
    result: list[PhysicalSource] = []
    seen: set[str] = set()
    for family in declarations:
        kind = (
            SourceKind.GENERATED_REFERENCE_PRODUCER
            if family.ownership_mode == "GeneratedReferenceOutput"
            else SourceKind.V1_SHIPPED_RESOURCE
        )
        for locator in _enumerate_regular_files(root, family.source_root):
            if locator in seen:
                raise InventoryError(f"duplicate authoritative source path: {locator}")
            seen.add(locator)
            result.append(_physical_source(root, locator, family.family_id, kind, family.ownership_mode))
    for locator in EXPLICIT_ROOT_INSTRUCTION_LOCATORS:
        if locator in seen:
            raise InventoryError(f"explicit root instruction overlaps a declared family root: {locator}")
        seen.add(locator)
        result.append(
            _physical_source(
                root,
                locator,
                "root-agent-instruction",
                SourceKind.V1_SHIPPED_RESOURCE,
                "ExplicitE204RootInstruction",
            )
        )
    tui_locators = {
        *_enumerate_regular_files(root, LEGACY_TUI_ROOT),
        *LEGACY_TUI_ROOT_DEPENDENCY_LOCATORS,
    }
    if tui_locators != LEGACY_TUI_EXPECTED_LOCATORS:
        raise InventoryError(
            "legacy TUI closure changed: "
            f"missing={sorted(LEGACY_TUI_EXPECTED_LOCATORS - tui_locators)}, "
            f"extra={sorted(tui_locators - LEGACY_TUI_EXPECTED_LOCATORS)}"
        )
    for locator in sorted(tui_locators):
        if locator in seen:
            raise InventoryError(f"legacy TUI input overlaps another authoritative source: {locator}")
        seen.add(locator)
        result.append(
            _physical_source(
                root,
                locator,
                "legacy-tui",
                SourceKind.V1_SHIPPED_RESOURCE,
                "ExplicitLegacyTuiClosure",
            )
        )
    return tuple(sorted(result, key=lambda row: row.stable_locator))


def _manifest_digest(rows: Mapping[str, str]) -> str:
    stream = "".join(f"{digest}  {path}\n" for path, digest in sorted(rows.items()))
    return _sha256(stream.encode("utf-8"))


def load_historical_e204(repo_root: Path | str) -> HistoricalE204Ledger:
    """Validate E204 as sealed, non-promoting evidence without admitting rows."""

    root = _repo_root(repo_root)
    raw = _read_regular_file(root, E204_LEDGER_LOCATOR)
    try:
        document = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise InventoryError("E204 ledger is not valid UTF-8 JSON") from error
    required = {
        "schema": "maestro.vnext.embedded-resource-evidence-ledger.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "runtime_registration": False,
        "evidence_classification": "non_promoting_historical_coverage",
        "current_source_equality_claimed": False,
        "expected_count": E204_EXPECTED_COUNT,
        "expected_digest": E204_EXPECTED_DIGEST,
    }
    for key, expected in required.items():
        if document.get(key) != expected:
            raise InventoryError(f"E204 ledger field {key!r} changed")
    raw_rows = document.get("rows")
    if not isinstance(raw_rows, list) or len(raw_rows) != E204_EXPECTED_COUNT:
        raise InventoryError("E204 ledger does not contain exactly 204 rows")
    paths: dict[str, str] = {}
    evidence: list[HistoricalEvidence] = []
    for raw_row in raw_rows:
        if not isinstance(raw_row, dict):
            raise InventoryError("E204 row is not an object")
        locator = _normal_locator(str(raw_row.get("path", "")))
        digest = str(raw_row.get("sha256", ""))
        family = str(raw_row.get("family", ""))
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise InventoryError(f"E204 row has invalid sha256: {locator}")
        if locator in paths:
            raise InventoryError(f"duplicate E204 path: {locator}")
        paths[locator] = digest
        path = root.joinpath(*PurePosixPath(locator).parts)
        current_equal = False
        if path.exists() and not path.is_symlink() and path.is_file():
            current_equal = _sha256(path.read_bytes()) == digest
        evidence.append(HistoricalEvidence(locator, digest, family, current_equal))
    if tuple(paths) != tuple(sorted(paths)):
        raise InventoryError("E204 rows are not deterministically sorted by exact path")
    if _manifest_digest(paths) != E204_EXPECTED_DIGEST:
        raise InventoryError("E204 exact manifest digest changed")
    return HistoricalE204Ledger(
        locator=E204_LEDGER_LOCATOR,
        content_sha256=_sha256(raw),
        expected_digest=E204_EXPECTED_DIGEST,
        rows=tuple(evidence),
    )


def _candidate_group(locator: str) -> _CandidateGroup | None:
    matches = [
        group
        for group in CANDIDATE_GROUPS
        if locator == group.prefix or locator.startswith(f"{group.prefix}/")
    ]
    if len(matches) > 1:
        raise InventoryError(f"vNext path has duplicate group classification: {locator}")
    return matches[0] if matches else None


def _is_post_release_generated_output(locator: str) -> bool:
    """Classify every downstream or output-owned file as noncanonical."""

    return (
        locator in POST_RELEASE_GENERATED_OUTPUT_LOCATORS
        or (
            locator.startswith(f"{RESOURCE_RELEASE_ROOT}/")
            and locator not in RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS
        )
    ) or any(locator.startswith(f"{root}/") for root in POST_RELEASE_GENERATED_OUTPUT_ROOTS)


def enumerate_vnext_candidates(repo_root: Path | str) -> tuple[PhysicalSource, ...]:
    """Enumerate the two exact vNext trees; classifications are applied later."""

    root = _repo_root(repo_root)
    locators = sorted(
        {
            *_enumerate_regular_files(root, "embedded/vnext"),
            *_enumerate_regular_files(root, "contracts/vnext"),
        }
    )
    result: list[PhysicalSource] = []
    for locator in locators:
        if _is_post_release_generated_output(locator):
            result.append(
                _physical_source(
                    root,
                    locator,
                    "noncanonical-post-release-generated-output",
                    SourceKind.GENERATED_PROOF_OUTPUT,
                    "GeneratedOutputNotResource",
                )
            )
            continue
        if locator in HISTORICAL_ONLY_LOCATORS:
            result.append(
                _physical_source(
                    root,
                    locator,
                    "historical-evidence",
                    SourceKind.HISTORICAL_NON_PROMOTING_EVIDENCE,
                    "SealedEvidenceOnly",
                )
            )
            continue
        if locator in DOCUMENTATION_ONLY_LOCATORS:
            result.append(
                _physical_source(
                    root,
                    locator,
                    "vnext-documentation",
                    SourceKind.DOCUMENTATION_NOT_RESOURCE,
                    "DocumentationOnly",
                )
            )
            continue
        group = _candidate_group(locator)
        if group is None:
            result.append(
                _physical_source(root, locator, "unclassified", SourceKind.VNEXT_CONTRACT_ARTIFACT, "Unclassified")
            )
            continue
        result.append(_physical_source(root, locator, group.family, group.source_kind, "CandidateResource"))
    return tuple(result)


def _load_skill_dispositions(root: Path, sources: Sequence[PhysicalSource]) -> Mapping[str, ResourceDisposition]:
    locator = "contracts/vnext/public/v1_skill_ledger.v1.json"
    document = json.loads(_read_regular_file(root, locator))
    rows = document.get("rows")
    if not isinstance(rows, list):
        raise InventoryError("v1 Skill ledger rows are missing")
    mapping: dict[str, ResourceDisposition] = {}
    for row in rows:
        relative = _normal_locator(str(row.get("path", "")))
        full = f"embedded/skills/{relative}"
        try:
            disposition = ResourceDisposition(str(row.get("disposition", "")))
        except ValueError as error:
            raise InventoryError(f"unknown Skill disposition for {full}") from error
        if full in mapping:
            raise InventoryError(f"duplicate Skill ledger path: {full}")
        mapping[full] = disposition
    exact = {source.stable_locator for source in sources if source.family == "skills"}
    if set(mapping) != exact:
        raise InventoryError(
            f"v1 Skill ledger is not exact: missing={sorted(exact - set(mapping))}, extra={sorted(set(mapping) - exact)}"
        )
    totals = {disposition: list(mapping.values()).count(disposition) for disposition in ResourceDisposition}
    if (
        totals[ResourceDisposition.REWRITE],
        totals[ResourceDisposition.REPLACE],
        totals[ResourceDisposition.MIGRATION_ONLY],
    ) != (19, 9, 7):
        raise InventoryError("v1 Skill disposition closure is not exactly 19 Rewrite / 9 Replace / 7 MigrationOnly")
    return mapping


def _load_membership_overrides(root: Path) -> Mapping[str, tuple[str, _ResourcePolicy]]:
    document = json.loads(_read_regular_file(root, "contracts/vnext/public/bundle_membership_inputs.v1.json"))
    rows = document.get("membership_inputs")
    if not isinstance(rows, list):
        raise InventoryError("bundle membership inputs are missing")
    result: dict[str, tuple[str, _ResourcePolicy]] = {}
    disposition_map = {
        "retain": ResourceDisposition.RETAIN,
        "rewrite": ResourceDisposition.REWRITE,
        "replace": ResourceDisposition.REPLACE,
        "migration-only": ResourceDisposition.MIGRATION_ONLY,
        "remove": ResourceDisposition.REMOVE,
    }
    for row in rows:
        locator = _normal_locator(str(row.get("source_locator", "")))
        if locator in result:
            raise InventoryError(f"duplicate bundle membership source: {locator}")
        try:
            bundle = BundleKind(str(row["required_bundle_kind"]))
            declared_owner = SemanticOwner(str(row["semantic_owner"]))
            owner = (
                {
                    BundleKind.AGENT_BOOTSTRAP: SemanticOwner.AGENT_BOOTSTRAP,
                    BundleKind.ADAPTER: SemanticOwner.ADAPTER,
                    BundleKind.SHARED_CONTRACT: SemanticOwner.SHARED_CONTRACT,
                }.get(bundle, declared_owner)
                if declared_owner == SemanticOwner.INTEGRATION
                else declared_owner
            )
            disposition = disposition_map[str(row["disposition"])]
        except (KeyError, ValueError) as error:
            raise InventoryError(f"invalid bundle membership input for {locator}") from error
        result[locator] = (str(row["resource_key"]), _ResourcePolicy(owner, bundle, disposition))
    return result


def _collapse_posix_path(path: PurePosixPath) -> str:
    parts: list[str] = []
    for part in path.parts:
        if part in {"", "."}:
            continue
        if part == "..":
            if not parts:
                raise InventoryError(f"TypeScript import escapes the repository root: {path}")
            parts.pop()
        else:
            parts.append(part)
    return _normal_locator("/".join(parts))


def _resolve_tui_import(
    importer: str,
    specifier: str,
    tui_typescript_locators: frozenset[str],
) -> tuple[bool, str | None]:
    if specifier.startswith("@/tui/"):
        base = f"src/{specifier[2:]}"
    elif specifier.startswith("."):
        base = _collapse_posix_path(PurePosixPath(importer).parent / specifier)
    else:
        return False, None
    if not (base == LEGACY_TUI_ROOT or base.startswith(f"{LEGACY_TUI_ROOT}/")):
        return False, None

    suffix = PurePosixPath(base).suffix.lower()
    candidates: tuple[str, ...]
    if suffix in {".js", ".jsx", ".mjs", ".cjs"}:
        stem = base[: -len(suffix)]
        candidates = (f"{stem}.ts", f"{stem}.tsx")
    elif suffix in {".ts", ".tsx"}:
        candidates = (base,)
    else:
        candidates = (
            base,
            f"{base}.ts",
            f"{base}.tsx",
            f"{base}/index.ts",
            f"{base}/index.tsx",
        )
    matches = tuple(candidate for candidate in candidates if candidate in tui_typescript_locators)
    if len(matches) > 1:
        raise InventoryError(
            f"ambiguous TypeScript import from {importer}: {specifier} -> {matches}"
        )
    return True, matches[0] if matches else None


def _typescript_project_inputs(root: Path, tui_typescript_locators: frozenset[str]) -> frozenset[str]:
    try:
        document = json.loads(_read_regular_file(root, "tsconfig.json"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise InventoryError("legacy TUI tsconfig.json is not valid UTF-8 JSON") from error
    compiler_options = document.get("compilerOptions")
    if not isinstance(compiler_options, dict) or compiler_options.get("paths", {}).get("@/*") != ["./src/*"]:
        raise InventoryError("legacy TUI tsconfig no longer binds the @/* repository alias")
    includes = document.get("include")
    if not isinstance(includes, list) or not all(isinstance(pattern, str) for pattern in includes):
        raise InventoryError("legacy TUI tsconfig has no exact include closure")

    matched: set[str] = set()
    for pattern in includes:
        if not pattern or pattern.startswith("/") or "\\" in pattern or ".." in PurePosixPath(pattern).parts:
            raise InventoryError(f"unsafe tsconfig include pattern: {pattern!r}")
        for path in root.glob(pattern):
            try:
                relative = path.relative_to(root).as_posix()
            except ValueError as error:
                raise InventoryError(f"tsconfig include escapes the repository: {path}") from error
            if relative not in tui_typescript_locators:
                continue
            _read_regular_file(root, relative)
            matched.add(relative)
    return frozenset(matched)


def _legacy_tui_closure(root: Path) -> _TuiClosure:
    locators = tuple(sorted(LEGACY_TUI_EXPECTED_LOCATORS))
    actual = {
        *_enumerate_regular_files(root, LEGACY_TUI_ROOT),
        *LEGACY_TUI_ROOT_DEPENDENCY_LOCATORS,
    }
    if actual != LEGACY_TUI_EXPECTED_LOCATORS:
        raise InventoryError(
            "legacy TUI closure changed: "
            f"missing={sorted(LEGACY_TUI_EXPECTED_LOCATORS - actual)}, "
            f"extra={sorted(actual - LEGACY_TUI_EXPECTED_LOCATORS)}"
        )
    tui_typescript_locators = frozenset(
        locator for locator in actual if PurePosixPath(locator).suffix.lower() in {".ts", ".tsx"}
    )
    if LEGACY_TUI_ENTRY_LOCATOR not in tui_typescript_locators:
        raise InventoryError("legacy TUI Bun entry is not in the exact TypeScript source closure")

    package = json.loads(_read_regular_file(root, "package.json"))
    scripts = package.get("scripts")
    if not isinstance(scripts, dict) or scripts.get("tui") != "bun run src/tui/sidecar.ts":
        raise InventoryError("package.json no longer binds the exact legacy TUI Bun entry")
    if scripts.get("tui:check") != "tsc --noEmit":
        raise InventoryError("package.json no longer binds the TypeScript project check")
    dependencies = package.get("dependencies")
    if not isinstance(dependencies, dict):
        raise InventoryError("package.json dependencies are missing")
    lock_text = _read_regular_file(root, "bun.lock").decode("utf-8")
    for dependency in ("@opentui/core", "@opentui/react", "cli-spinners", "react"):
        version = dependencies.get(dependency)
        if not isinstance(version, str) or f'"{dependency}": "{version}"' not in lock_text:
            raise InventoryError(f"bun.lock does not bind package.json dependency {dependency}")

    import_pattern = re.compile(
        r'''(?mx)(?:\bfrom\s*|\bimport\s*\(\s*|^\s*import\s*)["'](?P<specifier>[^"']+)["']'''
    )
    edges_by_importer: dict[str, list[_TuiImportEdge]] = {
        locator: [] for locator in tui_typescript_locators
    }
    unresolved: list[tuple[str, int, str]] = []
    for importer in sorted(tui_typescript_locators):
        text = _read_regular_file(root, importer).decode("utf-8")
        for match in import_pattern.finditer(text):
            specifier = match.group("specifier")
            is_tui_import, imported = _resolve_tui_import(
                importer, specifier, tui_typescript_locators
            )
            if not is_tui_import:
                continue
            line = text.count("\n", 0, match.start("specifier")) + 1
            if imported is None:
                unresolved.append((importer, line, specifier))
                continue
            edges_by_importer[importer].append(
                _TuiImportEdge(importer, imported, line, specifier)
            )

    reachable = {LEGACY_TUI_ENTRY_LOCATOR}
    parent_edges: dict[str, _TuiImportEdge] = {}
    queue = [LEGACY_TUI_ENTRY_LOCATOR]
    while queue:
        importer = queue.pop(0)
        for edge in sorted(edges_by_importer[importer]):
            if edge.imported_locator in reachable:
                continue
            reachable.add(edge.imported_locator)
            parent_edges[edge.imported_locator] = edge
            queue.append(edge.imported_locator)
    reachable_unresolved = [row for row in unresolved if row[0] in reachable]
    if reachable_unresolved:
        raise InventoryError(f"live TUI import closure has unresolved internal imports: {reachable_unresolved}")

    return _TuiClosure(
        locators=locators,
        runtime_reachable=frozenset(reachable),
        runtime_parent_edges=parent_edges,
        typescript_project_inputs=_typescript_project_inputs(root, tui_typescript_locators),
        unresolved_imports=tuple(sorted(unresolved)),
    )


def _tui_reachability(locator: str, closure: _TuiClosure) -> ResourceReachability:
    if locator in LEGACY_TUI_ROOT_DEPENDENCY_LOCATORS:
        return ResourceReachability.BUN_PROJECT_INPUT
    if locator == LEGACY_TUI_ENTRY_LOCATOR:
        return ResourceReachability.RUNTIME_ENTRY
    if locator in closure.runtime_reachable:
        return ResourceReachability.RUNTIME_REACHABLE
    if locator in closure.typescript_project_inputs:
        return ResourceReachability.TYPESCRIPT_PROJECT_ONLY
    return ResourceReachability.MIGRATION_CENSUS_ONLY


def _physical_candidate_id(
    stable_key: str,
    locator: str,
    content_sha256: str,
    policy: _ResourcePolicy,
) -> str:
    value = "\0".join(
        (
            "maestro.current-physical-resource-candidate.v1",
            stable_key,
            locator,
            content_sha256,
            policy.owner.value,
            policy.bundle.value,
            policy.disposition.value,
        )
    )
    return _sha256(value.encode("utf-8"))


def _frozen_resource_kind(source: PhysicalSource, policy: _ResourcePolicy) -> FrozenResourceKind:
    name = PurePosixPath(source.stable_locator).name
    suffix = PurePosixPath(source.stable_locator).suffix.lower()
    if name == "LICENSE" or name.startswith("LICENSE."):
        return FrozenResourceKind.LICENSE
    if suffix in {".sh", ".fish"}:
        return FrozenResourceKind.EXECUTABLE
    if policy.bundle == BundleKind.AGENT_BOOTSTRAP:
        return FrozenResourceKind.AGENT_INSTRUCTION
    if policy.bundle == BundleKind.CAPABILITY:
        return FrozenResourceKind.AGENT_INSTRUCTION
    if policy.bundle == BundleKind.ORCHESTRATION:
        return FrozenResourceKind.ORCHESTRATION_DEFINITION
    if policy.bundle == BundleKind.ADAPTER:
        return FrozenResourceKind.ADAPTER_ARTIFACT
    if policy.bundle == BundleKind.EXTERNAL_PATTERN:
        return FrozenResourceKind.EXTERNAL_PATTERN
    if policy.bundle == BundleKind.MIGRATION:
        return FrozenResourceKind.MIGRATION_ARTIFACT
    if any(token in name for token in ("inventory", "receipt", "manifest")):
        return FrozenResourceKind.PROVENANCE_MANIFEST
    if policy.bundle == BundleKind.RELEASE:
        return FrozenResourceKind.BILL_OF_MATERIALS
    return FrozenResourceKind.PUBLIC_CONTRACT


def _bundle_group(source: PhysicalSource, policy: _ResourcePolicy) -> str:
    if policy.bundle != BundleKind.EXTERNAL_PATTERN:
        return f"{policy.bundle.value}:default"
    if source.stable_locator == "embedded/design/styles/neutral/DESIGN.md":
        return "ExternalPattern:first-party-neutral-baseline"
    if source.stable_locator.startswith("embedded/design/vendor/awesome-design-md/"):
        return "ExternalPattern:third-party-awesome-design-md"
    if source.stable_locator.startswith("embedded/vnext/patterns/"):
        return "ExternalPattern:first-party-neutral-baseline"
    raise InventoryError(
        f"ExternalPattern Resource lacks an explicit provenance-separated Bundle group: {source.stable_locator}"
    )


def _provenance_fields(
    source: PhysicalSource, policy: _ResourcePolicy
) -> tuple[ResourceProvenanceKind, str | None, str]:
    if policy.bundle == BundleKind.EXTERNAL_PATTERN:
        if source.stable_locator == "embedded/design/styles/neutral/DESIGN.md":
            return (
                ResourceProvenanceKind.FIRST_PARTY,
                None,
                "explicit UI-design journey with no project design system; default-off and non-authoritative",
            )
        if source.stable_locator.startswith("embedded/design/vendor/awesome-design-md/"):
            return (
                ResourceProvenanceKind.THIRD_PARTY,
                "embedded/design/vendor/awesome-design-md/LICENSE",
                "optional selected-pattern disclosure from the pinned awesome-design-md vendor pack",
            )
    return (
        ResourceProvenanceKind.FIRST_PARTY,
        None,
        "bounded by its declared Resource family and target Bundle contract",
    )


def _make_resource(
    source: PhysicalSource,
    policy: _ResourcePolicy,
    stable_key: str,
    registry_locator: str | None,
    historical: Sequence[HistoricalEvidence],
    source_reachability: ResourceReachability = ResourceReachability.NOT_APPLICABLE,
) -> ResourceCandidate:
    candidate_id = _physical_candidate_id(stable_key, source.stable_locator, source.content_sha256, policy)
    provenance_kind, license_locator, applicability = _provenance_fields(source, policy)
    return ResourceCandidate(
        inventory_ordinal=0,
        stable_key=stable_key,
        physical_candidate_id=candidate_id,
        stable_locator=source.stable_locator,
        family=source.family,
        source_kind=source.source_kind,
        semantic_owner=policy.owner,
        target_bundle_kind=policy.bundle,
        target_bundle_group=_bundle_group(source, policy),
        disposition=policy.disposition,
        content_bytes=source.content_bytes,
        content_sha256=source.content_sha256,
        encoding=source.encoding,
        media_type=source.media_type,
        c868_content_encoding=(
            C868ContentEncoding.UTF8_TEXT
            if source.encoding == ContentEncoding.UTF8
            else C868ContentEncoding.OPAQUE_BYTES
        ),
        frozen_resource_kind=_frozen_resource_kind(source, policy),
        source_reachability=source_reachability,
        provenance=Provenance(
            source_locator=source.stable_locator,
            current_content_sha256=source.content_sha256,
            registry_locator=registry_locator,
            kind=provenance_kind,
            license_locator=license_locator,
            applicability=applicability,
            historical_evidence=tuple(sorted(historical)),
            statement=(
                "current repository bytes from an exact declared root; historical rows are annotations only and do not "
                "establish membership or current equality"
            ),
        ),
    )


def _reader_file_and_symbol(reader_locator: str) -> tuple[str, str | None]:
    file_locator, separator, symbol = reader_locator.partition("#")
    return _normal_locator(file_locator), symbol if separator else None


def _validate_reader_locator(root: Path, reader_locator: str) -> str:
    file_locator, symbol = _reader_file_and_symbol(reader_locator)
    data = _read_regular_file(root, file_locator)
    if symbol:
        text = data.decode("utf-8")
        escaped = re.escape(symbol)
        if re.search(rf"(?:def|fn)\s+{escaped}\b", text) is None:
            raise InventoryError(f"reader symbol does not exist: {reader_locator}")
    return _sha256(data)


def _python_function_read_literals(root: Path, reader_locator: str) -> frozenset[str]:
    file_locator, symbol = _reader_file_and_symbol(reader_locator)
    if symbol is None or not file_locator.endswith(".py"):
        raise InventoryError(f"Python function reader locator is required: {reader_locator}")
    data = _read_regular_file(root, file_locator)
    tree = ast.parse(data, filename=file_locator)
    functions = [
        node
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == symbol
    ]
    if len(functions) != 1:
        raise InventoryError(f"reader symbol is missing or ambiguous: {reader_locator}")

    literals: set[str] = set()
    for node in ast.walk(functions[0]):
        if not isinstance(node, ast.Call):
            continue
        if (
            isinstance(node.func, ast.Name)
            and node.func.id == "load_json"
            and node.args
            and isinstance(node.args[0], ast.Constant)
            and isinstance(node.args[0].value, str)
        ):
            literals.add(node.args[0].value)
            continue
        if not (isinstance(node.func, ast.Attribute) and node.func.attr == "read_bytes"):
            continue
        receiver = node.func.value
        if (
            isinstance(receiver, ast.BinOp)
            and isinstance(receiver.op, ast.Div)
            and isinstance(receiver.left, ast.Name)
            and receiver.left.id == "OUT"
            and isinstance(receiver.right, ast.Constant)
            and isinstance(receiver.right.value, str)
        ):
            literals.add(receiver.right.value)
    return frozenset(literals)


def _macro_proves_binding(root: Path, spec: _MacroReader) -> str:
    data = _read_regular_file(root, spec.reader_locator)
    text = data.decode("utf-8")
    if spec.kind == ReaderEvidenceKind.INCLUDE_DIR_TYPED_EXTRACTOR:
        needle = f'$CARGO_MANIFEST_DIR/{spec.resource_root}'
        pattern = rf'include_dir!\(\s*"{re.escape(needle)}"\s*\)'
    else:
        target = spec.exact_resource_locator
        if target is None:
            raise InventoryError("exact include macro reader has no Resource locator")
        reader_parent = PurePosixPath(spec.reader_locator).parent
        relative = os.path.relpath(target, reader_parent.as_posix()).replace(os.sep, "/")
        macro = "include_bytes" if spec.kind == ReaderEvidenceKind.INCLUDE_BYTES else "include_str"
        pattern = rf'{macro}!\(\s*"{re.escape(relative)}"\s*\)'
    match = re.search(pattern, text)
    if match is None:
        raise InventoryError(f"declared macro reader no longer binds its exact Resource scope: {spec.reader_locator}")
    line = text.count("\n", 0, match.start()) + 1
    return f"{spec.reader_locator}#line:{line}"


def _tui_launcher_reader_sha(root: Path) -> str:
    reader_sha = _validate_reader_locator(root, LEGACY_TUI_LAUNCHER_LOCATOR)
    text = _read_regular_file(root, "src/interfaces/cli/mission_control.rs").decode("utf-8")
    launcher_start = text.find("fn run_opentui(")
    child_start = text.find("fn run_opentui_child(")
    if launcher_start < 0 or child_start <= launcher_start:
        raise InventoryError("run_opentui launcher/child structure changed")
    launcher_body = text[launcher_start:child_start]
    child_body = text[child_start:]
    launcher_fragments = (
        'join("src/tui/sidecar.ts")',
        "run_opentui_child(paths, args, size, &snapshot_file)",
    )
    child_fragments = (
        'Command::new("bun")',
        '.arg("run")',
        '.arg("src/tui/sidecar.ts")',
        '.current_dir(paths.repo_root())',
    )
    missing = [fragment for fragment in launcher_fragments if fragment not in launcher_body]
    missing.extend(fragment for fragment in child_fragments if fragment not in child_body)
    if missing:
        raise InventoryError(f"run_opentui no longer proves the exact Bun sidecar launch: {missing}")
    return reader_sha


def _migration_census_reader(root: Path) -> tuple[str, str]:
    locator = (
        "tools/vnext_contracts/stage0/resource_release/"
        "current_inventory.py#enumerate_authoritative_family_sources"
    )
    return locator, _validate_reader_locator(root, locator)


def build_direct_reader_registry(
    repo_root: Path | str,
    resources: Sequence[ResourceCandidate],
) -> tuple[DirectReaderEvidence, ...]:
    """Expand only closed macro and typed-reader declarations into exact edges."""

    root = _repo_root(repo_root)
    rows: list[DirectReaderEvidence] = []
    resources_by_locator = {resource.stable_locator: resource for resource in resources}
    if len(resources_by_locator) != len(resources):
        raise InventoryError("duplicate Resource locator before reader binding")

    for spec in MACRO_READERS:
        exact_reader = _macro_proves_binding(root, spec)
        reader_sha = _sha256(_read_regular_file(root, spec.reader_locator))
        matched = [
            resource
            for resource in resources
            if (
                resource.stable_locator == spec.exact_resource_locator
                if spec.exact_resource_locator is not None
                else resource.stable_locator.startswith(f"{spec.resource_root}/")
            )
        ]
        if not matched:
            raise InventoryError(f"macro reader binds no admitted Resource: {spec.reader_locator}")
        for resource in matched:
            rows.append(
                DirectReaderEvidence(
                    reader_locator=exact_reader,
                    reader_content_sha256=reader_sha,
                    semantic_owner=spec.owner,
                    kind=(
                        DirectConsumerKind.INSTALL
                        if resource.family in {"skills", "harness", "hooks", "shell"}
                        else DirectConsumerKind.RUNTIME
                    ),
                    evidence_kind=spec.kind,
                    role=ReaderRole.SEALED_READER
                    if resource.disposition == ResourceDisposition.MIGRATION_ONLY
                    else ReaderRole.LIVE_READER,
                    resource_stable_key=resource.stable_key,
                    resource_candidate_id=resource.physical_candidate_id,
                    resource_locator=resource.stable_locator,
                    disposition=resource.disposition,
                    evidence=spec.evidence,
                )
            )

    migration_reader, migration_reader_sha = _migration_census_reader(root)
    for resource in resources:
        if resource.family != "root-agent-instruction":
            continue
        rows.append(
            DirectReaderEvidence(
                reader_locator=migration_reader,
                reader_content_sha256=migration_reader_sha,
                semantic_owner=SemanticOwner.MIGRATION,
                kind=DirectConsumerKind.MIGRATION,
                evidence_kind=ReaderEvidenceKind.MIGRATION_CENSUS,
                role=(
                    ReaderRole.SEALED_READER
                    if resource.disposition == ResourceDisposition.REMOVE
                    else ReaderRole.LIVE_READER
                ),
                resource_stable_key=resource.stable_key,
                resource_candidate_id=resource.physical_candidate_id,
                resource_locator=resource.stable_locator,
                disposition=resource.disposition,
                evidence="exact E204 root instruction bytes are read as explicit Migration Resources",
            )
        )

    tui_closure = _legacy_tui_closure(root)
    launcher_sha = _tui_launcher_reader_sha(root)
    tsconfig_sha = _sha256(_read_regular_file(root, "tsconfig.json"))
    for resource in resources:
        if resource.family != "legacy-tui":
            continue
        reachability = resource.source_reachability
        if reachability == ResourceReachability.RUNTIME_ENTRY:
            reader_locator = LEGACY_TUI_LAUNCHER_LOCATOR
            reader_sha = launcher_sha
            consumer_kind = DirectConsumerKind.RUNTIME
            evidence_kind = ReaderEvidenceKind.BUN_LAUNCHER
            evidence = "run_opentui launches the exact src/tui/sidecar.ts Bun entry from the repository root"
        elif reachability == ResourceReachability.RUNTIME_REACHABLE:
            edge = tui_closure.runtime_parent_edges.get(resource.stable_locator)
            if edge is None:
                raise InventoryError(f"reachable TUI Resource has no exact parent import: {resource.stable_locator}")
            reader_locator = f"{edge.importer_locator}#line:{edge.line}"
            reader_sha = _sha256(_read_regular_file(root, edge.importer_locator))
            consumer_kind = DirectConsumerKind.RUNTIME
            evidence_kind = ReaderEvidenceKind.TYPESCRIPT_IMPORT
            evidence = (
                f"Bun entry import closure resolves {edge.specifier!r} from "
                f"{edge.importer_locator} to this exact Resource"
            )
        elif reachability == ResourceReachability.TYPESCRIPT_PROJECT_ONLY:
            reader_locator = "tsconfig.json#include"
            reader_sha = tsconfig_sha
            consumer_kind = DirectConsumerKind.BUILD
            evidence_kind = ReaderEvidenceKind.TYPESCRIPT_PROJECT_INPUT
            evidence = "tsconfig include closure admits this non-runtime TypeScript source to tsc --noEmit"
        elif reachability == ResourceReachability.BUN_PROJECT_INPUT:
            reader_locator = LEGACY_TUI_LAUNCHER_LOCATOR
            reader_sha = launcher_sha
            consumer_kind = DirectConsumerKind.BUILD
            evidence_kind = ReaderEvidenceKind.BUN_PROJECT_INPUT
            evidence = (
                "run_opentui launches Bun from the repository root, binding the exact package, lock, "
                "and TypeScript project inputs"
            )
        elif reachability == ResourceReachability.MIGRATION_CENSUS_ONLY:
            reader_locator = migration_reader
            reader_sha = migration_reader_sha
            consumer_kind = DirectConsumerKind.MIGRATION
            evidence_kind = ReaderEvidenceKind.MIGRATION_CENSUS
            evidence = (
                "exact legacy TUI bytes are inventoried for Migration but are unreachable from the live Bun entry "
                "and direct TypeScript project include roots"
            )
        else:
            raise InventoryError(f"legacy TUI Resource lacks typed reachability: {resource.stable_locator}")
        rows.append(
            DirectReaderEvidence(
                reader_locator=reader_locator,
                reader_content_sha256=reader_sha,
                semantic_owner=SemanticOwner.MIGRATION,
                kind=consumer_kind,
                evidence_kind=evidence_kind,
                role=ReaderRole.LIVE_READER,
                resource_stable_key=resource.stable_key,
                resource_candidate_id=resource.physical_candidate_id,
                resource_locator=resource.stable_locator,
                disposition=resource.disposition,
                evidence=evidence,
            )
        )

    for group in CANDIDATE_GROUPS:
        reader_sha = _validate_reader_locator(root, group.reader_locator)
        bound_literals = (
            _python_function_read_literals(root, group.reader_locator)
            if group.family == "vnext-resource-release"
            else None
        )
        for resource in resources:
            if resource.family != group.family:
                continue
            if bound_literals is not None and PurePosixPath(resource.stable_locator).name not in bound_literals:
                raise InventoryError(
                    "declared preidentity validator no longer reads exact admitted Resource: "
                    f"{resource.stable_locator}"
                )
            rows.append(
                DirectReaderEvidence(
                    reader_locator=group.reader_locator,
                    reader_content_sha256=reader_sha,
                    semantic_owner=group.reader_owner,
                    kind=(
                        DirectConsumerKind.MIGRATION
                        if group.reader_role == ReaderRole.SEALED_READER
                        else DirectConsumerKind.PROOF
                    ),
                    evidence_kind=group.reader_kind,
                    role=group.reader_role,
                    resource_stable_key=resource.stable_key,
                    resource_candidate_id=resource.physical_candidate_id,
                    resource_locator=resource.stable_locator,
                    disposition=resource.disposition,
                    evidence=group.reader_evidence,
                )
            )

    def identity(row: DirectReaderEvidence) -> tuple[str, str, str, str, str]:
        return (
            row.reader_locator,
            row.resource_stable_key,
            row.role.value,
            row.kind.value,
            row.evidence_kind.value,
        )
    if len({identity(row) for row in rows}) != len(rows):
        raise InventoryError("duplicate direct-reader evidence row")
    return tuple(sorted(rows, key=identity))


def _resource_rows(
    root: Path,
    families: Sequence[ResourceFamilyDeclaration],
    authoritative: Sequence[PhysicalSource],
    vnext: Sequence[PhysicalSource],
    historical: HistoricalE204Ledger,
) -> tuple[ResourceCandidate, ...]:
    family_by_id = {family.family_id: family for family in families}
    historical_by_locator: dict[str, list[HistoricalEvidence]] = {}
    for row in historical.rows:
        historical_by_locator.setdefault(row.locator, []).append(row)
    skill_dispositions = _load_skill_dispositions(root, authoritative)
    membership = _load_membership_overrides(root)
    tui_closure = _legacy_tui_closure(root)
    resources: list[ResourceCandidate] = []

    for source in authoritative:
        if source.source_kind == SourceKind.GENERATED_REFERENCE_PRODUCER:
            continue
        if source.family not in FAMILY_POLICIES:
            raise InventoryError(f"unknown Resource family policy: {source.family}")
        policy = FAMILY_POLICIES[source.family]
        if source.family == "skills":
            policy = _ResourcePolicy(policy.owner, policy.bundle, skill_dispositions[source.stable_locator])
        elif source.family == "root-agent-instruction":
            policy = _ResourcePolicy(
                policy.owner,
                policy.bundle,
                ROOT_INSTRUCTION_DISPOSITIONS[source.stable_locator],
            )
        key = f"v1.{source.family}.{source.stable_locator}"
        if source.stable_locator in membership:
            key, policy = membership[source.stable_locator]
        family = family_by_id.get(source.family)
        resources.append(
            _make_resource(
                source,
                policy,
                key,
                family.registry_locator if family is not None else None,
                historical_by_locator.get(source.stable_locator, ()),
                (
                    _tui_reachability(source.stable_locator, tui_closure)
                    if source.family == "legacy-tui"
                    else ResourceReachability.NOT_APPLICABLE
                ),
            )
        )

    for source in vnext:
        if source.source_kind in {
            SourceKind.HISTORICAL_NON_PROMOTING_EVIDENCE,
            SourceKind.DOCUMENTATION_NOT_RESOURCE,
            SourceKind.GENERATED_PROOF_OUTPUT,
        }:
            continue
        group = _candidate_group(source.stable_locator)
        if group is None or source.family == "unclassified":
            continue
        key = f"current.{group.family}.{source.stable_locator}"
        policy = group.policy
        if source.stable_locator in membership:
            key, policy = membership[source.stable_locator]
        resources.append(_make_resource(source, policy, key, None, historical_by_locator.get(source.stable_locator, ())))

    resources.sort(key=lambda row: row.stable_locator)
    resources = [replace(row, inventory_ordinal=index) for index, row in enumerate(resources, 1)]
    if len({row.stable_locator for row in resources}) != len(resources):
        raise InventoryError("one physical path was classified as more than one Resource")
    if len({row.stable_key for row in resources}) != len(resources):
        raise InventoryError("duplicate Resource stable key")
    if len({row.physical_candidate_id for row in resources}) != len(resources):
        raise InventoryError("duplicate physical Resource candidate id")
    return tuple(resources)


def build_current_inventory(repo_root: Path | str) -> CurrentInventory:
    """Build the deterministic read-only inventory from current repository bytes."""

    root = _repo_root(repo_root)
    families = parse_authoritative_families(root)
    authoritative = enumerate_authoritative_family_sources(root, families)
    historical = load_historical_e204(root)
    vnext = enumerate_vnext_candidates(root)
    unclassified = tuple(sorted(source.stable_locator for source in vnext if source.family == "unclassified"))
    exclusions = tuple(
        sorted(
            (
                ClassifiedExclusion(
                    source.stable_locator,
                    source.source_kind,
                    (
                        "sealed historical evidence; never Resource membership"
                        if source.source_kind == SourceKind.HISTORICAL_NON_PROMOTING_EVIDENCE
                        else (
                            "downstream generated proof/output; cannot feed back into Resource membership"
                            if source.source_kind == SourceKind.GENERATED_PROOF_OUTPUT
                            else "documentation; not a distribution Resource"
                        )
                    ),
                )
                for source in vnext
                if source.source_kind
                in {
                    SourceKind.HISTORICAL_NON_PROMOTING_EVIDENCE,
                    SourceKind.DOCUMENTATION_NOT_RESOURCE,
                    SourceKind.GENERATED_PROOF_OUTPUT,
                }
            ),
            key=lambda row: row.stable_locator,
        )
    )
    resources = _resource_rows(root, families, authoritative, vnext, historical)
    readers = build_direct_reader_registry(root, resources)
    inventory = CurrentInventory(
        families=families,
        authoritative_sources=authoritative,
        vnext_sources=vnext,
        resources=resources,
        direct_readers=readers,
        historical_e204=historical,
        exclusions=exclusions,
        unclassified_paths=unclassified,
    )
    validate_inventory(inventory)
    return inventory


def canonical_inventory_payload(inventory: CurrentInventory) -> dict[str, object]:
    """Return the identity-bearing projection, excluding every generated output fact."""

    return {
        "families": [
            {
                "id": row.family_id,
                "root": row.source_root,
                "ownership_mode": row.ownership_mode,
                "parser": row.parser_owner,
                "validator": row.validator_owner,
            }
            for row in inventory.families
        ],
        "authoritative_sources": [
            {
                "locator": row.stable_locator,
                "family": row.family,
                "kind": row.source_kind.value,
                "kind_tag": row.source_kind.tag,
                "sha256": row.content_sha256,
                "bytes": len(row.content_bytes),
                "encoding": row.encoding.value,
                "media_type": row.media_type,
            }
            for row in inventory.authoritative_sources
        ],
        "vnext_sources": [
            {
                "locator": row.stable_locator,
                "family": row.family,
                "kind": row.source_kind.value,
                "kind_tag": row.source_kind.tag,
                "sha256": row.content_sha256,
                "bytes": len(row.content_bytes),
                "encoding": row.encoding.value,
                "media_type": row.media_type,
            }
            for row in inventory.vnext_sources
            if row.source_kind
            not in {SourceKind.GENERATED_PROOF_OUTPUT, SourceKind.DOCUMENTATION_NOT_RESOURCE}
        ],
        "resources": [
            {
                "inventory_ordinal": row.inventory_ordinal,
                "key": row.stable_key,
                "candidate_id": row.physical_candidate_id,
                "locator": row.stable_locator,
                "family": row.family,
                "source_kind": row.source_kind.value,
                "source_kind_tag": row.source_kind.tag,
                "owner": row.semantic_owner.value,
                "owner_tag": row.semantic_owner.frozen_tag,
                "bundle": row.target_bundle_kind.value,
                "bundle_tag": row.target_bundle_kind.tag,
                "bundle_group": row.target_bundle_group,
                "disposition": row.disposition.value,
                "disposition_tag": row.disposition.tag,
                "sha256": row.content_sha256,
                "bytes": len(row.content_bytes),
                "encoding": row.encoding.value,
                "c868_content_encoding": row.c868_content_encoding.value,
                "c868_content_encoding_tag": row.c868_content_encoding.tag,
                "media_type": row.media_type,
                "frozen_resource_kind": row.frozen_resource_kind.value,
                "frozen_resource_kind_tag": row.frozen_resource_kind.tag,
                "source_reachability": row.source_reachability.value,
                "provenance": {
                    "source_locator": row.provenance.source_locator,
                    "current_content_sha256": row.provenance.current_content_sha256,
                    "registry_locator": row.provenance.registry_locator,
                    "kind": row.provenance.kind.value,
                    "kind_tag": row.provenance.kind.tag,
                    "license_locator": row.provenance.license_locator,
                    "applicability": row.provenance.applicability,
                    "statement": row.provenance.statement,
                },
                "historical": [
                    {
                        "locator": evidence.locator,
                        "sha256": evidence.recorded_sha256,
                        "family": evidence.family,
                        "current_bytes_equal": evidence.current_bytes_equal,
                    }
                    for evidence in row.provenance.historical_evidence
                ],
            }
            for row in inventory.resources
        ],
        "readers": [
            {
                "reader": row.reader_locator,
                "reader_sha256": row.reader_content_sha256,
                "owner": row.semantic_owner.value,
                "kind": row.kind.value,
                "kind_tag": row.kind.tag,
                "evidence_kind": row.evidence_kind.value,
                "role": row.role.value,
                "resource_key": row.resource_stable_key,
                "resource_candidate_id": row.resource_candidate_id,
                "resource_locator": row.resource_locator,
                "disposition": row.disposition.value,
                "evidence": row.evidence,
                "dual_role": row.explicit_dual_role_contract,
            }
            for row in inventory.direct_readers
        ],
        "historical_e204": {
            "locator": inventory.historical_e204.locator,
            "ledger_sha256": inventory.historical_e204.content_sha256,
            "manifest_digest": inventory.historical_e204.expected_digest,
            "count": len(inventory.historical_e204.rows),
        },
        "exclusions": [
            {"locator": row.stable_locator, "kind": row.source_kind.value, "reason": row.reason}
            for row in inventory.exclusions
            if row.source_kind
            not in {SourceKind.GENERATED_PROOF_OUTPUT, SourceKind.DOCUMENTATION_NOT_RESOURCE}
        ],
        "unclassified_paths": list(inventory.unclassified_paths),
    }


def noncanonical_generated_output_audit(inventory: CurrentInventory) -> dict[str, object]:
    """Report post-Release generated bytes without admitting any identity dependency."""

    rows = sorted(
        (
            {
                "locator": row.stable_locator,
                "sha256": row.content_sha256,
                "bytes": len(row.content_bytes),
                "encoding": row.encoding.value,
                "media_type": row.media_type,
            }
            for row in inventory.vnext_sources
            if row.source_kind == SourceKind.GENERATED_PROOF_OUTPUT
        ),
        key=lambda row: str(row["locator"]),
    )
    return {
        "schema": "maestro.vnext.noncanonical-generated-output-audit.v1",
        "post_release_only": True,
        "canonical_inventory_hash_participation": False,
        "source_count": len(rows),
        "sources": rows,
    }


def inventory_hash(inventory: CurrentInventory) -> str:
    """Hash a canonical JSON projection; raw bytes are represented by hash+length."""

    encoded = json.dumps(
        canonical_inventory_payload(inventory),
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return _sha256(encoded)


def _validate_inventory_hash_isolation(inventory: CurrentInventory) -> None:
    """Run in-memory mutants for noncanonical isolation and source sensitivity."""

    for root in (RESOURCE_RELEASE_ROOT, *POST_RELEASE_GENERATED_OUTPUT_ROOTS):
        if not _is_post_release_generated_output(
            f"{root}/__generated-output-isolation-mutant.v1.json"
        ):
            raise InventoryError(f"downstream generated-output root is not excluded: {root}")

    baseline_payload = canonical_inventory_payload(inventory)
    baseline_hash = inventory_hash(inventory)
    baseline_resource_identity = tuple(
        (row.stable_key, row.physical_candidate_id) for row in inventory.resources
    )
    baseline_reader_identity = tuple(
        (row.reader_locator, row.resource_stable_key, row.resource_candidate_id)
        for row in inventory.direct_readers
    )

    def assert_isolated(mutant: CurrentInventory, label: str) -> None:
        if tuple(
            (row.stable_key, row.physical_candidate_id) for row in mutant.resources
        ) != baseline_resource_identity:
            raise InventoryError(f"{label} mutant changed the Resource set")
        if (
            tuple(
                (row.reader_locator, row.resource_stable_key, row.resource_candidate_id)
                for row in mutant.direct_readers
            )
            != baseline_reader_identity
        ):
            raise InventoryError(f"{label} mutant changed the direct-reader set")
        if canonical_inventory_payload(mutant) != baseline_payload or inventory_hash(mutant) != baseline_hash:
            raise InventoryError(f"{label} byte/path/presence leaked into canonical inventory identity")

    generated_removal = replace(
        inventory,
        vnext_sources=tuple(
            row
            for row in inventory.vnext_sources
            if row.source_kind != SourceKind.GENERATED_PROOF_OUTPUT
        ),
        exclusions=tuple(
            row
            for row in inventory.exclusions
            if row.source_kind != SourceKind.GENERATED_PROOF_OUTPUT
        ),
    )
    generated_mutants = [generated_removal]
    for root in (RESOURCE_RELEASE_ROOT, *POST_RELEASE_GENERATED_OUTPUT_ROOTS):
        for suffix in (b"presence", b"byte-change"):
            synthetic_bytes = b'{"mutant":"generated-output-' + suffix + b'"}\n'
            synthetic = PhysicalSource(
                stable_locator=f"{root}/__generated-output-isolation-mutant.v1.json",
                family="noncanonical-post-release-generated-output",
                source_kind=SourceKind.GENERATED_PROOF_OUTPUT,
                ownership_mode="GeneratedOutputNotResource",
                content_bytes=synthetic_bytes,
                content_sha256=_sha256(synthetic_bytes),
                encoding=ContentEncoding.UTF8,
                media_type="application/json",
            )
            synthetic_exclusion = ClassifiedExclusion(
                synthetic.stable_locator,
                SourceKind.GENERATED_PROOF_OUTPUT,
                "downstream generated proof/output; cannot feed back into Resource membership",
            )
            generated_mutants.append(
                replace(
                    inventory,
                    vnext_sources=tuple((*inventory.vnext_sources, synthetic)),
                    exclusions=tuple((*inventory.exclusions, synthetic_exclusion)),
                )
            )
    existing_generated = next(
        (
            row
            for row in inventory.vnext_sources
            if row.source_kind == SourceKind.GENERATED_PROOF_OUTPUT
        ),
        None,
    )
    if existing_generated is not None:
        changed_bytes = existing_generated.content_bytes + b"\x00generated-output-mutant"
        changed = replace(
            existing_generated,
            stable_locator=(
                f"{existing_generated.stable_locator.rsplit('/', 1)[0]}"
                "/__renamed-generated-output-mutant.v1.json"
            ),
            content_bytes=changed_bytes,
            content_sha256=_sha256(changed_bytes),
        )
        changed_sources = tuple(
            changed if row is existing_generated else row for row in inventory.vnext_sources
        )
        changed_exclusions = tuple(
            replace(row, stable_locator=changed.stable_locator)
            if row.stable_locator == existing_generated.stable_locator
            and row.source_kind == SourceKind.GENERATED_PROOF_OUTPUT
            else row
            for row in inventory.exclusions
        )
        generated_mutants.append(
            replace(inventory, vnext_sources=changed_sources, exclusions=changed_exclusions)
        )

    for mutant in generated_mutants:
        assert_isolated(mutant, "generated-output")

    synthetic_documentation_bytes = b"# noncanonical documentation mutant\n"
    synthetic_documentation = PhysicalSource(
        stable_locator="embedded/vnext/__documentation-isolation-mutant.md",
        family="vnext-documentation",
        source_kind=SourceKind.DOCUMENTATION_NOT_RESOURCE,
        ownership_mode="DocumentationOnly",
        content_bytes=synthetic_documentation_bytes,
        content_sha256=_sha256(synthetic_documentation_bytes),
        encoding=ContentEncoding.UTF8,
        media_type="text/markdown",
    )
    synthetic_documentation_exclusion = ClassifiedExclusion(
        synthetic_documentation.stable_locator,
        SourceKind.DOCUMENTATION_NOT_RESOURCE,
        "documentation; not a distribution Resource",
    )
    documentation_mutants = [
        replace(
            inventory,
            vnext_sources=tuple((*inventory.vnext_sources, synthetic_documentation)),
            exclusions=tuple((*inventory.exclusions, synthetic_documentation_exclusion)),
        ),
        replace(
            inventory,
            vnext_sources=tuple(
                row
                for row in inventory.vnext_sources
                if row.source_kind != SourceKind.DOCUMENTATION_NOT_RESOURCE
            ),
            exclusions=tuple(
                row
                for row in inventory.exclusions
                if row.source_kind != SourceKind.DOCUMENTATION_NOT_RESOURCE
            ),
        ),
    ]
    existing_documentation = next(
        (
            row
            for row in inventory.vnext_sources
            if row.source_kind == SourceKind.DOCUMENTATION_NOT_RESOURCE
        ),
        None,
    )
    if existing_documentation is not None:
        changed_documentation_bytes = existing_documentation.content_bytes + b"\x00documentation-mutant"
        changed_documentation = replace(
            existing_documentation,
            stable_locator="embedded/vnext/__renamed-documentation-mutant.md",
            content_bytes=changed_documentation_bytes,
            content_sha256=_sha256(changed_documentation_bytes),
        )
        documentation_mutants.append(
            replace(
                inventory,
                vnext_sources=tuple(
                    changed_documentation if row is existing_documentation else row
                    for row in inventory.vnext_sources
                ),
                exclusions=tuple(
                    replace(row, stable_locator=changed_documentation.stable_locator)
                    if row.stable_locator == existing_documentation.stable_locator
                    and row.source_kind == SourceKind.DOCUMENTATION_NOT_RESOURCE
                    else row
                    for row in inventory.exclusions
                ),
            )
        )
    for mutant in documentation_mutants:
        assert_isolated(mutant, "documentation-only")

    admitted_locator = min(RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS)
    admitted_source = next(
        (row for row in inventory.vnext_sources if row.stable_locator == admitted_locator),
        None,
    )
    if admitted_source is None:
        raise InventoryError(f"admitted Resource/Release source is missing: {admitted_locator}")
    admitted_bytes = admitted_source.content_bytes + b"\x00admitted-source-mutant"
    admitted_mutant = replace(
        inventory,
        vnext_sources=tuple(
            replace(
                row,
                content_bytes=admitted_bytes,
                content_sha256=_sha256(admitted_bytes),
            )
            if row is admitted_source
            else row
            for row in inventory.vnext_sources
        ),
    )
    if canonical_inventory_payload(admitted_mutant) == baseline_payload:
        raise InventoryError("admitted source bytes are absent from the canonical inventory projection")
    if inventory_hash(admitted_mutant) == baseline_hash:
        raise InventoryError("admitted source byte mutation did not change canonical inventory identity")


def _validate_legacy_tui_resource_closure(resources: Sequence[ResourceCandidate]) -> None:
    actual = {row.stable_locator for row in resources if row.family == "legacy-tui"}
    if actual != LEGACY_TUI_EXPECTED_LOCATORS:
        raise InventoryError(
            "legacy TUI Resource closure is not exact: "
            f"missing={sorted(LEGACY_TUI_EXPECTED_LOCATORS - actual)}, "
            f"extra={sorted(actual - LEGACY_TUI_EXPECTED_LOCATORS)}"
        )


def validate_inventory(inventory: CurrentInventory) -> InventoryValidation:
    """Validate exact classification and same-Resource direct-reader closure."""

    expected_families = tuple((row.family_id, row.source_root) for row in inventory.families)
    if expected_families != EXPECTED_FAMILY_ROOTS:
        raise InventoryError("inventory does not retain the authoritative nine-family closure")
    authoritative_locators = [row.stable_locator for row in inventory.authoritative_sources]
    vnext_locators = [row.stable_locator for row in inventory.vnext_sources]
    if authoritative_locators != sorted(authoritative_locators) or len(set(authoritative_locators)) != len(
        authoritative_locators
    ):
        raise InventoryError("authoritative sources are not unique and sorted")
    if vnext_locators != sorted(vnext_locators) or len(set(vnext_locators)) != len(vnext_locators):
        raise InventoryError("vNext sources are not unique and sorted")
    if set(authoritative_locators) & set(vnext_locators):
        raise InventoryError("authoritative and vNext source universes overlap")

    generated_producers = {
        row.stable_locator
        for row in inventory.authoritative_sources
        if row.source_kind == SourceKind.GENERATED_REFERENCE_PRODUCER
    }
    if len(generated_producers) != EXPECTED_GENERATED_REFERENCE_PRODUCER_COUNT or any(
        not locator.startswith("src/interfaces/") for locator in generated_producers
    ):
        raise InventoryError(
            "generated CLI/MCP/reference producer closure changed: "
            f"expected {EXPECTED_GENERATED_REFERENCE_PRODUCER_COUNT} exact src/interfaces files, "
            f"got {len(generated_producers)}"
        )
    resource_by_key = {row.stable_key: row for row in inventory.resources}
    if len(resource_by_key) != len(inventory.resources):
        raise InventoryError("duplicate Resource stable key")
    resource_by_locator = {row.stable_locator: row for row in inventory.resources}
    if len(resource_by_locator) != len(inventory.resources):
        raise InventoryError("duplicate physical Resource classification")
    if generated_producers & set(resource_by_locator):
        raise InventoryError("GeneratedReferenceOutput producer source was promoted to Resource membership")
    if HISTORICAL_ONLY_LOCATORS & set(resource_by_locator):
        raise InventoryError("historical evidence was promoted to Resource membership")
    generated_output_sources = {
        row.stable_locator
        for row in inventory.vnext_sources
        if row.source_kind == SourceKind.GENERATED_PROOF_OUTPUT
    }
    if generated_output_sources & set(resource_by_locator):
        raise InventoryError("downstream generated output was promoted to Resource membership")
    resource_release_sources = {
        locator
        for locator in vnext_locators
        if locator.startswith(f"{RESOURCE_RELEASE_ROOT}/")
    }
    if not RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS <= resource_release_sources:
        raise InventoryError(
            "Resource/Release admitted input closure is incomplete: "
            f"missing={sorted(RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS - resource_release_sources)}"
        )
    expected_generated_outputs = (
        resource_release_sources - RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS
    ) | POST_RELEASE_GENERATED_OUTPUT_LOCATORS | {
        locator
        for locator in vnext_locators
        if any(locator.startswith(f"{root}/") for root in POST_RELEASE_GENERATED_OUTPUT_ROOTS)
    }
    if generated_output_sources != expected_generated_outputs:
        raise InventoryError(
            "every post-Release or non-input Resource/Release path must be a noncanonical generated output: "
            f"missing={sorted(expected_generated_outputs - generated_output_sources)}, "
            f"extra={sorted(generated_output_sources - expected_generated_outputs)}"
        )
    admitted_resource_release = set(resource_by_locator) & resource_release_sources
    if admitted_resource_release != RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS:
        raise InventoryError(
            "Resource/Release admission is not the exact seven pre-identity inputs: "
            f"missing={sorted(RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS - admitted_resource_release)}, "
            f"extra={sorted(admitted_resource_release - RESOURCE_RELEASE_ADMITTED_INPUT_LOCATORS)}"
        )
    if len(inventory.resources) != EXPECTED_RESOURCE_COUNT:
        raise InventoryError(
            f"Resource closure changed: expected {EXPECTED_RESOURCE_COUNT}, got {len(inventory.resources)}"
        )
    if [row.inventory_ordinal for row in inventory.resources] != list(range(1, len(inventory.resources) + 1)):
        raise InventoryError("Resource inventory ordinals are not exact contiguous sorted tags")

    e204_locators = {row.locator for row in inventory.historical_e204.rows}
    current_e204_sources = {
        row.stable_locator
        for row in inventory.authoritative_sources
        if row.source_kind == SourceKind.V1_SHIPPED_RESOURCE and row.family != "legacy-tui"
    }
    if current_e204_sources != e204_locators:
        raise InventoryError(
            "current 204-byte-source closure differs from sealed E204 path coverage: "
            f"missing={sorted(e204_locators - current_e204_sources)}, "
            f"extra={sorted(current_e204_sources - e204_locators)}"
        )
    if not e204_locators <= set(resource_by_locator):
        raise InventoryError(
            f"E204 current bytes lack Resource ownership/disposition: {sorted(e204_locators - set(resource_by_locator))}"
        )
    for locator in sorted(e204_locators):
        resource = resource_by_locator[locator]
        expected_owner = SemanticOwner.DESIGN if locator.startswith("embedded/design/") else SemanticOwner.MIGRATION
        expected_bundle = BundleKind.EXTERNAL_PATTERN if locator.startswith("embedded/design/") else BundleKind.MIGRATION
        if resource.semantic_owner != expected_owner or resource.target_bundle_kind != expected_bundle:
            raise InventoryError(
                f"locked E204 legacy ownership changed for {locator}: "
                f"{resource.semantic_owner.value}/{resource.target_bundle_kind.value}"
            )
        if len(resource.provenance.historical_evidence) != 1:
            raise InventoryError(f"E204 Resource lacks its exact non-promoting historical annotation: {locator}")

    _validate_legacy_tui_resource_closure(inventory.resources)
    omitted_tui_locator = min(LEGACY_TUI_EXPECTED_LOCATORS)
    try:
        _validate_legacy_tui_resource_closure(
            tuple(row for row in inventory.resources if row.stable_locator != omitted_tui_locator)
        )
    except InventoryError:
        pass
    else:
        raise InventoryError("legacy TUI omission mutant was not rejected")
    tui_resources = [row for row in inventory.resources if row.family == "legacy-tui"]
    for resource in tui_resources:
        if (
            resource.semantic_owner != SemanticOwner.MIGRATION
            or resource.target_bundle_kind != BundleKind.MIGRATION
            or resource.disposition != ResourceDisposition.REPLACE
            or resource.frozen_resource_kind != FrozenResourceKind.MIGRATION_ARTIFACT
            or resource.source_reachability == ResourceReachability.NOT_APPLICABLE
        ):
            raise InventoryError(f"legacy TUI Resource is activatable or not Migration-owned: {resource.stable_locator}")
    tui_runtime_count = sum(
        row.source_reachability
        in {ResourceReachability.RUNTIME_ENTRY, ResourceReachability.RUNTIME_REACHABLE}
        for row in tui_resources
    )
    tui_typescript_only_count = sum(
        row.source_reachability == ResourceReachability.TYPESCRIPT_PROJECT_ONLY
        for row in tui_resources
    )
    tui_migration_only_count = sum(
        row.source_reachability == ResourceReachability.MIGRATION_CENSUS_ONLY
        for row in tui_resources
    )
    if (
        tui_runtime_count != LEGACY_TUI_EXPECTED_RUNTIME_REACHABLE_COUNT
        or tui_typescript_only_count != LEGACY_TUI_EXPECTED_TYPESCRIPT_PROJECT_ONLY_COUNT
        or tui_migration_only_count != LEGACY_TUI_EXPECTED_MIGRATION_CENSUS_ONLY_COUNT
    ):
        raise InventoryError(
            "legacy TUI reachability changed: "
            f"runtime={tui_runtime_count}, typescript_only={tui_typescript_only_count}, "
            f"migration_only={tui_migration_only_count}"
        )
    if {
        row.stable_locator
        for row in tui_resources
        if row.source_reachability == ResourceReachability.BUN_PROJECT_INPUT
    } != set(LEGACY_TUI_ROOT_DEPENDENCY_LOCATORS):
        raise InventoryError("legacy TUI Bun project root input closure changed")

    external_pattern_resources = [
        row for row in inventory.resources if row.target_bundle_kind == BundleKind.EXTERNAL_PATTERN
    ]
    external_pattern_groups: dict[str, set[str]] = {}
    for resource in external_pattern_resources:
        external_pattern_groups.setdefault(resource.target_bundle_group, set()).add(resource.stable_locator)
    expected_external_pattern_groups = {
        "ExternalPattern:first-party-neutral-baseline": {
            "embedded/design/styles/neutral/DESIGN.md",
            *{
                row.stable_locator
                for row in inventory.vnext_sources
                if row.stable_locator.startswith("embedded/vnext/patterns/")
            },
        },
        "ExternalPattern:third-party-awesome-design-md": {
            row.stable_locator
            for row in inventory.authoritative_sources
            if row.stable_locator.startswith("embedded/design/vendor/awesome-design-md/")
        },
    }
    if external_pattern_groups != expected_external_pattern_groups:
        raise InventoryError("ExternalPattern Resources are not split into exact first-party/vendor Bundles")
    for resource in external_pattern_resources:
        if resource.target_bundle_group == "ExternalPattern:first-party-neutral-baseline":
            if resource.provenance.kind != ResourceProvenanceKind.FIRST_PARTY or resource.provenance.license_locator:
                raise InventoryError("neutral design baseline provenance changed")
        elif (
            resource.provenance.kind != ResourceProvenanceKind.THIRD_PARTY
            or resource.provenance.license_locator != "embedded/design/vendor/awesome-design-md/LICENSE"
        ):
            raise InventoryError(f"vendor ExternalPattern provenance changed: {resource.stable_locator}")

    actual_bundle_counts = {
        kind: sum(row.target_bundle_kind == kind for row in inventory.resources) for kind in BundleKind
    }
    if actual_bundle_counts != EXPECTED_BUNDLE_COUNTS:
        raise InventoryError(
            "locked Bundle assignment counts changed: "
            f"expected={{{', '.join(f'{key.value}: {value}' for key, value in EXPECTED_BUNDLE_COUNTS.items())}}}, "
            f"actual={{{', '.join(f'{key.value}: {value}' for key, value in actual_bundle_counts.items())}}}"
        )
    non_release_owner = {
        BundleKind.AGENT_BOOTSTRAP: SemanticOwner.AGENT_BOOTSTRAP,
        BundleKind.CAPABILITY: SemanticOwner.CAPABILITY,
        BundleKind.ORCHESTRATION: SemanticOwner.ORCHESTRATION,
        BundleKind.SHARED_CONTRACT: SemanticOwner.SHARED_CONTRACT,
        BundleKind.ADAPTER: SemanticOwner.ADAPTER,
        BundleKind.EXTERNAL_PATTERN: SemanticOwner.DESIGN,
        BundleKind.MIGRATION: SemanticOwner.MIGRATION,
    }
    release_root_kinds = {
        FrozenResourceKind.EXECUTABLE,
        FrozenResourceKind.SIGNATURE,
        FrozenResourceKind.BILL_OF_MATERIALS,
    }
    for resource in inventory.resources:
        if resource.target_bundle_kind == BundleKind.RELEASE:
            if resource.frozen_resource_kind not in release_root_kinds:
                raise InventoryError(
                    f"non-Executable/Signature/BillOfMaterials Resource targets Release: {resource.stable_locator}"
                )
            if resource.stable_locator not in EMBEDDED_RELEASE_ROOT_ADMISSION_LOCATORS:
                raise InventoryError(
                    f"Resource targets Release without explicit EmbeddedReleaseHeader admission: {resource.stable_locator}"
                )
        elif non_release_owner.get(resource.target_bundle_kind) != resource.semantic_owner:
            raise InventoryError(
                f"non-root Resource lacks its exact concrete non-Release owner/Bundle: {resource.stable_locator} "
                f"{resource.semantic_owner.value}/{resource.target_bundle_kind.value}"
            )
        _ = (
            resource.source_kind.tag,
            resource.semantic_owner.frozen_tag,
            resource.target_bundle_kind.tag,
            resource.disposition.tag,
            resource.c868_content_encoding.tag,
            resource.frozen_resource_kind.tag,
            resource.provenance.kind.tag,
        )

    edges_by_resource: dict[str, list[DirectReaderEvidence]] = {key: [] for key in resource_by_key}
    roles_by_evidence: dict[
        tuple[str, str, DirectConsumerKind, ReaderEvidenceKind], set[ReaderRole]
    ] = {}
    for edge in inventory.direct_readers:
        bound_resource = resource_by_key.get(edge.resource_stable_key)
        if bound_resource is None:
            raise InventoryError(f"reader edge references unknown Resource key: {edge.resource_stable_key}")
        if (
            edge.resource_candidate_id != bound_resource.physical_candidate_id
            or edge.resource_locator != bound_resource.stable_locator
            or edge.disposition != bound_resource.disposition
        ):
            raise InventoryError(f"reader edge is not bound to the same Resource: {edge.reader_locator}")
        edges_by_resource[edge.resource_stable_key].append(edge)
        role_key = (edge.reader_locator, edge.resource_stable_key, edge.kind, edge.evidence_kind)
        roles_by_evidence.setdefault(role_key, set()).add(edge.role)
    for key, roles in roles_by_evidence.items():
        if len(roles) > 1:
            rows = [
                row
                for row in inventory.direct_readers
                if (row.reader_locator, row.resource_stable_key, row.kind, row.evidence_kind) == key
            ]
            if not all(row.explicit_dual_role_contract for row in rows):
                raise InventoryError(f"one reader evidence row carries multiple typed roles without a dual-role contract: {key}")

    for resource in inventory.resources:
        edges = edges_by_resource[resource.stable_key]
        if len(edges) != 1:
            raise InventoryError(
                f"Resource must have exactly one non-fabricated direct-reader edge: {resource.stable_locator}"
            )
        live = [edge for edge in edges if edge.role == ReaderRole.LIVE_READER]
        sealed = [edge for edge in edges if edge.role == ReaderRole.SEALED_READER]
        removal = [edge for edge in edges if edge.role == ReaderRole.REMOVAL_PROOF]
        if resource.disposition == ResourceDisposition.REMOVE:
            if not (sealed or removal):
                raise InventoryError(
                    f"Remove Resource has neither exact removal proof nor sealed reader: {resource.stable_locator}"
                )
        elif resource.disposition == ResourceDisposition.MIGRATION_ONLY:
            if not (live or sealed):
                raise InventoryError(f"MigrationOnly Resource has no actual reader: {resource.stable_locator}")
        elif not live:
            raise InventoryError(f"admitted non-Remove Resource has no actual live reader: {resource.stable_locator}")
        if resource.family == "legacy-tui":
            edge = edges[0]
            expected_evidence_kind = {
                ResourceReachability.RUNTIME_ENTRY: ReaderEvidenceKind.BUN_LAUNCHER,
                ResourceReachability.RUNTIME_REACHABLE: ReaderEvidenceKind.TYPESCRIPT_IMPORT,
                ResourceReachability.TYPESCRIPT_PROJECT_ONLY: ReaderEvidenceKind.TYPESCRIPT_PROJECT_INPUT,
                ResourceReachability.BUN_PROJECT_INPUT: ReaderEvidenceKind.BUN_PROJECT_INPUT,
                ResourceReachability.MIGRATION_CENSUS_ONLY: ReaderEvidenceKind.MIGRATION_CENSUS,
            }.get(resource.source_reachability)
            if edge.evidence_kind != expected_evidence_kind:
                raise InventoryError(f"legacy TUI reader does not match typed reachability: {resource.stable_locator}")

    if len(inventory.historical_e204.rows) != E204_EXPECTED_COUNT:
        raise InventoryError("historical E204 row count changed")
    if inventory.historical_e204.expected_digest != E204_EXPECTED_DIGEST:
        raise InventoryError("historical E204 digest changed")
    if inventory.unclassified_paths:
        raise InventoryError(f"unclassified declared paths: {list(inventory.unclassified_paths)}")
    excluded = {row.stable_locator for row in inventory.exclusions}
    expected_excluded = (
        HISTORICAL_ONLY_LOCATORS
        | DOCUMENTATION_ONLY_LOCATORS
        | generated_output_sources
    )
    if excluded != expected_excluded:
        raise InventoryError(
            f"classified exclusion closure changed: missing={sorted(expected_excluded - excluded)}, "
            f"extra={sorted(excluded - expected_excluded)}"
        )

    _validate_inventory_hash_isolation(inventory)
    generated_audit = noncanonical_generated_output_audit(inventory)

    return InventoryValidation(
        family_count=len(inventory.families),
        authoritative_source_count=len(inventory.authoritative_sources),
        generated_reference_producer_count=len(generated_producers),
        vnext_source_count=len(inventory.vnext_sources),
        resource_count=len(inventory.resources),
        direct_reader_edge_count=len(inventory.direct_readers),
        historical_e204_count=len(inventory.historical_e204.rows),
        exclusion_count=len(inventory.exclusions),
        generated_output_audit_count=len(generated_output_sources),
        external_pattern_bundle_group_count=len(external_pattern_groups),
        legacy_tui_source_count=len(tui_resources),
        legacy_tui_runtime_reachable_count=tui_runtime_count,
        legacy_tui_typescript_project_only_count=tui_typescript_only_count,
        legacy_tui_migration_census_only_count=tui_migration_only_count,
        unclassified_paths=inventory.unclassified_paths,
        inventory_sha256=inventory_hash(inventory),
    )


__all__ = (
    "BundleKind",
    "C868ContentEncoding",
    "ClassifiedExclusion",
    "ContentEncoding",
    "CurrentInventory",
    "DirectConsumerKind",
    "DirectReaderEvidence",
    "HistoricalE204Ledger",
    "HistoricalEvidence",
    "FrozenResourceKind",
    "InventoryError",
    "InventoryValidation",
    "PhysicalSource",
    "Provenance",
    "ReaderEvidenceKind",
    "ReaderRole",
    "ResourceCandidate",
    "ResourceDisposition",
    "ResourceFamilyDeclaration",
    "ResourceProvenanceKind",
    "ResourceReachability",
    "SemanticOwner",
    "SourceKind",
    "build_current_inventory",
    "build_direct_reader_registry",
    "canonical_inventory_payload",
    "enumerate_authoritative_family_sources",
    "enumerate_vnext_candidates",
    "inventory_hash",
    "load_historical_e204",
    "noncanonical_generated_output_audit",
    "parse_authoritative_families",
    "validate_inventory",
)
