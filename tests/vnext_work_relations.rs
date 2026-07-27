use maestro::domain::identity::{ContractRootIdV1, StoreDomainIdV1};
use maestro::domain::persistence::{StoreDomainV1, StoreRoleV1};
use maestro::domain::work::{
    ExactStepRevisionRefV1, WorkIdV1, WorkLifecycleStateV1, WorkRecordWriterV1,
    WorkRelationAdmissionV1, WorkRelationEndpointV1, WorkRelationError, WorkRelationGraphV1,
    WorkRelationIdV1, WorkRelationKindV1, WorkRelationRecordV1, WorkRequirementIdV1,
    WorkRequirementScopeV1, WorkRequirementV1, WorkRevisionV1, WorkSnapshotV1,
};

fn root(value: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&format!("sha256:{}", format!("{value:02x}").repeat(32))).unwrap()
}

fn snapshot(
    repository_id: StoreDomainIdV1,
    seed: &str,
    state: WorkLifecycleStateV1,
    root_value: u8,
) -> WorkSnapshotV1 {
    WorkSnapshotV1::new(
        repository_id,
        WorkIdV1::derive(seed).unwrap(),
        WorkRevisionV1::new(1).unwrap(),
        state,
        vec![root(root_value)],
    )
    .unwrap()
}

fn endpoint(snapshot: &WorkSnapshotV1) -> WorkRelationEndpointV1 {
    WorkRelationEndpointV1::new(
        snapshot.repository_id(),
        snapshot.work_id(),
        snapshot.revision(),
        snapshot.published_contract_roots().first().copied(),
    )
    .unwrap()
}

fn relation(
    kind: WorkRelationKindV1,
    source: &WorkSnapshotV1,
    target: &WorkSnapshotV1,
    seed: &str,
) -> WorkRelationRecordV1 {
    WorkRelationRecordV1::new(
        WorkRelationIdV1::derive(seed).unwrap(),
        kind,
        endpoint(source),
        endpoint(target),
        format!("reason-{seed}"),
        format!("provenance-{seed}"),
        format!("key-{seed}"),
    )
    .unwrap()
}

fn requirement(
    repository_id: StoreDomainIdV1,
    consumer: &WorkSnapshotV1,
    target: &WorkSnapshotV1,
    seed: &str,
    scope: WorkRequirementScopeV1,
) -> WorkRequirementV1 {
    WorkRequirementV1::new(
        WorkRequirementIdV1::derive(seed).unwrap(),
        repository_id,
        consumer.work_id(),
        repository_id,
        target.work_id(),
        target.published_contract_roots()[0],
        scope,
    )
    .unwrap()
}

fn repository_id(seed: &str) -> StoreDomainIdV1 {
    StoreDomainV1::derive(StoreRoleV1::Repository, seed.as_bytes())
        .unwrap()
        .id()
}

#[test]
fn requirement_scopes_are_closed_and_canonical() {
    let before_step = ExactStepRevisionRefV1::new("step-release", 4).unwrap();
    let scopes = [
        WorkRequirementScopeV1::BeforeExecution,
        WorkRequirementScopeV1::BeforeStep(before_step.clone()),
        WorkRequirementScopeV1::BeforeCompletion,
    ];
    for (index, scope) in scopes.iter().enumerate() {
        assert_eq!(scope.tag(), u64::try_from(index + 1).unwrap());
    }
    assert_eq!(
        WorkRequirementScopeV1::from_tag(2, Some(before_step.clone())).unwrap(),
        WorkRequirementScopeV1::BeforeStep(before_step)
    );
    assert_eq!(
        WorkRequirementScopeV1::from_tag(99, None).unwrap_err(),
        WorkRelationError::UnknownRequirementScopeTag(99)
    );
    assert_eq!(
        WorkRequirementScopeV1::from_tag(1, Some(ExactStepRevisionRefV1::new("step", 1).unwrap()))
            .unwrap_err(),
        WorkRelationError::InvalidScopePayload
    );
    assert!(ExactStepRevisionRefV1::new("", 1).is_err());
    assert!(ExactStepRevisionRefV1::new("step", 0).is_err());
}

