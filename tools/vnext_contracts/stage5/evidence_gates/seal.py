#!/usr/bin/env python3
"""Run one immutable, resumable, isolated three-engine Stage 5 proof seal."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, cast


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
sys.dont_write_bytecode = True


DEFAULT_SNAPSHOT_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src",
    "embedded",
    "tests",
    "tools/vnext_contracts",
    "contracts/vnext/catalogs",
    "contracts/vnext/stage0",
    "contracts/vnext/stage2",
    "contracts/vnext/stage3",
    "contracts/vnext/stage4/execution",
)
SNAPSHOT_PATHS = DEFAULT_SNAPSHOT_PATHS
SNAPSHOT_SCHEMA = "maestro.vnext.stage5.immutable-workspace-snapshot.v1"
STAGE4_SOURCE_COMMIT = "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2"
STAGE4_SOURCE_TREE = "2f832a04c7109e17b4b298e40b4827c1ced2d527"
STAGE4_SOURCE_ARCHIVE = "predecessors/stage4-source.tar.gz"
STAGE4_SOURCE_ARCHIVE_LENGTH = 16_486_231
STAGE4_SOURCE_ARCHIVE_SHA256 = (
    "347eaf928f81d9ce6e07e3767f0cdaf2cde23cd98d13bad41b745d5fbc359910"
)


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def toolchain_binary(name: str) -> Path:
    rustup = shutil.which("rustup")
    if rustup:
        completed = subprocess.run(
            [rustup, "which", name], capture_output=True, check=False, text=True
        )
        if completed.returncode == 0:
            return Path(completed.stdout.strip()).resolve(strict=True)
    candidate = shutil.which(name)
    if candidate is None:
        raise RuntimeError(f"required proof tool is unavailable: {name}")
    return Path(candidate).resolve(strict=True)


def required_tool(name: str) -> Path:
    candidate = shutil.which(name)
    if candidate is None:
        raise RuntimeError(f"required proof tool is unavailable: {name}")
    return Path(candidate).resolve(strict=True)


def read_regular_file(path: Path) -> tuple[bytes, bool]:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError(f"snapshot source is not a regular file: {path}")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
        after = os.fstat(descriptor)
        if (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        ) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        ):
            raise RuntimeError(f"snapshot source changed while it was read: {path}")
        data = b"".join(chunks)
        if len(data) != before.st_size:
            raise RuntimeError(f"snapshot source length changed while it was read: {path}")
        return data, bool(before.st_mode & 0o111)
    finally:
        os.close(descriptor)


def path_rows(root: Path, relatives: tuple[str, ...]) -> list[list[object]]:
    rows: list[list[object]] = []
    for relative in relatives:
        path = root / relative
        if not path.exists() or path.is_symlink():
            raise RuntimeError(f"snapshot source is absent or unsafe: {path}")
        children = [path] if path.is_file() else sorted(path.rglob("*"))
        for child in children:
            if child.is_symlink():
                raise RuntimeError(f"snapshot source contains an unsafe entry: {child}")
            if child.is_dir() or "__pycache__" in child.parts or child.suffix == ".pyc":
                continue
            if not child.is_file():
                raise RuntimeError(f"snapshot source contains an unsafe entry: {child}")
            name = child.relative_to(root).as_posix()
            data, executable = read_regular_file(child)
            rows.append([name, len(data), sha256(data), executable])
    rows.sort(key=lambda row: str(row[0]))
    return rows


def source_rows() -> list[list[object]]:
    rows = path_rows(WORKSPACE, SNAPSHOT_PATHS)
    if SNAPSHOT_PATHS == DEFAULT_SNAPSHOT_PATHS:
        rows.append(
            [
                STAGE4_SOURCE_ARCHIVE,
                STAGE4_SOURCE_ARCHIVE_LENGTH,
                STAGE4_SOURCE_ARCHIVE_SHA256,
                False,
            ]
        )
        rows.sort(key=lambda row: str(row[0]))
    return rows


def stage4_source_archive() -> bytes:
    tree = subprocess.run(
        ["git", "rev-parse", f"{STAGE4_SOURCE_COMMIT}^{{tree}}"],
        cwd=WORKSPACE,
        capture_output=True,
        check=False,
        text=True,
    )
    if tree.returncode != 0 or tree.stdout.strip() != STAGE4_SOURCE_TREE:
        raise RuntimeError("exact Stage 4 source commit/tree is unavailable or substituted")
    completed = subprocess.run(
        ["git", "archive", "--format=tar.gz", STAGE4_SOURCE_COMMIT],
        cwd=WORKSPACE,
        capture_output=True,
        check=False,
    )
    if (
        completed.returncode != 0
        or len(completed.stdout) != STAGE4_SOURCE_ARCHIVE_LENGTH
        or sha256(completed.stdout) != STAGE4_SOURCE_ARCHIVE_SHA256
    ):
        raise RuntimeError("exact Stage 4 source archive is unavailable or substituted")
    return completed.stdout


def snapshot_tree_rows(root: Path) -> list[list[object]]:
    if root.is_symlink() or not root.is_dir():
        raise RuntimeError(f"immutable snapshot root is absent or unsafe: {root}")
    rows = []
    for child in sorted(root.rglob("*")):
        if child.is_symlink():
            raise RuntimeError(f"immutable snapshot contains an unsafe entry: {child}")
        if child.is_dir() or child.name == "snapshot-manifest.v1.json":
            continue
        if not child.is_file():
            raise RuntimeError(f"immutable snapshot contains an unsafe entry: {child}")
        data, executable = read_regular_file(child)
        rows.append(
            [
                child.relative_to(root).as_posix(),
                len(data),
                sha256(data),
                executable,
            ]
        )
    return rows


def require_frozen_snapshot(root: Path) -> None:
    for child in [root, *root.rglob("*")]:
        info = child.lstat()
        if stat.S_ISLNK(info.st_mode) or info.st_mode & 0o222:
            raise RuntimeError(f"immutable snapshot cache entry is mutable or unsafe: {child}")


def freeze_tree(root: Path) -> None:
    for child in sorted(root.rglob("*"), reverse=True):
        if child.is_symlink():
            raise RuntimeError(f"immutable snapshot contains a symlink: {child}")
        if child.is_file():
            child.chmod(0o555 if child.stat().st_mode & 0o111 else 0o444)
        elif child.is_dir():
            child.chmod(0o555)
    root.chmod(0o555)


def copy_snapshot_sources(destination: Path, source_rows: list[list[object]]) -> None:
    for name_value, length_value, digest_value, executable_value in source_rows:
        name = str(name_value)
        source = WORKSPACE / name
        if name == STAGE4_SOURCE_ARCHIVE and SNAPSHOT_PATHS == DEFAULT_SNAPSHOT_PATHS:
            data, executable = stage4_source_archive(), False
        else:
            data, executable = read_regular_file(source)
        if (
            len(data) != cast(int, length_value)
            or sha256(data) != cast(str, digest_value)
            or executable != cast(bool, executable_value)
        ):
            raise RuntimeError(f"snapshot source changed before it was copied: {source}")
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        target.chmod(0o755 if executable else 0o644)


def build_snapshot(cargo: Path, snapshot_cache: Path) -> Path:
    if snapshot_cache.is_symlink() or (snapshot_cache.exists() and not snapshot_cache.is_dir()):
        raise RuntimeError(f"Stage 5 snapshot cache root is unsafe: {snapshot_cache}")
    snapshot_cache.mkdir(parents=True, exist_ok=True)
    if snapshot_cache.is_symlink():
        raise RuntimeError(f"Stage 5 snapshot cache root is unsafe: {snapshot_cache}")
    bound_source_rows = source_rows()
    source_identity = f"sha256:{sha256(canonical_json(bound_source_rows))}"
    objects = snapshot_cache / "objects"
    objects.mkdir(parents=True, exist_ok=True)
    if objects.is_symlink() or not objects.is_dir():
        raise RuntimeError("immutable Stage 5 snapshot object root is unsafe")
    temporary = Path(tempfile.mkdtemp(prefix=".stage5-snapshot-", dir=objects))
    try:
        copy_snapshot_sources(temporary, bound_source_rows)
        if source_rows() != bound_source_rows:
            raise RuntimeError("Stage 5 snapshot source tree changed while it was copied")
        snapshot_source_paths: tuple[str, ...] = SNAPSHOT_PATHS
        if SNAPSHOT_PATHS == DEFAULT_SNAPSHOT_PATHS:
            snapshot_source_paths += (STAGE4_SOURCE_ARCHIVE,)
        if path_rows(temporary, snapshot_source_paths) != bound_source_rows:
            raise RuntimeError("Stage 5 snapshot copy does not match its bound source closure")
        environment = dict(os.environ)
        environment.pop("CARGO_NET_OFFLINE", None)
        vendored = subprocess.run(
            [str(cargo), "vendor", "--locked", "--versioned-dirs", "vendor"],
            cwd=temporary,
            env=environment,
            capture_output=True,
            check=False,
        )
        if vendored.returncode != 0:
            raise RuntimeError(vendored.stderr[-8_000:].decode("utf-8", errors="replace"))
        cargo_config = temporary / ".cargo/config.toml"
        cargo_config.parent.mkdir()
        cargo_config.write_bytes(vendored.stdout)
        snapshot_rows = snapshot_tree_rows(temporary)
        snapshot_identity = f"sha256:{sha256(canonical_json(snapshot_rows))}"
        manifest = {
            "schema_version": SNAPSHOT_SCHEMA,
            "snapshot_identity": snapshot_identity,
            "source_identity": source_identity,
            "source_rows": bound_source_rows,
        }
        (temporary / "snapshot-manifest.v1.json").write_bytes(canonical_json(manifest))
        target = objects / snapshot_identity.removeprefix("sha256:")
        if target.exists():
            manifest_bytes, _ = read_regular_file(target / "snapshot-manifest.v1.json")
            manifest = json.loads(manifest_bytes.decode("ascii"))
            actual_rows = snapshot_tree_rows(target)
            actual_identity = f"sha256:{sha256(canonical_json(actual_rows))}"
            if (
                actual_rows != snapshot_rows
                or actual_identity != snapshot_identity
                or manifest
                != {
                    "schema_version": SNAPSHOT_SCHEMA,
                    "snapshot_identity": snapshot_identity,
                    "source_identity": source_identity,
                    "source_rows": bound_source_rows,
                }
            ):
                raise RuntimeError("same immutable snapshot identity has different bytes")
            require_frozen_snapshot(target)
        else:
            freeze_tree(temporary)
            os.rename(temporary, target)
        return target
    finally:
        if temporary.exists():
            shutil.rmtree(temporary, ignore_errors=True)


def macos_sdk_root() -> Path:
    xcrun = required_tool("xcrun")
    completed = subprocess.run(
        [str(xcrun), "--show-sdk-path"], capture_output=True, check=False, text=True
    )
    if completed.returncode != 0 or not completed.stdout.strip():
        raise RuntimeError("the exact macOS SDK root is unavailable")
    return Path(completed.stdout.strip()).resolve(strict=True)


def xcrun_tool(name: str) -> Path:
    xcrun = required_tool("xcrun")
    completed = subprocess.run(
        [str(xcrun), "--find", name], capture_output=True, check=False, text=True
    )
    if completed.returncode != 0 or not completed.stdout.strip():
        raise RuntimeError(f"the active developer toolchain does not provide {name}")
    return Path(completed.stdout.strip()).resolve(strict=True)


def developer_toolchain_rows(root: Path) -> list[list[object]]:
    rows = snapshot_tree_rows(root)
    return [row for row in rows if row[0] != "developer-toolchain-manifest.v1.json"]


def verify_developer_toolchain_execution(root: Path) -> None:
    clang = root / "usr/bin/clang"
    ar = root / "usr/bin/ar"
    ranlib = root / "usr/bin/ranlib"
    with tempfile.TemporaryDirectory(prefix="maestro-stage5-clt-probe-") as directory:
        probe_root = Path(directory)
        source = probe_root / "probe.c"
        object_file = probe_root / "probe.o"
        archive = probe_root / "libprobe.a"
        source.write_text("int maestro_stage5_probe(void) { return 5; }\n", encoding="ascii")
        commands = (
            [str(clang), "-c", str(source), "-o", str(object_file)],
            [str(ar), "rcs", str(archive), str(object_file)],
            [str(ranlib), str(archive)],
        )
        for command in commands:
            completed = subprocess.run(command, capture_output=True, check=False)
            if completed.returncode != 0:
                raise RuntimeError(
                    "relocated developer toolchain is not executable: "
                    + completed.stderr[-4_000:].decode("utf-8", errors="replace")
                )


def build_developer_toolchain_closure(cache_root: Path) -> Path:
    sources = {
        "usr/bin/clang": xcrun_tool("clang"),
        "usr/bin/ar": xcrun_tool("ar"),
        "usr/bin/ranlib": xcrun_tool("ranlib"),
    }
    ranlib_root = sources["usr/bin/ranlib"].parent.parent
    sources["usr/lib/libLTO.dylib"] = (ranlib_root / "lib/libLTO.dylib").resolve(strict=True)
    if cache_root.is_symlink() or (cache_root.exists() and not cache_root.is_dir()):
        raise RuntimeError(f"developer toolchain cache root is unsafe: {cache_root}")
    objects = cache_root / "objects"
    if objects.is_symlink() or (objects.exists() and not objects.is_dir()):
        raise RuntimeError("developer toolchain object root is unsafe")
    objects.mkdir(parents=True, exist_ok=True)
    if objects.is_symlink() or not objects.is_dir():
        raise RuntimeError("developer toolchain object root is unsafe")
    temporary = Path(tempfile.mkdtemp(prefix=".developer-toolchain-", dir=objects))
    try:
        source_rows: list[list[object]] = []
        for relative, source in sorted(sources.items()):
            data, executable = read_regular_file(source)
            source_rows.append([relative, len(data), sha256(data), executable])
            destination = temporary / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("xb") as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            destination.chmod(0o755 if executable else 0o644)
        (temporary / "usr/bin/clang++").hardlink_to(temporary / "usr/bin/clang")
        verify_developer_toolchain_execution(temporary)
        rows = developer_toolchain_rows(temporary)
        identity = f"sha256:{sha256(canonical_json(rows))}"
        manifest = {
            "identity": identity,
            "schema_version": "maestro.vnext.stage5.developer-toolchain-closure.v1",
            "source_rows": source_rows,
            "tree_rows": rows,
        }
        (temporary / "developer-toolchain-manifest.v1.json").write_bytes(
            canonical_json(manifest)
        )
        target = objects / identity.removeprefix("sha256:")
        if target.exists():
            actual_manifest = json.loads(
                (target / "developer-toolchain-manifest.v1.json").read_text(encoding="ascii")
            )
            if developer_toolchain_rows(target) != rows or actual_manifest != manifest:
                raise RuntimeError("same developer toolchain identity has different bytes")
            require_frozen_snapshot(target)
        else:
            freeze_tree(temporary)
            os.rename(temporary, target)
        verify_developer_toolchain_execution(target)
        return target
    finally:
        if temporary.exists():
            shutil.rmtree(temporary, ignore_errors=True)


def probe(command: list[str]) -> str:
    completed = subprocess.run(command, capture_output=True, check=False)
    return sha256(
        str(completed.returncode).encode("ascii")
        + b"\0"
        + completed.stdout
        + b"\0"
        + completed.stderr
    )


def fresh_seal_token(snapshot: Path, sdk_root: Path, tools: tuple[Path, ...]) -> str:
    digest = hashlib.sha256(b"maestro.vnext.stage5.seal-token.v2\0")
    digest.update(snapshot.name.encode("ascii"))
    digest.update(str(sdk_root).encode("utf-8"))
    for tool in tools:
        digest.update(hashlib.sha256(tool.read_bytes()).digest())
    return f"stage5-{digest.hexdigest()}-{secrets.token_hex(16)}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-root", type=Path)
    parser.add_argument("--cache-root", type=Path)
    parser.add_argument("--snapshot-cache", type=Path)
    parser.add_argument("--performance-log", type=Path)
    parser.add_argument("--resume-token")
    parser.add_argument("--prepare-snapshot-only", action="store_true")
    parser.add_argument("--immutable-snapshot", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--sdk-root", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--publication-workspace", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()

    python = Path(sys.executable).resolve(strict=True)
    cargo = toolchain_binary("cargo")
    snapshot_cache = args.snapshot_cache or Path("/private/tmp/maestro-vnext-proof-snapshots")
    if args.immutable_snapshot is None:
        workspace_snapshot = build_snapshot(cargo, snapshot_cache)
        sdk_root = macos_sdk_root()
        if args.prepare_snapshot_only:
            print(
                json.dumps(
                    {"sdk_root": str(sdk_root), "workspace_snapshot": str(workspace_snapshot)},
                    sort_keys=True,
                )
            )
            return 0
        child_arguments = [
            str(python),
            str(workspace_snapshot / "tools/vnext_contracts/stage5/evidence_gates/seal.py"),
            "--immutable-snapshot",
            str(workspace_snapshot),
            "--sdk-root",
            str(sdk_root),
            "--publication-workspace",
            str(WORKSPACE),
            "--snapshot-cache",
            str(snapshot_cache),
        ]
        for name, value in [
            ("--run-root", args.run_root),
            ("--cache-root", args.cache_root),
            ("--performance-log", args.performance_log),
            ("--resume-token", args.resume_token),
        ]:
            if value is not None:
                child_arguments.extend([name, str(value)])
        environment = dict(os.environ)
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        environment.pop("PYTHONPATH", None)
        os.execve(str(python), child_arguments, environment)

    workspace_snapshot = args.immutable_snapshot.resolve(strict=True)
    if workspace_snapshot != WORKSPACE.resolve(strict=True):
        raise RuntimeError("Stage 5 seal is not executing from its bound immutable snapshot")
    require_frozen_snapshot(workspace_snapshot)
    if args.sdk_root is None:
        raise RuntimeError("immutable Stage 5 seal execution lacks its macOS SDK closure")
    sdk_root = args.sdk_root.resolve(strict=True)
    if args.prepare_snapshot_only or args.publication_workspace is None:
        raise RuntimeError("immutable Stage 5 seal execution lacks its publication workspace")
    publication_workspace = args.publication_workspace.resolve(strict=True)
    if not publication_workspace.is_dir() or publication_workspace.is_symlink():
        raise RuntimeError("Stage 5 publication workspace is absent or unsafe")
    sys.path.insert(0, str(workspace_snapshot))
    from tools.vnext_contracts.proof_engine import (  # noqa: PLC0415
        CommandSpec,
        InputBinding,
        PhaseSpec,
        ProofEngine,
        ProofPlan,
        PublicationSpec,
        PublishedOutput,
        ToolSpec,
    )

    ruby = required_tool("ruby")
    rustc = toolchain_binary("rustc")
    developer_toolchain = build_developer_toolchain_closure(
        Path("/private/tmp/maestro-vnext-stage5-developer-toolchains")
    )
    cc = developer_toolchain / "usr/bin/clang"
    cxx = developer_toolchain / "usr/bin/clang++"
    ar = developer_toolchain / "usr/bin/ar"
    ranlib = developer_toolchain / "usr/bin/ranlib"
    git = required_tool("git")
    tools = (python, ruby, cargo, rustc, cc, cxx, ar, ranlib, git)
    token = args.resume_token or fresh_seal_token(workspace_snapshot, sdk_root, tools)
    run_root = args.run_root or Path("/private/tmp/maestro-vnext-stage5-proof") / token
    cache_root = args.cache_root or Path("/private/tmp/maestro-vnext-proof-cache")
    performance_log = args.performance_log or Path(
        "/private/tmp/maestro-vnext-proof-performance"
    ) / f"{token}.jsonl"
    publication_root = publication_workspace / "contracts/vnext/stage5/evidence-gates/releases"
    pointer = publication_workspace / "contracts/vnext/stage5/evidence-gates/current-proof.json"

    target_probe = subprocess.run(
        [str(rustc), "-vV"], capture_output=True, check=True, text=True
    ).stdout
    target = next(
        line.removeprefix("host: ")
        for line in target_probe.splitlines()
        if line.startswith("host: ")
    )
    rust_toolchain_root = rustc.parent.parent
    rustc_drivers = sorted((rust_toolchain_root / "lib").glob("librustc_driver-*.dylib"))
    if len(rustc_drivers) != 1:
        raise RuntimeError("Rust compiler closure must contain exactly one driver library")
    rustc_driver = rustc_drivers[0].resolve(strict=True)
    rust_target_lib = rust_toolchain_root / "lib/rustlib" / target / "lib"
    if rust_target_lib.is_symlink() or not rust_target_lib.is_dir():
        raise RuntimeError("Rust compiler target library closure is absent or unsafe")
    bindings = (
        InputBinding.tree("workspace-snapshot", workspace_snapshot, path_identity="content"),
        InputBinding.symlink_tree("sdk-root", sdk_root, path_identity="content"),
        InputBinding.file("python-bin", python),
        InputBinding.file("ruby-bin", ruby),
        InputBinding.file("cargo-bin", cargo),
        InputBinding.file("rustc-bin", rustc),
        InputBinding.file("rustc-driver", rustc_driver),
        InputBinding.tree("rust-target-lib", rust_target_lib),
        InputBinding.tree("developer-toolchain", developer_toolchain),
        InputBinding.file("git-bin", git),
        InputBinding.literal("target-triple", target),
        InputBinding.literal("profile", "test-unoptimized"),
        InputBinding.literal("mutant", "none-stage5"),
        InputBinding.literal("cargo-probe", probe([str(cargo), "-Vv"])),
        InputBinding.literal("rustc-probe", probe([str(rustc), "-vV"])),
        InputBinding.literal("cc-probe", probe([str(cc), "--version"])),
        InputBinding.literal("cxx-probe", probe([str(cxx), "--version"])),
        InputBinding.literal("ar-probe", probe([str(ar), "--version"])),
        InputBinding.literal("ranlib-probe", probe([str(ranlib), "--version"])),
        InputBinding.literal("git-probe", probe([str(git), "--version"])),
    )
    all_inputs = tuple(binding.name for binding in bindings)
    proof_environment = (
        ("CARGO_HOME", "{phase_temp}/cargo-home"),
        ("HOME", "{phase_temp}/home"),
        ("MAESTRO_VERSION", "0.107.0-stage5-proof"),
    )
    cargo_environment = (
        ("CARGO_TARGET_DIR", "{phase_temp}/cargo-target"),
        ("CC", "{input:developer-toolchain}/usr/bin/clang"),
        ("CXX", "{input:developer-toolchain}/usr/bin/clang++"),
        ("AR", "{input:developer-toolchain}/usr/bin/ar"),
        ("RANLIB", "{input:developer-toolchain}/usr/bin/ranlib"),
        ("RUSTC", "{dependency:toolchain}/out/toolchain/bin/rustc"),
        ("SDKROOT", "{input:sdk-root}"),
        ("PATH", "{dependency:toolchain}/out/toolchain/bin:/usr/bin:/bin"),
    )
    plan = ProofPlan(
        inputs=bindings,
        tools=(
            ToolSpec("python", python, ("--version",)),
            ToolSpec("ruby", ruby, ("--version",)),
        ),
        phases=(
            PhaseSpec(
                name="toolchain",
                commands=(
                    CommandSpec(
                        tool="python",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/toolchain.py",
                            "--rustc",
                            "{input:rustc-bin}",
                            "--cargo",
                            "{input:cargo-bin}",
                            "--python",
                            "{input:python-bin}",
                            "--ruby",
                            "{input:ruby-bin}",
                            "--cc",
                            "{input:developer-toolchain}/usr/bin/clang",
                            "--cxx",
                            "{input:developer-toolchain}/usr/bin/clang++",
                            "--ar",
                            "{input:developer-toolchain}/usr/bin/ar",
                            "--ranlib",
                            "{input:developer-toolchain}/usr/bin/ranlib",
                            "--lib-lto",
                            "{input:developer-toolchain}/usr/lib/libLTO.dylib",
                            "--git",
                            "{input:git-bin}",
                            "--driver",
                            "{input:rustc-driver}",
                            "--target-lib",
                            "{input:rust-target-lib}",
                            "--target",
                            target,
                            "--output-root",
                            "{phase_root}/out",
                        ),
                        cwd="{input:workspace-snapshot}",
                        label="immutable-rust-toolchain-closure",
                    ),
                ),
                inputs=(
                    "workspace-snapshot",
                    "python-bin",
                    "ruby-bin",
                    "rustc-bin",
                    "rustc-driver",
                    "rust-target-lib",
                    "cargo-bin",
                    "developer-toolchain",
                    "git-bin",
                    "target-triple",
                ),
                cache_mode="content",
            ),
            PhaseSpec(
                name="predecessor",
                commands=(
                    CommandSpec(
                        tool="python",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/predecessor.py",
                            "--output-root",
                            "{phase_root}/out",
                            "--stage4-source",
                            "{input:workspace-snapshot}/predecessors/stage4-source.tar.gz",
                        ),
                        cwd="{input:workspace-snapshot}",
                        environment=cargo_environment,
                        label="sealed-predecessor-closure",
                    ),
                ),
                inputs=all_inputs,
                dependencies=("toolchain",),
                cache_mode="run",
            ),
            PhaseSpec(
                name="harness",
                commands=(
                    CommandSpec(
                        tool="python",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/harness.py",
                            "--output-root",
                            "{phase_root}/out",
                        ),
                        cwd="{input:workspace-snapshot}",
                        label="proof-engine-and-snapshot-adversarial-closure",
                    ),
                ),
                inputs=all_inputs,
                dependencies=("predecessor",),
                cache_mode="run",
            ),
            PhaseSpec(
                name="builder",
                commands=(
                    CommandSpec(
                        tool="python",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/build.py",
                            "--output-root",
                            "{phase_root}/out",
                            "--cargo",
                            "{input:cargo-bin}",
                            "--rustc",
                            "{dependency:toolchain}/out/toolchain/bin/rustc",
                            "--run-behavior",
                        ),
                        environment=cargo_environment,
                        label="python-builder-and-compiled-behavior",
                    ),
                ),
                inputs=all_inputs,
                dependencies=("predecessor", "harness", "toolchain"),
                cache_mode="run",
            ),
            PhaseSpec(
                name="validator",
                commands=(
                    CommandSpec(
                        tool="python",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/validate.py",
                            "--artifact",
                            "{dependency:builder}/out/evidence-gates.v1.json",
                            "--artifact-cbor",
                            "{dependency:builder}/out/evidence-gates.v1.cbor",
                            "--output-root",
                            "{phase_root}/out",
                            "--cargo",
                            "{input:cargo-bin}",
                            "--rustc",
                            "{dependency:toolchain}/out/toolchain/bin/rustc",
                        ),
                        environment=cargo_environment,
                        label="python-semantic-reexecution",
                    ),
                ),
                inputs=all_inputs,
                dependencies=("builder", "toolchain"),
                cache_mode="run",
            ),
            PhaseSpec(
                name="ruby",
                commands=(
                    CommandSpec(
                        tool="ruby",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/verify.rb",
                            "--artifact",
                            "{dependency:builder}/out/evidence-gates.v1.json",
                            "--artifact-cbor",
                            "{dependency:builder}/out/evidence-gates.v1.cbor",
                            "--output-root",
                            "{phase_root}/out",
                            "--cargo",
                            "{input:cargo-bin}",
                            "--rustc",
                            "{dependency:toolchain}/out/toolchain/bin/rustc",
                        ),
                        environment=cargo_environment,
                        label="ruby-independent-reexecution",
                    ),
                ),
                inputs=all_inputs,
                dependencies=("builder", "toolchain"),
                cache_mode="run",
            ),
            PhaseSpec(
                name="consensus",
                commands=(
                    CommandSpec(
                        tool="python",
                        args=(
                            "{input:workspace-snapshot}/tools/vnext_contracts/stage5/evidence_gates/consensus.py",
                            "--artifact",
                            "{dependency:builder}/out/evidence-gates.v1.json",
                            "--builder",
                            "{dependency:builder}/out/python-builder-receipt.v1.json",
                            "--validator",
                            "{dependency:validator}/out/semantic-validation-receipt.v1.json",
                            "--ruby",
                            "{dependency:ruby}/out/ruby-verification-receipt.v1.json",
                            "--predecessor",
                            "{dependency:predecessor}/out/predecessor-closure.v1.json",
                            "--predecessor-source",
                            "{dependency:predecessor}/out/stage4-source.tar.gz",
                            "--harness",
                            "{dependency:harness}/out/proof-harness-receipt.v1.json",
                            "--snapshot-manifest",
                            "{input:workspace-snapshot}/snapshot-manifest.v1.json",
                            "--toolchain",
                            "{dependency:toolchain}/out/rust-toolchain-closure.v1.json",
                            "--target",
                            target,
                            "--output-root",
                            "{phase_root}/out",
                        ),
                        label="three-engine-consensus-before-publication",
                    ),
                ),
                inputs=all_inputs,
                dependencies=(
                    "toolchain",
                    "predecessor",
                    "harness",
                    "builder",
                    "validator",
                    "ruby",
                ),
                cache_mode="run",
            ),
        ),
        environment=proof_environment,
        publication=PublicationSpec(
            release_root=publication_root,
            pointer_path=pointer,
            outputs=(
                PublishedOutput(
                    "toolchain",
                    "out/rust-toolchain-closure.v1.json",
                    "rust-toolchain-closure.v1.json",
                ),
                PublishedOutput(
                    "harness",
                    "out/proof-harness-receipt.v1.json",
                    "proof-harness-receipt.v1.json",
                ),
                PublishedOutput(
                    "predecessor",
                    "out/predecessor-closure.v1.json",
                    "predecessor-closure.v1.json",
                ),
                PublishedOutput(
                    "predecessor",
                    "out/stage4-source.tar.gz",
                    "stage4-source.tar.gz",
                ),
                PublishedOutput("builder", "out/evidence-gates.v1.json", "evidence-gates.v1.json"),
                PublishedOutput("builder", "out/evidence-gates.v1.cbor", "evidence-gates.v1.cbor"),
                PublishedOutput(
                    "builder",
                    "out/python-builder-receipt.v1.json",
                    "python-builder-receipt.v1.json",
                ),
                PublishedOutput(
                    "validator",
                    "out/semantic-validation-receipt.v1.json",
                    "semantic-validation-receipt.v1.json",
                ),
                PublishedOutput(
                    "ruby",
                    "out/ruby-verification-receipt.v1.json",
                    "ruby-verification-receipt.v1.json",
                ),
                PublishedOutput(
                    "consensus",
                    "out/three-engine-consensus-receipt.v1.json",
                    "three-engine-consensus-receipt.v1.json",
                ),
                PublishedOutput(
                    "consensus",
                    "out/workspace-snapshot-manifest.v1.json",
                    "workspace-snapshot-manifest.v1.json",
                ),
            ),
        ),
    )
    result = ProofEngine().execute(
        plan,
        run_root=run_root,
        cache_root=cache_root,
        run_token=token,
        max_workers=1,
        performance_log=performance_log,
    )
    print(
        json.dumps(
            {
                "performance_log": str(performance_log),
                "phase_cache": {phase.name: phase.cache_status for phase in result.phases},
                "plan_identity": result.plan_identity,
                "publication_identity": result.publication_identity,
                "run_token": result.run_token,
                "workspace_snapshot": str(workspace_snapshot),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
