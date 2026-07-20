use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use maestro::domain::vnext::execution::{
    CeremonyRequestModeV1, DispatchReservationModeV1, EffectIntentControlTransitionContenderV1,
    EffectIntentControlWriterTermKindV1, EffectIntentHomeKindV1, EffectIntentLiveDispatchV1,
    EffectOriginHomeCompatibilityV1, EffectOriginRouteRoleV1, RemoteClassificationV1,
    WITHDRAWN_LOCALLY_RENDERING_V1, WithdrawalAuthorityPathV1, WithdrawalError,
    WithdrawalRequestV1, bootstrap_target_census, validate_withdrawal, withdrawal_catalog_cells_v1,
};
use serde_json::Value;
use std::fs;

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"))
}

fn assert_success(label: &str, output: Output) {
    assert!(
        output.status.success(),
        "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn request() -> WithdrawalRequestV1 {
    WithdrawalRequestV1::legal_for_catalog_cell(withdrawal_catalog_cells_v1()[0])
}

fn artifact_bytes(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(root)
        .expect("read Effect Home artifact root")
        .map(|entry| {
            let entry = entry.expect("read Effect Home artifact");
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("read Effect Home artifact bytes"),
            )
        })
        .collect()
}

fn hermetic_stage0_workspace(repo: &Path) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "maestro-stage0-hermetic-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("create hermetic Stage-0 workspace");
    let tracked = run(
        repo,
        "git",
        &[
            "ls-files",
            "-z",
            "--",
            "src",
            "contracts/vnext",
            "tools",
            "tests",
        ],
    );
    assert!(
        tracked.status.success(),
        "enumerate tracked Stage-0 proof inputs failed with {}: {}",
        tracked.status,
        String::from_utf8_lossy(&tracked.stderr)
    );
    let mut copied = 0_usize;
    for path in tracked.stdout.split(|byte| *byte == 0) {
        if path.is_empty() {
            continue;
        }
        let relative = Path::new(std::str::from_utf8(path).expect("tracked proof path is UTF-8"));
        assert!(
            !relative.is_absolute()
                && relative
                    .components()
                    .all(|component| matches!(component, std::path::Component::Normal(_))),
            "tracked proof path escaped the repository: {}",
            relative.display()
        );
        let source = repo.join(relative);
        let metadata = fs::symlink_metadata(&source).expect("read tracked proof input metadata");
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "tracked proof input is not a regular non-link file: {}",
            relative.display()
        );
        let destination = workspace.join(relative);
        fs::create_dir_all(
            destination
                .parent()
                .expect("tracked proof input has a parent"),
        )
        .expect("create tracked proof input parent");
        fs::copy(source, destination).expect("copy tracked proof input");
        copied += 1;
    }
    assert!(copied > 0, "tracked Stage-0 proof input set is empty");
    workspace
}

#[test]
fn stage0_effect_home_artifacts_are_reproducible_and_reject_mutants() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let artifact_root = repo.join("contracts/vnext/stage0/effect-home");
    let builder_source =
        fs::read_to_string(repo.join("tools/vnext_contracts/stage0/effect_home/build.py"))
            .expect("read Stage-0 Effect Home builder source");
    assert!(builder_source.contains("contracts/vnext/stage0/input-bindings.json"));
    assert!(!builder_source.contains("/Users/reinamaccredy/Code/maestro/.maestro/cards"));
    let before = artifact_bytes(&artifact_root);
    assert_success(
        "Stage-0 Effect Home reproducibility",
        run(
            repo,
            "python3",
            &[
                "tools/vnext_contracts/stage0/effect_home/build.py",
                "--check",
            ],
        ),
    );
    assert_success(
        "Stage-0 Effect Home semantic mutants",
        run(
            repo,
            "python3",
            &[
                "tools/vnext_contracts/stage0/effect_home/validate.py",
                "--mutants",
            ],
        ),
    );
    assert_eq!(artifact_bytes(&artifact_root), before);

    let hermetic = hermetic_stage0_workspace(repo);
    assert!(!hermetic.join(".maestro").exists());
    assert_success(
        "tracked-only Stage-0 Effect Home materialization",
        run(
            &hermetic,
            "python3",
            &[
                "tools/vnext_contracts/stage0/effect_home/build.py",
                "--output",
                "hermetic-output",
            ],
        ),
    );
    let bindings = hermetic.join("contracts/vnext/stage0/input-bindings.json");
    let mut tampered: Value =
        serde_json::from_slice(&fs::read(&bindings).expect("read hermetic Stage-0 input bindings"))
            .expect("parse hermetic Stage-0 input bindings");
    tampered["feature_id"] = Value::String("tampered-feature".to_owned());
    fs::write(
        &bindings,
        serde_json::to_vec(&tampered).expect("encode tampered Stage-0 input bindings"),
    )
    .expect("write tampered Stage-0 input bindings");
    let rejected = run(
        &hermetic,
        "python3",
        &[
            "tools/vnext_contracts/stage0/effect_home/build.py",
            "--output",
            "tampered-output",
        ],
    );
    assert!(
        !rejected.status.success(),
        "tampered tracked input bindings unexpectedly materialized Stage 0"
    );
    let _ = fs::remove_dir_all(hermetic);
}

