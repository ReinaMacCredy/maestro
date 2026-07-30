use maestro::domain::design::{
    AlternativeConsequenceV1, AlternativeV1, DecisionIdV1, DecisionRevisionV1, DecisionStateV1,
    DecisionV1, DecisionV1Error, ExactRecordRefV1, WorkDecisionEligibilityV1, WorkIdV1,
};
use maestro::domain::identity::{ContractRootIdV1, StoreDomainIdV1};

fn exact(seed: u8) -> ExactRecordRefV1 {
    ExactRecordRefV1::from_digest([seed; 32])
}

fn root(seed: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&format!("sha256:{}", format!("{seed:02x}").repeat(32)))
        .expect("Contract Root identity")
}

fn repository() -> StoreDomainIdV1 {
    StoreDomainIdV1::parse(&format!("sha256:{}", "51".repeat(32))).expect("repository Store domain")
}

fn work() -> WorkIdV1 {
    WorkIdV1::derive("work-a").expect("Work identity")
}

fn decision_id(value: &str) -> DecisionIdV1 {
    DecisionIdV1::new(value).expect("Decision identity")
}

fn alternatives() -> Vec<AlternativeV1> {
    vec![
        AlternativeV1::new(
            b"keep rationale only".to_vec(),
            b"no Contract change".to_vec(),
            AlternativeConsequenceV1::NoContractEffect,
        )
        .expect("alternative"),
        AlternativeV1::new(
            b"record another rationale".to_vec(),
            b"still no Contract change".to_vec(),
            AlternativeConsequenceV1::NoContractEffect,
        )
        .expect("alternative"),
    ]
}

fn revision(
    id: DecisionIdV1,
    ordinal: u32,
    parent: Option<&DecisionRevisionV1>,
) -> DecisionRevisionV1 {
    DecisionRevisionV1::new(
        id,
        ordinal,
        parent.map(|revision| *revision.revision_id()),
        b"which exact alternative?".to_vec(),
        exact(22),
        root(1),
        alternatives(),
    )
    .expect("Decision Revision")
}

#[test]
fn revisions_require_two_or_more_unique_alternatives_and_closed_parentage() {
    let id = decision_id("decision-cardinality");
    let only = AlternativeV1::new(
        b"only".to_vec(),
        b"only".to_vec(),
        AlternativeConsequenceV1::NoContractEffect,
    )
    .expect("alternative");
    assert_eq!(
        DecisionRevisionV1::new(
            id.clone(),
            1,
            None,
            b"invalid".to_vec(),
            exact(1),
            root(1),
            vec![only]
        ),
        Err(DecisionV1Error::AlternativeCardinality)
    );

    let first = revision(id.clone(), 1, None);
    assert_eq!(first.ordinal(), 1);
    assert!(first.parent_revision_id().is_none());
    assert_eq!(first.alternatives().len(), 2);
    assert_eq!(
        DecisionRevisionV1::new(
            id,
            2,
            None,
            b"invalid successor".to_vec(),
            exact(1),
            root(1),
            alternatives()
        ),
        Err(DecisionV1Error::InvalidRevisionParentage)
    );
}

#[test]
fn open_decisions_append_exact_successors_and_withdraw_once() {
    let id = decision_id("decision-lifecycle");
    let first = revision(id.clone(), 1, None);
    let decision =
        DecisionV1::new(repository(), work(), id.clone(), first.clone()).expect("open Decision");
    let second = revision(id, 2, Some(&first));
    let advanced = decision
        .append_revision(
            first.revision_id(),
            second,
            WorkDecisionEligibilityV1::Eligible,
        )
        .expect("append revision");
    assert_eq!(advanced.revisions().len(), 2);

    let withdrawn = advanced
        .withdraw(
            b"fork is no longer relevant".to_vec(),
            WorkDecisionEligibilityV1::Eligible,
        )
        .expect("withdraw Decision");
    assert!(matches!(withdrawn.state(), DecisionStateV1::Withdrawn(_)));
    assert_eq!(
        withdrawn.withdraw(b"too late".to_vec(), WorkDecisionEligibilityV1::Eligible),
        Err(DecisionV1Error::DecisionNotOpen)
    );
    assert_eq!(
        decision.withdraw(
            b"terminal".to_vec(),
            WorkDecisionEligibilityV1::TerminalWork
        ),
        Err(DecisionV1Error::TerminalWorkRejectsDecisionChange)
    );
}
