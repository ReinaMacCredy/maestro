use maestro::domain::vnext::contract::runtime::ContractGenerationIdV1;
use maestro::domain::vnext::identity::{ContractRootIdV1, StoreDomainIdV1};
use maestro::domain::vnext::step::{
    NamedMaterialConstraintV1, StepAmendmentError, StepBindingV1, StepGraphNodeV1,
    StepGraphSnapshotV1, StepIdV1, StepRevisionIdV1, StepRevisionMaterialV1, StepRevisionV1,
    StepScopeV1, plan_step_amendment_v1,
};
use maestro::domain::vnext::work::WorkIdV1;

fn hash(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn root(byte: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
}

fn generation(byte: u8) -> ContractGenerationIdV1 {
    ContractGenerationIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
}

fn repository(byte: u8) -> StoreDomainIdV1 {
    StoreDomainIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
}

fn work(byte: u8) -> WorkIdV1 {
    WorkIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
}

fn scope(work: u8) -> StepScopeV1 {
    StepScopeV1::new(repository(1), self::work(work))
}

fn revision(changed_slot: Option<usize>) -> StepRevisionV1 {
    let mut fields = [
        hash(11),
        hash(12),
        hash(13),
        hash(14),
        hash(15),
        hash(16),
        hash(17),
        hash(18),
        hash(19),
        hash(20),
        hash(21),
        hash(22),
        hash(23),
        hash(24),
    ];
    if let Some(slot) = changed_slot {
        fields[slot] = hash(200);
    }
    StepRevisionV1::new(
        StepRevisionMaterialV1::new(
            fields[0],
            fields[1],
            fields[2],
            fields[3],
            fields[4],
            fields[5],
            fields[6],
            fields[7],
            fields[8],
            fields[9],
            fields[10],
            fields[11],
            fields[12],
            fields[13],
            vec![],
        )
        .unwrap(),
    )
    .unwrap()
}

fn binding(
    scope: StepScopeV1,
    root_byte: u8,
    key: &str,
    revision: StepRevisionIdV1,
) -> StepBindingV1 {
    StepBindingV1::new(
        scope,
        generation(root_byte),
        root(root_byte),
        StepIdV1::new(scope, key).unwrap(),
        revision,
    )
    .unwrap()
}

fn graph(scope: StepScopeV1, root_byte: u8, bindings: Vec<StepBindingV1>) -> StepGraphSnapshotV1 {
    StepGraphSnapshotV1::new(
        scope,
        generation(root_byte),
        root(root_byte),
        bindings
            .into_iter()
            .map(|binding| StepGraphNodeV1::new(binding, true).unwrap())
            .collect(),
        vec![],
    )
    .unwrap()
}

#[test]
fn every_material_execution_or_completion_constraint_rotates_revision_identity() {
    let baseline = revision(None);
    for slot in 0..14 {
        let changed = revision(Some(slot));
        assert_ne!(baseline.id(), changed.id(), "material slot {slot}");
        assert_ne!(
            baseline.canonical_bytes().unwrap(),
            changed.canonical_bytes().unwrap(),
            "material bytes slot {slot}"
        );
    }

    let first = NamedMaterialConstraintV1::new("provider-operation", hash(31)).unwrap();
    let second = NamedMaterialConstraintV1::new("toolchain", hash(32)).unwrap();
    let fields: Vec<_> = (41..=54).map(hash).collect();
    let ordered = StepRevisionV1::new(
        StepRevisionMaterialV1::new(
            fields[0],
            fields[1],
            fields[2],
            fields[3],
            fields[4],
            fields[5],
            fields[6],
            fields[7],
            fields[8],
            fields[9],
            fields[10],
            fields[11],
            fields[12],
            fields[13],
            vec![first.clone(), second.clone()],
        )
        .unwrap(),
    )
    .unwrap();
    let reversed = StepRevisionV1::new(
        StepRevisionMaterialV1::new(
            fields[0],
            fields[1],
            fields[2],
            fields[3],
            fields[4],
            fields[5],
            fields[6],
            fields[7],
            fields[8],
            fields[9],
            fields[10],
            fields[11],
            fields[12],
            fields[13],
            vec![second, first],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(ordered.id(), reversed.id());
    assert_eq!(
        ordered.canonical_bytes().unwrap(),
        reversed.canonical_bytes().unwrap()
    );
}

#[test]
fn partitions_retain_replace_remove_and_add_exactly_once() {
    let scope = scope(2);
    let base = revision(None).id();
    let changed_effect = revision(Some(5)).id();
    let old_a = binding(scope, 10, "a", base);
    let old_b = binding(scope, 10, "b", base);
    let old_c = binding(scope, 10, "c", base);
    let new_a = binding(scope, 11, "a", base);
    let new_b = binding(scope, 11, "b", changed_effect);
    let new_d = binding(scope, 11, "d", base);
    let current = graph(scope, 10, vec![old_c, old_a, old_b]);
    let candidate = graph(scope, 11, vec![new_d, new_b, new_a]);

    let plan = plan_step_amendment_v1(&current, &candidate).unwrap();
    assert_eq!(plan.retain_exact().len(), 1);
    assert_eq!(plan.retain_exact()[0].old(), old_a);
    assert_eq!(plan.retain_exact()[0].new_binding(), new_a);
    assert_eq!(plan.replace().len(), 1);
    assert_eq!(plan.replace()[0].old(), old_b);
    assert_eq!(plan.replace()[0].replacement(), new_b);
    assert_eq!(plan.remove(), &[old_c]);
    assert_eq!(plan.add(), &[new_d]);

    let again = plan_step_amendment_v1(&current, &candidate).unwrap();
    assert_eq!(
        plan.canonical_bytes().unwrap(),
        again.canonical_bytes().unwrap()
    );
}

#[test]
fn split_and_merge_are_only_remove_and_add() {
    let scope = scope(2);
    let revision = revision(None).id();
    let old = binding(scope, 10, "compound", revision);
    let left = binding(scope, 11, "left", revision);
    let right = binding(scope, 11, "right", revision);
    let split = plan_step_amendment_v1(
        &graph(scope, 10, vec![old]),
        &graph(scope, 11, vec![right, left]),
    )
    .unwrap();
    assert!(split.retain_exact().is_empty());
    assert!(split.replace().is_empty());
    assert_eq!(split.remove(), &[old]);
    let mut split_additions = vec![left, right];
    split_additions.sort();
    assert_eq!(split.add(), split_additions);

    let merged = binding(scope, 12, "merged", revision);
    let merge = plan_step_amendment_v1(
        &graph(scope, 11, vec![left, right]),
        &graph(scope, 12, vec![merged]),
    )
    .unwrap();
    assert!(merge.replace().is_empty());
    let mut merge_removals = vec![left, right];
    merge_removals.sort();
    assert_eq!(merge.remove(), merge_removals);
    assert_eq!(merge.add(), &[merged]);
}

#[test]
fn refuses_same_root_and_cross_work_amendments() {
    let revision = revision(None).id();
    let current_scope = scope(2);
    let other_scope = scope(3);
    let current = graph(
        current_scope,
        10,
        vec![binding(current_scope, 10, "a", revision)],
    );
    let same_root = graph(
        current_scope,
        10,
        vec![binding(current_scope, 10, "a", revision)],
    );
    assert_eq!(
        plan_step_amendment_v1(&current, &same_root).unwrap_err(),
        StepAmendmentError::ContractRootNotAdvanced
    );
    let foreign = graph(
        other_scope,
        11,
        vec![binding(other_scope, 11, "a", revision)],
    );
    assert_eq!(
        plan_step_amendment_v1(&current, &foreign).unwrap_err(),
        StepAmendmentError::CrossWork
    );
}
