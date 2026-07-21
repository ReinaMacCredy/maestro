#!/usr/bin/env python3
"""Materialize the minimal immutable Rust compiler closure used by Stage 5."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import tempfile
from pathlib import Path


TARGET = re.compile(r"[a-z0-9_]+(?:-[a-z0-9_]+)+")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def read_regular(path: Path) -> tuple[bytes, bool]:
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise RuntimeError(f"toolchain input is not a regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        opened = os.fstat(descriptor)
        chunks = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
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
    ):
        raise RuntimeError(f"toolchain input changed while it was read: {path}")
    data = b"".join(chunks)
    if len(data) != opened.st_size:
        raise RuntimeError(f"toolchain input length changed while it was read: {path}")
    return data, bool(opened.st_mode & 0o111)


def copy_exact(source: Path, destination: Path, *, executable: bool | None = None) -> list[object]:
    data, source_executable = read_regular(source)
    installed_executable = source_executable if executable is None else executable
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("xb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    destination.chmod(0o755 if installed_executable else 0o644)
    if read_regular(source)[0] != data or destination.read_bytes() != data:
        raise RuntimeError(f"toolchain input changed while it was copied: {source}")
    return [destination.as_posix(), len(data), sha256(data), installed_executable]


def target_library_sources(root: Path) -> list[Path]:
    if root.is_symlink() or not root.is_dir():
        raise RuntimeError("Rust target library closure is absent or unsafe")
    sources = []
    for child in sorted(root.iterdir()):
        if child.is_symlink() or not child.is_file():
            raise RuntimeError(f"Rust target library closure contains an unsafe entry: {child}")
        sources.append(child)
    if not sources:
        raise RuntimeError("Rust target library closure is empty")
    return sources


def verify_developer_toolchain_execution(toolchain: Path) -> None:
    cc = toolchain / "bin/cc"
    ar = toolchain / "bin/ar"
    ranlib = toolchain / "bin/ranlib"
    with tempfile.TemporaryDirectory(prefix="maestro-stage5-materialized-clt-") as directory:
        probe_root = Path(directory)
        source = probe_root / "probe.c"
        object_file = probe_root / "probe.o"
        archive = probe_root / "libprobe.a"
        source.write_text("int maestro_stage5_probe(void) { return 5; }\n", encoding="ascii")
        commands = (
            [str(cc), "-c", str(source), "-o", str(object_file)],
            [str(ar), "rcs", str(archive), str(object_file)],
            [str(ranlib), str(archive)],
        )
        for command in commands:
            completed = subprocess.run(command, capture_output=True, check=False)
            if completed.returncode != 0:
                raise RuntimeError(
                    "materialized developer toolchain is not executable: "
                    + completed.stderr[-4_000:].decode("utf-8", errors="replace")
                )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rustc", type=Path, required=True)
    parser.add_argument("--cargo", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--ruby", type=Path, required=True)
    parser.add_argument("--cc", type=Path, required=True)
    parser.add_argument("--cxx", type=Path, required=True)
    parser.add_argument("--ar", type=Path, required=True)
    parser.add_argument("--ranlib", type=Path, required=True)
    parser.add_argument("--lib-lto", type=Path, required=True)
    parser.add_argument("--git", type=Path, required=True)
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--target-lib", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()
    if TARGET.fullmatch(args.target) is None:
        raise RuntimeError("Rust target triple is invalid")
    if not args.driver.name.startswith("librustc_driver-") or args.driver.suffix != ".dylib":
        raise RuntimeError("Rust compiler driver identity is invalid")
    toolchain = args.output_root / "toolchain"
    if toolchain.exists() or toolchain.is_symlink():
        raise RuntimeError("Rust toolchain output already exists")
    rows = [
        copy_exact(args.rustc, toolchain / "bin/rustc", executable=True),
        copy_exact(args.cargo, toolchain / "bin/cargo", executable=True),
        copy_exact(args.python, toolchain / "bin/python3", executable=True),
        copy_exact(args.ruby, toolchain / "bin/ruby", executable=True),
        copy_exact(args.cc, toolchain / "bin/cc", executable=True),
        copy_exact(args.cxx, toolchain / "bin/c++", executable=True),
        copy_exact(args.ar, toolchain / "bin/ar", executable=True),
        copy_exact(args.ranlib, toolchain / "bin/ranlib", executable=True),
        copy_exact(args.lib_lto, toolchain / "lib/libLTO.dylib", executable=False),
        copy_exact(args.git, toolchain / "bin/git", executable=True),
        copy_exact(args.driver, toolchain / "lib" / args.driver.name, executable=False),
    ]
    for source in target_library_sources(args.target_lib):
        rows.append(
            copy_exact(
                source,
                toolchain / "lib/rustlib" / args.target / "lib" / source.name,
                executable=False,
            )
        )
    verify_developer_toolchain_execution(toolchain)
    normalized_rows = [
        [Path(str(row[0])).relative_to(args.output_root).as_posix(), *row[1:]] for row in rows
    ]
    normalized_rows.sort(key=lambda row: str(row[0]))
    receipt = {
        "files": normalized_rows,
        "identity": f"sha256:{sha256(canonical_json(normalized_rows))}",
        "schema_version": "maestro.vnext.stage5.rust-toolchain-closure.v1",
        "target": args.target,
    }
    (args.output_root / "rust-toolchain-closure.v1.json").write_bytes(canonical_json(receipt))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
