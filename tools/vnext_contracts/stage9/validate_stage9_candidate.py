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
INTEGRATION_COMMIT = "36076d45da6ffa7934aef4f7ccf6fdd9b2b13340"
INTEGRATION_TREE = "a3f31252d4a5e32f6b8b5cc4a2b944efd37248da"
CANONICAL_MERGE = "78133fe05a08976b0d2092f853ed6fcab1a806f4"
LOCATOR_SEED = ROOT / "src/domain/persistence/protected_locator_stage9_seed.rs"
LOCATOR_FACADE = ROOT / "src/domain/persistence/mod.rs"
FINALITY_SEED = ROOT / "src/domain/installation/durable_finality_stage9_seed.rs"
INSTALLATION_FACADE = ROOT / "src/domain/installation/mod.rs"
DIAGNOSTIC_SEED = ROOT / "src/domain/persistence/protected_diagnostic_stage9_seed.rs"
FIXTURE = ROOT / "tests/fixtures/vnext/stage9/distribution-installation-proof-input.v1.json"

CANONICAL_SOURCE_SHA256 = {
    "src/domain/distribution/runtime/catalog.rs": "c0e83caa58fd8a3c2384f35b177c7cd5d2bda624176d6e9bb3154abfe94a9164",
    "src/domain/distribution/runtime/custody.rs": "d2c859cc39c2166f7d55dc7f3a5ba28539357ca4573caca301cd8b6b7e327f36",
    "src/domain/distribution/runtime/mod.rs": "74c9427bc55d0754da9108e297b74bff8259db048b8c8070560eb1ed4836e31d",
    "src/domain/distribution/runtime/model.rs": "0f3760fbd90785879bea72e486e8be3b2601f4eee2771b752bc67412530f56ee",
    "src/domain/distribution/runtime/owner_facts.rs": "5f1dc86d2e4cabae9729a5b7599b40a222797094901bff75a972d50e8deda18e",
    "src/domain/distribution/runtime/records.rs": "abae64a98bf1e6429306489eb5461b16da2d24ecd342eed81831b8ac92fe3097",
    "src/domain/distribution/runtime/transaction.rs": "99f5dd9971ddc0dfa2dc46ffcc343c3d8c97f36a5a85ac42a072b293a5af900f",
    "src/domain/installation/census.rs": "38d5923904bbed7621f82542a46884ea0675de78768342faa845623f7626526f",
    "src/domain/installation/closure.rs": "bf44711633bafcfc2505f28127ec05fd0c1e88b15c3547c78cc8662ac152c64f",
    "src/domain/installation/consumer_materialization.rs": "8a55d73e2680a37c7626aa0add67b1d68d0299f8f5517a4e8fffd449f3d3cb75",
    "src/domain/installation/currentness.rs": "37db63d7644f465d114263c0b85bda5c8e846cd262e61d854ed45f62fd623a44",
    "src/domain/installation/cutover.rs": "ba8567ac533c539a5b8f3493bebab090843447d7320c09f851424815000004d0",
    "src/domain/installation/durable_finality.rs": "cd6bfea5b763d2f2a716c7de7f7474092643cad5115d2fd6d6fb74339f894720",
    "src/domain/installation/durable_finality_stage9_seed.rs": "ed0ce18ae28f5ce10b3005b185cdf9fd7df039d6e2edf2e476fd1f8addeef627",
    "src/domain/installation/mod.rs": "96e0adc3a6242203491aac1dde041c049f3e4bc7bc22290ba3f2ebed462439b4",
    "src/domain/persistence/mod.rs": "437699cbe16752592df9db1f5391411fef0441fb22f6078b678d51e2ad428a26",
    "src/domain/persistence/protected_diagnostic_stage9_seed.rs": "12fae00cbb1d7f324edb1d91954b3d4eeda1cb098087dbd2a9ec390ae3b4f9dd",
    "src/domain/persistence/protected_locator_lease.rs": "7dfdb74bcfbe5cd039195aae336c04706e103a867b854fa154a737abbce67eca",
    "src/domain/persistence/protected_locator_stage9_seed.rs": "5e1483ca007d7215027eef4c4921e02a5d443a7eb225f8e77cf6a8cb9f3be767",
    "src/operations/installation/active.rs": "8481c8254fa9f663d7a4c07ca7a120f7884c79a1cddfa74e464e27cb4f604af2",
    "src/operations/installation/agent_resource_release.rs": "143edd66d45a61208bd3fbb441098a2ebca34507a30ac172dd99eab0a4b9c4b5",
    "src/operations/installation/effects.rs": "86c894b78baeeff6149a4591796ba4d84428ebe9c2a8e6ae2ac20f4dfedecf82",
    "src/operations/installation/mod.rs": "5649e94819182902f7731ccc356e1011d0d0e48970f85f0afd547c155fb719ee",
    "src/operations/installation/prestore.rs": "53cd125c7da191f2f542e76d6bd5055199b566addc74cc5378aec8c1060c2fa8",
    "tests/vnext_stage9_distribution.rs": "6832562e653aa7258b0ba0f418c28324a7882335121fd937b3c2be215f517623",
    "tests/vnext_stage9_installation.rs": "5f86e6c9e128adb3bf2ca9fb673c231f0b444215bcf87334edc8afb42b4bd3cb",
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


def validate_paths():
    for relative, expected in CANONICAL_SOURCE_SHA256.items():
        source = ROOT / relative
        mode = source.lstat().st_mode
        assert stat.S_ISREG(mode) and not mode & stat.S_IXUSR, relative
        assert sha256(source) == expected, relative
        legacy = relative.replace("src/domain/", "src/domain/vnext/", 1).replace(
            "src/operations/", "src/operations/vnext/", 1
        )
        if legacy != relative:
            assert not (ROOT / legacy).exists(), legacy


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
    production = "\n".join((locator, locator_facade, finality))
    forbidden = (
        "Box<dyn",
        "unsafe {",
        "unsafe fn",
        "static mut",
        "AmbientStore",
        "GLOBAL_STORE",
        "bind_stage9_owner_provider",
    )
    for needle in forbidden:
        assert needle not in production, needle
    assert "trait Stage9ProtectedLocatorProviderV2" not in locator + locator_facade
    assert "trait Stage9ActiveStoreFinalityProviderV2" not in finality + installation
    assert "PreStoreCutoverCandidateV1" not in locator + locator_facade
    assert "#[cfg(test)]\npub(in crate::domain) struct Stage9ActiveStoreFinalitySeedV1" in finality
    assert "#[cfg(test)]\npub(in crate::domain::persistence) struct Stage9ProtectedLocatorBackendSeedV1" in locator
    capture_signature = locator.split("fn acquire_stage9_backend_v2<'locator>(", 1)[1].split(
        ") -> Result", 1
    )[0]
    for forbidden_input in ("candidate", "expected_old", "cas", "digest", "root:"):
        assert forbidden_input not in capture_signature, forbidden_input


def main():
    assert git_output("merge-base", INTEGRATION_COMMIT, "HEAD") == INTEGRATION_COMMIT
    assert git_output("rev-parse", f"{INTEGRATION_COMMIT}^{{tree}}") == INTEGRATION_TREE
    assert git_output("merge-base", CANONICAL_MERGE, "HEAD") == CANONICAL_MERGE
    validate_paths()

    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    assert fixture["candidate_state"] == "canonical_namespace_integrated_unverified"
    assert fixture["base_commit"] == BASE
    assert fixture["integration_commit"] == INTEGRATION_COMMIT
    assert fixture["integration_tree"] == INTEGRATION_TREE
    assert fixture["canonical_merge"] == CANONICAL_MERGE

    locator = LOCATOR_SEED.read_text(encoding="utf-8")
    locator_facade = LOCATOR_FACADE.read_text(encoding="utf-8")
    finality = FINALITY_SEED.read_text(encoding="utf-8")
    installation = INSTALLATION_FACADE.read_text(encoding="utf-8")
    diagnostic = DIAGNOSTIC_SEED.read_text(encoding="utf-8")
    validate_mutants(locator, locator_facade, finality, installation, diagnostic)
    validate_forbidden_shapes(locator, locator_facade, finality, installation)
    print(
        json.dumps(
            {
                "authority_state": "none",
                "canonical_source_count": len(CANONICAL_SOURCE_SHA256),
                "candidate_state": fixture["candidate_state"],
                "mutant_count": 13,
                "status": "pass",
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, KeyError, subprocess.CalledProcessError) as error:
        print(f"stage9 V4 static validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