#[test]
fn work_requirements_refuse_self_cross_repository_missing_root_and_cycles() {
    let repository = repository_id("repository");
    let foreign_repository = repository_id("foreign-repository");
    let a = snapshot(repository, "a", WorkLifecycleStateV1::Ready, 1);
    let b = snapshot(repository, "b", WorkLifecycleStateV1::Active, 2);
    let c = snapshot(repository, "c", WorkLifecycleStateV1::Completed, 3);

    assert_eq!(
        WorkRequirementV1::new(
            WorkRequirementIdV1::derive("self").unwrap(),
            repository,
            a.work_id(),
            repository,
            a.work_id(),
            root(1),
            WorkRequirementScopeV1::BeforeExecution,
        )
        .unwrap_err(),
        WorkRelationError::SelfEdge
    );
    assert_eq!(
        WorkRequirementV1::new(
            WorkRequirementIdV1::derive("foreign").unwrap(),
            repository,
            a.work_id(),
            foreign_repository,
            b.work_id(),
            root(2),
            WorkRequirementScopeV1::BeforeCompletion,
        )
        .unwrap_err(),
        WorkRelationError::CrossRepository
    );

    let mut graph =
        WorkRelationGraphV1::new(repository, vec![a.clone(), b.clone(), c.clone()]).unwrap();
    let a_requires_b = requirement(
        repository,
        &a,
        &b,
        "a-requires-b",
        WorkRequirementScopeV1::BeforeExecution,
    );
    assert_eq!(
        graph
            .admit_requirement(WorkRecordWriterV1::Contract, a_requires_b.clone())
            .unwrap(),
        WorkRelationAdmissionV1::Inserted
    );
    assert_eq!(
        graph
            .admit_requirement(WorkRecordWriterV1::Contract, a_requires_b.clone())
            .unwrap(),
        WorkRelationAdmissionV1::AlreadyPresent
    );
    assert_eq!(
        a_requires_b.canonical_bytes().unwrap(),
        a_requires_b.canonical_bytes().unwrap()
    );
    graph
        .admit_requirement(
            WorkRecordWriterV1::Contract,
            requirement(
                repository,
                &b,
                &c,
                "b-requires-c",
                WorkRequirementScopeV1::BeforeStep(
                    ExactStepRevisionRefV1::new("step-c", 2).unwrap(),
                ),
            ),
        )
        .unwrap();
    assert_eq!(
        graph
            .admit_requirement(
                WorkRecordWriterV1::Contract,
                requirement(
                    repository,
                    &c,
                    &a,
                    "c-requires-a",
                    WorkRequirementScopeV1::BeforeCompletion,
                ),
            )
            .unwrap_err(),
        WorkRelationError::Cycle
    );

    let wrong_root = WorkRequirementV1::new(
        WorkRequirementIdV1::derive("wrong-root").unwrap(),
        repository,
        a.work_id(),
        repository,
        c.work_id(),
        root(99),
        WorkRequirementScopeV1::BeforeCompletion,
    )
    .unwrap();
    assert_eq!(
        graph
            .admit_requirement(WorkRecordWriterV1::Contract, wrong_root)
            .unwrap_err(),
        WorkRelationError::UnknownTargetContractRoot
    );
}

#[test]
fn supersession_correction_and_continuation_enforce_cycles_and_cardinality() {
    let repository = repository_id("lineage-repository");
    let a = snapshot(repository, "lineage-a", WorkLifecycleStateV1::Active, 1);
    let b = snapshot(repository, "lineage-b", WorkLifecycleStateV1::Active, 2);
    let c = snapshot(repository, "lineage-c", WorkLifecycleStateV1::Active, 3);
    let mut supersession =
        WorkRelationGraphV1::new(repository, vec![a.clone(), b.clone(), c.clone()]).unwrap();
    supersession
        .admit_relation(
            WorkRecordWriterV1::Work,
            relation(WorkRelationKindV1::SupersededBy, &a, &b, "a-by-b"),
        )
        .unwrap();
    assert_eq!(
        supersession
            .admit_relation(
                WorkRecordWriterV1::Work,
                relation(WorkRelationKindV1::SupersededBy, &a, &c, "a-by-c"),
            )
            .unwrap_err(),
        WorkRelationError::SupersessionCardinality
    );
    supersession
        .admit_relation(
            WorkRecordWriterV1::Work,
            relation(WorkRelationKindV1::SupersededBy, &b, &c, "b-by-c"),
        )
        .unwrap();
    assert_eq!(
        supersession
            .admit_relation(
                WorkRecordWriterV1::Work,
                relation(WorkRelationKindV1::SupersededBy, &c, &a, "c-by-a"),
            )
            .unwrap_err(),
        WorkRelationError::Cycle
    );

    let terminal = snapshot(
        repository,
        "terminal-predecessor",
        WorkLifecycleStateV1::Completed,
        9,
    );
    let mut terminal_graph =
        WorkRelationGraphV1::new(repository, vec![terminal.clone(), a.clone()]).unwrap();
    assert_eq!(
        terminal_graph
            .admit_relation(
                WorkRecordWriterV1::Work,
                relation(
                    WorkRelationKindV1::SupersededBy,
                    &terminal,
                    &a,
                    "terminal-by-a",
                ),
            )
            .unwrap_err(),
        WorkRelationError::SupersessionSourceIneligible
    );

    let completed_a = snapshot(
        repository,
        "completed-a",
        WorkLifecycleStateV1::Completed,
        4,
    );
    let completed_b = snapshot(
        repository,
        "completed-b",
        WorkLifecycleStateV1::Completed,
        5,
    );
    let completed_c = snapshot(
        repository,
        "completed-c",
        WorkLifecycleStateV1::Completed,
        6,
    );
    let mut corrections = WorkRelationGraphV1::new(
        repository,
        vec![
            completed_a.clone(),
            completed_b.clone(),
            completed_c.clone(),
        ],
    )
    .unwrap();
    corrections
        .admit_relation(
            WorkRecordWriterV1::Work,
            relation(
                WorkRelationKindV1::Corrects,
                &completed_a,
                &completed_b,
                "correct-a-b",
            ),
        )
        .unwrap();
    corrections
        .admit_relation(
            WorkRecordWriterV1::Work,
            relation(
                WorkRelationKindV1::Corrects,
                &completed_b,
                &completed_c,
                "correct-b-c",
            ),
        )
        .unwrap();
    assert_eq!(
        corrections
            .admit_relation(
                WorkRecordWriterV1::Work,
                relation(
                    WorkRelationKindV1::Corrects,
                    &completed_c,
                    &completed_a,
                    "correct-c-a",
                ),
            )
            .unwrap_err(),
        WorkRelationError::Cycle
    );

    let cancelled = snapshot(repository, "cancelled", WorkLifecycleStateV1::Cancelled, 7);
    let active = snapshot(repository, "active", WorkLifecycleStateV1::Active, 8);
    let mut continuations = WorkRelationGraphV1::new(
        repository,
        vec![completed_a.clone(), cancelled.clone(), active.clone()],
    )
    .unwrap();
    continuations
        .admit_relation(
            WorkRecordWriterV1::Work,
            relation(
                WorkRelationKindV1::Continues,
                &active,
                &cancelled,
                "continue-cancelled",
            ),
        )
        .unwrap();
    assert_eq!(
        continuations
            .admit_relation(
                WorkRecordWriterV1::Work,
                relation(
                    WorkRelationKindV1::Continues,
                    &cancelled,
                    &active,
                    "continue-active",
                ),
            )
            .unwrap_err(),
        WorkRelationError::ContinuationTargetIneligible
    );
}

