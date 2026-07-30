#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SNAPSHOT_ROOT = ROOT
ANCESTRY_REPOSITORY = ROOT
FINAL_REF = "HEAD"
V2_DESCRIPTOR_PATHS = (
    "embedded/vnext/hosts/agents-compatible-cli.v2.json",
    "embedded/vnext/hosts/claude-code.v2.json",
)
V2_PATTERN_PATH = "embedded/vnext/patterns/trusted-host-diagnostic.v2.json"
V2_SCHEMA_PATH = "embedded/vnext/schemas/host-descriptor.v2.json"
INACTIVE_REASON = "supported_host_native_provider_unavailable"
GIT_ENV = {
    **os.environ,
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_GLOBAL": "/dev/null",
}


def fail(message: str) -> None:
    raise SystemExit(f"Stage12Product validation failed: {message}")


def run(*args: str) -> str:
    result = subprocess.run(
        args,
        cwd=ANCESTRY_REPOSITORY,
        env=GIT_ENV,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(relative: str) -> object:
    path = SNAPSHOT_ROOT / relative
    if not path.is_file():
        fail(f"missing required file: {relative}")
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"invalid JSON in {relative}: {error}")


def changed_paths(base: str, target: str) -> set[str]:
    return {
        line
        for line in run(
            "git",
            "diff",
            "--no-renames",
            "--name-only",
            base,
            target,
        ).splitlines()
        if line
    }


def first_parent_commits(ancestor: str, descendant: str) -> list[str]:
    return [
        line
        for line in run(
            "git",
            "rev-list",
            "--first-parent",
            "--reverse",
            f"{ancestor}..{descendant}",
        ).splitlines()
        if line
    ]


def base_blob(base: str, relative: str) -> tuple[str, str, str] | None:
    row = run("git", "ls-tree", base, "--", relative).strip()
    if not row:
        return None
    metadata, path = row.split("\t", 1)
    mode, object_type, object_id = metadata.split()
    if path != relative:
        fail(f"unexpected base tree path for {relative}: {path}")
    return mode, object_type, object_id


def require_ancestor(ancestor: str, descendant: str, label: str) -> None:
    if run("git", "merge-base", ancestor, descendant).strip() != ancestor:
        fail(f"{label} ancestry differs: {ancestor} is not an ancestor of {descendant}")


def tree_id(commit: str) -> str:
    return run("git", "rev-parse", f"{commit}^{{tree}}").strip()