#[test]
fn stage0_bootstrap_literal_uses_the_effective_efa0_leaf_and_predecessor_order() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let artifact: Value = serde_json::from_slice(
        &fs::read(
            repo.join("contracts/vnext/stage0/effect-home/bootstrap-control-withdrawal-v1.json"),
        )
        .expect("read Stage 0 Bootstrap withdrawal artifact"),
    )
    .expect("parse Stage 0 Bootstrap withdrawal artifact");
    let rows = artifact["rows"]
        .as_array()
        .expect("Bootstrap withdrawal rows");
    assert_eq!(
        rows.iter()
            .map(|row| row["action"].as_str().expect("Bootstrap action"))
            .collect::<Vec<_>>(),
        [
            "EnrollRecoveryCommitmentSelection",
            "RotateRecoveryCommitmentSelection",
            "RevokeRecoveryCommitmentSelection",
            "FirstHumanBindingEnrollment",
            "ReserveBootstrapMandateInteractionEffect",
            "PublishBootstrapMandateInteractionOutcome",
            "PublishBootstrapMandatePresentationObservation",
            "PublishBootstrapMandateResponseObservation",
            "ReconcileBootstrapMandateInteractionEffect",
            "IssueBootstrapMandate",
            "WithdrawBootstrapMandateInteractionEffect",
        ]
    );
    assert!(
        !serde_json::to_string(&artifact)
            .expect("serialize artifact")
            .contains("PublishBootstrapMandateInteractionEffectOutcome")
    );
}

#[test]
fn withdrawal_literals_cover_the_exact_sixty_positive_cells() {
    let cells = withdrawal_catalog_cells_v1();
    for cell in &cells {
        let result = validate_withdrawal(WithdrawalRequestV1::legal_for_catalog_cell(*cell))
            .expect("legal exact withdrawal cell");
        assert_eq!(
            result.next_classification,
            RemoteClassificationV1::Cancelled
        );
        assert!(!result.creates_intent && !result.creates_attempt && !result.creates_run);
        assert!(!result.performs_provider_io && !result.refunds_or_remints);
    }
    assert_eq!(cells.len(), 60);
    assert_eq!(
        WITHDRAWN_LOCALLY_RENDERING_V1,
        "withdrawn locally; no provider cancellation performed"
    );
}

#[test]
fn withdrawal_and_route_literals_refuse_cross_products() {
    let mut illegal = request();
    illegal.live_dispatch = EffectIntentLiveDispatchV1::Reserved;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::LiveDispatch)
    );

    let mut illegal = request();
    illegal.classification = RemoteClassificationV1::ConfirmedApplied;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::CatalogCellMismatch)
    );

    let mut illegal = request();
    illegal.has_seal = true;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::LiveAttemptFenceSealOrCapability)
    );

    let mut illegal = request();
    illegal.runs_closed = false;
    assert_eq!(validate_withdrawal(illegal), Err(WithdrawalError::OpenRuns));

    let mut illegal = request();
    illegal.home = EffectIntentHomeKindV1::NoStoreCeremony;
    illegal.path = WithdrawalAuthorityPathV1::Ordinary;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::CatalogCellMismatch)
    );

    assert!(EffectOriginHomeCompatibilityV1::counts_are_exact());
    assert_eq!(EffectOriginRouteRoleV1::ALL.len(), 9);
    assert!(EffectOriginRouteRoleV1::CeremonyResolveResult.creates_no_effect_records());
    assert!(EffectOriginRouteRoleV1::CeremonyWithdraw.creates_no_effect_records());
    assert!(
        EffectOriginHomeCompatibilityV1::validate(
            EffectOriginRouteRoleV1::ActionRecoverReserved,
            EffectIntentHomeKindV1::ActiveStore,
            Some(DispatchReservationModeV1::RecoverReserved),
            None,
            false,
        )
        .is_ok()
    );
    assert!(
        EffectOriginHomeCompatibilityV1::validate(
            EffectOriginRouteRoleV1::CeremonyResolveResult,
            EffectIntentHomeKindV1::NoStoreCeremony,
            None,
            Some(CeremonyRequestModeV1::ResolveResult),
            true,
        )
        .is_ok()
    );
    assert!(
        EffectOriginHomeCompatibilityV1::validate(
            EffectOriginRouteRoleV1::CeremonyWithdraw,
            EffectIntentHomeKindV1::ActiveStore,
            None,
            Some(CeremonyRequestModeV1::Withdraw),
            false,
        )
        .is_err()
    );
}

#[test]
fn h2_control_contenders_are_exact_and_are_not_writer_terms() {
    assert_eq!(EffectIntentControlTransitionContenderV1::ALL.len(), 11);
    assert!(matches!(
        EffectIntentControlTransitionContenderV1::ALL[8],
        EffectIntentControlTransitionContenderV1::Redispatcher
    ));
    assert!(matches!(
        EffectIntentControlTransitionContenderV1::ALL[9],
        EffectIntentControlTransitionContenderV1::Withdrawal
    ));
    assert_eq!(EffectIntentControlWriterTermKindV1::ALL.len(), 2);
    assert!(matches!(
        EffectIntentControlWriterTermKindV1::ALL[1],
        EffectIntentControlWriterTermKindV1::SameHomeRestore
    ));
}

#[test]
fn bootstrap_census_is_eleven_rows_with_three_targets_and_eight_exclusions() {
    let census = bootstrap_target_census();
    assert_eq!(census.len(), 11);
    assert_eq!(
        census
            .iter()
            .filter(|row| matches!(
                row.disposition,
                maestro::domain::vnext::execution::BootstrapTargetDispositionV1::CandidateTarget
            ))
            .count(),
        3,
    );
    assert_eq!(
        census
            .iter()
            .filter(|row| matches!(
                row.disposition,
                maestro::domain::vnext::execution::BootstrapTargetDispositionV1::HardExclusion
            ))
            .count(),
        8,
    );
    assert_eq!(
        census[10].action,
        "WithdrawBootstrapMandateInteractionEffect"
    );
}
