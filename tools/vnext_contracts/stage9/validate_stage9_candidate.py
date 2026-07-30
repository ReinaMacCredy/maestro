#!/usr/bin/env python3
"""Validate the exact Stage 9 V4 real-provider candidate boundary."""

import hashlib
import json
import pathlib
import stat
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
BASE = "a14b782a4aea161a10bbd3b194c8396eab6011e8"
LOCATOR_SEED = ROOT / "src/domain/vnext/persistence/protected_locator_stage9_seed.rs"
LOCATOR_FACADE = ROOT / "src/domain/vnext/persistence/mod.rs"
LOCATOR_CORE = ROOT / "src/domain/vnext/persistence/protected_locator_lease.rs"
FINALITY_SEED = ROOT / "src/domain/vnext/installation/durable_finality_stage9_seed.rs"
INSTALLATION_FACADE = ROOT / "src/domain/vnext/installation/mod.rs"
FINALITY_CORE = ROOT / "src/domain/vnext/installation/durable_finality.rs"
DIAGNOSTIC_SEED = ROOT / "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs"
FIXTURE = ROOT / "tests/fixtures/vnext/stage9/distribution-installation-proof-input.v1.json"

APPROVED_PATHS = {
    "src/domain/vnext/distribution/runtime/catalog.rs": "A",
    "src/domain/vnext/distribution/runtime/custody.rs": "A",
    "src/domain/vnext/distribution/runtime/mod.rs": "M",
    "src/domain/vnext/distribution/runtime/model.rs": "A",
    "src/domain/vnext/distribution/runtime/records.rs": "A",
    "src/domain/vnext/distribution/runtime/transaction.rs": "A",
    "src/domain/vnext/installation/census.rs": "A",
    "src/domain/vnext/installation/closure.rs": "A",
    "src/domain/vnext/installation/consumer_materialization.rs": "A",
    "src/domain/vnext/installation/currentness.rs": "A",
    "src/domain/vnext/installation/cutover.rs": "A",
    "src/domain/vnext/installation/durable_finality_stage9_seed.rs": "M",
    "src/domain/vnext/installation/mod.rs": "M",
    "src/domain/vnext/persistence/mod.rs": "M",
    "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs": "M",
    "src/domain/vnext/persistence/protected_locator_stage9_seed.rs": "M",
    "src/operations/vnext/installation/active.rs": "A",
    "src/operations/vnext/installation/effects.rs": "A",
    "src/operations/vnext/installation/mod.rs": "M",
    "src/operations/vnext/installation/prestore.rs": "A",
    "tests/fixtures/vnext/stage9/distribution-installation-proof-input.v1.json": "A",
    "tests/vnext_stage9_distribution.rs": "A",
    "tests/vnext_stage9_installation.rs": "A",
    "tools/vnext_contracts/stage9/validate_stage9_candidate.py": "A",
}


def git_output(*args):
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def base_sha256(path):
    return hashlib.sha256(
        subprocess.run(
            ["git", "show", f"{BASE}:{path.relative_to(ROOT)}"],
            cwd=ROOT,
            check=True,
            capture_output=True,
        ).stdout
    ).hexdigest()


def candidate_changes():
    rows = {}
    for line in git_output("diff", "--no-renames", "--name-status", BASE).splitlines():
        status, path = line.split("\t", maxsplit=1)
        rows[path] = status
    for path in git_output("ls-files", "--others", "--exclude-standard").splitlines():
        rows[path] = "A"
    return rows


def validate_paths():
    changes = candidate_changes()
    assert changes == APPROVED_PATHS, (
        sorted(set(changes) - set(APPROVED_PATHS)),
        sorted(set(APPROVED_PATHS) - set(changes)),
        sorted((path, changes.get(path), status) for path, status in APPROVED_PATHS.items()
               if changes.get(path) != status),
    )
    for path in APPROVED_PATHS:
        mode = (ROOT / path).lstat().st_mode
        assert stat.S_ISREG(mode) and not mode & stat.S_IXUSR, path


