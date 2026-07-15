use maestro::domain::vnext::contract::runtime::ContractGenerationIdV1;
use maestro::domain::vnext::identity::{ContractRootIdV1, StoreDomainIdV1};
use maestro::domain::vnext::step::{
    RetainExactInitializationV1, StepAmendmentError, StepBindingV1, StepGraphNodeV1,
    StepGraphSnapshotV1, StepIdV1, StepLifecycleV1, StepRevisionIdV1, StepScopeV1, StepStateV1,
    plan_step_amendment_v1,
};
use maestro::domain::vnext::work::WorkIdV1;

fn hash(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn identity(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn root(byte: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&identity(byte)).unwrap()
}

fn generation(byte: u8) -> ContractGenerationIdV1 {
    ContractGenerationIdV1::parse(&identity(byte)).unwrap()
}

fn scope() -> StepScopeV1 {
    StepScopeV1::new(
        StoreDomainIdV1::parse(&identity(1)).unwrap(),
        WorkIdV1::parse(&identity(2)).unwrap(),
    )
}

fn binding(root_byte: u8, key: &str, revision: u8) -> StepBindingV1 {
    let scope = scope();
    StepBindingV1::new(
        scope,
        generation(root_byte),
        root(root_byte),
        StepIdV1::new(scope, key).unwrap(),
        StepRevisionIdV1::from_bytes(hash(revision)).unwrap(),
    )
    .unwrap()
}

fn graph(root_byte: u8, bindings: &[StepBindingV1]) -> StepGraphSnapshotV1 {
    StepGraphSnapshotV1::new(
        scope(),
        generation(root_byte),
        root(root_byte),
        bindings
            .iter()
            .copied()
            .map(|binding| StepGraphNodeV1::new(binding, true).unwrap())
            .collect(),
        vec![],
    )
    .unwrap()
}

#[test]
fn total_plan_produces_deterministic_dispositions_and_conserves_obligations() {
    let old_exact = binding(10, "exact", 20);
    let old_replaced = binding(10, "replaced", 21);
    let old_removed = binding(10, "removed", 22);
    let new_exact = binding(11, "exact", 20);
    let new_replacement = binding(11, "replaced", 23);
    let new_added = binding(11, "added", 24);
    let current = graph(10, &[old_removed, old_exact, old_replaced]);
    let candidate = graph(11, &[new_added, new_replacement, new_exact]);
    let plan = plan_step_amendment_v1(&current, &candidate).unwrap();

    let exact_state = StepStateV1::new_open(old_exact);
    let replaced_state = StepStateV1::new_open(old_replaced);
    let removed_state = StepStateV1::new_open(old_removed);
    let applied = plan
        .apply(
            &current,
            &candidate,
            &[removed_state, exact_state, replaced_state],
            hash(70),
        )
        .unwrap();
    let repeated = plan
        .apply(
            &current,
            &candidate,
            &[replaced_state, removed_state, exact_state],
            hash(70),
        )
        .unwrap();
    assert_eq!(applied, repeated);

    assert_eq!(applied.retain_exact().len(), 1);
    assert_eq!(applied.retain_exact()[0].prior_state(), exact_state);
    assert_eq!(applied.retain_exact()[0].historical_state(), exact_state);
    assert_eq!(applied.retain_exact()[0].next_state().binding(), new_exact);
    assert_eq!(
        applied.retain_exact()[0].next_state().initialization(),
        RetainExactInitializationV1::OpenFreshV1
    );
    let carried_state = applied.retain_exact()[0]
        .next_state()
        .materialize()
        .unwrap();
    assert_eq!(carried_state.binding(), new_exact);
    assert!(matches!(
        carried_state.lifecycle(),
        StepLifecycleV1::Open { .. }
    ));

    assert_eq!(applied.replace().len(), 1);
    assert_eq!(applied.replace()[0].prior_state(), replaced_state);
    assert!(matches!(
        applied.replace()[0].historical_state().lifecycle(),
        StepLifecycleV1::Superseded { successor, amendment_receipt_hash }
            if successor == new_replacement && amendment_receipt_hash == hash(70)
    ));
    assert_eq!(
        applied.replace()[0].next_state().initialization(),
        RetainExactInitializationV1::OpenFreshV1
    );
    assert!(matches!(
        applied.replace()[0]
            .next_state()
            .materialize()
            .unwrap()
            .lifecycle(),
        StepLifecycleV1::Open { .. }
    ));

    assert_eq!(applied.remove().len(), 1);
    assert_eq!(applied.remove()[0].prior_state(), removed_state);
    assert!(matches!(
        applied.remove()[0].historical_state().lifecycle(),
        StepLifecycleV1::Cancelled { amendment_receipt_hash } if amendment_receipt_hash == hash(70)
    ));

    assert_eq!(applied.add().len(), 1);
    assert_eq!(applied.add()[0].next_state().binding(), new_added);
    assert_eq!(
        applied.add()[0].next_state().initialization(),
        RetainExactInitializationV1::OpenFreshV1
    );

    let conservation = applied.obligation_conservation();
    assert_eq!(conservation.current_obligation_count(), 3);
    assert_eq!(conservation.candidate_obligation_count(), 3);
    assert_eq!(conservation.retain_exact_count(), 1);
    assert_eq!(conservation.replace_count(), 1);
    assert_eq!(conservation.remove_count(), 1);
    assert_eq!(conservation.add_count(), 1);
    assert_eq!(conservation.current_partition_count(), 3);
    assert_eq!(conservation.candidate_partition_count(), 3);
}

#[test]
fn application_refuses_a_stale_plan_and_incomplete_states() {
    let old = binding(10, "exact", 20);
    let new = binding(11, "exact", 20);
    let later = binding(12, "exact", 20);
    let current = graph(10, &[old]);
    let candidate = graph(11, &[new]);
    let later_candidate = graph(12, &[later]);
    let plan = plan_step_amendment_v1(&current, &candidate).unwrap();
    let state = StepStateV1::new_open(old);

    assert_eq!(
        plan.apply(&current, &later_candidate, &[state], hash(70))
            .unwrap_err(),
        StepAmendmentError::PlanGraphMismatch
    );
    assert_eq!(
        plan.apply(&current, &candidate, &[], hash(70)).unwrap_err(),
        StepAmendmentError::CurrentStateSetMismatch
    );
}
