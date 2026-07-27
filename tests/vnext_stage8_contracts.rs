use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(workspace().join(relative)).unwrap()
}

fn assert_contains_all(source: &str, values: &[&str]) {
    for value in values {
        assert!(source.contains(value), "missing frozen marker {value}");
    }
}

fn assert_hermetic_source(path: &Path) {
    let source = fs::read_to_string(path).unwrap();
    for forbidden in [
        "std::fs",
        "std::net",
        "std::process",
        "Command::",
        "env::var",
        "Serialize",
        "Deserialize",
        "ScopeAtom",
        "ActionSpec",
    ] {
        assert!(
            !source.contains(forbidden),
            "{} contains forbidden Stage-8 authority or I/O surface {forbidden}",
            path.display()
        );
    }
}

fn assert_test_only_function(source: &str, name: &str) {
    let root_marker = format!("#[cfg(test)]\npub(crate) fn {name}(");
    let impl_marker = format!("#[cfg(test)]\n    pub(crate) fn {name}(");
    assert!(
        source.contains(&root_marker) || source.contains(&impl_marker),
        "{name} is not confined to the Stage-8 test adapter"
    );
}

#[test]
fn protected_diagnostic_builder_is_canonical_in_tests_and_facade_owned_in_production() {
    let source = read("src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs");
    assert_contains_all(
        &source,
        &[
            "fn bind_stage8_production_consumer<'store>(",
            "facade: &mut AuthorityFacadeV1<'store>",
            "connection: &mut dyn TrustedHostDiagnosticConnectionPortV1",
            "current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1",
            ".protected_continuity_diagnostic_with_ports(",
            ".map(|released| released.into_bytes())",
            "encode_canonical_envelope(input)?",
            "ProtectedContinuityDiagnosticAssemblerModeV1::Canonical",
            "#[cfg(test)]",
            "Some(candidate)",
        ],
    );
    assert!(!source.contains("Box<dyn"));
    assert_hermetic_source(
        &workspace()
            .join("src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs"),
    );

    let diagnostics = read("src/domain/vnext/evidence/diagnostics/mod.rs");
    let declaration = diagnostics
        .find("pub(crate) struct ProtectedDiagnosticEnvelopeV1")
        .unwrap();
    let derive_window = &diagnostics[declaration.saturating_sub(128)..declaration];
    assert!(
        !derive_window.contains("#[derive"),
        "protected diagnostic carrier must remain move-only"
    );
    assert_contains_all(
        &diagnostics,
        &[
            "from_authority_release(\n        released: ProtectedContinuityDiagnosticReleasedEnvelopeV1",
            "pub(crate) fn into_bytes(self)",
        ],
    );
    assert!(
        !derive_window.contains("#[cfg(test)]"),
        "protected diagnostic consumer must be production reachable"
    );
    for forbidden in [
        "impl Clone for ProtectedDiagnosticEnvelopeV1",
        "impl Copy for ProtectedDiagnosticEnvelopeV1",
    ] {
        assert!(!diagnostics.contains(forbidden));
    }
    assert_hermetic_source(&workspace().join("src/domain/vnext/evidence/diagnostics/mod.rs"));
}

#[test]
fn protected_diagnostic_acquisition_matches_the_stage5_test_adapter_shape() {
    let observation = read("src/operations/vnext/observation/mod.rs");
    assert_contains_all(
        &observation,
        &[
            "pub(crate) fn acquire_protected_continuity_diagnostic(",
            "authority: &mut AuthorityFacadeV1<'_>",
            "connection: &mut dyn TrustedHostDiagnosticConnectionPortV1",
            "current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1",
            "requested_subject: ContinuityReferenceV1",
            ".protected_continuity_diagnostic_with_ports(",
            ".map(ProtectedDiagnosticEnvelopeV1::from_authority_release)",
            ".map_err(|_| InformationObservationErrorV1::Unavailable)",
        ],
    );

    let facade = read("src/domain/vnext/authority/facade.rs");
    assert_contains_all(
        &facade,
        &[
            "pub(crate) fn protected_continuity_diagnostic_with_ports(",
            "connection: &mut dyn TrustedHostDiagnosticConnectionPortV1",
            "current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1",
            "Result<ProtectedContinuityDiagnosticReleasedEnvelopeV1, AuthorityPublicationError>",
        ],
    );
    let integration = read("src/domain/vnext/integration/trusted_host_diagnostic.rs");
    assert_contains_all(
        &integration,
        &[
            "#[cfg(test)]\nimpl TrustedHostDiagnosticConnectionPortV1 for TrustedHostDiagnosticTestConnectionV1",
            "pub(crate) trait TrustedHostDiagnosticConnectionPortV1",
        ],
    );
    let persistence = read("src/domain/vnext/persistence/protected_diagnostic.rs");
    assert_contains_all(
        &persistence,
        &[
            "#[cfg(test)]\nimpl ProtectedDiagnosticCurrentViewProviderV1 for ProtectedDiagnosticTestCurrentViewProviderV1",
            "pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1",
        ],
    );
}

