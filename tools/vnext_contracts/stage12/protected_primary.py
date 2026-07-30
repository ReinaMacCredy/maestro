#!/usr/bin/env python3
"""Reconstruct the canonical V7 protected-primary currentness identity."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import stat
import subprocess
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, cast


BINDING_SCHEMA = "maestro.external.v7.1-protected-primary-binding.v1"
BINDING_POLICY = (
    "read_only_never_stage_stash_reset_clean_normalize_overwrite_or_CAS_target"
)
BINDING_RELATIVE_PATH = PurePosixPath(
    "control/stage12/packet/protected-primary-binding.v7.1.json"
)
SHA1 = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
CURRENTNESS_FIELDS = (
    "repository_realpath",
    "commit",
    "tree",
    "dirty_path_count",
    "dirty_path_manifest_sha256",
    "tracked_binary_diff_sha256",
    "untracked_regular_file_count",
    "untracked_regular_file_manifest_identity",
)
BINDING_FIELDS = {
    "schema",
    *CURRENTNESS_FIELDS,
    "boundary_identity",
    "boundary_file_sha256",
    "policy",
    "identity_sha256",
}
GIT_ENV = {
    **os.environ,
    "GIT_OPTIONAL_LOCKS": "0",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "LC_ALL": "C",
    "LANG": "C",
}


class ProtectedPrimaryError(RuntimeError):
    """The protected-primary binding or live checkout is not exact."""


def canonical_bytes(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode("ascii")


def sha256(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def with_identity(value: Mapping[str, Any]) -> dict[str, Any]:
    result = copy.deepcopy(dict(value))
    result["identity_sha256"] = sha256(canonical_bytes(value))
    return result


def _reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ProtectedPrimaryError(f"duplicate protected-primary JSON key: {key}")
        result[key] = value
    return result


def validate_binding(value: Mapping[str, Any]) -> dict[str, Any]:
    if set(value) != BINDING_FIELDS:
        raise ProtectedPrimaryError("protected-primary binding fields differ")
    core = dict(value)
    identity = core.pop("identity_sha256")
    if (
        value.get("schema") != BINDING_SCHEMA
        or value.get("policy") != BINDING_POLICY
        or not isinstance(value.get("repository_realpath"), str)
        or not str(value["repository_realpath"]).startswith("/")
        or SHA1.fullmatch(str(value.get("commit", ""))) is None
        or SHA1.fullmatch(str(value.get("tree", ""))) is None
        or not isinstance(value.get("dirty_path_count"), int)
        or cast(int, value["dirty_path_count"]) < 0
        or not isinstance(value.get("untracked_regular_file_count"), int)
        or cast(int, value["untracked_regular_file_count"]) < 0
        or any(
            SHA256.fullmatch(str(value.get(field, ""))) is None
            for field in (
                "dirty_path_manifest_sha256",
                "tracked_binary_diff_sha256",
                "untracked_regular_file_manifest_identity",
                "boundary_identity",
                "boundary_file_sha256",
            )
        )
        or identity != sha256(canonical_bytes(core))
    ):
        raise ProtectedPrimaryError("protected-primary binding identity differs")
    return copy.deepcopy(dict(value))


def load_binding(path: Path) -> dict[str, Any]:
    try:
        metadata = os.lstat(path)
        if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
            raise ProtectedPrimaryError(
                "protected-primary binding is not a regular non-symlink file"
            )
        raw = path.read_bytes()
    except OSError as error:
        raise ProtectedPrimaryError(
            f"cannot read protected-primary binding: {error}"
        ) from error
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise ProtectedPrimaryError("protected-primary binding JSON is not canonical")
    try:
        value = json.loads(raw, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProtectedPrimaryError(
            f"protected-primary binding JSON is invalid: {error}"
        ) from error
    if not isinstance(value, Mapping):
        raise ProtectedPrimaryError("protected-primary binding must be one object")
    return validate_binding(value)


def _git(repository: Path, *arguments: str) -> bytes:
    result = subprocess.run(
        [
            "git",
            "--no-replace-objects",
            "--no-optional-locks",
            "-C",
            str(repository),
            *arguments,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=GIT_ENV,
    )
    if result.returncode != 0:
        error = result.stderr.decode("utf-8", "replace").strip()
        raise ProtectedPrimaryError(
            f"protected-primary Git read failed: {error or result.returncode}"
        )
    return result.stdout


def _status_rows(raw: bytes) -> list[tuple[str, str]]:
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ProtectedPrimaryError(
            "protected-primary status is not canonical UTF-8"
        ) from error
    rows: list[tuple[str, str]] = []
    for line in lines:
        if len(line) < 4:
            raise ProtectedPrimaryError(
                f"invalid protected-primary status row: {line!r}"
            )
        rows.append((line[:2], line[3:]))
    return rows


def _status_manifest(rows: list[tuple[str, str]]) -> bytes:
    return (
        "schema\tExternalPrimaryDirtyPathManifestV1\n"
        + "\n".join(f"{status}\t{path}" for status, path in rows)
        + "\n"
    ).encode("utf-8")


def _safe_untracked_path(repository: Path, relative: str) -> Path:
    if not relative or "\\" in relative:
        raise ProtectedPrimaryError(
            f"protected-primary untracked path is unsafe: {relative!r}"
        )
    parsed = PurePosixPath(relative)
    if parsed.is_absolute() or any(part in {"", ".", ".."} for part in parsed.parts):
        raise ProtectedPrimaryError(
            f"protected-primary untracked path is unsafe: {relative!r}"
        )
    return repository.joinpath(*parsed.parts)


def _untracked_manifest(
    repository: Path, rows: list[tuple[str, str]]
) -> dict[str, Any]:
    files: list[dict[str, Any]] = []
    for status_code, relative in rows:
        if status_code != "??":
            continue
        path = _safe_untracked_path(repository, relative)
        try:
            before = os.lstat(path)
            if not stat.S_ISREG(before.st_mode) or path.is_symlink():
                raise ProtectedPrimaryError(
                    "protected-primary untracked path is not a regular file: "
                    f"{relative}"
                )
            raw = path.read_bytes()
            after = os.lstat(path)
        except OSError as error:
            raise ProtectedPrimaryError(
                f"cannot read protected-primary untracked file {relative}: {error}"
            ) from error
        before_identity = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_size,
            before.st_mtime_ns,
        )
        after_identity = (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_size,
            after.st_mtime_ns,
        )
        if before_identity != after_identity:
            raise ProtectedPrimaryError(
                f"protected-primary untracked file changed while read: {relative}"
            )
        files.append(
            {
                "length": len(raw),
                "mode": stat.S_IMODE(after.st_mode),
                "path": relative,
                "sha256": sha256(raw),
            }
        )
    return with_identity(
        {
            "schema": "maestro.external.primary-untracked-regular-file-manifest.v1",
            "authority_state": "none_read_only_exact_byte_boundary",
            "repository_realpath": str(repository),
            "file_count": len(files),
            "files": files,
            "policy": (
                "exact_path_length_mode_sha256_"
                "fail_closed_before_any_successor_mutation"
            ),
        }
    )


def _observe_once(repository: Path) -> dict[str, Any]:
    status = _status_rows(
        _git(repository, "status", "--porcelain=v1", "--untracked-files=all")
    )
    untracked = _untracked_manifest(repository, status)
    return {
        "repository_realpath": str(repository),
        "commit": _git(repository, "rev-parse", "HEAD").decode("ascii").strip(),
        "tree": _git(repository, "rev-parse", "HEAD^{tree}").decode("ascii").strip(),
        "dirty_path_count": len(status),
        "dirty_path_manifest_sha256": sha256(_status_manifest(status)),
        "tracked_binary_diff_sha256": sha256(
            _git(repository, "diff", "--binary", "HEAD", "--")
        ),
        "untracked_regular_file_count": untracked["file_count"],
        "untracked_regular_file_manifest_identity": untracked["identity_sha256"],
    }


def observe_currentness(repository: Path) -> dict[str, Any]:
    try:
        resolved = repository.resolve(strict=True)
    except OSError as error:
        raise ProtectedPrimaryError(
            f"protected-primary checkout is absent: {error}"
        ) from error
    if not resolved.is_dir():
        raise ProtectedPrimaryError("protected-primary checkout is not a directory")
    first = _observe_once(resolved)
    second = _observe_once(resolved)
    if first != second:
        raise ProtectedPrimaryError("protected-primary changed while observed")
    if (
        SHA1.fullmatch(str(first["commit"])) is None
        or SHA1.fullmatch(str(first["tree"])) is None
    ):
        raise ProtectedPrimaryError("protected-primary Git identity is invalid")
    return first


def verify_currentness(
    binding: Mapping[str, Any], repository: Path | None = None
) -> dict[str, Any]:
    validated = validate_binding(binding)
    bound_repository = Path(str(validated["repository_realpath"]))
    if repository is not None:
        try:
            requested = repository.resolve(strict=True)
        except OSError as error:
            raise ProtectedPrimaryError(
                f"protected-primary checkout is absent: {error}"
            ) from error
        if requested != bound_repository:
            raise ProtectedPrimaryError(
                "protected-primary checkout differs from its binding"
            )
    observed = observe_currentness(bound_repository)
    expected = {field: validated[field] for field in CURRENTNESS_FIELDS}
    if observed != expected:
        raise ProtectedPrimaryError("protected-primary currentness differs")
    return observed
