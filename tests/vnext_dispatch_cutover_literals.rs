use std::path::Path;
use std::process::{Command, Output};

use maestro::domain::execution::dispatch_state::{
    DispatchAttemptOutcomeV1, DispatchAttemptStateV1, DispatchAttemptTerminalV1,
    DispatchBindingPartsV1, DispatchBindingV1, DispatchCommitmentV1, DispatchCrossingSealV1,
    DispatchRaceDescriptorV1, DispatchRecoveryDescriptorV1, DispatchStateError,
    PreSealLocallyRejectedV1, ReservedUnsealedV1, SealedDispatchOutcomeV1,
    SealedDispatchTerminalV1, SealedInFlightV1,
};
use maestro::domain::migration::{
    ActiveStoreAtomicParticipantV1, ActiveStoreFinalityPartsV1, ActiveStoreFinalityV1,
    ActiveStoreOwningHeadV1, ActiveStorePreconditionV1, AssociationConsumptionSetV1,
    C868_RUNTIME_EDGE_COUNT, C868_SCHEMA_COUNT, C868_SUITE_COMPONENT_COUNT, CutoverCommitmentV1,
    CutoverDomainRefV1, CutoverDomainV1, EXPECTED_DELTA_SLOT_COUNT, ExpectedDeltaManifestV1,
    ExpectedDeltaRowV1, MIGRATION_CUTOVER_COMPONENT_COUNT,
    MIGRATION_CUTOVER_FINALITY_EDGE_ROW_COUNT, MIGRATION_CUTOVER_FINALITY_SCHEMA_ID_COUNT,
    MIGRATION_CUTOVER_INVARIANT_COUNT, MIGRATION_CUTOVER_PREDECESSOR_COUNT,
    MIGRATION_CUTOVER_READ_WRITE_COHORT_COUNT, MIGRATION_CUTOVER_READ_WRITE_COHORT_ROW_COUNT,
    MIGRATION_CUTOVER_SCHEMA_COUNT, MigrationCutoverAssociationV1, MigrationCutoverContextV1,
    MigrationCutoverError, MigrationCutoverMaterialV1, MigrationCutoverSuccessorLiteralV1,
    PREDECESSOR_ASSOCIATION_SCHEMA_ID, PreStoreAtomicParticipantV1, PreStoreCandidateSealV1,
    PreStoreFinalityPartsV1, PreStoreFinalityV1, PreStorePreconditionV1, ProtectedExpectedOldCasV1,
    ReleaseBindingV1, SuccessorDependencySlotV1,
};

fn dispatch_commitment(value: u8) -> DispatchCommitmentV1 {
    DispatchCommitmentV1::new([value; 32]).expect("non-zero dispatch commitment")
}

fn dispatch_binding(offset: u8) -> DispatchBindingV1 {
    DispatchBindingV1::new(DispatchBindingPartsV1 {
        attempt_id: dispatch_commitment(offset),
        attempt_revision: u64::from(offset),
        effect_intent_home_id: dispatch_commitment(offset + 1),
        effect_intent_use_fence_id: dispatch_commitment(offset + 2),
        application_envelope_id: dispatch_commitment(offset + 3),
        provider_operation_contract_id: dispatch_commitment(offset + 4),
        provider_scope_id: dispatch_commitment(offset + 5),
        provider_key_id: dispatch_commitment(offset + 6),
        credential_id: dispatch_commitment(offset + 7),
        authority_basis_id: dispatch_commitment(offset + 8),
        dispatch_fence_id: dispatch_commitment(offset + 9),
        material_stamp_id: dispatch_commitment(offset + 10),
        run_set_revision_id: dispatch_commitment(offset + 11),
        accounting_basis_id: dispatch_commitment(offset + 12),
    })
    .expect("valid dispatch binding")
}

fn cutover_commitment(value: u8) -> CutoverCommitmentV1 {
    CutoverCommitmentV1::new([value; 32]).expect("non-zero cutover commitment")
}

