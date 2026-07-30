#!/usr/bin/env python3
"""Build the exact non-promoting pre-root Stage0ProofManifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


WORKSPACE = Path(__file__).resolve().parents[4]
OUTPUT = WORKSPACE / "contracts/vnext/stage0/proof-matrix"
MANIFEST = OUTPUT / "stage0-proof-manifest.v1.json"
MANIFEST_CBOR = MANIFEST.with_suffix(".cbor")
DOMAIN = "maestro.vnext.stage0-proof-manifest.v1"
VERIFIED_NON_PROMOTING = "verified_non_promoting"
GATE_NAMES = (
    "external_input_authorization",
    "decision_closure",
    "catalog_predecessor",
    "incorporated_catalog_checkpoints",
    "catalog_successor",
    "public_contracts",
    "public_identity",
    "submission_claim",
    "dispatch",
    "effect_home",
    "resource_release",
    "current_surface_consumer_census",
    "persistence_archive_golden_fixtures",
    "migration_rollback",
    "root_assembly_source_binding",
)

DECISION = "contracts/vnext/stage0/decision-closure/decision-closure.v1.json"
CATALOG_INVENTORY = "contracts/vnext/catalogs/generated/inventory.json"
RESOURCE_RELEASE = "contracts/vnext/stage0/resource-release/resource-release.v1.json"
RESOURCE_DELTA = "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json"


class ProofError(RuntimeError):
    pass


@dataclass(frozen=True)
class Bytes:
    value: bytes


def load(path: str | Path) -> dict[str, Any]:
    location = WORKSPACE / path if isinstance(path, str) else path
    try:
        value = json.loads(location.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ProofError(f"required proof input is unavailable: {location}") from error
    if not isinstance(value, dict):
        raise ProofError(f"proof input must be an object: {location}")
    return value


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as error:
        raise ProofError(f"required proof artifact is unavailable: {path}") from error


def json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n"
    ).encode("ascii")


def json_value(value: Any) -> Any:
    if isinstance(value, Bytes):
        return {"bytes": value.value.hex()}
    if isinstance(value, list):
        return [json_value(item) for item in value]
    return value


def value_from_json(value: Any) -> Any:
    if isinstance(value, dict) and set(value) == {"bytes"}:
        try:
            raw = bytes.fromhex(value["bytes"])
        except (TypeError, ValueError) as error:
            raise ProofError("invalid proof byte wrapper") from error
        return Bytes(raw)
    if isinstance(value, list):
        return [value_from_json(item) for item in value]
    return value


def cbor(value: Any) -> bytes:
    if isinstance(value, Bytes):
        return cbor_head(2, len(value.value)) + value.value
    if isinstance(value, str):
        try:
            raw = value.encode("ascii")
        except UnicodeEncodeError as error:
            raise ProofError("proof canonical text must be ASCII") from error
        return cbor_head(3, len(raw)) + raw
    if isinstance(value, int) and not isinstance(value, bool) and value >= 0:
        return cbor_head(0, value)
    if isinstance(value, list):
        return cbor_head(4, len(value)) + b"".join(cbor(item) for item in value)
    raise ProofError(f"unsupported proof canonical value: {value!r}")


def cbor_head(major: int, value: int) -> bytes:
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    if value <= 0xFFFFFFFFFFFFFFFF:
        return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")
    raise ProofError("proof canonical integer exceeds u64")


def identity(canonical: Any) -> str:
    return f"sha256:{sha256_bytes(cbor([DOMAIN, canonical]))}"


def artifact(path: str, physical: Path | None = None) -> dict[str, str]:
    if (
        not path
        or path.startswith("/")
        or "\\" in path
        or any(part in ("", ".", "..") for part in path.split("/"))
        or not path.isascii()
    ):
        raise ProofError(f"non-canonical proof artifact path: {path}")
    location = physical or WORKSPACE / path
    return {"path": path, "sha256": sha256_file(location)}


def artifacts(paths: list[str], external: dict[str, Path] | None = None) -> list[dict[str, str]]:
    external = external or {}
    rows = sorted((artifact(path, external.get(path)) for path in paths), key=lambda row: row["path"])
    if len({row["path"] for row in rows}) != len(rows):
        raise ProofError("proof artifact paths must be unique")
    return rows


def run(command: list[str], env: dict[str, str] | None = None) -> None:
    process = subprocess.run(
        command,
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
        env=env or proof_environment(),
    )
    if process.returncode != 0:
        detail = process.stderr.strip() or process.stdout.strip() or "no diagnostic"
        raise ProofError(f"proof validator failed ({' '.join(command)}): {detail}")


def proof_environment(extra: dict[str, str] | None = None) -> dict[str, str]:
    env = {
        "HOME": tempfile.gettempdir(),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONNOUSERSITE": "1",
        "RUBYOPT": "",
        "RUBYLIB": "",
    }
    if extra:
        env.update(extra)
    return env


def result_hash(
    name: str,
    source_rows: list[dict[str, str]],
    validator_rows: list[dict[str, str]],
    input_rows: list[dict[str, str]],
    semantic_counts: list[tuple[str, int]],
    assertions: dict[str, Any],
) -> str:
    return sha256_bytes(
        json_bytes(
            {
                "assertions": assertions,
                "gate": name,
                "input_artifacts": input_rows,
                "result": "passed",
                "semantic_counts": [
                    {"name": count_name, "value": count}
                    for count_name, count in semantic_counts
                ],
                "source_artifacts": source_rows,
                "validator_artifacts": validator_rows,
            }
        )
    )


def gate(
    tag: int,
    name: str,
    *,
    source_paths: list[str],
    validator_paths: list[str],
    input_paths: list[str],
    semantic_counts: dict[str, int],
    assertions: dict[str, Any],
    commands: list[list[str]] | None = None,
    command_env: dict[str, str] | None = None,
    external_paths: dict[str, Path] | None = None,
) -> tuple[dict[str, Any], list[Any]]:
    if name != GATE_NAMES[tag - 1]:
        raise ProofError("proof gate tag/name mismatch")
    for command in commands or []:
        run(command, command_env)
    source_rows = artifacts(source_paths, external_paths)
    validator_rows = artifacts(validator_paths)
    input_rows = artifacts(input_paths, external_paths)
    if not validator_rows:
        raise ProofError(f"proof gate has no validator source: {name}")
    counts = sorted(semantic_counts.items())
    result_class = VERIFIED_NON_PROMOTING if tag == 1 else "verified"
    if tag == 1:
        if source_rows or input_rows or counts or assertions:
            raise ProofError("external authorization gate attempted to promote verifier inputs")
        result_sha256 = sha256_bytes(VERIFIED_NON_PROMOTING.encode("ascii"))
    else:
        result_sha256 = result_hash(
            name,
            source_rows,
            validator_rows,
            input_rows,
            counts,
            assertions,
        )
    canonical = [
        tag,
        name,
        [[row["path"], Bytes(bytes.fromhex(row["sha256"]))] for row in source_rows],
        [[row["path"], Bytes(bytes.fromhex(row["sha256"]))] for row in validator_rows],
        [[row["path"], Bytes(bytes.fromhex(row["sha256"]))] for row in input_rows],
        1,
        result_class,
        Bytes(bytes.fromhex(result_sha256)),
        [[count_name, count] for count_name, count in counts],
    ]
    document = {
        "tag": tag,
        "name": name,
        "source_artifacts": source_rows,
        "validator_artifacts": validator_rows,
        "input_artifacts": input_rows,
        "result": "passed",
        "result_class": result_class,
        "result_sha256": result_sha256,
        "semantic_counts": [
            {"name": count_name, "value": count} for count_name, count in counts
        ],
    }
    if assertions:
        document["assertions"] = assertions
    return document, canonical


def rust_enum_tags(path: str, enum_name: str) -> list[int]:
    try:
        source = (WORKSPACE / path).read_text(encoding="utf-8")
    except OSError as error:
        raise ProofError(f"canonical Rust contract is unavailable: {path}") from error
    matched = re.search(
        rf"pub enum {re.escape(enum_name)}\s*\{{(?P<body>.*?)\n\}}",
        source,
        re.DOTALL,
    )
    if matched is None:
        raise ProofError(f"canonical Rust enum is unavailable: {enum_name}")
    tags = [int(value) for value in re.findall(r"^\s*[A-Za-z][A-Za-z0-9_]*\s*=\s*(\d+),\s*$", matched["body"], re.MULTILINE)]
    if not tags or tags != list(range(1, len(tags) + 1)):
        raise ProofError(f"canonical Rust enum tags are not exact and contiguous: {enum_name}")
    return tags


def checkpoint_assertions() -> tuple[dict[str, Any], dict[str, int]]:
    decision = load(DECISION)
    records = {row["id"]: row for row in decision.get("records", [])}
    required = {
        "dec-canonical-typed-recoverreserved-d116": "593ee2afa0356819033aa2e2d955b2fbf38a2cc2af7e23844a94159085ef37f7",
        "dec-canonical-split-plane-catalog-owner-d70b": "2ed739642474a92b110002a224b7f36fa39867244d6368d1904fd78de24e3a80",
        "dec-correct-split-plane-catalog-owner-d0aa": "85870762931cc790a0dd16e5e4b7c55c56c871fe500106274472d2308fe7d72a",
    }
    for decision_id, body_sha256 in required.items():
        if records.get(decision_id, {}).get("raw_body_sha256") != body_sha256:
            raise ProofError(f"incorporated Decision body drifted: {decision_id}")

    bindings = load("contracts/vnext/stage0/input-bindings.json")
    source_root = Path(bindings["source_repository_realpath"])
    card_root = source_root / ".maestro/cards" / bindings["feature_id"]
    decisions_text = (card_root / "decisions.yaml").read_text(encoding="utf-8")
    required_phrases = (
        "RouteRoleV1 = ActionReserve | ActionRecoverReserved | ActionOutcome | ActionReconcile | CeremonyInitiate | CeremonyRecoverReserved | CeremonyResolveResult",
        "19 Action branches x 4 + 11 Ceremony branches x 3 = 109 routes",
        "147-symbol proof",
        "150 total pre-manifest symbols",
    )
    if any(phrase not in decisions_text for phrase in required_phrases):
        raise ProofError("incorporated catalog checkpoint semantics are absent from source Decision bytes")
    assertions = {
        "d0aa": {"body_sha256": required["dec-correct-split-plane-catalog-owner-d0aa"], "symbol_count": 150},
        "d116": {"body_sha256": required["dec-canonical-typed-recoverreserved-d116"], "route_count": 109, "role_count": 7},
        "d70b": {"body_sha256": required["dec-canonical-split-plane-catalog-owner-d70b"], "symbol_count": 147},
        "e346_disposition": "separate_catalog_predecessor_gate",
    }
    counts = {
        "d0aa_symbol_count": 150,
        "d116_role_count": 7,
        "d116_route_count": 109,
        "d70b_symbol_count": 147,
        "incorporated_checkpoint_count": 3,
    }
    return assertions, counts


def catalog_successor_assertions() -> tuple[dict[str, Any], dict[str, int]]:
    inventory = load(CATALOG_INVENTORY)
    grammar = load("contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json")
    action = load("contracts/vnext/catalogs/generated/catalog-06-action-leaf.json")
    publish_rows = [row for row in action["descriptors"] if row["value"][1] == "PublishObservation"]
    if len(publish_rows) != 1 or publish_rows[0]["value"][0] != 39:
        raise ProofError("efa0 PublishObservation successor descriptor is not exact tag 39")
    semantic = inventory["semantic_counts"]
    if semantic.get("grammar_symbols") != 156:
        raise ProofError("efa0 current grammar is not the exact 156-symbol successor")
    assertions = {
        "effective_decision": inventory["effective_decision"],
        "grammar_id": inventory["grammar_id"],
        "publish_observation": {
            "current_descriptor_id": publish_rows[0]["descriptor_id"],
            "current_manifest_id": action["manifest_id"],
            "current_tag": 39,
            "predecessor_tag": 30,
        },
    }
    counts = {
        "action_count": semantic["actions"],
        "catalog_manifest_count": len(inventory["artifacts"]) - 1,
        "grammar_symbol_count": semantic["grammar_symbols"],
        "owner_relation_row_count": semantic["owner_relation_rows"],
        "route_count": semantic["effect_routes"],
        "route_role_count": semantic["route_roles"],
    }
    if (
        grammar.get("status") != "stage0_candidate_not_published"
        or grammar.get("publication_state") != "inactive_candidate"
    ):
        raise ProofError("efa0 grammar is not a generated candidate")
    return assertions, counts


def source_card_paths() -> tuple[dict[str, Path], list[str]]:
    bindings = load("contracts/vnext/stage0/input-bindings.json")
    card_root = (
        Path(bindings["source_repository_realpath"])
        / ".maestro/cards"
        / bindings["feature_id"]
    )
    logical = [
        f".maestro/cards/{bindings['feature_id']}/card.yaml",
        f".maestro/cards/{bindings['feature_id']}/decisions.yaml",
        f".maestro/cards/{bindings['feature_id']}/design.md",
    ]
    return {path: card_root / Path(path).name for path in logical}, logical


def build_manifest(check: bool = False) -> tuple[dict[str, Any], bytes]:
    python = sys.executable
    external_validator = "tools/vnext_contracts/stage0/verify_input_bindings.py"
    run([python, external_validator], proof_environment())
    bindings = load("contracts/vnext/stage0/input-bindings.json")
    authoritative_env = proof_environment(
        {"MAESTRO_AUTHORITATIVE_SOURCE": bindings["source_repository_realpath"]}
    )
    receipt_check = ["--check"] if check else []

    decision = load(DECISION)
    records = decision["records"]
    decision_counts = {
        "decision_count": len(records),
        "locked_count": sum(row["terminal_status"] == "locked" for row in records),
        "material_count": sum(row["consequence_classification"] == "material" for row in records),
        "materialization_count": len(decision["materializations"]),
        "rationale_only_count": sum(row["consequence_classification"] == "rationale_only" for row in records),
        "superseded_count": sum(row["terminal_status"] == "superseded" for row in records),
    }
    checkpoint_claims, checkpoint_counts = checkpoint_assertions()
    catalog_claims, catalog_counts = catalog_successor_assertions()
    source_external, source_logical = source_card_paths()

    public = load("contracts/vnext/public/public_contracts.v1.json")
    public_identity = load("contracts/vnext/stage0/public-identity/public-identity-closure.v1.json")
    submission = load("contracts/vnext/stage0/submission-claim/submission-claim-set.v1.json")
    dispatch = load("contracts/vnext/stage0/dispatch-cutover/validation-receipt.v1.json")
    effect_inventory = load("contracts/vnext/stage0/effect-home/inventory.json")
    resource = load(RESOURCE_RELEASE)

    predecessor = load("contracts/vnext/catalogs/predecessor/e346/reproduction.json")
    predecessor_counts = {
        "catalog_count": len(predecessor["catalog_suite"]["catalogs"]),
        "grammar_action_symbol_count": predecessor["grammar"]["action_symbol_count"],
        "grammar_owner_symbol_count": predecessor["grammar"]["owner_symbol_count"],
        "grammar_route_count": predecessor["grammar"]["route_count"],
    }

    common_resource_validators = [
        "tools/vnext_contracts/stage0/resource_release/build.py",
        "tools/vnext_contracts/stage0/resource_release/validate.py",
        "tools/vnext_contracts/stage0/resource_release/verify.rb",
    ]
    resource_input_paths = [
        CATALOG_INVENTORY,
        "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
        "contracts/vnext/stage0/public-identity/public-identity-closure.v1.json",
    ]

    gate_specs: list[tuple[dict[str, Any], list[Any]]] = []
    gate_specs.append(
        gate(
            1,
            GATE_NAMES[0],
            source_paths=[],
            validator_paths=[external_validator],
            input_paths=[],
            semantic_counts={},
            assertions={},
        )
    )
    gate_specs.append(
        gate(
            2,
            GATE_NAMES[1],
            source_paths=[
                DECISION,
                "contracts/vnext/stage0/decision-closure/decision-closure.v1.cbor",
            ],
            validator_paths=[
                "tools/vnext_contracts/stage0/decision_closure/validate.py",
                "tools/vnext_contracts/stage0/decision_closure/validate.rb",
                "tools/vnext_contracts/stage0/decision_closure/verify.py",
            ],
            input_paths=[
                "contracts/vnext/stage0/decision-closure/external-design-authority-closure.v1.json",
                "contracts/vnext/stage0/decision-closure/root-binding-requirements.v1.json",
            ],
            semantic_counts=decision_counts,
            assertions={"decision_closure_id": decision["identity"]},
            commands=[[python, "tools/vnext_contracts/stage0/decision_closure/verify.py", *receipt_check]],
        )
    )
    predecessor_validators = [
        "tools/vnext_contracts/catalogs/verify_predecessors.py",
        *[
            f"tools/vnext_contracts/catalogs/predecessor_e346/{name}"
            for name in (
                "vnext_catalog_profile_grammar_build.py",
                "vnext_catalog_profile_grammar_validate.py",
                "vnext_catalog_suite_build.py",
                "vnext_catalog_suite_validate.py",
                "vnext_manifest_encode_py.py",
                "vnext_manifest_encode_rb.rb",
            )
        ],
    ]
    gate_specs.append(
        gate(
            3,
            GATE_NAMES[2],
            source_paths=["contracts/vnext/catalogs/predecessor/e346/reproduction.json"],
            validator_paths=predecessor_validators,
            input_paths=[
                "contracts/vnext/catalogs/evidence/e346-nominal-source.json",
                "contracts/vnext/catalogs/evidence/e346-reproduction-sources.json",
            ],
            semantic_counts=predecessor_counts,
            assertions={
                "e346_body_sha256": "8c920b7cde0fc96daf12275d9a1aa0db48158c84fd86ea1c306f5dc8ad601545",
                "grammar_id": predecessor["grammar"]["grammar_id"],
                "status": predecessor["status"],
            },
            commands=[[python, "tools/vnext_contracts/catalogs/verify_predecessors.py"]],
        )
    )
    gate_specs.append(
        gate(
            4,
            GATE_NAMES[3],
            source_paths=[DECISION],
            validator_paths=[
                "tools/vnext_contracts/stage0/decision_closure/validate.py",
                "tools/vnext_contracts/stage0/proof_matrix/validate.py",
            ],
            input_paths=source_logical[1:2],
            semantic_counts=checkpoint_counts,
            assertions=checkpoint_claims,
            external_paths=source_external,
        )
    )

    generated_catalog_paths = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for path in (WORKSPACE / "contracts/vnext/catalogs/generated").glob("*.json")
    )
    gate_specs.append(
        gate(
            5,
            GATE_NAMES[4],
            source_paths=generated_catalog_paths,
            validator_paths=[
                "tools/vnext_contracts/catalogs/build.py",
                "tools/vnext_contracts/catalogs/validate.py",
                "tools/vnext_contracts/catalogs/cbor_py.py",
                "tools/vnext_contracts/catalogs/cbor_rb.rb",
            ],
            input_paths=[DECISION, "contracts/vnext/catalogs/evidence/predecessors.json"],
            semantic_counts=catalog_counts,
            assertions=catalog_claims,
            commands=[
                [python, "tools/vnext_contracts/catalogs/build.py", "--check"],
                [python, "tools/vnext_contracts/catalogs/validate.py", "--generated", "contracts/vnext/catalogs/generated", "--mutants", "--json"],
            ],
        )
    )

    public_files = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for path in (WORKSPACE / "contracts/vnext/public").glob("*.json")
    )
    gate_specs.append(
        gate(
            6,
            GATE_NAMES[5],
            source_paths=["contracts/vnext/public/public_contracts.v1.json"],
            validator_paths=[
                "tools/vnext_contracts/public/validate_public_contracts.py",
                "tools/vnext_contracts/public/validate_census.py",
            ],
            input_paths=[path for path in public_files if not path.endswith("public_contracts.v1.json")],
            semantic_counts={
                "schema_definition_count": public["schema_definition_count"],
                "semantic_artifact_count": len(public["semantic_artifacts"]),
            },
            assertions={"runtime_activation": public["runtime_activation"]},
            commands=[
                [python, "tools/vnext_contracts/public/validate_public_contracts.py", "--mutant-suite"],
                [python, "tools/vnext_contracts/public/validate_census.py", "--mutant-suite"],
            ],
        )
    )

    public_identity_paths = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for path in (WORKSPACE / "contracts/vnext/stage0/public-identity").glob("*")
        if path.is_file()
    )
    gate_specs.append(
        gate(
            7,
            GATE_NAMES[6],
            source_paths=public_identity_paths,
            validator_paths=[
                "tools/vnext_contracts/stage0/public_identity/build.py",
                "tools/vnext_contracts/stage0/public_identity/validate.py",
                "tools/vnext_contracts/stage0/public_identity/verify.py",
                "tools/vnext_contracts/stage0/public_identity/encode.rb",
            ],
            input_paths=["contracts/vnext/public/public_contracts.v1.json"],
            semantic_counts={
                "schema_descriptor_count": len(public_identity["schema_descriptors"]),
                "semantic_snapshot_field_count": len(public_identity["semantic_snapshot"]),
            },
            assertions={
                "closure_id": public_identity["closure_id"],
                "manifest_id": public_identity["manifest"]["manifest_id"],
            },
            commands=[[python, "tools/vnext_contracts/stage0/public_identity/verify.py", *receipt_check]],
            command_env=authoritative_env,
        )
    )
    gate_specs.append(
        gate(
            8,
            GATE_NAMES[7],
            source_paths=[
                "contracts/vnext/stage0/submission-claim/submission-claim-set.v1.json",
                "contracts/vnext/stage0/submission-claim/encoder-receipt.v1.json",
            ],
            validator_paths=[
                "tools/vnext_contracts/stage0/submission_claim/build.py",
                "tools/vnext_contracts/stage0/submission_claim/verify.rb",
            ],
            input_paths=[],
            semantic_counts={
                "semantic_mutant_count": len(submission["semantic_mutants_rejected"]),
                "vector_count": len(submission["vectors"]),
            },
            assertions={"schema_id": submission["schema_id"]},
            commands=[["/usr/bin/ruby", "tools/vnext_contracts/stage0/submission_claim/verify.rb"]],
        )
    )
    dispatch_paths = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for path in (WORKSPACE / "contracts/vnext/stage0/dispatch-cutover").glob("*")
        if path.is_file()
    )
    gate_specs.append(
        gate(
            9,
            GATE_NAMES[8],
            source_paths=dispatch_paths,
            validator_paths=[
                "tools/vnext_contracts/stage0/dispatch_cutover/build.py",
                "tools/vnext_contracts/stage0/dispatch_cutover/validate.py",
                "tools/vnext_contracts/stage0/dispatch_cutover/verify.rb",
            ],
            input_paths=[DECISION],
            semantic_counts={
                "blocked_dependency_count": dispatch["blocked_dependencies"],
                "semantic_mutant_count": len(dispatch["mutants"]["cases"]),
            },
            assertions={"runtime_activated": dispatch["runtime_activated"]},
            commands=[[
                python,
                "tools/vnext_contracts/stage0/dispatch_cutover/validate.py",
                "--mutant-suite",
                *receipt_check,
            ]],
        )
    )
    effect_paths = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for path in (WORKSPACE / "contracts/vnext/stage0/effect-home").glob("*.json")
    )
    gate_specs.append(
        gate(
            10,
            GATE_NAMES[9],
            source_paths=effect_paths,
            validator_paths=[
                "tools/vnext_contracts/stage0/effect_home/build.py",
                "tools/vnext_contracts/stage0/effect_home/validate.py",
                "tools/vnext_contracts/stage0/effect_home/encode.rb",
            ],
            input_paths=[CATALOG_INVENTORY],
            semantic_counts={key: value for key, value in effect_inventory["counts"].items() if isinstance(value, int)},
            assertions={"publication_state": effect_inventory["publication_state"]},
            commands=[[python, "tools/vnext_contracts/stage0/effect_home/validate.py", "--mutants"]],
        )
    )

    resource_paths = [
        "contracts/vnext/stage0/resource-release/resource-descriptors.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-001-migration.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-002-external-pattern-neutral.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-003-external-pattern-vendor.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-004-shared-contract.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-005-orchestration.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-006-capability.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-007-adapter.v1.json",
        "contracts/vnext/stage0/resource-release/bundle-008-agent-bootstrap.v1.json",
        "contracts/vnext/stage0/resource-release/release-resource-census.v1.json",
        "contracts/vnext/stage0/resource-release/embedded-release-bundle.v1.json",
        RESOURCE_DELTA,
        RESOURCE_RELEASE,
    ]
    release_census = resource["release_resource_census"]
    embedded_release = resource["embedded_release_bundle"]
    gate_specs.append(
        gate(
            11,
            GATE_NAMES[10],
            source_paths=resource_paths,
            validator_paths=common_resource_validators,
            input_paths=resource_input_paths,
            semantic_counts={
                "bundle_count": len(resource["bundles"]),
                "consumer_edge_count": len(release_census["consumer_edges"]),
                "downstream_obligation_count": len(resource["downstream_delta_obligations"]),
                "resource_count": len(resource["resources"]),
            },
            assertions={
                "expected_delta_commitment_id": resource["resolved_expected_delta_commitment_id"],
                "release_id": embedded_release["release_id"],
                "resource_release_commitment_id": resource["identity"],
            },
            commands=[
                [python, "tools/vnext_contracts/stage0/resource_release/build.py", "--check"],
                [python, "tools/vnext_contracts/stage0/resource_release/validate.py", "--mutants"],
                [python, "tools/vnext_contracts/stage0/resource_release/validate.py", "--gate", "resource_release_and_delta"],
            ],
        )
    )
    surface_paths = [
        "contracts/vnext/stage0/resource-release/current-surface-manifest.v1.json",
        "contracts/vnext/stage0/resource-release/current-consumer-census.v1.json",
    ]
    gate_specs.append(
        gate(
            12,
            GATE_NAMES[11],
            source_paths=surface_paths,
            validator_paths=common_resource_validators,
            input_paths=[RESOURCE_RELEASE],
            semantic_counts={
                "direct_reader_edge_count": resource["current_surface_manifest"]["direct_reader_edge_count"],
                "resource_count": resource["current_surface_manifest"]["resource_count"],
            },
            assertions={
                "exact_one_reader_evidence_per_resource": resource["current_consumer_census"][
                    "exact_one_reader_evidence_per_resource"
                ]
            },
            commands=[[python, "tools/vnext_contracts/stage0/resource_release/validate.py", "--gate", "current_surface_consumer_census"]],
        )
    )
    schema_surface_paths = [
        "contracts/vnext/stage0/resource-release/current-persistence-manifest.v1.json",
        "contracts/vnext/stage0/resource-release/current-archive-manifest.v1.json",
        "contracts/vnext/stage0/resource-release/golden-fixture-manifest.v1.json",
    ]
    schema_surface_counts = {
        "archive_schema_count": load(schema_surface_paths[1]).get("exact_count", 0),
        "golden_fixture_count": load(schema_surface_paths[2]).get("exact_count", 0),
        "persistence_schema_count": load(schema_surface_paths[0]).get("exact_count", 0),
    }
    gate_specs.append(
        gate(
            13,
            GATE_NAMES[12],
            source_paths=schema_surface_paths,
            validator_paths=common_resource_validators,
            input_paths=[RESOURCE_RELEASE],
            semantic_counts=schema_surface_counts,
            assertions={"exact_set_equality": True},
            commands=[[python, "tools/vnext_contracts/stage0/resource_release/validate.py", "--gate", "persistence_archive_fixtures"]],
        )
    )
    migration_path = "contracts/vnext/stage0/resource-release/migration-rollback-requirements.v1.json"
    migration = load(migration_path)
    migration_boundary = {
        "proof_status": "pending_stage0_execution_and_rehearsal",
        "runtime_proof_complete": False,
        "stage": "stage0_candidate_only",
        "stage0_execution_complete": False,
        "stage0_rehearsal_complete": False,
        "status": "requirements_complete_runtime_proof_pending",
    }
    if any(migration.get(key) != value for key, value in migration_boundary.items()):
        raise ProofError("migration gate attempted to claim runtime completion")
    if migration.get("pending_runtime_proof_count", 0) <= 0:
        raise ProofError("migration gate has no pending Stage11 runtime-proof obligations")
    gate_specs.append(
        gate(
            14,
            GATE_NAMES[13],
            source_paths=[migration_path],
            validator_paths=common_resource_validators,
            input_paths=[RESOURCE_RELEASE, DECISION, CATALOG_INVENTORY],
            semantic_counts={
                "pending_runtime_proof_count": migration["pending_runtime_proof_count"],
                "requirement_row_count": len(migration["requirements"]),
            },
            assertions={
                **migration_boundary,
                "passed_claim": "requirements_frozen_not_runtime_complete",
                "pending_obligation_stage": "Stage11",
            },
            commands=[[python, "tools/vnext_contracts/stage0/resource_release/validate.py", "--gate", "migration_rollback_removal"]],
        )
    )
    component_kind_tags = rust_enum_tags(
        "src/domain/vnext/contract/component_kind.rs",
        "ContractComponentKindV1",
    )
    finalization_input_kind_tags = rust_enum_tags(
        "src/domain/vnext/contract/finalization.rs",
        "FinalizationInputKindV1",
    )
    gate_specs.append(
        gate(
            15,
            GATE_NAMES[14],
            source_paths=source_logical,
            validator_paths=[
                "tools/vnext_contracts/stage0/candidate_root/build.py",
                "tools/vnext_contracts/stage0/candidate_root/validate.py",
                "tools/vnext_contracts/stage0/candidate_root/encode.rb",
                "tools/vnext_contracts/stage0/candidate_root/test_build.py",
            ],
            input_paths=[
                "src/domain/vnext/contract/assembly.rs",
                "src/domain/vnext/contract/component_kind.rs",
                "src/domain/vnext/contract/finalization.rs",
                "src/domain/vnext/contract/proof.rs",
                "src/domain/vnext/identity/digest.rs",
            ],
            semantic_counts={
                "component_kind_count": len(component_kind_tags),
                "finalization_input_kind_count": len(finalization_input_kind_tags),
                "proof_gate_count": len(GATE_NAMES),
            },
            assertions={
                "approval_derivation": "forbidden",
                "component_kind_source": "ContractComponentKindV1",
                "component_kind_tags": component_kind_tags,
                "finalization_input_kind_source": "FinalizationInputKindV1",
                "finalization_input_kind_tags": finalization_input_kind_tags,
                "materialization_count_source": "decision_closure",
                "public_descriptor_count_source": "public_identity_closure",
            },
            commands=[[python, "-m", "unittest", "tools.vnext_contracts.stage0.candidate_root.test_build"]],
            external_paths=source_external,
        )
    )

    documents = [item[0] for item in gate_specs]
    canonical_gates = [item[1] for item in gate_specs]
    canonical = [1, canonical_gates]
    encoded = cbor(canonical)
    document = {
        "schema": DOMAIN,
        "candidate_only": True,
        "runtime_activation": False,
        "identity": identity(canonical),
        "gate_count": len(documents),
        "gates": documents,
        "canonical_value": json_value(canonical),
        "canonical_cbor_sha256": sha256_bytes(encoded),
        "canonical_cbor_byte_length": len(encoded),
    }
    return document, encoded


def execute(check: bool) -> None:
    try:
        document, encoded = build_manifest(check=check)
    except (KeyError, IndexError, OSError, ProofError, TypeError, ValueError) as error:
        print(json.dumps({"status": "blocked", "reason": str(error)}, sort_keys=True))
        raise SystemExit(2) from error
    expected_json = json_bytes(document)
    if check:
        if not MANIFEST.is_file() or MANIFEST.read_bytes() != expected_json:
            raise SystemExit("Stage0ProofManifest JSON drifted or is missing")
        if not MANIFEST_CBOR.is_file() or MANIFEST_CBOR.read_bytes() != encoded:
            raise SystemExit("Stage0ProofManifest CBOR drifted or is missing")
    else:
        OUTPUT.mkdir(parents=True, exist_ok=True)
        json_staging = MANIFEST.with_name(f"{MANIFEST.name}.tmp")
        cbor_staging = MANIFEST_CBOR.with_name(f"{MANIFEST_CBOR.name}.tmp")
        json_staging.write_bytes(expected_json)
        cbor_staging.write_bytes(encoded)
        json_staging.replace(MANIFEST)
        cbor_staging.replace(MANIFEST_CBOR)
    print(
        json.dumps(
            {
                "status": "checked" if check else "built",
                "identity": document["identity"],
                "gate_count": document["gate_count"],
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    execute(parser.parse_args().check)
