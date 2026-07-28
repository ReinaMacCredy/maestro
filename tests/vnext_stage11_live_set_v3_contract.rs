use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn stage11_v3_source_contract_passes_independent_validators() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (runtime, validator) in [
        ("python3", "tools/vnext_contracts/stage11/validate_v3.py"),
        ("ruby", "tools/vnext_contracts/stage11/verify_v3.rb"),
    ] {
        let output = Command::new(runtime)
            .arg(root.join(validator))
            .current_dir(root)
            .output()
            .unwrap_or_else(|error| panic!("run {runtime} Stage-11 validator: {error}"));
        assert!(
            output.status.success(),
            "{runtime} validator failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn stage11_v3_runtime_facade_is_crate_scoped_and_v2_aggregate_stays_historical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let migration_root =
        fs::read_to_string(root.join("src/domain/migration/mod.rs")).expect("migration root");
    assert!(migration_root.contains("pub(crate) mod runtime;"));
    assert!(!migration_root.contains("LegacyQuarantineEpochV3"));

    for historical in [
        "src/foundation/core/aggregate_census.rs",
        "src/foundation/core/aggregate_census_stage11_seed.rs",
    ] {
        let text = fs::read_to_string(root.join(historical)).expect("historical V2 source");
        assert!(!text.contains("LegacyQuarantineEpochV3"));
        assert!(!text.contains("FoundationLegacyQuarantineLeaseV1"));
    }
}

#[test]
fn stage11_v4_loss_materialization_accepts_only_a_foundation_receipt() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = fs::read_to_string(root.join("src/domain/migration/runtime/live_set_v3.rs"))
        .expect("V4 migration runtime");
    let loss = runtime
        .split("pub struct UnavailablePreexistingLossV4")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub struct UnavailablePreexistingLossManifestV4")
                .next()
        })
        .expect("V4 loss section");
    for required in [
        "FoundationValidatedUnavailablePreexistingLossReceiptV1",
        "owner_snapshot_id",
        "issuer_id",
        "historical_tuple_id",
        "owner_current_tuple_id",
        "owner_admission_id",
        "owner_currentness_id",
        "validation_invocation_id",
        "pass_a_absence_id",
        "pass_b_absence_id",
        "unavailable-preexisting-loss.v4",
    ] {
        assert!(
            loss.contains(required),
            "missing V4 loss binding {required}"
        );
    }
    for forbidden in [
        "display_locator",
        "relative_locator",
        "PathBuf",
        "StoreV1",
        "loss_evidence_id",
        "UnavailablePreexistingLossV3::new",
    ] {
        assert!(
            !loss.contains(forbidden),
            "V4 loss regained raw/caller authority: {forbidden}"
        );
    }
    for required in [
        "UnavailablePreexistingLossManifestV4",
        "LegacyRollbackAssessmentV4",
        "LegacyQuarantineEpochBasisV4",
        "LegacyQuarantineEpochV4",
        "legacy-quarantine-final-currentness.v4",
        "legacy-quarantine-epoch.v4",
    ] {
        assert!(
            runtime.contains(required),
            "missing V4 dependent {required}"
        );
    }
}

#[test]
fn stage11_v4_operation_has_no_detached_store_census_or_limit_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let operation = fs::read_to_string(root.join("src/operations/migration/live_set_v3.rs"))
        .expect("V4 operation");
    let v4 = operation
        .split("pub(crate) fn execute_offline_live_set_v4")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) struct Stage11LiveSetContinuationV4")
                .next()
        })
        .expect("V4 operation signature");
    for required in [
        "DeclaredRootUniverseLeaseV1",
        "LegacyQuarantineExpectedSourceSetV4",
        "OwnerUnavailablePreexistingLossEvidenceIssuerPortV1",
        "Stage11PhysicalClosureV4",
    ] {
        assert!(v4.contains(required), "missing V4 owner input {required}");
    }
    for forbidden in [
        "StoreV1",
        "InstallationCensusV1",
        "DescriptorCensusLimitsV1",
        "PathBuf",
        "root_binding",
        "loss_evidence_id",
    ] {
        assert!(
            !v4.contains(forbidden),
            "V4 operation signature regained caller authority: {forbidden}"
        );
    }
    for required in [
        "FoundationLegacyQuarantineFinalityV2::RecoveryRequired",
        "FoundationLegacyQuarantineFinalityV2::InDoubt",
        "LegacyRollbackAssessmentV4::assess",
        "UnavailablePreexistingLossManifestV4::new",
    ] {
        assert!(
            operation.contains(required),
            "missing V4 finality path {required}"
        );
    }
}

