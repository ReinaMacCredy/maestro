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
