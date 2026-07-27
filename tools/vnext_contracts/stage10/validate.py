#!/usr/bin/env python3
"""Validate the hermetic Stage-10 V4 preparation slice."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
BASE = "89b29639c75d1d83f58b33489320ec690af6b845"
REQUIRED = (
    "embedded/vnext/bootstrap/wiring.v1.json",
    "embedded/vnext/connectors/read-only-global-mcp.v1.json",
    "embedded/vnext/connectors/installation-bound-cli.v1.json",
    "embedded/vnext/hosts/agents-compatible-cli.v1.json",
    "embedded/vnext/hosts/claude-code.v1.json",
    "embedded/vnext/patterns/trusted-host-diagnostic.v1.json",
    "tools/vnext_contracts/stage10/proof-matrix.v1.json",
    "tools/vnext_contracts/stage10/interface-gap.v2.json",
)
FORBIDDEN_SOURCE = (
    "trusted_host_diagnostic_stage10_seed",
    "LiveAuthenticatedHostConnectionV1",
)


def changed_paths() -> set[str]:
    tracked = subprocess.run(
        [
            "git",
            "diff",
            "--name-only",
            BASE,
        ],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    return {
        *{line for line in tracked.stdout.splitlines() if line},
        *{line for line in untracked.stdout.splitlines() if line},
    }


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    for relative in REQUIRED:
        path = ROOT / relative
        if not path.is_file():
            raise SystemExit(f"missing required Stage-10 path: {relative}")
        json.loads(path.read_text())

    adapter_sources = (
        "src/interfaces/vnext/connectors/mod.rs",
        "src/interfaces/vnext/mcp/mod.rs",
        "src/operations/vnext/adapters/mod.rs",
    )
    for relative in adapter_sources:
        source = (ROOT / relative).read_text()
        for forbidden in FORBIDDEN_SOURCE:
            if forbidden in source:
                raise SystemExit(f"forbidden V1 trusted-host source in {relative}: {forbidden}")

    adapter = (ROOT / "src/operations/vnext/adapters/mod.rs").read_text()
    mcp = (ROOT / "src/interfaces/vnext/mcp/mod.rs").read_text()
    required_provider = "&mut dyn ProtectedDiagnosticCurrentViewProviderV1"
    optional_provider = "Option<&mut dyn ProtectedDiagnosticCurrentViewProviderV1>"
    for relative, source in (
        ("src/operations/vnext/adapters/mod.rs", adapter),
        ("src/interfaces/vnext/mcp/mod.rs", mcp),
    ):
        if required_provider not in source:
            raise SystemExit(f"Stage-10 adapter does not require the sealed provider: {relative}")
        if optional_provider in source or "Stage9CurrentnessUnavailable" in source:
            raise SystemExit(f"Stage-10 adapter retains the superseded optional provider: {relative}")

    gap = json.loads((ROOT / "tools/vnext_contracts/stage10/interface-gap.v2.json").read_text())
    if gap["status"] != "stage9-currentness-provider-bound":
        raise SystemExit("Stage-10 must bind the real Stage-9 currentness provider")
    if gap["satisfied_by_upstream"] != [
        "TrustedHostDiagnosticConnectionPortV1",
        "AuthorityFacadeV1::protected_continuity_diagnostic_with_ports",
        "Stage9OwnerLocalCurrentViewProviderV1: ProtectedDiagnosticCurrentViewProviderV1",
    ]:
        raise SystemExit("Stage-10 upstream trusted-host/currentness binding drifted")
    if gap["required_before_runtime_activation"] or gap["runtime_activation"]:
        raise SystemExit("Stage-10 provider binding must not claim top-level runtime activation")

    manifest = json.loads((ROOT / "tools/vnext_contracts/stage10/path-manifest.v1.json").read_text())
    if manifest["base"] != BASE:
        raise SystemExit("Stage-10 path manifest predecessor drifted")
    allowed = set(manifest["modified"]) | set(manifest["added"])
    if len(allowed) != 29:
        raise SystemExit(f"Stage-10 must own exactly 29 paths, found {len(allowed)}")
    changed = changed_paths()
    if changed != allowed:
        raise SystemExit(f"Stage-10 changed-path manifest mismatch: {sorted(changed ^ allowed)}")
    for prefix in manifest["denylist"]:
        if any(path == prefix or path.startswith(prefix) for path in changed):
            raise SystemExit(f"Stage-10 denylist violation: {prefix}")

    fanout = json.loads((ROOT / "tools/vnext_contracts/fanout/fanout-base.v1.json").read_text())
    stage = next(owner for owner in fanout["stage_owners"] if owner["stage"] == 10)
    mutable = set(stage["mutable_seed_files"]) | set(stage["inherited_mutable_seed_files"])
    prefixes = tuple(stage["write_prefixes"])
    denied_exact = set(fanout["shared_denylist"]["exact_files"])
    denied_prefixes = tuple(fanout["shared_denylist"]["path_prefixes"])
    for path in manifest["modified"]:
        if path not in mutable:
            raise SystemExit(f"Stage-10 modified path is not a mutable seed: {path}")
    for path in manifest["added"]:
        if not path.startswith(prefixes):
            raise SystemExit(f"Stage-10 added path is outside owned prefixes: {path}")
    for path in changed:
        denied = path in denied_exact or path.startswith(denied_prefixes)
        if denied and path not in stage["inherited_mutable_seed_files"]:
            raise SystemExit(f"Stage-10 changed path hits the shared denylist: {path}")

    parity = json.loads(
        (ROOT / "tests/fixtures/vnext/stage10/trusted-host-parity.v1.json").read_text()
    )
    if parity["schema"] != "maestro.vnext.stage10.trusted-host-parity.v2":
        raise SystemExit("Stage-10 trusted-host parity schema drifted")
    for reference in parity["upstream_references"]:
        path = ROOT / reference["path"]
        if digest(path) != reference["sha256"]:
            raise SystemExit(f"Stage-10 upstream parity drifted: {reference['path']}")
    if parity["runtime_activation"]:
        raise SystemExit("Stage-10 parity fixture must not claim top-level runtime activation")

    stage9 = (
        ROOT / "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs"
    ).read_text()
    provider_definition = "struct Stage9OwnerLocalCurrentViewProviderV1"
    provider_impl = (
        "impl ProtectedDiagnosticCurrentViewProviderV1 "
        "for Stage9OwnerLocalCurrentViewProviderV1"
    )
    if provider_definition not in stage9 or provider_impl not in stage9:
        raise SystemExit("Stage-9 production currentness provider is absent")
    cfg_test = stage9.find("#[cfg(test)]")
    if cfg_test != -1 and stage9.find(provider_definition) > cfg_test:
        raise SystemExit("Stage-9 currentness provider is test-only")

    print(
        json.dumps(
            {
                "schema": "maestro.external.stage10-validation.v2",
                "base": BASE,
                "changed_path_count": len(changed),
                "currentness_provider": "stage9-production-bound",
                "runtime_activation": False,
                "status": "integrated_unverified",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
