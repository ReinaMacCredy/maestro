#!/usr/bin/env python3
"""Validate non-promoting historical census evidence and Stage-11 recensus gates."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
from pathlib import Path


SESSION_SHA256 = "607854b051f0860377827fe190328b7f11a1d6f5bf902c191c8c4f79d861db52"
EXEC_COMMAND_SHA256 = "1d7c331f5010c6565098e8a43fa7e29c12cbd103ec95260d8002a5dd71679810"
RUBY_BODY_SHA256 = "c016209541b43280719c877491070395572b2dc2cc49aa1e272166280467e46d"
OUTPUT_SHA256 = "5b27eb5d880e9b8ab313672676fc25b5b39d95b1bd476b10b626ed13b5155341"
PHYSICAL_CALL_ID = "call_A0D1D7nKo4p9dOyEXjGYyWgN"


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def inactive_source(value: dict) -> bool:
    return (
        value.get("candidate_only") is True
        and value.get("runtime_activation") is False
        and value.get("runtime_registration") is False
    )


def manifest_digest(rows: list[dict]) -> str:
    stream = "".join(
        f"{row['sha256']}  {row['path']}\n"
        for row in sorted(rows, key=lambda row: row["path"])
    )
    return sha256(stream.encode("utf-8"))


def live_mismatches(repo: Path, rows: list[dict]) -> list[dict]:
    mismatches = []
    for row in rows:
        path = repo / row["path"]
        if not path.is_file():
            mismatches.append({"path": row["path"], "expected": row["sha256"], "actual": "missing"})
            continue
        actual = sha256(path.read_bytes())
        if actual != row["sha256"]:
            mismatches.append({"path": row["path"], "expected": row["sha256"], "actual": actual})
    return mismatches


def parse_pairs(line: str, prefix: str) -> dict[str, str]:
    if not line.startswith(prefix):
        raise ValueError(f"missing {prefix.strip()} line")
    return dict(item.split("=", 1) for item in line[len(prefix):].split())


def parse_historical_output(value: bytes) -> dict:
    text = value.decode("utf-8")
    wrapper = "Script completed\nWall time 8.2 seconds\nOutput:\n"
    if not text.startswith(wrapper):
        raise ValueError("historical tool-output wrapper mismatch")
    lines = text[len(wrapper):].splitlines()
    if len(lines) != 6:
        raise ValueError("historical tool-output line count mismatch")
    counts = parse_pairs(lines[0], "COUNTS ")
    types = parse_pairs(lines[1], "TYPES ")
    if not lines[2].startswith("LOCATOR "):
        raise ValueError("historical locator line missing")
    pass1 = parse_pairs(lines[3], "PASS1 ")
    pass2 = parse_pairs(lines[4], "PASS2 ")
    stable_match = re.fullmatch(r"STABLE (true|false) changed=(\d+)", lines[5])
    if stable_match is None:
        raise ValueError("historical stability line mismatch")
    return {
        "category_counts": {key: int(counts[key]) for key in ["legacy", "c115", "repo", "cache", "binary", "perroot", "user"]},
        "node_count": int(counts["total"]),
        "regular_file_count": int(types["files"]),
        "symlink_count": int(types["symlinks"]),
        "locator_digest": lines[2].split(" ", 1)[1],
        "identity_digest_pass1": pass1["identity"],
        "identity_digest_pass2": pass2["identity"],
        "payload_bytes_pass1": int(pass1["bytes"]),
        "payload_bytes_pass2": int(pass2["bytes"]),
        "stable": stable_match.group(1) == "true",
        "changed_rows": int(stable_match.group(2)),
    }


def recovered_body_digest(procedure_bytes: bytes) -> str:
    marker = b'require "set"\n\n'
    if marker not in procedure_bytes:
        return "missing-marker"
    body = procedure_bytes.split(marker, 1)[1].rstrip(b"\n")
    return sha256(body)


def historical_evidence_valid(
    physical: dict,
    historical_output: bytes,
    procedure_bytes: bytes,
) -> tuple[bool, dict]:
    try:
        parsed = parse_historical_output(historical_output)
    except (UnicodeDecodeError, ValueError, KeyError) as error:
        return False, {"parse_error": str(error)}
    historical = physical.get("historical_attested_receipt", {})
    current = physical.get("current_live_drift_receipt", {})
    procedure = physical.get("recovered_procedure", {})
    categories = {row["category"]: row["count"] for row in physical.get("historical_category_counts", [])}
    output_digest = sha256(historical_output)
    body_digest = recovered_body_digest(procedure_bytes)
    valid = all(
        [
            inactive_source(physical),
            physical.get("admission_status") == "historical_attested_current_live_drift",
            physical.get("stage0_historical_evidence_admission") == "pass",
            physical.get("stage11_live_migration_admission") == "blocked_pending_recensus",
            physical.get("historical_baseline_is_non_promoting_evidence") is True,
            physical.get("current_live_rows_equal_historical_snapshot") is False,
            physical.get("current_live_equality_claim_must_fail_closed") is True,
            physical.get("literal_historical_rows_retained") is False,
            procedure.get("attestation_kind") == "immutable_historical_tool_call_output",
            procedure.get("source_call_id") == PHYSICAL_CALL_ID,
            procedure.get("source_session_jsonl_sha256") == SESSION_SHA256,
            procedure.get("exact_exec_command_sha256") == EXEC_COMMAND_SHA256,
            procedure.get("extracted_ruby_body_sha256") == body_digest == RUBY_BODY_SHA256,
            procedure.get("historical_output_bytes") == len(historical_output) == 459,
            procedure.get("historical_output_sha256") == output_digest == OUTPUT_SHA256,
            procedure.get("authority_or_currentness") is False,
            categories == parsed["category_counts"],
            historical.get("node_count") == parsed["node_count"] == 28102,
            historical.get("regular_file_count") == parsed["regular_file_count"] == 27883,
            historical.get("symlink_count") == parsed["symlink_count"] == 219,
            historical.get("payload_bytes") == parsed["payload_bytes_pass1"] == parsed["payload_bytes_pass2"] == 2723337235,
            historical.get("locator_digest") == parsed["locator_digest"],
            historical.get("identity_digest_pass1") == parsed["identity_digest_pass1"],
            historical.get("identity_digest_pass2") == parsed["identity_digest_pass2"],
            historical.get("stable") is parsed["stable"] is True,
            historical.get("changed_rows") == parsed["changed_rows"] == 0,
            current.get("claim_scope") == "informational_current_drift_observation_only",
            current.get("node_count") != historical.get("node_count"),
            current.get("locator_digest") != historical.get("locator_digest"),
            current.get("identity_digest_pass1") != historical.get("identity_digest_pass1"),
        ]
    )
    return valid, {
        "output_sha256": output_digest,
        "output_bytes": len(historical_output),
        "recovered_body_sha256": body_digest,
        "parsed": parsed,
    }


def verify_session_jsonl(path: Path, stored_output: bytes) -> dict:
    command = None
    output_blocks = None
    for line in path.read_text(encoding="utf-8").splitlines():
        payload = json.loads(line).get("payload", {})
        if payload.get("call_id") != PHYSICAL_CALL_ID:
            continue
        if payload.get("type") == "custom_tool_call":
            command = payload["input"]
        elif payload.get("type") == "custom_tool_call_output":
            output_blocks = payload["output"]
    if command is None or output_blocks is None:
        return {"status": "fail", "reason": "call input or output missing"}
    command_match = re.search(r"cmd: `(.*?)`, workdir:", command, re.DOTALL)
    body_match = re.search(r"ruby -rjson -rdigest -rset - <<'RUBY'\n(.*?)\nRUBY", command, re.DOTALL)
    if command_match is None or body_match is None:
        return {"status": "fail", "reason": "command or Ruby body framing missing"}
    joined_output = "".join(block["text"] for block in output_blocks).encode("utf-8")
    observed = {
        "session_jsonl_sha256": sha256(path.read_bytes()),
        "exec_command_sha256": sha256(command_match.group(1).encode("utf-8")),
        "ruby_body_sha256": sha256(body_match.group(1).encode("utf-8")),
        "output_sha256": sha256(joined_output),
        "output_matches_stored_evidence": joined_output == stored_output,
    }
    expected = {
        "session_jsonl_sha256": SESSION_SHA256,
        "exec_command_sha256": EXEC_COMMAND_SHA256,
        "ruby_body_sha256": RUBY_BODY_SHA256,
        "output_sha256": OUTPUT_SHA256,
        "output_matches_stored_evidence": True,
    }
    return {"status": "pass" if observed == expected else "fail", "observed": observed}


def ledger_valid(value: dict, count: int) -> tuple[bool, str]:
    digest = manifest_digest(value["rows"])
    valid = all(
        [
            inactive_source(value),
            value.get("evidence_classification") == "non_promoting_historical_coverage",
            value.get("current_source_equality_claimed") is False,
            len(value["rows"]) == value.get("expected_count") == count,
            digest == value.get("expected_digest"),
        ]
    )
    return valid, digest


def validate(
    repo: Path,
    verify_live: bool,
    live_root: Path,
    session_jsonl: Path | None,
) -> tuple[dict, bool]:
    root = repo / "contracts/vnext/public"
    embedded = load(root / "embedded_resources.e204.v1.json")
    consumers = load(root / "direct_consumers.c325.v1.json")
    physical = load(root / "physical_census.commitment.v1.json")
    historical_output_path = repo / physical["recovered_procedure"]["historical_output_path"]
    procedure_path = repo / physical["recovered_procedure"]["path"]
    historical_output = historical_output_path.read_bytes()
    embedded_pass, embedded_digest = ledger_valid(embedded, 204)
    consumer_pass, consumer_digest = ledger_valid(consumers, 325)
    physical_pass, physical_details = historical_evidence_valid(
        physical,
        historical_output,
        procedure_path.read_bytes(),
    )
    receipt = {
        "schema": "maestro.vnext.census-validation-receipt.v1",
        "stage0_historical_evidence_admission": "pass" if embedded_pass and consumer_pass and physical_pass else "fail",
        "stage11_live_migration_admission": "blocked_pending_recensus",
        "embedded": {
            "status": "non_promoting_historical_coverage_attested" if embedded_pass else "fail",
            "count": len(embedded["rows"]),
            "digest": embedded_digest,
            "current_source_equality_claimed": False,
        },
        "direct_consumers": {
            "status": "non_promoting_historical_coverage_attested" if consumer_pass else "fail",
            "count": len(consumers["rows"]),
            "digest": consumer_digest,
            "current_source_equality_claimed": False,
        },
        "physical": {
            "status": "historical_tool_output_attestation_pass" if physical_pass else "fail",
            "historical_locator_count": physical["historical_attested_receipt"]["node_count"],
            "current_live_locator_count": physical["current_live_drift_receipt"]["node_count"],
            "current_live_equality": False,
            "stage11_recensus_required": True,
            **physical_details,
        },
    }
    session_pass = True
    if session_jsonl is not None:
        session_receipt = verify_session_jsonl(session_jsonl, historical_output)
        receipt["session_source_verification"] = session_receipt
        session_pass = session_receipt["status"] == "pass"
    if verify_live:
        embedded_mismatches = live_mismatches(live_root, embedded["rows"])
        consumer_mismatches = live_mismatches(live_root, consumers["rows"])
        receipt["optional_current_comparison"] = {
            "root": str(live_root),
            "claim_scope": "listed-path byte comparison only; no currentness, completeness, Release, or global-absence claim",
            "current_source_equality_claimed": False,
            "embedded": {
                "status": "current_drift" if embedded_mismatches else "byte_equal_listed_paths_only",
                "mismatch_count": len(embedded_mismatches),
                "mismatches": embedded_mismatches,
            },
            "direct_consumers": {
                "status": "current_drift" if consumer_mismatches else "byte_equal_listed_paths_only",
                "mismatch_count": len(consumer_mismatches),
                "mismatches": consumer_mismatches,
            },
        }
    valid = embedded_pass and consumer_pass and physical_pass and session_pass
    return receipt, valid


def mutant_suite(repo: Path) -> dict:
    root = repo / "contracts/vnext/public"
    physical = load(root / "physical_census.commitment.v1.json")
    output = (repo / physical["recovered_procedure"]["historical_output_path"]).read_bytes()
    procedure = (repo / physical["recovered_procedure"]["path"]).read_bytes()
    mutants = []
    changed_output = bytearray(output)
    changed_output[-2] = ord("1")
    mutants.append(("historical-output-byte-flip", physical, bytes(changed_output)))
    changed = copy.deepcopy(physical)
    changed["historical_attested_receipt"]["node_count"] += 1
    mutants.append(("historical-count-substitution", changed, output))
    changed = copy.deepcopy(physical)
    changed["current_live_rows_equal_historical_snapshot"] = True
    mutants.append(("current-equality-promotion", changed, output))
    changed = copy.deepcopy(physical)
    changed["stage11_live_migration_admission"] = "pass"
    mutants.append(("stage11-recursor-bypass", changed, output))
    escaped = [
        name
        for name, mutant, mutant_output in mutants
        if historical_evidence_valid(mutant, mutant_output, procedure)[0]
    ]
    receipt = {
        "schema": "maestro.vnext.census-mutant-receipt.v1",
        "status": "pass" if not escaped else "fail",
        "total_mutants": len(mutants),
        "rejected_mutants": len(mutants) - len(escaped),
        "escaped": escaped,
    }
    if escaped:
        raise SystemExit(json.dumps(receipt, sort_keys=True))
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--live-root", type=Path)
    parser.add_argument("--verify-live", action="store_true")
    parser.add_argument("--verify-session-jsonl", type=Path)
    parser.add_argument("--mutant-suite", action="store_true")
    parser.add_argument("--require-live-migration", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    if args.mutant_suite:
        print(json.dumps(mutant_suite(repo), indent=2, sort_keys=True))
        return
    live_root = (args.live_root or repo).resolve()
    session_jsonl = args.verify_session_jsonl.resolve() if args.verify_session_jsonl else None
    receipt, valid = validate(repo, args.verify_live, live_root, session_jsonl)
    print(json.dumps(receipt, indent=2, sort_keys=True))
    if not valid:
        raise SystemExit(1)
    if args.require_live_migration:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
