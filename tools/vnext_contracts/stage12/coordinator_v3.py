#!/usr/bin/env python3
"""Validate and perform the sole V8 isolated successor candidate-ref CAS."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, cast


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_CONTRACT = (
    ROOT
    / "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v3.json"
)
SCHEMA = "maestro.external.stage12-legacy-cut-coordinator.v3"
PACKET_IDENTITY = (
    "sha256:d0953ac33f361ccad2fe0c7844294324b7b33cb974e16a11639ad3aad19e40e2"
)
DESIGN = {
    "commit": "bb7b1ee0e51fa591b21943e8c7d50844cb4d0b05",
    "parent": "1685b39138a045bcd5e87744860d95eb589999d2",
    "tree": "cb6b62cc187abdecebef8f621206289029fb590b",
}
IMPLEMENTATION_PREIMAGE = {
    "commit": "1685b39138a045bcd5e87744860d95eb589999d2",
    "tree": "2daa5f8458411cf9e6d6288bf51606c98a4e31c9",
}
PRIMARY = {
    "commit": "13b9a5e9b5ec67e7086b0b21992a207d2e4cde94",
    "tree": "97e08a00f8a721318cda13241129a3b06651accc",
    "boundary_identity": "sha256:e5b4c0592b8cf373ea68fc5e0e3f84020c14f3f422c5779e8d4a423930aa6054",
}
OWNERSHIP_IDENTITY = (
    "699c6b98c8e4f1c8d92bf3a7377759fcc65e685c4f59272c36f13b65b3dc9cfd"
)
INTEGRATION_IDENTITY = (
    "789cd36b82f4e6a0d534833446b9a2c35d6cfafcd96e1123fb9e3215a5df0f29"
)
DESIGN_SOURCE_IDENTITY = (
    "ec045cac1f2fff4fb9d98494ee925fbe86c62dd2c06b4576157cf2b19c1e3bbe"
)
PRIMARY_BINDING_IDENTITY = (
    "04e19005b2c4882d507dfb6e70e1e300f62b57883c08304bbafd9b761e0498c9"
)
IMPLEMENTATION_PREIMAGE_IDENTITY = (
    "85209102c19257928cb7761db01d7bfc86880ac4ec5ab542bceb39dc8f084085"
)
AUTHORITY_SCOPE = (
    "one_packet_bound_expected_preimage_named_isolated_candidate_ref_cas_only"
)
CANDIDATE_REF = (
    "refs/heads/codex/maestro-vnext-legacy-cutover-successor-candidate-v8"
)
REF_UPDATE_ALGORITHM = "git-update-ref-no-deref-new-old"
CRASH_STATES = ["exact_expected_preimage", "exact_declared_postimage"]
GATE_ORDER = (
    ("foundation_legacy_quarantine_closure_v2", "closed_current"),
    ("legacy_source_case_manifest_v3", "current_complete"),
    ("stage12_sighting_manifest_v2", "current_complete"),
    ("migration_classification_manifest_v3", "closed"),
    ("declared_overlap_manifest_v2", "closed_current"),
    ("unavailable_preexisting_loss_manifest_v4", "closed_current"),
    ("unavailable_preexisting_loss_audit_v4", "durable_custody_current"),
    ("sealed_quarantine_manifest_v3", "sealed_current"),
    ("legacy_rollback_assessment_v4", "rehearsed_current"),
    ("legacy_quarantine_epoch_v4", "sealed_current"),
    ("legacy_removal_expected_old_binding_v3", "bound_current"),
    ("legacy_removal_guard_v3", "minted_current"),
    ("replacement_activation", "active_current"),
    ("adapter_parity", "exact"),
    ("consumer_manifest", "zero_current"),
    ("reader_manifest", "zero_current"),
    ("hold_manifest", "zero_current"),
    ("rollback_rehearsal", "rehearsed_current"),
    ("namespace_promotion_manifest", "exact"),
    ("release_currentness", "current"),
    ("proof_registry_currentness", "current"),
)
PACKET_BINDING_KEYS = {
    "approval",
    "design",
    "implementation_preimage",
    "ownership",
    "integration",
    "primary",
    "primary_path_manifest",
    "primary_untracked_manifest",
}
PACKET_BINDINGS = {
    "approval": {
        "path": "packet/build-approval-packet.v8.json",
        "byte_length": 6042,
        "sha256": "sha256:29aba389282d11406daa719a12996d9b7ab2ec113819af65f0b0e7bb71e42e83",
    },
    "design": {
        "path": "packet/design-source-binding.v8.json",
        "byte_length": 9514,
        "sha256": "sha256:e4043056a34516db1dbb98f840b2b523d7588b18c728c8a4e47ff820cce3d76c",
    },
    "implementation_preimage": {
        "path": "packet/implementation-preimage.v8.json",
        "byte_length": 516,
        "sha256": "sha256:b947f1769b278422caddb6034ba88b632ebc619556e63af5072aaa2f50770d4d",
    },
    "ownership": {
        "path": "packet/ownership-manifest.v8.json",
        "byte_length": 17395,
        "sha256": "sha256:af25aa17d5f3a94b8f67bd156c52653dcb0e0e92c4a0db78c07a58e75b54591a",
    },
    "integration": {
        "path": "packet/integration-plan.v8.json",
        "byte_length": 2042,
        "sha256": "sha256:50db9dbfbcc0c393b1db20527e4e2d91eed5abfa196fa525984b6ef33b7bf5e1",
    },
    "primary": {
        "path": "packet/protected-primary-binding.v8.json",
        "byte_length": 1133,
        "sha256": "sha256:09c222f94ae0188c30b59e17e59b216aafb0630d9080b30f95aa6f2abba6f4ff",
    },
    "primary_path_manifest": {
        "path": "packet/protected-primary-dirty-path-manifest.v8.txt",
        "byte_length": 1249,
        "sha256": "sha256:a25912f9899851cc72b39254d62e1c71e289f43b775b7b0d236b7315129d0e83",
    },
    "primary_untracked_manifest": {
        "path": "packet/protected-primary-untracked-regular-files.v8.json",
        "byte_length": 2246,
        "sha256": "sha256:90a700292b24bbfdec5b3932585a6549cbc723b62d71ed922e99e84f752b8083",
    },
}
EFFECT_BOUNDARY = {
    "primary_never_target": True,
    "live_installation_never_target": True,
    "authority_guard_mint_or_reconstruction": False,
    "live_product_path_pruning": False,
    "adapter_activation": False,
    "installation": False,
    "publication": False,
    "release": False,
    "seal": False,
    "receipt": False,
    "pointer": False,
    "proof_runner_candidate_ref_write": False,
    "coordinator_candidate_ref_cas_only": True,
}
SHA1 = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")


class CoordinatorError(RuntimeError):
    """The exact V8 packet-bound cut contract was not satisfied."""


def canonical_bytes(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode("ascii")


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def _reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise CoordinatorError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_contract(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise CoordinatorError("coordinator contract is absent or unsafe")
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise CoordinatorError("coordinator JSON is not canonical UTF-8/LF")
    try:
        value = json.loads(raw, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CoordinatorError(f"invalid coordinator JSON: {error}") from error
    if not isinstance(value, dict):
        raise CoordinatorError("coordinator JSON must be one object")
    result = cast(dict[str, Any], value)
    validate_contract(result)
    return result


def _keys(value: object, expected: set[str], label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != expected:
        raise CoordinatorError(f"{label} fields differ")
    return cast(Mapping[str, Any], value)


def _sha1(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA1.fullmatch(value) is None:
        raise CoordinatorError(f"{label} must be a lowercase SHA-1")
    return value


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256.fullmatch(value) is None:
        raise CoordinatorError(f"{label} must be a prefixed lowercase SHA-256")
    return value


def _absolute(value: object, label: str) -> Path:
    if not isinstance(value, str) or not value.startswith("/") or "\0" in value:
        raise CoordinatorError(f"{label} must be an absolute path")
    path = Path(value)
    if ".." in path.parts:
        raise CoordinatorError(f"{label} contains an unsafe component")
    return path


def _relative(value: object, label: str) -> PurePosixPath:
    if (
        not isinstance(value, str)
        or not value
        or value.startswith("/")
        or "\\" in value
        or any(part in {"", ".", ".."} for part in value.split("/"))
    ):
        raise CoordinatorError(f"{label} must be a portable relative path")
    return PurePosixPath(value)


def _identity(value: object, label: str) -> Mapping[str, str]:
    row = _keys(value, {"commit", "tree"}, label)
    return {
        "commit": _sha1(row["commit"], f"{label} commit"),
        "tree": _sha1(row["tree"], f"{label} tree"),
    }


def _binding(value: object, label: str) -> Mapping[str, Any]:
    row = _keys(value, {"path", "byte_length", "sha256"}, label)
    path = _relative(row["path"], f"{label} path")
    if not (
        path.is_relative_to(PurePosixPath("packet"))
        or path.is_relative_to(PurePosixPath("control/stage12"))
    ):
        raise CoordinatorError(f"{label} escapes approved artifact roots")
    if (
        not isinstance(row["byte_length"], int)
        or isinstance(row["byte_length"], bool)
        or row["byte_length"] < 1
    ):
        raise CoordinatorError(f"{label} byte length is invalid")
    _sha256(row["sha256"], f"{label} sha256")
    return row


def validate_contract(value: Mapping[str, Any]) -> None:
    expected_top = {
        "schema_version",
        "authority_scope",
        "approved_packet_identity",
        "design",
        "implementation_preimage",
        "packet_bindings",
        "protected_primary",
        "candidate_ref",
        "retained_inputs",
        "cas_observation",
        "effect_boundary",
    }
    _keys(value, expected_top, "coordinator")
    if (
        value["schema_version"] != SCHEMA
        or value["authority_scope"] != AUTHORITY_SCOPE
        or value["approved_packet_identity"] != PACKET_IDENTITY
    ):
        raise CoordinatorError("coordinator identity or authority scope differs")
    design = _keys(value["design"], {"commit", "parent", "tree"}, "design")
    if dict(design) != DESIGN:
        raise CoordinatorError("V8 design binding differs")
    if _identity(value["implementation_preimage"], "implementation preimage") != IMPLEMENTATION_PREIMAGE:
        raise CoordinatorError("implementation preimage differs")
    bindings = _keys(value["packet_bindings"], PACKET_BINDING_KEYS, "packet bindings")
    if dict(bindings) != PACKET_BINDINGS:
        raise CoordinatorError("packet artifact bindings differ")
    binding_paths: set[PurePosixPath] = set()
    for name, binding_value in bindings.items():
        binding = _binding(binding_value, f"packet binding {name}")
        path = _relative(binding["path"], f"packet binding {name} path")
        if path in binding_paths:
            raise CoordinatorError("packet bindings alias")
        binding_paths.add(path)
    primary = _keys(
        value["protected_primary"],
        {
            "checkout_realpath",
            "ref",
            "commit",
            "tree",
            "boundary_identity",
            "candidate_target",
        },
        "protected primary",
    )
    primary_path = _absolute(primary["checkout_realpath"], "protected primary checkout")
    if (
        {key: primary[key] for key in ("commit", "tree", "boundary_identity")} != PRIMARY
        or primary["ref"] != "refs/heads/main"
        or primary["candidate_target"] is not False
    ):
        raise CoordinatorError("protected primary binding differs")
    candidate = _keys(
        value["candidate_ref"],
        {
            "repository_realpath",
            "git_common_dir_realpath",
            "ref",
            "expected_preimage",
            "declared_postimage",
            "declared_postimage_parent",
            "ref_update_algorithm",
            "crash_states",
        },
        "candidate ref",
    )
    candidate_path = _absolute(candidate["repository_realpath"], "candidate repository")
    _absolute(candidate["git_common_dir_realpath"], "candidate Git common directory")
    if (
        candidate["ref"] != CANDIDATE_REF
        or candidate["ref"] == primary["ref"]
        or candidate_path == primary_path
        or candidate_path.is_relative_to(primary_path)
    ):
        raise CoordinatorError("candidate target is not the one isolated successor ref")
    expected = _identity(candidate["expected_preimage"], "candidate expected preimage")
    declared = _identity(candidate["declared_postimage"], "candidate declared postimage")
    if (
        candidate["declared_postimage_parent"] != expected["commit"]
        or expected == declared
        or candidate["ref_update_algorithm"] != REF_UPDATE_ALGORITHM
        or candidate["crash_states"] != CRASH_STATES
    ):
        raise CoordinatorError("candidate preimage, postimage, or CAS algorithm differs")
    gates = value["retained_inputs"]
    if not isinstance(gates, list) or len(gates) != len(GATE_ORDER):
        raise CoordinatorError("retained input count differs")
    identities: set[str] = set()
    evidence_paths: set[PurePosixPath] = set()
    for index, ((expected_kind, expected_state), gate_value) in enumerate(
        zip(GATE_ORDER, gates, strict=True)
    ):
        expected_fields = {"kind", "state", "identity", "evidence"}
        if expected_kind in {"consumer_manifest", "reader_manifest", "hold_manifest"}:
            expected_fields.add("count")
        if expected_kind == "namespace_promotion_manifest":
            expected_fields.update({"entry_count", "mismatch_count"})
        gate = _keys(gate_value, expected_fields, f"retained input {index}")
        if (gate["kind"], gate["state"]) != (expected_kind, expected_state):
            raise CoordinatorError("retained input order or state differs")
        identity = _sha256(gate["identity"], f"{expected_kind} identity")
        evidence = _binding(gate["evidence"], f"{expected_kind} evidence")
        path = _relative(evidence["path"], f"{expected_kind} evidence path")
        if identity in identities or path in evidence_paths:
            raise CoordinatorError("retained input identity or path is duplicated")
        identities.add(identity)
        evidence_paths.add(path)
        if "count" in gate and gate["count"] != 0:
            raise CoordinatorError(f"{expected_kind} is not zero")
        if expected_kind == "namespace_promotion_manifest" and (
            gate["entry_count"] != 210 or gate["mismatch_count"] != 0
        ):
            raise CoordinatorError("namespace promotion is not exact")
    observation = _keys(
        value["cas_observation"],
        {"state", "observed_commit", "observed_tree"},
        "CAS observation",
    )
    state = observation["state"]
    if state not in {"not_executed", *CRASH_STATES}:
        raise CoordinatorError("CAS observation state differs")
    observed = (
        _sha1(observation["observed_commit"], "observed commit"),
        _sha1(observation["observed_tree"], "observed tree"),
    )
    if state in {"not_executed", "exact_expected_preimage"} and observed != (
        expected["commit"],
        expected["tree"],
    ):
        raise CoordinatorError("expected-preimage observation differs")
    if state == "exact_declared_postimage" and observed != (
        declared["commit"],
        declared["tree"],
    ):
        raise CoordinatorError("declared-postimage observation differs")
    if value["effect_boundary"] != EFFECT_BOUNDARY:
        raise CoordinatorError("coordinator effect boundary differs")


def _bound_path(root: Path, binding_value: object, label: str) -> Path:
    binding = _binding(binding_value, label)
    path = root.joinpath(*_relative(binding["path"], f"{label} path").parts)
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise CoordinatorError(f"{label} is absent") from error
    if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
        raise CoordinatorError(f"{label} is not a regular non-symlink file")
    raw = path.read_bytes()
    if len(raw) != binding["byte_length"] or digest(raw) != binding["sha256"]:
        raise CoordinatorError(f"{label} bytes differ")
    return path


def _bound_json(path: Path, label: str) -> Mapping[str, Any]:
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise CoordinatorError(f"{label} JSON is not canonical")
    try:
        value = json.loads(raw, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CoordinatorError(f"{label} JSON is invalid: {error}") from error
    if not isinstance(value, Mapping):
        raise CoordinatorError(f"{label} JSON is not an object")
    return cast(Mapping[str, Any], value)


def validate_bound_artifacts(value: Mapping[str, Any], artifact_root: Path) -> None:
    validate_contract(value)
    root = artifact_root.resolve(strict=True)
    bindings = cast(Mapping[str, Any], value["packet_bindings"])
    verified: dict[str, Path] = {}
    resolved: set[Path] = set()
    for name, binding in bindings.items():
        path = _bound_path(root, binding, f"packet {name}").resolve(strict=True)
        if not path.is_relative_to(root) or path in resolved:
            raise CoordinatorError("packet artifact escapes or aliases")
        resolved.add(path)
        verified[name] = path
    for gate in cast(list[Mapping[str, Any]], value["retained_inputs"]):
        label = str(gate["kind"])
        path = _bound_path(root, gate["evidence"], label).resolve(strict=True)
        if not path.is_relative_to(root) or path in resolved:
            raise CoordinatorError("retained artifact escapes or aliases")
        resolved.add(path)
    approval = _bound_json(verified["approval"], "approval packet")
    design = _bound_json(verified["design"], "design binding")
    preimage = _bound_json(verified["implementation_preimage"], "preimage binding")
    ownership = _bound_json(verified["ownership"], "ownership binding")
    integration = _bound_json(verified["integration"], "integration binding")
    primary = _bound_json(verified["primary"], "primary binding")
    if (
        approval.get("schema") != "maestro.external-build-approval-packet.v8"
        or approval.get("packet_sha256") != PACKET_IDENTITY.removeprefix("sha256:")
        or approval.get("locked_design") != DESIGN
        or approval.get("implementation_preimage") != IMPLEMENTATION_PREIMAGE
        or approval.get("ownership_identity") != OWNERSHIP_IDENTITY
        or approval.get("integration_plan_identity") != INTEGRATION_IDENTITY
        or approval.get("shared_file_authority") != "MainIntegration"
    ):
        raise CoordinatorError("V8 approval packet semantics differ")
    if (
        design.get("schema") != "maestro.external.v8-design-source-binding.v1"
        or design.get("identity_sha256") != DESIGN_SOURCE_IDENTITY
        or design.get("design") != DESIGN
        or preimage.get("schema") != "maestro.external.v8-implementation-preimage.v1"
        or preimage.get("identity_sha256") != IMPLEMENTATION_PREIMAGE_IDENTITY
        or {
            "commit": preimage.get("commit"),
            "tree": preimage.get("tree"),
        }
        != IMPLEMENTATION_PREIMAGE
    ):
        raise CoordinatorError("V8 design or implementation-preimage binding differs")
    lanes = ownership.get("lanes")
    external = (
        next(
            (lane for lane in lanes if lane.get("owner") == "ExternalProofControl"),
            None,
        )
        if isinstance(lanes, list)
        else None
    )
    if (
        ownership.get("schema") != "maestro.external.v8-ownership-manifest.v1"
        or ownership.get("identity_sha256") != OWNERSHIP_IDENTITY
        or ownership.get("default_deny_unowned") is not True
        or ownership.get("writable_prefixes") != []
        or not isinstance(external, Mapping)
        or external.get("counts") != {"existing": 6, "planned_new": 7, "total": 13}
    ):
        raise CoordinatorError("V8 ownership binding differs")
    coordinator = integration.get("coordinator_boundary")
    if (
        integration.get("schema") != "maestro.external.v8-linear-integration-plan.v1"
        or integration.get("identity_sha256") != INTEGRATION_IDENTITY
        or integration.get("main_integration_checkpoints")
        != [
            "owner_wiring",
            "foundation_v4_wiring",
            "guard_coordinator_wiring",
            "final_closure",
        ]
        or not isinstance(coordinator, Mapping)
        or coordinator.get("name") != "Stage12LegacyCutCoordinatorV3"
        or coordinator.get("owner") != "ExternalProofControl"
        or coordinator.get("mutation_count") != 1
        or coordinator.get("primary_target") is not False
        or coordinator.get("proof_runner_effect_inert") is not True
    ):
        raise CoordinatorError("V8 integration or coordinator binding differs")
    if (
        primary.get("schema") != "maestro.external.v8-protected-primary-binding.v1"
        or primary.get("identity_sha256") != PRIMARY_BINDING_IDENTITY
        or {
            "commit": primary.get("commit"),
            "tree": primary.get("tree"),
            "boundary_identity": f"sha256:{primary.get('boundary_identity')}",
        }
        != PRIMARY
        or primary.get("policy")
        != "read_only_never_stage_stash_reset_clean_normalize_overwrite_or_target"
    ):
        raise CoordinatorError("protected-primary packet binding differs")


def _git_read(repository: Path, *arguments: str, binary: bool = False) -> str | bytes:
    allowed = {
        "cat-file",
        "check-ref-format",
        "diff",
        "for-each-ref",
        "merge-base",
        "rev-list",
        "rev-parse",
        "status",
    }
    if not arguments or arguments[0] not in allowed:
        raise CoordinatorError("non-read Git operation reached read boundary")
    result = subprocess.run(
        ["git", "--no-optional-locks", "-C", str(repository), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={**os.environ, "LC_ALL": "C", "LANG": "C"},
        text=not binary,
    )
    if result.returncode != 0:
        stderr = (
            result.stderr.decode(errors="replace")
            if isinstance(result.stderr, bytes)
            else result.stderr
        )
        raise CoordinatorError(f"Git read failed: {stderr.strip()}")
    return result.stdout


def _tree(repository: Path, commit: str) -> str:
    return _sha1(
        str(_git_read(repository, "rev-parse", f"{commit}^{{tree}}")).strip(),
        "observed tree",
    )


def _candidate_repository(value: Mapping[str, Any], repository: Path) -> Path:
    candidate = cast(Mapping[str, Any], value["candidate_ref"])
    primary = cast(Mapping[str, Any], value["protected_primary"])
    resolved = repository.resolve(strict=True)
    primary_path = Path(str(primary["checkout_realpath"]))
    if (
        str(resolved) != candidate["repository_realpath"]
        or resolved == primary_path
        or resolved.is_relative_to(primary_path)
    ):
        raise CoordinatorError("candidate repository is not the isolated successor")
    common_raw = str(_git_read(resolved, "rev-parse", "--git-common-dir")).strip()
    common = (
        (resolved / common_raw).resolve()
        if not Path(common_raw).is_absolute()
        else Path(common_raw).resolve()
    )
    if str(common) != candidate["git_common_dir_realpath"]:
        raise CoordinatorError("candidate Git common directory differs")
    if str(_git_read(resolved, "check-ref-format", CANDIDATE_REF)).strip():
        raise CoordinatorError("candidate ref format differs")
    if str(
        _git_read(resolved, "for-each-ref", "--format=%(symref)", CANDIDATE_REF)
    ).strip():
        raise CoordinatorError("candidate ref may not be symbolic")
    return resolved


def _validate_candidate_ancestry(value: Mapping[str, Any], repository: Path) -> None:
    candidate = cast(Mapping[str, Any], value["candidate_ref"])
    expected = cast(Mapping[str, str], candidate["expected_preimage"])
    declared = cast(Mapping[str, str], candidate["declared_postimage"])
    if _tree(repository, DESIGN["commit"]) != DESIGN["tree"]:
        raise CoordinatorError("V8 design tree differs")
    design_row = str(
        _git_read(repository, "rev-list", "--parents", "--max-count=1", DESIGN["commit"])
    ).split()
    if design_row != [DESIGN["commit"], DESIGN["parent"]]:
        raise CoordinatorError("V8 design is not the direct implementation-preimage child")
    ancestry = subprocess.run(
        [
            "git",
            "--no-optional-locks",
            "-C",
            str(repository),
            "merge-base",
            "--is-ancestor",
            DESIGN["commit"],
            expected["commit"],
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if ancestry.returncode != 0:
        raise CoordinatorError("candidate expected preimage lacks the V8 design")
    if _tree(repository, expected["commit"]) != expected["tree"]:
        raise CoordinatorError("candidate expected-preimage tree differs")
    if _tree(repository, declared["commit"]) != declared["tree"]:
        raise CoordinatorError("candidate declared-postimage tree differs")
    postimage_row = str(
        _git_read(
            repository,
            "rev-list",
            "--parents",
            "--max-count=1",
            declared["commit"],
        )
    ).split()
    if postimage_row != [declared["commit"], expected["commit"]]:
        raise CoordinatorError("candidate declared postimage is not the direct child")


def _serialize_status(status: str) -> bytes:
    rows = []
    for line in status.splitlines():
        if len(line) < 4:
            raise CoordinatorError("protected-primary status row is malformed")
        rows.append((line[:2], line[3:]))
    return (
        "schema\tExternalPrimaryDirtyPathManifestV1\n"
        + "\n".join(f"{code}\t{path}" for code, path in rows)
        + "\n"
    ).encode("utf-8")


def verify_protected_primary_currentness(
    value: Mapping[str, Any], artifact_root: Path
) -> None:
    bindings = cast(Mapping[str, Any], value["packet_bindings"])
    root = artifact_root.resolve(strict=True)
    primary_binding = _bound_json(
        _bound_path(root, bindings["primary"], "primary binding"),
        "primary binding",
    )
    primary = cast(Mapping[str, Any], value["protected_primary"])
    repository = Path(str(primary["checkout_realpath"])).resolve(strict=True)
    if (
        str(_git_read(repository, "rev-parse", "HEAD")).strip() != PRIMARY["commit"]
        or str(_git_read(repository, "rev-parse", "HEAD^{tree}")).strip()
        != PRIMARY["tree"]
    ):
        raise CoordinatorError("protected primary Git identity drifted")
    status = str(
        _git_read(repository, "status", "--porcelain=v1", "--untracked-files=all")
    )
    path_manifest = _bound_path(
        root, bindings["primary_path_manifest"], "primary path manifest"
    ).read_bytes()
    if _serialize_status(status) != path_manifest:
        raise CoordinatorError("protected primary dirty-path boundary drifted")
    diff = cast(
        bytes,
        _git_read(
            repository,
            "diff",
            "--no-ext-diff",
            "--binary",
            "HEAD",
            "--",
            binary=True,
        ),
    )
    if hashlib.sha256(diff).hexdigest() != primary_binding.get(
        "tracked_binary_diff_sha256"
    ):
        raise CoordinatorError("protected primary tracked diff drifted")
    untracked = _bound_json(
        _bound_path(
            root, bindings["primary_untracked_manifest"], "primary untracked manifest"
        ),
        "primary untracked manifest",
    )
    if untracked.get("identity_sha256") != primary_binding.get(
        "untracked_manifest_identity"
    ):
        raise CoordinatorError("protected primary untracked identity differs")
    for expected in cast(list[Mapping[str, Any]], untracked.get("files")):
        path = repository / str(expected["path"])
        if path.is_symlink() or not path.is_file():
            raise CoordinatorError("protected primary untracked file is unsafe")
        raw = path.read_bytes()
        if {
            "path": expected["path"],
            "length": len(raw),
            "mode": stat.S_IMODE(path.lstat().st_mode),
            "sha256": hashlib.sha256(raw).hexdigest(),
        } != expected:
            raise CoordinatorError("protected primary untracked file drifted")


def observe_candidate_ref(
    value: Mapping[str, Any], artifact_root: Path, candidate_repository: Path
) -> dict[str, Any]:
    validate_bound_artifacts(value, artifact_root)
    repository = _candidate_repository(value, candidate_repository)
    _validate_candidate_ancestry(value, repository)
    candidate = cast(Mapping[str, Any], value["candidate_ref"])
    expected = cast(Mapping[str, str], candidate["expected_preimage"])
    declared = cast(Mapping[str, str], candidate["declared_postimage"])
    observed_commit = str(
        _git_read(repository, "rev-parse", "--verify", CANDIDATE_REF)
    ).strip()
    observed_tree = _tree(repository, observed_commit)
    if (observed_commit, observed_tree) == (expected["commit"], expected["tree"]):
        state = "exact_expected_preimage"
    elif (observed_commit, observed_tree) == (
        declared["commit"],
        declared["tree"],
    ):
        state = "exact_declared_postimage"
    else:
        raise CoordinatorError(
            "candidate crash state is neither exact preimage nor postimage"
        )
    result = copy.deepcopy(dict(value))
    result["cas_observation"] = {
        "state": state,
        "observed_commit": observed_commit,
        "observed_tree": observed_tree,
    }
    validate_contract(result)
    return result


def _update_candidate_ref_once(
    repository: Path, ref: str, postimage: str, preimage: str
) -> None:
    result = subprocess.run(
        ["git", "-C", str(repository), "update-ref", "--no-deref", ref, postimage, preimage],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={**os.environ, "LC_ALL": "C", "LANG": "C"},
        text=True,
    )
    if result.returncode != 0:
        raise CoordinatorError(
            f"isolated candidate-ref CAS refused: {result.stderr.strip()}"
        )


def execute_isolated_candidate_ref_cas(
    value: Mapping[str, Any], artifact_root: Path, candidate_repository: Path
) -> dict[str, Any]:
    observed = observe_candidate_ref(value, artifact_root, candidate_repository)
    if observed["cas_observation"]["state"] == "exact_declared_postimage":
        return observed
    repository = _candidate_repository(observed, candidate_repository)
    verify_protected_primary_currentness(observed, artifact_root)
    candidate = cast(Mapping[str, Any], observed["candidate_ref"])
    expected = cast(Mapping[str, str], candidate["expected_preimage"])
    declared = cast(Mapping[str, str], candidate["declared_postimage"])
    _update_candidate_ref_once(
        repository,
        CANDIDATE_REF,
        declared["commit"],
        expected["commit"],
    )
    completed = observe_candidate_ref(observed, artifact_root, repository)
    if completed["cas_observation"]["state"] != "exact_declared_postimage":
        raise CoordinatorError("candidate-ref CAS did not reach the declared postimage")
    return completed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    args = parser.parse_args()
    try:
        value = load_contract(args.contract)
    except (OSError, CoordinatorError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    print(
        json.dumps(
            {
                "authority_state": "not_executed",
                "candidate_ref": value["candidate_ref"]["ref"],
                "effect_state": "contract_validation_only",
                "schema_version": value["schema_version"],
                "status": "pass",
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.dont_write_bytecode = True
    raise SystemExit(main())
