use std::collections::BTreeSet;

use crate::domain::authority::{PrincipalIdV1, SessionIdV1};

use super::projection::{
    InboxQueryV1, PresenceDispositionV1, PresenceSignalKindV1, PresenceSignalV1, conflict_view,
    project_inbox, project_presence, project_scope_overlaps,
};
use super::state::{
    CoordinationStateErrorV1, FocusDeclarationV1, MessageAcknowledgementV1, ScopeDeclarationV1,
    test_adapter,
};
use super::*;

fn principal(seed: &str) -> PrincipalIdV1 {
    PrincipalIdV1::derive(seed).expect("stage7 coordination test invariant")
}

fn session(seed: &str) -> SessionIdV1 {
    SessionIdV1::derive(seed).expect("stage7 coordination test invariant")
}

fn repository() -> RepositoryInstallationRefV1 {
    RepositoryInstallationRefV1::new("repo:stage7").expect("stage7 coordination test invariant")
}

fn audience(actor: PrincipalIdV1) -> (AudienceMemberV1, AudienceEligibilitySnapshotV1) {
    let snapshot = AudienceEligibilitySnapshotV1::new(
        CoordinationAddressV1::Repository {
            repository_installation: repository(),
        },
        vec![actor],
    )
    .expect("stage7 coordination test invariant");
    (AudienceMemberV1::new(snapshot.clone()), snapshot)
}

#[expect(
    clippy::too_many_arguments,
    reason = "test helper mirrors the exact immutable Message constructor"
)]
fn message(
    seed: &str,
    order: u64,
    thread_id: ThreadIdV1,
    revision: u64,
    audience_hash: [u8; 32],
    actor: PrincipalIdV1,
    actor_session: SessionIdV1,
    content: CoordinationMessageContentV1,
) -> MessageV1 {
    MessageV1::new(
        MessageIdV1::derive(seed).expect("stage7 coordination test invariant"),
        StoreOrderV1::new(order, 0).expect("stage7 coordination test invariant"),
        thread_id,
        revision,
        audience_hash,
        actor,
        actor_session,
        content,
        Vec::new(),
        None,
        None,
        order,
    )
    .expect("stage7 coordination test invariant")
}

