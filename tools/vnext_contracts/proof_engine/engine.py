from __future__ import annotations

import hashlib
import fcntl
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
import threading
import time
import uuid
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping, Sequence, cast


ENGINE_SCHEMA = "maestro.vnext.proof-engine.v1"
CHECKPOINT_SCHEMA = "maestro.vnext.proof-phase-checkpoint.v1"
CHECKPOINT_BINDING_SCHEMA = "maestro.vnext.proof-phase-checkpoint-binding.v1"
PERFORMANCE_SCHEMA = "maestro.vnext.proof-performance-event.v1"
PUBLICATION_SCHEMA = "maestro.vnext.proof-publication.v1"
POINTER_SCHEMA = "maestro.vnext.proof-publication-pointer.v1"
RUN_MARKER_SCHEMA = "maestro.vnext.proof-run-marker.v1"
COMPLETION_MARKER_SCHEMA = "maestro.vnext.proof-run-completion.v1"

_NAME = re.compile(r"[a-z][a-z0-9-]{0,63}")
_PLACEHOLDER = re.compile(
    r"\{(?:phase_root|phase_temp|input:[a-z][a-z0-9-]{0,63}|dependency:[a-z][a-z0-9-]{0,63})\}"
)
_RESERVED_ENVIRONMENT = frozenset({"LANG", "LC_ALL", "TZ", "PYTHONDONTWRITEBYTECODE", "TMPDIR", "TMP", "TEMP"})
_BASE_ENVIRONMENT = {
    "LANG": "C",
    "LC_ALL": "C",
    "PYTHONDONTWRITEBYTECODE": "1",
    "TZ": "UTC",
}
_CHECKPOINT_KEYS = frozenset(
    {
        "command_receipts",
        "dependency_outputs",
        "identity",
        "input_identities",
        "integrity",
        "output_manifest",
        "phase",
        "phase_spec_identity",
        "schema_version",
        "tool_identities",
    }
)


class EngineError(RuntimeError):
    pass


class PlanError(EngineError):
    pass


class CacheCorruptionError(EngineError):
    pass


class InputMutationError(EngineError):
    pass


class PublicationError(EngineError):
    pass


@dataclass(frozen=True)
class InputBinding:
    name: str
    kind: str
    path: Path | None = None
    value: str | None = None
    path_identity: str = "resolved"

    @classmethod
    def file(cls, name: str, path: Path, *, path_identity: str = "resolved") -> InputBinding:
        return cls(name=name, kind="file", path=path, path_identity=path_identity)

    @classmethod
    def tree(cls, name: str, path: Path, *, path_identity: str = "resolved") -> InputBinding:
        return cls(name=name, kind="tree", path=path, path_identity=path_identity)

    @classmethod
    def symlink_tree(
        cls, name: str, path: Path, *, path_identity: str = "resolved"
    ) -> InputBinding:
        return cls(name=name, kind="symlink_tree", path=path, path_identity=path_identity)

    @classmethod
    def literal(cls, name: str, value: str) -> InputBinding:
        return cls(name=name, kind="literal", value=value, path_identity="none")


@dataclass(frozen=True)
class ToolSpec:
    name: str
    path: Path
    probe_args: tuple[str, ...] = ()


@dataclass(frozen=True)
class CommandSpec:
    tool: str
    args: tuple[str, ...] = ()
    cwd: str = "{phase_root}"
    environment: tuple[tuple[str, str], ...] = ()
    expected_exit_code: int = 0
    label: str = "command"


@dataclass(frozen=True)
class PhaseSpec:
    name: str
    commands: tuple[CommandSpec, ...]
    inputs: tuple[str, ...] = ()
    dependencies: tuple[str, ...] = ()
    cache_mode: str = "run"


@dataclass(frozen=True)
class PublishedOutput:
    phase: str
    source: str
    destination: str


@dataclass(frozen=True)
class PublicationSpec:
    release_root: Path
    pointer_path: Path
    outputs: tuple[PublishedOutput, ...]


@dataclass(frozen=True)
class ProofPlan:
    inputs: tuple[InputBinding, ...]
    tools: tuple[ToolSpec, ...]
    phases: tuple[PhaseSpec, ...]
    environment: tuple[tuple[str, str], ...] = ()
    publication: PublicationSpec | None = None


@dataclass(frozen=True)
class PhaseResult:
    name: str
    identity: str
    output_identity: str
    output_root: Path
    cache_status: str
    command_receipts: tuple[dict[str, Any], ...]


@dataclass(frozen=True)
class ProofRunResult:
    plan_identity: str
    run_token: str
    phases: tuple[PhaseResult, ...]
    publication_identity: str | None


def _canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("ascii")


def _digest(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def _require_name(value: str, label: str) -> None:
    if _NAME.fullmatch(value) is None:
        raise PlanError(f"{label} must match {_NAME.pattern}: {value!r}")


def _relative_path(value: str, label: str, *, allow_dot: bool = False) -> PurePosixPath:
    if "\\" in value:
        raise PlanError(f"{label} must use portable forward slashes: {value!r}")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or (not allow_dot and value in {"", "."}):
        raise PlanError(f"{label} must be a safe relative path: {value!r}")
    return path


def _paths_overlap(left: Path, right: Path) -> bool:
    left = left.resolve(strict=False)
    right = right.resolve(strict=False)
    return left == right or left in right.parents or right in left.parents


def _file_row(path: Path, relative: str) -> dict[str, Any]:
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode):
        raise EngineError(f"proof inputs and outputs must not contain symlinks: {path}")
    if stat.S_ISREG(info.st_mode):
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(path, flags)
        except OSError as error:
            raise InputMutationError(f"proof file changed while opening: {path}") from error
        try:
            opened = os.fstat(descriptor)
            if (
                not stat.S_ISREG(opened.st_mode)
                or (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino)
            ):
                raise InputMutationError(f"proof file binding changed while opening: {path}")
            chunks = []
            while chunk := os.read(descriptor, 1024 * 1024):
                chunks.append(chunk)
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        current = path.lstat()
        stable = (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
        if stable != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        ) or stable != (
            current.st_dev,
            current.st_ino,
            current.st_size,
            current.st_mtime_ns,
            current.st_ctime_ns,
        ):
            raise InputMutationError(f"proof file changed while hashing: {path}")
        data = b"".join(chunks)
        if len(data) != opened.st_size:
            raise InputMutationError(f"proof file length changed while hashing: {path}")
        return {
            "byte_length": len(data),
            "executable": bool(info.st_mode & 0o111),
            "path": relative,
            "sha256": _digest(data),
            "type": "file",
        }
    if stat.S_ISDIR(info.st_mode):
        return {"path": relative, "type": "directory"}
    raise EngineError(f"proof inputs and outputs must contain only regular files and directories: {path}")


def _safe_symlink_row(path: Path, root: Path, relative: str) -> dict[str, Any]:
    target = os.readlink(path)
    try:
        resolved = path.resolve(strict=True)
        resolved_relative = resolved.relative_to(root.resolve(strict=True)).as_posix()
    except (OSError, RuntimeError, ValueError) as error:
        raise EngineError(f"proof input symlink escapes or cycles outside its root: {path}") from error
    return {
        "path": relative,
        "resolved_path": resolved_relative,
        "target": target,
        "type": "symlink",
    }