fn cutover_material(offset: u8) -> MigrationCutoverMaterialV1 {
    MigrationCutoverMaterialV1 {
        association_id: cutover_commitment(offset),
        inventory_id: cutover_commitment(offset + 1),
        target_set_id: cutover_commitment(offset + 2),
        quarantine_set_id: cutover_commitment(offset + 3),
        consumer_set_id: cutover_commitment(offset + 4),
        distribution_receipt_id: cutover_commitment(offset + 5),
        candidate_store_root_id: cutover_commitment(offset + 6),
        schema_read_write_set_id: cutover_commitment(offset + 7),
        writer_protocol_epoch_id: cutover_commitment(offset + 8),
        migration_epoch_id: cutover_commitment(offset + 9),
    }
}

fn run(repo: &Path, program: &str, args: &[&str], label: &str) -> Output {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert!(
        output.status.success(),
        "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

#[test]
fn dispatch_attempt_algebra_has_only_the_frozen_transitions_and_outcomes() {
    let binding = dispatch_binding(1);
    let seal = DispatchCrossingSealV1::new(dispatch_commitment(40), binding.clone());
    let reserved = DispatchAttemptStateV1::ReservedUnsealed(Box::new(ReservedUnsealedV1::new(
        binding.clone(),
    )));
    let in_flight = DispatchAttemptStateV1::SealedInFlight(Box::new(
        SealedInFlightV1::new(binding.clone(), seal.clone()).expect("matching seal"),
    ));
    let local_terminal = DispatchAttemptStateV1::Terminal(
        DispatchAttemptTerminalV1::PreSealLocallyRejected(Box::new(PreSealLocallyRejectedV1::new(
            binding.clone(),
            dispatch_commitment(41),
        ))),
    );
    let remote_terminal = DispatchAttemptStateV1::Terminal(
        DispatchAttemptTerminalV1::SealedDispatchTerminal(Box::new(
            SealedDispatchTerminalV1::new(
                binding.clone(),
                seal.clone(),
                SealedDispatchOutcomeV1::ResponseReceived,
                dispatch_commitment(42),
            )
            .expect("matching terminal seal"),
        )),
    );

    assert_eq!(reserved.validate_transition_to(&local_terminal), Ok(()));
    assert_eq!(reserved.validate_transition_to(&in_flight), Ok(()));
    assert_eq!(in_flight.validate_transition_to(&remote_terminal), Ok(()));
    assert_eq!(
        local_terminal.terminal_outcome(),
        Ok(DispatchAttemptOutcomeV1::LocallyRejected)
    );
    assert_eq!(
        remote_terminal.terminal_outcome(),
        Ok(DispatchAttemptOutcomeV1::ResponseReceived)
    );
    assert_eq!(
        reserved.terminal_outcome(),
        Err(DispatchStateError::NonTerminalOutcome)
    );
    assert_eq!(
        in_flight.terminal_outcome(),
        Err(DispatchStateError::NonTerminalOutcome)
    );
    assert_eq!(
        reserved.validate_transition_to(&remote_terminal),
        Err(DispatchStateError::DirectReservedToSealedTerminal)
    );
    assert_eq!(
        in_flight.validate_transition_to(&local_terminal),
        Err(DispatchStateError::SealedLocalRejection)
    );
    assert_eq!(
        remote_terminal.validate_transition_to(&reserved),
        Err(DispatchStateError::TerminalEscape)
    );
    assert_eq!(
        DispatchAttemptOutcomeV1::from_numeric_tag(5),
        Err(DispatchStateError::UnknownOutcomeTag(5))
    );
    assert_eq!(
        SealedDispatchOutcomeV1::from_dispatch_outcome(DispatchAttemptOutcomeV1::LocallyRejected),
        Err(DispatchStateError::SealedLocalRejection)
    );
}

#[test]
fn dispatch_seal_is_immutable_and_persistence_cannot_recreate_release_authority() {
    let binding = dispatch_binding(1);
    let original_seal = DispatchCrossingSealV1::new(dispatch_commitment(40), binding.clone());
    let replacement_seal = DispatchCrossingSealV1::new(dispatch_commitment(41), binding.clone());
    let in_flight = DispatchAttemptStateV1::SealedInFlight(Box::new(
        SealedInFlightV1::new(binding.clone(), original_seal).expect("matching seal"),
    ));
    let replacement_terminal = DispatchAttemptStateV1::Terminal(
        DispatchAttemptTerminalV1::SealedDispatchTerminal(Box::new(
            SealedDispatchTerminalV1::new(
                binding,
                replacement_seal,
                SealedDispatchOutcomeV1::AmbiguousTransport,
                dispatch_commitment(42),
            )
            .expect("seal binds same attempt basis"),
        )),
    );

    assert_eq!(
        in_flight.validate_transition_to(&replacement_terminal),
        Err(DispatchStateError::SealReplacement)
    );
    assert!(!in_flight.can_reconstruct_live_release_capability());
    let race = DispatchRaceDescriptorV1;
    assert_eq!(race.persisted_winner_count(), 1);
    assert_eq!(race.release_scope(), "successful_live_seal_cas_caller_only");
    assert!(!race.losing_writer_may_dispatch());
    let recovery = DispatchRecoveryDescriptorV1;
    assert_eq!(recovery.reconstruction_io_operations(), 0);
    assert!(!recovery.reconstructs_release_capability());
    assert!(!recovery.permits_synthetic_truth());
    assert!(!recovery.permits_synthetic_refund());
    assert!(!recovery.permits_synthetic_retry());
}

#[test]
fn migration_association_and_finality_are_context_and_release_exact() {
    let repository_ref =
        CutoverDomainRefV1::new(CutoverDomainV1::Repository, cutover_commitment(1), 7, 9)
            .expect("repository ref");
    let material = cutover_material(10);
    let commit_record_id = cutover_commitment(30);
    let association = MigrationCutoverAssociationV1::new(
        repository_ref.clone(),
        ReleaseBindingV1::RepositoryAbsent,
        MigrationCutoverContextV1::ActiveStore {
            distribution_commit_record_id: commit_record_id,
        },
        material.clone(),
    )
    .expect("repository association");
    let head = ActiveStoreOwningHeadV1 {
        association_id: material.association_id,
        distribution_commit_record_id: commit_record_id,
        distribution_receipt_id: material.distribution_receipt_id,
        domain_ref: repository_ref.clone(),
        release: ReleaseBindingV1::RepositoryAbsent,
        candidate_store_root_id: material.candidate_store_root_id,
    };
    let active_parts = ActiveStoreFinalityPartsV1 {
        association: association.clone(),
        ordered_preconditions: vec![
            ActiveStorePreconditionV1::DistributionReceipt(material.distribution_receipt_id),
            ActiveStorePreconditionV1::DistributionCommitRecord {
                commit_record_id,
                receipt_id: material.distribution_receipt_id,
            },
        ],
        atomic_participants: vec![
            ActiveStoreAtomicParticipantV1::Association(material.association_id),
            ActiveStoreAtomicParticipantV1::OwningHead(head),
        ],
    };
    assert!(ActiveStoreFinalityV1::new(active_parts.clone()).is_ok());

    let mut omitted = active_parts.clone();
    omitted.atomic_participants.remove(0);
    assert_eq!(
        ActiveStoreFinalityV1::new(omitted),
        Err(MigrationCutoverError::InvalidAtomicParticipants)
    );
    let mut duplicate = active_parts.clone();
    duplicate.atomic_participants.insert(
        1,
        ActiveStoreAtomicParticipantV1::Association(material.association_id),
    );
    assert_eq!(
        ActiveStoreFinalityV1::new(duplicate),
        Err(MigrationCutoverError::InvalidAtomicParticipants)
    );
    let mut wrong_receipt = active_parts;
    wrong_receipt.ordered_preconditions[0] =
        ActiveStorePreconditionV1::DistributionReceipt(cutover_commitment(31));
    assert_eq!(
        ActiveStoreFinalityV1::new(wrong_receipt),
        Err(MigrationCutoverError::InvalidFinalityPreconditions)
    );

    assert_eq!(
        MigrationCutoverAssociationV1::new(
            repository_ref,
            ReleaseBindingV1::InstallationExact(cutover_commitment(32)),
            MigrationCutoverContextV1::ActiveStore {
                distribution_commit_record_id: commit_record_id,
            },
            material.clone(),
        ),
        Err(MigrationCutoverError::RepositoryReleasePresent)
    );
    let installation_ref =
        CutoverDomainRefV1::new(CutoverDomainV1::Installation, cutover_commitment(2), 7, 9)
            .expect("installation ref");
    assert_eq!(
        MigrationCutoverAssociationV1::new(
            installation_ref,
            ReleaseBindingV1::RepositoryAbsent,
            MigrationCutoverContextV1::ActiveStore {
                distribution_commit_record_id: commit_record_id,
            },
            material,
        ),
        Err(MigrationCutoverError::InstallationReleaseMissing)
    );
}

#[test]
fn pre_store_finality_requires_sealed_attempt_association_seal_and_protected_cas() {
    let domain_ref =
        CutoverDomainRefV1::new(CutoverDomainV1::Installation, cutover_commitment(1), 3, 4)
            .expect("installation ref");
    let release = ReleaseBindingV1::InstallationExact(cutover_commitment(2));
    let material = cutover_material(10);
    let attempt_id = cutover_commitment(30);
    let seal_id = cutover_commitment(31);
    let expected_old = cutover_commitment(32);
    let association = MigrationCutoverAssociationV1::new(
        domain_ref.clone(),
        release.clone(),
        MigrationCutoverContextV1::PreStore {
            sealed_ceremony_attempt_id: attempt_id,
            candidate_seal_id: seal_id,
            expected_old_root_id: expected_old,
        },
        material.clone(),
    )
    .expect("pre-store association");
    let parts = PreStoreFinalityPartsV1 {
        association,
        ordered_preconditions: vec![PreStorePreconditionV1::SealedCeremonyAttempt(attempt_id)],
        atomic_participants: vec![
            PreStoreAtomicParticipantV1::Association(material.association_id),
            PreStoreAtomicParticipantV1::CandidateSeal(PreStoreCandidateSealV1 {
                association_id: material.association_id,
                candidate_seal_id: seal_id,
                sealed_ceremony_attempt_id: attempt_id,
                domain_ref,
                release,
                candidate_store_root_id: material.candidate_store_root_id,
            }),
            PreStoreAtomicParticipantV1::ProtectedExpectedOldCas(ProtectedExpectedOldCasV1 {
                association_id: material.association_id,
                expected_old_root_id: expected_old,
                candidate_store_root_id: material.candidate_store_root_id,
            }),
        ],
    };
    assert!(PreStoreFinalityV1::new(parts.clone()).is_ok());
    let mut partial = parts;
    partial.atomic_participants.pop();
    assert_eq!(
        PreStoreFinalityV1::new(partial),
        Err(MigrationCutoverError::InvalidAtomicParticipants)
    );
}

#[test]
fn migration_successor_preserves_counts_but_keeps_rotations_blocking() {
    assert_eq!(MIGRATION_CUTOVER_SCHEMA_COUNT, 12);
    assert_eq!(MIGRATION_CUTOVER_INVARIANT_COUNT, 23);
    assert_eq!(MIGRATION_CUTOVER_PREDECESSOR_COUNT, 10);
    assert_eq!(MIGRATION_CUTOVER_COMPONENT_COUNT, 50);
    assert_eq!(MIGRATION_CUTOVER_FINALITY_SCHEMA_ID_COUNT, 3);
    assert_eq!(MIGRATION_CUTOVER_FINALITY_EDGE_ROW_COUNT, 11);
    assert_eq!(MIGRATION_CUTOVER_READ_WRITE_COHORT_COUNT, 4);
    assert_eq!(MIGRATION_CUTOVER_READ_WRITE_COHORT_ROW_COUNT, 46);
    assert_eq!(
        (
            C868_SCHEMA_COUNT,
            C868_SUITE_COMPONENT_COUNT,
            C868_RUNTIME_EDGE_COUNT,
        ),
        (38, 62, 61)
    );

    let delta = ExpectedDeltaManifestV1::unresolved();
    assert_eq!(delta.rows().len(), EXPECTED_DELTA_SLOT_COUNT);
    assert!(
        delta
            .rows()
            .iter()
            .all(|row| row.blocking && row.successor_id.is_none())
    );
    let successor = MigrationCutoverSuccessorLiteralV1::new(delta.clone(), [None, None, None])
        .expect("explicitly blocked successor");
    assert_eq!(successor.successor_manifest_id(), None);
    assert!(!successor.h2_h3_can_promote_causal_evidence());
    assert!(!successor.filenames_or_sidecars_are_authority());
    assert!(!successor.old_reader_admission_allowed());

    let mut missing = delta.rows().to_vec();
    missing.pop();
    assert_eq!(
        ExpectedDeltaManifestV1::new(missing),
        Err(MigrationCutoverError::ExpectedDeltaCoverage)
    );
    let mut nonblocking = delta.rows().to_vec();
    nonblocking[0].blocking = false;
    assert_eq!(
        ExpectedDeltaManifestV1::new(nonblocking),
        Err(MigrationCutoverError::UnresolvedDependencyNotBlocking)
    );
    let predecessor = CutoverCommitmentV1::new(PREDECESSOR_ASSOCIATION_SCHEMA_ID)
        .expect("predecessor evidence commitment");
    assert_eq!(
        MigrationCutoverSuccessorLiteralV1::new(delta, [Some(predecessor), None, None],),
        Err(MigrationCutoverError::PredecessorIdentityPromoted)
    );
    assert_eq!(
        SuccessorDependencySlotV1::ALL.map(SuccessorDependencySlotV1::as_str),
        [
            "7138_public_contract",
            "d116_bounded_recovery",
            "h2_causal_join",
            "h3_cancellation_label",
            "efa0_core_catalogs",
            "c868_behavioral_suite",
            "release_binding",
            "writer_compatibility",
        ]
    );
    assert_eq!(
        AssociationConsumptionSetV1::new(vec![cutover_commitment(1), cutover_commitment(1)]),
        Err(MigrationCutoverError::AssociationReused)
    );
    let fabricated_rows = SuccessorDependencySlotV1::ALL
        .into_iter()
        .map(ExpectedDeltaRowV1::unresolved)
        .collect::<Vec<_>>();
    assert!(ExpectedDeltaManifestV1::new(fabricated_rows).is_ok());
}

#[test]
fn generated_literals_match_both_encoders_and_reject_all_mutants() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let build = run(
        repo,
        "python3",
        &[
            "tools/vnext_contracts/stage0/dispatch_cutover/build.py",
            "--check",
        ],
        "dispatch/cutover deterministic build check",
    );
    let build_receipt: serde_json::Value =
        serde_json::from_slice(&build.stdout).expect("parse build receipt");
    assert_eq!(build_receipt["status"], "pass");
    assert_eq!(build_receipt["mismatches"].as_array().unwrap().len(), 0);

    let python = run(
        repo,
        "python3",
        &["tools/vnext_contracts/stage0/dispatch_cutover/validate.py"],
        "Python dispatch/cutover validation",
    );
    let python_receipt: serde_json::Value =
        serde_json::from_slice(&python.stdout).expect("parse Python receipt");
    let ruby = run(
        repo,
        "ruby",
        &["tools/vnext_contracts/stage0/dispatch_cutover/verify.rb"],
        "Ruby dispatch/cutover validation",
    );
    let ruby_receipt: serde_json::Value =
        serde_json::from_slice(&ruby.stdout).expect("parse Ruby receipt");
    assert_eq!(python_receipt["artifact_ids"], ruby_receipt["artifact_ids"]);
    assert_eq!(python_receipt["blocked_dependencies"], 8);

    let mutants = run(
        repo,
        "python3",
        &[
            "tools/vnext_contracts/stage0/dispatch_cutover/validate.py",
            "--mutant-suite",
        ],
        "dispatch/cutover mutant suite",
    );
    let mutant_receipt: serde_json::Value =
        serde_json::from_slice(&mutants.stdout).expect("parse mutant receipt");
    assert_eq!(mutant_receipt["status"], "pass");
    assert_eq!(mutant_receipt["mutants"]["total"], 68);
    assert_eq!(mutant_receipt["mutants"]["rejected"]["python"], 34);
    assert_eq!(mutant_receipt["mutants"]["rejected"]["ruby"], 34);
    assert_eq!(
        mutant_receipt["mutants"]["escaped"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}
