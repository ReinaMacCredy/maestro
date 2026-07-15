use std::path::Path;
use std::process::{Command, Output};

use maestro::domain::vnext::execution::{
    CeremonyRequestModeV1, DispatchReservationModeV1, EffectIntentControlTransitionContenderV1,
    EffectIntentControlWriterTermKindV1, EffectIntentHomeKindV1, EffectIntentLiveDispatchV1,
    EffectOriginHomeCompatibilityV1, EffectOriginRouteRoleV1, EffectWithdrawalSlotFamilyV1,
    RemoteClassificationV1, WITHDRAWN_LOCALLY_RENDERING_V1, WithdrawalAuthorityPathV1,
    WithdrawalError, WithdrawalRequestV1, bootstrap_target_census, validate_withdrawal,
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

fn request(home: EffectIntentHomeKindV1, path: WithdrawalAuthorityPathV1) -> WithdrawalRequestV1 {
    WithdrawalRequestV1 {
        home,
        path,
        live_dispatch: EffectIntentLiveDispatchV1::None,
        classification: RemoteClassificationV1::Prepared,
        has_live_attempt: false,
        has_dispatch_fence: false,
        has_seal: false,
        has_release_capability: false,
        runs_closed: true,
        same_home_current: true,
        authority_current: true,
        capacity_current: true,
        expected_old_head: true,
        expected_old_carrier: true,
    }
}

#[test]
fn stage0_effect_home_artifacts_are_reproducible_and_reject_mutants() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
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
    let mut cells = 0;
    for classification in [
        RemoteClassificationV1::Prepared,
        RemoteClassificationV1::ConfirmedNotApplied,
    ] {
        for index in 0..19 {
            let path = if index < 12 {
                WithdrawalAuthorityPathV1::Ordinary
            } else if index < 14 {
                WithdrawalAuthorityPathV1::BootstrapG0
            } else {
                WithdrawalAuthorityPathV1::ContinuityMaintenance(
                    EffectWithdrawalSlotFamilyV1::ALL[index - 14],
                )
            };
            let mut cell = request(EffectIntentHomeKindV1::ActiveStore, path);
            cell.classification = classification;
            let result = validate_withdrawal(cell).expect("legal Action withdrawal cell");
            assert_eq!(
                result.next_classification,
                RemoteClassificationV1::Cancelled
            );
            assert!(!result.creates_intent && !result.creates_attempt && !result.creates_run);
            assert!(!result.performs_provider_io && !result.refunds_or_remints);
            cells += 1;
        }
        for index in 0..11 {
            let home = if index == 0 {
                EffectIntentHomeKindV1::NoStoreCeremony
            } else {
                EffectIntentHomeKindV1::PreStoreCeremony
            };
            let mut cell = request(home, WithdrawalAuthorityPathV1::Ceremony);
            cell.classification = classification;
            validate_withdrawal(cell).expect("legal Ceremony withdrawal cell");
            cells += 1;
        }
    }
    assert_eq!(cells, 60);
    assert_eq!(
        WITHDRAWN_LOCALLY_RENDERING_V1,
        "withdrawn locally; no provider cancellation performed"
    );
}

#[test]
fn withdrawal_and_route_literals_refuse_cross_products() {
    let mut illegal = request(
        EffectIntentHomeKindV1::ActiveStore,
        WithdrawalAuthorityPathV1::Ordinary,
    );
    illegal.live_dispatch = EffectIntentLiveDispatchV1::Reserved;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::LiveDispatch)
    );

    let mut illegal = request(
        EffectIntentHomeKindV1::ActiveStore,
        WithdrawalAuthorityPathV1::Ordinary,
    );
    illegal.classification = RemoteClassificationV1::ConfirmedApplied;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::Classification)
    );

    let mut illegal = request(
        EffectIntentHomeKindV1::ActiveStore,
        WithdrawalAuthorityPathV1::Ordinary,
    );
    illegal.has_seal = true;
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::LiveAttemptFenceSealOrCapability)
    );

    let mut illegal = request(
        EffectIntentHomeKindV1::ActiveStore,
        WithdrawalAuthorityPathV1::Ordinary,
    );
    illegal.runs_closed = false;
    assert_eq!(validate_withdrawal(illegal), Err(WithdrawalError::OpenRuns));

    let illegal = request(
        EffectIntentHomeKindV1::NoStoreCeremony,
        WithdrawalAuthorityPathV1::Ordinary,
    );
    assert_eq!(
        validate_withdrawal(illegal),
        Err(WithdrawalError::CrossHomeBasisDonation)
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
