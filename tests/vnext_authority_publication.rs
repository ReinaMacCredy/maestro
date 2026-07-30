mod support;

use std::fs;

use maestro::domain::authority::{
    ActionRequestIdV1, AuthorityContextIdV1, AuthorityFacadeV1, AuthorityPublicationError,
    AuthorityPublicationLineageV1, AuthorityPublicationPlanError, BootstrapMandateTargetV1,
    ConsentSlotEvaluationFactsV1, HalfOpenValidityV1, IdempotencyKeyIdV1,
    IssueBootstrapMandateInputV1, IssueBootstrapMandatePublicationV1,
    IssueBootstrapMandateRequestV1, ObservationIdV1, PrincipalBindingIdV1, SessionIdV1,
    StateTokenIdV1, TargetActionEffectKindV1, TargetActionOwnerV1, TargetActionProjectionV1,
    TargetActionProtocolV1, TargetExpectedHeadsV1,
};
use maestro::domain::identity::{
    ContractRootIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};
use maestro::domain::persistence::{StoreDomainV1, StoreRoleV1, StoreV1};
use rusqlite::Connection;

use support::TestTempDir;

fn rendered(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn request(key: &str) -> IssueBootstrapMandateRequestV1 {
    let context_id = AuthorityContextIdV1::derive("repository-authority").unwrap();
    let validity = HalfOpenValidityV1::new(100, 200).unwrap();
    let target = TargetActionProjectionV1::new(
        BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
        "recovery-selection-a",
        9,
        TargetActionOwnerV1::Authority,
        TargetActionProtocolV1::RecoveryCommitmentSelection,
        TargetActionEffectKindV1::Rotate,
        "sha256:effect-closure",
        TargetExpectedHeadsV1::new(
            context_id,
            1,
            7,
            11,
            9,
            StateTokenIdV1::derive("target-head").unwrap(),
        )
        .unwrap(),
        validity,
    )
    .unwrap();
    let consent = ConsentSlotEvaluationFactsV1::derive_for_target(&target, validity).unwrap();
    IssueBootstrapMandateRequestV1::try_from(IssueBootstrapMandateInputV1 {
        request_id: ActionRequestIdV1::derive("request-a").unwrap(),
        idempotency_key: IdempotencyKeyIdV1::derive(key).unwrap(),
        context_id,
        actor_binding_id: PrincipalBindingIdV1::derive("actor-binding").unwrap(),
        actor_session_id: SessionIdV1::derive("actor-session").unwrap(),
        responder_binding_id: PrincipalBindingIdV1::derive("responder-binding").unwrap(),
        presentation_observation_id: ObservationIdV1::derive("presentation").unwrap(),
        response_observation_id: ObservationIdV1::derive("affirmative-response").unwrap(),
        target: BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
        target_subject: "recovery-selection-a".to_owned(),
        target_revision: 9,
        consent_slot: consent.binding().clone(),
        supplied_mandates: vec![],
    })
    .unwrap()
}

#[test]
fn public_authority_mutation_requires_an_exact_successor_lineage() {
    let contract_root = ContractRootIdV1::parse(&rendered(70)).unwrap();
    assert_eq!(
        IssueBootstrapMandatePublicationV1::new(
            request("key-a"),
            AuthorityPublicationLineageV1::initial(contract_root),
            None,
        ),
        Err(AuthorityPublicationPlanError::InvalidGenerationLineage)
    );
}

#[test]
fn facade_fails_closed_before_writing_when_the_authoritative_store_head_is_absent() {
    let temp = TestTempDir::new("maestro-vnext-authority-publication");
    let path = fs::canonicalize(temp.path()).unwrap().join("store");
    let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"authority-publication").unwrap();
    let mut store = StoreV1::create(&path, domain).unwrap();
    let publication = IssueBootstrapMandatePublicationV1::new(
        request("key-a"),
        AuthorityPublicationLineageV1::successor(
            ContractRootIdV1::parse(&rendered(71)).unwrap(),
            StoreGenerationIdV1::parse(&rendered(72)).unwrap(),
            StoreHeadIdV1::parse(&rendered(73)).unwrap(),
            StoreObjectIdV1::parse(&rendered(74)).unwrap(),
        ),
        Some([23; 32]),
    )
    .unwrap();

    assert!(matches!(
        AuthorityFacadeV1::new(&mut store).issue_bootstrap_mandate(publication),
        Err(AuthorityPublicationError::InactiveStore)
    ));
    let connection = Connection::open(path.join("store.sqlite3")).unwrap();
    let (clock, objects, idempotency): (i64, i64, i64) = connection
        .query_row(
            "SELECT
                 (SELECT publication_clock FROM store_publication_clock WHERE singleton = 1),
                 (SELECT COUNT(*) FROM store_objects),
                 (SELECT COUNT(*) FROM store_idempotency)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((clock, objects, idempotency), (0, 0, 0));
}