#[test]
fn raw_action_adapters_are_test_only_and_match_the_atomic_authority_shape() {
    let cases = [
        (
            "src/domain/vnext/search/mod.rs",
            &["130", "131"][..],
            &["authorized_rebuild", "authorized_purge"][..],
        ),
        (
            "src/domain/vnext/memory/mod.rs",
            &["132", "133", "134", "135", "136", "137", "138"][..],
            &[
                "record_candidate",
                "promote",
                "reject",
                "quarantine",
                "invalidate",
                "supersede",
                "security_erase",
            ][..],
        ),
        (
            "src/domain/vnext/intake/mod.rs",
            &["139", "140", "141"][..],
            &["record_source", "publish_finding", "dispose_source"][..],
        ),
        (
            "src/domain/vnext/research/mod.rs",
            &["142", "143", "144", "145"][..],
            &[
                "begin_question",
                "append_revision",
                "publish_synthesis",
                "dispose_question",
            ][..],
        ),
    ];
    for (path, tags, functions) in cases {
        let source = read(path);
        assert_contains_all(
            &source,
            &[
                "AdmittedRepositoryActionV1",
                "action().global_tag()",
                "current_snapshot_id()",
                "successor_snapshot()",
            ],
        );
        assert_contains_all(&source, tags);
        assert!(source.contains(
            "#[cfg(test)]\nuse crate::domain::vnext::authority::AdmittedRepositoryActionV1;"
        ));
        for function in functions {
            assert_test_only_function(&source, function);
        }
        assert_hermetic_source(&workspace().join(path));
    }

    let authority = read("src/domain/vnext/authority/materialization.rs");
    assert_contains_all(
        &authority,
        &[
            "SearchMaintenanceRepositoryActionBindingOwnerV1",
            "MemoryRepositoryActionBindingOwnerV1",
            "IntakeRepositoryActionBindingOwnerV1",
            "ResearchRepositoryActionBindingOwnerV1",
            "RepositoryActionBindingFactsV1",
        ],
    );
    let facade = read("src/domain/vnext/authority/facade.rs");
    assert_contains_all(
        &facade,
        &[
            "pub(in crate::domain::vnext::authority) fn execute_owner_materialization",
            "pub(in crate::domain::vnext::authority) fn publish_repository_materialization",
            "fn execute_scheduling_policy_materialization(",
            "fn derive_scheduling_policy_binding_facts(",
            "self.publish_repository_materialization(probe, move |port| {",
            "port.execute_scheduling_policy_materialization(probe, owner)",
            "pub(super) struct SchedulingPolicyPublicationInputV1",
            "impl SchedulingPolicyPublicationInputV1 {\n    pub(super) fn new(",
            "pub(super) fn publish_scheduling_policy(",
        ],
    );
    for widened in [
        "pub(in crate::domain::vnext) struct SchedulingPolicyPublicationInputV1",
        "impl SchedulingPolicyPublicationInputV1 {\n    pub(in crate::domain::vnext) fn new(",
        "pub(in crate::domain::vnext) fn publish_scheduling_policy(",
    ] {
        assert!(
            !facade.contains(widened),
            "private scheduling publication facade widened to {widened}"
        );
    }
    let stage7_seed = read("src/domain/vnext/authority/governance_attestation_stage7_seed.rs");
    assert_contains_all(
        &stage7_seed,
        &[
            "pub(in crate::domain::vnext) fn publish_scheduling_policy_from_stage7(",
            "SchedulingPolicyPublicationInputV1::new(",
            "facade.publish_scheduling_policy(",
            "PlanningSchedulingPolicyInputV1::from_stage7_planning(",
        ],
    );
    assert!(
        authority.contains(
            "pub(in crate::domain::vnext::authority) struct RepositoryActionBindingFactsV1"
        ),
        "Authority fact bag escaped its owner-private boundary"
    );
}

#[test]
fn coherent_observation_join_covers_every_stage8_view() {
    let source = read("src/operations/vnext/observation/mod.rs");
    assert_contains_all(
        &source,
        &[
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
            "projection_ref",
            "planning_assessment_ref",
            "recipe_application_ref",
        ],
    );
    assert_hermetic_source(&workspace().join("src/operations/vnext/observation/mod.rs"));
}

#[test]
fn capability_and_maturity_have_no_passive_probe_or_permission_surface() {
    for path in [
        "src/domain/vnext/capability/runtime/mod.rs",
        "src/domain/vnext/maturity/mod.rs",
    ] {
        let source = read(path);
        assert_hermetic_source(&workspace().join(path));
        for forbidden in ["fn probe(", "permission:", "authorized:", "safe_to_act"] {
            assert!(!source.contains(forbidden), "{path} contains {forbidden}");
        }
    }
}

#[test]
fn stage8_fixture_names_the_complete_non_authority_matrix() {
    let fixture: Value = serde_json::from_str(&read(
        "tests/fixtures/vnext/stage8/information-capabilities.v1.json",
    ))
    .unwrap();
    assert_eq!(
        fixture["schema"],
        "maestro.vnext.stage8-information-capabilities-fixture.v1"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 5);
    assert!(cases.iter().all(|case| {
        case["id"].as_str().is_some_and(|value| !value.is_empty())
            && case["contract"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
    }));
}
