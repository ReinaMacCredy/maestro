#!/usr/bin/env python3
"""Runs independent Stage-0 encoder/validator receipts and adversarial mutants."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
CONTRACT = ROOT / "contracts/vnext/stage0/decision-closure"
PYTHON = Path(__file__).with_name("validate.py")
RUBY = Path(__file__).with_name("validate.rb")


def command(command: list[str], root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, "STAGE0_DECISION_CLOSURE_ROOT": str(root)},
    )


def rows(document: dict[str, object]) -> list[dict[str, object]]:
    return document["records"]  # type: ignore[return-value]


def mutate_documents(name: str, external: dict[str, object], decision: dict[str, object]) -> None:
    external_rows = rows(external)
    decision_rows = rows(decision)
    if name == "open_status":
        external_rows[0]["terminal_status"] = decision_rows[0]["terminal_status"] = "open"
    elif name == "raw_lineage_omission":
        index = next(index for index, row in enumerate(external_rows) if row["terminal_status"] == "superseded")
        external_rows[index]["raw_superseded_by"] = decision_rows[index]["raw_superseded_by"] = []
    elif name == "normalized_cycle":
        source = next(row for row in external_rows if row["normalized_successor"] is not None)
        target_id = source["normalized_successor"]
        target_index = next(index for index, row in enumerate(external_rows) if row["id"] == target_id)
        external_rows[target_index]["normalized_successor"] = decision_rows[target_index]["normalized_successor"] = source["id"]
    elif name == "unilateral_repair":
        index = next(index for index, row in enumerate(external_rows) if row["external_authoring_disposition"] == "unilateral_raw_claim")
        external_rows[index]["normalized_successor"] = decision_rows[index]["normalized_successor"] = external_rows[0]["id"]
    elif name == "composite_promotion":
        index = next(index for index, row in enumerate(external_rows) if row["external_authoring_disposition"] == "composite_external_authoring")
        external_rows[index]["normalized_successor"] = decision_rows[index]["normalized_successor"] = external_rows[0]["id"]
    elif name == "omitted_record":
        external_rows.pop()
        decision_rows.pop()
    elif name == "changed_body_hash":
        index = next(index for index, row in enumerate(external_rows) if row["consequence_classification"] == "material")
        external_rows[index]["raw_body_sha256"] = decision_rows[index]["raw_body_sha256"] = "00" * 32
    elif name == "missing_rationale":
        index = next(index for index, row in enumerate(external_rows) if row["consequence_classification"] == "rationale_only")
        external_rows[index]["rationale_disposition"] = decision_rows[index]["rationale_disposition"] = None
    elif name == "missing_materialization":
        index = next(index for index, row in enumerate(external_rows) if row["consequence_classification"] == "material")
        external_rows[index]["materialization_ids"] = decision_rows[index]["materialization_ids"] = []
    elif name == "stale_materialization":
        external["materializations"][0]["decision_sources"][0]["body_sha256"] = "00" * 32  # type: ignore[index]
        decision["materializations"][0]["decision_sources"][0]["body_sha256"] = "00" * 32  # type: ignore[index]
    elif name == "duplicate_materialization":
        external["materializations"].append(copy.deepcopy(external["materializations"][0]))  # type: ignore[index]
        decision["materializations"].append(copy.deepcopy(decision["materializations"][0]))  # type: ignore[index]
    elif name == "effect_live_omission":
        index = next(index for index, row in enumerate(external_rows) if row["derived_effect_status"] == "superseded_but_effect_live")
        external_rows[index]["derived_effect_status"] = decision_rows[index]["derived_effect_status"] = "historical"
    elif name == "reordered_input":
        external_rows[0], external_rows[1] = external_rows[1], external_rows[0]
        decision_rows[0], decision_rows[1] = decision_rows[1], decision_rows[0]
    elif name == "fabricated_root_resolution":
        external["root_assembly"]["state"] = decision["root_assembly"]["state"] = "resolved"
    else:
        raise AssertionError(name)


def write_documents(root: Path, external: dict[str, object], decision: dict[str, object]) -> None:
    (root / "external-design-authority-closure.v1.json").write_text(json.dumps(external, separators=(",", ":")) + "\n", encoding="ascii")
    (root / "decision-closure.v1.json").write_text(json.dumps(decision, separators=(",", ":")) + "\n", encoding="ascii")


def main() -> int:
    baseline_python = command(["python3", str(PYTHON)], CONTRACT)
    baseline_ruby = command(["ruby", str(RUBY)], CONTRACT)
    if baseline_python.returncode or baseline_ruby.returncode:
        raise SystemExit("baseline validation failed")
    python_receipt = json.loads(baseline_python.stdout)
    ruby_receipt = json.loads(baseline_ruby.stdout)
    if python_receipt["external_closure_id"] != ruby_receipt["external_closure_id"] or python_receipt["decision_closure_id"] != ruby_receipt["decision_closure_id"]:
        raise SystemExit("independent encoders disagree")

    external = json.loads((CONTRACT / "external-design-authority-closure.v1.json").read_text(encoding="ascii"))
    decision = json.loads((CONTRACT / "decision-closure.v1.json").read_text(encoding="ascii"))
    mutant_names = [
        "open_status", "raw_lineage_omission", "normalized_cycle", "unilateral_repair",
        "composite_promotion", "omitted_record", "changed_body_hash", "missing_rationale",
        "missing_materialization", "stale_materialization", "duplicate_materialization",
        "effect_live_omission", "reordered_input", "fabricated_root_resolution",
    ]
    rejected = {"python": 0, "ruby": 0}
    for name in mutant_names:
        with tempfile.TemporaryDirectory(prefix="maestro-stage0-decision-mutant-") as directory:
            root = Path(directory)
            shutil.copy(CONTRACT / "external-design-authority-closure.v1.cbor", root)
            shutil.copy(CONTRACT / "decision-closure.v1.cbor", root)
            mutated_external = copy.deepcopy(external)
            mutated_decision = copy.deepcopy(decision)
            mutate_documents(name, mutated_external, mutated_decision)
            write_documents(root, mutated_external, mutated_decision)
            for label, executable in (("python", ["python3", str(PYTHON)]), ("ruby", ["ruby", str(RUBY)])):
                result = command(executable, root)
                if result.returncode == 0:
                    raise SystemExit(f"{label} accepted mutant {name}")
                rejected[label] += 1

    receipt = {
        "schema": "maestro.vnext.stage0-decision-closure-encoder-receipt.v1",
        "external_closure_id": python_receipt["external_closure_id"],
        "decision_closure_id": python_receipt["decision_closure_id"],
        "encoder_equality": "pass",
        "semantic_validation": {"python": "pass", "ruby": "pass"},
        "mutants": {"cases": mutant_names, "rejected": rejected, "total": sum(rejected.values())},
        "validator_sha256": {"python": hashlib.sha256(PYTHON.read_bytes()).hexdigest(), "ruby": hashlib.sha256(RUBY.read_bytes()).hexdigest()},
    }
    (CONTRACT / "encoder-receipt.v1.json").write_text(json.dumps(receipt, sort_keys=True, separators=(",", ":")) + "\n", encoding="ascii")
    print(json.dumps(receipt, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
