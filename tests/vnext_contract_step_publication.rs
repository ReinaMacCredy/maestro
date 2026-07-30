use maestro::domain::contract::runtime::{
    ContractGenerationIdV1, ContractRuntimeError, InitialContractStepPublicationV1,
};
use maestro::domain::identity::{ContractRootIdV1, StoreDomainIdV1};
use maestro::domain::step::{
    StepBindingV1, StepGraphNodeV1, StepGraphSnapshotV1, StepIdV1, StepLifecycleV1,
    StepOpenBasisV1, StepRevisionIdV1, StepScopeV1,
};
use maestro::domain::work::WorkIdV1;

fn identity(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

fn root(byte: u8) -> ContractRootIdV1 {
    ContractRootIdV1::parse(&identity(byte)).unwrap()
}

fn generation(byte: u8) -> ContractGenerationIdV1 {
    ContractGenerationIdV1::parse(&identity(byte)).unwrap()
}

fn repository(byte: u8) -> StoreDomainIdV1 {
    StoreDomainIdV1::parse(&identity(byte)).unwrap()
}

fn work(byte: u8) -> WorkIdV1 {
    WorkIdV1::parse(&identity(byte)).unwrap()
}

fn binding(scope: StepScopeV1, key: &str, revision: u8) -> StepBindingV1 {
    StepBindingV1::new(
        scope,
        generation(10),
        root(20),
        StepIdV1::new(scope, key).unwrap(),
        StepRevisionIdV1::from_bytes([revision; 32]).unwrap(),
    )
    .unwrap()
}

fn graph() -> StepGraphSnapshotV1 {
    let scope = StepScopeV1::new(repository(1), work(2));
    let first = binding(scope, "first", 30);
    let second = binding(scope, "second", 31);
    StepGraphSnapshotV1::new(
        scope,
        generation(10),
        root(20),
        vec![
            StepGraphNodeV1::new(second, true).unwrap(),
            StepGraphNodeV1::new(first, true).unwrap(),
        ],
        vec![],
    )
    .unwrap()
}

#[test]
fn initial_publication_initializes_every_exact_graph_binding_open_fresh() {
    let graph = graph();
    let publication =
        InitialContractStepPublicationV1::new(work(2), generation(10), root(20), graph.clone())
            .unwrap();

    assert_eq!(publication.graph().id(), graph.id());
    assert_eq!(publication.step_states().len(), graph.nodes().len());
    for (state, node) in publication.step_states().iter().zip(graph.nodes()) {
        assert_eq!(state.binding(), node.binding());
        assert_eq!(
            state.lifecycle(),
            StepLifecycleV1::Open {
                basis: StepOpenBasisV1::Fresh
            }
        );
    }

    for (wrong_work, wrong_generation, wrong_root) in [
        (work(3), generation(10), root(20)),
        (work(2), generation(11), root(20)),
        (work(2), generation(10), root(21)),
    ] {
        assert_eq!(
            InitialContractStepPublicationV1::new(
                wrong_work,
                wrong_generation,
                wrong_root,
                graph.clone(),
            )
            .unwrap_err(),
            ContractRuntimeError::CandidateStepGraphMismatch
        );
    }
}
