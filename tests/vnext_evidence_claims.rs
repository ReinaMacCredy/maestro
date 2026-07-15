use maestro::domain::vnext::contract::submission_claim::{
    SubmissionClaimSetError, SubmissionClaimSetV1,
};
use maestro::domain::vnext::evidence::{
    ClaimError, ClaimSubjectV1, ClaimV1, EvidenceIdentityError, ObservationRecordIdV1,
    SubmissionRefV1,
};
use maestro::domain::vnext::identity::ContractRootIdV1;
use maestro::domain::vnext::work::{WorkIdV1, WorkSubmissionIdV1};

fn hash(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn contract_root(byte: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
}

fn work_claim(submission: SubmissionRefV1, proposition: u8, observation: u8) -> ClaimV1 {
    ClaimV1::new(
        submission,
        ClaimSubjectV1::for_work(
            WorkIdV1::derive("evidence-owner-work").unwrap(),
            contract_root(9),
            vec![],
        )
        .unwrap(),
        hash(proposition),
        vec![ObservationRecordIdV1::from_bytes(hash(observation)).unwrap()],
    )
    .unwrap()
}

#[test]
fn claim_identity_and_record_bind_one_exact_submission_deterministically() {
    let work_id = WorkIdV1::derive("evidence-owner-work").unwrap();
    let submission_id = WorkSubmissionIdV1::derive("evidence-owner-submission").unwrap();
    let submission = SubmissionRefV1::for_work(submission_id).unwrap();
    let subject = ClaimSubjectV1::for_work(work_id, contract_root(9), vec![]).unwrap();
    let first_observation = ObservationRecordIdV1::from_bytes(hash(31)).unwrap();
    let second_observation = ObservationRecordIdV1::from_bytes(hash(32)).unwrap();

    let claim = ClaimV1::new(
        submission,
        subject.clone(),
        hash(41),
        vec![second_observation, first_observation],
    )
    .unwrap();
    let reordered = ClaimV1::new(
        submission,
        subject,
        hash(41),
        vec![first_observation, second_observation],
    )
    .unwrap();

    assert_eq!(claim, reordered);
    assert_eq!(claim.submission(), submission);
    assert_eq!(claim.claim_id().render().len(), 71);
    assert_ne!(claim.claim_id().as_bytes(), &[0; 32]);
    assert_ne!(claim.record_hash(), &[0; 32]);
    assert_eq!(
        claim.canonical_bytes().unwrap(),
        reordered.canonical_bytes().unwrap()
    );
}

#[test]
fn authoritative_claim_set_is_derived_only_from_claims_bound_to_one_submission() {
    let submission =
        SubmissionRefV1::for_work(WorkSubmissionIdV1::derive("authoritative-claim-set").unwrap())
            .unwrap();
    let first = work_claim(submission, 1, 31);
    let second = work_claim(submission, 2, 32);
    let claim_set =
        SubmissionClaimSetV1::from_claims(submission, &[second.clone(), first.clone()]).unwrap();

    assert_eq!(claim_set.submission_ref(), Some(submission));
    assert!(claim_set.is_authoritative());
    assert_eq!(claim_set.claim_subjects().unwrap().len(), 2);
    assert_eq!(claim_set.claim_count(), 2);
    assert_eq!(
        claim_set.entries()[0].claim_id(),
        first.claim_id().render().as_bytes()
    );
    assert_eq!(
        claim_set.entries()[1].claim_id(),
        second.claim_id().render().as_bytes()
    );

    let other_submission = SubmissionRefV1::for_work(
        WorkSubmissionIdV1::derive("other-authoritative-claim-set").unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionClaimSetV1::from_claims(other_submission, &[first]).unwrap_err(),
        SubmissionClaimSetError::CrossSubmissionClaim
    );
    assert_eq!(
        SubmissionClaimSetV1::from_claims(submission, &[second.clone(), second]).unwrap_err(),
        SubmissionClaimSetError::DuplicateClaimId
    );

    let duplicate_proposition = work_claim(submission, 1, 33);
    assert_eq!(
        SubmissionClaimSetV1::from_claims(
            submission,
            &[work_claim(submission, 1, 31), duplicate_proposition],
        )
        .unwrap_err(),
        SubmissionClaimSetError::DuplicateNormalizedProposition
    );
}

#[test]
fn zero_or_missing_claim_identity_material_is_rejected_before_publication() {
    let submission_id = WorkSubmissionIdV1::derive("invalid-claim-material").unwrap();
    let submission = SubmissionRefV1::for_work(submission_id).unwrap();
    let subject = ClaimSubjectV1::for_work(
        WorkIdV1::derive("invalid-claim-work").unwrap(),
        contract_root(9),
        vec![],
    )
    .unwrap();
    let observation = ObservationRecordIdV1::from_bytes(hash(31)).unwrap();

    assert_eq!(
        ClaimV1::new(submission, subject.clone(), [0; 32], vec![observation]).unwrap_err(),
        ClaimError::Identity(EvidenceIdentityError::MissingReference(
            "normalized Claim proposition"
        ))
    );
    assert_eq!(
        ClaimV1::new(submission, subject, hash(41), vec![]).unwrap_err(),
        ClaimError::EmptyObservationReferences
    );
    assert_eq!(
        ObservationRecordIdV1::from_bytes([0; 32]).unwrap_err(),
        EvidenceIdentityError::MissingReference("ObservationRecordIdV1")
    );
}

#[test]
fn authoritative_claim_set_has_no_second_claim_count_cap() {
    let submission =
        SubmissionRefV1::for_work(WorkSubmissionIdV1::derive("claim-set-over-common-cap").unwrap())
            .unwrap();
    let subject = ClaimSubjectV1::for_work(
        WorkIdV1::derive("claim-set-over-common-cap-work").unwrap(),
        contract_root(9),
        vec![],
    )
    .unwrap();
    let observation = ObservationRecordIdV1::from_bytes(hash(31)).unwrap();
    let claims: Vec<_> = (1_u64..=4_097)
        .map(|index| {
            let mut proposition = [0_u8; 32];
            proposition[..8].copy_from_slice(&index.to_be_bytes());
            ClaimV1::new(submission, subject.clone(), proposition, vec![observation]).unwrap()
        })
        .collect();

    assert_eq!(
        SubmissionClaimSetV1::from_claims(submission, &claims)
            .unwrap()
            .claim_count(),
        4_097
    );
}
