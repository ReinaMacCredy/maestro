use maestro::domain::evidence::{ClaimEntryV1, SubmissionClaimSetError, SubmissionClaimSetV1};
use maestro::domain::evidence::{ClaimSubjectV1, ClaimV1, ObservationRecordIdV1, SubmissionRefV1};
use maestro::domain::identity::ContractRootIdV1;
use maestro::domain::step::StepSubmissionIdV1;
use maestro::domain::work::{
    WorkIdV1, WorkLifecycleError, WorkLifecycleStateV1, WorkRecordV1, WorkRecordWriterV1,
    WorkRevisionV1, WorkSubmissionError, WorkSubmissionIdV1, WorkSubmissionV1,
    WorkTransitionKindV1, WorkTransitionReasonV1, WorkTransitionV1,
};

fn claim(submission: SubmissionRefV1, work_id: WorkIdV1, index: u8) -> ClaimV1 {
    claim_for_root(submission, work_id, contract_root(42), index)
}

fn claim_for_root(
    submission: SubmissionRefV1,
    work_id: WorkIdV1,
    root: ContractRootIdV1,
    index: u8,
) -> ClaimV1 {
    claim_for_subject(submission, work_id, root, vec![], index)
}

fn claim_for_subject(
    submission: SubmissionRefV1,
    work_id: WorkIdV1,
    root: ContractRootIdV1,
    current_step_submissions: Vec<StepSubmissionIdV1>,
    index: u8,
) -> ClaimV1 {
    ClaimV1::new(
        submission,
        ClaimSubjectV1::for_work(work_id, root, current_step_submissions).unwrap(),
        [index; 32],
        vec![ObservationRecordIdV1::from_bytes([index.saturating_add(100); 32]).unwrap()],
    )
    .unwrap()
}

fn contract_root(byte: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
}

fn submission(
    work_id: WorkIdV1,
    expected_revision: u64,
    seed: &str,
    claim_count: u8,
) -> WorkSubmissionV1 {
    let id = WorkSubmissionIdV1::derive(seed).unwrap();
    let submission_ref = SubmissionRefV1::for_work(id).unwrap();
    let claims: Vec<_> = (1..=claim_count)
        .map(|index| claim(submission_ref, work_id, index))
        .collect();
    WorkSubmissionV1::publish_from_claims(
        WorkRecordWriterV1::Work,
        id,
        work_id,
        contract_root(42),
        expected_revision,
        &claims,
    )
    .unwrap()
}

fn ready(work: &WorkRecordV1) -> WorkRecordV1 {
    work.apply(
        WorkRecordWriterV1::Work,
        work.revision(),
        WorkTransitionV1::PublishInitialContract,
    )
    .unwrap()
}

#[test]
fn pure_lifecycle_appends_revision_facts_and_refuses_unverified_completion() {
    let id = WorkIdV1::derive("lifecycle-work").unwrap();
    let draft = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, id).unwrap();
    let ready = ready(&draft);
    let active = ready
        .apply(
            WorkRecordWriterV1::Work,
            ready.revision(),
            WorkTransitionV1::AcquireFirstStepExecution,
        )
        .unwrap();
    let candidate = submission(id, active.revision().get(), "unverified-submission", 1);
    assert_eq!(
        active
            .apply(
                WorkRecordWriterV1::Work,
                active.revision(),
                WorkTransitionV1::SubmitWorkCompletion {
                    submission: Box::new(candidate),
                },
            )
            .unwrap_err(),
        WorkLifecycleError::UnverifiedCompletionBasis
    );
    let cancelled = active
        .apply(
            WorkRecordWriterV1::Work,
            active.revision(),
            WorkTransitionV1::CancelWork {
                reason: WorkTransitionReasonV1::new("cancel requested").unwrap(),
            },
        )
        .unwrap();

    assert_eq!(draft.revision().get(), 1);
    assert_eq!(draft.history().len(), 1);
    assert_eq!(cancelled.revision().get(), 4);
    assert_eq!(cancelled.history().len(), 4);
    assert_eq!(cancelled.state(), &WorkLifecycleStateV1::Cancelled);
    assert!(cancelled.submissions().is_empty());
    assert!(cancelled.current_submission().is_none());
    assert_eq!(
        cancelled.history()[3].transition(),
        WorkTransitionKindV1::CancelWork
    );
    assert_eq!(
        cancelled.history()[3].reason().unwrap().as_str(),
        "cancel requested"
    );
    assert_eq!(
        cancelled.history()[3].canonical_bytes().unwrap(),
        cancelled.history()[3].canonical_bytes().unwrap()
    );
}

