#!/usr/bin/env python3
"""Run and publish one V4 seal from a previously frozen closure."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import secrets
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Mapping


ENGINE_IDS = ("python", "rust", "ruby")
SCHEMA = "maestro.external.vnext-final-cumulative-seal-receipt.v1"
POINTER_SCHEMA = "maestro.external.vnext-final-cumulative-seal-pointer.v1"
EFFECT_DENYLIST = [
    "install",
    "publish",
    "activate",
    "release",
    "push",
    "tag",
    "network",
    "remote_connector",
    "live_external_system",
    "candidate_ref_write",
    "protected_primary_checkout_write",
    "outside_packet_bound_roots_write",
]
STAGE12_COORDINATOR_SCHEMA = "maestro.external.stage12-legacy-cut-coordinator.v2"
STAGE12_PACKET_IDENTITY = (
    "sha256:171de6121c62f1c8af55e9e248da506ca96322cb5a588c75ee3762f7d8082472"
)
STAGE12_CANONICAL_ANCESTRY = [
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
]
STAGE12_GATE_ORDER = [
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
]
STAGE12_EFFECT_BOUNDARY = {
    "primary_never_target": True,
    "authority_guard_mint_or_reconstruction": False,
    "live_product_path_pruning": False,
    "adapter_activation": False,
    "installation": False,
    "publication": False,
    "release": False,
    "final_runner_candidate_ref_write": False,
}


class FinalChainError(RuntimeError):
    pass


def canonical_bytes(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise FinalChainError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path) -> dict[str, Any]:
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise FinalChainError(f"noncanonical JSON: {path}")
    try:
        value = json.loads(raw, object_pairs_hook=reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FinalChainError(f"invalid JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise FinalChainError(f"JSON object required: {path}")
    return value


def safe_relative(value: object) -> PurePosixPath:
    if not isinstance(value, str) or not value or "\\" in value:
        raise FinalChainError("portable relative path required")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise FinalChainError(f"unsafe relative path: {value!r}")
    return path


def bound_file(root: Path, binding: object, label: str) -> Path:
    if not isinstance(binding, Mapping):
        raise FinalChainError(f"{label} binding is absent")
    path = root.joinpath(*safe_relative(binding.get("path")).parts)
    if path.is_symlink() or not path.is_file():
        raise FinalChainError(f"{label} is absent or unsafe: {path}")
    raw = path.read_bytes()
    if binding.get("byte_length") != len(raw) or binding.get("sha256") != digest(raw):
        raise FinalChainError(f"{label} bytes differ: {path}")
    return path


def ensure_readonly_tree(root: Path) -> None:
    if root.is_symlink() or not root.is_dir():
        raise FinalChainError(f"immutable root is absent or unsafe: {root}")
    for path in root.rglob("*"):
        if path.is_symlink():
            raise FinalChainError(f"immutable root contains a symlink: {path}")
        if path.stat().st_mode & 0o222:
            raise FinalChainError(f"immutable root remains writable: {path}")


def validate_packet(closure: Path, binding: object) -> None:
    manifest_path = bound_file(closure, binding, "packet manifest")
    manifest = read_json(manifest_path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-packet-manifest.v1":
        raise FinalChainError("packet manifest schema differs")
    if manifest.get("approved_packet_identity") != (
        "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
    ):
        raise FinalChainError("packet identity differs")
    rows = manifest.get("files")
    if not isinstance(rows, list) or not rows:
        raise FinalChainError("packet manifest is empty")
    seen = set()
    total = 0
    for row in rows:
        path = bound_file(closure, row, "packet artifact")
        if path.name in seen:
            raise FinalChainError("packet manifest duplicates a file")
        seen.add(path.name)
        total += int(row["byte_length"])
    required = {
        "replacement-build-approval-packet.v4.json",
        "proof-inputs.v4.json",
        "fanout-manifest.v4.json",
        "integration-ancestry.v4.txt",
        "external-build-plan-handoff.v4.json",
        "independent-verification.v4.json",
    }
    if not required.issubset(seen):
        raise FinalChainError("packet manifest omits a required V4 artifact")
    if manifest.get("file_count") != len(rows) or manifest.get("byte_length") != total:
        raise FinalChainError("packet manifest totals differ")
    actual = {
        path.name
        for path in (closure / "packet").iterdir()
        if path.is_file() and path.name != "packet-manifest.v1.json"
    }
    if actual != seen:
        raise FinalChainError("packet directory differs from its byte-total manifest")


def validate_manifest(path: Path, source: Path, commit: str, tree: str) -> dict[str, Any]:
    manifest = read_json(path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-input-manifest.v1":
        raise FinalChainError("input manifest schema differs")
    if manifest.get("commit") != commit or manifest.get("tree") != tree:
        raise FinalChainError("input manifest binds another final integration")
    rows = manifest.get("entries")
    if not isinstance(rows, list) or not rows:
        raise FinalChainError("input manifest is empty")
    seen = set()
    total = 0
    for row in rows:
        if not isinstance(row, Mapping):
            raise FinalChainError("input manifest row is invalid")
        path = str(row.get("path"))
        if path in seen:
            raise FinalChainError("input manifest duplicates a path")
        seen.add(path)
        bound_file(source, row, "input manifest row")
        total += int(row["byte_length"])
    actual = {
        path.relative_to(source).as_posix()
        for path in source.rglob("*")
        if path.is_file()
    }
    if actual != seen:
        raise FinalChainError("input manifest has an omission or extra path")
    if manifest.get("entry_count") != len(rows) or manifest.get("byte_length") != total:
        raise FinalChainError("input manifest totals differ")
    return manifest


def stream_identity(raw: bytes) -> dict[str, object]:
    return {"byte_length": len(raw), "sha256": digest(raw)}


def validate_toolchain(path: Path, source: Path | None = None) -> dict[str, Any]:
    value = read_json(path)
    if value.get("schema_version") != "maestro.external.vnext-final-toolchain.v1":
        raise FinalChainError("toolchain schema differs")
    tools = value.get("tools")
    if not isinstance(tools, Mapping) or set(tools) != {
        "python",
        "rust",
        "ruby",
        "cargo",
        "git",
    }:
        raise FinalChainError("toolchain closure differs")
    for name, row in tools.items():
        if not isinstance(row, Mapping):
            raise FinalChainError(f"tool row is invalid: {name}")
        executable = Path(str(row.get("resolved_path")))
        if executable.is_symlink() or not executable.is_file():
            raise FinalChainError(f"tool is absent or unsafe: {name}")
        raw = executable.read_bytes()
        if row.get("byte_length") != len(raw) or row.get("sha256") != digest(raw):
            raise FinalChainError(f"tool bytes differ: {name}")
        result = subprocess.run(
            row["probe_argv"], stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        if (
            result.returncode != row.get("probe_exit_code")
            or stream_identity(result.stdout) != row.get("probe_stdout")
            or stream_identity(result.stderr) != row.get("probe_stderr")
        ):
            raise FinalChainError(f"tool probe differs: {name}")
    if value.get("environment") != {"LC_ALL": "C", "LANG": "C", "TZ": "UTC"}:
        raise FinalChainError("toolchain environment differs")
    if not isinstance(value.get("target"), str) or not isinstance(value.get("profile"), str):
        raise FinalChainError("toolchain target or profile is absent")
    lockfiles = value.get("lockfiles")
    if not isinstance(lockfiles, list) or not lockfiles:
        raise FinalChainError("lockfile closure is absent")
    if source is not None:
        for lockfile in lockfiles:
            bound_file(source, lockfile, "toolchain lockfile")
    dependencies = value.get("dependency_outputs")
    expected_dependency_names = {
        f"{engine}-complete-cargo-native-closure" for engine in ENGINE_IDS
    }
    if (
        not isinstance(dependencies, list)
        or len(dependencies) != 3
        or {
            row.get("name")
            for row in dependencies
            if isinstance(row, Mapping)
        }
        != expected_dependency_names
    ):
        raise FinalChainError("dependency-output closure is absent")
    for dependency in dependencies:
        if not isinstance(dependency, Mapping):
            raise FinalChainError("dependency-output row is invalid")
        root = Path(str(dependency.get("resolved_path")))
        if root.is_symlink() or not root.is_dir():
            raise FinalChainError("dependency-output root is absent or unsafe")
        if source is not None and not root.is_relative_to(
            source.parent / "dependencies"
        ):
            raise FinalChainError(
                "dependency-output root is outside the frozen dependency closure"
            )
        expected = dependency.get("files")
        if not isinstance(expected, list) or not expected:
            raise FinalChainError("dependency-output file closure is empty")
        rows = []
        for row in expected:
            relative = safe_relative(row.get("path"))
            candidate = root.joinpath(*relative.parts)
            if candidate.is_symlink() or not candidate.is_file():
                raise FinalChainError("dependency-output file is absent or unsafe")
            raw = candidate.read_bytes()
            actual = {
                "path": str(row["path"]),
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
            if actual != row:
                raise FinalChainError("dependency-output bytes differ")
            rows.append(actual)
        actual_paths = {
            item.relative_to(root).as_posix()
            for item in root.rglob("*")
            if item.is_file()
        }
        if actual_paths != {str(row["path"]) for row in rows}:
            raise FinalChainError("dependency-output manifest has an omission")
        if (
            dependency.get("file_count") != len(rows)
            or dependency.get("byte_length")
            != sum(int(row["byte_length"]) for row in rows)
            or dependency.get("identity") != digest(canonical_bytes(rows))
        ):
            raise FinalChainError("dependency-output identity differs")
        probe = dependency.get("completeness_probe")
        if (
            not isinstance(probe, Mapping)
            or probe.get("exit_code") != 0
            or not {"fetch", "--offline", "--frozen", "--locked", "--target"}.issubset(
                set(probe.get("argv", []))
            )
            or not isinstance(probe.get("stdout"), Mapping)
            or not isinstance(probe.get("stderr"), Mapping)
        ):
            raise FinalChainError("dependency closure completeness probe is absent")
    return value


def validate_ledger(path: Path, source: Path, commit: str) -> list[dict[str, Any]]:
    ledger = read_json(path)
    if ledger.get("schema_version") != "maestro.external.vnext-final-proof-ledger.v1":
        raise FinalChainError("proof ledger schema differs")
    if ledger.get("snapshot_commit") != commit:
        raise FinalChainError("proof ledger binds another final commit")
    rows = ledger.get("proofs")
    if not isinstance(rows, list) or ledger.get("proof_count") != len(rows):
        raise FinalChainError("proof ledger count differs")
    ids = set()
    stages = set()
    kinds = set()
    for row in rows:
        if not isinstance(row, dict) or row.get("proof_id") in ids:
            raise FinalChainError("proof row or identifier differs")
        ids.add(row["proof_id"])
        stages.add(row.get("stage"))
        kinds.add(row.get("kind"))
        if row.get("engines") != list(ENGINE_IDS):
            raise FinalChainError("proof engine coverage differs")
        for binding in row.get("input_bindings", []):
            bound_file(source, binding, "proof input")
        command = row.get("command")
        if not isinstance(command, Mapping):
            raise FinalChainError("proof command is absent")
        identity = digest(
            canonical_bytes(
                {
                    "argv": command.get("argv"),
                    "expected_exit_code": command.get("expected_exit_code"),
                }
            )
        )
        if command.get("identity") != identity:
            raise FinalChainError("proof command identity differs")
        harness = row.get("harness")
        if not isinstance(harness, Mapping):
            raise FinalChainError("proof harness is absent")
        if row.get("kind") in {"race", "crash_replay"}:
            bound_file(source, harness.get("fault_schedule"), "fault schedule")
        if row.get("kind") in {"migration", "rollback"}:
            bound_file(source, harness.get("cohort"), "migration cohort")
    if stages != set(range(13)) or len(kinds) != 14:
        raise FinalChainError("proof Stage or kind closure differs")
    return rows


def validate_readback(path: Path, commit: str) -> dict[str, Any]:
    value = read_json(path)
    if value.get("schema_version") != "maestro.external.vnext-stage12-semantic-readback-plan.v1":
        raise FinalChainError("readback schema differs")
    if value.get("snapshot_commit") != commit:
        raise FinalChainError("readback binds another final commit")
    checks = value.get("checks")
    required = {
        "compiled_namespace_absence",
        "generated_resource_absence",
        "persisted_identity_parity",
        "canonical_facade_behavior",
        "migration_route_absence",
        "retained_reader_absence",
        "consumer_reader_hold_zero",
        "negative_fixture",
    }
    if not isinstance(checks, list) or {row.get("kind") for row in checks} != required:
        raise FinalChainError("semantic readback closure differs")
    for check in checks:
        kinds = check.get("required_artifact_kinds")
        if not isinstance(kinds, list) or not kinds:
            raise FinalChainError("semantic produced-artifact closure is absent")
        if (
            not isinstance(check.get("minimum_canonical_reads"), int)
            or check["minimum_canonical_reads"] < 1
            or not isinstance(check.get("minimum_negative_routes"), int)
            or check["minimum_negative_routes"] < 1
        ):
            raise FinalChainError(
                "semantic canonical-read or negative-route proof is absent"
            )
        identity = digest(
            canonical_bytes(
                {
                    "argv": check.get("argv"),
                    "expected_exit_code": check.get("expected_exit_code"),
                }
            )
        )
        if check.get("command_identity") != identity:
            raise FinalChainError("readback command identity differs")
    return value


def validate_stage12_coordinator(
    closure: Path, path: Path, final_commit: str, final_tree: str
) -> dict[str, Any]:
    value = read_json(path)
    if set(value) != {
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
    }:
        raise FinalChainError("Stage 12 coordinator fields differ")
    if (
        value.get("schema_version") != STAGE12_COORDINATOR_SCHEMA
        or value.get("authority_scope")
        != "one_expected_preimage_isolated_candidate_ref_cas_only"
        or value.get("approved_packet_identity") != STAGE12_PACKET_IDENTITY
    ):
        raise FinalChainError("Stage 12 coordinator identity differs")
    if value.get("approved_packet") != {
        "path": "control/stage12/packet/replacement-build-approval-packet.v7.json",
        "byte_length": 10927,
        "sha256": "sha256:0c525951a49c7406d1008c64a3ad328505777c09cb7388b11a5db8634c3f4f65",
    }:
        raise FinalChainError("Stage 12 approved packet artifact differs")
    primary = value.get("protected_primary")
    source = value.get("source_git_binding")
    ancestry = value.get("canonical_ancestry")
    clean = value.get("clean_successor_preimage")
    candidate = value.get("candidate_ref")
    if (
        not isinstance(primary, Mapping)
        or set(primary)
        != {
            "checkout_realpath",
            "ref",
            "commit",
            "tree",
            "boundary_identity",
            "boundary",
            "candidate_target",
        }
        or primary.get("candidate_target") is not False
        or not Path(str(primary.get("checkout_realpath", ""))).is_absolute()
        or not str(primary.get("ref", "")).startswith("refs/heads/")
        or primary.get("commit") != "13b9a5e9b5ec67e7086b0b21992a207d2e4cde94"
        or primary.get("tree") != "97e08a00f8a721318cda13241129a3b06651accc"
        or primary.get("boundary_identity")
        != "sha256:e5b4c0592b8cf373ea68fc5e0e3f84020c14f3f422c5779e8d4a423930aa6054"
        or primary.get("boundary")
        != {
            "path": "control/stage12/packet/primary-dirty-boundary.v7.json",
            "byte_length": 1126,
            "sha256": "sha256:4f4ec8207a5f5824c9113cca1a3b04cf390f2bf731f1188e399ba56d8ad6c26a",
        }
        or not isinstance(source, Mapping)
        or set(source)
        != {
            "identity",
            "repository_realpath",
            "git_common_dir_realpath",
            "object_format",
            "artifact",
        }
        or not Path(str(source.get("repository_realpath", ""))).is_absolute()
        or not Path(str(source.get("git_common_dir_realpath", ""))).is_absolute()
        or source.get("identity")
        != "sha256:1099c62b3c9a333da68733a098ceece9e6754f28f1ea53f30b4b8dfcc6ae92d7"
        or source.get("object_format") != "sha1"
        or source.get("artifact")
        != {
            "path": "control/stage12/packet/source-git-control-binding.v7.json",
            "byte_length": 1706,
            "sha256": "sha256:7d73e0746497566712a1c6782c8d3435627aa2c7b997e59ceaea1b32756e792d",
        }
        or ancestry != STAGE12_CANONICAL_ANCESTRY
        or clean
        != {
            "commit": "e69295329c29c1c75901315a56e947b85b7a69cf",
            "tree": "cd36cbb2963a264cb67a834bb38c709c0ea144ae",
        }
        or not isinstance(candidate, Mapping)
        or set(candidate)
        != {
            "repository_realpath",
            "git_common_dir_realpath",
            "ref",
            "expected_preimage",
            "declared_postimage",
            "declared_postimage_parent",
            "ref_update_algorithm",
            "crash_states",
        }
    ):
        raise FinalChainError("Stage 12 coordinator protected identities differ")
    expected = candidate.get("expected_preimage")
    declared = candidate.get("declared_postimage")
    if (
        not isinstance(expected, Mapping)
        or set(expected) != {"commit", "tree"}
        or not isinstance(declared, Mapping)
        or set(declared) != {"commit", "tree"}
        or candidate.get("ref") == primary.get("ref")
        or not str(candidate.get("ref", "")).startswith("refs/heads/")
        or not Path(str(candidate.get("repository_realpath", ""))).is_absolute()
        or not Path(str(candidate.get("git_common_dir_realpath", ""))).is_absolute()
        or candidate.get("repository_realpath") == primary.get("checkout_realpath")
        or candidate.get("git_common_dir_realpath")
        != source.get("git_common_dir_realpath")
        or candidate.get("declared_postimage_parent") != expected.get("commit")
        or candidate.get("ref_update_algorithm")
        != "git-update-ref-no-deref-new-old"
        or candidate.get("crash_states")
        != ["exact_expected_preimage", "exact_declared_postimage"]
        or declared != {"commit": final_commit, "tree": final_tree}
    ):
        raise FinalChainError("Stage 12 coordinator candidate-ref closure differs")
    observation = value.get("cas_observation")
    if (
        not isinstance(observation, Mapping)
        or set(observation) != {"state", "observed_commit", "observed_tree"}
        or observation.get("state") != "exact_declared_postimage"
        or observation.get("observed_commit") != final_commit
        or observation.get("observed_tree") != final_tree
    ):
        raise FinalChainError("Stage 12 coordinator postimage was not observed")
    gates = value.get("retained_inputs")
    if (
        not isinstance(gates, list)
        or len(gates) != len(STAGE12_GATE_ORDER)
        or any(not isinstance(row, Mapping) for row in gates)
        or [(row.get("kind"), row.get("state")) for row in gates]
        != STAGE12_GATE_ORDER
        or any(
            row.get("count") != 0
            for row in gates
            if row.get("kind")
            in {"consumer_manifest", "reader_manifest", "hold_manifest"}
        )
    ):
        raise FinalChainError("Stage 12 coordinator retained-input order differs")
    for row in gates:
        expected_fields = {"kind", "state", "identity", "evidence"}
        if row.get("kind") in {
            "consumer_manifest",
            "reader_manifest",
            "hold_manifest",
        }:
            expected_fields.add("count")
        if row.get("kind") == "namespace_promotion_manifest":
            expected_fields.update({"entry_count", "mismatch_count"})
        identity = row.get("identity")
        if (
            set(row) != expected_fields
            or not isinstance(identity, str)
            or len(identity) != 71
            or not identity.startswith("sha256:")
            or any(character not in "0123456789abcdef" for character in identity[7:])
        ):
            raise FinalChainError("Stage 12 coordinator retained-input row differs")
    namespace = gates[13]
    if namespace.get("entry_count") != 210 or namespace.get("mismatch_count") != 0:
        raise FinalChainError("Stage 12 coordinator namespace parity differs")
    if value.get("effect_boundary") != STAGE12_EFFECT_BOUNDARY:
        raise FinalChainError("Stage 12 coordinator effect boundary differs")
    bindings = [
        ("approved packet", value.get("approved_packet")),
        ("protected primary boundary", primary.get("boundary")),
        ("source Git binding", source.get("artifact")),
        *[
            (f"Stage 12 {row.get('kind')}", row.get("evidence"))
            for row in gates
        ],
    ]
    bound_paths = {
        label: bound_file(closure, binding, label) for label, binding in bindings
    }
    packet = read_json(bound_paths["approved packet"])
    primary_boundary = read_json(bound_paths["protected primary boundary"])
    source_binding = read_json(bound_paths["source Git binding"])
    packet_artifacts = packet.get("artifact_sha256")
    source_git_control = source_binding.get("git_control")
    source_primary = source_binding.get("primary")
    successor_preimage = source_binding.get("successor_preimage")
    design = source_binding.get("design")
    if (
        packet.get("schema") != "maestro.external-build-approval-packet.v7"
        or f"sha256:{packet.get('packet_sha256')}" != STAGE12_PACKET_IDENTITY
        or packet.get("source_repository_realpath") != source.get("repository_realpath")
        or packet.get("primary_boundary_identity")
        != str(primary.get("boundary_identity"))[7:]
        or not isinstance(packet_artifacts, Mapping)
        or packet_artifacts.get("primary-dirty-boundary.v7.json")
        != str(primary["boundary"]["sha256"])[7:]
        or packet_artifacts.get("source-git-control-binding.v7.json")
        != str(source["artifact"]["sha256"])[7:]
        or primary_boundary.get("schema")
        != "maestro.external.primary-dirty-boundary.v7"
        or primary_boundary.get("identity_sha256")
        != str(primary.get("boundary_identity"))[7:]
        or primary_boundary.get("repository_realpath")
        != primary.get("checkout_realpath")
        or primary_boundary.get("head") != primary.get("commit")
        or primary_boundary.get("tree") != primary.get("tree")
        or source_binding.get("schema")
        != "maestro.external.source-git-control-binding.v7"
        or source_binding.get("identity_sha256") != str(source.get("identity"))[7:]
        or source_binding.get("repository_realpath")
        != source.get("repository_realpath")
        or not isinstance(source_git_control, Mapping)
        or source_git_control.get("path") != source.get("git_common_dir_realpath")
        or source.get("repository_realpath") != primary.get("checkout_realpath")
        or not isinstance(source_primary, Mapping)
        or {
            "commit": source_primary.get("commit"),
            "tree": source_primary.get("tree"),
        }
        != {
            "commit": primary.get("commit"),
            "tree": primary.get("tree"),
        }
        or not isinstance(successor_preimage, Mapping)
        or {
            "commit": successor_preimage.get("commit"),
            "tree": successor_preimage.get("tree"),
        }
        != clean
        or not isinstance(design, Mapping)
        or design.get("commit")
        != "ff454521b7037d5df7b8e836b8ce30f77e1ff8dc"
        or design.get("tree") != "bd2c08f87809d5093252943f2fd04a5be551aa13"
    ):
        raise FinalChainError(
            "Stage 12 V7 packet, source Git, or protected-primary binding differs"
        )
    return value


def validate_snapshot(closure: Path) -> tuple[dict[str, Any], dict[str, Path]]:
    snapshot_path = closure / "control/final-cumulative-closure-snapshot.v1.json"
    snapshot = read_json(snapshot_path)
    if snapshot.get("schema_version") != "maestro.external.vnext-final-cumulative-closure-snapshot.v1":
        raise FinalChainError("snapshot schema differs")
    if snapshot.get("state") != "frozen":
        raise FinalChainError("snapshot is not frozen")
    if snapshot.get("approved_packet_identity") != (
        "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
    ):
        raise FinalChainError("snapshot packet identity differs")
    final = snapshot.get("final_integration")
    if not isinstance(final, Mapping):
        raise FinalChainError("final integration identity is absent")
    commit = str(final.get("commit"))
    tree = str(final.get("tree"))
    stages = snapshot.get("first_parent_stages")
    if not isinstance(stages, list) or [row.get("stage") for row in stages] != list(range(13)):
        raise FinalChainError("Stage checkpoint closure differs")
    if stages[-1].get("commit") != commit or stages[-1].get("tree") != tree:
        raise FinalChainError("current V4 Stage 12 checkpoint differs")
    for row in stages:
        checkpoint = bound_file(closure, row.get("checkpoint"), "Stage checkpoint")
        value = read_json(checkpoint)
        if any(
            value.get(field) != row.get(field)
            for field in ("stage", "commit", "tree", "parents")
        ):
            raise FinalChainError("Stage checkpoint bytes differ from snapshot")
    if (
        len(stages[5].get("parents", [])) != 2
        or stages[5]["parents"][0] != stages[4]["commit"]
    ):
        raise FinalChainError("Stage 5 merge parent topology differs")
    for stage in range(6, 12):
        if stages[stage].get("parents") != [stages[stage - 1]["commit"]]:
            raise FinalChainError("Stage 6-11 direct-parent topology differs")
    reviewed = snapshot.get("stage12_reviewed_candidate")
    if (
        not isinstance(reviewed, Mapping)
        or stages[12].get("parents")
        != [stages[11]["commit"], reviewed.get("commit")]
        or stages[12].get("tree") != reviewed.get("tree")
    ):
        raise FinalChainError("Stage 12 reviewed-candidate merge topology differs")
    ancestry = snapshot.get("stage5_second_parent_ancestry")
    if (
        not isinstance(ancestry, list)
        or not ancestry
        or ancestry[0].get("commit") != stages[5]["parents"][1]
        or ancestry[-1].get("commit")
        != snapshot.get("provisional_stage5_source_commit")
    ):
        raise FinalChainError("Stage 5 second-parent ancestry differs")
    overlay_path = bound_file(
        closure, snapshot.get("stage12_overlay"), "Stage 12 overlay manifest"
    )
    overlay = read_json(overlay_path)
    if (
        overlay.get("stage11_commit") != stages[11]["commit"]
        or overlay.get("reviewed_candidate_commit") != reviewed.get("commit")
        or overlay.get("reviewed_candidate_tree") != reviewed.get("tree")
        or overlay.get("stage12_commit") != stages[12]["commit"]
        or overlay.get("stage12_tree") != stages[12]["tree"]
    ):
        raise FinalChainError("Stage 12 overlay identity differs")
    coordinator_path = bound_file(
        closure,
        snapshot.get("stage12_legacy_cut_coordinator"),
        "Stage 12 legacy-cut coordinator",
    )
    validate_stage12_coordinator(closure, coordinator_path, commit, tree)
    promotion_path = bound_file(
        closure,
        snapshot.get("promotion_prerequisites"),
        "promotion prerequisites",
    )
    promotion = read_json(promotion_path)
    if (
        promotion.get("schema_version")
        != "maestro.external.vnext-final-promotion-prerequisites.v1"
        or promotion.get("stage11_commit") != stages[11]["commit"]
        or promotion.get("stage12_reviewed_candidate") != reviewed.get("commit")
        or promotion.get("legacy_prune_gate", {}).get("observed_legacy_row_count")
        != 0
        or promotion.get("consumer_reader_hold", {}).get("consumer_count") != 0
        or promotion.get("consumer_reader_hold", {}).get("reader_count") != 0
        or promotion.get("consumer_reader_hold", {}).get("hold_count") != 0
        or promotion.get("promotion_parity", {}).get("source_file_count") != 210
        or promotion.get("promotion_parity", {}).get("promoted_file_count") != 210
        or promotion.get("promotion_parity", {}).get("mismatch_count") != 0
    ):
        raise FinalChainError("promotion prerequisites are absent or nonzero")
    promotion_receipts = {}
    for section in ("legacy_prune_gate", "consumer_reader_hold", "promotion_parity"):
        promotion_receipts[f"promotion_{section}_receipt"] = bound_file(
            promotion_path.parent,
            promotion[section].get("receipt"),
            f"{section} receipt",
        )
    if snapshot.get("immutable_input_roots") != [
        "source",
        "packet",
        "control",
        "dependencies",
    ]:
        raise FinalChainError("immutable root roles differ")
    if snapshot.get("environment_allowlist") != [
        "HOME",
        "LANG",
        "LC_ALL",
        "PATH",
        "TMPDIR",
        "TZ",
    ]:
        raise FinalChainError("environment allowlist differs")
    roles = snapshot.get("writable_root_roles")
    if not isinstance(roles, list) or len(roles) != 12 or len(set(roles)) != 12:
        raise FinalChainError("writable-root roles are not disjoint")
    if snapshot.get("cache_policy") != "immutable_compilation_and_dependency_bytes_only":
        raise FinalChainError("cache policy admits verdict reuse")
    if snapshot.get("sandbox_profile") != "macos-sandbox-exec-no-network-v1":
        raise FinalChainError("sandbox profile differs")
    if snapshot.get("effect_denylist") != EFFECT_DENYLIST:
        raise FinalChainError("effect denylist differs")
    protected_primary = snapshot.get("protected_primary_checkout")
    if not isinstance(protected_primary, str) or not Path(protected_primary).is_absolute():
        raise FinalChainError("protected primary identity is absent")
    if not isinstance(snapshot.get("pointer_preimage"), Mapping):
        raise FinalChainError("pointer preimage is absent")
    publication_identity = snapshot.get("publication_root_identity")
    expected_generation = snapshot.get("expected_generation")
    if (
        not isinstance(publication_identity, Mapping)
        or set(publication_identity)
        != {"path", "device", "inode", "mount_device", "mode", "link_count", "ctime_ns"}
        or not isinstance(expected_generation, int)
        or expected_generation < 0
        or snapshot["pointer_preimage"].get("generation") != expected_generation
    ):
        raise FinalChainError("publication custody or expected generation differs")
    engines = snapshot.get("engines")
    if not isinstance(engines, list) or [row.get("id") for row in engines] != list(ENGINE_IDS):
        raise FinalChainError("engine closure differs")
    for row in engines:
        bound_file(closure / "source", row.get("source"), "engine source")
    validate_packet(closure, snapshot.get("packet_manifest"))
    paths = {
        "snapshot": snapshot_path,
        "packet_manifest": bound_file(
            closure, snapshot.get("packet_manifest"), "packet manifest"
        ),
        "manifest": bound_file(closure, snapshot.get("input_manifest"), "input manifest"),
        "registry": bound_file(
            closure / "source", snapshot.get("proof_registry"), "proof registry"
        ),
        "ledger": bound_file(closure, snapshot.get("proof_ledger"), "proof ledger"),
        "readback": bound_file(closure, snapshot.get("stage12_readback"), "readback plan"),
        "toolchain": bound_file(closure, snapshot.get("toolchain"), "toolchain"),
        "overlay": overlay_path,
        "stage12_coordinator": coordinator_path,
        "ancestry_pack": bound_file(
            closure, snapshot.get("ancestry_pack"), "ancestry object pack"
        ),
        "promotion": promotion_path,
        **promotion_receipts,
    }
    if len({path.name for path in paths.values()}) != len(paths):
        raise FinalChainError("published control filenames are not unique")
    validate_manifest(paths["manifest"], closure / "source", commit, tree)
    validate_toolchain(paths["toolchain"], closure / "source")
    validate_ledger(paths["ledger"], closure / "source", commit)
    validate_readback(paths["readback"], commit)
    for root_name in ("source", "packet", "control", "dependencies"):
        ensure_readonly_tree(closure / root_name)
    return snapshot, paths


def copy_source(source: Path, destination: Path) -> None:
    shutil.copytree(source, destination, symlinks=False, copy_function=shutil.copyfile)
    for path in destination.rglob("*"):
        if path.is_symlink():
            raise FinalChainError(f"engine source contains a symlink: {path}")
        path.chmod(
            stat.S_IRUSR
            | (stat.S_IXUSR if path.is_dir() or os.access(path, os.X_OK) else 0)
        )
    destination.chmod(stat.S_IRUSR | stat.S_IXUSR)


def sandbox_literal(path: Path) -> str:
    return str(path).replace("\\", "\\\\").replace('"', '\\"')


def sandbox_profile(
    read_roots: list[Path], writable_roots: list[Path]
) -> str:
    reads = "\n".join(
        f'(allow file-read* (subpath "{sandbox_literal(path)}"))'
        for path in sorted(set(read_roots))
    )
    writes = "\n".join(
        f'(allow file-write* (subpath "{sandbox_literal(path)}"))'
        for path in sorted(set(writable_roots))
    )
    return (
        "(version 1)\n"
        "(deny default)\n"
        "(allow process*)\n"
        "(allow sysctl-read)\n"
        f"{reads}\n"
        f"{writes}\n"
        "(deny network*)\n"
    )


def verify_sandbox(sandbox_exec: Path, run_root: Path, protected_primary: Path) -> None:
    if sandbox_exec != Path("/usr/bin/sandbox-exec") or not sandbox_exec.is_file():
        raise FinalChainError("required no-network sandbox is unavailable")
    probe_root = run_root / "sandbox-probe"
    probe_root.mkdir()
    profile = probe_root / "profile.sb"
    system_reads = [
        Path("/System"),
        Path("/usr"),
        Path("/Library"),
        Path("/dev"),
        probe_root,
    ]
    profile.write_text(
        sandbox_profile(system_reads, [probe_root]), encoding="utf-8"
    )
    allowed = probe_root / "allowed-write"
    allowed_write = subprocess.run(
        [str(sandbox_exec), "-f", str(profile), "/usr/bin/touch", str(allowed)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if allowed_write.returncode != 0 or not allowed.is_file():
        raise FinalChainError("sandbox exact writable-root probe failed")
    allowed.unlink()
    network = subprocess.run(
        [
            str(sandbox_exec),
            "-f",
            str(profile),
            "/usr/bin/python3",
            "-c",
            (
                "import errno,socket,sys\n"
                "try:\n"
                " s=socket.socket(); s.bind(('127.0.0.1',0))\n"
                "except OSError as e:\n"
                " sys.exit(0 if e.errno in (errno.EPERM,errno.EACCES) else 3)\n"
                "sys.exit(4)\n"
            ),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if network.returncode != 0:
        raise FinalChainError("sandbox network denial probe was not a policy denial")
    outside = protected_primary / ".final-chain-sandbox-write-probe"
    write = subprocess.run(
        [str(sandbox_exec), "-f", str(profile), "/usr/bin/touch", str(outside)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if write.returncode == 0 or outside.exists():
        raise FinalChainError("sandbox protected-primary write denial probe failed")
    protected_read = subprocess.run(
        [
            str(sandbox_exec),
            "-f",
            str(profile),
            "/bin/cat",
            str(protected_primary / "Cargo.toml"),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if protected_read.returncode == 0:
        raise FinalChainError(
            "sandbox protected-primary read denial probe failed"
        )


def verify_engine_sandbox(
    sandbox_exec: Path,
    profile: Path,
    immutable_roots: list[Path],
    writable_roots: list[Path],
) -> None:
    for root in immutable_roots:
        probe = root / ".final-chain-write-probe"
        result = subprocess.run(
            [str(sandbox_exec), "-f", str(profile), "/usr/bin/touch", str(probe)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if result.returncode == 0 or probe.exists():
            raise FinalChainError(f"sandbox immutable-root write probe failed: {root}")
    for root in writable_roots:
        probe = root / ".final-chain-write-probe"
        result = subprocess.run(
            [str(sandbox_exec), "-f", str(profile), "/usr/bin/touch", str(probe)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if result.returncode != 0 or not probe.is_file():
            raise FinalChainError(f"sandbox writable-role probe failed: {root}")
        probe.unlink()


def engine_environment(root: Path, toolchain: Mapping[str, Any]) -> dict[str, str]:
    tools = toolchain["tools"]
    path = os.pathsep.join(
        sorted({str(Path(row["resolved_path"]).parent) for row in tools.values()})
    )
    return {
        "HOME": str(root / "temp/home"),
        "TMPDIR": str(root / "temp"),
        "CARGO_HOME": str(root / "deps/cargo-home"),
        "CARGO_TARGET_DIR": str(root / "target"),
        "PATH": path,
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
    }


def run_engine(
    engine: str,
    closure: Path,
    paths: Mapping[str, Path],
    run_root: Path,
    sandbox_exec: Path,
    toolchain: Mapping[str, Any],
) -> dict[str, Any]:
    root = run_root / engine
    for role in ("temp", "target", "deps", "output"):
        (root / role).mkdir(parents=True, exist_ok=False)
    dependency_row = next(
        row
        for row in toolchain["dependency_outputs"]
        if row["name"] == f"{engine}-complete-cargo-native-closure"
    )
    shutil.copytree(
        Path(dependency_row["resolved_path"]),
        root / "deps",
        dirs_exist_ok=True,
        symlinks=False,
    )
    source = root / "source"
    copy_source(closure / "source", source)
    packet = root / "packet"
    copy_source(closure / "packet", packet)
    controls = root / "control"
    controls.mkdir()
    copied = {}
    for name, path in paths.items():
        if name == "promotion":
            continue
        target = controls / path.name
        shutil.copyfile(path, target)
        target.chmod(stat.S_IRUSR)
        copied[name] = target
    ancestry_repository = controls / "ancestry-repository"
    git_tool = str(toolchain["tools"]["git"]["resolved_path"])
    environment = {
        "HOME": str(root / "temp"),
        "LC_ALL": "C",
        "LANG": "C",
        "PATH": str(Path(git_tool).parent),
        "TZ": "UTC",
    }
    initialized = subprocess.run(
        [
            git_tool,
            "-c",
            "init.defaultBranch=master",
            "init",
            str(ancestry_repository),
        ],
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if initialized.returncode != 0:
        raise FinalChainError("ancestry proof repository initialization failed")
    indexed = subprocess.run(
        [git_tool, "-C", str(ancestry_repository), "index-pack", "--stdin", "--fix-thin"],
        env=environment,
        input=copied["ancestry_pack"].read_bytes(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if indexed.returncode != 0:
        raise FinalChainError("ancestry proof object-pack indexing failed")
    for path in ancestry_repository.rglob("*"):
        if path.is_symlink():
            raise FinalChainError(f"ancestry proof repository contains a symlink: {path}")
        path.chmod(stat.S_IRUSR | (stat.S_IXUSR if path.is_dir() else 0))
    ancestry_repository.chmod(stat.S_IRUSR | stat.S_IXUSR)
    promotion_controls = controls / "promotion"
    shutil.copytree(
        paths["promotion"].parent,
        promotion_controls,
        symlinks=False,
        copy_function=shutil.copyfile,
    )
    for path in promotion_controls.rglob("*"):
        if path.is_symlink():
            raise FinalChainError(f"promotion control contains a symlink: {path}")
        path.chmod(stat.S_IRUSR | (stat.S_IXUSR if path.is_dir() else 0))
    promotion_controls.chmod(stat.S_IRUSR | stat.S_IXUSR)
    copied["promotion"] = promotion_controls / paths["promotion"].name
    profile = root / "sandbox.sb"
    writable_roots = [root / role for role in ("temp", "target", "deps", "output")]
    immutable_roots = [
        source,
        packet,
        controls,
        *[
            Path(row["resolved_path"])
            for row in toolchain["dependency_outputs"]
        ],
    ]
    tool_read_roots = [
        Path("/System"),
        Path("/usr"),
        Path("/Library"),
        Path("/dev"),
        *{
            Path(row["resolved_path"]).parent
            for row in toolchain["tools"].values()
        },
        *immutable_roots,
        root,
    ]
    profile.write_text(
        sandbox_profile(tool_read_roots, writable_roots), encoding="utf-8"
    )
    profile.chmod(stat.S_IRUSR)
    verify_engine_sandbox(
        sandbox_exec, profile, immutable_roots, writable_roots
    )
    engine_rows = {
        row["id"]: row for row in read_json(paths["snapshot"])["engines"]
    }
    source_binding = engine_rows[engine]["source"]
    engine_source = bound_file(source, source_binding, f"{engine} engine")
    output = root / "output/engine-ledger.v1.json"
    if engine == "python":
        executable = toolchain["tools"]["python"]["resolved_path"]
        command = [executable, str(engine_source)]
    elif engine == "ruby":
        executable = toolchain["tools"]["ruby"]["resolved_path"]
        command = [executable, str(engine_source)]
    else:
        executable = toolchain["tools"]["rust"]["resolved_path"]
        binary = root / "target/rust-final-chain-engine"
        compile_result = subprocess.run(
            [
                str(sandbox_exec),
                "-f",
                str(profile),
                executable,
                "--edition=2021",
                str(engine_source),
                "-O",
                "-o",
                str(binary),
            ],
            cwd=source,
            env=engine_environment(root, toolchain),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if compile_result.returncode != 0:
            raise FinalChainError(
                f"Rust engine compilation failed: "
                f"{compile_result.stderr.decode('utf-8', 'replace')}"
            )
        command = [str(binary)]
    result = subprocess.run(
        [
            str(sandbox_exec),
            "-f",
            str(profile),
            *command,
            str(copied["snapshot"]),
            str(copied["manifest"]),
            str(copied["ledger"]),
            str(copied["readback"]),
            str(copied["toolchain"]),
            str(packet),
            str(source),
            str(output),
        ],
        cwd=source,
        env=engine_environment(root, toolchain),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise FinalChainError(
            f"{engine} engine refused: "
            f"{(result.stderr or result.stdout).decode('utf-8', 'replace')}"
        )
    value = read_json(output)
    raw = output.read_bytes()
    value["_engine_ledger_file"] = {
        "byte_length": len(raw),
        "sha256": digest(raw),
    }
    value["_produced_roots"] = [
        produced_root_identity(root / role, f"{engine}-{role}")
        for role in ("target", "deps", "output")
    ]
    return value


def produced_root_identity(root: Path, role: str) -> dict[str, object]:
    rows = []
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        if path.is_symlink():
            raise FinalChainError(f"produced root contains a symlink: {path}")
        raw = path.read_bytes()
        rows.append(
            {
                "path": path.relative_to(root).as_posix(),
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
        )
    return {
        "role": role,
        "file_count": len(rows),
        "byte_length": sum(int(row["byte_length"]) for row in rows),
        "identity": digest(canonical_bytes(rows)),
    }


def consensus(
    snapshot: Mapping[str, Any],
    paths: Mapping[str, Path],
    proofs: list[dict[str, Any]],
    receipts: list[dict[str, Any]],
) -> dict[str, Any]:
    by_engine = {receipt.get("engine"): receipt for receipt in receipts}
    if set(by_engine) != set(ENGINE_IDS):
        raise FinalChainError("engine ledgers are not one per independent engine")
    identities = {
        "snapshot_identity": digest(paths["snapshot"].read_bytes()),
        "input_manifest_identity": digest(paths["manifest"].read_bytes()),
        "ledger_identity": digest(paths["ledger"].read_bytes()),
        "readback_plan_identity": digest(paths["readback"].read_bytes()),
        "toolchain_identity": digest(paths["toolchain"].read_bytes()),
    }
    expected_ids = [row["proof_id"] for row in proofs]
    consensus_rows = []
    engine_ledgers = []
    produced = []
    semantic_consensus = [
        by_engine[engine].get("semantic_readback") for engine in ENGINE_IDS
    ]
    if semantic_consensus[1:] != semantic_consensus[:-1]:
        raise FinalChainError("semantic readback consensus differs")
    for engine in ENGINE_IDS:
        receipt = by_engine[engine]
        ledger_file = receipt.pop("_engine_ledger_file", None)
        produced_roots = receipt.pop("_produced_roots", None)
        if not isinstance(ledger_file, Mapping):
            raise FinalChainError(f"{engine} engine-ledger file identity is absent")
        if not isinstance(produced_roots, list) or len(produced_roots) != 3:
            raise FinalChainError(f"{engine} produced-root identity is absent")
        produced.extend(produced_roots)
        if any(receipt.get(key) != value for key, value in identities.items()):
            raise FinalChainError(f"{engine} binds another frozen control input")
        rows = receipt.get("proofs")
        if not isinstance(rows, list) or [row.get("proof_id") for row in rows] != expected_ids:
            raise FinalChainError(f"{engine} proof rows differ")
        if receipt.get("semantic_readback", {}).get("status") != "pass":
            raise FinalChainError(f"{engine} semantic readback failed")
        counts = receipt["semantic_readback"]
        if (
            counts.get("consumer_count"),
            counts.get("reader_count"),
            counts.get("hold_count"),
        ) != (0, 0, 0):
            raise FinalChainError(f"{engine} semantic zero counts differ")
        engine_ledgers.append(
            {
                "engine": engine,
                "byte_length": ledger_file["byte_length"],
                "sha256": ledger_file["sha256"],
            }
        )
    for index, proof in enumerate(proofs):
        rows = [by_engine[engine]["proofs"][index] for engine in ENGINE_IDS]
        typed = [
            {
                key: row.get(key)
                for key in (
                    "proof_id",
                    "stage",
                    "kind",
                    "command_identity",
                    "expected_outcome",
                    "actual_outcome",
                    "exit_code",
                    "produced_artifacts",
                    "fault_schedule_identity",
                    "injection_points_reached",
                    "fault_observation",
                    "cohort_identity",
                    "cohort_observation",
                    "harness_receipt_identity",
                    "edge_sweep_identity",
                    "edge_sweep",
                    "semantic_observation",
                )
            }
            for row in rows
        ]
        if typed[1:] != typed[:-1]:
            raise FinalChainError(f"typed row consensus differs: {proof['proof_id']}")
        if typed[0]["actual_outcome"] != proof["expected_outcome"]:
            raise FinalChainError(f"proof outcome differs: {proof['proof_id']}")
        if proof["kind"] in {"race", "crash_replay"} and not typed[0][
            "injection_points_reached"
        ]:
            raise FinalChainError(
                f"fault schedule reachability is absent: {proof['proof_id']}"
            )
        if proof["kind"] in {"migration", "rollback"} and not typed[0][
            "cohort_identity"
        ]:
            raise FinalChainError(f"cohort identity is absent: {proof['proof_id']}")
        consensus_rows.append(typed[0])
        produced.extend(typed[0]["produced_artifacts"])
    ancestry_rows = [
        row for row in consensus_rows if row.get("kind") == "ancestry"
    ]
    if len(ancestry_rows) != 1:
        raise FinalChainError(
            "exactly one independently executed ancestry row is required"
        )
    edge_sweep = ancestry_rows[0].get("edge_sweep")
    edge_identity = ancestry_rows[0].get("edge_sweep_identity")
    if (
        not isinstance(edge_sweep, Mapping)
        or edge_sweep.get("status") != "pass"
        or edge_sweep.get("checkpoint_count") != 13
        or not isinstance(edge_sweep.get("edges"), list)
        or len(edge_sweep["edges"]) != 12
        or not isinstance(edge_identity, str)
        or not edge_identity.startswith("sha256:")
    ):
        raise FinalChainError(
            "ancestry and edge sweep did not derive from engine proof rows"
        )
    return {
        "schema_version": SCHEMA,
        **identities,
        "final_integration": snapshot["final_integration"],
        "proof_count": len(proofs),
        "consensus_rows": consensus_rows,
        "engine_ledgers": engine_ledgers,
        "produced_artifacts": produced,
        "semantic_readback": semantic_consensus[0],
        "ancestry_closure": {
            "checkpoint_count": 13,
            "edges": edge_sweep["edges"],
            "identity": edge_identity,
        },
        "final_edge_sweep": {
            "status": "pass",
            "edge_count": 12,
            "identity": edge_identity,
        },
        "verdict": "pass",
    }


def read_descriptor(descriptor: int) -> bytes:
    chunks = []
    while True:
        chunk = os.read(descriptor, 1024 * 1024)
        if not chunk:
            return b"".join(chunks)
        chunks.append(chunk)


def open_directory_at(parent: int, name: str) -> int:
    return os.open(
        name,
        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
        dir_fd=parent,
    )


def ensure_directory_at(parent: int, name: str) -> int:
    try:
        os.mkdir(name, 0o550, dir_fd=parent)
        os.fsync(parent)
    except FileExistsError:
        pass
    return open_directory_at(parent, name)


def pointer_state_at(root_descriptor: int) -> dict[str, object]:
    try:
        descriptor = os.open(
            "current.json",
            os.O_RDONLY | os.O_NOFOLLOW,
            dir_fd=root_descriptor,
        )
    except FileNotFoundError:
        return {"state": "absent", "generation": 0}
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            raise FinalChainError("final pointer custody is unsafe")
        raw = read_descriptor(descriptor)
    finally:
        os.close(descriptor)
    value = json.loads(raw, object_pairs_hook=reject_duplicates)
    generation = value.get("generation") if isinstance(value, Mapping) else None
    if not isinstance(generation, int) or generation < 1:
        raise FinalChainError("final pointer generation is invalid")
    return {
        "state": "present",
        "generation": generation,
        "sha256": digest(raw),
    }


def write_file_at(directory: int, name: str, raw: bytes, mode: int = 0o440) -> None:
    descriptor = os.open(
        name,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
        mode,
        dir_fd=directory,
    )
    try:
        view = memoryview(raw)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def read_object_at(objects: int, name: str) -> dict[str, bytes]:
    descriptor = open_directory_at(objects, name)
    try:
        result: dict[str, bytes] = {}
        for child in os.listdir(descriptor):
            child_descriptor = os.open(
                child,
                os.O_RDONLY | os.O_NOFOLLOW,
                dir_fd=descriptor,
            )
            try:
                metadata = os.fstat(child_descriptor)
                if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                    raise FinalChainError("release object has an unsafe entry")
                result[child] = read_descriptor(child_descriptor)
            finally:
                os.close(child_descriptor)
        return result
    finally:
        os.close(descriptor)


def immutable_payload(
    receipt: Mapping[str, Any], paths: Mapping[str, Path]
) -> dict[str, bytes]:
    payload = {"final-cumulative-seal-receipt.v1.json": canonical_bytes(receipt)}
    for name, path in paths.items():
        payload[path.name] = path.read_bytes()
    manifest_rows = [
        {"path": name, "byte_length": len(raw), "sha256": digest(raw)}
        for name, raw in sorted(payload.items())
    ]
    payload["release-manifest.v1.json"] = canonical_bytes(
        {
            "schema_version": "maestro.external.vnext-final-release-manifest.v1",
            "file_count": len(manifest_rows),
            "byte_length": sum(row["byte_length"] for row in manifest_rows),
            "files": manifest_rows,
        }
    )
    return payload


def publish(
    receipt: Mapping[str, Any],
    paths: Mapping[str, Path],
    publication_root: Path,
    pointer_preimage: Mapping[str, Any],
    publication_identity: Mapping[str, Any],
    expected_generation: int,
) -> str:
    payload = immutable_payload(receipt, paths)
    release_identity = digest(
        canonical_bytes(
            {
                "files": [
                    {"path": name, "byte_length": len(raw), "sha256": digest(raw)}
                    for name, raw in sorted(payload.items())
                ]
            }
        )
    )
    root_descriptor = os.open(
        publication_root,
        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
    )
    try:
        custody = os.fstat(root_descriptor)
        actual_identity = {
            "path": str(publication_root),
            "device": custody.st_dev,
            "inode": custody.st_ino,
            "mount_device": custody.st_dev,
            "mode": stat.S_IMODE(custody.st_mode),
            "link_count": custody.st_nlink,
            "ctime_ns": custody.st_ctime_ns,
        }
        if actual_identity != dict(publication_identity):
            raise FinalChainError("publication root identity or mount custody changed")
        objects_descriptor = ensure_directory_at(root_descriptor, "objects")
        try:
            object_name = release_identity.removeprefix("sha256:")
            object_preexisted = True
            try:
                actual = read_object_at(objects_descriptor, object_name)
            except FileNotFoundError:
                object_preexisted = False
                temporary_name = f".object-{secrets.token_hex(16)}"
                os.mkdir(temporary_name, 0o550, dir_fd=objects_descriptor)
                temporary_descriptor = open_directory_at(
                    objects_descriptor, temporary_name
                )
                try:
                    for name, raw in payload.items():
                        write_file_at(temporary_descriptor, name, raw)
                    os.fsync(temporary_descriptor)
                finally:
                    os.close(temporary_descriptor)
                os.rename(
                    temporary_name,
                    object_name,
                    src_dir_fd=objects_descriptor,
                    dst_dir_fd=objects_descriptor,
                )
                os.fsync(objects_descriptor)
                actual = read_object_at(objects_descriptor, object_name)
            if object_preexisted and actual != payload:
                raise FinalChainError("pre-existing release object bytes differ")
            if read_object_at(objects_descriptor, object_name) != payload:
                raise FinalChainError("release-object semantic readback differs")
        finally:
            os.close(objects_descriptor)
        lock_descriptor = os.open(
            ".current.lock",
            os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW,
            0o600,
            dir_fd=root_descriptor,
        )
        try:
            fcntl.flock(lock_descriptor, fcntl.LOCK_EX)
            current = pointer_state_at(root_descriptor)
            if current != dict(pointer_preimage):
                raise FinalChainError("final pointer advanced after snapshot freeze")
            if current["generation"] != expected_generation:
                raise FinalChainError("final pointer generation differs")
            next_generation = expected_generation + 1
            generations_descriptor = ensure_directory_at(
                root_descriptor, "generations"
            )
            try:
                marker = canonical_bytes(
                    {
                        "expected_generation": expected_generation,
                        "generation": next_generation,
                        "pointer_preimage": pointer_preimage,
                        "release_identity": release_identity,
                    }
                )
                write_file_at(
                    generations_descriptor,
                    f"{next_generation:020d}.json",
                    marker,
                )
                os.fsync(generations_descriptor)
            finally:
                os.close(generations_descriptor)
            pointer = {
                "schema_version": POINTER_SCHEMA,
                "generation": next_generation,
                "object": f"objects/{release_identity.removeprefix('sha256:')}",
                "release_identity": release_identity,
            }
            desired = canonical_bytes(pointer)
            temporary_name = f".current-{secrets.token_hex(16)}"
            write_file_at(root_descriptor, temporary_name, desired)
            os.rename(
                temporary_name,
                "current.json",
                src_dir_fd=root_descriptor,
                dst_dir_fd=root_descriptor,
            )
            os.fsync(root_descriptor)
            if pointer_state_at(root_descriptor) != {
                "state": "present",
                "generation": next_generation,
                "sha256": digest(desired),
            }:
                raise FinalChainError("final pointer descriptor readback differs")
        finally:
            fcntl.flock(lock_descriptor, fcntl.LOCK_UN)
            os.close(lock_descriptor)
        return release_identity
    finally:
        os.close(root_descriptor)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--closure-root", type=Path, required=True)
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--publication-root", type=Path, required=True)
    parser.add_argument("--sandbox-exec", type=Path, default=Path("/usr/bin/sandbox-exec"))
    args = parser.parse_args()
    closure = args.closure_root.resolve()
    run_root = args.run_root.resolve()
    publication_argument = args.publication_root.absolute()
    if publication_argument.is_symlink():
        raise FinalChainError("publication root may not be a symlink")
    publication = publication_argument.resolve(strict=True)
    if run_root.exists():
        raise FinalChainError(f"run root already exists: {run_root}")
    snapshot, paths = validate_snapshot(closure)
    protected = Path(snapshot["protected_primary_checkout"]).resolve()
    for path, label in (
        (closure, "closure"),
        (run_root, "run"),
        (publication, "publication"),
    ):
        if path == protected or path.is_relative_to(protected):
            raise FinalChainError(f"{label} root enters the protected primary checkout")
    roots = [closure, run_root, publication]
    for index, root in enumerate(roots):
        if any(
            root == other or root.is_relative_to(other) or other.is_relative_to(root)
            for other in roots[index + 1 :]
        ):
            raise FinalChainError("closure, run, and publication roots are not disjoint")
    run_root.mkdir(parents=True)
    verify_sandbox(args.sandbox_exec.resolve(), run_root, protected)
    toolchain = validate_toolchain(paths["toolchain"], closure / "source")
    proofs = validate_ledger(
        paths["ledger"], closure / "source", snapshot["final_integration"]["commit"]
    )
    receipts = [
        run_engine(
            engine,
            closure,
            paths,
            run_root,
            args.sandbox_exec.resolve(),
            toolchain,
        )
        for engine in ENGINE_IDS
    ]
    receipt = consensus(snapshot, paths, proofs, receipts)
    release_identity = publish(
        receipt,
        paths,
        publication,
        snapshot["pointer_preimage"],
        snapshot["publication_root_identity"],
        snapshot["expected_generation"],
    )
    print(release_identity)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except FinalChainError as error:
        print(f"final-chain seal refused: {error}", file=os.sys.stderr)
        raise SystemExit(2)
