use maestro::domain::vnext::evidence::{
    ClaimError, ClaimSubjectV1, ClaimV1, EvidenceIdentityError, ObservationRecordIdV1,
    SubmissionRefV1,
};
use maestro::domain::vnext::evidence::{SubmissionClaimSetError, SubmissionClaimSetV1};
use maestro::domain::vnext::identity::ContractRootIdV1;
use maestro::domain::vnext::work::{
    WorkIdV1, WorkRecordWriterV1, WorkSubmissionIdV1, WorkSubmissionV1,
};
use sha2::{Digest, Sha256};

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

#[test]
fn stage3_claim_and_work_submission_v1_vectors_remain_exact() {
    let work_id = WorkIdV1::derive("stage3-predecessor-work").unwrap();
    let submission_id = WorkSubmissionIdV1::derive("stage3-predecessor-submission").unwrap();
    let submission = SubmissionRefV1::for_work(submission_id).unwrap();
    let claim = ClaimV1::new(
        submission,
        ClaimSubjectV1::for_work(work_id, contract_root(9), vec![]).unwrap(),
        hash(41),
        vec![ObservationRecordIdV1::from_bytes(hash(31)).unwrap()],
    )
    .unwrap();
    let work_submission = WorkSubmissionV1::publish_from_claims(
        WorkRecordWriterV1::Work,
        submission_id,
        work_id,
        contract_root(9),
        3,
        std::slice::from_ref(&claim),
    )
    .unwrap();
    let claim_bytes = claim.canonical_bytes().unwrap();
    let submission_bytes = work_submission.canonical_bytes().unwrap();
    assert_eq!(
        hex(&claim_bytes),
        "860158203b821ce98a069d8fe09ac0180a9098c29920fb591cb6056b7e20cc3022d9281482015820070b852d0068b30535cce64ca573f6c7f4a08669bf8ff13b5cc50b551157c45b8401582035e012be549c59e666982129745dcf7ea0552b05206ad89c59e866fc1fb975505820090909090909090909090909090909090909090909090909090909090909090980582029292929292929292929292929292929292929292929292929292929292929298158201f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f"
    );
    assert_eq!(
        hex(&Sha256::digest(&claim_bytes)),
        "e2694c1b40da76efac793c64ffb4b67f6e036a2c6dad6bb09b40c864e0f723b8"
    );
    assert_eq!(
        hex(&submission_bytes),
        "85015820070b852d0068b30535cce64ca573f6c7f4a08669bf8ff13b5cc50b551157c45b8401582035e012be549c59e666982129745dcf7ea0552b05206ad89c59e866fc1fb975505820090909090909090909090909090909090909090909090909090909090909090980038478477368613235363a3037306238353264303036386233303533356363653634636135373366366337663461303836363962663866663133623563633530623535313135376334356201818378477368613235363a336238323163653938613036396438666530396163303138306139303938633239393230666235393163623630353662376532306363333032326439323831345820292929292929292929292929292929292929292929292929292929292929292958207968b890c00dec67f9fbb7dd0e4cf9e9b7ea05a7bd2ba5392730589d7af0775d5820d40c35cc048bbde5e6bb6ba35728b46bf7780f85822a83deca9b7fe51bd01210"
    );
    assert_eq!(
        hex(&Sha256::digest(&submission_bytes)),
        "0109adbf4a720d76efc7f1deff4b9f61d34b2fadecf54c9ed0f3f571b8289926"
    );
    assert_eq!(ClaimV1::from_canonical_bytes(&claim_bytes).unwrap(), claim);
    assert_eq!(
        WorkSubmissionV1::from_canonical_bytes(&submission_bytes, &[claim]).unwrap(),
        work_submission
    );
    let mut claim_mutant = claim_bytes;
    claim_mutant[1] ^= 1;
    assert!(ClaimV1::from_canonical_bytes(&claim_mutant).is_err());
    let mut submission_mutant = submission_bytes;
    submission_mutant[1] ^= 1;
    assert!(WorkSubmissionV1::from_canonical_bytes(&submission_mutant, &[]).is_err());
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
