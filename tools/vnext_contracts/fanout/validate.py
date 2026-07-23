#!/usr/bin/env python3
"""Fail-closed validation for external Stage 6-12 candidate ownership."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import unicodedata
import zlib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping, Sequence, cast


SCHEMA_VERSION = "maestro.external.vnext-fanout-base.v1"
MANIFEST_PATH = Path(__file__).with_name("fanout-base.v1.json")
MANIFEST_REPOSITORY_PATH = "tools/vnext_contracts/fanout/fanout-base.v1.json"
STAGES = (6, 7, 8, 9, 10, 11, 12)
GIT_OBJECT_ID = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
REGULAR_BLOB_MODES = {"100644", "100755"}
EXECUTABLE_SCRIPT_SUFFIXES = {".py", ".rb", ".sh"}
FORBIDDEN_OBJECT_INFO_NAMES = {"alternates", "http-alternates"}
PROMISOR_CONFIG_KEYS = {"extensions.partialclone"}
FORBIDDEN_DIFF_CONFIG_KEYS = {"diff.ignoresubmodules"}
MAX_LOOSE_OBJECT_BYTES = 512 * 1024 * 1024
DESIGN_PATH = ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md"
DESIGN_SHA256 = "8009a8f5d13c65d781a124559482725e2f05b2f41bf22953ebb5afbfc611ee61"
DESIGN_BYTE_LENGTH = 2_178_218
MANIFEST_IDENTITY = "sha256:b7fe6a736c906ccbe8eb830348c63b62b8562f4c7799d2cd87a606e6b12e7393"
SUCCESSOR_DECISIONS = (
    (
        "dec-canonical-non-action-protected-90a9",
        "8c6be56db78d8695b4e85e09fc4217257fee0b2dce0f5b5be8ef10230f24c20e",
        2_963,
    ),
    (
        "dec-canonical-trusted-host-protected-1fbc",
        "e572dc28e0c811c81207558e64b0372f757a873122b7f537f6354af819f118d8",
        9_943,
    ),
)
CANONICAL_INPUTS = (
    (DESIGN_PATH, DESIGN_SHA256, DESIGN_BYTE_LENGTH),
    (
        ".maestro/cards/dec-canonical-non-action-protected-90a9/card.yaml",
        SUCCESSOR_DECISIONS[0][1],
        SUCCESSOR_DECISIONS[0][2],
    ),
    (
        ".maestro/cards/dec-canonical-trusted-host-protected-1fbc/card.yaml",
        SUCCESSOR_DECISIONS[1][1],
        SUCCESSOR_DECISIONS[1][2],
    ),
)
STAGE5_POINTER = "contracts/vnext/stage5/evidence-gates/current-proof.json"
CERTIFIED_STAGE5 = {
    "commit": "527f7b2687a7d51737dc3e6e0c02dfdb6d6f611a",
    "tree": "ebc01e90cd4f4bd9452662251f5252513358b86c",
    "publication_pointer": STAGE5_POINTER,
    "release_identity": "sha256:7c0a4aab9f2fdc8989c1affc9818ce6235ef9338008f8d00d53ad2d4022940c6",
    "plan_identity": "sha256:4e1cc2633a93645c457f78f326b155be44e8f1b3f267098e43c43b0c44f8296c",
    "snapshot_identity": "sha256:e76ae1421bb871b9f35edb23eec7a7b510d07f12d25df1ffa36d62abae8f7ece",
}
AUTHORITY_POLICY = {
    "canonical_integration_branch": "codex/maestro-vnext-refoundation",
    "canonical_writer": "external-orchestrator",
    "worker_merge_authority": False,
    "worker_canonical_artifact_authority": False,
    "worker_seal_authority": False,
}
SCHEDULING_POLICY = {
    "canonical_integration_order": list(STAGES),
    "initial_candidates": list(STAGES[:-1]),
    "queued_candidates": [12],
    "max_logical_stage_workers": 6,
    "max_concurrent_compile_or_focused_test_jobs": 2,
    "max_concurrent_broad_gates": 1,
    "max_concurrent_canonical_seals": 1,
}
PATH_POLICY = {
    "candidate_diff_statuses": ["A", "M"],
    "candidate_blob_modes": sorted(REGULAR_BLOB_MODES),
    "candidate_executable_suffixes": sorted(EXECUTABLE_SCRIPT_SUFFIXES),
    "renames": "disabled",
    "base_existing_paths": "immutable_except_exact_mutable_seed_files_and_inherited_mutable_seed_files",
    "new_paths": "must_match_exact_stage_write_prefix",
    "shared_files": "external_orchestrator_only_except_owning_stage_inherited_mutable_seed_files",
}
FROZEN_INTERFACE_ROOTS = (
    "contracts/vnext/catalogs/",
    "contracts/vnext/public/",
    "contracts/vnext/stage0/",
    "contracts/vnext/stage2/",
    "contracts/vnext/stage3/",
    "contracts/vnext/stage4/",
    "contracts/vnext/stage5/",
    "src/domain/vnext/authority/",
    "src/domain/vnext/contract/",
    "src/domain/vnext/design/",
    "src/domain/vnext/execution/",
    "src/domain/vnext/gate/",
    "src/domain/vnext/identity/",
    "src/domain/vnext/integration/",
    "src/domain/vnext/persistence/",
    "src/domain/vnext/repository/",
    "src/domain/vnext/step/",
    "src/domain/vnext/work/",
)
SHARED_DENYLIST_EXACT_FILES = (
    "AGENTS.md",
    "ARCHITECTURE.md",
    "Cargo.lock",
    "Cargo.toml",
    "MAINTENANCE.md",
    "README.md",
    "TESTING.md",
    "build.rs",
    "src/domain/mod.rs",
    "src/domain/vnext/capability/mod.rs",
    "src/domain/vnext/distribution/mod.rs",
    "src/domain/vnext/evidence/mod.rs",
    "src/domain/vnext/migration/mod.rs",
    "src/domain/vnext/mod.rs",
    "src/domain/vnext/orchestration/mod.rs",
    "src/interfaces/mod.rs",
    "src/lib.rs",
    "src/main.rs",
    "src/operations/mod.rs",
    "src/interfaces/vnext/mod.rs",
    "src/operations/vnext/mod.rs",
    "tests/vnext_stage3_contracts.rs",
    "tests/vnext_stage4_contracts.rs",
    "tests/vnext_stage5_contracts.rs",
    "tests/vnext_stage5_evidence_gates.rs",
    "tests/architecture_imports.rs",
    "tools/vnext_contracts/stage0/effect_home/build.py",
    "tools/vnext_contracts/stage0/effect_home/test_build.py",
    "tools/vnext_contracts/stage3/domain/build.py",
    "tools/vnext_contracts/stage3/domain/test_build.py",
    "tools/vnext_contracts/stage3/domain/validate.py",
    "tools/vnext_contracts/stage3/domain/verify.rb",
    "tools/vnext_contracts/stage4/execution/build.py",
    "tools/vnext_contracts/stage4/execution/test_behavior_census.py",
    "tools/vnext_contracts/stage4/execution/validate.py",
    "tools/vnext_contracts/stage4/execution/verify.rb",
)
SHARED_DENYLIST_PREFIXES = (
    ".git/",
    ".maestro/",
    "contracts/vnext/",
    "embedded/vnext/capability/",
    "embedded/vnext/orchestration/",
    "embedded/vnext/release/",
    "src/domain/vnext/authority/",
    "src/domain/vnext/contract/",
    "src/domain/vnext/design/",
    "src/domain/vnext/execution/",
    "src/domain/vnext/gate/",
    "src/domain/vnext/identity/",
    "src/domain/vnext/integration/",
    "src/domain/vnext/persistence/",
    "src/domain/vnext/repository/",
    "src/domain/vnext/step/",
    "src/domain/vnext/work/",
    "src/foundation/",
    "src/interfaces/cli/",
    "src/interfaces/hooks/",
    "src/interfaces/mcp/",
    "src/interfaces/shell/",
    "src/interfaces/tui/",
    "tests/architecture_",
    "tests/common/",
    "tests/support/",
    "tests/witness_support/",
    "tools/vnext_contracts/fanout/",
)


@dataclass(frozen=True)
class StageOwnerPolicy:
    stage: int
    candidate_scope: str
    write_prefixes: tuple[str, ...]
    mutable_seed_files: tuple[str, ...]
    inherited_mutable_seed_files: tuple[str, ...] = ()
    production_mutation: str | None = None

    def manifest_row(self) -> dict[str, object]:
        row: dict[str, object] = {
            "stage": self.stage,
            "candidate_scope": self.candidate_scope,
            "write_prefixes": list(self.write_prefixes),
            "mutable_seed_files": list(self.mutable_seed_files),
            "inherited_mutable_seed_files": list(self.inherited_mutable_seed_files),
        }
        if self.production_mutation is not None:
            row["production_mutation"] = self.production_mutation
        return row


STAGE_OWNER_POLICIES = (
    StageOwnerPolicy(
        6,
        "Projection, transport, Action submission and replay, CLI and JSON, generated capability catalog",
        (
            "src/domain/vnext/capability/generated_catalog/",
            "src/domain/vnext/projection/",
            "src/domain/vnext/transport/",
            "src/interfaces/vnext/cli/",
            "src/operations/vnext/action/",
            "tests/fixtures/vnext/stage6/",
            "tests/vnext_stage6_",
            "tools/vnext_contracts/stage6/",
        ),
        (
            "src/domain/vnext/capability/generated_catalog/mod.rs",
            "src/domain/vnext/projection/mod.rs",
            "src/domain/vnext/transport/mod.rs",
            "src/interfaces/vnext/cli/mod.rs",
            "src/operations/vnext/action/mod.rs",
        ),
    ),
    StageOwnerPolicy(
        7,
        "Orchestration, Recipe application and return, Planning, Scheduling Assessment, Coordination and Inbox",
        (
            "src/domain/vnext/coordination/",
            "src/domain/vnext/orchestration/runtime/",
            "src/domain/vnext/planning/",
            "src/operations/vnext/orchestration/",
            "tests/fixtures/vnext/stage7/",
            "tests/vnext_stage7_",
            "tools/vnext_contracts/stage7/",
        ),
        (
            "src/domain/vnext/coordination/mod.rs",
            "src/domain/vnext/orchestration/runtime/mod.rs",
            "src/domain/vnext/planning/mod.rs",
            "src/operations/vnext/orchestration/mod.rs",
        ),
    ),
    StageOwnerPolicy(
        8,
        "Search, Memory, Intake, Research, Capability and Maturity, observation-facing diagnostics",
        (
            "src/domain/vnext/capability/runtime/",
            "src/domain/vnext/evidence/diagnostics/",
            "src/domain/vnext/intake/",
            "src/domain/vnext/maturity/",
            "src/domain/vnext/memory/",
            "src/domain/vnext/research/",
            "src/domain/vnext/search/",
            "src/operations/vnext/observation/",
            "tests/fixtures/vnext/stage8/",
            "tests/vnext_stage8_",
            "tools/vnext_contracts/stage8/",
        ),
        (
            "src/domain/vnext/capability/runtime/mod.rs",
            "src/domain/vnext/evidence/diagnostics/mod.rs",
            "src/domain/vnext/intake/mod.rs",
            "src/domain/vnext/maturity/mod.rs",
            "src/domain/vnext/memory/mod.rs",
            "src/domain/vnext/research/mod.rs",
            "src/domain/vnext/search/mod.rs",
            "src/operations/vnext/observation/mod.rs",
        ),
        inherited_mutable_seed_files=(
            "src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs",
        ),
    ),
    StageOwnerPolicy(
        9,
        "Distribution and Installation transaction, custody, snapshot catalog, recovery, currentness and domain-local publication",
        (
            "src/domain/vnext/distribution/runtime/",
            "src/domain/vnext/installation/",
            "src/operations/vnext/installation/",
            "tests/fixtures/vnext/stage9/",
            "tests/vnext_stage9_",
            "tools/vnext_contracts/stage9/",
        ),
        (
            "src/domain/vnext/distribution/runtime/mod.rs",
            "src/domain/vnext/installation/mod.rs",
            "src/operations/vnext/installation/mod.rs",
        ),
        inherited_mutable_seed_files=(
            "src/domain/vnext/persistence/mod.rs",
            "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs",
        ),
    ),
    StageOwnerPolicy(
        10,
        "Two read-only MCP adapters, production trusted-host acquisition and presentation adapter with Stage 5/8 parity proof, host descriptors, Skill and bootstrap wiring, TUI, hooks, shell, schemas, patterns and connectors",
        (
            "embedded/vnext/bootstrap/",
            "embedded/vnext/connectors/",
            "embedded/vnext/hosts/",
            "embedded/vnext/patterns/",
            "embedded/vnext/schemas/",
            "src/interfaces/vnext/connectors/",
            "src/interfaces/vnext/hooks/",
            "src/interfaces/vnext/mcp/",
            "src/interfaces/vnext/shell/",
            "src/interfaces/vnext/tui/",
            "src/operations/vnext/adapters/",
            "tests/fixtures/vnext/stage10/",
            "tests/vnext_stage10_",
            "tools/vnext_contracts/stage10/",
        ),
        (
            "src/interfaces/vnext/connectors/mod.rs",
            "src/interfaces/vnext/hooks/mod.rs",
            "src/interfaces/vnext/mcp/mod.rs",
            "src/interfaces/vnext/shell/mod.rs",
            "src/interfaces/vnext/tui/mod.rs",
            "src/operations/vnext/adapters/mod.rs",
        ),
        inherited_mutable_seed_files=(
            "src/domain/vnext/integration/mod.rs",
            "src/domain/vnext/integration/trusted_host_diagnostic_stage10_seed.rs",
        ),
    ),
    StageOwnerPolicy(
        11,
        "Byte-total inventory, classification, identity map, quarantine, inactive-store import, migration fixtures and consumer closure",
        (
            "src/domain/vnext/migration/runtime/",
            "src/operations/vnext/migration/",
            "tests/fixtures/vnext/stage11/",
            "tests/vnext_stage11_",
            "tools/vnext_contracts/stage11/",
        ),
        (
            "src/domain/vnext/migration/runtime/mod.rs",
            "src/operations/vnext/migration/mod.rs",
        ),
    ),
    StageOwnerPolicy(
        12,
        "Initially read-only consumer census, architecture guards, negative compatibility fixtures and release-proof inputs",
        (
            "tests/fixtures/vnext/stage12/",
            "tests/vnext_stage12_",
            "tools/vnext_contracts/stage12/",
        ),
        (),
        production_mutation="external_orchestrator_only_after_stage11_integrated_unsealed",
    ),
)

EXPECTED_MANIFEST_FIELDS = {
    "schema_version",
    "feature_id",
    "design",
    "successor_decisions",
    "certified_stage5",
    "authority",
    "scheduling",
    "path_policy",
    "fanout_base",
    "frozen_interface_roots",
    "shared_denylist",
    "stage_owners",
}

class FanoutValidationError(RuntimeError):
    """The candidate cannot cross the external fanout boundary."""


@dataclass(frozen=True)
class CommitObject:
    tree: str
    parents: tuple[str, ...]


@dataclass(frozen=True)
class TreeEntry:
    mode: str
    object_type: str
    object_id: str
    path: str


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def manifest_identity(manifest: Mapping[str, Any]) -> str:
    try:
        return f"sha256:{hashlib.sha256(canonical_json(manifest)).hexdigest()}"
    except (TypeError, ValueError) as error:
        raise FanoutValidationError("fanout manifest is not canonical JSON") from error


def load_manifest(path: Path = MANIFEST_PATH) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise FanoutValidationError("fanout manifest must be one JSON object")
    return cast(dict[str, Any], value)


def strings(value: object, field: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise FanoutValidationError(f"{field} must be a string array")
    return cast(list[str], value)


def normalized_path(value: str, *, prefix: bool) -> str:
    if not value or "\\" in value or value.startswith("/"):
        raise FanoutValidationError(f"unsafe repository path: {value!r}")
    if any(ord(character) < 32 or ord(character) == 127 for character in value):
        raise FanoutValidationError(f"repository path contains an ASCII control: {value!r}")
    if any(unicodedata.category(character).startswith("C") for character in value):
        raise FanoutValidationError(f"repository path contains a control-like code point: {value!r}")
    if unicodedata.normalize("NFC", value) != value:
        raise FanoutValidationError(f"repository path is not NFC-normalized: {value!r}")
    if unicodedata.normalize("NFKC", value) != value:
        raise FanoutValidationError(
            f"repository path has ambiguous compatibility normalization: {value!r}"
        )
    if not prefix and value.endswith("/"):
        raise FanoutValidationError(f"repository file has the wrong terminator: {value}")
    candidate = value[:-1] if value.endswith("/") else value
    parts = PurePosixPath(candidate).parts
    if not parts or any(part in {"", ".", ".."} for part in parts):
        raise FanoutValidationError(f"repository path is not normalized: {value}")
    if any(part.casefold() == ".git" for part in parts) and not (
        prefix and candidate.casefold() == ".git"
    ):
        raise FanoutValidationError(f"repository path contains reserved .git metadata: {value}")
    if any(part.endswith((" ", ".")) for part in parts):
        raise FanoutValidationError(f"repository path has a trailing-space/dot alias: {value!r}")
    if str(PurePosixPath(*parts)) != candidate:
        raise FanoutValidationError(f"repository path is not canonical: {value}")
    return value


def filesystem_alias_key(path: str) -> str:
    return "/".join(
        unicodedata.normalize("NFD", component).casefold()
        for component in path.split("/")
    )


def validate_tree_path_keys(paths: Iterable[str], label: str) -> set[str]:
    exact_paths: set[str] = set()
    aliases: dict[str, str] = {}
    for raw_path in paths:
        path = normalized_path(raw_path, prefix=False)
        if path in exact_paths:
            raise FanoutValidationError(f"{label} contains duplicate path {path}")
        exact_paths.add(path)
        components = path.split("/")
        for length in range(1, len(components) + 1):
            path_prefix = "/".join(components[:length])
            alias = filesystem_alias_key(path_prefix)
            previous = aliases.setdefault(alias, path_prefix)
            if previous != path_prefix:
                raise FanoutValidationError(
                    f"{label} contains case/normalization alias collision: "
                    f"{previous!r} and {path_prefix!r}"
                )
    return exact_paths


def owner_rows(manifest: Mapping[str, Any]) -> list[dict[str, Any]]:
    rows = manifest.get("stage_owners")
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise FanoutValidationError("stage_owners must be an object array")
    return cast(list[dict[str, Any]], rows)


def owner_for_stage(manifest: Mapping[str, Any], stage: int) -> dict[str, Any]:
    matches = [row for row in owner_rows(manifest) if row.get("stage") == stage]
    if len(matches) != 1:
        raise FanoutValidationError(f"Stage {stage} must have exactly one owner row")
    return matches[0]


def denied_paths(manifest: Mapping[str, Any]) -> tuple[set[str], tuple[str, ...]]:
    deny = manifest.get("shared_denylist")
    if not isinstance(deny, dict):
        raise FanoutValidationError("shared_denylist must be an object")
    files = {
        normalized_path(path, prefix=False)
        for path in strings(deny.get("exact_files"), "shared_denylist.exact_files")
    }
    prefixes = tuple(
        normalized_path(path, prefix=True)
        for path in strings(deny.get("path_prefixes"), "shared_denylist.path_prefixes")
    )
    return files, prefixes


def path_is_denied(path: str, files: set[str], prefixes: Sequence[str]) -> bool:
    return path in files or any(path.startswith(prefix) for prefix in prefixes)


def prefixes_overlap(first: str, second: str) -> bool:
    return first.startswith(second) or second.startswith(first)


def expected_fanout_base_changes(manifest: Mapping[str, Any]) -> dict[str, str]:
    fanout_base = manifest.get("fanout_base")
    if not isinstance(fanout_base, dict) or set(fanout_base) != {
        "orchestrator_owned_files"
    }:
        raise FanoutValidationError("fanout_base policy differs")
    raw_files = fanout_base.get("orchestrator_owned_files")
    if not isinstance(raw_files, dict) or not raw_files:
        raise FanoutValidationError(
            "fanout_base.orchestrator_owned_files must be a non-empty object"
        )

    denied_files, denied_prefixes = denied_paths(manifest)
    expected: dict[str, str] = {}
    for raw_path, status in raw_files.items():
        if not isinstance(raw_path, str) or not isinstance(status, str):
            raise FanoutValidationError(
                "fanout_base.orchestrator_owned_files must map paths to statuses"
            )
        path = normalized_path(raw_path, prefix=False)
        if status not in {"A", "M"}:
            raise FanoutValidationError(
                f"fanout base path {path} has unsupported status {status}"
            )
        if not path_is_denied(path, denied_files, denied_prefixes):
            raise FanoutValidationError(
                f"fanout base orchestrator path is not shared-denied: {path}"
            )
        expected[path] = status

    for path, _, _ in CANONICAL_INPUTS:
        if expected.get(path) != "A":
            raise FanoutValidationError(
                f"canonical fanout input must be an orchestrator-owned addition: {path}"
            )

    inherited_paths: list[str] = []
    for row in owner_rows(manifest):
        stage = row.get("stage")
        for seed in strings(
            row.get("mutable_seed_files"), f"Stage {stage} mutable_seed_files"
        ):
            path = normalized_path(seed, prefix=False)
            if path in expected:
                raise FanoutValidationError(
                    f"fanout base mutable seed is also orchestrator-owned: {path}"
                )
            expected[path] = "A"
        for seed in strings(
            row.get("inherited_mutable_seed_files"),
            f"Stage {stage} inherited_mutable_seed_files",
        ):
            path = normalized_path(seed, prefix=False)
            if path in expected:
                raise FanoutValidationError(
                    f"fanout base inherited mutable seed is also a delta: {path}"
                )
            inherited_paths.append(path)
    validate_tree_path_keys(
        (*expected, *inherited_paths), "fanout manifest path set"
    )
    return expected


def validate_manifest(manifest: Mapping[str, Any]) -> None:
    if set(manifest) != EXPECTED_MANIFEST_FIELDS:
        raise FanoutValidationError("fanout manifest field set differs")
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise FanoutValidationError("fanout manifest schema differs")
    if manifest.get("feature_id") != "maestro-whole-flow-architecture-refoundation":
        raise FanoutValidationError("fanout feature identity differs")
    design = manifest.get("design")
    if not isinstance(design, dict) or design != {
        "path": DESIGN_PATH,
        "sha256": DESIGN_SHA256,
    }:
        raise FanoutValidationError("fanout design identity differs")
    decisions = manifest.get("successor_decisions")
    expected_decisions = [
        {
            "card_id": card_id,
            "card_yaml_sha256": digest,
            "status": "locked",
        }
        for card_id, digest, _ in SUCCESSOR_DECISIONS
    ]
    if decisions != expected_decisions:
        raise FanoutValidationError("fanout successor Decision identities differ")
    certified = manifest.get("certified_stage5")
    if certified != CERTIFIED_STAGE5:
        raise FanoutValidationError("certified Stage-5 tuple differs")

    if manifest.get("authority") != AUTHORITY_POLICY:
        raise FanoutValidationError("fanout authority policy differs")

    scheduling = manifest.get("scheduling")
    if scheduling != SCHEDULING_POLICY:
        raise FanoutValidationError("fanout scheduling policy differs")

    path_policy = manifest.get("path_policy")
    if path_policy != PATH_POLICY:
        raise FanoutValidationError("fanout path policy differs")

    if manifest.get("frozen_interface_roots") != list(FROZEN_INTERFACE_ROOTS):
        raise FanoutValidationError("frozen interface root policy differs")
    if manifest.get("shared_denylist") != {
        "exact_files": list(SHARED_DENYLIST_EXACT_FILES),
        "path_prefixes": list(SHARED_DENYLIST_PREFIXES),
    }:
        raise FanoutValidationError("shared denylist policy differs")

    rows = owner_rows(manifest)
    if rows != [policy.manifest_row() for policy in STAGE_OWNER_POLICIES]:
        raise FanoutValidationError("Stage ownership policy differs")
    denied_files, denied_prefixes = denied_paths(manifest)
    seen_prefixes: list[tuple[int, str]] = []
    seen_seeds: dict[str, int] = {}
    for row in rows:
        stage = row["stage"]
        prefixes = [
            normalized_path(path, prefix=True)
            for path in strings(row.get("write_prefixes"), f"Stage {stage} write_prefixes")
        ]
        seeds = [
            normalized_path(path, prefix=False)
            for path in strings(row.get("mutable_seed_files"), f"Stage {stage} mutable_seed_files")
        ]
        inherited_seeds = [
            normalized_path(path, prefix=False)
            for path in strings(
                row.get("inherited_mutable_seed_files"),
                f"Stage {stage} inherited_mutable_seed_files",
            )
        ]
        if (
            len(prefixes) != len(set(prefixes))
            or len(seeds) != len(set(seeds))
            or len(inherited_seeds) != len(set(inherited_seeds))
            or set(seeds).intersection(inherited_seeds)
        ):
            raise FanoutValidationError(f"Stage {stage} contains duplicate ownership paths")
        for prefix in prefixes:
            if path_is_denied(prefix, denied_files, denied_prefixes):
                raise FanoutValidationError(
                    f"Stage {stage} write prefix intersects the shared denylist: {prefix}"
                )
            for other_stage, other_prefix in seen_prefixes:
                if prefixes_overlap(prefix, other_prefix):
                    raise FanoutValidationError(
                        f"Stage {stage} prefix {prefix} overlaps Stage {other_stage} prefix {other_prefix}"
                    )
            seen_prefixes.append((stage, prefix))
        for seed in seeds:
            if not any(seed.startswith(prefix) for prefix in prefixes):
                raise FanoutValidationError(
                    f"Stage {stage} mutable seed is outside its owned prefixes: {seed}"
                )
            if path_is_denied(seed, denied_files, denied_prefixes):
                raise FanoutValidationError(
                    f"Stage {stage} mutable seed intersects the shared denylist: {seed}"
                )
            previous = seen_seeds.setdefault(seed, stage)
            if previous != stage:
                raise FanoutValidationError(
                    f"mutable seed {seed} is assigned to Stages {previous} and {stage}"
                )
        for seed in inherited_seeds:
            if not path_is_denied(seed, denied_files, denied_prefixes):
                raise FanoutValidationError(
                    f"Stage {stage} inherited mutable seed is not shared-denied: {seed}"
                )
            previous = seen_seeds.setdefault(seed, stage)
            if previous != stage:
                raise FanoutValidationError(
                    f"mutable seed {seed} is assigned to Stages {previous} and {stage}"
                )

    validate_tree_path_keys(seen_seeds, "Stage ownership seed set")

    frozen = strings(manifest.get("frozen_interface_roots"), "frozen_interface_roots")
    for prefix in frozen:
        normalized_path(prefix, prefix=True)
        if not any(prefix.startswith(denied) for denied in denied_prefixes):
            raise FanoutValidationError(f"frozen interface root is not denied: {prefix}")
    expected_fanout_base_changes(manifest)
    if manifest_identity(manifest) != MANIFEST_IDENTITY:
        raise FanoutValidationError("fanout manifest identity differs")


def validate_changes(
    manifest: Mapping[str, Any],
    stage: int,
    changes: Iterable[tuple[str, str, str, str]],
    existing_at_fanout: set[str],
) -> list[str]:
    validate_manifest(manifest)
    owner = owner_for_stage(manifest, stage)
    prefixes = strings(owner["write_prefixes"], f"Stage {stage} write_prefixes")
    seeds = set(strings(owner["mutable_seed_files"], f"Stage {stage} mutable_seed_files"))
    inherited_seeds = set(
        strings(
            owner["inherited_mutable_seed_files"],
            f"Stage {stage} inherited_mutable_seed_files",
        )
    )
    denied_files, denied_prefixes = denied_paths(manifest)
    allowed_statuses = set(
        strings(
            cast(Mapping[str, Any], manifest["path_policy"])["candidate_diff_statuses"],
            "path_policy.candidate_diff_statuses",
        )
    )
    accepted: list[str] = []
    for status, raw_path, old_mode, new_mode in changes:
        path = normalized_path(raw_path, prefix=False)
        is_inherited_seed = path in inherited_seeds
        if status not in allowed_statuses:
            raise FanoutValidationError(
                f"Stage {stage} path {path} uses forbidden diff status {status}"
            )
        if is_inherited_seed and status != "M":
            raise FanoutValidationError(
                f"Stage {stage} inherited mutable seed {path} requires status M"
            )
        if new_mode not in REGULAR_BLOB_MODES:
            raise FanoutValidationError(
                f"Stage {stage} path {path} is not an ordinary blob: {new_mode}"
            )
        existed = path in existing_at_fanout
        if is_inherited_seed:
            if not existed:
                raise FanoutValidationError(
                    f"Stage {stage} inherited mutable seed is absent from the fanout base: {path}"
                )
            if old_mode not in REGULAR_BLOB_MODES or old_mode != new_mode:
                raise FanoutValidationError(
                    f"Stage {stage} changed the object type or mode for {path}"
                )
            accepted.append(path)
            continue
        if (
            new_mode == "100755"
            and PurePosixPath(path).suffix not in EXECUTABLE_SCRIPT_SUFFIXES
        ):
            raise FanoutValidationError(
                f"Stage {stage} path {path} uses an unapproved executable mode"
            )
        if path_is_denied(path, denied_files, denied_prefixes):
            raise FanoutValidationError(f"Stage {stage} touched shared path {path}")
        if not any(path.startswith(prefix) for prefix in prefixes):
            raise FanoutValidationError(f"Stage {stage} does not own {path}")
        if existed and path not in seeds:
            raise FanoutValidationError(
                f"Stage {stage} changed frozen fanout-base path {path}"
            )
        if status == "A" and existed:
            raise FanoutValidationError(f"Stage {stage} re-added existing path {path}")
        if status == "A" and old_mode != "000000":
            raise FanoutValidationError(
                f"Stage {stage} added path {path} with a non-absent old mode"
            )
        if status == "M" and not existed:
            raise FanoutValidationError(f"Stage {stage} modified absent path {path}")
        if status == "M" and (
            old_mode not in REGULAR_BLOB_MODES or old_mode != new_mode
        ):
            raise FanoutValidationError(
                f"Stage {stage} changed the object type or mode for {path}"
            )
        accepted.append(path)
    if not accepted:
        raise FanoutValidationError(f"Stage {stage} candidate has no owned changes")
    return sorted(accepted)


def git_process(
    repository: Path, arguments: Sequence[str]
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        git_command(repository, arguments),
        check=False,
        capture_output=True,
        env=git_environment(),
    )


def git(repository: Path, arguments: Sequence[str], *, check: bool = True) -> bytes:
    completed = git_process(repository, arguments)
    if check and completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise FanoutValidationError(f"git {' '.join(arguments)} failed: {detail}")
    return completed.stdout


def git_command(repository: Path, arguments: Sequence[str]) -> list[str]:
    return [
        "git",
        "--no-replace-objects",
        "--no-optional-locks",
        "--no-lazy-fetch",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.commitGraph=false",
        "-c",
        "core.multiPackIndex=false",
        "-c",
        "gc.auto=0",
        "-c",
        "maintenance.auto=false",
        "-C",
        str(repository),
        *arguments,
    ]


def git_environment() -> dict[str, str]:
    environment = {
        key: value for key, value in os.environ.items() if not key.startswith("GIT_")
    }
    environment.update(
        {
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_TERMINAL_PROMPT": "0",
        }
    )
    return environment


def decoded_git_line(raw: bytes, label: str) -> str:
    if not raw.endswith(b"\n") or b"\0" in raw or b"\n" in raw[:-1] or b"\r" in raw:
        raise FanoutValidationError(f"git returned a malformed {label}")
    try:
        value = raw[:-1].decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise FanoutValidationError(f"git returned a non-UTF-8 {label}") from error
    if not value:
        raise FanoutValidationError(f"git returned an empty {label}")
    return value


def path_entry_exists(path: Path) -> bool:
    return os.path.lexists(path)


def resolved_git_directory(repository: Path, argument: str, label: str) -> Path:
    value = decoded_git_line(
        git(repository, ["rev-parse", "--path-format=absolute", argument]), label
    )
    path = Path(value)
    if not path.is_absolute() or path.is_symlink():
        raise FanoutValidationError(f"{label} is not a local ordinary directory")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise FanoutValidationError(f"cannot resolve {label}") from error
    if not resolved.is_dir():
        raise FanoutValidationError(f"{label} is not a directory")
    return resolved


def git_metadata_path(repository: Path, argument: str, label: str) -> Path:
    value = decoded_git_line(
        git(
            repository,
            ["rev-parse", "--path-format=absolute", "--git-path", argument],
        ),
        label,
    )
    path = Path(value)
    if not path.is_absolute():
        raise FanoutValidationError(f"{label} is not absolute")
    return path


def reject_present_metadata(path: Path, label: str) -> None:
    if path_entry_exists(path):
        raise FanoutValidationError(f"repository uses forbidden {label}: {path}")


def require_ordinary_directory(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_dir():
        raise FanoutValidationError(f"{label} is not a local ordinary directory")


def git_object_digest(object_type: str, contents: bytes, object_id_length: int) -> str:
    payload = f"{object_type} {len(contents)}\0".encode("ascii") + contents
    if object_id_length == 40:
        return hashlib.sha1(payload, usedforsecurity=False).hexdigest()
    if object_id_length == 64:
        return hashlib.sha256(payload).hexdigest()
    raise FanoutValidationError("repository uses an unsupported Git object format")


def validate_git_object_bytes(
    object_type: str, contents: bytes, expected_object_id: str
) -> None:
    if git_object_digest(object_type, contents, len(expected_object_id)) != expected_object_id:
        raise FanoutValidationError(
            f"Git {object_type} object fails cryptographic identity: {expected_object_id}"
        )


def require_ordinary_object_file(path: Path, label: str) -> os.stat_result:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise FanoutValidationError(f"cannot inspect {label}: {path}") from error
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise FanoutValidationError(
            f"{label} is symlinked, hardlinked, or non-ordinary: {path}"
        )
    return metadata


def validate_loose_object(path: Path, object_id: str) -> None:
    metadata = require_ordinary_object_file(path, "loose Git object")
    if metadata.st_size > MAX_LOOSE_OBJECT_BYTES:
        raise FanoutValidationError(f"loose Git object exceeds the validation cap: {object_id}")
    try:
        compressed = path.read_bytes()
        decompressor = zlib.decompressobj()
        inflated = decompressor.decompress(compressed, MAX_LOOSE_OBJECT_BYTES + 1)
    except (OSError, zlib.error) as error:
        raise FanoutValidationError(f"loose Git object is unreadable: {object_id}") from error
    if (
        len(inflated) > MAX_LOOSE_OBJECT_BYTES
        or not decompressor.eof
        or decompressor.unused_data
        or decompressor.unconsumed_tail
    ):
        raise FanoutValidationError(f"loose Git object is malformed: {object_id}")
    header, separator, contents = inflated.partition(b"\0")
    raw_type, field_separator, raw_size = header.partition(b" ")
    if not separator or not field_separator:
        raise FanoutValidationError(f"loose Git object header is malformed: {object_id}")
    try:
        object_type = raw_type.decode("ascii", errors="strict")
        declared_size = raw_size.decode("ascii", errors="strict")
    except UnicodeDecodeError as error:
        raise FanoutValidationError(
            f"loose Git object header is malformed: {object_id}"
        ) from error
    if (
        object_type not in {"blob", "tree", "commit", "tag"}
        or not declared_size.isdecimal()
        or (declared_size.startswith("0") and declared_size != "0")
        or int(declared_size) != len(contents)
    ):
        raise FanoutValidationError(f"loose Git object header is malformed: {object_id}")
    validate_git_object_bytes(object_type, contents, object_id)


def validate_pack_directory(
    repository: Path, pack_directory: Path, oid_length: int
) -> None:
    require_ordinary_directory(pack_directory, "Git object pack directory")
    packs: dict[str, Path] = {}
    indexes: dict[str, Path] = {}
    pack_name = re.compile(rf"(pack-[0-9a-f]{{{oid_length}}})\.(pack|idx)")
    for child in pack_directory.iterdir():
        require_ordinary_object_file(child, "Git object pack entry")
        if child.name.endswith(".promisor"):
            raise FanoutValidationError(
                f"repository uses a forbidden promisor pack: {child.name}"
            )
        match = pack_name.fullmatch(child.name)
        if match is None:
            continue
        stem, suffix = match.groups()
        target = packs if suffix == "pack" else indexes
        target[stem] = child
    if set(packs) != set(indexes):
        raise FanoutValidationError("Git pack/index set is incomplete")
    for _, pack in sorted(packs.items()):
        git(
            repository,
            ["index-pack", "--verify", "--strict", "--no-rev-index", str(pack)],
        )


def validate_git_config_listing(raw: bytes, label: str) -> None:
    for row in raw.split(b"\0"):
        if not row:
            continue
        key, separator, _ = row.partition(b"\n")
        if not separator:
            raise FanoutValidationError(f"{label} output is malformed")
        try:
            normalized_key = key.decode("ascii", errors="strict").lower()
        except UnicodeDecodeError as error:
            raise FanoutValidationError(f"{label} contains a non-ASCII key") from error
        if normalized_key == "include.path" or normalized_key.startswith("includeif."):
            raise FanoutValidationError(
                f"repository uses forbidden external Git config inclusion: {normalized_key}"
            )
        if normalized_key in FORBIDDEN_DIFF_CONFIG_KEYS or (
            normalized_key.startswith("submodule.")
            and normalized_key.endswith(".ignore")
        ):
            raise FanoutValidationError(
                f"repository uses forbidden diff/submodule-ignore Git config: {normalized_key}"
            )
        if (
            normalized_key in PROMISOR_CONFIG_KEYS
            or (
                normalized_key.startswith("remote.")
                and normalized_key.endswith((".promisor", ".partialclonefilter"))
            )
        ):
            raise FanoutValidationError(
                f"repository uses forbidden promisor/lazy Git config: {normalized_key}"
            )


def validate_local_git_config(
    repository: Path, config_paths: Sequence[Path]
) -> None:
    for config_path in config_paths:
        if not path_entry_exists(config_path):
            continue
        require_ordinary_object_file(config_path, "Git config")
        raw = git(
            repository,
            [
                "config",
                "--file",
                str(config_path),
                "--no-includes",
                "--null",
                "--list",
            ],
        )
        validate_git_config_listing(raw, f"Git config {config_path}")


def validate_repository_object_store(repository: Path) -> None:
    if repository.is_symlink() or not repository.is_dir():
        raise FanoutValidationError("repository is not a local ordinary directory")
    if decoded_git_line(
        git(repository, ["rev-parse", "--is-inside-work-tree"]),
        "worktree status",
    ) != "true":
        raise FanoutValidationError("repository is not an ordinary Git worktree")
    if decoded_git_line(
        git(repository, ["rev-parse", "--is-bare-repository"]),
        "bare-repository status",
    ) != "false":
        raise FanoutValidationError("bare repositories are not valid fanout worktrees")

    common_directory = resolved_git_directory(
        repository, "--git-common-dir", "Git common directory"
    )
    git_directory = resolved_git_directory(repository, "--git-dir", "Git directory")
    if git_directory != common_directory:
        try:
            git_directory.relative_to(common_directory)
        except ValueError as error:
            raise FanoutValidationError(
                "linked-worktree Git directory is outside its common directory"
            ) from error

    objects_path = git_metadata_path(repository, "objects", "Git object directory")
    expected_objects = common_directory / "objects"
    if objects_path.is_symlink():
        raise FanoutValidationError("Git object directory is a symlink")
    try:
        resolved_objects = objects_path.resolve(strict=True)
        resolved_expected = expected_objects.resolve(strict=True)
    except OSError as error:
        raise FanoutValidationError("cannot resolve the Git object directory") from error
    if resolved_objects != resolved_expected:
        raise FanoutValidationError("repository uses an external Git object directory")
    require_ordinary_directory(expected_objects, "Git object directory")

    config_paths = tuple(
        dict.fromkeys(
            (common_directory / "config", git_directory / "config.worktree")
        )
    )
    for config_path in config_paths:
        if config_path.is_symlink():
            raise FanoutValidationError("repository Git config is a symlink")

    for info_root in {common_directory / "info", git_directory / "info"}:
        if info_root.is_symlink():
            raise FanoutValidationError("repository Git info directory is a symlink")
        reject_present_metadata(info_root / "grafts", "Git grafts metadata")
        reject_present_metadata(info_root / "grafts.lock", "Git grafts lock metadata")

    shallow_path = git_metadata_path(repository, "shallow", "Git shallow metadata")
    for path in {
        shallow_path,
        Path(f"{shallow_path}.lock"),
        common_directory / "shallow",
        common_directory / "shallow.lock",
        git_directory / "shallow",
        git_directory / "shallow.lock",
    }:
        reject_present_metadata(path, "shallow metadata")

    validate_local_git_config(repository, config_paths)
    object_format = decoded_git_line(
        git(repository, ["rev-parse", "--show-object-format"]),
        "Git object format",
    )
    if object_format not in {"sha1", "sha256"}:
        raise FanoutValidationError("repository uses an unsupported Git object format")
    oid_length = 40 if object_format == "sha1" else 64
    for child in expected_objects.iterdir():
        if child.is_symlink():
            raise FanoutValidationError(
                f"Git object store contains a symlinked entry: {child.name}"
            )
        if re.fullmatch(r"[0-9a-f]{2}", child.name):
            require_ordinary_directory(child, "Git loose-object fan directory")
            for loose_object in child.iterdir():
                if re.fullmatch(
                    rf"[0-9a-f]{{{oid_length - 2}}}", loose_object.name
                ) is None:
                    raise FanoutValidationError(
                        f"Git loose-object fan contains a noncanonical entry: {loose_object.name}"
                    )
                validate_loose_object(
                    loose_object, f"{child.name}{loose_object.name}"
                )
        elif child.name not in {"info", "pack"}:
            metadata = child.lstat()
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise FanoutValidationError(
                    f"Git object store contains a non-ordinary entry: {child.name}"
                )

    object_info = expected_objects / "info"
    if path_entry_exists(object_info):
        require_ordinary_directory(object_info, "Git objects/info directory")
        for child in object_info.iterdir():
            if child.is_symlink():
                raise FanoutValidationError(
                    f"Git objects/info contains a symlinked entry: {child.name}"
                )
            metadata = child.lstat()
            if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)):
                raise FanoutValidationError(
                    f"Git objects/info contains a non-ordinary entry: {child.name}"
                )
            if stat.S_ISREG(metadata.st_mode) and metadata.st_nlink != 1:
                raise FanoutValidationError(
                    f"Git objects/info contains a hardlinked entry: {child.name}"
                )
            if any(
                child.name == forbidden or child.name.startswith(f"{forbidden}.")
                for forbidden in FORBIDDEN_OBJECT_INFO_NAMES
            ):
                raise FanoutValidationError(
                    f"repository uses forbidden Git object alternates: {child.name}"
                )

    pack_directory = expected_objects / "pack"
    if path_entry_exists(pack_directory):
        validate_pack_directory(repository, pack_directory, oid_length)


def resolve_commit(repository: Path, revision: str) -> str:
    if not revision:
        raise FanoutValidationError("empty Git revision")
    resolved = git(
        repository,
        ["rev-parse", "--verify", "--end-of-options", f"{revision}^{{commit}}"],
    ).decode("ascii", errors="strict").strip()
    if GIT_OBJECT_ID.fullmatch(resolved) is None:
        raise FanoutValidationError(f"Git revision did not resolve to a full commit: {revision}")
    return resolved


def raw_commit_object(repository: Path, commit: str) -> CommitObject:
    if GIT_OBJECT_ID.fullmatch(commit) is None:
        raise FanoutValidationError(f"cannot inspect malformed commit object id: {commit}")
    raw = git(repository, ["cat-file", "commit", commit])
    validate_git_object_bytes("commit", raw, commit)
    header, separator, _ = raw.partition(b"\n\n")
    if not separator:
        raise FanoutValidationError(f"raw commit object has no header terminator: {commit}")
    tree: str | None = None
    parents: list[str] = []
    for line in header.split(b"\n"):
        if line.startswith(b" "):
            continue
        name, field_separator, raw_value = line.partition(b" ")
        if not field_separator or name not in {b"tree", b"parent", b"author", b"committer", b"encoding", b"gpgsig", b"mergetag"}:
            if not field_separator:
                raise FanoutValidationError(f"raw commit header is malformed: {commit}")
            continue
        if name not in {b"tree", b"parent"}:
            continue
        try:
            object_id = raw_value.decode("ascii", errors="strict")
        except UnicodeDecodeError as error:
            raise FanoutValidationError(f"raw commit object id is malformed: {commit}") from error
        if (
            GIT_OBJECT_ID.fullmatch(object_id) is None
            or len(object_id) != len(commit)
        ):
            raise FanoutValidationError(f"raw commit object id is malformed: {commit}")
        if name == b"tree":
            if tree is not None:
                raise FanoutValidationError(f"raw commit has duplicate tree headers: {commit}")
            tree = object_id
        else:
            if object_id in parents:
                raise FanoutValidationError(f"raw commit has duplicate parents: {commit}")
            parents.append(object_id)
    if tree is None:
        raise FanoutValidationError(f"raw commit has no tree: {commit}")
    return CommitObject(tree=tree, parents=tuple(parents))


def tree_entries(repository: Path, tree: str, label: str) -> dict[str, TreeEntry]:
    if GIT_OBJECT_ID.fullmatch(tree) is None:
        raise FanoutValidationError(f"{label} has a malformed tree object id")
    validate_git_object_bytes(
        "tree", git(repository, ["cat-file", "tree", tree]), tree
    )
    raw = git(repository, ["ls-tree", "-r", "--full-tree", "-z", tree])
    rows = raw.split(b"\0")
    if rows and rows[-1] == b"":
        rows.pop()
    entries: list[TreeEntry] = []
    for row in rows:
        metadata, separator, path_bytes = row.partition(b"\t")
        fields = metadata.split(b" ")
        if not separator or len(fields) != 3:
            raise FanoutValidationError(f"{label} tree output is malformed")
        raw_mode, raw_type, raw_object_id = fields
        try:
            mode = raw_mode.decode("ascii", errors="strict")
            object_type = raw_type.decode("ascii", errors="strict")
            object_id = raw_object_id.decode("ascii", errors="strict")
            path = path_bytes.decode("utf-8", errors="strict")
        except UnicodeDecodeError as error:
            raise FanoutValidationError(f"{label} tree output is not canonical UTF-8") from error
        if (
            re.fullmatch(r"[0-7]{6}", mode) is None
            or object_type not in {"blob", "commit"}
            or GIT_OBJECT_ID.fullmatch(object_id) is None
            or len(object_id) != len(tree)
        ):
            raise FanoutValidationError(f"{label} tree output is malformed")
        if object_type != "blob" or mode not in REGULAR_BLOB_MODES:
            raise FanoutValidationError(
                f"{label} contains a forbidden tree entry: {mode} {object_type} {path}"
            )
        entries.append(TreeEntry(mode, object_type, object_id, path))
    validate_tree_path_keys((entry.path for entry in entries), label)
    return {entry.path: entry for entry in entries}


def authenticated_tree(
    repository: Path, commit: str, label: str
) -> tuple[CommitObject, dict[str, TreeEntry]]:
    commit_object = raw_commit_object(repository, commit)
    return commit_object, tree_entries(repository, commit_object.tree, label)


def blob_bytes(repository: Path, entry: TreeEntry, label: str) -> bytes:
    if entry.object_type != "blob" or entry.mode not in REGULAR_BLOB_MODES:
        raise FanoutValidationError(f"{label} is not an ordinary Git blob")
    contents = git(repository, ["cat-file", "blob", entry.object_id])
    validate_git_object_bytes("blob", contents, entry.object_id)
    return contents


def validate_canonical_input_blobs(
    repository: Path,
    fanout_entries: Mapping[str, TreeEntry],
) -> None:
    for raw_path, expected_sha256, expected_byte_length in CANONICAL_INPUTS:
        path = normalized_path(raw_path, prefix=False)
        entry = fanout_entries.get(path)
        if entry is None:
            raise FanoutValidationError(
                f"canonical fanout input is absent from the fanout-base tree: {path}"
            )
        if entry.path != path or entry.object_type != "blob" or entry.mode != "100644":
            raise FanoutValidationError(
                f"canonical fanout input mode or path differs: {path}"
            )
        contents = blob_bytes(repository, entry, f"canonical fanout input {path}")
        if len(contents) != expected_byte_length:
            raise FanoutValidationError(
                f"canonical fanout input byte length differs: {path}"
            )
        if hashlib.sha256(contents).hexdigest() != expected_sha256:
            raise FanoutValidationError(
                f"canonical fanout input SHA-256 differs: {path}"
            )


def validate_inherited_seed_baseline(
    manifest: Mapping[str, Any],
    repository: Path,
    certified_entries: Mapping[str, TreeEntry],
    fanout_entries: Mapping[str, TreeEntry],
) -> None:
    for row in owner_rows(manifest):
        stage = row.get("stage")
        for raw_path in strings(
            row.get("inherited_mutable_seed_files"),
            f"Stage {stage} inherited_mutable_seed_files",
        ):
            path = normalized_path(raw_path, prefix=False)
            certified_entry = certified_entries.get(path)
            fanout_entry = fanout_entries.get(path)
            if certified_entry is None or fanout_entry is None:
                raise FanoutValidationError(
                    f"inherited mutable seed is absent from certified Stage 5 or fanout base: {path}"
                )
            certified_bytes = blob_bytes(
                repository, certified_entry, f"certified Stage-5 inherited seed {path}"
            )
            fanout_bytes = blob_bytes(
                repository, fanout_entry, f"fanout-base inherited seed {path}"
            )
            if (
                certified_entry.mode != fanout_entry.mode
                or certified_entry.object_id != fanout_entry.object_id
                or certified_bytes != fanout_bytes
            ):
                raise FanoutValidationError(
                    f"inherited mutable seed differs between certified Stage 5 and fanout base: {path}"
                )


def parse_raw_changes(raw: bytes) -> list[tuple[str, str, str, str]]:
    fields = raw.split(b"\0")
    if fields and fields[-1] == b"":
        fields.pop()
    if len(fields) % 2 != 0:
        raise FanoutValidationError("git raw diff output is truncated")
    changes: list[tuple[str, str, str, str]] = []
    for index in range(0, len(fields), 2):
        header_bytes = fields[index]
        path_bytes = fields[index + 1]
        try:
            header = header_bytes.decode("ascii")
            path = path_bytes.decode("utf-8")
        except UnicodeDecodeError as error:
            raise FanoutValidationError("git raw diff output is not canonical UTF-8") from error
        fields_in_header = header.split()
        if len(fields_in_header) != 5 or not fields_in_header[0].startswith(":"):
            raise FanoutValidationError("git raw diff header is malformed")
        old_mode = fields_in_header[0][1:]
        new_mode, old_object, new_object, status = fields_in_header[1:]
        if (
            re.fullmatch(r"[0-7]{6}", old_mode) is None
            or re.fullmatch(r"[0-7]{6}", new_mode) is None
            or GIT_OBJECT_ID.fullmatch(old_object) is None
            or GIT_OBJECT_ID.fullmatch(new_object) is None
            or len(old_object) != len(new_object)
            or re.fullmatch(r"[A-Z]", status) is None
        ):
            raise FanoutValidationError("git raw diff header is malformed")
        changes.append((status, path, old_mode, new_mode))
    return changes


def raw_changes_between(
    repository: Path, base: str, tip: str
) -> list[tuple[str, str, str, str]]:
    return parse_raw_changes(
        git(
            repository,
            [
                "diff",
                "--raw",
                "--abbrev=64",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=none",
                base,
                tip,
            ],
        )
    )


def validate_fanout_base_range(
    manifest: Mapping[str, Any],
    repository: Path,
    certified_commit: str,
    fanout_commit: str,
) -> None:
    validate_manifest(manifest)
    expected = expected_fanout_base_changes(manifest)
    changes = raw_changes_between(repository, certified_commit, fanout_commit)
    actual: dict[str, str] = {}
    for status, raw_path, old_mode, new_mode in changes:
        path = normalized_path(raw_path, prefix=False)
        if path in actual:
            raise FanoutValidationError(f"fanout base changed {path} more than once")
        expected_status = expected.get(path)
        if expected_status is None:
            if any(path.startswith(prefix) for prefix in FROZEN_INTERFACE_ROOTS):
                raise FanoutValidationError(
                    f"fanout base changed undeclared frozen fanout interface {path}"
                )
            raise FanoutValidationError(f"fanout base contains unreviewed path {path}")
        if status != expected_status:
            raise FanoutValidationError(
                f"fanout base path {path} uses {status}, expected {expected_status}"
            )
        if new_mode not in REGULAR_BLOB_MODES:
            raise FanoutValidationError(f"fanout base path {path} is not an ordinary blob")
        if status == "A" and old_mode != "000000":
            raise FanoutValidationError(f"fanout base addition {path} was not absent")
        if status == "M" and (
            old_mode not in REGULAR_BLOB_MODES or old_mode != new_mode
        ):
            raise FanoutValidationError(f"fanout base changed object type or mode for {path}")
        actual[path] = status

    if actual != expected:
        missing = sorted(set(expected) - set(actual))
        unexpected = sorted(set(actual) - set(expected))
        raise FanoutValidationError(
            "fanout base path delta differs"
            f"; missing={missing}; unexpected={unexpected}"
        )


def linear_commits_between(
    repository: Path, base: str, tip: str, expected_count: int
) -> list[str]:
    if expected_count < 0 or expected_count > len(STAGES):
        raise FanoutValidationError("hidden comparison range has an invalid checkpoint cap")
    current = tip
    newest_first: list[str] = []
    for _ in range(expected_count):
        if current == base:
            raise FanoutValidationError(
                "hidden comparison range checkpoint count differs"
            )
        commit_object = raw_commit_object(repository, current)
        if len(commit_object.parents) != 1:
            raise FanoutValidationError(
                "hidden comparison range is not a linear single-parent checkpoint chain"
            )
        newest_first.append(current)
        current = commit_object.parents[0]
    if current != base:
        raise FanoutValidationError(
            "hidden comparison range checkpoint count differs or does not reach the fanout base"
        )
    newest_first.reverse()
    return newest_first


def validate_comparison_range(
    manifest: Mapping[str, Any],
    repository: Path,
    stage: int,
    fanout_commit: str,
    comparison_commit: str,
    existing_at_fanout: set[str],
) -> None:
    expected_stages = list(range(STAGES[0], stage))
    commits = linear_commits_between(
        repository, fanout_commit, comparison_commit, len(expected_stages)
    )

    parent = fanout_commit
    for expected_stage, commit in zip(expected_stages, commits, strict=True):
        authenticated_tree(
            repository,
            commit,
            f"hidden comparison range Stage {expected_stage} tree",
        )
        changes = raw_changes_between(repository, parent, commit)
        existing = {
            path
            for _, path, _, _ in changes
            if path in existing_at_fanout
        }
        try:
            validate_changes(manifest, expected_stage, changes, existing)
        except FanoutValidationError as error:
            raise FanoutValidationError(
                f"hidden comparison range Stage {expected_stage} checkpoint is invalid: {error}"
            ) from error
        parent = commit


def validate_candidate(
    repository: Path,
    manifest_path: Path,
    stage: int,
    fanout_base: str,
    comparison_base: str,
    candidate: str,
) -> dict[str, object]:
    validate_repository_object_store(repository)
    fanout_commit = resolve_commit(repository, fanout_base)
    comparison_commit = resolve_commit(repository, comparison_base)
    candidate_commit = resolve_commit(repository, candidate)
    manifest_bytes = manifest_path.read_bytes()
    manifest = load_manifest(manifest_path)
    validate_manifest(manifest)
    certified = cast(Mapping[str, Any], manifest["certified_stage5"])
    certified_commit = cast(str, certified["commit"])
    certified_tree = cast(str, certified["tree"])
    certified_object, certified_entries = authenticated_tree(
        repository, certified_commit, "certified Stage-5 tree"
    )
    if certified_object.tree != certified_tree:
        raise FanoutValidationError("certified Stage-5 tree differs")
    pointer_path = cast(str, certified["publication_pointer"])
    pointer_entry = certified_entries.get(pointer_path)
    if pointer_entry is None or pointer_entry.mode != "100644":
        raise FanoutValidationError("certified Stage-5 pointer mode differs")
    try:
        pointer = json.loads(
            blob_bytes(repository, pointer_entry, "certified Stage-5 pointer")
        )
    except json.JSONDecodeError as error:
        raise FanoutValidationError("certified Stage-5 pointer is not valid JSON") from error
    release_identity = cast(str, certified["release_identity"])
    release_digest = release_identity.removeprefix("sha256:")
    if pointer != {
        "object": f"objects/{release_digest}",
        "release_identity": release_identity,
        "schema_version": "maestro.vnext.proof-publication-pointer.v1",
    }:
        raise FanoutValidationError("certified Stage-5 pointer identity differs")
    release_root = (
        "contracts/vnext/stage5/evidence-gates/releases/objects/"
        f"{release_digest}"
    )
    release_entries = {
        path: entry
        for path, entry in certified_entries.items()
        if path.startswith(f"{release_root}/")
    }
    if not release_entries or any(
        entry.object_type != "blob" or entry.mode != "100644"
        for entry in release_entries.values()
    ):
        raise FanoutValidationError("certified Stage-5 release modes differ")
    release_path = f"{release_root}/release.json"
    snapshot_path = f"{release_root}/payload/workspace-snapshot-manifest.v1.json"
    release_entry = release_entries.get(release_path)
    snapshot_entry = release_entries.get(snapshot_path)
    if release_entry is None or snapshot_entry is None:
        raise FanoutValidationError("certified Stage-5 release path set differs")
    try:
        release = json.loads(
            blob_bytes(repository, release_entry, "certified Stage-5 release")
        )
        snapshot = json.loads(
            blob_bytes(repository, snapshot_entry, "certified Stage-5 snapshot")
        )
    except json.JSONDecodeError as error:
        raise FanoutValidationError("certified Stage-5 release metadata is not valid JSON") from error
    if not isinstance(release, dict) or release.get("identity") != release_identity:
        raise FanoutValidationError("certified Stage-5 release identity differs")
    canonical_release = release.get("canonical_value")
    if (
        not isinstance(canonical_release, dict)
        or canonical_release.get("plan_identity") != certified["plan_identity"]
    ):
        raise FanoutValidationError("certified Stage-5 plan identity differs")
    if (
        not isinstance(snapshot, dict)
        or snapshot.get("snapshot_identity") != certified["snapshot_identity"]
    ):
        raise FanoutValidationError("certified Stage-5 snapshot identity differs")
    fanout_object, fanout_entries = authenticated_tree(
        repository, fanout_commit, "fanout-base tree"
    )
    if fanout_object.parents != (certified_commit,):
        raise FanoutValidationError("fanout base is not the sole direct child of certified Stage 5")
    frozen_manifest_entry = fanout_entries.get(MANIFEST_REPOSITORY_PATH)
    if frozen_manifest_entry is None:
        raise FanoutValidationError("fanout manifest is absent from the fanout-base tree")
    frozen_manifest = blob_bytes(
        repository, frozen_manifest_entry, "frozen fanout manifest"
    )
    if frozen_manifest != manifest_bytes:
        raise FanoutValidationError("fanout manifest differs from the frozen fanout-base blob")
    validate_canonical_input_blobs(repository, fanout_entries)
    validate_inherited_seed_baseline(
        manifest, repository, certified_entries, fanout_entries
    )
    validate_fanout_base_range(
        manifest, repository, certified_commit, fanout_commit
    )
    validate_comparison_range(
        manifest,
        repository,
        stage,
        fanout_commit,
        comparison_commit,
        set(fanout_entries),
    )
    candidate_object, _ = authenticated_tree(
        repository, candidate_commit, f"Stage {stage} candidate tree"
    )
    if candidate_object.parents != (comparison_commit,):
        raise FanoutValidationError(
            "candidate is not the sole direct child of its authenticated comparison base"
        )
    changes = raw_changes_between(repository, comparison_commit, candidate_commit)
    existing = {
        path
        for _, path, _, _ in changes
        if path in fanout_entries
    }
    accepted = validate_changes(manifest, stage, changes, existing)
    return {
        "candidate": candidate_commit,
        "changed_paths": accepted,
        "comparison_base": comparison_commit,
        "fanout_base": fanout_commit,
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "stage": stage,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=MANIFEST_PATH)
    parser.add_argument("--stage", type=int, choices=STAGES, required=True)
    parser.add_argument("--fanout-base", required=True)
    parser.add_argument("--comparison-base", required=True)
    parser.add_argument("--candidate", required=True)
    args = parser.parse_args()
    result = validate_candidate(
        args.repository.resolve(strict=True),
        args.manifest.resolve(strict=True),
        args.stage,
        args.fanout_base,
        args.comparison_base,
        args.candidate,
    )
    print(canonical_json(result).decode("utf-8"), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