def snapshot_blob(relative: str) -> str:
    path = SNAPSHOT_ROOT / relative
    try:
        bytes_ = path.read_bytes()
    except OSError as error:
        fail(f"cannot read snapshot file {relative}: {error}")
    result = subprocess.run(
        ("git", "hash-object", "--stdin"),
        cwd=ANCESTRY_REPOSITORY,
        env=GIT_ENV,
        input=bytes_,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(
            f"cannot hash snapshot file {relative}: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return result.stdout.decode().strip()


def is_product_owned(
    relative: str,
    existing: set[str],
    planned_new: set[str],
    prefixes: tuple[str, ...],
) -> bool:
    return (
        relative in existing
        or relative in planned_new
        or relative.startswith(prefixes)
    )


def find_correction_checkpoint(
    correction_predecessor: str,
    descendant: str,
    existing: set[str],
    planned_new: set[str],
    prefixes: tuple[str, ...],
) -> tuple[str, set[str]]:
    expected_parent = correction_predecessor
    checkpoint = ""
    correction_changed: set[str] = set()
    for commit in first_parent_commits(correction_predecessor, descendant):
        parent = run("git", "rev-parse", f"{commit}^1").strip()
        if parent != expected_parent:
            fail("Stage12 correction first-parent chain drifted")
        changed = changed_paths(parent, commit)
        owned_changed = {
            relative
            for relative in changed
            if is_product_owned(relative, existing, planned_new, prefixes)
        }
        unowned_changed = changed - owned_changed
        if owned_changed and unowned_changed:
            fail(
                "first-parent commit mixes Stage12Product-owned and unowned paths: "
                f"{sorted(unowned_changed)[0]}"
            )
        if owned_changed:
            for relative in owned_changed:
                blob = base_blob(commit, relative)
                if (
                    blob is None
                    or blob[0] not in {"100644", "100755"}
                    or blob[1] != "blob"
                ):
                    fail(
                        "Stage12 correction path is not a regular blob: "
                        f"{relative}"
                    )
            summary = run("git", "diff", "--summary", parent, commit)
            for forbidden in ("delete mode", "rename from", "rename to", "mode change"):
                if forbidden in summary:
                    fail(f"forbidden Stage12 correction shape present: {forbidden}")
            checkpoint = commit
            correction_changed.update(owned_changed)
        expected_parent = commit
    if not checkpoint:
        fail("Stage12 correction has no owner-only checkpoint")
    require_ancestor(checkpoint, descendant, "Stage12 correction checkpoint")
    return checkpoint, correction_changed


def validate_ownership(manifest: dict[str, object]) -> set[str]:
    base = str(manifest["base"])
    product_checkpoint = str(manifest["product_checkpoint"])
    affected_suffix_parent = str(manifest["affected_suffix_parent"])
    affected_suffix_checkpoint = str(manifest["affected_suffix_checkpoint"])
    affected_suffix_tree = str(manifest["affected_suffix_tree"])
    correction_predecessor = str(manifest["correction_predecessor"])
    historical_validation_checkpoint = manifest.get(
        "historical_validation_checkpoint"
    )
    integration_checkpoint = manifest.get("integration_checkpoint")
    integration_tree = manifest.get("integration_tree")
    canonical_merge_checkpoint = manifest.get("canonical_merge_checkpoint")
    canonical_merge_first_parent = manifest.get("canonical_merge_first_parent")
    canonical_merge_tree = manifest.get("canonical_merge_tree")
    affected_suffix_proof_inputs = set(manifest["affected_suffix_proof_inputs"])
    existing = set(manifest["existing_exact_files"])
    planned_new = set(manifest["planned_new_exact_files"])
    prefixes = tuple(manifest["path_prefixes"])
    denied = dict(manifest["explicit_deny_files"])
    if manifest["owner"] != "Stage12Product" or manifest["default_deny_unowned"] is not True:
        fail("ownership manifest must bind Stage12Product with default-deny")
    if len(existing) != 24 or len(planned_new) != 7 or prefixes != ("tests/vnext_stage12_",):
        fail("ownership manifest does not match the V7 24/7/one-prefix grant")
    if existing & planned_new:
        fail("existing and planned-new exact path sets overlap")
    if affected_suffix_proof_inputs != {
        "tests/fixtures/vnext/stage10/trusted-host-parity.v1.json",
        "tools/vnext_contracts/stage10/path-manifest.v1.json",
        "tools/vnext_contracts/stage10/validate.py",
    }:
        fail("affected-suffix proof input closure drifted")
    if not affected_suffix_proof_inputs <= existing:
        fail("affected-suffix proof input is outside the original exact grant")
    require_ancestor(base, product_checkpoint, "original Stage12 product")
    require_ancestor(product_checkpoint, affected_suffix_parent, "affected Stage12 suffix")
    require_ancestor(
        affected_suffix_parent,
        affected_suffix_checkpoint,
        "affected Stage12 suffix checkpoint",
    )
    require_ancestor(
        affected_suffix_checkpoint,
        FINAL_REF,
        "final candidate affected Stage12 suffix",
    )
    if integration_checkpoint is None:
        if manifest.get("schema") != "maestro.vnext.stage12-product-path-manifest.v3":
            fail("historical Stage12 ownership manifest schema drifted")
        correction_checkpoint, correction_changed = find_correction_checkpoint(
            correction_predecessor,
            FINAL_REF,
            existing,
            planned_new,
            prefixes,
        )
    else:
        if (
            manifest.get("schema")
            != "maestro.vnext.stage12-product-path-manifest.v4"
            or not isinstance(historical_validation_checkpoint, str)
            or not isinstance(integration_checkpoint, str)
            or not isinstance(integration_tree, str)
            or not isinstance(canonical_merge_checkpoint, str)
            or not isinstance(canonical_merge_first_parent, str)
            or not isinstance(canonical_merge_tree, str)
        ):
            fail("post-merge Stage12 ownership manifest closure drifted")
        require_ancestor(
            correction_predecessor,
            historical_validation_checkpoint,
            "Stage12 correction",
        )
        require_ancestor(
            historical_validation_checkpoint,
            integration_checkpoint,
            "Stage12 V8 integration",
        )
        if tree_id(integration_checkpoint) != integration_tree:
            fail("Stage12 V8 integration tree drifted")
        require_ancestor(
            integration_checkpoint,
            canonical_merge_checkpoint,
            "Stage12 candidate integration",
        )
        require_ancestor(
            canonical_merge_checkpoint,
            FINAL_REF,
            "Stage12 canonical merge",
        )
        merge_row = run(
            "git",
            "rev-list",
            "--parents",
            "-n",
            "1",
            canonical_merge_checkpoint,
        ).split()
        if merge_row != [
            canonical_merge_checkpoint,
            canonical_merge_first_parent,
            integration_checkpoint,
        ]:
            fail("canonical Stage12 merge parent closure drifted")
        if tree_id(canonical_merge_checkpoint) != canonical_merge_tree:
            fail("canonical Stage12 merge tree drifted")
        historical_checkpoint, historical_changed = find_correction_checkpoint(
            correction_predecessor,
            historical_validation_checkpoint,
            existing,
            planned_new,
            prefixes,
        )
        merge_owned_changed = {
            relative
            for relative in changed_paths(
                integration_checkpoint,
                canonical_merge_checkpoint,
            )
            if is_product_owned(relative, existing, planned_new, prefixes)
        }
        for relative in merge_owned_changed:
            blob = base_blob(canonical_merge_checkpoint, relative)
            if (
                blob is None
                or blob[0] not in {"100644", "100755"}
                or blob[1] != "blob"
            ):
                fail(f"canonical merge Stage12 path is not a regular blob: {relative}")
        correction_checkpoint, post_merge_changed = find_correction_checkpoint(
            canonical_merge_checkpoint,
            FINAL_REF,
            existing,
            planned_new,
            prefixes,
        )
        correction_changed = (
            historical_changed | merge_owned_changed | post_merge_changed
        )
        require_ancestor(
            historical_checkpoint,
            historical_validation_checkpoint,
            "historical Stage12 correction checkpoint",
        )
    if tree_id(affected_suffix_checkpoint) != affected_suffix_tree:
        fail("affected Stage12 suffix checkpoint tree drifted")
    for relative in existing:
        blob = base_blob(base, relative)
        if blob is None or blob[0] not in {"100644", "100755"} or blob[1] != "blob":
            fail(f"existing exact path lacks a regular-blob preimage: {relative}")
    for relative in planned_new:
        if base_blob(base, relative) is not None:
            fail(f"planned-new exact path exists at the V7 design preimage: {relative}")
    original_changed = changed_paths(base, product_checkpoint)
    if len(original_changed) != 22:
        fail("original Stage12 product checkpoint does not preserve the exact 22-path change")
    for relative in original_changed:
        if relative in denied:
            fail(f"immutable V1 path changed: {relative}")
        if relative not in existing and relative not in planned_new and not relative.startswith(prefixes):
            fail(f"original Stage12 checkpoint changed an unowned path: {relative}")
        checkpoint_blob = base_blob(product_checkpoint, relative)
        if checkpoint_blob is None or checkpoint_blob[0] not in {"100644", "100755"} or checkpoint_blob[1] != "blob":
            fail(f"original Stage12 checkpoint path is not a regular blob: {relative}")
    affected_changed = changed_paths(affected_suffix_parent, affected_suffix_checkpoint)
    if affected_changed != affected_suffix_proof_inputs:
        fail("affected Stage12 suffix is not the exact three-file proof-input rebind")
    affected_summary = run(
        "git",
        "diff",
        "--summary",
        affected_suffix_parent,
        affected_suffix_checkpoint,
    )
    for forbidden in ("delete mode", "rename from", "rename to", "mode change"):
        if forbidden in affected_summary:
            fail(f"forbidden affected-suffix change shape present: {forbidden}")

    for relative in correction_changed:
        if not is_product_owned(relative, existing, planned_new, prefixes):
            fail(f"Stage12 correction changed an unowned path: {relative}")
        blob = base_blob(correction_checkpoint, relative)
        if blob is None or blob[0] not in {"100644", "100755"} or blob[1] != "blob":
            fail(f"Stage12 correction path is not a regular blob: {relative}")

    checkpoint_prefix_paths = {
        relative
        for relative in run(
            "git",
            "ls-tree",
            "-r",
            "--name-only",
            correction_checkpoint,
        ).splitlines()
        if relative.startswith(prefixes)
    }
    final_prefix_paths = {
        relative
        for relative in run("git", "ls-tree", "-r", "--name-only", FINAL_REF).splitlines()
        if relative.startswith(prefixes)
    }
    if final_prefix_paths != checkpoint_prefix_paths:
        fail("Stage12-owned prefix path set changed after the correction checkpoint")
    snapshot_tests = SNAPSHOT_ROOT / "tests"
    snapshot_prefix_paths = {
        path.relative_to(SNAPSHOT_ROOT).as_posix()
        for path in snapshot_tests.glob("vnext_stage12_*")
        if path.is_file() and not path.is_symlink()
    }
    if snapshot_prefix_paths != final_prefix_paths:
        fail("Stage12-owned prefix paths differ between final ref and snapshot")
    checkpoint_product_paths = existing | planned_new | checkpoint_prefix_paths
    for relative in checkpoint_product_paths:
        checkpoint_blob = base_blob(correction_checkpoint, relative)
        final_blob = base_blob(FINAL_REF, relative)
        if (
            checkpoint_blob is None
            or checkpoint_blob[0] not in {"100644", "100755"}
            or checkpoint_blob[1] != "blob"
        ):
            fail(f"correction checkpoint Stage12-owned path is not a regular blob: {relative}")
        if (
            final_blob is None
            or final_blob[0] not in {"100644", "100755"}
            or final_blob[1] != "blob"
        ):
            fail(f"final Stage12-owned path is not a regular blob: {relative}")
        if final_blob != checkpoint_blob:
            fail(f"final Stage12-owned path differs from correction checkpoint: {relative}")
        path = SNAPSHOT_ROOT / relative
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            fail(f"final snapshot is missing Stage12-owned path: {relative}")
        if not stat.S_ISREG(mode) or path.is_symlink():
            fail(f"final snapshot Stage12 path is not a regular file: {relative}")
        snapshot_mode = "100755" if mode & stat.S_IXUSR else "100644"
        if snapshot_mode != checkpoint_blob[0]:
            fail(f"final snapshot Stage12 mode differs: {relative}")
        if snapshot_blob(relative) != checkpoint_blob[2]:
            fail(f"final snapshot Stage12 bytes differ: {relative}")
    for relative, expected in denied.items():
        if digest(SNAPSHOT_ROOT / relative) != expected:
            fail(f"immutable V1 bytes drifted: {relative}")
    return correction_changed


def validate_v2_resources() -> None:
    schema = load_json(V2_SCHEMA_PATH)
    if schema["$id"] != "maestro.vnext.host-descriptor.v2":
        fail("HostDescriptorV2 schema identity drifted")
    if schema["additionalProperties"] is not False:
        fail("HostDescriptorV2 root must reject unknown fields")
    variants = schema["$defs"]["ProtectedRuntimeActivationBindingV2"]["oneOf"]
    if len(variants) != 2 or any(row["additionalProperties"] is not False for row in variants):
        fail("ProtectedRuntimeActivationBindingV2 must have two closed variants")
    if variants[0]["required"] != ["variant", "reason_code"]:
        fail("Inactive activation binding shape drifted")
    if len(variants[1]["required"]) != 8:
        fail("Active activation binding must bind all eight provider/currentness fields")
    expected_profiles = ["agents-compatible-cli", "claude-code"]
    for relative, profile in zip(V2_DESCRIPTOR_PATHS, expected_profiles, strict=True):
        descriptor = load_json(relative)
        expected_keys = {
            "schema",
            "profile_id",
            "installation_scope",
            "project_registration",
            "public_skill",
            "currentness_source",
            "protected_runtime_activation",
        }
        if set(descriptor) != expected_keys:
            fail(f"HostDescriptorV2 root shape drifted: {relative}")
        if descriptor["schema"] != "maestro.vnext.host-descriptor.v2":
            fail(f"HostDescriptorV2 schema marker drifted: {relative}")
        if descriptor["profile_id"] != profile:
            fail(f"HostDescriptorV2 profile drifted: {relative}")
        if descriptor["installation_scope"] != "global-user-agent-installation":
            fail(f"HostDescriptorV2 scope drifted: {relative}")
        if descriptor["project_registration"] is not False:
            fail(f"HostDescriptorV2 project registration must remain false: {relative}")
        activation = descriptor["protected_runtime_activation"]
        if activation != {"variant": "Inactive", "reason_code": INACTIVE_REASON}:
            fail(f"current host profile is not truthfully inactive: {relative}")
    pattern = load_json(V2_PATTERN_PATH)
    if pattern["host_descriptor_schema"] != "maestro.vnext.host-descriptor.v2":
        fail("trusted-host V2 pattern does not bind HostDescriptorV2")
    if pattern["current_profiles"] != {"variant": "Inactive", "reason_code": INACTIVE_REASON}:
        fail("trusted-host V2 pattern overclaims current provider activation")
    if pattern["mcp_tools"] != ["maestro_packet", "maestro_cli_search"]:
        fail("trusted-host V2 pattern does not preserve the exact-two MCP surface")
    if pattern["ambient_fallback"] is not False or pattern["runtime_activation"] is not False:
        fail("trusted-host V2 pattern admits ambient fallback or false activation")


def validate_product_sources() -> None:
    descriptor = load_json("embedded/vnext/adapter/mcp-tools.v1.json")
    if [row["name"] for row in descriptor["tools"]] != [
        "maestro_packet",
        "maestro_cli_search",
    ]:
        fail("MCP production descriptor is not the exact ordered pair")
    if descriptor["candidate_only"] is not False:
        fail("exact-two MCP production descriptor remains candidate-only")
    if descriptor["runtime_activation"] is not True or descriptor["runtime_registration"] is not True:
        fail("exact-two MCP production descriptor is not activated and registered")
    if any(
        not isinstance(row.get("description"), str)
        or not row["description"]
        or not isinstance(row.get("request_schema"), str)
        or not isinstance(row.get("response_schema"), str)
        or row["read_only"] is not True
        or row["writes"] is not False
        or row["network_io"] is not False
        for row in descriptor["tools"]
    ):
        fail("exact-two MCP descriptor is incomplete or contains a non-read-only Tool")
    connectors = (SNAPSHOT_ROOT / "src/interfaces/connectors/mod.rs").read_text()
    for required in (
        "HostDescriptorV2",
        "ProtectedRuntimeActivationBindingV2",
        "LiveAuthenticatedHostConnectionV1",
        "acquire_trusted_host_diagnostic_connection",
        "Stage10OwnerLocalConnectionSeedV1::acquire_from_designated_connector",
        "connection.provider_implementation_identity()",
        "connection.production_conformance_proof_identity()",
        "connection.production_negative_proof_identity()",
        "connection.binary_identity()",
        "connection.release_id()",
        "supported_host_native_provider_unavailable",
        "agents-compatible-cli.v2.json",
        "claude-code.v2.json",
    ):
        if required not in connectors:
            fail(f"host-native connector seam is missing {required}")
    for forbidden in (
        "agents-compatible-cli.v1.json",
        "claude-code.v1.json",
        "std::env::var",
        "credential",
        "global_registry",
    ):
        if forbidden in connectors:
            fail(f"host-native connector seam contains forbidden fallback source: {forbidden}")
    if connectors.count(
        "Stage10OwnerLocalConnectionSeedV1::acquire_from_designated_connector("
    ) != 1:
        fail("designated connector must be the exact-one seed-construction caller")
    seed = (
        SNAPSHOT_ROOT
        / "src/domain/integration/trusted_host_diagnostic_stage10_seed.rs"
    ).read_text()
    if "pub(crate) fn acquire_from_designated_connector(" not in seed:
        fail("Integration seed lacks its caller-censused designated constructor")
    if "acquire_from_authenticated_host" in seed or "acquire_from_authenticated_host" in connectors:
        fail("raw Stage10 host seed constructor remains reachable")
    packet = (SNAPSHOT_ROOT / "src/interfaces/cli/packet.rs").read_text()
    for required in (
        "request.repository_locator",
        "open_explicit_repository",
    ):
        if required not in packet:
            fail(f"Packet adapter does not enforce explicit alias-closed locator: {required}")
    packet_production = packet.split("#[cfg(test)]", 1)[0]
    if (
        "discover_repo_root" in packet_production
        or "current_dir()" in packet_production
        or "canonicalize()" in packet_production
    ):
        fail("Packet production adapter discovers repository identity from cwd")
    adapter_facade = (SNAPSHOT_ROOT / "src/operations/adapters/mod.rs").read_text()
    for required in (
        "mod live_projection;",
        "pub(crate) use live_projection::{",
        "decode_cli_search_request",
        "encode_cli_search_envelope",
        "RunningBinaryIdentityV1",
        "GlobalMcpAdapterKindV1",
        "global_mcp_adapter",
        "packet_read_with_protected_continuity",
    ):
        if required not in adapter_facade:
            fail(f"binary-local CLI search facade is missing {required}")
    if "pub(crate) mod live_projection;" in adapter_facade:
        fail("binary-local CLI search exposes its implementation leaf")
    search = (SNAPSHOT_ROOT / "src/operations/adapters/live_projection.rs").read_text()
    for required in (
        "decode_cli_search_request",
        "encode_cli_search_envelope",
        "RunningBinaryIdentityV1",
        "GeneratedCapabilityCatalogV1::load_frozen",
    ):
        if required not in search:
            fail(f"binary-local CLI search adapter is missing {required}")
    projection = (SNAPSHOT_ROOT / "src/operations/adapters/live_projection.rs").read_text()
    for required in (
        "SecureRoot::open",
        "verify_path_binding",
        "validate_regular_file",
        "open_dir",
        "explicit_component_normal_locator",
    ):
        if required not in projection:
            fail(f"Projection lacks descriptor-root enforcement: {required}")
    projection_production = projection.split("#[cfg(test)]", 1)[0]
    if "canonicalize()" in projection_production or ".exists()" in projection_production:
        fail("Projection reopens or probes repository state by pathname")


def validate_parity() -> None:
    parity = load_json("tests/fixtures/vnext/stage10/trusted-host-parity.v1.json")
    if parity["schema"] != "maestro.vnext.stage10.trusted-host-parity.v4":
        fail("trusted-host parity schema drifted")
    if parity["protected_runtime_activation"] is not False or parity["ambient_fallback"] is not False:
        fail("trusted-host parity overclaims provider activation")
    for reference in parity["upstream_references"]:
        if digest(SNAPSHOT_ROOT / reference["path"]) != reference["sha256"]:
            fail(f"trusted-host upstream parity drifted: {reference['path']}")
    gap = load_json("tools/vnext_contracts/stage10/interface-gap.v2.json")
    if gap["status"] != "host-native-injection-seam-bound-with-inactive-profiles":
        fail("Stage-10 interface gap does not describe the V7 host-native seam")
    if gap["protected_runtime_activation"] is not False:
        fail("Stage-10 interface gap overclaims a current supported host provider")
    proof = load_json("tools/vnext_contracts/stage10/proof-matrix.v1.json")
    if proof["base"] != "ff454521b7037d5df7b8e836b8ce30f77e1ff8dc":
        fail("Stage-10 proof matrix is not bound to the V7 design commit")
    if len(proof["required"]) != 12:
        fail("Stage-10/12 product proof matrix is incomplete")


def main() -> int:
    global ANCESTRY_REPOSITORY, FINAL_REF, SNAPSHOT_ROOT

    parser = argparse.ArgumentParser()
    parser.add_argument("--ancestry-repository", type=Path, default=ROOT)
    parser.add_argument("--snapshot-root", type=Path, default=ROOT)
    parser.add_argument("--final-ref", default="HEAD")
    args = parser.parse_args()
    try:
        ANCESTRY_REPOSITORY = args.ancestry_repository.resolve(strict=True)
        SNAPSHOT_ROOT = args.snapshot_root.resolve(strict=True)
    except OSError as error:
        fail(f"invalid ancestry repository or snapshot root: {error}")
    if not ANCESTRY_REPOSITORY.is_dir() or not SNAPSHOT_ROOT.is_dir():
        fail("ancestry repository and snapshot root must be directories")
    FINAL_REF = run(
        "git",
        "rev-parse",
        "--verify",
        f"{args.final_ref}^{{commit}}",
    ).strip()

    manifest = load_json("tools/vnext_contracts/stage10/path-manifest.v1.json")
    changed = validate_ownership(manifest)
    validate_v2_resources()
    validate_product_sources()
    validate_parity()
    print(
        json.dumps(
            {
                "schema": "maestro.external.stage12-product-validation.v4",
                "base": manifest["base"],
                "product_checkpoint": manifest["product_checkpoint"],
                "affected_suffix_parent": manifest["affected_suffix_parent"],
                "affected_suffix_checkpoint": manifest["affected_suffix_checkpoint"],
                "affected_suffix_tree": manifest["affected_suffix_tree"],
                "integration_checkpoint": manifest.get("integration_checkpoint"),
                "canonical_merge_checkpoint": manifest.get(
                    "canonical_merge_checkpoint"
                ),
                "final_ref": run("git", "rev-parse", FINAL_REF).strip(),
                "original_changed_path_count": 22,
                "changed_path_count": len(changed),
                "host_profiles": "v2-inactive",
                "mcp_tools": ["maestro_packet", "maestro_cli_search"],
                "ownership": "Stage12Product-default-deny",
                "status": "provisional-unintegrated-unverified",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