#[test]
fn every_lifecycle_state_accepts_exactly_the_closed_transition_set() {
    let id = WorkIdV1::derive("transition-matrix").unwrap();
    let draft = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, id).unwrap();
    let ready = ready(&draft);
    let active = ready
        .apply(
            WorkRecordWriterV1::Work,
            ready.revision(),
            WorkTransitionV1::AcquireFirstStepExecution,
        )
        .unwrap();
    let current_submission_id = WorkSubmissionIdV1::derive("matrix-submission-id").unwrap();
    let cancelled = active
        .apply(
            WorkRecordWriterV1::Work,
            active.revision(),
            WorkTransitionV1::CancelWork {
                reason: WorkTransitionReasonV1::new("cancelled").unwrap(),
            },
        )
        .unwrap();
    for record in [&draft, &ready, &active, &cancelled] {
        let current = record
            .current_submission()
            .map_or(current_submission_id, WorkSubmissionV1::id);
        let candidates = vec![
            WorkTransitionV1::PublishInitialContract,
            WorkTransitionV1::AcquireFirstStepExecution,
            WorkTransitionV1::SubmitWorkCompletion {
                submission: Box::new(submission(
                    id,
                    record.revision().get(),
                    &format!("candidate-{}", record.revision().get()),
                    1,
                )),
            },
            WorkTransitionV1::CompleteWork {
                submission_id: current,
            },
            WorkTransitionV1::RejectWorkCompletion {
                submission_id: current,
                reason: WorkTransitionReasonV1::new("rejected").unwrap(),
            },
            WorkTransitionV1::ReturnWorkForRepair {
                submission_id: current,
                reason: WorkTransitionReasonV1::new("repair").unwrap(),
            },
            WorkTransitionV1::AmendContract {
                invalidated_submission_id: matches!(
                    record.state(),
                    WorkLifecycleStateV1::AwaitingAcceptance
                )
                .then_some(current),
                reason: WorkTransitionReasonV1::new("material amendment").unwrap(),
            },
            WorkTransitionV1::CancelWork {
                reason: WorkTransitionReasonV1::new("cancelled").unwrap(),
            },
        ];

        for transition in candidates {
            let kind = transition.kind();
            let expected_legal = match record.state() {
                WorkLifecycleStateV1::Draft => matches!(
                    kind,
                    WorkTransitionKindV1::PublishInitialContract | WorkTransitionKindV1::CancelWork
                ),
                WorkLifecycleStateV1::Ready => matches!(
                    kind,
                    WorkTransitionKindV1::AcquireFirstStepExecution
                        | WorkTransitionKindV1::AmendContract
                        | WorkTransitionKindV1::CancelWork
                ),
                WorkLifecycleStateV1::Active => matches!(
                    kind,
                    WorkTransitionKindV1::AmendContract | WorkTransitionKindV1::CancelWork
                ),
                WorkLifecycleStateV1::AwaitingAcceptance => matches!(
                    kind,
                    WorkTransitionKindV1::CompleteWork
                        | WorkTransitionKindV1::RejectWorkCompletion
                        | WorkTransitionKindV1::ReturnWorkForRepair
                        | WorkTransitionKindV1::AmendContract
                        | WorkTransitionKindV1::CancelWork
                ),
                WorkLifecycleStateV1::Completed
                | WorkLifecycleStateV1::Cancelled
                | WorkLifecycleStateV1::Superseded { .. } => false,
            };
            assert_eq!(
                record
                    .apply(WorkRecordWriterV1::Work, record.revision(), transition)
                    .is_ok(),
                expected_legal,
                "state {:?}, transition {kind:?}",
                record.state()
            );
        }
    }
}

