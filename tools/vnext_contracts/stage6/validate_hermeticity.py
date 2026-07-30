#!/usr/bin/env python3
"""Reject external I/O in the pure Stage-6 domain and transport surfaces."""

from pathlib import Path

FILES = (
    "src/domain/vnext/capability/generated_catalog/catalog.rs",
    "src/domain/vnext/projection/engine.rs",
    "src/domain/vnext/transport/json.rs",
    "src/operations/vnext/action/service.rs",
)
FORBIDDEN = (
    "std::process::Command",
    "std::net::",
    "TcpStream",
    "UdpSocket",
    "reqwest::",
    "ureq::",
    "File::create",
    "OpenOptions",
    "write_all(",
    "remove_file(",
    "rename(",
)


def main() -> None:
    root = Path(__file__).resolve().parents[3]
    findings: list[str] = []
    for relative in FILES:
        source = (root / relative).read_text(encoding="utf-8")
        for token in FORBIDDEN:
            if token in source:
                findings.append(f"{relative}: {token}")
    if findings:
        raise SystemExit("non-hermetic Stage-6 source:\n" + "\n".join(findings))
    print("stage6 hermeticity: ok")


if __name__ == "__main__":
    main()