def real_provider_contract_holds(locator, locator_facade, finality, installation, diagnostic):
    required = (
        (locator, "struct Stage9ProtectedLocatorBackendSeedV2<'locator>"),
        (locator, "ProtectedLocatorAcquisitionRequestV2::from_stage9_owner"),
        (locator, "ProtectedLocatorObservedStateV2::from_stage9_owner"),
        (locator, "ProtectedLocatorCandidateStateV2::from_stage9_owner"),
        (locator, "consume_stage9_dispatch_projection()"),
        (locator, "matches_exact_owner_effect("),
        (locator, ".issue_request("),
        (locator, "CeremonyRequestModeV1::ResolveResult"),
        (locator, "self.store.publish(request)"),
        (locator, "ProtectedLocatorFinalReadbackV2::exact_candidate_from_stage9_owner"),
        (locator, "fn acquire_stage9_backend_v2<'locator>("),
        (locator_facade, "fn capture_pre_candidate<'locator>("),
        (locator_facade, "fn acquire_pre_candidate<'lease, 'provider>("),
        (finality, "struct Stage9ActiveStoreFinalityProviderV2"),
        (finality, "store: StoreV1"),
        (finality, "fn capture_owner_publication_v2("),
        (finality, "store.replay_idempotency(&probe)"),
        (finality, "publish_generation_atomically_with_prepare"),
        (finality, "coherent_publication_snapshot()"),
        (finality, ".consume_stage9_owner_view()?"),
        (finality, ".validate_committed_readback("),
        (finality, "impl ActiveStoreFinalityOwnerV2 for Stage9ActiveStoreFinalitySeedV2"),
        (installation, "fn execute_active("),
        (diagnostic, "Stage9OwnerLocalCurrentViewProviderV1"),
        (diagnostic, "coherent_publication_snapshot()"),
    )
    return all(needle in source for source, needle in required)


def validate_mutants(locator, locator_facade, finality, installation, diagnostic):
    sources = (locator, locator_facade, finality, installation, diagnostic)
    assert real_provider_contract_holds(*sources)
    mutants = (
        (0, "consume_stage9_dispatch_projection()", "removed_projection()"),
        (0, "matches_exact_owner_effect(", "removed_owner_match("),
        (0, ".issue_request(", ".removed_issue_request("),
        (0, "self.store.publish(request)", "removed_store_publish(request)"),
        (0, "ProtectedLocatorFinalReadbackV2::exact_candidate_from_stage9_owner",
            "RemovedFinalReadbackFactory"),
        (0, "ProtectedLocatorObservedStateV2::from_stage9_owner", "RemovedObservedFactory"),
        (1, "fn capture_pre_candidate<'locator>(", "fn removed_capture<'locator>("),
        (2, "store.replay_idempotency(&probe)", "removed_replay(&probe)"),
        (2, "publish_generation_atomically_with_prepare", "removed_atomic_publication"),
        (2, "coherent_publication_snapshot()", "removed_store_snapshot()"),
        (2, ".consume_stage9_owner_view()?", ".removed_owner_view()?"),
        (2, ".validate_committed_readback(", ".removed_readback_validation("),
        (4, "coherent_publication_snapshot()", "removed_diagnostic_snapshot()"),
    )
    for index, needle, replacement in mutants:
        mutated = list(sources)
        assert needle in mutated[index], (index, needle)
        mutated[index] = mutated[index].replace(needle, replacement)
        assert not real_provider_contract_holds(*mutated), (
            "Stage 9 real-provider mutant survived",
            needle,
        )


def validate_forbidden_shapes(locator, locator_facade, finality, installation):
    production = "\n".join((locator, locator_facade, finality, installation))
    forbidden = (
        "Box<dyn",
        "unsafe {",
        "unsafe fn",
        "static mut",
        "AmbientStore",
        "GLOBAL_STORE",
        "bind_stage9_owner_provider",
        "trait Stage9ProtectedLocatorProviderV2",
        "trait Stage9ActiveStoreFinalityProviderV2",
    )
    for needle in forbidden:
        assert needle not in production, needle
    assert "PreStoreCutoverCandidateV1" not in locator + locator_facade
    assert "#[cfg(test)]\npub(in crate::domain::vnext) struct Stage9ActiveStoreFinalitySeedV1" in finality
    assert "#[cfg(test)]\npub(in crate::domain::vnext::persistence) struct Stage9ProtectedLocatorBackendSeedV1" in locator
    capture_signature = locator.split("fn acquire_stage9_backend_v2<'locator>(", 1)[1].split(
        ") -> Result", 1
    )[0]
    for forbidden_input in ("candidate", "expected_old", "cas", "digest", "root:"):
        assert forbidden_input not in capture_signature, forbidden_input


def main():
    assert git_output("merge-base", BASE, "HEAD") == BASE
    validate_paths()
    for frozen in (LOCATOR_CORE, FINALITY_CORE):
        assert sha256(frozen) == base_sha256(frozen), frozen

    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    assert fixture["candidate_state"] == "v4_real_provider_candidate_ready"
    assert fixture["base_commit"] == BASE

    locator = LOCATOR_SEED.read_text(encoding="utf-8")
    locator_facade = LOCATOR_FACADE.read_text(encoding="utf-8")
    finality = FINALITY_SEED.read_text(encoding="utf-8")
    installation = INSTALLATION_FACADE.read_text(encoding="utf-8")
    diagnostic = DIAGNOSTIC_SEED.read_text(encoding="utf-8")
    validate_mutants(locator, locator_facade, finality, installation, diagnostic)
    validate_forbidden_shapes(locator, locator_facade, finality, installation)


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, KeyError, subprocess.CalledProcessError) as error:
        print(f"stage9 V4 static validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