def _path_manifest(path: Path, kind: str) -> dict[str, Any]:
    if not path.exists() and not path.is_symlink():
        raise EngineError(f"bound {kind} does not exist: {path}")
    if path.is_symlink():
        raise EngineError(f"bound {kind} must not be a symlink: {path}")
    if kind == "file":
        if not path.is_file():
            raise EngineError(f"bound file is not a regular file: {path}")
        rows = [_file_row(path, ".")]
    elif kind in {"tree", "symlink_tree"}:
        if not path.is_dir():
            raise EngineError(f"bound tree is not a directory: {path}")
        rows = []
        for child in sorted(path.rglob("*"), key=lambda item: item.relative_to(path).as_posix()):
            relative = child.relative_to(path).as_posix()
            if child.is_symlink() and kind == "symlink_tree":
                rows.append(_safe_symlink_row(child, path, relative))
            else:
                rows.append(_file_row(child, relative))
    else:
        raise EngineError(f"unsupported path manifest kind: {kind}")
    return {"identity": _digest(_canonical_bytes(rows)), "rows": rows}


def _output_manifest(path: Path) -> dict[str, Any]:
    return _path_manifest(path, "tree")


def _copy_tree(source: Path, destination: Path, *, allow_internal_symlinks: bool = False) -> None:
    kind = "symlink_tree" if allow_internal_symlinks else "tree"
    before = _path_manifest(source, kind)
    if destination.exists() or destination.is_symlink():
        raise EngineError(f"copy destination already exists: {destination}")
    shutil.copytree(source, destination, symlinks=allow_internal_symlinks)
    if _path_manifest(source, kind) != before or _path_manifest(destination, kind) != before:
        shutil.rmtree(destination, ignore_errors=True)
        raise InputMutationError(f"proof tree changed while copying: {source}")


def _copy_file(source: Path, destination: Path) -> None:
    before = _file_row(source, ".")
    shutil.copy2(source, destination)
    if _file_row(source, ".") != before or _file_row(destination, ".") != before:
        destination.unlink(missing_ok=True)
        raise InputMutationError(f"proof file changed while copying: {source}")


def _freeze_tree(root: Path, *, allow_internal_symlinks: bool = False) -> None:
    for child in sorted(root.rglob("*"), reverse=True):
        info = child.lstat()
        if stat.S_ISLNK(info.st_mode):
            if not allow_internal_symlinks:
                raise PublicationError(f"immutable proof release contains a symlink: {child}")
            _safe_symlink_row(child, root, child.relative_to(root).as_posix())
            continue
        if stat.S_ISREG(info.st_mode):
            child.chmod(0o555 if info.st_mode & 0o111 else 0o444)
            with child.open("rb") as handle:
                os.fsync(handle.fileno())
        elif stat.S_ISDIR(info.st_mode):
            child.chmod(0o555)
            _fsync_directory(child)
        else:
            raise PublicationError(f"immutable proof release contains an unsafe entry: {child}")
    root.chmod(0o555)
    _fsync_directory(root)


def _require_frozen_tree(root: Path, *, allow_internal_symlinks: bool = False) -> None:
    for child in [root, *root.rglob("*")]:
        info = child.lstat()
        if stat.S_ISLNK(info.st_mode):
            if not allow_internal_symlinks:
                raise PublicationError(f"proof release is not immutable: {child}")
            _safe_symlink_row(child, root, child.relative_to(root).as_posix())
            continue
        if info.st_mode & 0o222:
            raise PublicationError(f"proof release is not immutable: {child}")