#[test]
fn amendment_preserves_ready_and_active_while_completion_requires_repository_admission() {
    let id = WorkIdV1::derive("amendment-state-matrix").unwrap();
    let draft = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, id).unwrap();
    let ready = ready(&draft);
    let amended_ready = ready
        .apply(
            WorkRecordWriterV1::Work,
            ready.revision(),
            WorkTransitionV1::AmendContract {
                invalidated_submission_id: None,
                reason: WorkTransitionReasonV1::new("ready amendment").unwrap(),
            },
        )
        .unwrap();
    assert_eq!(amended_ready.state(), &WorkLifecycleStateV1::Ready);

    let active = amended_ready
        .apply(
            WorkRecordWriterV1::Work,
            amended_ready.revision(),
            WorkTransitionV1::AcquireFirstStepExecution,
        )
        .unwrap();
    let amended_active = active
        .apply(
            WorkRecordWriterV1::Work,
            active.revision(),
            WorkTransitionV1::AmendContract {
                invalidated_submission_id: None,
                reason: WorkTransitionReasonV1::new("active amendment").unwrap(),
            },
        )
        .unwrap();
    assert_eq!(amended_active.state(), &WorkLifecycleStateV1::Active);

    let completion = submission(
        id,
        amended_active.revision().get(),
        "amendment-awaiting-submission",
        1,
    );
    assert_eq!(
        amended_active
            .apply(
                WorkRecordWriterV1::Work,
                amended_active.revision(),
                WorkTransitionV1::SubmitWorkCompletion {
                    submission: Box::new(completion),
                },
            )
            .unwrap_err(),
        WorkLifecycleError::UnverifiedCompletionBasis
    );

    assert!(
        ready
            .apply(
                WorkRecordWriterV1::Work,
                ready.revision(),
                WorkTransitionV1::AmendContract {
                    invalidated_submission_id: Some(
                        WorkSubmissionIdV1::derive("wrong-ready-submission").unwrap(),
                    ),
                    reason: WorkTransitionReasonV1::new("wrong ready invalidation").unwrap(),
                },
            )
            .is_err()
    );
}

#[test]
fn submission_binds_exactly_one_existing_nonempty_one_or_many_claim_set() {
    let work_id = WorkIdV1::derive("submission-work").unwrap();
    let one = submission(work_id, 3, "one-claim", 1);
    let many = submission(work_id, 3, "many-claims", 4);
    assert_eq!(one.claim_set().claim_count(), 1);
    assert_eq!(many.claim_set().claim_count(), 4);
    assert_eq!(
        one.canonical_bytes().unwrap(),
        one.canonical_bytes().unwrap()
    );
    assert_ne!(one.digest(), many.digest());

    let empty_id = WorkSubmissionIdV1::derive("empty-claim-set").unwrap();
    let empty_ref = SubmissionRefV1::for_work(empty_id).unwrap();
    assert_eq!(
        SubmissionClaimSetV1::from_claims(empty_ref, &[]).unwrap_err(),
        SubmissionClaimSetError::Empty
    );
    let id = WorkSubmissionIdV1::derive("mismatch").unwrap();
    let other_id = WorkSubmissionIdV1::derive("other").unwrap();
    let other_ref = SubmissionRefV1::for_work(other_id).unwrap();
    let mismatched =
        SubmissionClaimSetV1::from_claims(other_ref, &[claim(other_ref, work_id, 1)]).unwrap();
    assert_eq!(
        WorkSubmissionV1::publish(
            WorkRecordWriterV1::Work,
            id,
            work_id,
            contract_root(1),
            3,
            mismatched,
        )
        .unwrap_err(),
        WorkSubmissionError::ClaimSetSubmissionMismatch
    );

    let carrier_id = WorkSubmissionIdV1::derive("stage0-carrier").unwrap();
    let carrier = SubmissionClaimSetV1::from_stage0_carrier(
        carrier_id.render().into_bytes(),
        vec![ClaimEntryV1::from_stage0_carrier(b"claim".to_vec(), [1; 32], [2; 32]).unwrap()],
    )
    .unwrap();
    assert_eq!(
        WorkSubmissionV1::publish(
            WorkRecordWriterV1::Work,
            carrier_id,
            work_id,
            contract_root(1),
            3,
            carrier,
        )
        .unwrap_err(),
        WorkSubmissionError::NonAuthoritativeClaimSet
    );
}

#[test]
fn work_submission_refuses_wrong_or_mixed_work_and_root_subjects() {
    let work_id = WorkIdV1::derive("subject-work").unwrap();
    let other_work_id = WorkIdV1::derive("other-subject-work").unwrap();
    let id = WorkSubmissionIdV1::derive("subject-submission").unwrap();
    let submission_ref = SubmissionRefV1::for_work(id).unwrap();
    let correct = claim_for_root(submission_ref, work_id, contract_root(42), 1);
    let wrong_work = claim_for_root(submission_ref, other_work_id, contract_root(42), 2);
    let wrong_root = claim_for_root(submission_ref, work_id, contract_root(43), 3);
    for claims in [
        vec![wrong_work.clone()],
        vec![wrong_root.clone()],
        vec![correct.clone(), wrong_work],
        vec![correct.clone(), wrong_root.clone()],
    ] {
        assert_eq!(
            WorkSubmissionV1::publish_from_claims(
                WorkRecordWriterV1::Work,
                id,
                work_id,
                contract_root(42),
                3,
                &claims,
            )
            .unwrap_err(),
            WorkSubmissionError::ClaimSubjectMismatch
        );
    }

    let projected =
        SubmissionClaimSetV1::from_claims(submission_ref, &[correct, wrong_root]).unwrap();
    assert_eq!(
        WorkSubmissionV1::publish(
            WorkRecordWriterV1::Work,
            id,
            work_id,
            contract_root(42),
            3,
            projected,
        )
        .unwrap_err(),
        WorkSubmissionError::ClaimSubjectMismatch
    );
}