#[test]
fn exact_nine_coordination_actions_apply_with_owner_local_cas() {
    let actor = principal("stage7-coordination-actor");
    let actor_session = session("stage7-coordination-session");
    let thread_id =
        ThreadIdV1::derive("stage7-thread").expect("stage7 coordination test invariant");
    let (member, snapshot) = audience(actor);
    let thread = ThreadDescriptorV1::new(thread_id, vec![member])
        .expect("stage7 coordination test invariant");
    let first = message(
        "message-1",
        1,
        thread_id,
        1,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:first".into(),
        },
    );
    let mut state = CoordinationStateV1::default();
    let mut actions = Vec::new();
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::PublishInitialMessage {
                thread: thread.clone(),
                message: first.clone(),
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );

    let second = message(
        "message-2",
        2,
        thread_id,
        2,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Question {
            body_ref: "body:question".into(),
        },
    );
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::PublishMessage {
                expected_thread_revision: 1,
                message: second.clone(),
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::AcknowledgeMessage {
                acknowledgement: MessageAcknowledgementV1 {
                    message_ref: second
                        .exact_ref()
                        .expect("stage7 coordination test invariant"),
                    acknowledging_principal: actor,
                    actor_session,
                    via_address: snapshot.address().clone(),
                    audience_hash: thread.audience_hash,
                    eligibility_snapshot_id: *snapshot.id(),
                    eligibility_snapshot_hash: snapshot.semantic_hash(),
                    acknowledged_at: 3,
                },
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );

    let focus_1 = FocusDeclarationV1::new(
        FocusIdV1::derive("focus-1").expect("stage7 coordination test invariant"),
        repository(),
        actor,
        actor_session,
        FocusSubjectV1::Work("work:first".into()),
        TrustedIntervalV1::new(1, 100).expect("stage7 coordination test invariant"),
        StoreOrderV1::new(4, 0).expect("stage7 coordination test invariant"),
    )
    .expect("stage7 coordination test invariant");
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::ReplaceFocus {
                expected_current: None,
                replacement: focus_1.clone(),
                withdrawal_order: None,
                withdrawn_at: None,
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );
    let focus_2 = FocusDeclarationV1::new(
        FocusIdV1::derive("focus-2").expect("stage7 coordination test invariant"),
        repository(),
        actor,
        actor_session,
        FocusSubjectV1::StepBinding("step:second".into()),
        TrustedIntervalV1::new(1, 100).expect("stage7 coordination test invariant"),
        StoreOrderV1::new(5, 0).expect("stage7 coordination test invariant"),
    )
    .expect("stage7 coordination test invariant");
    apply_coordination_mutation(
        &mut state,
        CoordinationMutationV1::ReplaceFocus {
            expected_current: Some(focus_1.focus_id),
            replacement: focus_2.clone(),
            withdrawal_order: Some(
                StoreOrderV1::new(5, 1).expect("stage7 coordination test invariant"),
            ),
            withdrawn_at: Some(5),
        },
    )
    .expect("stage7 coordination test invariant");
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::WithdrawFocus {
                focus_id: focus_2.focus_id,
                principal: actor,
                session: actor_session,
                withdrawn_at: 6,
                store_order: StoreOrderV1::new(6, 0).expect("stage7 coordination test invariant"),
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );

    let scope = ScopeDeclarationV1::new(
        ScopeIdV1::derive("scope-1").expect("stage7 coordination test invariant"),
        repository(),
        actor,
        actor_session,
        vec![
            ScopeAtomV1::new(
                "checkout:one",
                NormalizedScopePathV1::new("/src").expect("stage7 coordination test invariant"),
                ScopeExtentV1::Subtree,
            )
            .expect("stage7 coordination test invariant"),
        ],
        TrustedIntervalV1::new(1, 100).expect("stage7 coordination test invariant"),
        StoreOrderV1::new(7, 0).expect("stage7 coordination test invariant"),
    )
    .expect("stage7 coordination test invariant");
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::PublishScope {
                scope: scope.clone(),
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::WithdrawScope {
                scope_id: scope.scope_id,
                principal: actor,
                session: actor_session,
                withdrawn_at: 8,
                store_order: StoreOrderV1::new(8, 0).expect("stage7 coordination test invariant"),
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );

    let conflict_id =
        ConflictIdV1::derive("conflict-1").expect("stage7 coordination test invariant");
    let assertion = message(
        "message-assert",
        9,
        thread_id,
        3,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::ConflictAssert {
            conflict_id,
            concern_ref: "concern:one".into(),
            explanation_ref: "body:assert".into(),
        },
    );
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::AssertConflict {
                expected_thread_revision: 2,
                message: assertion.clone(),
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );
    let resolution = message(
        "message-resolve",
        10,
        thread_id,
        4,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::ConflictResolve {
            conflict_id,
            assert_ref: assertion
                .exact_ref()
                .expect("stage7 coordination test invariant"),
            resolution: ConflictResolutionKindV1::Reconciled,
            explanation_ref: "body:resolve".into(),
            evidence_refs: vec!["evidence:one".into()],
        },
    );
    actions.push(
        apply_coordination_mutation(
            &mut state,
            CoordinationMutationV1::ResolveConflict {
                expected_thread_revision: 3,
                message: resolution,
            },
        )
        .expect("stage7 coordination test invariant")
        .action_literal(),
    );
    assert_eq!(
        actions,
        [
            "PublishInitialMessage",
            "PublishMessage",
            "AcknowledgeMessage",
            "ReplaceFocus",
            "WithdrawFocus",
            "PublishScope",
            "WithdrawScope",
            "AssertConflict",
            "ResolveConflict",
        ]
    );
    assert!(
        !conflict_view(&state, conflict_id)
            .expect("stage7 coordination test invariant")
            .current
    );
}

#[test]
fn thread_and_focus_races_refuse_stale_expected_state() {
    let actor = principal("race-actor");
    let actor_session = session("race-session");
    let thread_id = ThreadIdV1::derive("race-thread").expect("stage7 coordination test invariant");
    let (member, _) = audience(actor);
    let thread = ThreadDescriptorV1::new(thread_id, vec![member])
        .expect("stage7 coordination test invariant");
    let first = message(
        "race-first",
        1,
        thread_id,
        1,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:first".into(),
        },
    );
    let (base, _) = test_adapter::apply(
        &CoordinationStateV1::default(),
        CoordinationMutationV1::PublishInitialMessage {
            thread: thread.clone(),
            message: first,
        },
    )
    .expect("stage7 coordination test invariant");
    let duplicate_order = message(
        "race-duplicate-order",
        1,
        thread_id,
        2,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:duplicate-order".into(),
        },
    );
    let mut duplicate_order_state = base.clone();
    assert_eq!(
        apply_coordination_mutation(
            &mut duplicate_order_state,
            CoordinationMutationV1::PublishMessage {
                expected_thread_revision: 1,
                message: duplicate_order,
            },
        )
        .unwrap_err(),
        CoordinationStateErrorV1::DuplicateMessageStoreOrder
    );
    let second = message(
        "race-second",
        2,
        thread_id,
        2,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:second".into(),
        },
    );
    let (mut committed, _) = test_adapter::apply(
        &base,
        CoordinationMutationV1::PublishMessage {
            expected_thread_revision: 1,
            message: second,
        },
    )
    .expect("stage7 coordination test invariant");
    let losing = message(
        "race-loser",
        3,
        thread_id,
        2,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:loser".into(),
        },
    );
    assert_eq!(
        apply_coordination_mutation(
            &mut committed,
            CoordinationMutationV1::PublishMessage {
                expected_thread_revision: 1,
                message: losing,
            },
        )
        .unwrap_err(),
        CoordinationStateErrorV1::StaleThread
    );
}

