#!/usr/bin/env python3
"""Read-only Stage-8 ownership, hermeticity, and mutant-proof preflight."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


DEFAULT_BASE = "d9442aa6fda9b21d0cd60536b06ffca5c03d0645"
BASE_MANIFEST_IDENTITY = (
    "sha256:b7fe6a736c906ccbe8eb830348c63b62b8562f4c7799d2cd87a606e6b12e7393"
)
V4_FANOUT_MANIFEST_SHA256 = (
    "e299556c31c6a788285d984f9cd3040cfde200ba24e7ed5a5d90caff96ee5954"
)
V4_DESIGN_COMMIT = "f6b4af285b0b24b1192e636f34c2b0c7b6bc1b6d"
V4_DESIGN_TREE = "51ff828b67af7d59e1aa915f9122479e4eed1c31"
V4_CANONICAL_INTEGRATION_ORDER = (6, 7, 8, 9, 10, 11, 12)
PRESERVED_CANDIDATE = "702577f1d61ce8c5acd84218e526c4167d84529c"
ALLOWED_STATUSES = frozenset({"A", "M"})
ALLOWED_MODES = frozenset({"100644", "100755"})
EXECUTABLE_SUFFIXES = (".py", ".rb", ".sh")
OWNED_PREFIXES = (
    "src/domain/vnext/capability/runtime/",
    "src/domain/vnext/evidence/diagnostics/",
    "src/domain/vnext/intake/",
    "src/domain/vnext/maturity/",
    "src/domain/vnext/memory/",
    "src/domain/vnext/research/",
    "src/domain/vnext/search/",
    "src/operations/vnext/observation/",
    "tests/fixtures/vnext/stage8/",
    "tests/vnext_stage8_",
    "tools/vnext_contracts/stage8/",
)
INHERITED_SEED = (
    "src/domain/vnext/authority/"
    "protected_diagnostic_envelope_stage8_seed.rs"
)
MUTABLE_SEEDS = frozenset(
    {
        "src/domain/vnext/capability/runtime/mod.rs",
        "src/domain/vnext/evidence/diagnostics/mod.rs",
        "src/domain/vnext/intake/mod.rs",
        "src/domain/vnext/maturity/mod.rs",
        "src/domain/vnext/memory/mod.rs",
        "src/domain/vnext/research/mod.rs",
        "src/domain/vnext/search/mod.rs",
        "src/operations/vnext/observation/mod.rs",
    }
)
RUST_SOURCES = (
    INHERITED_SEED,
    "src/domain/vnext/search/mod.rs",
    "src/domain/vnext/memory/mod.rs",
    "src/domain/vnext/intake/mod.rs",
    "src/domain/vnext/research/mod.rs",
    "src/domain/vnext/capability/runtime/mod.rs",
    "src/domain/vnext/maturity/mod.rs",
    "src/domain/vnext/evidence/diagnostics/mod.rs",
    "src/operations/vnext/observation/mod.rs",
)
FORBIDDEN_RUST = (
    "std::fs",
    "std::net",
    "std::process",
    "Command::",
    "env::var",
    "Serialize",
    "Deserialize",
    "ScopeAtom",
    "ActionSpec",
)


class ValidationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def changed_entries(root: Path, base: str) -> list[tuple[str, str, str]]:
    result = subprocess.run(
        ["git", "diff", "--raw", "--no-renames", base],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    entries = []
    for line in result.stdout.splitlines():
        metadata, path = line.split("\t", 1)
        _, mode, _, _, status = metadata.split()
        entries.append((status, mode, path))
    return entries


def exists_at_base(root: Path, base: str, path: str) -> bool:
    return (
        subprocess.run(
            ["git", "cat-file", "-e", f"{base}:{path}"],
            cwd=root,
            capture_output=True,
        ).returncode
        == 0
    )


def validate_paths(
    root: Path, base: str, entries: list[tuple[str, str, str]]
) -> None:
    for status, mode, path in entries:
        require(status in ALLOWED_STATUSES, f"Stage-8 candidate status is forbidden: {status}")
        require(mode in ALLOWED_MODES, f"Stage-8 candidate mode is forbidden: {mode}")
        require(
            mode != "100755" or path.endswith(EXECUTABLE_SUFFIXES),
            f"Stage-8 executable suffix is forbidden: {path}",
        )
        require(
            path == INHERITED_SEED
            or any(path.startswith(prefix) for prefix in OWNED_PREFIXES),
            f"Stage-8 candidate changed an unowned path: {path}",
        )
        require(
            not exists_at_base(root, base, path)
            or path == INHERITED_SEED
            or path in MUTABLE_SEEDS,
            f"Stage-8 candidate changed an immutable base-existing path: {path}",
        )


def validate_base_manifest(root: Path) -> None:
    path = root / "tools/vnext_contracts/fanout/fanout-base.v1.json"
    manifest = json.loads(path.read_text())
    canonical = (
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()
    identity = f"sha256:{hashlib.sha256(canonical).hexdigest()}"
    require(identity == BASE_MANIFEST_IDENTITY, "base fanout manifest identity differs")
    require(
        frozenset(manifest["path_policy"]["candidate_diff_statuses"])
        == ALLOWED_STATUSES,
        "fanout candidate statuses differ",
    )
    require(
        frozenset(manifest["path_policy"]["candidate_blob_modes"]) == ALLOWED_MODES,
        "fanout candidate modes differ",
    )
    require(
        tuple(manifest["path_policy"]["candidate_executable_suffixes"])
        == EXECUTABLE_SUFFIXES,
        "fanout candidate executable suffixes differ",
    )
    stage8 = next(row for row in manifest["stage_owners"] if row["stage"] == 8)
    require(
        tuple(stage8["write_prefixes"]) == OWNED_PREFIXES,
        "Stage-8 write prefixes differ",
    )
    require(
        frozenset(stage8["mutable_seed_files"]) == MUTABLE_SEEDS,
        "Stage-8 mutable seed files differ",
    )
    require(
        stage8["inherited_mutable_seed_files"] == [INHERITED_SEED],
        "Stage-8 inherited mutable seed differs",
    )


def validate_v4_manifest(
    path: Path, entries: list[tuple[str, str, str]]
) -> None:
    require(path.is_file() and not path.is_symlink(), "V4 fanout manifest is unsafe")
    raw = path.read_bytes()
    require(
        hashlib.sha256(raw).hexdigest() == V4_FANOUT_MANIFEST_SHA256,
        "V4 fanout manifest identity differs",
    )
    manifest = json.loads(raw)
    require(
        manifest.get("schema") == "maestro.external.vnext-successor-fanout.v4",
        "V4 fanout manifest schema differs",
    )
    require(
        manifest.get("design_commit") == V4_DESIGN_COMMIT
        and manifest.get("design_tree") == V4_DESIGN_TREE
        and tuple(manifest.get("canonical_integration_order", ()))
        == V4_CANONICAL_INTEGRATION_ORDER
        and manifest.get("preservation_only") is True,
        "V4 fanout design identity or integration order differs",
    )
    require(
        manifest.get("preserved_candidate_commits", {}).get("8")
        == PRESERVED_CANDIDATE,
        "V4 preserved Stage-8 candidate differs",
    )
    stage8 = next(row for row in manifest["stage_owners"] if row["stage"] == 8)
    require(
        stage8["candidate_scope"]
        == (
            "Search, Memory, Intake, Research, Capability and Maturity, "
            "observation-facing diagnostics"
        ),
        "V4 Stage-8 candidate scope differs",
    )
    require(
        tuple(stage8["write_prefixes"]) == OWNED_PREFIXES,
        "V4 Stage-8 write prefixes differ",
    )
    require(
        frozenset(stage8["mutable_seed_files"]) == MUTABLE_SEEDS,
        "V4 Stage-8 mutable seed files differ",
    )
    require(
        stage8["inherited_mutable_seed_files"] == [INHERITED_SEED],
        "V4 Stage-8 inherited mutable seed differs",
    )

    shared = manifest["shared_denylist"]
    denied_files = frozenset(shared["exact_files"])
    denied_prefixes = tuple(shared["path_prefixes"])
    for _, _, candidate_path in entries:
        require(
            candidate_path == INHERITED_SEED
            or (
                candidate_path not in denied_files
                and not candidate_path.startswith(denied_prefixes)
            ),
            f"Stage-8 candidate changed a V4 shared-denylist path: {candidate_path}",
        )


def source_map(root: Path) -> dict[str, str]:
    return {path: (root / path).read_text() for path in RUST_SOURCES}


def validate_sources(sources: dict[str, str]) -> None:
    for path, source in sources.items():
        for forbidden in FORBIDDEN_RUST:
            require(
                forbidden not in source,
                f"{path} contains forbidden authority or I/O surface {forbidden}",
            )

    builder = sources[INHERITED_SEED]
    for marker in (
        "fn bind_stage8_production_consumer<'store>(",
        "facade: &mut AuthorityFacadeV1<'store>",
        "connection: &mut dyn TrustedHostDiagnosticConnectionPortV1",
        "current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1",
        ".protected_continuity_diagnostic_with_ports(",
        ".map(|released| released.into_bytes())",
    ):
        require(marker in builder, f"inherited diagnostic seed lost {marker}")
    require("encode_canonical_envelope(input)?" in builder, "builder is not canonical")
    require("#[cfg(not(test))]" in builder, "production builder branch is absent")
    production_builder = builder.split("#[cfg(not(test))]", 1)[1].split(
        "#[cfg(test)]", 1
    )[0]
    require(
        "let _ = (input, mode);" in production_builder
        and "None" in production_builder
        and "encode_canonical_envelope(input)?" not in production_builder
        and "Some(candidate)" not in production_builder,
        "production assembler bypasses the Authority-owned diagnostic consumer",
    )
    test_builder = builder.split("#[cfg(test)]", 1)[1]
    require(
        "encode_canonical_envelope(input)?" in test_builder
        and "Some(candidate)" in test_builder,
        "test-only canonical assembler is absent",
    )
    require("Some(candidate)" in builder, "builder does not return the candidate")

    diagnostics = sources["src/domain/vnext/evidence/diagnostics/mod.rs"]
    declaration = diagnostics.find(
        "pub(crate) struct ProtectedDiagnosticEnvelopeV1"
    )
    require(declaration >= 0, "protected diagnostic carrier is absent")
    derive_window = diagnostics[max(0, declaration - 128) : declaration]
    require(
        "#[derive" not in derive_window,
        "protected diagnostic carrier is not move-only",
    )
    require(
        "#[cfg(test)]" not in derive_window,
        "protected diagnostic consumer is not production reachable",
    )
    for forbidden in (
        "impl Clone for ProtectedDiagnosticEnvelopeV1",
        "impl Copy for ProtectedDiagnosticEnvelopeV1",
    ):
        require(
            forbidden not in diagnostics,
            "protected diagnostic carrier is not move-only",
        )
    for marker in (
        "from_authority_release(\n        released: ProtectedContinuityDiagnosticReleasedEnvelopeV1",
        "pub(crate) fn into_bytes(self)",
    ):
        require(marker in diagnostics, f"protected diagnostic carrier lost {marker}")

    observation = sources["src/operations/vnext/observation/mod.rs"]
    for marker in (
        "inputs.search.snapshot_ref()",
        "inputs.memory.snapshot_ref()",
        "inputs.intake.snapshot_ref()",
        "inputs.research.snapshot_ref()",
        "inputs.capability.snapshot_ref()",
        "inputs.maturity.snapshot_ref()",
        "inputs.diagnostics.snapshot_ref()",
        "SearchProjectionFreshnessV1::Current",
        "inputs.maturity.capability_source_closure_ref()",
        "inputs.capability.source_closure_ref()",
    ):
        require(marker in observation, f"coherent observation join lost {marker}")
    for marker in (
        "pub(crate) fn acquire_protected_continuity_diagnostic(",
        "authority: &mut AuthorityFacadeV1<'_>",
        "connection: &mut dyn TrustedHostDiagnosticConnectionPortV1",
        "current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1",
        "requested_subject: ContinuityReferenceV1",
        ".protected_continuity_diagnostic_with_ports(",
        ".map(ProtectedDiagnosticEnvelopeV1::from_authority_release)",
        ".map_err(|_| InformationObservationErrorV1::Unavailable)",
    ):
        require(marker in observation, f"protected diagnostic acquisition lost {marker}")

    action_markers = {
        "src/domain/vnext/search/mod.rs": (
            range(130, 132),
            ("authorized_rebuild", "authorized_purge"),
        ),
        "src/domain/vnext/memory/mod.rs": (
            range(132, 139),
            (
                "record_candidate",
                "promote",
                "reject",
                "quarantine",
                "invalidate",
                "supersede",
                "security_erase",
            ),
        ),
        "src/domain/vnext/intake/mod.rs": (
            range(139, 142),
            ("record_source", "publish_finding", "dispose_source"),
        ),
        "src/domain/vnext/research/mod.rs": (
            range(142, 146),
            (
                "begin_question",
                "append_revision",
                "publish_synthesis",
                "dispose_question",
            ),
        ),
    }
    for path, (tags, functions) in action_markers.items():
        source = sources[path]
        for marker in (
            "AdmittedRepositoryActionV1",
            "action().global_tag()",
            "current_snapshot_id()",
            "successor_snapshot()",
        ):
            require(marker in source, f"{path} lost {marker}")
        require(
            "#[cfg(test)]\nuse crate::domain::vnext::authority::"
            "AdmittedRepositoryActionV1;" in source,
            f"{path} exposes the raw admitted-Action adapter in production",
        )
        for function in functions:
            require(
                f"#[cfg(test)]\npub(crate) fn {function}(" in source
                or f"#[cfg(test)]\n    pub(crate) fn {function}(" in source,
                f"{path} exposes raw adapter function {function} in production",
            )
        for tag in tags:
            require(str(tag) in source, f"{path} lost Action tag {tag}")

    action_call_counts = {
        "src/domain/vnext/search/mod.rs": ("require_action_and_snapshot(", 3),
        "src/domain/vnext/memory/mod.rs": ("self.require_action(admitted", 6),
        "src/domain/vnext/intake/mod.rs": ("self.require_action(admitted", 3),
        "src/domain/vnext/research/mod.rs": ("self.require_action(admitted", 4),
    }
    for path, (marker, expected) in action_call_counts.items():
        require(
            sources[path].count(marker) == expected,
            f"{path} does not guard every mutating entry point",
        )


def validate_frozen_stage5_interfaces(
    root: Path, *, facade_source: str | None = None
) -> None:
    materialization = (
        root / "src/domain/vnext/authority/materialization.rs"
    ).read_text()
    facade = (
        facade_source
        if facade_source is not None
        else (root / "src/domain/vnext/authority/facade.rs").read_text()
    )
    for marker in (
        "SearchMaintenanceRepositoryActionBindingOwnerV1",
        "MemoryRepositoryActionBindingOwnerV1",
        "IntakeRepositoryActionBindingOwnerV1",
        "ResearchRepositoryActionBindingOwnerV1",
        "RepositoryActionBindingFactsV1",
    ):
        require(
            marker in materialization,
            f"frozen Stage-5 owner materialization shape lost {marker}",
        )
    for marker in (
        "pub(in crate::domain::vnext::authority) fn execute_owner_materialization",
        "pub(in crate::domain::vnext::authority) fn publish_repository_materialization",
        "fn execute_scheduling_policy_materialization(",
        "fn derive_scheduling_policy_binding_facts(",
        "self.publish_repository_materialization(probe, move |port| {",
        "port.execute_scheduling_policy_materialization(probe, owner, "
        "requires_downgrade_mandate)",
    ):
        require(
            marker in facade,
            f"frozen Stage-5 owner consumption shape lost {marker}",
        )
    require(
        "pub(in crate::domain::vnext::authority) struct RepositoryActionBindingFactsV1"
        in materialization,
        "Authority fact bag escaped its owner-private boundary",
    )
    for marker in (
        "pub(super) struct SchedulingPolicyPublicationInputV1",
        "impl SchedulingPolicyPublicationInputV1 {\n    pub(super) fn new(",
        "pub(super) fn publish_scheduling_policy_without_downgrade(",
        "pub(super) fn publish_scheduling_policy_with_downgrade(",
    ):
        require(
            marker in facade,
            f"private scheduling publication facade lost {marker}",
        )
    for widened in (
        "pub(in crate::domain::vnext) struct SchedulingPolicyPublicationInputV1",
        "impl SchedulingPolicyPublicationInputV1 {\n"
        "    pub(in crate::domain::vnext) fn new(",
        "pub(in crate::domain::vnext) fn publish_scheduling_policy_without_downgrade(",
        "pub(in crate::domain::vnext) fn publish_scheduling_policy_with_downgrade(",
    ):
        require(
            widened not in facade,
            f"private scheduling publication facade widened to {widened}",
        )
    stage7_seed = (
        root
        / "src/domain/vnext/authority/governance_attestation_stage7_seed.rs"
    ).read_text()
    for marker in (
        "pub(in crate::domain::vnext) fn publish_scheduling_policy_from_stage7(",
        "SchedulingPolicyPublicationInputV1::new(",
        "facade.publish_scheduling_policy_without_downgrade(",
        "facade.publish_scheduling_policy_with_downgrade(",
    ):
        require(
            marker in stage7_seed,
            f"crate-visible Stage-7 Authority seed lost {marker}",
        )

    integration = (
        root / "src/domain/vnext/integration/trusted_host_diagnostic.rs"
    ).read_text()
    persistence = (
        root / "src/domain/vnext/persistence/protected_diagnostic.rs"
    ).read_text()
    for marker in (
        "pub(crate) fn protected_continuity_diagnostic_with_ports(",
        "connection: &mut dyn TrustedHostDiagnosticConnectionPortV1",
        "current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1",
        "Result<ProtectedContinuityDiagnosticReleasedEnvelopeV1, AuthorityPublicationError>",
    ):
        require(
            marker in facade,
            f"frozen Stage-5 diagnostic acquisition shape lost {marker}",
        )
    for marker in (
        "pub(crate) trait TrustedHostDiagnosticConnectionPortV1",
        "#[cfg(test)]\nimpl TrustedHostDiagnosticConnectionPortV1 for "
        "TrustedHostDiagnosticTestConnectionV1",
    ):
        require(
            marker in integration,
            f"frozen Stage-5 host adapter parity shape lost {marker}",
        )
    for marker in (
        "pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1",
        "#[cfg(test)]\nimpl ProtectedDiagnosticCurrentViewProviderV1 for "
        "ProtectedDiagnosticTestCurrentViewProviderV1",
    ):
        require(
            marker in persistence,
            f"frozen Stage-5 current-view adapter parity shape lost {marker}",
        )


def validate_mutants(sources: dict[str, str]) -> int:
    mutants = []

    builder = dict(sources)
    builder[INHERITED_SEED] = builder[INHERITED_SEED].replace(
        "Some(candidate)", "None", 1
    )
    mutants.append(("builder-none", builder))

    seed_consumer = dict(sources)
    seed_consumer[INHERITED_SEED] = seed_consumer[INHERITED_SEED].replace(
        "fn bind_stage8_production_consumer<'store>(",
        "fn detached_stage8_production_consumer<'store>(",
        1,
    )
    mutants.append(("missing-inherited-seed-consumer", seed_consumer))

    observation = dict(sources)
    observation["src/operations/vnext/observation/mod.rs"] = observation[
        "src/operations/vnext/observation/mod.rs"
    ].replace("inputs.memory.snapshot_ref(),", "", 1)
    mutants.append(("mixed-memory-snapshot", observation))

    memory = dict(sources)
    memory["src/domain/vnext/memory/mod.rs"] = memory[
        "src/domain/vnext/memory/mod.rs"
    ].replace("current_snapshot_id()", "authorization_receipt()", 1)
    mutants.append(("memory-currentness", memory))

    passive_probe = dict(sources)
    passive_probe["src/domain/vnext/capability/runtime/mod.rs"] += (
        "\nuse std::process::Command;\n"
    )
    mutants.append(("passive-probe", passive_probe))

    invented_action = dict(sources)
    invented_action["src/domain/vnext/evidence/diagnostics/mod.rs"] += (
        "\nstruct ActionSpec;\n"
    )
    mutants.append(("diagnostic-action", invented_action))

    test_only_consumer = dict(sources)
    test_only_consumer["src/domain/vnext/evidence/diagnostics/mod.rs"] = test_only_consumer[
        "src/domain/vnext/evidence/diagnostics/mod.rs"
    ].replace(
        "pub(crate) struct ProtectedDiagnosticEnvelopeV1",
        "#[cfg(test)]\npub(crate) struct ProtectedDiagnosticEnvelopeV1",
        1,
    )
    mutants.append(("test-only-diagnostic-consumer", test_only_consumer))

    missing_acquisition = dict(sources)
    missing_acquisition["src/operations/vnext/observation/mod.rs"] = missing_acquisition[
        "src/operations/vnext/observation/mod.rs"
    ].replace(
        "pub(crate) fn acquire_protected_continuity_diagnostic(",
        "pub(crate) fn detached_protected_diagnostic(",
        1,
    )
    mutants.append(("missing-diagnostic-acquisition", missing_acquisition))

    cloneable_carrier = dict(sources)
    cloneable_carrier["src/domain/vnext/evidence/diagnostics/mod.rs"] = cloneable_carrier[
        "src/domain/vnext/evidence/diagnostics/mod.rs"
    ].replace(
        "pub(crate) struct ProtectedDiagnosticEnvelopeV1",
        "#[derive(Clone)]\npub(crate) struct ProtectedDiagnosticEnvelopeV1",
        1,
    )
    mutants.append(("cloneable-diagnostic-carrier", cloneable_carrier))

    search_guard = dict(sources)
    search_guard["src/domain/vnext/search/mod.rs"] = search_guard[
        "src/domain/vnext/search/mod.rs"
    ].replace("    require_action_and_snapshot(\n", "    unchecked_action(\n", 1)
    mutants.append(("unguarded-search-mutation", search_guard))

    intake_guard = dict(sources)
    intake_guard["src/domain/vnext/intake/mod.rs"] = intake_guard[
        "src/domain/vnext/intake/mod.rs"
    ].replace(
        "        self.require_action(admitted, RECORD_INTAKE_SOURCE_TAG_V1)?;\n",
        "",
        1,
    )
    mutants.append(("unguarded-intake-mutation", intake_guard))

    research_guard = dict(sources)
    research_guard["src/domain/vnext/research/mod.rs"] = research_guard[
        "src/domain/vnext/research/mod.rs"
    ].replace(
        "        self.require_action(admitted, BEGIN_RESEARCH_QUESTION_TAG_V1)?;\n",
        "",
        1,
    )
    mutants.append(("unguarded-research-mutation", research_guard))

    for name, mutant in mutants:
        try:
            validate_sources(mutant)
        except ValidationError:
            continue
        raise ValidationError(f"Stage-8 validator accepted mutant {name}")
    return len(mutants)


def validate_path_mutants(root: Path, base: str) -> int:
    mutants = (
        ("deleted-owned-path", ("D", "100644", INHERITED_SEED)),
        ("symlink-owned-path", ("M", "120000", INHERITED_SEED)),
        (
            "executable-rust-path",
            ("M", "100755", "src/domain/vnext/search/mod.rs"),
        ),
        ("shared-denylist-path", ("M", "100644", "Cargo.toml")),
    )
    for name, entry in mutants:
        try:
            validate_paths(root, base, [entry])
        except ValidationError:
            continue
        raise ValidationError(f"Stage-8 validator accepted path mutant {name}")
    return len(mutants)


def validate_frozen_stage5_interface_mutants(root: Path) -> int:
    facade = (root / "src/domain/vnext/authority/facade.rs").read_text()
    widened = facade
    for private_marker in (
        "pub(super) struct SchedulingPolicyPublicationInputV1",
        "impl SchedulingPolicyPublicationInputV1 {\n    pub(super) fn new(",
        "pub(super) fn publish_scheduling_policy_without_downgrade(",
        "pub(super) fn publish_scheduling_policy_with_downgrade(",
    ):
        require(
            private_marker in widened,
            f"frozen facade cannot build visibility mutant for {private_marker}",
        )
        widened = widened.replace(
            private_marker,
            private_marker.replace(
                "pub(super)", "pub(in crate::domain::vnext)"
            ),
            1,
        )
    try:
        validate_frozen_stage5_interfaces(root, facade_source=widened)
    except ValidationError:
        return 1
    raise ValidationError(
        "Stage-8 validator accepted widened scheduling facade mutant"
    )


def validate_fixture(root: Path) -> None:
    fixture = json.loads(
        (
            root
            / "tests/fixtures/vnext/stage8/information-capabilities.v1.json"
        ).read_text()
    )
    require(
        fixture.get("schema")
        == "maestro.vnext.stage8-information-capabilities-fixture.v1",
        "Stage-8 fixture schema differs",
    )
    cases = fixture.get("cases")
    require(isinstance(cases, list) and len(cases) == 5, "Stage-8 fixture is incomplete")
    require(
        len({case.get("id") for case in cases}) == 5,
        "Stage-8 fixture case ids are not unique",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", default=DEFAULT_BASE)
    parser.add_argument("--fanout-manifest", required=True, type=Path)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[3])
    args = parser.parse_args()
    root = args.root.resolve()

    entries = changed_entries(root, args.base)
    paths = [path for _, _, path in entries]
    validate_base_manifest(root)
    validate_v4_manifest(args.fanout_manifest.resolve(), entries)
    validate_paths(root, args.base, entries)
    sources = source_map(root)
    validate_sources(sources)
    mutants_rejected = validate_mutants(sources)
    mutants_rejected += validate_path_mutants(root, args.base)
    mutants_rejected += validate_frozen_stage5_interface_mutants(root)
    validate_fixture(root)
    validate_frozen_stage5_interfaces(root)
    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage8-static-preflight.v1",
                "status": "static-proof-inputs-valid",
                "base": args.base,
                "fanout_manifest_sha256": V4_FANOUT_MANIFEST_SHA256,
                "preserved_candidate": PRESERVED_CANDIDATE,
                "changed_paths": paths,
                "mutants_rejected": mutants_rejected,
                "compile_or_runtime_proof": False,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