#[test]
fn references_allow_cycles_but_never_self_and_all_foreign_writers_refuse() {
    let repository = repository_id("reference-repository");
    let foreign_repository = repository_id("other-repository");
    let a = snapshot(repository, "reference-a", WorkLifecycleStateV1::Draft, 1);
    let b = snapshot(repository, "reference-b", WorkLifecycleStateV1::Draft, 2);
    let mut graph = WorkRelationGraphV1::new(repository, vec![a.clone(), b.clone()]).unwrap();
    let a_to_b = relation(WorkRelationKindV1::Reference, &a, &b, "a-ref-b");
    let b_to_a = relation(WorkRelationKindV1::Reference, &b, &a, "b-ref-a");
    graph
        .admit_relation(WorkRecordWriterV1::Work, a_to_b.clone())
        .unwrap();
    graph
        .admit_relation(WorkRecordWriterV1::Work, b_to_a)
        .unwrap();
    assert_eq!(graph.relations().len(), 2);
    assert_eq!(
        a_to_b.canonical_bytes().unwrap(),
        a_to_b.canonical_bytes().unwrap()
    );

    assert_eq!(
        WorkRelationRecordV1::new(
            WorkRelationIdV1::derive("self-reference").unwrap(),
            WorkRelationKindV1::Reference,
            endpoint(&a),
            endpoint(&a),
            "self",
            "test",
            "self-key",
        )
        .unwrap_err(),
        WorkRelationError::SelfEdge
    );
    let foreign = snapshot(
        foreign_repository,
        "foreign-work",
        WorkLifecycleStateV1::Draft,
        3,
    );
    assert_eq!(
        WorkRelationRecordV1::new(
            WorkRelationIdV1::derive("cross-reference").unwrap(),
            WorkRelationKindV1::Reference,
            endpoint(&a),
            endpoint(&foreign),
            "cross",
            "test",
            "cross-key",
        )
        .unwrap_err(),
        WorkRelationError::CrossRepository
    );

    for writer in WorkRecordWriterV1::ALL {
        if writer != WorkRecordWriterV1::Work {
            let mut relation_graph =
                WorkRelationGraphV1::new(repository, vec![a.clone(), b.clone()]).unwrap();
            assert_eq!(
                relation_graph
                    .admit_relation(writer, a_to_b.clone())
                    .unwrap_err(),
                WorkRelationError::ForeignRelationWriter(writer)
            );
        }
        if writer != WorkRecordWriterV1::Contract {
            let mut requirement_graph =
                WorkRelationGraphV1::new(repository, vec![a.clone(), b.clone()]).unwrap();
            let item = requirement(
                repository,
                &a,
                &b,
                &format!("writer-{writer:?}"),
                WorkRequirementScopeV1::BeforeExecution,
            );
            assert_eq!(
                requirement_graph
                    .admit_requirement(writer, item)
                    .unwrap_err(),
                WorkRelationError::ForeignRequirementWriter(writer)
            );
        }
    }
    assert_eq!(
        WorkRelationKindV1::from_tag(99).unwrap_err(),
        WorkRelationError::UnknownRelationKindTag(99)
    );
}
