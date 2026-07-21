#!/usr/bin/env python3
"""Verify the immutable external inputs admitted to vNext Stage 0.

The verified values are provenance only. This tool never returns one of them as
a canonical vNext identity and never writes the source repository.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import re
import stat
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_BINDINGS = ROOT / "contracts/vnext/stage0/input-bindings.json"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_equal(label: str, actual: object, expected: object) -> None:
    if actual != expected:
        raise SystemExit(f"{label}: expected {expected!r}, got {actual!r}")


def lp(name: str, value: str) -> bytes:
    encoded = value.encode("utf-8")
    return name.encode("utf-8") + b"=" + str(len(encoded)).encode("ascii") + b":" + encoded + b"\n"


def digest_records(domain: bytes, records: list[tuple[str, str]]) -> str:
    return hashlib.sha256(domain + b"".join(lp(name, value) for name, value in records)).hexdigest()


def path_metadata(path: Path) -> dict[str, str]:
    metadata = os.stat(path, follow_symlinks=False)
    if not stat.S_ISDIR(metadata.st_mode) or path.is_symlink():
        raise SystemExit(f"bound path is not a no-follow directory: {path}")
    return {
        "device": str(metadata.st_dev),
        "inode": str(metadata.st_ino),
        "uid": str(metadata.st_uid),
        "gid": str(metadata.st_gid),
        "mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
        "type": "directory",
    }


def verify_external_control_bindings(bindings: dict[str, object], source: Path) -> None:
    controls = bindings.get(
        "current_external_control_bindings", bindings["external_control_bindings"]
    )
    assert isinstance(controls, dict)
    source_control = controls["source_git_control"]
    destination = controls["destination_ancestors"]
    sandbox = controls["checkout_sandbox_profile"]
    assert isinstance(source_control, dict)
    assert isinstance(destination, dict)
    assert isinstance(sandbox, dict)

    git_dir = source / ".git"
    object_dir = git_dir / "objects"
    require_equal("git common dir realpath", str(git_dir.resolve(strict=True)), source_control["git_common_dir_realpath"])
    require_equal("object directory realpath", str(object_dir.resolve(strict=True)), source_control["object_directory_realpath"])
    for prefix, path in (("git_common_dir", git_dir), ("object_directory", object_dir)):
        metadata = path_metadata(path)
        for key in ("device", "inode", "uid", "gid", "mode"):
            require_equal(f"{prefix} {key}", metadata[key], source_control[f"{prefix}_{key}"])

    fixed_paths = {
        "config.worktree",
        "objects/info/alternates",
        "info/attributes",
        "info/grafts",
        "shallow",
    }
    replace_root = git_dir / "refs/replace"
    if replace_root.exists():
        fixed_paths.update(
            str(path.relative_to(git_dir))
            for path in replace_root.rglob("*")
            if path.is_file() or path.is_symlink()
        )
    fixed_paths.update(
        str(path.relative_to(git_dir)) for path in object_dir.glob("pack/*.promisor")
    )
    rows: list[str] = []
    for relative in sorted(fixed_paths):
        path = git_dir / relative
        if not path.exists() and not path.is_symlink():
            rows.append(f"{relative}\tabsent\t-")
        elif path.is_file() and not path.is_symlink():
            rows.append(f"{relative}\tregular\t{sha256(path)}")
        else:
            raise SystemExit(f"unsafe Git control path: {path}")
    require_equal("Git control rows", rows, source_control["git_control_path_manifest"])
    manifest = ("\n".join(rows) + "\n").encode("utf-8")
    require_equal(
        "Git control manifest digest",
        hashlib.sha256(manifest).hexdigest(),
        source_control["git_control_path_manifest_sha256"],
    )
    require_equal("repository config digest", sha256(git_dir / "config"), source_control["repository_config_sha256"])

    baseline = bindings["baseline"]
    assert isinstance(baseline, dict)
    source_records = [
        ("source_repository_realpath", str(source)),
        ("git_common_dir_realpath", str(git_dir.resolve(strict=True))),
        ("git_common_dir_device", str(path_metadata(git_dir)["device"])),
        ("git_common_dir_inode", str(path_metadata(git_dir)["inode"])),
        ("git_common_dir_uid", str(path_metadata(git_dir)["uid"])),
        ("git_common_dir_gid", str(path_metadata(git_dir)["gid"])),
        ("git_common_dir_mode", str(path_metadata(git_dir)["mode"])),
        ("object_directory_realpath", str(object_dir.resolve(strict=True))),
        ("object_directory_device", str(path_metadata(object_dir)["device"])),
        ("object_directory_inode", str(path_metadata(object_dir)["inode"])),
        ("object_directory_uid", str(path_metadata(object_dir)["uid"])),
        ("object_directory_gid", str(path_metadata(object_dir)["gid"])),
        ("object_directory_mode", str(path_metadata(object_dir)["mode"])),
        ("object_format", str(source_control["object_format"])),
        ("repository_config_sha256", str(source_control["repository_config_sha256"])),
        ("git_control_path_manifest_sha256", str(source_control["git_control_path_manifest_sha256"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
    ]
    require_equal(
        "source Git control identity",
        digest_records(b"maestro.external-git-source-control.v1\0", source_records),
        source_control["identity_sha256"],
    )

    ancestors = destination["ancestors"]
    assert isinstance(ancestors, list)
    destination_records = [("ancestor_count", str(len(ancestors)))]
    for index, entry in enumerate(ancestors):
        assert isinstance(entry, dict)
        path = Path(str(entry["path"]))
        require_equal(f"ancestor {index} realpath", str(path.resolve(strict=True)), entry["realpath"])
        metadata = path_metadata(path)
        for key in ("device", "inode", "uid", "gid", "mode", "type"):
            require_equal(f"ancestor {index} {key}", metadata[key], entry[key])
        for key in ("path", "realpath", "device", "inode", "uid", "gid", "mode", "type"):
            destination_records.append((f"ancestor_{index}_{key}", str(entry[key])))
    require_equal(
        "destination ancestor identity",
        digest_records(b"maestro.external-destination-ancestors.v1\0", destination_records),
        destination["identity_sha256"],
    )

    sandbox_names = [
        "profile_id", "config_sources", "hooks", "filters", "attributes_transform",
        "external_commands", "network", "submodule_checkout", "object_source",
        "registration", "materializer", "verifier", "symlink_policy",
    ]
    sandbox_records = [(name, str(sandbox[name])) for name in sandbox_names]
    require_equal(
        "checkout sandbox profile identity",
        digest_records(b"maestro.external-checkout-sandbox.v1\0", sandbox_records),
        sandbox["identity_sha256"],
    )


def verify_current_control_rebind(bindings: dict[str, object]) -> None:
    approved = bindings["external_control_bindings"]
    current = bindings["current_external_control_bindings"]
    assert isinstance(approved, dict)
    assert isinstance(current, dict)

    approved_source = approved["source_git_control"]
    current_source = current["source_git_control"]
    assert isinstance(approved_source, dict)
    assert isinstance(current_source, dict)
    require_equal("current source-control keys", set(current_source), set(approved_source))
    source_device_keys = {
        "identity_sha256",
        "git_common_dir_device",
        "object_directory_device",
    }
    for key in sorted(set(approved_source) - source_device_keys):
        require_equal(
            f"current source-control stable field {key}",
            current_source[key],
            approved_source[key],
        )
    approved_device = str(approved_source["git_common_dir_device"])
    current_device = str(current_source["git_common_dir_device"])
    if approved_device == current_device:
        raise SystemExit("current source-control rebind did not change the device")
    require_equal(
        "current object-directory device",
        current_source["object_directory_device"],
        current_device,
    )
    require_equal(
        "approved object-directory device",
        approved_source["object_directory_device"],
        approved_device,
    )

    approved_destination = approved["destination_ancestors"]
    current_destination = current["destination_ancestors"]
    assert isinstance(approved_destination, dict)
    assert isinstance(current_destination, dict)
    approved_ancestors = approved_destination["ancestors"]
    current_ancestors = current_destination["ancestors"]
    assert isinstance(approved_ancestors, list)
    assert isinstance(current_ancestors, list)
    require_equal(
        "current destination ancestor count",
        len(current_ancestors),
        len(approved_ancestors),
    )
    for index, (old_entry, new_entry) in enumerate(
        zip(approved_ancestors, current_ancestors, strict=True)
    ):
        assert isinstance(old_entry, dict)
        assert isinstance(new_entry, dict)
        require_equal(
            f"current destination ancestor {index} keys",
            set(new_entry),
            set(old_entry),
        )
        for key in sorted(set(old_entry) - {"device"}):
            require_equal(
                f"current destination ancestor {index} stable field {key}",
                new_entry[key],
                old_entry[key],
            )
        require_equal(
            f"current destination ancestor {index} device",
            new_entry["device"],
            current_device,
        )
        require_equal(
            f"approved destination ancestor {index} device",
            old_entry["device"],
            approved_device,
        )
    require_equal(
        "current checkout sandbox profile",
        current["checkout_sandbox_profile"],
        approved["checkout_sandbox_profile"],
    )


def verify_post_approval_execution_plan_revisions(
    bindings: dict[str, object],
) -> None:
    revision_set = bindings["post_approval_execution_plan_revisions"]
    canonical_inputs = bindings["canonical_source_inputs"]
    current_inputs = bindings["current_source_inputs"]
    assert isinstance(revision_set, dict)
    assert isinstance(canonical_inputs, dict)
    assert isinstance(current_inputs, dict)
    require_equal(
        "execution-plan revision schema",
        revision_set["schema"],
        "maestro.external-execution-plan-revisions.v1",
    )
    require_equal(
        "execution-plan revision attestation source",
        revision_set["attestation_source"],
        "pinned_local_codex_session_records_v1",
    )
    require_equal(
        "execution-plan revision provenance assurance",
        revision_set["provenance_assurance"],
        "unsigned_local_platform_log_bound_by_sha256",
    )
    require_equal(
        "current source input keys",
        set(current_inputs),
        {"card_sha256", "design_sha256", "decisions_sha256"},
    )
    for key in ("card_sha256", "decisions_sha256"):
        require_equal(
            f"post-approval stable {key}", current_inputs[key], canonical_inputs[key]
        )

    design_revisions = revision_set["design_revisions"]
    handoff = revision_set["stage5_handoff"]
    amendment = revision_set["stage5_execution_amendment"]
    assert isinstance(design_revisions, list)
    assert isinstance(handoff, dict)
    assert isinstance(amendment, dict)
    require_equal("post-approval design revision count", len(design_revisions), 2)
    records = [*design_revisions, handoff, amendment]
    wanted: dict[str, dict[str, object]] = {}
    for entry in records:
        assert isinstance(entry, dict)
        digest = str(entry["record_sha256"])
        if digest in wanted:
            raise SystemExit("post-approval revision record digest is duplicated")
        wanted[digest] = entry

    log_path = Path(str(revision_set["log_realpath"]))
    if not log_path.is_file() or log_path.is_symlink():
        raise SystemExit(f"execution-plan revision log is not a regular file: {log_path}")
    require_equal(
        "execution-plan revision log realpath",
        str(log_path.resolve(strict=True)),
        str(log_path),
    )
    captured: dict[str, dict[str, object]] = {}
    with log_path.open(encoding="utf-8") as handle:
        for raw_line in handle:
            digest = hashlib.sha256(raw_line.encode()).hexdigest()
            if digest not in wanted:
                continue
            if digest in captured:
                raise SystemExit(
                    f"post-approval revision record occurs more than once: {digest}"
                )
            record = json.loads(raw_line)
            if not isinstance(record, dict):
                raise SystemExit("post-approval revision record is not an object")
            captured[digest] = record
    require_equal(
        "post-approval revision record closure", set(captured), set(wanted)
    )

    expected_previous = str(canonical_inputs["design_sha256"])
    last_timestamp: str | None = None
    for index, entry in enumerate(design_revisions):
        assert isinstance(entry, dict)
        require_equal(
            f"design revision {index} previous hash",
            entry["previous_design_sha256"],
            expected_previous,
        )
        record = captured[str(entry["record_sha256"])]
        timestamp = str(record["timestamp"])
        if last_timestamp is not None and timestamp <= last_timestamp:
            raise SystemExit("post-approval design revisions are not in causal order")
        last_timestamp = timestamp
        payload = record["payload"]
        assert isinstance(payload, dict)
        require_equal(f"design revision {index} record type", record["type"], "response_item")
        require_equal(f"design revision {index} payload type", payload["type"], "message")
        require_equal(f"design revision {index} actor", payload["role"], "user")
        require_equal(f"design revision {index} message id", payload["id"], entry["message_id"])
        metadata = payload["internal_chat_message_metadata_passthrough"]
        assert isinstance(metadata, dict)
        require_equal(f"design revision {index} turn", metadata["turn_id"], entry["turn_id"])
        content = payload["content"]
        if not isinstance(content, list) or len(content) != 1 or not isinstance(content[0], dict):
            raise SystemExit(f"design revision {index} is not one exact text item")
        require_equal(f"design revision {index} content type", content[0]["type"], "input_text")
        text = str(content[0]["text"])
        required = (
            f"<source_thread_id>{entry['source_thread_id']}</source_thread_id>",
            str(entry["current_design_sha256"]),
        )
        if not all(fragment in text for fragment in required):
            raise SystemExit(f"design revision {index} does not bind its source and hash")
        classification = entry["classification"]
        if classification == "stage12_acceptance_clarification_only":
            fragments = (
                "post-approval Phase-12 acceptance clarification",
                "should not alter the active Stage 4 runtime slice",
                "preservation of all *V1/content-addressed contract, proof, Receipt, and migration identities",
            )
        elif classification == "implementation_order_only":
            fragments = (
                "The product contracts, Stage deliverables, and canonical integration/certification order remain unchanged.",
                "Existing proof artifacts with design provenance must be rebound/regenerated at the next safe boundary.",
                "Do not create or launch the Orchestrator yet.",
            )
        else:
            raise SystemExit(f"unknown post-approval design classification: {classification}")
        if not all(fragment in text for fragment in fragments):
            raise SystemExit(f"design revision {index} exceeds its declared classification")
        expected_previous = str(entry["current_design_sha256"])
    handoff_record = captured[str(handoff["record_sha256"])]
    handoff_timestamp = str(handoff_record["timestamp"])
    if last_timestamp is not None and handoff_timestamp <= last_timestamp:
        raise SystemExit("Stage-5 handoff is not causally after the design revisions")
    handoff_payload = handoff_record["payload"]
    assert isinstance(handoff_payload, dict)
    require_equal("Stage-5 handoff record type", handoff_record["type"], "response_item")
    require_equal("Stage-5 handoff payload type", handoff_payload["type"], "message")
    require_equal("Stage-5 handoff actor", handoff_payload["role"], "user")
    require_equal("Stage-5 handoff message id", handoff_payload["id"], handoff["message_id"])
    handoff_metadata = handoff_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(handoff_metadata, dict)
    require_equal("Stage-5 handoff turn", handoff_metadata["turn_id"], handoff["turn_id"])
    handoff_content = handoff_payload["content"]
    if not isinstance(handoff_content, list) or len(handoff_content) != 1 or not isinstance(handoff_content[0], dict):
        raise SystemExit("Stage-5 handoff is not one exact text item")
    require_equal("Stage-5 handoff content type", handoff_content[0]["type"], "input_text")
    handoff_text = str(handoff_content[0]["text"])
    require_equal("Stage-5 handoff design", handoff["design_sha256"], expected_previous)
    require_equal(
        "Stage-5 handoff boundary",
        handoff["boundary"],
        "finish_and_commit_stage5_do_not_begin_stage6",
    )
    handoff_fragments = (
        f"<source_thread_id>{handoff['source_thread_id']}</source_thread_id>",
        str(handoff["design_sha256"]),
        "Commit the complete Stage-5-owned source, tests and proof artifacts atomically",
        "Do not begin Stage 6.",
        "Do not create worker threads or an Orchestrator thread yet.",
    )
    if not all(fragment in handoff_text for fragment in handoff_fragments):
        raise SystemExit("Stage-5 handoff does not bind its exact continuation boundary")

    amendment_record = captured[str(amendment["record_sha256"])]
    amendment_timestamp = str(amendment_record["timestamp"])
    if amendment_timestamp <= handoff_timestamp:
        raise SystemExit("Stage-5 execution amendment is not causally after the handoff")
    amendment_payload = amendment_record["payload"]
    assert isinstance(amendment_payload, dict)
    require_equal(
        "Stage-5 execution amendment record type", amendment_record["type"], "response_item"
    )
    require_equal(
        "Stage-5 execution amendment payload type", amendment_payload["type"], "message"
    )
    require_equal("Stage-5 execution amendment actor", amendment_payload["role"], "user")
    require_equal(
        "Stage-5 execution amendment message id",
        amendment_payload["id"],
        amendment["message_id"],
    )
    amendment_metadata = amendment_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(amendment_metadata, dict)
    require_equal(
        "Stage-5 execution amendment turn",
        amendment_metadata["turn_id"],
        amendment["turn_id"],
    )
    amendment_content = amendment_payload["content"]
    if (
        not isinstance(amendment_content, list)
        or len(amendment_content) != 1
        or not isinstance(amendment_content[0], dict)
    ):
        raise SystemExit("Stage-5 execution amendment is not one exact text item")
    require_equal(
        "Stage-5 execution amendment content type",
        amendment_content[0]["type"],
        "input_text",
    )
    require_equal(
        "Stage-5 execution amendment previous design",
        amendment["previous_design_sha256"],
        expected_previous,
    )
    require_equal(
        "Stage-5 execution amendment current design",
        amendment["current_design_sha256"],
        current_inputs["design_sha256"],
    )
    require_equal(
        "Stage-5 execution amendment classification",
        amendment["classification"],
        "full_seal_scheduling_only",
    )
    require_equal(
        "Stage-5 execution amendment boundary",
        amendment["boundary"],
        "checkpoint_then_one_current_stage5_full_seal_then_stop_before_stage6",
    )
    amendment_text = str(amendment_content[0]["text"])
    amendment_fragments = (
        f"<source_thread_id>{amendment['source_thread_id']}</source_thread_id>",
        str(amendment["previous_design_sha256"]),
        str(amendment["current_design_sha256"]),
        "Preserve prior committed Stage receipts as immutable history.",
        "Main runs exactly one full independent multi-engine seal for the current Stage 5.",
        "checkpoint commit / primary ff-only preservation sequence remains in force",
        "Do not open an Orchestrator or workers.",
    )
    if not all(fragment in amendment_text for fragment in amendment_fragments):
        raise SystemExit("Stage-5 execution amendment exceeds its scheduling-only boundary")


def external_candidate_commitment(
    bindings: dict[str, object],
    *,
    source_control_sha256: str | None = None,
    destination_sha256: str | None = None,
) -> str:
    source_inputs = bindings["canonical_source_inputs"]
    controls = bindings["external_control_bindings"]
    external = bindings["external_candidate_input_fields"]
    approval = bindings["external_approval"]
    baseline = bindings["baseline"]
    assert isinstance(source_inputs, dict)
    assert isinstance(controls, dict)
    assert isinstance(external, dict)
    assert isinstance(approval, dict)
    assert isinstance(baseline, dict)
    records = [
        ("source_repository_realpath", str(bindings["source_repository_realpath"])),
        ("implementation_workspace_path", str(bindings["implementation_workspace_path"])),
        ("implementation_workspace_policy", str(bindings["implementation_workspace_policy"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
        ("source_git_control_binding_sha256", source_control_sha256 or str(controls["source_git_control"]["identity_sha256"])),
        ("destination_ancestor_binding_sha256", destination_sha256 or str(controls["destination_ancestors"]["identity_sha256"])),
        ("checkout_sandbox_profile_sha256", str(controls["checkout_sandbox_profile"]["identity_sha256"])),
        ("feature_id", str(bindings["feature_id"])),
        ("feature_state", str(bindings["feature_state"])),
        ("card_sha256", str(source_inputs["card_sha256"])),
        ("design_sha256", str(source_inputs["design_sha256"])),
        ("decisions_sha256", str(source_inputs["decisions_sha256"])),
        ("raw_decision_inventory_sha256", str(external["raw_decision_inventory_sha256"])),
        ("external_design_authority_closure_sha256", str(external["external_design_authority_closure_sha256"])),
        ("capability_census_sha256", str(external["capability_census_sha256"])),
        ("resource_consumer_census_sha256", str(external["resource_consumer_census_sha256"])),
        ("migration_rollback_removal_sha256", str(external["migration_rollback_removal_sha256"])),
    ]
    return digest_records(b"maestro.external-candidate-input.v1\0", records)


def verify_external_candidate_commitment(bindings: dict[str, object]) -> None:
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    require_equal(
        "external candidate input commitment",
        external_candidate_commitment(bindings),
        approval["candidate_input_commitment"],
    )


def verify_baseline_objects(bindings: dict[str, object], source: Path) -> None:
    workspace = Path(str(bindings["implementation_workspace_path"]))
    git_file = workspace / ".git"
    if not git_file.is_file() or git_file.is_symlink():
        raise SystemExit(f"implementation workspace lacks a regular Git pointer: {git_file}")
    pointer = git_file.read_text(encoding="utf-8").strip()
    if not pointer.startswith("gitdir: "):
        raise SystemExit("implementation workspace Git pointer is malformed")
    administrative_dir = Path(pointer.removeprefix("gitdir: ")).resolve(strict=True)
    common_dir = (source / ".git").resolve(strict=True)
    try:
        administrative_dir.relative_to(common_dir / "worktrees")
    except ValueError as error:
        raise SystemExit("implementation workspace is not registered under the bound source") from error

    baseline = bindings["baseline"]
    controls = bindings["external_control_bindings"]
    assert isinstance(baseline, dict)
    assert isinstance(controls, dict)
    source_control = controls["source_git_control"]
    assert isinstance(source_control, dict)
    commit_id = str(baseline["commit"])
    tree_id = str(baseline["tree"])
    head_value = (administrative_dir / "HEAD").read_text().strip()
    if head_value.startswith("ref: "):
        relative_ref = head_value.removeprefix("ref: ")
        if not relative_ref.startswith("refs/heads/") or ".." in Path(relative_ref).parts:
            raise SystemExit("implementation workspace HEAD has an unsafe symbolic ref")
        ref_path = common_dir / relative_ref
        if not ref_path.is_file() or ref_path.is_symlink():
            raise SystemExit("implementation workspace branch ref is not a regular file")
        head_value = ref_path.read_text().strip()
    current_base = bindings.get("current_implementation_base", baseline)
    assert isinstance(current_base, dict)
    current_commit_id = str(current_base["commit"])
    current_tree_id = str(current_base["tree"])
    require_equal("linked worktree common-dir pointer", (administrative_dir / "commondir").read_text().strip(), "../..")

    object_dir = Path(str(source_control["object_directory_realpath"]))

    def loose_object(object_id: str, expected_kind: str) -> bytes:
        path = object_dir / object_id[:2] / object_id[2:]
        if not path.is_file() or path.is_symlink():
            raise SystemExit(f"bound baseline {expected_kind} is not an available loose object: {object_id}")
        raw = zlib.decompress(path.read_bytes())
        require_equal(f"{expected_kind} object identity", hashlib.sha1(raw).hexdigest(), object_id)
        header, body = raw.split(b"\0", 1)
        kind, length = header.decode("ascii").split(" ", 1)
        require_equal(f"{expected_kind} object kind", kind, expected_kind)
        require_equal(f"{expected_kind} object length", len(body), int(length))
        return body

    commit = loose_object(commit_id, "commit")
    first_line = commit.splitlines()[0].decode("ascii")
    require_equal("baseline commit tree", first_line, f"tree {tree_id}")
    loose_object(tree_id, "tree")
    current_commit = loose_object(current_commit_id, "commit")
    current_first_line = current_commit.splitlines()[0].decode("ascii")
    require_equal(
        "current implementation commit tree",
        current_first_line,
        f"tree {current_tree_id}",
    )
    loose_object(current_tree_id, "tree")

    head_commit = loose_object(head_value, "commit")
    head_first_line = head_commit.splitlines()[0].decode("ascii")
    if not head_first_line.startswith("tree "):
        raise SystemExit("implementation workspace HEAD commit lacks a tree")
    loose_object(head_first_line.removeprefix("tree "), "tree")

    pending = [head_value]
    visited: set[str] = set()
    while pending:
        candidate = pending.pop()
        if candidate in visited:
            continue
        visited.add(candidate)
        if candidate == current_commit_id:
            break
        body = loose_object(candidate, "commit")
        pending.extend(
            line.removeprefix(b"parent ").decode("ascii")
            for line in body.splitlines()
            if line.startswith(b"parent ")
        )
    else:
        raise SystemExit("implementation workspace HEAD does not descend from the bound base")

    pending = [current_commit_id]
    visited = set()
    while pending:
        candidate = pending.pop()
        if candidate in visited:
            continue
        visited.add(candidate)
        if candidate == commit_id:
            break
        body = loose_object(candidate, "commit")
        parents = [
            line.removeprefix(b"parent ").decode("ascii")
            for line in body.splitlines()
            if line.startswith(b"parent ")
        ]
        pending.extend(parents)
    else:
        raise SystemExit("current implementation HEAD does not descend from the approved baseline")


def external_build_plan_handoff(
    bindings: dict[str, object], candidate_commitment: str | None = None
) -> str:
    plan = bindings["external_build_plan"]
    approval = bindings["external_approval"]
    assert isinstance(plan, dict)
    assert isinstance(approval, dict)
    records = [
        ("external_candidate_input_commitment", candidate_commitment or str(approval["candidate_input_commitment"])),
        ("recipient_task_id", str(plan["recipient_task_id"])),
        ("stage_plan_sha256", str(plan["stage_plan_sha256"])),
        ("proof_gate_sha256", str(plan["proof_gate_sha256"])),
        ("risk_recovery_sha256", str(plan["risk_recovery_sha256"])),
        ("adapter_removal_sha256", str(plan["adapter_removal_sha256"])),
    ]
    return digest_records(b"maestro.external-build-plan-handoff.v1\0", records)


def verify_external_build_plan(bindings: dict[str, object], source: Path) -> None:
    plan = bindings["external_build_plan"]
    current_plan = bindings.get("current_external_build_plan", plan)
    approval = bindings["external_approval"]
    assert isinstance(plan, dict)
    assert isinstance(current_plan, dict)
    assert isinstance(approval, dict)

    design = source / ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md"
    lines = design.read_text(encoding="utf-8").splitlines()
    header = "| Stage | Prerequisites | Exact deliverables | Smallest falsifying proof and recovery boundary | Next-stage condition |"
    try:
        start = lines.index(header)
    except ValueError as error:
        raise SystemExit("Stage-0 source design lacks the bound stage table") from error
    rows: list[list[str]] = []
    for line in lines[start + 2 :]:
        if not line:
            break
        if not line.startswith("|"):
            continue
        columns = [column.strip() for column in line[1:-1].split("|")]
        require_equal("stage-table column count", len(columns), 5)
        rows.append(columns)
    require_equal("stage-table row count", len(rows), 13)

    stage_lines = ["schema\tExternalStagePlanSummaryV1"] + [
        "\t".join((stage, prerequisites, deliverables, next_condition))
        for stage, prerequisites, deliverables, _proof, next_condition in rows
    ]
    proof_lines = ["schema\tExternalProofGateSummaryV1"] + [
        "\t".join((stage, proof)) for stage, _prerequisites, _deliverables, proof, _next_condition in rows
    ]
    require_equal(
        "external stage-plan digest",
        hashlib.sha256(("\n".join(stage_lines) + "\n").encode()).hexdigest(),
        current_plan["stage_plan_sha256"],
    )
    require_equal(
        "external proof-gate digest",
        hashlib.sha256(("\n".join(proof_lines) + "\n").encode()).hexdigest(),
        current_plan["proof_gate_sha256"],
    )
    require_equal(
        "current external build-plan classification",
        current_plan.get("classification"),
        "post_approval_stage12_clarification_execution_order_and_full_seal_scheduling_only",
    )
    for label, key in (
        ("risk-recovery", "risk_recovery_canonical_lines"),
        ("adapter-removal", "adapter_removal_canonical_lines"),
    ):
        canonical_lines = plan[key]
        assert isinstance(canonical_lines, list)
        require_equal(
            f"external {label} digest",
            hashlib.sha256(("\n".join(str(line) for line in canonical_lines) + "\n").encode()).hexdigest(),
            plan[f"{label.replace('-', '_')}_sha256"],
        )

    require_equal(
        "external build-plan handoff",
        external_build_plan_handoff(bindings),
        approval["build_plan_handoff"],
    )


def external_packet(
    bindings: dict[str, object],
    *,
    source_control_sha256: str | None = None,
    destination_sha256: str | None = None,
    candidate_commitment: str | None = None,
    build_plan_handoff: str | None = None,
) -> str:
    source_inputs = bindings["canonical_source_inputs"]
    controls = bindings["external_control_bindings"]
    external = bindings["external_candidate_input_fields"]
    approval = bindings["external_approval"]
    sections = bindings["external_packet_sections"]
    baseline = bindings["baseline"]
    assert isinstance(source_inputs, dict)
    assert isinstance(controls, dict)
    assert isinstance(external, dict)
    assert isinstance(approval, dict)
    assert isinstance(sections, dict)
    assert isinstance(baseline, dict)
    records = [
        ("feature_id", str(bindings["feature_id"])),
        ("feature_state", str(bindings["feature_state"])),
        ("source_repository_realpath", str(bindings["source_repository_realpath"])),
        ("implementation_workspace_path", str(bindings["implementation_workspace_path"])),
        ("implementation_workspace_policy", str(bindings["implementation_workspace_policy"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
        ("source_git_control_binding_sha256", source_control_sha256 or str(controls["source_git_control"]["identity_sha256"])),
        ("destination_ancestor_binding_sha256", destination_sha256 or str(controls["destination_ancestors"]["identity_sha256"])),
        ("checkout_sandbox_profile_sha256", str(controls["checkout_sandbox_profile"]["identity_sha256"])),
        ("card_sha256", str(source_inputs["card_sha256"])),
        ("design_sha256", str(source_inputs["design_sha256"])),
        ("decisions_sha256", str(source_inputs["decisions_sha256"])),
        ("raw_decision_inventory_sha256", str(external["raw_decision_inventory_sha256"])),
        ("external_design_authority_closure_sha256", str(external["external_design_authority_closure_sha256"])),
        ("external_candidate_input_commitment", candidate_commitment or str(approval["candidate_input_commitment"])),
        ("external_build_plan_handoff", build_plan_handoff or str(approval["build_plan_handoff"])),
        ("candidate_contract_root", "absent-before-stage-0"),
        ("canonical_build_handoff", "absent-before-stage-0"),
    ]
    section_order = [
        "identity_state", "product_constitution", "architecture_ownership",
        "capability_resource_census", "lifecycle_authority", "migration_rollback_removal",
        "implementation_stages_proofs", "risk_recovery", "advisor_dispositions_edge_sweep",
        "deviations_recommendation",
    ]
    records.extend((f"{name}_sha256", str(sections[name])) for name in section_order)
    return digest_records(b"maestro.external-build-approval-packet.v1\0", records)


def verify_external_packet(bindings: dict[str, object]) -> None:
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    require_equal(
        "external approval packet",
        external_packet(bindings),
        approval["packet_sha256"],
    )


def verify_external_approval_event(bindings: dict[str, object]) -> None:
    event = bindings["external_approval_event"]
    plan = bindings["external_build_plan"]
    approval = bindings["external_approval"]
    assert isinstance(event, dict)
    assert isinstance(plan, dict)
    assert isinstance(approval, dict)
    superseded = event["superseded_packet"]
    assert isinstance(superseded, dict)
    require_equal(
        "approval attestation source",
        event["attestation_source"],
        "pinned_local_codex_session_records_v1",
    )
    require_equal(
        "approval provenance assurance",
        event["provenance_assurance"],
        "unsigned_local_platform_log_bound_by_sha256",
    )
    require_equal("approval actor role", event["actor_role"], "user")
    require_equal("approval recipient", event["recipient_thread_id"], plan["recipient_task_id"])

    log_path = Path(str(event["log_realpath"]))
    if not log_path.is_file() or log_path.is_symlink():
        raise SystemExit(f"approval capture is not a regular no-follow log: {log_path}")
    require_equal("approval capture realpath", str(log_path.resolve(strict=True)), str(log_path))
    expected_records = event["record_sha256"]
    assert isinstance(expected_records, dict)
    required_record_names = {
        "session_meta",
        "superseded_packet_task_complete",
        "superseded_approval_task_started",
        "superseded_approval_user_message",
        "packet_task_complete",
        "approval_task_started",
        "approval_user_message",
    }
    require_equal("approval capture record names", set(expected_records), required_record_names)
    expected_by_digest = {str(digest): name for name, digest in expected_records.items()}
    require_equal(
        "approval capture record digest uniqueness",
        len(expected_by_digest),
        len(expected_records),
    )
    captured: dict[str, dict[str, object]] = {}
    captured_ordinals: dict[str, int] = {}
    captured_timestamps: dict[str, datetime.datetime] = {}
    with log_path.open("rb") as source:
        for ordinal, raw_line in enumerate(source, start=1):
            digest = hashlib.sha256(raw_line).hexdigest()
            name = expected_by_digest.get(digest)
            if name is None:
                continue
            if name in captured:
                raise SystemExit(f"approval capture record occurs more than once: {name}")
            record = json.loads(raw_line)
            if not isinstance(record, dict):
                raise SystemExit(f"approval capture record is not an object: {name}")
            captured[name] = record
            captured_ordinals[name] = ordinal
            captured_timestamps[name] = datetime.datetime.fromisoformat(
                str(record["timestamp"]).replace("Z", "+00:00")
            )
    require_equal("approval capture record closure", set(captured), required_record_names)
    ordered_names = [
        "session_meta",
        "superseded_packet_task_complete",
        "superseded_approval_task_started",
        "superseded_approval_user_message",
        "packet_task_complete",
        "approval_task_started",
        "approval_user_message",
    ]
    if [captured_ordinals[name] for name in ordered_names] != sorted(
        captured_ordinals[name] for name in ordered_names
    ):
        raise SystemExit("approval capture records are not in causal log order")
    if [captured_timestamps[name] for name in ordered_names] != sorted(
        captured_timestamps[name] for name in ordered_names
    ):
        raise SystemExit("approval capture record timestamps are not causal")

    capture_records = [
        ("log_realpath", str(log_path)),
        ("session_id", str(event["recipient_thread_id"])),
        ("session_meta_sha256", str(expected_records["session_meta"])),
        (
            "superseded_packet_task_complete_sha256",
            str(expected_records["superseded_packet_task_complete"]),
        ),
        (
            "superseded_approval_task_started_sha256",
            str(expected_records["superseded_approval_task_started"]),
        ),
        (
            "superseded_approval_user_message_sha256",
            str(expected_records["superseded_approval_user_message"]),
        ),
        (
            "packet_task_complete_sha256",
            str(expected_records["packet_task_complete"]),
        ),
        (
            "approval_task_started_sha256",
            str(expected_records["approval_task_started"]),
        ),
        (
            "approval_user_message_sha256",
            str(expected_records["approval_user_message"]),
        ),
        (
            "superseded_packet_body_sha256",
            str(event["superseded_packet"]["packet_body_sha256"]),
        ),
        ("packet_body_sha256", str(event["packet_body_sha256"])),
    ]
    require_equal(
        "approval capture record-set identity",
        digest_records(b"maestro.external-approval-capture.v1\0", capture_records),
        event["record_set_sha256"],
    )

    session_meta = captured["session_meta"]
    require_equal("approval session record type", session_meta["type"], "session_meta")
    session_payload = session_meta["payload"]
    assert isinstance(session_payload, dict)
    require_equal("approval session id", session_payload["session_id"], event["recipient_thread_id"])
    require_equal("approval session payload id", session_payload["id"], event["recipient_thread_id"])
    require_equal(
        "approval session source repository",
        session_payload["cwd"],
        bindings["source_repository_realpath"],
    )
    created_at = int(
        datetime.datetime.fromisoformat(
            str(session_payload["timestamp"]).replace("Z", "+00:00")
        ).timestamp()
    )
    require_equal("approval recipient creation time", created_at, event["recipient_thread_created_at"])

    controls = bindings["external_control_bindings"]
    baseline = bindings["baseline"]
    assert isinstance(controls, dict)
    assert isinstance(baseline, dict)
    source_control = controls["source_git_control"]
    destination = controls["destination_ancestors"]
    sandbox = controls["checkout_sandbox_profile"]
    assert isinstance(source_control, dict)
    assert isinstance(destination, dict)
    assert isinstance(sandbox, dict)
    require_equal("rebind sandbox identity", superseded["checkout_sandbox_profile_sha256"], sandbox["identity_sha256"])
    old_device = str(event["superseded_device"])
    current_device = str(event["current_device"])
    if old_device == current_device:
        raise SystemExit("reboot rebind did not change the filesystem device")
    old_source_records = [
        ("source_repository_realpath", str(bindings["source_repository_realpath"])),
        ("git_common_dir_realpath", str(source_control["git_common_dir_realpath"])),
        ("git_common_dir_device", old_device),
        ("git_common_dir_inode", str(source_control["git_common_dir_inode"])),
        ("git_common_dir_uid", str(source_control["git_common_dir_uid"])),
        ("git_common_dir_gid", str(source_control["git_common_dir_gid"])),
        ("git_common_dir_mode", str(source_control["git_common_dir_mode"])),
        ("object_directory_realpath", str(source_control["object_directory_realpath"])),
        ("object_directory_device", old_device),
        ("object_directory_inode", str(source_control["object_directory_inode"])),
        ("object_directory_uid", str(source_control["object_directory_uid"])),
        ("object_directory_gid", str(source_control["object_directory_gid"])),
        ("object_directory_mode", str(source_control["object_directory_mode"])),
        ("object_format", str(source_control["object_format"])),
        ("repository_config_sha256", str(source_control["repository_config_sha256"])),
        ("git_control_path_manifest_sha256", str(source_control["git_control_path_manifest_sha256"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
    ]
    old_source_identity = digest_records(
        b"maestro.external-git-source-control.v1\0", old_source_records
    )
    require_equal(
        "superseded source-control identity",
        old_source_identity,
        superseded["source_git_control_binding_sha256"],
    )
    ancestors = destination["ancestors"]
    assert isinstance(ancestors, list)
    old_destination_records = [("ancestor_count", str(len(ancestors)))]
    for index, entry in enumerate(ancestors):
        assert isinstance(entry, dict)
        for key in ("path", "realpath", "device", "inode", "uid", "gid", "mode", "type"):
            value = old_device if key == "device" else str(entry[key])
            old_destination_records.append((f"ancestor_{index}_{key}", value))
    old_destination_identity = digest_records(
        b"maestro.external-destination-ancestors.v1\0", old_destination_records
    )
    require_equal(
        "superseded destination identity",
        old_destination_identity,
        superseded["destination_ancestor_binding_sha256"],
    )
    old_candidate = external_candidate_commitment(
        bindings,
        source_control_sha256=old_source_identity,
        destination_sha256=old_destination_identity,
    )
    require_equal(
        "superseded candidate input commitment",
        old_candidate,
        superseded["candidate_input_commitment"],
    )
    old_handoff = external_build_plan_handoff(bindings, old_candidate)
    require_equal(
        "superseded build-plan handoff",
        old_handoff,
        superseded["build_plan_handoff"],
    )
    require_equal(
        "superseded approval packet",
        external_packet(
            bindings,
            source_control_sha256=old_source_identity,
            destination_sha256=old_destination_identity,
            candidate_commitment=old_candidate,
            build_plan_handoff=old_handoff,
        ),
        superseded["packet_sha256"],
    )

    old_packet_record = captured["superseded_packet_task_complete"]
    require_equal("superseded packet record type", old_packet_record["type"], "event_msg")
    old_packet_payload = old_packet_record["payload"]
    assert isinstance(old_packet_payload, dict)
    require_equal("superseded packet event type", old_packet_payload["type"], "task_complete")
    require_equal("superseded packet turn", old_packet_payload["turn_id"], superseded["packet_turn_id"])
    require_equal("superseded packet completion", old_packet_payload["completed_at"], superseded["packet_turn_completed_at"])
    old_packet_body = str(old_packet_payload["last_agent_message"])
    require_equal(
        "superseded packet body identity",
        hashlib.sha256(old_packet_body.encode()).hexdigest(),
        superseded["packet_body_sha256"],
    )
    if f"External packet SHA-256:\n\n`{superseded['packet_sha256']}`" not in old_packet_body:
        raise SystemExit("superseded packet body does not bind its packet digest")

    old_started = captured["superseded_approval_task_started"]
    require_equal("superseded approval-start record type", old_started["type"], "event_msg")
    old_started_payload = old_started["payload"]
    assert isinstance(old_started_payload, dict)
    require_equal("superseded approval-start event", old_started_payload["type"], "task_started")
    require_equal("superseded approval-start turn", old_started_payload["turn_id"], superseded["approval_turn_id"])
    require_equal("superseded approval-start time", old_started_payload["started_at"], superseded["approval_turn_started_at"])

    old_message = captured["superseded_approval_user_message"]
    require_equal("superseded approval-message record type", old_message["type"], "response_item")
    old_message_payload = old_message["payload"]
    assert isinstance(old_message_payload, dict)
    require_equal("superseded approval-message type", old_message_payload["type"], "message")
    require_equal("superseded approval-message actor", old_message_payload["role"], "user")
    require_equal("superseded approval-message id", old_message_payload.get("id"), superseded["user_message_id"])
    old_metadata = old_message_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(old_metadata, dict)
    require_equal("superseded approval-message turn", old_metadata["turn_id"], superseded["approval_turn_id"])
    old_content = old_message_payload["content"]
    if not isinstance(old_content, list) or len(old_content) != 1 or not isinstance(old_content[0], dict):
        raise SystemExit("superseded approval-message content is not one text item")
    require_equal("superseded approval-message content type", old_content[0]["type"], "input_text")
    old_expected_instruction = (
        f"APPROVE BUILD PACKET sha256:{superseded['packet_sha256']}. "
        "Execute the staged build plan."
    )
    require_equal(
        "superseded declared approval instruction",
        superseded["exact_instruction"],
        old_expected_instruction,
    )
    require_equal(
        "superseded external approval instruction",
        old_content[0]["text"],
        old_expected_instruction + "\n",
    )
    if int(superseded["packet_turn_completed_at"]) >= int(superseded["approval_turn_started_at"]):
        raise SystemExit("superseded approval is not causally later than its packet")

    packet_record = captured["packet_task_complete"]
    require_equal("packet capture record type", packet_record["type"], "event_msg")
    packet_payload = packet_record["payload"]
    assert isinstance(packet_payload, dict)
    require_equal("packet capture event type", packet_payload["type"], "task_complete")
    require_equal("packet capture turn", packet_payload["turn_id"], event["packet_turn_id"])
    require_equal(
        "packet capture completion time",
        packet_payload["completed_at"],
        event["packet_turn_completed_at"],
    )
    packet_body = str(packet_payload["last_agent_message"])
    require_equal(
        "published packet body identity",
        hashlib.sha256(packet_body.encode()).hexdigest(),
        event["packet_body_sha256"],
    )
    publication_kind = event.get("packet_publication_kind", "full_build_packet_v1")
    if publication_kind == "full_build_packet_v1":
        if f"External packet SHA-256:\n\n`{approval['packet_sha256']}`" not in packet_body:
            raise SystemExit("published packet body does not bind the approved packet digest")
    elif publication_kind == "reboot_device_rebind_v1":
        source_control = bindings["external_control_bindings"]["source_git_control"]
        assert isinstance(source_control, dict)
        require_equal(
            "rebound packet current device",
            event["current_device"],
            source_control["git_common_dir_device"],
        )
        token = f"APPROVE BUILD PACKET sha256:{approval['packet_sha256']}. Execute the staged build plan."
        required_fragments = (
            event["superseded_device"],
            event["current_device"],
            "with no design or scope changes",
            token,
        )
        if not all(str(fragment) in packet_body for fragment in required_fragments):
            raise SystemExit("published reboot rebind packet does not bind its exact safety delta")
    else:
        raise SystemExit(f"unknown packet publication kind: {publication_kind}")

    started_record = captured["approval_task_started"]
    require_equal("approval-start record type", started_record["type"], "event_msg")
    started_payload = started_record["payload"]
    assert isinstance(started_payload, dict)
    require_equal("approval-start event type", started_payload["type"], "task_started")
    require_equal("approval-start turn", started_payload["turn_id"], event["approval_turn_id"])
    require_equal(
        "approval-start time",
        started_payload["started_at"],
        event["approval_turn_started_at"],
    )

    message_record = captured["approval_user_message"]
    require_equal("approval-message record type", message_record["type"], "response_item")
    message_payload = message_record["payload"]
    assert isinstance(message_payload, dict)
    require_equal("approval-message payload type", message_payload["type"], "message")
    require_equal("approval-message actor", message_payload["role"], event["actor_role"])
    require_equal("approval-message id", message_payload.get("id"), event.get("user_message_id"))
    metadata = message_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(metadata, dict)
    require_equal("approval-message turn", metadata["turn_id"], event["approval_turn_id"])
    content = message_payload["content"]
    if not isinstance(content, list) or len(content) != 1 or not isinstance(content[0], dict):
        raise SystemExit("approval-message content is not the exact single text item")
    require_equal("approval-message content type", content[0]["type"], "input_text")
    if int(event["packet_turn_completed_at"]) >= int(event["approval_turn_started_at"]):
        raise SystemExit("approval event is not causally later than packet publication")
    if int(event["recipient_thread_created_at"]) >= int(event["packet_turn_completed_at"]):
        raise SystemExit("recipient task incarnation does not predate the packet")
    if event["packet_turn_id"] == event["approval_turn_id"]:
        raise SystemExit("approval event reused the packet turn")
    expected = f"APPROVE BUILD PACKET sha256:{approval['packet_sha256']}. Execute the staged build plan."
    require_equal("exact external approval instruction", event["exact_instruction"], expected)
    completion = event.get("completion_instruction")
    expected_content = expected + "\n"
    if completion is not None:
        require_equal("completion instruction", completion, "Approve go until done")
        expected_content += completion
    require_equal("captured external approval instruction", content[0]["text"], expected_content)


def verify_source_inputs(bindings: dict[str, object], source: Path) -> None:
    canonical_inputs = bindings["canonical_source_inputs"]
    current_inputs = bindings["current_source_inputs"]
    provenance = bindings["provenance_evidence_inputs"]
    assert isinstance(canonical_inputs, dict)
    assert isinstance(current_inputs, dict)
    assert isinstance(provenance, dict)
    require_equal(
        "canonical source input keys",
        set(canonical_inputs),
        {"card_sha256", "design_sha256", "decisions_sha256"},
    )
    require_equal(
        "provenance evidence classification",
        provenance.get("classification"),
        "non_authoritative_coverage_evidence_only",
    )
    require_equal(
        "provenance evidence packet binding",
        provenance.get("packet_binding"),
        "excluded_by_design_canonical_card_design_decisions_are_authority",
    )
    evidence_inputs = provenance.get("artifacts")
    assert isinstance(evidence_inputs, dict)
    require_equal(
        "provenance evidence input keys",
        set(evidence_inputs),
        {"final_main_sha256", "post_refoundation_brainstorm_sha256"},
    )
    canonical_paths = {
        "card_sha256": source
        / ".maestro/cards/maestro-whole-flow-architecture-refoundation/card.yaml",
        "design_sha256": source
        / ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md",
        "decisions_sha256": source
        / ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml",
    }
    evidence_paths = {
        "final_main_sha256": source / "FINAL-MAIN.md",
        "post_refoundation_brainstorm_sha256": source
        / "SPEC-MAESTRO-VNEXT-POST-REFOUNDATION-BRAINSTORM.md",
    }
    for key, path in canonical_paths.items():
        if not path.is_file():
            raise SystemExit(f"{key}: missing regular file {path}")
        require_equal(key, sha256(path), current_inputs[key])
    for key, path in evidence_paths.items():
        if not path.is_file():
            raise SystemExit(f"{key}: missing regular file {path}")
        require_equal(key, sha256(path), evidence_inputs[key])


def verify_decision_inventory(bindings: dict[str, object], source: Path) -> None:
    decisions = source / (
        ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml"
    )
    text = decisions.read_text(encoding="utf-8")
    statuses = re.findall(r"^  status: (locked|superseded|open)$", text, re.MULTILINE)
    expected = bindings["decision_inventory"]
    assert isinstance(expected, dict)
    require_equal("decision total", len(statuses), expected["total"])
    for status in ("locked", "superseded", "open"):
        require_equal(
            f"decision {status}", statuses.count(status), expected[status]
        )

    head = str(expected["effective_composite_head"])
    if not re.search(rf"^  id: {re.escape(head)}$", text, re.MULTILINE):
        raise SystemExit(f"effective composite head is absent: {head}")


def verify_nonpromotion(bindings: dict[str, object]) -> None:
    require_equal(
        "canonical role",
        bindings["canonical_role"],
        "external_non_promoting_provenance",
    )
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    required_refusals = {
        "candidate_contract_root",
        "canonical_build_handoff",
        "manifest_identity",
        "mandate",
        "action_request",
        "receipt",
    }
    require_equal(
        "external approval nonpromotion set",
        set(approval["must_not_be_used_as"]),
        required_refusals,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bindings", type=Path, default=DEFAULT_BINDINGS)
    parser.add_argument("--source", type=Path)
    args = parser.parse_args()

    bindings_path = args.bindings.resolve(strict=True)
    bindings = json.loads(bindings_path.read_text(encoding="utf-8"))
    require_equal(
        "binding schema",
        bindings["schema"],
        "maestro.vnext.stage0-input-bindings.v1",
    )
    source = (args.source or Path(str(bindings["source_repository_realpath"]))).resolve(
        strict=True
    )
    require_equal(
        "source repository realpath",
        str(source),
        bindings["source_repository_realpath"],
    )
    verify_nonpromotion(bindings)
    verify_external_control_bindings(bindings, source)
    verify_current_control_rebind(bindings)
    verify_external_candidate_commitment(bindings)
    verify_baseline_objects(bindings, source)
    verify_external_build_plan(bindings, source)
    verify_external_packet(bindings)
    verify_external_approval_event(bindings)
    verify_post_approval_execution_plan_revisions(bindings)
    verify_source_inputs(bindings, source)
    verify_decision_inventory(bindings, source)
    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-input-verification.v1",
                "bindings_sha256": sha256(bindings_path),
                "feature_id": bindings["feature_id"],
                "decision_total": bindings["decision_inventory"]["total"],
                "result": "verified_non_promoting",
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
