#!/usr/bin/env python3
"""Validate the exact non-promoting pre-root Stage0ProofManifest."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
MANIFEST = WORKSPACE / "contracts/vnext/stage0/proof-matrix/stage0-proof-manifest.v1.json"
MANIFEST_CBOR = MANIFEST.with_suffix(".cbor")
INPUT_BINDINGS = WORKSPACE / "contracts/vnext/stage0/input-bindings.json"
if __package__:
    from . import build
else:
    sys.path.insert(0, str(TOOLS))
    import build


class ProofValidationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProofValidationError(message)


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ProofValidationError(f"required proof artifact is unavailable: {path}") from error
    require(isinstance(value, dict), f"proof artifact must be an object: {path}")
    return value


def exact_sha256(value: Any, label: str) -> str:
    require(
        isinstance(value, str)
        and len(value) == 64
        and value.isascii()
        and all(character in "0123456789abcdef" for character in value),
        f"{label} must be lowercase SHA-256 hexadecimal",
    )
    return value


def canonical_path(value: Any, label: str) -> str:
    require(isinstance(value, str) and value and value.isascii(), f"{label} must be non-empty ASCII")
    require(not value.startswith("/") and "\\" not in value, f"{label} must be repository-relative")
    require(
        all(part not in ("", ".", "..") for part in value.split("/")),
        f"{label} contains a non-canonical path segment",
    )
    return value


def artifact_location(path: str, bindings: dict[str, Any]) -> Path:
    if path.startswith(".maestro/cards/"):
        return Path(bindings["source_repository_realpath"]) / path
    return WORKSPACE / path


def validate_artifacts(
    rows: Any,
    label: str,
    bindings: dict[str, Any],
    *,
    verify_files: bool,
) -> list[list[Any]]:
    require(isinstance(rows, list), f"{label} must be an array")
    projected: list[list[Any]] = []
    paths: list[str] = []
    for index, row in enumerate(rows):
        row_label = f"{label}[{index}]"
        require(isinstance(row, dict) and set(row) == {"path", "sha256"}, f"{row_label} has extra or missing fields")
        path = canonical_path(row["path"], f"{row_label}.path")
        digest = exact_sha256(row["sha256"], f"{row_label}.sha256")
        paths.append(path)
        projected.append([path, build.Bytes(bytes.fromhex(digest))])
        if verify_files:
            if path.startswith("approved-commitment:.maestro/cards/"):
                source_inputs = bindings["canonical_source_inputs"]
                expected = {
                    "card.yaml": source_inputs["card_sha256"],
                    "decisions.yaml": source_inputs["decisions_sha256"],
                    "design.md": source_inputs["design_sha256"],
                }.get(Path(path).name)
                require(expected is not None, f"unexpected approved source commitment: {path}")
                require(
                    digest == expected,
                    f"proof-bound approved source commitment drifted: {path}",
                )
            else:
                location = artifact_location(path, bindings)
                require(location.is_file(), f"proof-bound artifact is missing: {path}")
                require(build.sha256_file(location) == digest, f"proof-bound artifact drifted: {path}")
    require(paths == sorted(paths) and len(paths) == len(set(paths)), f"{label} paths must be sorted and unique")
    return projected


def validate_counts(rows: Any, label: str) -> list[list[Any]]:
    require(isinstance(rows, list), f"{label} must be an array")
    projected: list[list[Any]] = []
    names: list[str] = []
    for index, row in enumerate(rows):
        row_label = f"{label}[{index}]"
        require(isinstance(row, dict) and set(row) == {"name", "value"}, f"{row_label} has extra or missing fields")
        name = row["name"]
        value = row["value"]
        require(isinstance(name, str) and name and name.isascii(), f"{row_label}.name must be non-empty ASCII")
        require(isinstance(value, int) and not isinstance(value, bool) and value >= 0, f"{row_label}.value must be unsigned")
        names.append(name)
        projected.append([name, value])
    require(names == sorted(names) and len(names) == len(set(names)), f"{label} names must be sorted and unique")
    return projected


def validate_locked_semantics(gates: list[dict[str, Any]]) -> None:
    checkpoint = gates[3]
    require(
        checkpoint.get("assertions")
        == {
            "d0aa": {
                "body_sha256": "85870762931cc790a0dd16e5e4b7c55c56c871fe500106274472d2308fe7d72a",
                "symbol_count": 150,
            },
            "d116": {
                "body_sha256": "593ee2afa0356819033aa2e2d955b2fbf38a2cc2af7e23844a94159085ef37f7",
                "role_count": 7,
                "route_count": 109,
            },
            "d70b": {
                "body_sha256": "2ed739642474a92b110002a224b7f36fa39867244d6368d1904fd78de24e3a80",
                "symbol_count": 147,
            },
            "e346_disposition": "separate_catalog_predecessor_gate",
        },
        "incorporated catalog checkpoint proof is not exact",
    )
    require(gates[2]["name"] == "catalog_predecessor", "e346 predecessor proof is not a separate gate")

    publish = gates[4].get("assertions", {}).get("publish_observation", {})
    require(
        publish.get("current_tag") == 39
        and publish.get("predecessor_tag") == 30
        and isinstance(publish.get("current_descriptor_id"), str)
        and isinstance(publish.get("current_manifest_id"), str),
        "PublishObservation successor/predecessor lineage proof is incomplete",
    )

    migration = gates[13]
    require(
        migration.get("assertions")
        == {
            "pending_obligation_stage": "Stage11",
            "passed_claim": "requirements_frozen_not_runtime_complete",
            "proof_status": "pending_stage0_execution_and_rehearsal",
            "runtime_proof_complete": False,
            "stage": "stage0_candidate_only",
            "stage0_execution_complete": False,
            "stage0_rehearsal_complete": False,
            "status": "requirements_complete_runtime_proof_pending",
        },
        "migration gate must prove frozen requirements without claiming runtime completion",
    )
    migration_counts = {row["name"]: row["value"] for row in migration["semantic_counts"]}
    require(
        migration_counts.get("pending_runtime_proof_count", 0) > 0,
        "migration gate must retain pending Stage11 runtime-proof obligations",
    )

    assembly = gates[14]
    component_tags = build.rust_enum_tags(
        "src/domain/contract/component_kind.rs",
        "ContractComponentKindV1",
    )
    finalization_tags = build.rust_enum_tags(
        "src/domain/contract/finalization.rs",
        "FinalizationInputKindV1",
    )
    assertions = assembly.get("assertions", {})
    counts = {row["name"]: row["value"] for row in assembly["semantic_counts"]}
    require(assertions.get("component_kind_tags") == component_tags, "component kinds are not derived from the Rust contract")
    require(
        assertions.get("finalization_input_kind_tags") == finalization_tags,
        "finalization input kinds are not derived from the Rust contract",
    )
    require(counts.get("component_kind_count") == len(component_tags), "component-kind count is self-asserted")
    require(
        counts.get("finalization_input_kind_count") == len(finalization_tags),
        "finalization-input count is self-asserted",
    )


def add_hash(forbidden: set[str], hexadecimal: str) -> None:
    forbidden.add(hexadecimal)
    forbidden.add(f"sha256:{hexadecimal}")


def forbidden_promotion_values(bindings: dict[str, Any]) -> set[str]:
    forbidden: set[str] = set()

    def canonical_json(value: Any) -> bytes:
        return json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        ).encode("ascii")

    def visit(value: Any) -> None:
        if isinstance(value, dict):
            add_hash(forbidden, hashlib.sha256(canonical_json(value)).hexdigest())
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            add_hash(forbidden, hashlib.sha256(canonical_json(value)).hexdigest())
            for child in value:
                visit(child)
        elif isinstance(value, (str, int)) and not isinstance(value, bool):
            text = str(value)
            forbidden.add(text)
            if text.startswith("sha256:"):
                candidate = text.removeprefix("sha256:")
                if len(candidate) == 64:
                    add_hash(forbidden, candidate)
            elif len(text) == 64 and all(character in "0123456789abcdef" for character in text):
                add_hash(forbidden, text)
            if text.isascii():
                add_hash(forbidden, hashlib.sha256(text.encode("ascii")).hexdigest())

    visit(bindings["external_approval"])
    visit(bindings["external_approval_event"])
    add_hash(forbidden, hashlib.sha256(INPUT_BINDINGS.read_bytes()).hexdigest())
    add_hash(forbidden, hashlib.sha256(canonical_json(bindings)).hexdigest())
    return forbidden


def scan_forbidden(value: Any, forbidden: set[str]) -> None:
    forbidden_keys = {
        "approval_turn_id",
        "approval_turn_started_at",
        "build_plan_handoff",
        "candidate_input_commitment",
        "external_approval",
        "external_approval_event",
        "exact_instruction",
        "packet_sha256",
        "packet_turn_id",
        "packet_turn_completed_at",
        "user_message_id",
    }
    if isinstance(value, dict):
        require(not (forbidden_keys & set(value)), "external approval field leaked into pre-root proof identity")
        for key, child in value.items():
            scan_forbidden(key, forbidden)
            scan_forbidden(child, forbidden)
    elif isinstance(value, list):
        for child in value:
            scan_forbidden(child, forbidden)
    elif isinstance(value, (str, int)) and not isinstance(value, bool):
        require(str(value) not in forbidden, "external approval value leaked into pre-root proof identity")


def validate_document(
    document: dict[str, Any],
    encoded: bytes,
    *,
    verify_files: bool,
) -> dict[str, Any]:
    bindings = load_object(INPUT_BINDINGS)
    require(
        set(document)
        == {
            "candidate_only",
            "canonical_cbor_byte_length",
            "canonical_cbor_sha256",
            "canonical_value",
            "gate_count",
            "gates",
            "identity",
            "runtime_activation",
            "schema",
        },
        "Stage0ProofManifest top-level field set is not exact",
    )
    require(document["schema"] == build.DOMAIN, "Stage0ProofManifest schema drifted")
    require(document["candidate_only"] is True, "Stage0ProofManifest is not candidate-only")
    require(document["runtime_activation"] is False, "Stage0ProofManifest attempted runtime activation")
    gates = document["gates"]
    require(isinstance(gates, list), "Stage0ProofManifest gates must be an array")
    require(document["gate_count"] == len(build.GATE_NAMES), "Stage0ProofManifest gate count is not exact")
    require(len(gates) == len(build.GATE_NAMES), "Stage0ProofManifest omits or adds a gate")
    require([row.get("tag") for row in gates] == list(range(1, len(build.GATE_NAMES) + 1)), "proof gate tags are missing, duplicate, or reordered")
    require([row.get("name") for row in gates] == list(build.GATE_NAMES), "proof gate names are not exact")

    canonical = build.value_from_json(document["canonical_value"])
    require(
        isinstance(canonical, list)
        and len(canonical) == 2
        and canonical[0] == 1
        and isinstance(canonical[1], list)
        and len(canonical[1]) == len(gates),
        "Stage0ProofManifest canonical envelope is malformed",
    )
    require(build.cbor(canonical) == encoded, "Stage0ProofManifest canonical value does not reproduce its bytes")
    require(document["canonical_cbor_sha256"] == hashlib.sha256(encoded).hexdigest(), "Stage0ProofManifest CBOR hash drifted")
    require(document["canonical_cbor_byte_length"] == len(encoded), "Stage0ProofManifest CBOR length drifted")
    require(document["identity"] == build.identity(canonical), "Stage0ProofManifest identity drifted")

    for index, (projection, canonical_gate) in enumerate(zip(gates, canonical[1], strict=True), start=1):
        label = f"gate {index}"
        allowed = {
            "assertions",
            "input_artifacts",
            "name",
            "result",
            "result_class",
            "result_sha256",
            "semantic_counts",
            "source_artifacts",
            "tag",
            "validator_artifacts",
        }
        required = allowed - {"assertions"}
        require(isinstance(projection, dict) and required <= set(projection) <= allowed, f"{label} projection fields are not exact")
        require(projection["result"] == "passed", f"{label} did not pass")
        require(projection["result_class"] == (build.VERIFIED_NON_PROMOTING if index == 1 else "verified"), f"{label} result class drifted")
        require(isinstance(canonical_gate, list) and len(canonical_gate) == 9, f"{label} canonical row is malformed")
        source_rows = validate_artifacts(projection["source_artifacts"], f"{label}.source_artifacts", bindings, verify_files=verify_files)
        validator_rows = validate_artifacts(projection["validator_artifacts"], f"{label}.validator_artifacts", bindings, verify_files=verify_files)
        input_rows = validate_artifacts(projection["input_artifacts"], f"{label}.input_artifacts", bindings, verify_files=verify_files)
        counts = validate_counts(projection["semantic_counts"], f"{label}.semantic_counts")
        require(validator_rows, f"{label} binds no validator source")
        expected_row = [
            index,
            build.GATE_NAMES[index - 1],
            source_rows,
            validator_rows,
            input_rows,
            1,
            projection["result_class"],
            build.Bytes(bytes.fromhex(exact_sha256(projection["result_sha256"], f"{label}.result_sha256"))),
            counts,
        ]
        require(canonical_gate == expected_row, f"{label} projection and canonical row diverged")
        assertions = projection.get("assertions", {})
        if index == 1:
            require(not source_rows and not input_rows and not counts and not assertions, "external verifier inputs attempted identity promotion")
            expected_result = hashlib.sha256(build.VERIFIED_NON_PROMOTING.encode("ascii")).hexdigest()
        else:
            expected_result = build.result_hash(
                projection["name"],
                projection["source_artifacts"],
                projection["validator_artifacts"],
                projection["input_artifacts"],
                [(row["name"], row["value"]) for row in projection["semantic_counts"]],
                assertions,
            )
        require(projection["result_sha256"] == expected_result, f"{label} result hash is not independently reproducible")

    validate_locked_semantics(gates)
    scan_forbidden(document, forbidden_promotion_values(bindings))
    return {"identity": document["identity"], "gate_count": len(gates)}


def resigned(document: dict[str, Any]) -> tuple[dict[str, Any], bytes]:
    canonical = build.value_from_json(document["canonical_value"])
    encoded = build.cbor(canonical)
    document["identity"] = build.identity(canonical)
    document["canonical_cbor_sha256"] = hashlib.sha256(encoded).hexdigest()
    document["canonical_cbor_byte_length"] = len(encoded)
    document["canonical_value"] = build.json_value(canonical)
    return document, encoded


def mutant_rejections(document: dict[str, Any]) -> list[str]:
    rejected: list[str] = []
    for gate_index, gate_name in enumerate(build.GATE_NAMES):
        omitted = copy.deepcopy(document)
        omitted["gates"].pop(gate_index)
        omitted["gate_count"] -= 1
        omitted["canonical_value"][1].pop(gate_index)
        omitted, encoded = resigned(omitted)
        try:
            validate_document(omitted, encoded, verify_files=False)
        except (ProofValidationError, build.ProofError):
            rejected.append(f"omit:{gate_name}")
        else:
            raise ProofValidationError(f"validator accepted omitted proof gate: {gate_name}")

        failed = copy.deepcopy(document)
        failed["gates"][gate_index]["result"] = "failed"
        failed["canonical_value"][1][gate_index][5] = 2
        failed, encoded = resigned(failed)
        try:
            validate_document(failed, encoded, verify_files=False)
        except (ProofValidationError, build.ProofError):
            rejected.append(f"fail:{gate_name}")
        else:
            raise ProofValidationError(f"validator accepted failed proof gate: {gate_name}")
    return rejected


def ruby_equality() -> dict[str, Any]:
    env = build.proof_environment()
    process = subprocess.run(
        ["/usr/bin/ruby", str(TOOLS / "encode.rb")],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    if process.returncode != 0:
        raise ProofValidationError(process.stderr.strip() or "independent Ruby proof encoder failed")
    try:
        result = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise ProofValidationError("independent Ruby proof encoder emitted invalid JSON") from error
    require(result.get("status") == "pass", "independent Ruby proof encoder did not pass")
    return result


def execute() -> None:
    document = load_object(MANIFEST)
    try:
        encoded = MANIFEST_CBOR.read_bytes()
    except OSError as error:
        raise ProofValidationError("Stage0ProofManifest CBOR is unavailable") from error
    summary = validate_document(document, encoded, verify_files=True)
    expected_document, expected_encoded = build.build_manifest(check=True)
    require(document == expected_document and encoded == expected_encoded, "Stage0ProofManifest was not built from final current producer bytes")
    ruby = ruby_equality()
    rejected = mutant_rejections(document)
    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-proof-validation-receipt.v1",
                "status": "pass",
                **summary,
                "ruby_encoder": ruby["encoder"],
                "mutants_rejected": len(rejected),
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--mutants", action="store_true")
    try:
        parser.parse_args()
        execute()
    except (KeyError, OSError, ProofValidationError, TypeError, ValueError, build.ProofError) as error:
        print(json.dumps({"status": "blocked", "reason": str(error)}, sort_keys=True))
        raise SystemExit(2) from error
