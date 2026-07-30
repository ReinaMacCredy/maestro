use std::fs;
use std::path::Path;

#[test]
fn product_pruning_places_expected_old_effect_inside_authority_linearization() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        fs::read_to_string(workspace.join("src/domain/installation/resource_cutover.rs")).unwrap();
    let body = source
        .split("pub(in crate::domain) fn execute_stage12_product_pruning(")
        .nth(1)
        .and_then(|tail| tail.split("fn migration_release_id(").next())
        .expect("Stage12 pruning body");

    assert!(body.contains("consume_with_linearization(&closure, ||"));
    assert!(body.contains("effects.compare_expected_old_and_prune("));
    assert!(body.contains("AgentResourceCutoverErrorV1::Stage12BindingMismatch)??"));
    assert!(body.contains("guard: LegacyRemovalGuardV3<'_>"));
    assert!(body.contains("closure: Stage12ConsumerReaderHoldClosureV3"));
    assert!(body.contains("continuation: InstallationPhysicalPruningContinuationV3"));
    assert!(body.contains("continuation.expected_old_state_id != closure.expected_old_state_id()"));
    assert!(body.contains("closure.expected_old_state_id().as_bytes() == &[0; 32]"));
    assert!(body.contains("closure.loss_manifest_id()"));
    assert!(body.contains("closure.foundation_closure_id()"));
    assert!(body.contains("closure.rollback_assessment_id()"));
    assert!(body.contains("closure.legacy_quarantine_epoch_id()"));
    assert!(!body.contains("LegacyRemovalGuardV2"));
    assert!(!body.contains(".consume_for(&closure)"));
    assert!(
        body.find("continuation.deletion_plan_id != closure.deletion_plan_id()")
            < body.find("consume_with_linearization(&closure, ||")
    );
    let callback = body
        .split("consume_with_linearization(&closure, ||")
        .nth(1)
        .and_then(|tail| tail.split(".map_err(").next())
        .expect("Authority-bracketed callback");
    assert!(callback.contains("effects.compare_expected_old_and_prune("));
    assert!(callback.contains("closure.expected_old_state_id()"));
    assert!(!callback.contains("continuation.expected_old_state_id"));
}

#[test]
fn v3_guard_is_private_one_use_v4_bound_and_has_no_historical_adapter() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let authority =
        fs::read_to_string(workspace.join("src/domain/authority/legacy_removal_guard.rs")).unwrap();
    let installation =
        fs::read_to_string(workspace.join("src/domain/installation/resource_cutover.rs")).unwrap();
    let operations =
        fs::read_to_string(workspace.join("src/operations/installation/agent_resource_release.rs"))
            .unwrap();
    let guard = authority
        .split("pub(in crate::domain) struct LegacyRemovalGuardV3<'cut>")
        .nth(1)
        .expect("V3 guard definition and implementation");

    for required_shape in [
        "_store: &'cut mut StoreV1",
        "_not_send_or_sync: PhantomData<Rc<()>>",
        "pub(super) const fn mint(",
        "pub(in crate::domain) fn consume_with_linearization<T, E>(",
        "linearize_expected_old: impl FnOnce() -> Result<T, E>",
        "with_serialized_active_view",
        "observe_legacy_removal_guard_currentness_v3",
    ] {
        assert!(
            guard.contains(required_shape),
            "missing private V3 authority shape {required_shape}"
        );
    }
    for exact_v4_binding in [
        "_loss_manifest",
        "_foundation_closure",
        "_rollback_assessment",
        "_quarantine_epoch",
        "_expected_old_state",
        "legacy-removal-invocation.v3",
    ] {
        assert!(
            authority.contains(exact_v4_binding),
            "missing V4 guard binding {exact_v4_binding}"
        );
    }
    for forbidden_shape in [
        "impl Clone for LegacyRemovalGuardV3",
        "impl std::fmt::Debug for LegacyRemovalGuardV3",
        "Serialize",
        "Deserialize",
        "<'static>",
        "into_",
        "impl From<LegacyRemovalGuardV2",
        "From<LegacyRemovalGuardV2",
    ] {
        assert!(
            !guard.contains(forbidden_shape),
            "forbidden V3 authority shape {forbidden_shape}"
        );
    }
    assert!(authority.contains("pub(super) fn consume_for("));
    assert!(authority.contains("pub(super) fn consume_with_linearization<T, E>("));

    for current_dependency in [
        "UnavailablePreexistingLossManifestV4",
        "FoundationLegacyQuarantineClosureV2",
        "LegacyRollbackAssessmentV4",
        "LegacyQuarantineEpochV4",
        "Stage12ConsumerReaderHoldClosureV3",
        "InstallationPhysicalPruningContinuationV3",
    ] {
        assert!(
            installation.contains(current_dependency) || operations.contains(current_dependency),
            "missing current Stage 12 dependency {current_dependency}"
        );
    }
    assert!(!operations.contains("LegacyQuarantineEpochV3"));
    assert!(!operations.contains("InstallationLegacyDeletionPlanV2"));
    assert!(!operations.contains("Stage12RollbackRehearsalV2"));
    assert!(!operations.contains("StoreV1"));
    assert!(!authority.contains("Stage12LegacyCutCoordinatorV3"));
    assert!(!installation.contains("Stage12LegacyCutCoordinatorV3"));
    assert!(!operations.contains("Stage12LegacyCutCoordinatorV3"));

    for forbidden_surface in [
        "src/domain/authority/action_basis.rs",
        "src/domain/authority/context.rs",
        "src/domain/authority/grant.rs",
        "src/domain/capability/literals.rs",
    ] {
        let source = fs::read_to_string(workspace.join(forbidden_surface)).unwrap();
        assert!(
            !source.contains("LegacyRemovalGuardV3"),
            "V3 removal authority leaked into {forbidden_surface}"
        );
    }
}