#[test]
fn inbox_presence_and_scope_overlap_are_pure_snapshot_projections() {
    let actor = principal("projection-actor");
    let actor_session = session("projection-session");
    let thread_id =
        ThreadIdV1::derive("projection-thread").expect("stage7 coordination test invariant");
    let (member, _) = audience(actor);
    let thread = ThreadDescriptorV1::new(thread_id, vec![member])
        .expect("stage7 coordination test invariant");
    let first = message(
        "projection-first",
        1,
        thread_id,
        1,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:first".into(),
        },
    );
    let mut state = CoordinationStateV1::default();
    apply_coordination_mutation(
        &mut state,
        CoordinationMutationV1::PublishInitialMessage {
            thread: thread.clone(),
            message: first,
        },
    )
    .expect("stage7 coordination test invariant");
    let second = message(
        "projection-second",
        2,
        thread_id,
        2,
        thread.audience_hash,
        actor,
        actor_session,
        CoordinationMessageContentV1::Information {
            body_ref: "body:second".into(),
        },
    );
    apply_coordination_mutation(
        &mut state,
        CoordinationMutationV1::PublishMessage {
            expected_thread_revision: 1,
            message: second,
        },
    )
    .expect("stage7 coordination test invariant");
    let before = state
        .semantic_hash()
        .expect("stage7 coordination test invariant");
    let query = InboxQueryV1 {
        principal: actor,
        exact_snapshot_hash: [7; 32],
        authorized_threads: BTreeSet::from([thread_id]),
        as_of: 10,
        limit: 1,
        after: None,
    };
    let first_page = project_inbox(&state, &query).expect("stage7 coordination test invariant");
    assert!(first_page.has_more);
    let second_page = project_inbox(
        &state,
        &InboxQueryV1 {
            after: first_page.continuation,
            ..query
        },
    )
    .expect("stage7 coordination test invariant");
    assert_eq!(second_page.rows.len(), 1);
    assert_eq!(
        state
            .semantic_hash()
            .expect("stage7 coordination test invariant"),
        before
    );

    let presence = project_presence(
        actor,
        actor_session,
        [8; 32],
        20,
        5,
        true,
        &[PresenceSignalV1 {
            principal: actor,
            session: actor_session,
            kind: PresenceSignalKindV1::AuthenticatedActivity,
            source_ref: "observation:one".into(),
            observed_at: 18,
            invalidated: false,
        }],
    )
    .expect("stage7 coordination test invariant");
    assert_eq!(presence.disposition, PresenceDispositionV1::RecentSignals);
    let future_presence = project_presence(
        actor,
        actor_session,
        [9; 32],
        20,
        5,
        true,
        &[PresenceSignalV1 {
            principal: actor,
            session: actor_session,
            kind: PresenceSignalKindV1::AuthenticatedActivity,
            source_ref: "observation:future".into(),
            observed_at: 21,
            invalidated: false,
        }],
    )
    .expect("stage7 coordination test invariant");
    assert_eq!(future_presence.disposition, PresenceDispositionV1::Missing);
    assert!(future_presence.considered_signal_refs.is_empty());

    let scope_left = ScopeDeclarationV1::new(
        ScopeIdV1::derive("projection-scope-left").expect("stage7 coordination test invariant"),
        repository(),
        actor,
        actor_session,
        vec![
            ScopeAtomV1::new(
                "checkout:one",
                NormalizedScopePathV1::new("/src").expect("stage7 coordination test invariant"),
                ScopeExtentV1::Subtree,
            )
            .expect("stage7 coordination test invariant"),
        ],
        TrustedIntervalV1::new(1, 100).expect("stage7 coordination test invariant"),
        StoreOrderV1::new(3, 0).expect("stage7 coordination test invariant"),
    )
    .expect("stage7 coordination test invariant");
    let scope_right = ScopeDeclarationV1::new(
        ScopeIdV1::derive("projection-scope-right").expect("stage7 coordination test invariant"),
        repository(),
        principal("other"),
        session("other"),
        vec![
            ScopeAtomV1::new(
                "checkout:one",
                NormalizedScopePathV1::new("/src/lib.rs")
                    .expect("stage7 coordination test invariant"),
                ScopeExtentV1::Exact,
            )
            .expect("stage7 coordination test invariant"),
        ],
        TrustedIntervalV1::new(1, 100).expect("stage7 coordination test invariant"),
        StoreOrderV1::new(4, 0).expect("stage7 coordination test invariant"),
    )
    .expect("stage7 coordination test invariant");
    apply_coordination_mutation(
        &mut state,
        CoordinationMutationV1::PublishScope { scope: scope_left },
    )
    .expect("stage7 coordination test invariant");
    apply_coordination_mutation(
        &mut state,
        CoordinationMutationV1::PublishScope { scope: scope_right },
    )
    .expect("stage7 coordination test invariant");
    assert_eq!(project_scope_overlaps(&state, &repository(), 10).len(), 1);
}
