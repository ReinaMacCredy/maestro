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
