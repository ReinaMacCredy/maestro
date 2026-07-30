use maestro::domain::contract::runtime::ContractGenerationIdV1;
use maestro::domain::identity::{ContractRootIdV1, StoreDomainIdV1};
use maestro::domain::step::{
    StepBindingV1, StepGraphEdgeV1, StepGraphError, StepGraphNodeV1, StepGraphSnapshotV1, StepIdV1,
    StepIdentityError, StepRevisionIdV1, StepScopeV1,
};
use maestro::domain::work::WorkIdV1;

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

fn binding(scope: StepScopeV1, root: u8, key: &str, revision: u8) -> StepBindingV1 {
    binding_in_generation(scope, root, root, key, revision)
}

fn binding_in_generation(
    scope: StepScopeV1,
    generation_byte: u8,
    root: u8,
    key: &str,
    revision: u8,
) -> StepBindingV1 {
    StepBindingV1::new(
        scope,
        generation(generation_byte),
        self::root(root),
        StepIdV1::new(scope, key).unwrap(),
        StepRevisionIdV1::from_bytes(hash(revision)).unwrap(),
    )
    .unwrap()
}

fn node(binding: StepBindingV1) -> StepGraphNodeV1 {
    StepGraphNodeV1::new(binding, true).unwrap()
}

#[test]
fn stable_ids_and_graph_bytes_are_deterministic() {
    let scope = scope(2);
    let a = binding(scope, 10, "build", 21);
    let b = binding(scope, 10, "verify", 22);
    assert_eq!(
        StepIdV1::new(scope, "build").unwrap(),
        StepIdV1::new(scope, "build").unwrap()
    );
    assert_ne!(
        StepIdV1::new(scope, "build").unwrap(),
        StepIdV1::new(scope, "verify").unwrap()
    );
    let rendered = a.step_id().render();
    assert_eq!(StepIdV1::parse(scope, &rendered).unwrap(), a.step_id());
    assert!(matches!(
        StepIdV1::parse(scope, &rendered.to_uppercase()),
        Err(StepIdentityError::InvalidRenderedIdentity)
    ));

    let first = StepGraphSnapshotV1::new(
        scope,
        generation(10),
        root(10),
        vec![node(b), node(a)],
        vec![StepGraphEdgeV1::new(a, b)],
    )
    .unwrap();
    let second = StepGraphSnapshotV1::new(
        scope,
        generation(10),
        root(10),
        vec![node(a), node(b)],
        vec![StepGraphEdgeV1::new(a, b)],
    )
    .unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert!(first.nodes().iter().any(|node| node.binding() == a));
    assert!(first.nodes().iter().any(|node| node.binding() == b));
    assert!(
        first
            .nodes()
            .windows(2)
            .all(|pair| pair[0].binding() < pair[1].binding())
    );
    assert_eq!(first.incoming_dependency_closure_hash(a).unwrap().len(), 32);
    assert_ne!(
        first.incoming_dependency_closure_hash(a).unwrap(),
        first.incoming_dependency_closure_hash(b).unwrap()
    );
}

#[test]
fn refuses_empty_optional_duplicate_and_over_bound_nodes() {
    let scope = scope(2);
    let a = binding(scope, 10, "a", 21);
    assert_eq!(
        StepGraphSnapshotV1::new(scope, generation(10), root(10), vec![], vec![]).unwrap_err(),
        StepGraphError::Empty
    );
    assert_eq!(
        StepGraphNodeV1::new(a, false).unwrap_err(),
        StepGraphError::OptionalNode
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            scope,
            generation(10),
            root(10),
            vec![node(a), node(a)],
            vec![],
        )
        .unwrap_err(),
        StepGraphError::DuplicateNode
    );
    let stale_a = binding(scope, 10, "a", 22);
    assert_eq!(
        StepGraphSnapshotV1::new(
            scope,
            generation(10),
            root(10),
            vec![node(a), node(stale_a)],
            vec![],
        )
        .unwrap_err(),
        StepGraphError::DuplicateStepId
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            scope,
            generation(10),
            root(10),
            vec![node(a); 4_097],
            vec![],
        )
        .unwrap_err(),
        StepGraphError::TooManyNodes
    );
}

#[test]
fn refuses_dangling_stale_cross_work_and_duplicate_edges() {
    let graph_scope = scope(2);
    let other_scope = scope(3);
    let a = binding(graph_scope, 10, "a", 21);
    let b = binding(graph_scope, 10, "b", 22);
    let unknown = binding(graph_scope, 10, "unknown", 23);
    let stale_a = binding(graph_scope, 10, "a", 24);
    let foreign = binding(other_scope, 10, "foreign", 25);
    let foreign_generation = binding_in_generation(graph_scope, 11, 10, "foreign-gen", 26);

    assert_eq!(
        StepGraphSnapshotV1::new(
            graph_scope,
            generation(10),
            root(10),
            vec![node(a), node(b)],
            vec![StepGraphEdgeV1::new(a, unknown)],
        )
        .unwrap_err(),
        StepGraphError::DanglingEndpoint
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            graph_scope,
            generation(10),
            root(10),
            vec![node(a), node(b)],
            vec![StepGraphEdgeV1::new(stale_a, b)],
        )
        .unwrap_err(),
        StepGraphError::StaleRevisionEndpoint
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            graph_scope,
            generation(10),
            root(10),
            vec![node(a), node(foreign)],
            vec![],
        )
        .unwrap_err(),
        StepGraphError::CrossWorkNode
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            graph_scope,
            generation(10),
            root(10),
            vec![node(a), node(foreign_generation)],
            vec![],
        )
        .unwrap_err(),
        StepGraphError::CrossContractGenerationNode
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            graph_scope,
            generation(10),
            root(10),
            vec![node(a), node(b)],
            vec![StepGraphEdgeV1::new(a, foreign)],
        )
        .unwrap_err(),
        StepGraphError::CrossWorkEdge
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            graph_scope,
            generation(10),
            root(10),
            vec![node(a), node(b)],
            vec![StepGraphEdgeV1::new(a, b), StepGraphEdgeV1::new(a, b)],
        )
        .unwrap_err(),
        StepGraphError::DuplicateEdge
    );
}

#[test]
fn refuses_self_edges_and_cycles_of_any_input_order() {
    let scope = scope(2);
    let a = binding(scope, 10, "a", 21);
    let b = binding(scope, 10, "b", 22);
    let c = binding(scope, 10, "c", 23);
    assert_eq!(
        StepGraphSnapshotV1::new(
            scope,
            generation(10),
            root(10),
            vec![node(a)],
            vec![StepGraphEdgeV1::new(a, a)],
        )
        .unwrap_err(),
        StepGraphError::SelfEdge
    );
    assert_eq!(
        StepGraphSnapshotV1::new(
            scope,
            generation(10),
            root(10),
            vec![node(c), node(a), node(b)],
            vec![
                StepGraphEdgeV1::new(b, c),
                StepGraphEdgeV1::new(c, a),
                StepGraphEdgeV1::new(a, b),
            ],
        )
        .unwrap_err(),
        StepGraphError::Cycle
    );
}