#[test]
fn work_submission_requires_one_exact_current_step_submission_closure() {
    let work_id = WorkIdV1::derive("exact-subject-work").unwrap();
    let id = WorkSubmissionIdV1::derive("exact-subject-submission").unwrap();
    let submission_ref = SubmissionRefV1::for_work(id).unwrap();
    let first_step = StepSubmissionIdV1::derive("subject-step-one").unwrap();
    let second_step = StepSubmissionIdV1::derive("subject-step-two").unwrap();
    let first = claim_for_subject(
        submission_ref,
        work_id,
        contract_root(42),
        vec![first_step],
        1,
    );
    let conflicting = claim_for_subject(
        submission_ref,
        work_id,
        contract_root(42),
        vec![second_step],
        2,
    );

    assert_eq!(
        WorkSubmissionV1::publish_from_claims(
            WorkRecordWriterV1::Work,
            id,
            work_id,
            contract_root(42),
            3,
            &[first.clone(), conflicting],
        )
        .unwrap_err(),
        WorkSubmissionError::ClaimSubjectMismatch
    );

    let second = claim_for_subject(
        submission_ref,
        work_id,
        contract_root(42),
        vec![first_step],
        2,
    );
    let submission = WorkSubmissionV1::publish_from_claims(
        WorkRecordWriterV1::Work,
        id,
        work_id,
        contract_root(42),
        3,
        &[first, second],
    )
    .unwrap();
    assert_eq!(
        submission.subject().current_step_submissions(),
        &[first_step]
    );
}

#[test]
fn foreign_writers_stale_revisions_unknown_tags_and_terminal_reopen_refuse() {
    let id = WorkIdV1::derive("negative-writers").unwrap();
    for writer in WorkRecordWriterV1::ALL {
        if writer == WorkRecordWriterV1::Work {
            continue;
        }
        assert_eq!(
            WorkRecordV1::create_draft(writer, id).unwrap_err(),
            WorkLifecycleError::ForeignWriter(writer)
        );
        let submission_id = WorkSubmissionIdV1::derive(&format!("foreign-{writer:?}")).unwrap();
        let submission_ref = SubmissionRefV1::for_work(submission_id).unwrap();
        let claims =
            SubmissionClaimSetV1::from_claims(submission_ref, &[claim(submission_ref, id, 1)])
                .unwrap();
        assert_eq!(
            WorkSubmissionV1::publish(writer, submission_id, id, contract_root(1), 1, claims,)
                .unwrap_err(),
            WorkSubmissionError::ForeignWriter(writer)
        );
    }

    let draft = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, id).unwrap();
    let stale = WorkRevisionV1::new(2).unwrap();
    assert!(matches!(
        draft.apply(
            WorkRecordWriterV1::Work,
            stale,
            WorkTransitionV1::PublishInitialContract
        ),
        Err(WorkLifecycleError::StaleRevision { .. })
    ));
    for writer in WorkRecordWriterV1::ALL {
        if writer != WorkRecordWriterV1::Work {
            assert_eq!(
                draft
                    .apply(
                        writer,
                        draft.revision(),
                        WorkTransitionV1::PublishInitialContract
                    )
                    .unwrap_err(),
                WorkLifecycleError::ForeignWriter(writer)
            );
        }
    }
    assert_eq!(
        WorkLifecycleStateV1::from_tag(99, None).unwrap_err(),
        WorkLifecycleError::UnknownStateTag(99)
    );
    assert_eq!(
        WorkTransitionKindV1::from_tag(99).unwrap_err(),
        WorkLifecycleError::UnknownTransitionTag(99)
    );
    assert!(WorkTransitionReasonV1::new("").is_err());
    assert!(WorkTransitionReasonV1::new("x".repeat(1_025)).is_err());

    let cancelled = draft
        .apply(
            WorkRecordWriterV1::Work,
            draft.revision(),
            WorkTransitionV1::CancelWork {
                reason: WorkTransitionReasonV1::new("cancel").unwrap(),
            },
        )
        .unwrap();
    assert!(matches!(
        cancelled.apply(
            WorkRecordWriterV1::Work,
            cancelled.revision(),
            WorkTransitionV1::PublishInitialContract
        ),
        Err(WorkLifecycleError::IllegalTransition { .. })
    ));
}