def _remove_frozen_tree(root: Path) -> None:
    for child in [root, *root.rglob("*")]:
        if child.is_symlink():
            child.unlink(missing_ok=True)
        elif child.is_dir():
            child.chmod(0o755)
        elif child.is_file():
            child.chmod(0o644)
    shutil.rmtree(root)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{uuid.uuid4().hex}")
    try:
        with temporary.open("xb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        _fsync_directory(path.parent)
    finally:
        if temporary.exists():
            temporary.unlink()


def _write_once(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{uuid.uuid4().hex}")
    try:
        with temporary.open("xb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError as error:
            raise PlanError(f"immutable proof marker already exists: {path}") from error
        _fsync_directory(path.parent)
    finally:
        if temporary.exists():
            temporary.unlink()


class _PerformanceRecorder:
    def __init__(self, path: Path | None) -> None:
        self.path = path
        self.lock = threading.Lock()

    def emit(self, **value: object) -> None:
        if self.path is None:
            return
        row = {"schema_version": PERFORMANCE_SCHEMA, **value}
        data = _canonical_bytes(row)
        with self.lock:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(data)
                handle.flush()


class ProofEngine:
    def execute(
        self,
        plan: ProofPlan,
        *,
        run_root: Path,
        cache_root: Path,
        run_token: str | None = None,
        max_workers: int = 1,
        performance_log: Path | None = None,
    ) -> ProofRunResult:
        if max_workers < 1:
            raise PlanError("max_workers must be at least one")
        run_token = run_token or uuid.uuid4().hex
        if not run_token.strip() or len(run_token.encode("utf-8")) > 256:
            raise PlanError("run_token must contain between 1 and 256 UTF-8 bytes")
        self._validate_plan(plan, run_root, cache_root, performance_log)
        inputs = {binding.name: binding for binding in plan.inputs}
        tools = {tool.name: tool for tool in plan.tools}
        phases = {phase.name: phase for phase in plan.phases}
        environment = dict(plan.environment)
        input_identities = {name: self._input_identity(binding) for name, binding in inputs.items()}
        tool_identities = {
            name: self._tool_identity(tool, environment) for name, tool in tools.items()
        }
        plan_value = self._plan_value(plan, input_identities, tool_identities)
        plan_identity = _digest(_canonical_bytes(plan_value))
        cache_root.mkdir(parents=True, exist_ok=True)
        lock_descriptor = self._acquire_run_lock(cache_root, plan_identity, run_token)
        try:
            return self._execute_locked(
                plan,
                inputs,
                tools,
                phases,
                environment,
                input_identities,
                tool_identities,
                plan_value,
                plan_identity,
                run_root,
                cache_root,
                run_token,
                max_workers,
                performance_log,
            )
        finally:
            fcntl.flock(lock_descriptor, fcntl.LOCK_UN)
            os.close(lock_descriptor)

    def _execute_locked(
        self,
        plan: ProofPlan,
        inputs: Mapping[str, InputBinding],
        tools: Mapping[str, ToolSpec],
        phases: Mapping[str, PhaseSpec],
        environment: Mapping[str, str],
        input_identities: Mapping[str, dict[str, Any]],
        tool_identities: Mapping[str, dict[str, Any]],
        plan_value: Mapping[str, Any],
        plan_identity: str,
        run_root: Path,
        cache_root: Path,
        run_token: str,
        max_workers: int,
        performance_log: Path | None,
    ) -> ProofRunResult:
        if {
            name: self._input_identity(binding) for name, binding in inputs.items()
        } != input_identities or {
            name: self._tool_identity(tool, environment) for name, tool in tools.items()
        } != tool_identities:
            raise InputMutationError("proof inputs or tools changed before run lock acquisition")
        initial_pointer_state = self._prepare_run_root(
            run_root,
            cache_root,
            run_token,
            plan_identity,
            plan.publication.pointer_path if plan.publication is not None else None,
        )
        pinned_inputs, pinned_tools = self._prepare_bindings(
            run_root,
            inputs,
            tools,
            input_identities,
            tool_identities,
            environment,
        )
        recorder = _PerformanceRecorder(performance_log)
        results: dict[str, PhaseResult] = {}
        pending = set(phases)
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            while pending:
                ready = sorted(
                    name for name in pending if set(phases[name].dependencies) <= set(results)
                )
                if not ready:
                    raise PlanError("phase dependencies contain a cycle")
                futures = {
                    executor.submit(
                        self._execute_phase,
                        phases[name],
                        pinned_inputs,
                        pinned_tools,
                        environment,
                        input_identities,
                        tool_identities,
                        results,
                        run_root,
                        cache_root,
                        run_token,
                        recorder,
                    ): name
                    for name in ready
                }
                failures: list[BaseException] = []
                completed: dict[str, PhaseResult] = {}
                for future in as_completed(futures):
                    name = futures[future]
                    try:
                        completed[name] = future.result()
                    except BaseException as error:
                        failures.append(error)
                if failures:
                    raise failures[0]
                for name in ready:
                    results[name] = completed[name]
                    pending.remove(name)
        if {
            name: self._input_identity(binding) for name, binding in inputs.items()
        } != input_identities:
            raise InputMutationError("a proof command changed the bound input closure")
        if {
            name: self._tool_identity(tool, environment) for name, tool in tools.items()
        } != tool_identities:
            raise InputMutationError("a proof command changed the bound tool closure")
        self._verify_pinned_bindings(
            pinned_inputs,
            pinned_tools,
            input_identities,
            tool_identities,
            environment,
        )
        ordered = tuple(results[phase.name] for phase in plan.phases)
        for phase in ordered:
            if _output_manifest(phase.output_root)["identity"] != phase.output_identity:
                raise InputMutationError(
                    f"a proof command changed completed phase output {phase.name}"
                )
        publication_identity = (
            self._publish(
                plan.publication,
                plan_identity,
                plan_value,
                run_token,
                ordered,
                initial_pointer_state,
            )
            if plan.publication is not None
            else None
        )
        self._complete_run(
            cache_root,
            plan_identity,
            run_token,
            ordered,
            publication_identity,
        )
        return ProofRunResult(
            plan_identity=plan_identity,
            run_token=run_token,
            phases=ordered,
            publication_identity=publication_identity,
        )

    @staticmethod
    def _acquire_run_lock(cache_root: Path, plan_identity: str, run_token: str) -> int:
        lock_identity = _digest(
            _canonical_bytes(
                {
                    "plan_identity": plan_identity,
                    "run_token": run_token,
                    "schema_version": RUN_MARKER_SCHEMA,
                }
            )
        )
        locks = cache_root / "locks"
        locks.mkdir(parents=True, exist_ok=True)
        path = locks / f"run-{lock_identity.removeprefix('sha256:')}.lock"
        descriptor = os.open(
            path,
            os.O_CREAT
            | os.O_RDWR
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX)
        except BaseException:
            os.close(descriptor)
            raise
        return descriptor

    def _prepare_bindings(
        self,
        run_root: Path,
        inputs: Mapping[str, InputBinding],
        tools: Mapping[str, ToolSpec],
        input_identities: Mapping[str, dict[str, Any]],
        tool_identities: Mapping[str, dict[str, Any]],
        environment: Mapping[str, str],
    ) -> tuple[dict[str, InputBinding], dict[str, ToolSpec]]:
        target = run_root / "bindings"
        expected = {
            "input_identities": dict(input_identities),
            "schema_version": "maestro.vnext.proof-run-bindings.v1",
            "tool_identities": dict(tool_identities),
        }
        if not target.exists() and not target.is_symlink():
            temporary = Path(tempfile.mkdtemp(prefix=".bindings-", dir=run_root))
            try:
                for name, binding in inputs.items():
                    if binding.path is None:
                        continue
                    destination = temporary / "inputs" / name / binding.kind
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    if binding.kind == "file":
                        _copy_file(binding.path.resolve(strict=True), destination)
                    else:
                        _copy_tree(
                            binding.path.resolve(strict=True),
                            destination,
                            allow_internal_symlinks=binding.kind == "symlink_tree",
                        )
                for name, tool in tools.items():
                    destination = temporary / "tools" / name / "executable"
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    _copy_file(tool.path.resolve(strict=True), destination)
                    destination.chmod(0o555)
                (temporary / "bindings.json").write_bytes(_canonical_bytes(expected))
                _freeze_tree(temporary, allow_internal_symlinks=True)
                os.rename(temporary, target)
                _fsync_directory(run_root)
            finally:
                if temporary.exists():
                    _remove_frozen_tree(temporary)
        if target.is_symlink() or not target.is_dir():
            raise InputMutationError(f"proof binding root is absent or unsafe: {target}")
        try:
            _require_frozen_tree(target, allow_internal_symlinks=True)
            recorded = json.loads((target / "bindings.json").read_text(encoding="ascii"))
        except (OSError, UnicodeError, json.JSONDecodeError, EngineError) as error:
            raise InputMutationError(f"proof binding root is unreadable: {target}") from error
        if recorded != expected:
            raise InputMutationError("proof binding identities differ from this run")
        pinned_inputs = {
            name: (
                binding
                if binding.path is None
                else InputBinding(
                    name=name,
                    kind=binding.kind,
                    path=target / "inputs" / name / binding.kind,
                    path_identity=binding.path_identity,
                )
            )
            for name, binding in inputs.items()
        }
        pinned_tools = {
            name: ToolSpec(
                name=name,
                path=target / "tools" / name / "executable",
                probe_args=tool.probe_args,
            )
            for name, tool in tools.items()
        }
        self._verify_pinned_bindings(
            pinned_inputs,
            pinned_tools,
            input_identities,
            tool_identities,
            environment,
        )
        return pinned_inputs, pinned_tools

    def _verify_pinned_bindings(
        self,
        inputs: Mapping[str, InputBinding],
        tools: Mapping[str, ToolSpec],
        input_identities: Mapping[str, dict[str, Any]],
        tool_identities: Mapping[str, dict[str, Any]],
        environment: Mapping[str, str],
    ) -> None:
        for name, binding in inputs.items():
            if binding.path is None:
                continue
            expected = input_identities[name]["value"]["manifest"]
            if _path_manifest(binding.path, binding.kind) != expected:
                raise InputMutationError(f"pinned proof input changed: {name}")
        for name, tool in tools.items():
            expected = tool_identities[name]["value"]
            row = _file_row(tool.path, ".")
            if row["byte_length"] != expected["byte_length"] or row["sha256"] != expected["sha256"]:
                raise InputMutationError(f"pinned proof tool changed: {name}")
            if tool.probe_args:
                result = subprocess.run(
                    [str(tool.path), *tool.probe_args],
                    capture_output=True,
                    check=False,
                    env={**_BASE_ENVIRONMENT, **environment},
                )
                probe = {
                    "exit_code": result.returncode,
                    "stderr": {
                        "byte_length": len(result.stderr),
                        "sha256": _digest(result.stderr),
                    },
                    "stdout": {
                        "byte_length": len(result.stdout),
                        "sha256": _digest(result.stdout),
                    },
                }
                if probe != expected.get("probe"):
                    raise InputMutationError(f"pinned proof tool probe changed: {name}")

    def _validate_plan(
        self,
        plan: ProofPlan,
        run_root: Path,
        cache_root: Path,
        performance_log: Path | None,
    ) -> None:
        input_names: set[str] = set()
        input_kinds: dict[str, str] = {}
        input_paths: list[Path] = []
        for binding in plan.inputs:
            _require_name(binding.name, "input name")
            if binding.name in input_names:
                raise PlanError(f"duplicate input name: {binding.name}")
            input_names.add(binding.name)
            input_kinds[binding.name] = binding.kind
            if binding.kind not in {"file", "tree", "symlink_tree", "literal"}:
                raise PlanError(f"unsupported input kind: {binding.kind}")
            if binding.path_identity not in {"resolved", "content", "none"}:
                raise PlanError(f"unsupported input path identity: {binding.path_identity}")
            if binding.kind == "literal":
                if binding.value is None or binding.path is not None or binding.path_identity != "none":
                    raise PlanError(f"literal input {binding.name} has an invalid representation")
            else:
                if binding.path is None or binding.value is not None or binding.path_identity == "none":
                    raise PlanError(f"path input {binding.name} has an invalid representation")
                input_paths.append(binding.path)
        tool_names: set[str] = set()
        for tool in plan.tools:
            _require_name(tool.name, "tool name")
            if tool.name in tool_names:
                raise PlanError(f"duplicate tool name: {tool.name}")
            tool_names.add(tool.name)
        environment_keys: set[str] = set()
        for key, _ in plan.environment:
            if key in environment_keys or key in _RESERVED_ENVIRONMENT:
                raise PlanError(f"duplicate or engine-reserved environment key: {key}")
            environment_keys.add(key)
        phase_names: set[str] = set()
        for phase in plan.phases:
            _require_name(phase.name, "phase name")
            if phase.name in phase_names:
                raise PlanError(f"duplicate phase name: {phase.name}")
            phase_names.add(phase.name)
            if phase.cache_mode not in {"disabled", "run", "content"}:
                raise PlanError(f"unsupported cache mode for {phase.name}: {phase.cache_mode}")
            if len(set(phase.inputs)) != len(phase.inputs) or not set(phase.inputs) <= input_names:
                raise PlanError(f"phase {phase.name} has duplicate or unknown inputs")
            if len(set(phase.dependencies)) != len(phase.dependencies):
                raise PlanError(f"phase {phase.name} has duplicate dependencies")
            for command in phase.commands:
                if command.tool not in tool_names:
                    raise PlanError(f"phase {phase.name} references unknown tool {command.tool}")
                if len({key for key, _ in command.environment}) != len(command.environment):
                    raise PlanError(f"phase {phase.name} command has duplicate environment keys")
                if any(key in _RESERVED_ENVIRONMENT for key, _ in command.environment):
                    raise PlanError(
                        f"phase {phase.name} command replaces an engine-reserved environment key"
                    )
                for template in [
                    *command.args,
                    command.cwd,
                    *(value for _, value in command.environment),
                ]:
                    self._validate_template_bindings(template, phase, input_kinds)
        for phase in plan.phases:
            if phase.name in phase.dependencies or not set(phase.dependencies) <= phase_names:
                raise PlanError(f"phase {phase.name} has an invalid dependency")
        self._validate_acyclic(plan.phases)
        for mutable in [run_root, cache_root, *(tuple([performance_log]) if performance_log else ())]:
            if any(_paths_overlap(mutable, source) for source in input_paths):
                raise PlanError(f"engine-owned path overlaps a bound input: {mutable}")
        if _paths_overlap(run_root, cache_root):
            raise PlanError("run_root and cache_root must not overlap")
        if performance_log is not None and _paths_overlap(performance_log, cache_root):
            raise PlanError("performance_log must remain outside the canonical cache")
        if performance_log is not None and _paths_overlap(performance_log, run_root):
            raise PlanError("performance_log must remain outside the canonical run root")
        if plan.publication is not None:
            self._validate_publication(plan.publication, phase_names)
            if _paths_overlap(plan.publication.release_root, run_root) or _paths_overlap(
                plan.publication.release_root, cache_root
            ):
                raise PlanError("publication release_root must not overlap run or cache roots")
            if _paths_overlap(plan.publication.pointer_path, run_root) or _paths_overlap(
                plan.publication.pointer_path, cache_root
            ):
                raise PlanError("publication pointer must not overlap run or cache roots")
            if (
                plan.publication.release_root / "objects"
            ).resolve(strict=False) in plan.publication.pointer_path.resolve(strict=False).parents:
                raise PlanError("publication pointer must remain outside immutable release objects")
            for mutable in [plan.publication.release_root, plan.publication.pointer_path]:
                if any(_paths_overlap(mutable, source) for source in input_paths):
                    raise PlanError(f"publication path overlaps a bound input: {mutable}")
            if performance_log is not None and (
                _paths_overlap(performance_log, plan.publication.release_root)
                or _paths_overlap(performance_log, plan.publication.pointer_path)
            ):
                raise PlanError("performance_log must remain outside canonical publication paths")

    @staticmethod
    def _validate_acyclic(phases: Sequence[PhaseSpec]) -> None:
        dependencies = {phase.name: set(phase.dependencies) for phase in phases}
        remaining = set(dependencies)
        completed: set[str] = set()
        while remaining:
            ready = {name for name in remaining if dependencies[name] <= completed}
            if not ready:
                raise PlanError("phase dependencies contain a cycle")
            completed |= ready
            remaining -= ready

    @staticmethod
    def _validate_template_bindings(
        template: str, phase: PhaseSpec, input_kinds: Mapping[str, str]
    ) -> None:
        for match in _PLACEHOLDER.finditer(template):
            key = match.group(0)[1:-1]
            if key.startswith("input:"):
                name = key.removeprefix("input:")
                if name not in phase.inputs:
                    raise PlanError(
                        f"phase {phase.name} command references undeclared input {name}"
                    )
                if input_kinds[name] == "literal":
                    raise PlanError(
                        f"phase {phase.name} command cannot render literal input {name} as a path"
                    )
            elif key.startswith("dependency:"):
                name = key.removeprefix("dependency:")
                if name not in phase.dependencies:
                    raise PlanError(
                        f"phase {phase.name} command references undeclared dependency {name}"
                    )
        residual = _PLACEHOLDER.sub("", template)
        if any(
            prefix in residual
            for prefix in (
                "{input:",
                "{dependency:",
                "{phase_root}",
                "{phase_temp}",
                "{run_root}",
            )
        ):
            raise PlanError(f"phase {phase.name} command has an invalid proof placeholder")

    @staticmethod
    def _validate_publication(publication: PublicationSpec, phase_names: set[str]) -> None:
        destinations: list[PurePosixPath] = []
        if not publication.outputs:
            raise PlanError("publication requires at least one output")
        for output in publication.outputs:
            if output.phase not in phase_names:
                raise PlanError(f"publication references unknown phase: {output.phase}")
            _relative_path(output.source, "publication source", allow_dot=True)
            destination = _relative_path(output.destination, "publication destination")
            if any(
                destination == existing
                or destination in existing.parents
                or existing in destination.parents
                for existing in destinations
            ):
                raise PlanError(f"publication destinations overlap: {output.destination}")
            destinations.append(destination)

    @staticmethod
    def _input_identity(binding: InputBinding) -> dict[str, Any]:
        if binding.kind == "literal":
            value: dict[str, Any] = {
                "kind": "literal",
                "name": binding.name,
                "value": binding.value,
            }
        else:
            assert binding.path is not None
            resolved = binding.path.resolve(strict=True)
            value = {
                "kind": binding.kind,
                "manifest": _path_manifest(binding.path, binding.kind),
                "name": binding.name,
                "path": str(resolved) if binding.path_identity == "resolved" else None,
                "path_identity": binding.path_identity,
            }
        return {"identity": _digest(_canonical_bytes(value)), "value": value}

    @staticmethod
    def _tool_identity(tool: ToolSpec, environment: Mapping[str, str]) -> dict[str, Any]:
        resolved = tool.path.resolve(strict=True)
        if not resolved.is_file() or not os.access(resolved, os.X_OK):
            raise EngineError(f"proof tool is not an executable regular file: {tool.path}")
        row = _file_row(resolved, ".")
        data = resolved.read_bytes()
        if _file_row(resolved, ".") != row or _digest(data) != row["sha256"]:
            raise InputMutationError(f"proof tool changed while binding: {resolved}")
        value: dict[str, Any] = {
            "byte_length": len(data),
            "name": tool.name,
            "probe_args": list(tool.probe_args),
            "resolved_path": str(resolved),
            "sha256": _digest(data),
        }
        if tool.probe_args:
            probe_environment = {**_BASE_ENVIRONMENT, **environment}
            result = subprocess.run(
                [str(resolved), *tool.probe_args],
                capture_output=True,
                check=False,
                env=probe_environment,
            )
            if result.returncode != 0:
                raise EngineError(f"tool identity probe failed for {tool.name}")
            value["probe"] = {
                "exit_code": result.returncode,
                "stderr": {"byte_length": len(result.stderr), "sha256": _digest(result.stderr)},
                "stdout": {"byte_length": len(result.stdout), "sha256": _digest(result.stdout)},
            }
        return {"identity": _digest(_canonical_bytes(value)), "value": value}

    @staticmethod
    def _command_value(command: CommandSpec) -> dict[str, Any]:
        return {
            "args": list(command.args),
            "cwd": command.cwd,
            "environment": [[key, value] for key, value in command.environment],
            "expected_exit_code": command.expected_exit_code,
            "label": command.label,
            "tool": command.tool,
        }

    def _phase_value(self, phase: PhaseSpec, environment: Sequence[tuple[str, str]]) -> dict[str, Any]:
        return {
            "cache_mode": phase.cache_mode,
            "commands": [self._command_value(command) for command in phase.commands],
            "dependencies": list(phase.dependencies),
            "engine_environment": [[key, value] for key, value in environment],
            "inputs": list(phase.inputs),
            "name": phase.name,
        }

    def _plan_value(
        self,
        plan: ProofPlan,
        input_identities: Mapping[str, dict[str, Any]],
        tool_identities: Mapping[str, dict[str, Any]],
    ) -> dict[str, Any]:
        publication = None
        if plan.publication is not None:
            publication = {
                "outputs": [
                    {
                        "destination": output.destination,
                        "phase": output.phase,
                        "source": output.source,
                    }
                    for output in plan.publication.outputs
                ],
                "pointer_path": str(plan.publication.pointer_path.resolve(strict=False)),
                "release_root": str(plan.publication.release_root.resolve(strict=False)),
            }
        return {
            "environment": [[key, value] for key, value in plan.environment],
            "inputs": [input_identities[binding.name] for binding in plan.inputs],
            "phases": [self._phase_value(phase, plan.environment) for phase in plan.phases],
            "publication": publication,
            "schema_version": ENGINE_SCHEMA,
            "tools": [tool_identities[tool.name] for tool in plan.tools],
        }

    @staticmethod
    def _prepare_run_root(
        run_root: Path,
        cache_root: Path,
        run_token: str,
        plan_identity: str,
        publication_pointer: Path | None,
    ) -> dict[str, Any] | None:
        marker = run_root / ".maestro-proof-run.v1.json"
        global_completion = ProofEngine._completion_registry_path(
            cache_root, plan_identity, run_token
        )
        common = {
            "plan_identity": plan_identity,
            "run_token": run_token,
            "schema_version": RUN_MARKER_SCHEMA,
        }
        if run_root.exists():
            if not run_root.is_dir() or not marker.is_file():
                raise PlanError(f"existing run_root is not an owned proof run: {run_root}")
            try:
                current = json.loads(marker.read_text(encoding="ascii"))
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise PlanError(f"proof run marker is unreadable: {marker}") from error
            if (
                not isinstance(current, dict)
                or {key: current.get(key) for key in common} != common
                or set(current) != {*common, "publication_pointer_state"}
            ):
                raise PlanError("existing run_root belongs to another proof plan or run token")
            raw_pointer_state = current["publication_pointer_state"]
            if raw_pointer_state is not None and not isinstance(raw_pointer_state, dict):
                raise PlanError("proof run marker has an invalid publication pointer binding")
            pointer_state = cast(dict[str, Any] | None, raw_pointer_state)
        else:
            pointer_state = (
                ProofEngine._publication_pointer_state(publication_pointer)
                if publication_pointer is not None
                else None
            )
            run_root.mkdir(parents=True)
            _write_atomic(
                marker,
                _canonical_bytes({**common, "publication_pointer_state": pointer_state}),
            )
        if global_completion.exists() or global_completion.is_symlink():
            raise PlanError("completed proof run tokens cannot be resumed or replayed")
        (run_root / "phases").mkdir(exist_ok=True)
        return pointer_state

    @staticmethod
    def _publication_pointer_state(path: Path) -> dict[str, Any]:
        if not path.exists() and not path.is_symlink():
            return {"state": "absent"}
        if path.is_symlink() or not path.is_file():
            raise PublicationError(f"proof publication pointer is unsafe: {path}")
        row = _file_row(path, ".")
        return {
            "byte_length": row["byte_length"],
            "executable": row["executable"],
            "sha256": row["sha256"],
            "state": "present",
        }

    @staticmethod
    def _completion_registry_path(
        cache_root: Path, plan_identity: str, run_token: str
    ) -> Path:
        identity = _digest(
            _canonical_bytes(
                {
                    "plan_identity": plan_identity,
                    "run_token": run_token,
                    "schema_version": COMPLETION_MARKER_SCHEMA,
                }
            )
        )
        return cache_root / "completed-runs" / f"{identity.removeprefix('sha256:')}.json"

    @staticmethod
    def _complete_run(
        cache_root: Path,
        plan_identity: str,
        run_token: str,
        phases: Sequence[PhaseResult],
        publication_identity: str | None,
    ) -> None:
        value = {
            "phase_identities": {phase.name: phase.identity for phase in phases},
            "plan_identity": plan_identity,
            "publication_identity": publication_identity,
            "run_token": run_token,
            "schema_version": COMPLETION_MARKER_SCHEMA,
        }
        data = _canonical_bytes(value)
        _write_once(
            ProofEngine._completion_registry_path(cache_root, plan_identity, run_token), data
        )

    def _execute_phase(
        self,
        phase: PhaseSpec,
        inputs: Mapping[str, InputBinding],
        tools: Mapping[str, ToolSpec],
        environment: Mapping[str, str],
        input_identities: Mapping[str, dict[str, Any]],
        tool_identities: Mapping[str, dict[str, Any]],
        dependencies: Mapping[str, PhaseResult],
        run_root: Path,
        cache_root: Path,
        run_token: str,
        recorder: _PerformanceRecorder,
    ) -> PhaseResult:
        started = time.monotonic_ns()
        phase_tools = sorted({command.tool for command in phase.commands})
        phase_value = self._phase_value(phase, tuple(environment.items()))
        phase_spec_identity = _digest(_canonical_bytes(phase_value))
        identity_value: dict[str, Any] = {
            "dependencies": {
                name: dependencies[name].output_identity for name in phase.dependencies
            },
            "engine": ENGINE_SCHEMA,
            "inputs": {name: input_identities[name]["identity"] for name in phase.inputs},
            "phase_spec_identity": phase_spec_identity,
            "tools": {name: tool_identities[name]["identity"] for name in phase_tools},
        }
        if phase.cache_mode == "run":
            identity_value["run_token"] = run_token
        phase_identity = _digest(_canonical_bytes(identity_value))
        phase_root = run_root / "phases" / phase.name / "output"
        phase_parent = phase_root.parent
        phase_temp = phase_parent / "tmp"
        if phase_parent.exists():
            _remove_frozen_tree(phase_parent)
        phase_parent.mkdir(parents=True)
        checkpoint = None
        if phase.cache_mode != "disabled":
            checkpoint = self._load_checkpoint(cache_root, phase_identity)
        if checkpoint is not None:
            self._require_checkpoint_binding(
                checkpoint,
                phase,
                phase_spec_identity,
                identity_value["dependencies"],
                identity_value["inputs"],
                identity_value["tools"],
            )
            _copy_tree(self._checkpoint_root(cache_root, phase_identity) / "payload", phase_root)
            _freeze_tree(phase_root)
            elapsed = (time.monotonic_ns() - started) // 1_000_000
            recorder.emit(
                cache_status="hit",
                duration_ms=elapsed,
                kind="phase",
                phase=phase.name,
            )
            return PhaseResult(
                name=phase.name,
                identity=phase_identity,
                output_identity=checkpoint["output_manifest"]["identity"],
                output_root=phase_root,
                cache_status="hit",
                command_receipts=tuple(checkpoint["command_receipts"]),
            )
        phase_root.mkdir()
        command_receipts: list[dict[str, Any]] = []
        dependency_roots = {name: dependencies[name].output_root for name in phase.dependencies}
        input_paths = {
            name: binding.path.resolve(strict=True)
            for name in phase.inputs
            if (binding := inputs[name]).path is not None
        }
        for index, command in enumerate(phase.commands):
            command_receipts.append(
                self._execute_command(
                    phase,
                    index,
                    command,
                    tools[command.tool],
                    environment,
                    input_paths,
                    dependency_roots,
                    phase_root,
                    phase_temp,
                    recorder,
                )
            )
        for name in inputs:
            binding = inputs[name]
            if (
                binding.path is not None
                and _path_manifest(binding.path, binding.kind)
                != input_identities[name]["value"]["manifest"]
            ):
                raise InputMutationError(f"proof phase {phase.name} changed pinned input {name}")
        for name in phase_tools:
            row = _file_row(tools[name].path, ".")
            expected = tool_identities[name]["value"]
            if (
                row["byte_length"] != expected["byte_length"]
                or row["sha256"] != expected["sha256"]
            ):
                raise InputMutationError(f"proof phase {phase.name} changed pinned tool {name}")
        for name in phase.dependencies:
            if _output_manifest(dependencies[name].output_root)["identity"] != dependencies[name].output_identity:
                raise InputMutationError(
                    f"proof phase {phase.name} changed dependency output {name}"
                )
        shutil.rmtree(phase_temp)
        manifest = _output_manifest(phase_root)
        checkpoint_value = {
            "command_receipts": command_receipts,
            "dependency_outputs": identity_value["dependencies"],
            "identity": phase_identity,
            "input_identities": identity_value["inputs"],
            "output_manifest": manifest,
            "phase": phase.name,
            "phase_spec_identity": phase_spec_identity,
            "schema_version": CHECKPOINT_SCHEMA,
            "tool_identities": identity_value["tools"],
        }
        if phase.cache_mode != "disabled":
            self._seal_checkpoint(cache_root, phase_identity, checkpoint_value, phase_root)
        _freeze_tree(phase_root)
        elapsed = (time.monotonic_ns() - started) // 1_000_000
        cache_status = "disabled" if phase.cache_mode == "disabled" else "miss"
        recorder.emit(
            cache_status=cache_status,
            duration_ms=elapsed,
            kind="phase",
            phase=phase.name,
        )
        return PhaseResult(
            name=phase.name,
            identity=phase_identity,
            output_identity=manifest["identity"],
            output_root=phase_root,
            cache_status=cache_status,
            command_receipts=tuple(command_receipts),
        )

    def _execute_command(
        self,
        phase: PhaseSpec,
        index: int,
        command: CommandSpec,
        tool: ToolSpec,
        environment: Mapping[str, str],
        input_paths: Mapping[str, Path | None],
        dependency_roots: Mapping[str, Path],
        phase_root: Path,
        phase_temp: Path,
        recorder: _PerformanceRecorder,
    ) -> dict[str, Any]:
        values = {
            "phase_root": str(phase_root),
            "phase_temp": str(phase_temp),
            **{f"input:{name}": str(path) for name, path in input_paths.items()},
            **{f"dependency:{name}": str(path) for name, path in dependency_roots.items()},
        }
        args = [self._render(value, values) for value in command.args]
        cwd = Path(self._render(command.cwd, values))
        if not cwd.is_dir():
            raise EngineError(f"command cwd is not a directory: {cwd}")
        phase_environment = {
            **_BASE_ENVIRONMENT,
            **{key: self._render(value, values) for key, value in environment.items()},
            "TEMP": str(phase_temp),
            "TMP": str(phase_temp),
            "TMPDIR": str(phase_temp),
        }
        for key, value in command.environment:
            if key in _RESERVED_ENVIRONMENT:
                raise PlanError(f"command cannot replace engine-reserved environment key: {key}")
            phase_environment[key] = self._render(value, values)
        phase_temp.mkdir(exist_ok=True)
        executable = tool.path.resolve(strict=True)
        started = time.monotonic_ns()
        result = subprocess.run(
            [str(executable), *args],
            cwd=cwd,
            capture_output=True,
            check=False,
            env=phase_environment,
        )
        elapsed = (time.monotonic_ns() - started) // 1_000_000
        succeeded = result.returncode == command.expected_exit_code
        recorder.emit(
            cache_status="executed",
            command_index=index,
            command_label=command.label,
            duration_ms=elapsed,
            kind="command",
            phase=phase.name,
            result="pass" if succeeded else "fail",
        )
        if not succeeded:
            detail = (result.stderr or result.stdout)[-2000:].decode("utf-8", errors="replace")
            raise EngineError(
                f"proof phase {phase.name} command {command.label} exited {result.returncode}, "
                f"expected {command.expected_exit_code}: {detail.strip()}"
            )
        return {
            "command": self._command_value(command),
            "exit_code": result.returncode,
            "stderr": {"byte_length": len(result.stderr), "sha256": _digest(result.stderr)},
            "stdout": {"byte_length": len(result.stdout), "sha256": _digest(result.stdout)},
        }

    @staticmethod
    def _render(template: str, values: Mapping[str, str]) -> str:
        def replace(match: re.Match[str]) -> str:
            key = match.group(0)[1:-1]
            if key not in values:
                raise PlanError(f"template references unavailable proof value: {key}")
            return values[key]

        rendered = _PLACEHOLDER.sub(replace, template)
        if any(
            prefix in rendered
            for prefix in (
                "{input:",
                "{dependency:",
                "{phase_root}",
                "{phase_temp}",
                "{run_root}",
            )
        ):
            raise PlanError(f"template contains an invalid proof placeholder: {template}")
        return rendered

    @staticmethod
    def _checkpoint_root(cache_root: Path, phase_identity: str) -> Path:
        return cache_root / "objects" / phase_identity.removeprefix("sha256:")

    @staticmethod
    def _checkpoint_binding_path(cache_root: Path, phase_identity: str) -> Path:
        return (
            cache_root
            / "checkpoint-bindings"
            / f"{phase_identity.removeprefix('sha256:')}.json"
        )

    def _require_external_checkpoint_binding(
        self, cache_root: Path, phase_identity: str, root: Path
    ) -> None:
        path = self._checkpoint_binding_path(cache_root, phase_identity)
        if path.is_symlink() or not path.is_file() or path.stat().st_mode & 0o222:
            raise CacheCorruptionError(f"checkpoint binding is absent or mutable: {path}")
        try:
            binding = json.loads(path.read_text(encoding="ascii"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise CacheCorruptionError(f"checkpoint binding is unreadable: {path}") from error
        expected = {
            "checkpoint_tree_identity": _output_manifest(root)["identity"],
            "phase_identity": phase_identity,
            "schema_version": CHECKPOINT_BINDING_SCHEMA,
        }
        if binding != expected:
            raise CacheCorruptionError(f"checkpoint differs from external binding: {root}")

    def _write_checkpoint_binding(
        self, cache_root: Path, phase_identity: str, root: Path
    ) -> None:
        path = self._checkpoint_binding_path(cache_root, phase_identity)
        value = {
            "checkpoint_tree_identity": _output_manifest(root)["identity"],
            "phase_identity": phase_identity,
            "schema_version": CHECKPOINT_BINDING_SCHEMA,
        }
        data = _canonical_bytes(value)
        if path.exists() or path.is_symlink():
            if path.is_symlink() or not path.is_file() or path.read_bytes() != data:
                raise CacheCorruptionError(f"checkpoint binding differs: {path}")
            if path.stat().st_mode & 0o222:
                raise CacheCorruptionError(f"checkpoint binding is mutable: {path}")
            return
        _write_once(path, data)
        path.chmod(0o444)
        with path.open("rb") as handle:
            os.fsync(handle.fileno())
        _fsync_directory(path.parent)

    def _load_checkpoint(self, cache_root: Path, phase_identity: str) -> dict[str, Any] | None:
        root = self._checkpoint_root(cache_root, phase_identity)
        if root.is_symlink():
            raise CacheCorruptionError(f"checkpoint root is a symlink: {root}")
        if not root.exists():
            return None
        if not root.is_dir():
            raise CacheCorruptionError(f"checkpoint root is not a directory: {root}")
        try:
            _require_frozen_tree(root)
        except PublicationError as error:
            raise CacheCorruptionError(f"checkpoint is mutable or unsafe: {root}") from error
        self._require_external_checkpoint_binding(cache_root, phase_identity, root)
        checkpoint_path = root / "checkpoint.json"
        payload = root / "payload"
        try:
            checkpoint = json.loads(checkpoint_path.read_text(encoding="ascii"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise CacheCorruptionError(f"checkpoint is unreadable: {root}") from error
        if not isinstance(checkpoint, dict):
            raise CacheCorruptionError(f"checkpoint is not an object: {root}")
        if set(checkpoint) != _CHECKPOINT_KEYS:
            raise CacheCorruptionError(f"checkpoint has unknown or missing fields: {root}")
        integrity = checkpoint.get("integrity")
        unsigned = {key: value for key, value in checkpoint.items() if key != "integrity"}
        if integrity != _digest(_canonical_bytes(unsigned)):
            raise CacheCorruptionError(f"checkpoint integrity differs: {root}")
        if checkpoint.get("schema_version") != CHECKPOINT_SCHEMA or checkpoint.get("identity") != phase_identity:
            raise CacheCorruptionError(f"checkpoint identity differs: {root}")
        try:
            actual_manifest = _output_manifest(payload)
        except EngineError as error:
            raise CacheCorruptionError(f"checkpoint payload is invalid: {root}") from error
        if checkpoint.get("output_manifest") != actual_manifest:
            raise CacheCorruptionError(f"checkpoint payload differs from its manifest: {root}")
        return checkpoint

    def _seal_checkpoint(
        self,
        cache_root: Path,
        phase_identity: str,
        checkpoint: dict[str, Any],
        output_root: Path,
    ) -> None:
        locks = cache_root / "locks"
        locks.mkdir(parents=True, exist_ok=True)
        descriptor = os.open(
            locks / f"phase-{phase_identity.removeprefix('sha256:')}.lock",
            os.O_CREAT
            | os.O_RDWR
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            self._seal_checkpoint_locked(
                cache_root, phase_identity, checkpoint, output_root
            )
        finally:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
            os.close(descriptor)

    def _seal_checkpoint_locked(
        self,
        cache_root: Path,
        phase_identity: str,
        checkpoint: dict[str, Any],
        output_root: Path,
    ) -> None:
        objects = cache_root / "objects"
        if objects.is_symlink() or (objects.exists() and not objects.is_dir()):
            raise CacheCorruptionError(f"checkpoint object root is unsafe: {objects}")
        objects.mkdir(parents=True, exist_ok=True)
        if objects.is_symlink() or not objects.is_dir():
            raise CacheCorruptionError(f"checkpoint object root is unsafe: {objects}")
        target = self._checkpoint_root(cache_root, phase_identity)
        if target.exists():
            existing = self._load_checkpoint(cache_root, phase_identity)
            if existing is None:
                raise CacheCorruptionError(f"checkpoint disappeared while sealing: {target}")
            self._require_matching_checkpoint(existing, checkpoint, target)
            return
        temporary = Path(tempfile.mkdtemp(prefix=".checkpoint-", dir=objects))
        try:
            _copy_tree(output_root, temporary / "payload")
            unsigned = dict(checkpoint)
            signed = {**unsigned, "integrity": _digest(_canonical_bytes(unsigned))}
            (temporary / "checkpoint.json").write_bytes(_canonical_bytes(signed))
            _freeze_tree(temporary)
            self._write_checkpoint_binding(cache_root, phase_identity, temporary)
            try:
                os.rename(temporary, target)
                _fsync_directory(objects)
            except OSError:
                if not target.exists():
                    raise
                existing = self._load_checkpoint(cache_root, phase_identity)
                if existing is None:
                    raise CacheCorruptionError(
                        f"checkpoint disappeared after a concurrent seal: {target}"
                    )
                self._require_matching_checkpoint(existing, checkpoint, target)
        finally:
            if temporary.exists():
                _remove_frozen_tree(temporary)

    @staticmethod
    def _require_checkpoint_binding(
        checkpoint: Mapping[str, Any],
        phase: PhaseSpec,
        phase_spec_identity: str,
        dependency_outputs: Mapping[str, str],
        input_identities: Mapping[str, str],
        tool_identities: Mapping[str, str],
    ) -> None:
        expected = {
            "dependency_outputs": dict(dependency_outputs),
            "input_identities": dict(input_identities),
            "phase": phase.name,
            "phase_spec_identity": phase_spec_identity,
            "tool_identities": dict(tool_identities),
        }
        actual = {key: checkpoint.get(key) for key in expected}
        if actual != expected:
            raise CacheCorruptionError(
                f"checkpoint binding differs for proof phase {phase.name}"
            )

    @staticmethod
    def _require_matching_checkpoint(
        existing: Mapping[str, Any], expected_unsigned: Mapping[str, Any], root: Path
    ) -> None:
        existing_unsigned = {key: value for key, value in existing.items() if key != "integrity"}
        if existing_unsigned != dict(expected_unsigned):
            raise CacheCorruptionError(
                f"same proof identity produced a different checkpoint: {root}"
            )

    def _publish(
        self,
        publication: PublicationSpec,
        plan_identity: str,
        plan_value: Mapping[str, Any],
        run_token: str,
        phases: Sequence[PhaseResult],
        initial_pointer_state: Mapping[str, Any] | None,
    ) -> str:
        phase_by_name = {phase.name: phase for phase in phases}
        objects = publication.release_root / "objects"
        if objects.is_symlink() or (objects.exists() and not objects.is_dir()):
            raise PublicationError(f"release object root is unsafe: {objects}")
        objects.mkdir(parents=True, exist_ok=True)
        if objects.is_symlink() or not objects.is_dir():
            raise PublicationError(f"release object root is unsafe: {objects}")
        temporary = Path(tempfile.mkdtemp(prefix=".publication-", dir=objects))
        payload = temporary / "payload"
        payload.mkdir()
        try:
            for output in publication.outputs:
                source_path = phase_by_name[output.phase].output_root / Path(
                    _relative_path(output.source, "publication source", allow_dot=True)
                )
                destination_path = payload / Path(
                    _relative_path(output.destination, "publication destination")
                )
                try:
                    source_path.resolve(strict=True).relative_to(
                        phase_by_name[output.phase].output_root.resolve(strict=True)
                    )
                except (OSError, ValueError) as error:
                    raise PublicationError(f"publication source escapes its phase: {output.source}") from error
                if source_path.is_symlink() or not source_path.exists():
                    raise PublicationError(f"publication source is absent or unsafe: {output.source}")
                destination_path.parent.mkdir(parents=True, exist_ok=True)
                if source_path.is_dir():
                    _copy_tree(source_path, destination_path)
                elif source_path.is_file():
                    _file_row(source_path, ".")
                    _copy_file(source_path, destination_path)
                else:
                    raise PublicationError(f"publication source is unsupported: {output.source}")
            for phase in phases:
                if _output_manifest(phase.output_root)["identity"] != phase.output_identity:
                    raise InputMutationError(
                        f"a completed proof output changed during publication: {phase.name}"
                    )
            payload_manifest = _output_manifest(payload)
            value = {
                "outputs": [
                    {
                        "destination": output.destination,
                        "phase": output.phase,
                        "source": output.source,
                    }
                    for output in publication.outputs
                ],
                "payload_manifest": payload_manifest,
                "phase_receipts": [
                    {
                        "command_receipts": list(phase.command_receipts),
                        "identity": phase.identity,
                        "name": phase.name,
                        "output_identity": phase.output_identity,
                    }
                    for phase in phases
                ],
                "plan": plan_value,
                "plan_identity": plan_identity,
                "run_token": run_token,
                "schema_version": PUBLICATION_SCHEMA,
            }
            identity = _digest(_canonical_bytes(value))
            release = {"canonical_value": value, "identity": identity}
            (temporary / "release.json").write_bytes(_canonical_bytes(release))
            target = objects / identity.removeprefix("sha256:")
            if target.exists():
                self._validate_release(target, release)
            else:
                try:
                    _freeze_tree(temporary)
                    os.rename(temporary, target)
                    _fsync_directory(objects)
                except OSError:
                    if not target.exists():
                        raise
                    self._validate_release(target, release)
            pointer = {
                "object": f"objects/{identity.removeprefix('sha256:')}",
                "release_identity": identity,
                "schema_version": POINTER_SCHEMA,
            }
            pointer_bytes = _canonical_bytes(pointer)
            desired_pointer_state = {
                "byte_length": len(pointer_bytes),
                "executable": False,
                "sha256": _digest(pointer_bytes),
                "state": "present",
            }
            if initial_pointer_state is None:
                raise PublicationError("proof publication lacks its initial pointer binding")
            publication.pointer_path.parent.mkdir(parents=True, exist_ok=True)
            lock_path = publication.pointer_path.with_name(
                f".{publication.pointer_path.name}.lock"
            )
            descriptor = os.open(
                lock_path,
                os.O_CREAT
                | os.O_RDWR
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                0o600,
            )
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX)
                current_pointer_state = self._publication_pointer_state(
                    publication.pointer_path
                )
                if current_pointer_state == desired_pointer_state:
                    pass
                elif current_pointer_state == dict(initial_pointer_state):
                    _write_atomic(publication.pointer_path, pointer_bytes)
                else:
                    raise PublicationError(
                        "proof publication pointer advanced after this run began"
                    )
            finally:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
                os.close(descriptor)
            return identity
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)

    @staticmethod
    def _validate_release(root: Path, expected_release: dict[str, Any]) -> None:
        try:
            _require_frozen_tree(root)
            release = json.loads((root / "release.json").read_text(encoding="ascii"))
            manifest = _output_manifest(root / "payload")
        except (OSError, UnicodeError, json.JSONDecodeError, EngineError) as error:
            raise PublicationError(f"existing proof publication is unreadable: {root}") from error
        if release != expected_release or manifest != expected_release["canonical_value"]["payload_manifest"]:
            raise PublicationError(f"existing proof publication differs: {root}")
