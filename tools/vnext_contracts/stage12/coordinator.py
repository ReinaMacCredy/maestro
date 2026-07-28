#!/usr/bin/env python3
"""Validate and perform the one isolated Stage 12 candidate-ref CAS."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import stat
import subprocess
from pathlib import Path, PurePosixPath
from typing import Any, Mapping

if __package__:
    from . import protected_primary
else:
    import protected_primary


SCHEMA = "maestro.external.stage12-legacy-cut-coordinator.v2"
PACKET_IDENTITY = (
    "sha256:171de6121c62f1c8af55e9e248da506ca96322cb5a588c75ee3762f7d8082472"
)
SOURCE_BINDING_IDENTITY = (
    "sha256:1099c62b3c9a333da68733a098ceece9e6754f28f1ea53f30b4b8dfcc6ae92d7"
)
PRIMARY_BOUNDARY_IDENTITY = (
    "sha256:e5b4c0592b8cf373ea68fc5e0e3f84020c14f3f422c5779e8d4a423930aa6054"
)
PROTECTED_PRIMARY_BINDING_IDENTITY = (
    "bf5daf86bfdea2fed211da1b49abb51c62a17d08ade0b28eee0c4c8b68d0718e"
)
CLEAN_SUCCESSOR = {
    "commit": "e69295329c29c1c75901315a56e947b85b7a69cf",
    "tree": "cd36cbb2963a264cb67a834bb38c709c0ea144ae",
}
V7_DESIGN = {
    "commit": "ff454521b7037d5df7b8e836b8ce30f77e1ff8dc",
    "tree": "bd2c08f87809d5093252943f2fd04a5be551aa13",
}
PRIMARY_IDENTITY = {
    "commit": "13b9a5e9b5ec67e7086b0b21992a207d2e4cde94",
    "tree": "97e08a00f8a721318cda13241129a3b06651accc",
}
CANONICAL_ANCESTRY = [
    {
        "lane": "V7Design",
        "commit": "ff454521b7037d5df7b8e836b8ce30f77e1ff8dc",
        "tree": "bd2c08f87809d5093252943f2fd04a5be551aa13",
    },
    {
        "lane": "Stage11",
        "commit": "66ba4bf8470ee63b81a77bddc0f9d83e6cc4961c",
        "tree": "f697a328de1b0271bcc266f9cb12a7d1c9ef24a3",
    },
    {
        "lane": "MainIntegrationStage11Wiring",
        "commit": "0c27ccfe2c939b50ac2f99a9349d0aa56d065ff7",
        "tree": "4ed2c96e071532d275088d9b8089cccaebec0de9",
    },
    {
        "lane": "AuthorityOwner",
        "commit": "fc190ce78d940475073b0451c349f52016380d3c",
        "tree": "e32bcc029e96cfbcb0f805527c19bc1efbf964af",
    },
    {
        "lane": "Stage12Product",
        "commit": "73e2d226f51ac55ee9a92b411fade9b7737fa567",
        "tree": "9bca7075c5255b9bb3eb757693c7f13b8d294b19",
    },
    {
        "lane": "AuthorityOwnerModulePlacementCorrection",
        "commit": "acd2a469d058f5a17162d3f0a5a44fe394cf6676",
        "tree": "b97282eadfc10ad552cdc5b46bef7b62454367ef",
    },
    {
        "lane": "Stage12ProductAffectedSuffixRebind",
        "commit": "e03d21b64995a20cfda3e90d706048ca79038f30",
        "tree": "600171763b9e782d494fa0c04ba5de9a5d7fa5a4",
    },
]
AUTHORITY_SCOPE = "one_expected_preimage_isolated_candidate_ref_cas_only"
REF_UPDATE_ALGORITHM = "git-update-ref-no-deref-new-old"
CRASH_STATES = ["exact_expected_preimage", "exact_declared_postimage"]
GATE_ORDER = (
    ("legacy_source_case_manifest_v3", "current_complete"),
    ("stage12_sighting_manifest_v2", "current_complete"),
    ("migration_classification_manifest_v3", "closed"),
    ("declared_overlap_manifest_v2", "closed_current"),
    ("unavailable_preexisting_loss_manifest_v3", "closed_current"),
    ("sealed_quarantine_manifest_v3", "sealed_current"),
    ("legacy_quarantine_epoch_v3", "sealed_current"),
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
EFFECT_BOUNDARY = {
    "primary_never_target": True,
    "authority_guard_mint_or_reconstruction": False,
    "live_product_path_pruning": False,
    "adapter_activation": False,
    "installation": False,
    "publication": False,
    "release": False,
    "final_runner_candidate_ref_write": False,
}
SHA1 = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
REF = re.compile(r"^refs/heads/[A-Za-z0-9][A-Za-z0-9._/-]*$")


class CoordinatorError(RuntimeError):
    """The exact packet-bound Stage 12 cut contract was not satisfied."""


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
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise CoordinatorError("coordinator JSON is not canonical UTF-8/LF")
    try:
        value = json.loads(raw, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CoordinatorError(f"invalid coordinator JSON: {error}") from error
    if not isinstance(value, dict):
        raise CoordinatorError("coordinator JSON must be an object")
    validate_contract(value)
    return value


def _keys(value: object, expected: set[str], label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != expected:
        raise CoordinatorError(f"{label} fields differ")
    return value


def _sha1(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA1.fullmatch(value) is None:
        raise CoordinatorError(f"{label} must be a lowercase SHA-1 identity")
    return value


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256.fullmatch(value) is None:
        raise CoordinatorError(f"{label} must be a prefixed lowercase SHA-256 identity")
    return value


def _absolute(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.startswith("/") or "\0" in value:
        raise CoordinatorError(f"{label} must be an absolute path")
    return value


def _relative(value: object, label: str) -> PurePosixPath:
    if (
        not isinstance(value, str)
        or not value
        or value.startswith("/")
        or "\\" in value
    ):
        raise CoordinatorError(f"{label} must be a portable relative path")
    if any(part in {"", ".", ".."} for part in value.split("/")):
        raise CoordinatorError(f"{label} contains an unsafe component")
    return PurePosixPath(value)


def _binding(value: object, label: str) -> Mapping[str, Any]:
    row = _keys(value, {"path", "byte_length", "sha256"}, label)
    path = _relative(row["path"], f"{label} path")
    if not path.is_relative_to(PurePosixPath("control/stage12")):
        raise CoordinatorError(f"{label} escapes the Stage 12 control root")
    if not isinstance(row["byte_length"], int) or row["byte_length"] < 1:
        raise CoordinatorError(f"{label} byte length is invalid")
    _sha256(row["sha256"], f"{label} digest")
    return row


def _identity(value: object, label: str) -> Mapping[str, Any]:
    row = _keys(value, {"commit", "tree"}, label)
    _sha1(row["commit"], f"{label} commit")
    _sha1(row["tree"], f"{label} tree")
    return row


def validate_contract(value: Mapping[str, Any]) -> None:
    expected_top = {
        "schema_version",
        "authority_scope",
        "approved_packet_identity",
        "approved_packet",
        "protected_primary",
        "source_git_binding",
        "canonical_ancestry",
        "clean_successor_preimage",
        "candidate_ref",
        "retained_inputs",
        "cas_observation",
        "effect_boundary",
    }
    _keys(value, expected_top, "coordinator")
    if value["schema_version"] != SCHEMA:
        raise CoordinatorError("coordinator schema differs")
    if value["authority_scope"] != AUTHORITY_SCOPE:
        raise CoordinatorError("coordinator authority scope differs")
    if value["approved_packet_identity"] != PACKET_IDENTITY:
        raise CoordinatorError("approved packet identity differs")
    approved_packet = _binding(value["approved_packet"], "approved packet")
    if approved_packet != {
        "path": "control/stage12/packet/replacement-build-approval-packet.v7.json",
        "byte_length": 10927,
        "sha256": "sha256:0c525951a49c7406d1008c64a3ad328505777c09cb7388b11a5db8634c3f4f65",
    }:
        raise CoordinatorError("approved packet artifact binding differs")

    primary = _keys(
        value["protected_primary"],
        {
            "checkout_realpath",
            "ref",
            "commit",
            "tree",
            "boundary_identity",
            "boundary",
            "candidate_target",
        },
        "protected primary",
    )
    _absolute(primary["checkout_realpath"], "protected primary checkout")
    _validate_ref(primary["ref"], "protected primary ref")
    if {key: primary[key] for key in ("commit", "tree")} != PRIMARY_IDENTITY:
        raise CoordinatorError("protected primary commit or tree differs")
    if primary["boundary_identity"] != PRIMARY_BOUNDARY_IDENTITY:
        raise CoordinatorError("protected primary boundary identity differs")
    primary_boundary = _binding(
        primary["boundary"], "protected primary boundary"
    )
    if primary_boundary != {
        "path": "control/stage12/packet/primary-dirty-boundary.v7.json",
        "byte_length": 1126,
        "sha256": "sha256:4f4ec8207a5f5824c9113cca1a3b04cf390f2bf731f1188e399ba56d8ad6c26a",
    }:
        raise CoordinatorError("protected primary boundary artifact differs")
    if primary["candidate_target"] is not False:
        raise CoordinatorError("protected primary became a candidate target")

    source = _keys(
        value["source_git_binding"],
        {
            "identity",
            "repository_realpath",
            "git_common_dir_realpath",
            "object_format",
            "artifact",
        },
        "source Git binding",
    )
    if source["identity"] != SOURCE_BINDING_IDENTITY:
        raise CoordinatorError("source Git binding identity differs")
    _absolute(source["repository_realpath"], "source repository")
    _absolute(source["git_common_dir_realpath"], "source Git common directory")
    if source["object_format"] != "sha1":
        raise CoordinatorError("source Git object format differs")
    source_artifact = _binding(
        source["artifact"], "source Git binding artifact"
    )
    if source_artifact != {
        "path": "control/stage12/packet/source-git-control-binding.v7.json",
        "byte_length": 1706,
        "sha256": "sha256:7d73e0746497566712a1c6782c8d3435627aa2c7b997e59ceaea1b32756e792d",
    }:
        raise CoordinatorError("source Git binding artifact differs")

    if value["canonical_ancestry"] != CANONICAL_ANCESTRY:
        raise CoordinatorError("canonical lane ancestry differs")

    clean = _identity(value["clean_successor_preimage"], "clean successor preimage")
    if dict(clean) != CLEAN_SUCCESSOR:
        raise CoordinatorError("clean successor preimage differs")

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
    repository = _absolute(candidate["repository_realpath"], "candidate repository")
    _absolute(candidate["git_common_dir_realpath"], "candidate Git common directory")
    candidate_ref = _validate_ref(candidate["ref"], "candidate ref")
    preimage = _identity(candidate["expected_preimage"], "candidate expected preimage")
    postimage = _identity(
        candidate["declared_postimage"], "candidate declared postimage"
    )
    parent = _sha1(candidate["declared_postimage_parent"], "declared postimage parent")
    if parent != preimage["commit"]:
        raise CoordinatorError(
            "declared postimage parent differs from expected preimage"
        )
    if preimage == postimage:
        raise CoordinatorError("candidate preimage and postimage are identical")
    if candidate["ref_update_algorithm"] != REF_UPDATE_ALGORITHM:
        raise CoordinatorError("candidate ref update algorithm differs")
    if candidate["crash_states"] != CRASH_STATES:
        raise CoordinatorError("candidate crash-state closure differs")
    if candidate_ref == primary["ref"]:
        raise CoordinatorError("candidate ref targets the protected primary ref")
    if Path(repository) == Path(str(primary["checkout_realpath"])):
        raise CoordinatorError(
            "candidate repository targets the protected primary checkout"
        )

    gates = value["retained_inputs"]
    if not isinstance(gates, list) or len(gates) != len(GATE_ORDER):
        raise CoordinatorError("retained input count differs")
    identities: set[str] = set()
    paths: set[PurePosixPath] = set()
    for index, ((expected_kind, expected_state), gate_value) in enumerate(
        zip(GATE_ORDER, gates, strict=True)
    ):
        expected_keys = {"kind", "state", "identity", "evidence"}
        if expected_kind in {"consumer_manifest", "reader_manifest", "hold_manifest"}:
            expected_keys.add("count")
        if expected_kind == "namespace_promotion_manifest":
            expected_keys.update({"entry_count", "mismatch_count"})
        gate = _keys(gate_value, expected_keys, f"retained input {index}")
        if (gate["kind"], gate["state"]) != (expected_kind, expected_state):
            raise CoordinatorError("retained input order or state differs")
        identity = _sha256(gate["identity"], f"{expected_kind} identity")
        evidence = _binding(gate["evidence"], f"{expected_kind} evidence")
        path = _relative(evidence["path"], f"{expected_kind} evidence path")
        if identity in identities or path in paths:
            raise CoordinatorError("retained input identity or path is duplicated")
        identities.add(identity)
        paths.add(path)
        if "count" in gate and gate["count"] != 0:
            raise CoordinatorError(f"{expected_kind} is not zero")
        if expected_kind == "namespace_promotion_manifest" and (
            gate["entry_count"] != 210 or gate["mismatch_count"] != 0
        ):
            raise CoordinatorError("namespace promotion is not exact 210-entry parity")

    observation = _keys(
        value["cas_observation"],
        {"state", "observed_commit", "observed_tree"},
        "CAS observation",
    )
    if observation["state"] not in {"not_executed", *CRASH_STATES}:
        raise CoordinatorError("CAS observation state differs")
    _sha1(observation["observed_commit"], "CAS observed commit")
    _sha1(observation["observed_tree"], "CAS observed tree")
    if observation["state"] in {"not_executed", "exact_expected_preimage"} and (
        observation["observed_commit"],
        observation["observed_tree"],
    ) != (preimage["commit"], preimage["tree"]):
        raise CoordinatorError("preimage observation bytes differ")
    if observation["state"] == "exact_declared_postimage" and (
        observation["observed_commit"],
        observation["observed_tree"],
    ) != (postimage["commit"], postimage["tree"]):
        raise CoordinatorError("postimage observation bytes differ")
    if value["effect_boundary"] != EFFECT_BOUNDARY:
        raise CoordinatorError("coordinator effect boundary differs")


def _validate_ref(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or REF.fullmatch(value) is None
        or ".." in value
        or value.endswith((".", "/"))
        or "@{" in value
        or "//" in value
    ):
        raise CoordinatorError(f"{label} is unsafe")
    return value


def _bound_path(root: Path, binding: object, label: str) -> Path:
    row = _binding(binding, label)
    path = root.joinpath(*_relative(row["path"], f"{label} path").parts)
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise CoordinatorError(f"{label} is absent") from error
    if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
        raise CoordinatorError(f"{label} is not a regular non-symlink file")
    raw = path.read_bytes()
    if len(raw) != row["byte_length"] or digest(raw) != row["sha256"]:
        raise CoordinatorError(f"{label} bytes differ")
    return path


def _bound_json(path: Path, label: str) -> Mapping[str, Any]:
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise CoordinatorError(f"{label} JSON is not canonical UTF-8/LF")
    try:
        value = json.loads(raw, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CoordinatorError(f"{label} JSON is invalid: {error}") from error
    if not isinstance(value, Mapping):
        raise CoordinatorError(f"{label} JSON must be an object")
    return value


def validate_bound_artifacts(value: Mapping[str, Any], artifact_root: Path) -> None:
    validate_contract(value)
    root = artifact_root.resolve(strict=True)
    bindings = [
        ("approved packet", value["approved_packet"]),
        ("protected primary boundary", value["protected_primary"]["boundary"]),
        ("source Git binding", value["source_git_binding"]["artifact"]),
        *[
            (str(gate["kind"]), gate["evidence"])
            for gate in value["retained_inputs"]
        ],
    ]
    resolved: set[Path] = set()
    verified: dict[str, Path] = {}
    for label, binding in bindings:
        path = _bound_path(root, binding, label).resolve(strict=True)
        if not path.is_relative_to(root):
            raise CoordinatorError(f"{label} escapes the artifact root")
        if path in resolved:
            raise CoordinatorError("bound artifacts alias the same file")
        resolved.add(path)
        verified[label] = path

    packet = _bound_json(verified["approved packet"], "approved packet")
    primary_boundary = _bound_json(
        verified["protected primary boundary"], "protected primary boundary"
    )
    source_binding = _bound_json(
        verified["source Git binding"], "source Git binding"
    )
    primary = value["protected_primary"]
    source = value["source_git_binding"]
    packet_artifacts = packet.get("artifact_sha256")
    source_git_control = source_binding.get("git_control")
    source_primary = source_binding.get("primary")
    successor_preimage = source_binding.get("successor_preimage")
    design = source_binding.get("design")
    if (
        packet.get("schema") != "maestro.external-build-approval-packet.v7"
        or f"sha256:{packet.get('packet_sha256')}" != PACKET_IDENTITY
        or packet.get("source_repository_realpath") != source["repository_realpath"]
        or packet.get("primary_boundary_identity") != PRIMARY_BOUNDARY_IDENTITY[7:]
        or not isinstance(packet_artifacts, Mapping)
        or packet_artifacts.get("primary-dirty-boundary.v7.json")
        != str(primary["boundary"]["sha256"])[7:]
        or packet_artifacts.get("source-git-control-binding.v7.json")
        != str(source["artifact"]["sha256"])[7:]
        or primary_boundary.get("schema")
        != "maestro.external.primary-dirty-boundary.v7"
        or primary_boundary.get("identity_sha256") != PRIMARY_BOUNDARY_IDENTITY[7:]
        or primary_boundary.get("repository_realpath")
        != primary["checkout_realpath"]
        or primary_boundary.get("head") != primary["commit"]
        or primary_boundary.get("tree") != primary["tree"]
        or source_binding.get("schema")
        != "maestro.external.source-git-control-binding.v7"
        or source_binding.get("identity_sha256") != SOURCE_BINDING_IDENTITY[7:]
        or source_binding.get("repository_realpath") != source["repository_realpath"]
        or not isinstance(source_git_control, Mapping)
        or source_git_control.get("path") != source["git_common_dir_realpath"]
        or source["repository_realpath"] != primary["checkout_realpath"]
        or not isinstance(source_primary, Mapping)
        or {
            "commit": source_primary.get("commit"),
            "tree": source_primary.get("tree"),
        }
        != PRIMARY_IDENTITY
        or not isinstance(successor_preimage, Mapping)
        or {
            "commit": successor_preimage.get("commit"),
            "tree": successor_preimage.get("tree"),
        }
        != CLEAN_SUCCESSOR
        or not isinstance(design, Mapping)
        or {
            "commit": design.get("commit"),
            "tree": design.get("tree"),
        }
        != V7_DESIGN
    ):
        raise CoordinatorError("V7 packet, source Git, or protected-primary binding differs")
    binding_path = root.joinpath(*protected_primary.BINDING_RELATIVE_PATH.parts)
    try:
        currentness_binding = protected_primary.load_binding(binding_path)
    except protected_primary.ProtectedPrimaryError as error:
        raise CoordinatorError(
            f"V7.1 protected-primary binding refused: {error}"
        ) from error
    if (
        currentness_binding.get("identity_sha256")
        != PROTECTED_PRIMARY_BINDING_IDENTITY
        or currentness_binding.get("repository_realpath")
        != primary["checkout_realpath"]
        or {
            "commit": currentness_binding.get("commit"),
            "tree": currentness_binding.get("tree"),
        }
        != PRIMARY_IDENTITY
        or f"sha256:{currentness_binding.get('boundary_identity')}"
        != PRIMARY_BOUNDARY_IDENTITY
        or currentness_binding.get("boundary_file_sha256")
        != str(primary["boundary"]["sha256"]).removeprefix("sha256:")
    ):
        raise CoordinatorError(
            "V7.1 protected-primary binding differs from coordinator"
        )


def verify_protected_primary_currentness(
    value: Mapping[str, Any], artifact_root: Path
) -> dict[str, Any]:
    path = artifact_root.resolve(strict=True).joinpath(
        *protected_primary.BINDING_RELATIVE_PATH.parts
    )
    try:
        binding = protected_primary.load_binding(path)
        return protected_primary.verify_currentness(
            binding,
            Path(str(value["protected_primary"]["checkout_realpath"])),
        )
    except (OSError, protected_primary.ProtectedPrimaryError) as error:
        raise CoordinatorError(
            f"protected-primary currentness refused: {error}"
        ) from error


def _git(repo: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "--no-optional-locks", "-C", str(repo), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={**os.environ, "LC_ALL": "C", "LANG": "C"},
        text=True,
    )
    if result.returncode != 0:
        raise CoordinatorError(
            "Git read/control operation failed: "
            f"{result.stderr.strip() or result.stdout.strip()}"
        )
    return result.stdout.strip()


def _tree(repo: Path, commit: str) -> str:
    return _sha1(_git(repo, "rev-parse", f"{commit}^{{tree}}"), "observed Git tree")


def _validated_candidate_repository(value: Mapping[str, Any], repo: Path) -> Path:
    candidate = value["candidate_ref"]
    primary = value["protected_primary"]
    resolved = repo.resolve(strict=True)
    if str(resolved) != candidate["repository_realpath"]:
        raise CoordinatorError("candidate repository realpath differs")
    primary_path = Path(str(primary["checkout_realpath"]))
    if resolved == primary_path or resolved.is_relative_to(primary_path):
        raise CoordinatorError(
            "candidate repository enters the protected primary checkout"
        )
    common_raw = _git(resolved, "rev-parse", "--git-common-dir")
    common = (
        (resolved / common_raw).resolve()
        if not Path(common_raw).is_absolute()
        else Path(common_raw).resolve()
    )
    if str(common) != candidate["git_common_dir_realpath"]:
        raise CoordinatorError("candidate Git common-directory binding differs")
    if str(common) != value["source_git_binding"]["git_common_dir_realpath"]:
        raise CoordinatorError("candidate ref is outside the source Git binding")
    if _git(resolved, "rev-parse", "--show-object-format") != "sha1":
        raise CoordinatorError("candidate Git object format differs")
    if _git(resolved, "check-ref-format", candidate["ref"]) != "":
        raise CoordinatorError("candidate ref failed Git ref-format validation")
    if _git(resolved, "for-each-ref", "--format=%(symref)", candidate["ref"]):
        raise CoordinatorError("candidate ref may not be symbolic")
    return resolved


def _validate_candidate_ancestry(repository: Path, expected_commit: str) -> None:
    if _tree(repository, CLEAN_SUCCESSOR["commit"]) != CLEAN_SUCCESSOR["tree"]:
        raise CoordinatorError("V7 clean-successor tree differs")
    for row in CANONICAL_ANCESTRY:
        if _tree(repository, row["commit"]) != row["tree"]:
            raise CoordinatorError(f"{row['lane']} canonical tree differs")
    design_row = _git(
        repository, "rev-list", "--parents", "--max-count=1", V7_DESIGN["commit"]
    ).split()
    if design_row != [V7_DESIGN["commit"], CLEAN_SUCCESSOR["commit"]]:
        raise CoordinatorError("V7 design is not the direct clean-successor child")
    first_parent = _git(
        repository, "rev-list", "--first-parent", expected_commit
    ).splitlines()
    try:
        positions = [
            first_parent.index(row["commit"]) for row in CANONICAL_ANCESTRY
        ]
    except ValueError as error:
        raise CoordinatorError(
            "candidate expected preimage lacks exact canonical lane ancestry"
        ) from error
    if positions != sorted(positions, reverse=True):
        raise CoordinatorError("canonical lane ancestry order differs")


def observe_candidate_ref(
    value: Mapping[str, Any], artifact_root: Path, candidate_repository: Path
) -> dict[str, Any]:
    validate_bound_artifacts(value, artifact_root)
    repository = _validated_candidate_repository(value, candidate_repository)
    candidate = value["candidate_ref"]
    expected = candidate["expected_preimage"]
    declared = candidate["declared_postimage"]
    _validate_candidate_ancestry(repository, expected["commit"])
    if _tree(repository, expected["commit"]) != expected["tree"]:
        raise CoordinatorError("candidate expected-preimage tree differs")
    if _tree(repository, declared["commit"]) != declared["tree"]:
        raise CoordinatorError("candidate declared-postimage tree differs")
    if _git(repository, "cat-file", "-t", expected["commit"]) != "commit":
        raise CoordinatorError("candidate expected preimage is not a commit")
    if _git(repository, "cat-file", "-t", declared["commit"]) != "commit":
        raise CoordinatorError("candidate declared postimage is not a commit")
    postimage_row = _git(
        repository, "rev-list", "--parents", "--max-count=1", declared["commit"]
    ).split()
    if postimage_row != [declared["commit"], expected["commit"]]:
        raise CoordinatorError(
            "candidate declared postimage is not the direct preimage child"
        )
    observed_commit = _git(repository, "rev-parse", "--verify", candidate["ref"])
    observed_tree = _tree(repository, observed_commit)
    if (observed_commit, observed_tree) == (
        expected["commit"],
        expected["tree"],
    ):
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


def execute_isolated_candidate_ref_cas(
    value: Mapping[str, Any], artifact_root: Path, candidate_repository: Path
) -> dict[str, Any]:
    observed = observe_candidate_ref(value, artifact_root, candidate_repository)
    if observed["cas_observation"]["state"] == "exact_declared_postimage":
        return observed
    repository = _validated_candidate_repository(observed, candidate_repository)
    candidate = observed["candidate_ref"]
    verify_protected_primary_currentness(observed, artifact_root)
    _git(
        repository,
        "update-ref",
        "--no-deref",
        candidate["ref"],
        candidate["declared_postimage"]["commit"],
        candidate["expected_preimage"]["commit"],
    )
    completed = observe_candidate_ref(observed, artifact_root, repository)
    if completed["cas_observation"]["state"] != "exact_declared_postimage":
        raise CoordinatorError("candidate ref CAS did not reach the declared postimage")
    return completed