#[test]
fn v3_expected_old_is_bound_before_authority_admission_and_cannot_be_resupplied_late() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let authority =
        fs::read_to_string(workspace.join("src/domain/authority/legacy_removal_guard.rs")).unwrap();
    let installation =
        fs::read_to_string(workspace.join("src/domain/installation/resource_cutover.rs")).unwrap();
    let operations =
        fs::read_to_string(workspace.join("src/operations/installation/agent_resource_release.rs"))
            .unwrap();

    let deletion_plan = installation
        .split("pub(crate) fn stage12_deletion_plan_v3(")
        .nth(1)
        .and_then(|tail| tail.split("fn has_exact_embedded_journal").next())
        .expect("V3 deletion-plan constructor");
    for required in [
        "expected_old_state_id: MigrationDigestV1",
        "expected_old_state_id.as_bytes() == &[0; 32]",
        "*expected_old_state_id.as_bytes()",
        "expected_old_state_id,",
    ] {
        assert!(
            deletion_plan.contains(required),
            "deletion plan does not bind {required}"
        );
    }

    let closure = installation
        .split("impl Stage12ConsumerReaderHoldClosureV3")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(in crate::domain) trait InstallationPhysicalPruningEffectPortV2")
                .next()
        })
        .expect("V3 consumer closure");
    assert!(closure.contains("*deletion_plan.expected_old_state_id().as_bytes()"));
    assert!(closure.contains("expected_old_state_id: deletion_plan.expected_old_state_id()"));
    assert!(closure.contains("pub(crate) const fn expected_old_state_id(&self)"));

    let continuation = installation
        .split("impl InstallationPhysicalPruningContinuationV3")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(in crate::domain) struct CommittedLegacyPhysicalPruningV3")
                .next()
        })
        .expect("V3 pruning continuation");
    assert!(continuation.contains("closure: &Stage12ConsumerReaderHoldClosureV3"));
    assert!(continuation.contains("expected_old_state_id: closure.expected_old_state_id()"));
    assert!(
        !continuation.contains("expected_old_state_id: MigrationDigestV1"),
        "a caller can still resupply expected-old after Authority admission"
    );

    let rehearsal = operations
        .split("pub(crate) fn rehearse_stage12_agent_resource_rollback(")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn publish_agent_resource_release")
                .next()
        })
        .expect("Stage 12 rollback rehearsal");
    assert!(rehearsal.contains("expected_old_state_id: MigrationDigestV1"));
    assert!(rehearsal.contains("&rollback,\n            expected_old_state_id,"));

    let v3_authority = authority
        .split("pub(super) struct LegacyRemovalGuardBindingV3")
        .nth(1)
        .expect("V3 Authority binding");
    for required in [
        "_expected_old_state: [u8; 32]",
        "expected_old_state: &[u8; 32]",
        "expected_old_state.as_slice()",
        "expected_old_state: *closure.expected_old_state_id().as_bytes()",
        "self._expected_old_state != consumer.expected_old_state",
    ] {
        assert!(
            v3_authority.contains(required),
            "Authority does not bind {required}"
        );
    }
}
