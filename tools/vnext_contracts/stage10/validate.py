#!/usr/bin/env python3
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = ROOT / "tools/vnext_contracts/stage10/path-manifest.v1.json"
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
        cwd=ROOT,
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
    path = ROOT / relative
    if not path.is_file():
        fail(f"missing required file: {relative}")
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"invalid JSON in {relative}: {error}")


def changed_paths(base: str) -> set[str]:
    tracked = {
        line
        for line in run("git", "diff", "--name-only", base).splitlines()
        if line
    }
    untracked = {
        line
        for line in run("git", "ls-files", "--others", "--exclude-standard").splitlines()
        if line
    }
    return tracked | untracked


def base_blob(base: str, relative: str) -> tuple[str, str, str] | None:
    row = run("git", "ls-tree", base, "--", relative).strip()
    if not row:
        return None
    metadata, path = row.split("\t", 1)
    mode, object_type, object_id = metadata.split()
    if path != relative:
        fail(f"unexpected base tree path for {relative}: {path}")
    return mode, object_type, object_id


def validate_ownership(manifest: dict[str, object]) -> set[str]:
    base = str(manifest["base"])
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
    for relative in existing:
        blob = base_blob(base, relative)
        if blob is None or blob[0] not in {"100644", "100755"} or blob[1] != "blob":
            fail(f"existing exact path lacks a regular-blob preimage: {relative}")
    for relative in planned_new:
        if base_blob(base, relative) is not None:
            fail(f"planned-new exact path exists at the V7 design preimage: {relative}")
    changed = changed_paths(base)
    for relative in changed:
        if relative in denied:
            fail(f"immutable V1 path changed: {relative}")
        if relative not in existing and relative not in planned_new and not relative.startswith(prefixes):
            fail(f"default-deny rejected unowned path: {relative}")
        path = ROOT / relative
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            fail(f"changed path was deleted: {relative}")
        if not stat.S_ISREG(mode) or path.is_symlink():
            fail(f"changed path is not a regular file: {relative}")
    summary = run("git", "diff", "--summary", base)
    for forbidden in ("delete mode", "rename from", "rename to", "mode change"):
        if forbidden in summary:
            fail(f"forbidden change shape present: {forbidden}")
    for relative, expected in denied.items():
        if digest(ROOT / relative) != expected:
            fail(f"immutable V1 bytes drifted: {relative}")
    return changed


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
        row["read_only"] is not True
        or row["writes"] is not False
        or row["network_io"] is not False
        for row in descriptor["tools"]
    ):
        fail("exact-two MCP descriptor contains a non-read-only Tool")
    connectors = (ROOT / "src/interfaces/connectors/mod.rs").read_text()
    for required in (
        "HostDescriptorV2",
        "ProtectedRuntimeActivationBindingV2",
        "LiveAuthenticatedHostConnectionV1",
        "acquire_trusted_host_diagnostic_connection",
        "Stage10OwnerLocalConnectionSeedV1::acquire_from_authenticated_host",
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
    packet = (ROOT / "src/interfaces/cli/packet.rs").read_text()
    for required in (
        "request.repository_locator",
        "is_absolute()",
        "canonicalize()",
        "alias-closed canonical path",
    ):
        if required not in packet:
            fail(f"Packet adapter does not enforce explicit alias-closed locator: {required}")
    if "discover_repo_root" in packet or "current_dir()" in packet.split("#[cfg(test)]", 1)[0]:
        fail("Packet production adapter discovers repository identity from cwd")
    search = (ROOT / "src/operations/adapters/mod.rs").read_text()
    for required in (
        "decode_cli_search_request",
        "encode_cli_search_envelope",
        "RunningBinaryIdentityV1",
        "GeneratedCapabilityCatalogV1::load_frozen",
    ):
        if required not in search:
            fail(f"binary-local CLI search adapter is missing {required}")
    projection = (ROOT / "src/operations/adapters/live_projection.rs").read_text()
    if "_ => false" not in projection:
        fail("Projection repository identity does not fail closed on canonicalization failure")


def validate_parity() -> None:
    parity = load_json("tests/fixtures/vnext/stage10/trusted-host-parity.v1.json")
    if parity["schema"] != "maestro.vnext.stage10.trusted-host-parity.v3":
        fail("trusted-host parity schema drifted")
    if parity["protected_runtime_activation"] is not False or parity["ambient_fallback"] is not False:
        fail("trusted-host parity overclaims provider activation")
    for reference in parity["upstream_references"]:
        if digest(ROOT / reference["path"]) != reference["sha256"]:
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
    manifest = load_json("tools/vnext_contracts/stage10/path-manifest.v1.json")
    changed = validate_ownership(manifest)
    validate_v2_resources()
    validate_product_sources()
    validate_parity()
    print(
        json.dumps(
            {
                "schema": "maestro.external.stage12-product-validation.v3",
                "base": manifest["base"],
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