#[test]
fn stage11_v4_foundation_materializes_opaque_schema_identical_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let foundation = fs::read_to_string(root.join("src/foundation/core/legacy_quarantine.rs"))
        .expect("Foundation quarantine");
    let runtime = fs::read_to_string(root.join("src/domain/migration/runtime/live_set_v3.rs"))
        .expect("migration runtime");
    let operation = fs::read_to_string(root.join("src/operations/migration/live_set_v3.rs"))
        .expect("migration operation");

    for required in [
        "FoundationMigrationSourceCaseV1",
        "FoundationMigrationMaterializationV1",
        "membership_encoding",
        "source_case_encoding",
        "maestro.migration.membership-key.v3",
        "maestro.migration.source-case.v3",
    ] {
        assert!(
            foundation.contains(required),
            "missing Foundation materialization contract {required}"
        );
    }
    assert!(!foundation.contains("FoundationMigrationSourcePartsV1"));
    for opaque_type in [
        "pub(crate) struct FoundationMigrationSourceCaseV1",
        "pub(crate) struct FoundationMigrationOverlapPairV1",
    ] {
        let body = foundation
            .split(opaque_type)
            .nth(1)
            .and_then(|tail| tail.split('}').next())
            .expect("opaque Foundation type");
        assert!(
            !body.contains("pub(crate) "),
            "{opaque_type} exposed caller-constructible fields"
        );
    }

    let materializer = runtime
        .split("impl FoundationMaterializedSourceCaseV3")
        .nth(1)
        .and_then(|tail| tail.split("impl ProtectedPrimaryOverlapPairV1").next())
        .expect("opaque Migration materializer");
    for forbidden in [
        "display_locator",
        "relative_locator",
        "root_binding",
        "provider_identity",
        "mount_identity",
        "anchor_identity",
        "fence_identity",
        "resolved-leaf-locator",
        "source-metadata.v3",
    ] {
        assert!(
            !materializer.contains(forbidden),
            "Migration rederived or received physical authority: {forbidden}"
        );
    }
    assert!(operation.contains("take_migration_materialization"));
    let v4_operation = operation
        .split("pub(crate) struct Stage11LiveSetContinuationV4")
        .nth(1)
        .expect("V4 continuation");
    assert!(!v4_operation.contains(".overlap_pairs()"));
    for redacted in [
        "impl std::fmt::Debug for MembershipKeyV3",
        "impl std::fmt::Debug for SourceCaseV3",
        "impl std::fmt::Debug for ProtectedPrimaryOverlapPairV1",
    ] {
        assert!(
            runtime.contains(redacted),
            "missing redacted Debug {redacted}"
        );
    }
}

#[test]
fn stage11_v4_loss_audit_has_canonical_reload_currentness_and_persistence_port() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = fs::read_to_string(root.join("src/domain/migration/runtime/live_set_v3.rs"))
        .expect("migration runtime");
    let operation = fs::read_to_string(root.join("src/operations/migration/live_set_v3.rs"))
        .expect("migration operation");
    let operation_facade = fs::read_to_string(root.join("src/operations/migration/mod.rs"))
        .expect("migration operation facade");
    let foundation = fs::read_to_string(root.join("src/foundation/core/legacy_quarantine.rs"))
        .expect("Foundation quarantine");
    for required in [
        "encode_canonical_audit",
        "decode_canonical_audit",
        "UnavailablePreexistingLossAuditCurrentnessV4",
        "InvalidLossAudit",
        "v4_loss_audit_survives_reload_and_rejects_tamper_or_stale_currentness",
    ] {
        assert!(runtime.contains(required), "missing loss audit {required}");
    }
    for required in [
        "UnavailablePreexistingLossAuditPersistencePortV1",
        "create_audit_if_absent",
        "read_audit",
        "persist_unavailable_preexisting_loss_audits_v4",
        "LossAuditRollbackFailed",
        "audit_failure_after_rollback",
    ] {
        assert!(
            operation.contains(required),
            "missing owner-neutral persistence seam {required}"
        );
    }
    let persistence = operation
        .split("pub(crate) trait UnavailablePreexistingLossAuditPersistencePortV1")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug, Error)]").next())
        .expect("loss audit persistence section");
    for forbidden in ["StoreV1", "PathBuf", "owner:", "provider", "mount"] {
        assert!(
            !persistence.contains(forbidden),
            "loss audit persistence gained owner/bearer authority: {forbidden}"
        );
    }
    let normalized_operation = operation.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized_operation.contains(
            "impl<P, Q> UnavailablePreexistingLossAuditPersistencePortV1 for FoundationSourceCopyContinuationV2<P, Q>"
        ),
        "loss-audit persistence is not sealed to the captured Foundation custody continuation"
    );
    for required in ["fn create_loss_audit_if_absent(", "fn read_loss_audit("] {
        assert!(
            foundation.contains(required),
            "custody port is missing loss-audit capability: {required}"
        );
    }

    let execute = operation
        .split("pub(crate) fn execute_offline_live_set_v4")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) struct Stage11LiveSetContinuationV4")
                .next()
        })
        .expect("V4 execution gate");
    assert!(
        execute.contains(".finish()"),
        "execute path does not consume captured custody"
    );
    for forbidden in [
        "A: UnavailablePreexistingLossAuditPersistencePortV1",
        "persistence: &mut",
        "persistence_store",
        "StoreV1",
        ".finish(persistence)",
    ] {
        assert!(
            !execute.contains(forbidden),
            "V4 execution retains an external persistence target: {forbidden}"
        );
    }
    assert!(
        !operation_facade.contains("persistence_store"),
        "operation facade exposes a detached persistence Store"
    );

    let finish = operation
        .split("impl<P, Q> Stage11SealedCopyContinuationV4<P, Q>")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) fn finish(").nth(1))
        .and_then(|tail| {
            tail.split("pub(crate) enum Stage11PhysicalClosureV4")
                .next()
        })
        .expect("V4 finality audit gate");
    let persist = finish
        .find("persist_unavailable_preexisting_loss_audits_v4")
        .expect("persist loss audits");
    assert!(
        finish.contains("&mut self.physical"),
        "loss audits are not persisted through captured custody"
    );
    let rollback = finish
        .find("self.physical.rollback()")
        .expect("rollback on audit failure");
    let physical_finish = finish
        .find(".physical\n            .finish")
        .expect("physical finality");
    assert!(
        persist < rollback && rollback < physical_finish,
        "audit persistence and rollback must gate physical finality"
    );
}
