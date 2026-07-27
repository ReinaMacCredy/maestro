use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::domain::contract::runtime::ContractGenerationIdV1;
use crate::domain::identity::ContractRootIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::{
    StepGraphSnapshotIdV1, StepIdV1, StepIdentityError, StepRevisionIdV1, StepScopeV1, domain_hash,
};

const STEP_GRAPH_SNAPSHOT_DOMAIN_V1: &str = "maestro.vnext.step-graph-snapshot.v1";
const STEP_INCOMING_CLOSURE_DOMAIN_V1: &str = "maestro.vnext.step-incoming-closure.v1";
const STEP_GRAPH_SNAPSHOT_VERSION_V1: u64 = 1;
pub const MAX_STEP_GRAPH_NODES_V1: usize = 4_096;
pub const MAX_STEP_GRAPH_EDGES_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StepBindingV1 {
    scope: StepScopeV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    step_id: StepIdV1,
    revision_id: StepRevisionIdV1,
}

impl StepBindingV1 {
    pub fn new(
        scope: StepScopeV1,
        contract_generation_id: ContractGenerationIdV1,
        contract_root_id: ContractRootIdV1,
        step_id: StepIdV1,
        revision_id: StepRevisionIdV1,
    ) -> Result<Self, StepBindingError> {
        if step_id.scope() != scope {
            return Err(StepBindingError::StepIdScopeMismatch);
        }
        if *contract_root_id.as_bytes() == [0; 32] {
            return Err(StepBindingError::MissingContractRoot);
        }
        Ok(Self {
            scope,
            contract_generation_id,
            contract_root_id,
            step_id,
            revision_id,
        })
    }

    pub fn scope(&self) -> StepScopeV1 {
        self.scope
    }

    pub fn contract_root_id(&self) -> ContractRootIdV1 {
        self.contract_root_id
    }

    pub fn contract_generation_id(&self) -> ContractGenerationIdV1 {
        self.contract_generation_id
    }

    pub fn step_id(&self) -> StepIdV1 {
        self.step_id
    }

    pub fn revision_id(&self) -> StepRevisionIdV1 {
        self.revision_id
    }

    pub(super) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.scope.canonical_value(),
            CborValue::Bytes(self.contract_generation_id.as_bytes().to_vec()),
            CborValue::Bytes(self.contract_root_id.as_bytes().to_vec()),
            self.step_id.canonical_value(),
            CborValue::Bytes(self.revision_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepBindingError {
    #[error("Step id belongs to a different repository or Work")]
    StepIdScopeMismatch,
    #[error("Step Binding Contract Root must not be the all-zero missing reference")]
    MissingContractRoot,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StepGraphNodeV1 {
    binding: StepBindingV1,
}

impl StepGraphNodeV1 {
    pub fn new(binding: StepBindingV1, required: bool) -> Result<Self, StepGraphError> {
        if !required {
            return Err(StepGraphError::OptionalNode);
        }
        Ok(Self { binding })
    }

    pub fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub fn required(&self) -> bool {
        true
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![self.binding.canonical_value(), CborValue::Bool(true)])
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StepGraphEdgeV1 {
    predecessor: StepBindingV1,
    dependent: StepBindingV1,
}

impl StepGraphEdgeV1 {
    pub fn new(predecessor: StepBindingV1, dependent: StepBindingV1) -> Self {
        Self {
            predecessor,
            dependent,
        }
    }

    pub fn predecessor(&self) -> StepBindingV1 {
        self.predecessor
    }

    pub fn dependent(&self) -> StepBindingV1 {
        self.dependent
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.predecessor.canonical_value(),
            self.dependent.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepGraphSnapshotV1 {
    id: StepGraphSnapshotIdV1,
    scope: StepScopeV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    nodes: Vec<StepGraphNodeV1>,
    edges: Vec<StepGraphEdgeV1>,
}

impl StepGraphSnapshotV1 {
    pub fn new(
        scope: StepScopeV1,
        contract_generation_id: ContractGenerationIdV1,
        contract_root_id: ContractRootIdV1,
        mut nodes: Vec<StepGraphNodeV1>,
        mut edges: Vec<StepGraphEdgeV1>,
    ) -> Result<Self, StepGraphError> {
        if nodes.is_empty() {
            return Err(StepGraphError::Empty);
        }
        if nodes.len() > MAX_STEP_GRAPH_NODES_V1 {
            return Err(StepGraphError::TooManyNodes);
        }
        if edges.len() > MAX_STEP_GRAPH_EDGES_V1 {
            return Err(StepGraphError::TooManyEdges);
        }
        if *contract_root_id.as_bytes() == [0; 32] {
            return Err(StepGraphError::MissingContractRoot);
        }

        validate_nodes(scope, contract_generation_id, contract_root_id, &nodes)?;
        validate_edges(
            scope,
            contract_generation_id,
            contract_root_id,
            &nodes,
            &edges,
        )?;
        validate_acyclic(&nodes, &edges)?;
        nodes.sort();
        edges.sort();

        let value = canonical_graph_value(
            scope,
            contract_generation_id,
            contract_root_id,
            &nodes,
            &edges,
        );
        let id =
            StepGraphSnapshotIdV1::from_bytes(domain_hash(STEP_GRAPH_SNAPSHOT_DOMAIN_V1, &value)?)?;
        Ok(Self {
            id,
            scope,
            contract_generation_id,
            contract_root_id,
            nodes,
            edges,
        })
    }

    pub fn id(&self) -> StepGraphSnapshotIdV1 {
        self.id
    }

    pub fn scope(&self) -> StepScopeV1 {
        self.scope
    }

    pub fn contract_root_id(&self) -> ContractRootIdV1 {
        self.contract_root_id
    }

    pub fn contract_generation_id(&self) -> ContractGenerationIdV1 {
        self.contract_generation_id
    }

    pub fn nodes(&self) -> &[StepGraphNodeV1] {
        &self.nodes
    }

    pub fn edges(&self) -> &[StepGraphEdgeV1] {
        &self.edges
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StepGraphError> {
        Ok(deterministic_cbor::encode(&canonical_graph_value(
            self.scope,
            self.contract_generation_id,
            self.contract_root_id,
            &self.nodes,
            &self.edges,
        ))?)
    }

    pub fn incoming_dependency_closure_hash(
        &self,
        binding: StepBindingV1,
    ) -> Result<[u8; 32], StepGraphError> {
        if !self.nodes.iter().any(|node| node.binding == binding) {
            return Err(classify_missing_endpoint(&self.nodes, binding));
        }
        let mut closure = BTreeSet::new();
        let mut frontier = VecDeque::from([binding]);
        while let Some(dependent) = frontier.pop_front() {
            for edge in self.edges.iter().filter(|edge| edge.dependent == dependent) {
                if closure.insert(edge.predecessor) {
                    frontier.push_back(edge.predecessor);
                }
            }
        }
        let value = CborValue::Array(closure.iter().map(StepBindingV1::canonical_value).collect());
        Ok(domain_hash(STEP_INCOMING_CLOSURE_DOMAIN_V1, &value)?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepGraphError {
    #[error(transparent)]
    Identity(#[from] StepIdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Step Graph Snapshot must contain at least one required node")]
    Empty,
    #[error("Step Graph Snapshot exceeds the finite v1 node bound of {MAX_STEP_GRAPH_NODES_V1}")]
    TooManyNodes,
    #[error("Step Graph Snapshot exceeds the finite v1 edge bound of {MAX_STEP_GRAPH_EDGES_V1}")]
    TooManyEdges,
    #[error("Step Graph Snapshot cannot encode an optional node")]
    OptionalNode,
    #[error("Step Graph Snapshot Contract Root must not be the all-zero missing reference")]
    MissingContractRoot,
    #[error("Step Graph Snapshot contains a node from a different repository or Work")]
    CrossWorkNode,
    #[error("Step Graph Snapshot contains a node from a different Contract Root")]
    CrossContractRootNode,
    #[error("Step Graph Snapshot contains a node from a different Contract Generation")]
    CrossContractGenerationNode,
    #[error("Step Graph Snapshot contains a duplicate exact node")]
    DuplicateNode,
    #[error("Step Graph Snapshot contains more than one revision for a stable Step id")]
    DuplicateStepId,
    #[error("Step Graph Snapshot contains an edge from a different repository or Work")]
    CrossWorkEdge,
    #[error("Step Graph Snapshot contains an edge from a different Contract Root")]
    CrossContractRootEdge,
    #[error("Step Graph Snapshot contains an edge from a different Contract Generation")]
    CrossContractGenerationEdge,
    #[error("Step Graph Snapshot contains a self edge")]
    SelfEdge,
    #[error("Step Graph Snapshot contains a duplicate edge")]
    DuplicateEdge,
    #[error("Step Graph Snapshot edge has a dangling endpoint")]
    DanglingEndpoint,
    #[error("Step Graph Snapshot edge names a stale revision endpoint")]
    StaleRevisionEndpoint,
    #[error("Step Graph Snapshot must be acyclic")]
    Cycle,
}

fn validate_nodes(
    scope: StepScopeV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    nodes: &[StepGraphNodeV1],
) -> Result<(), StepGraphError> {
    let mut exact = BTreeSet::new();
    let mut stable_ids = BTreeSet::new();
    for node in nodes {
        if node.binding.scope != scope {
            return Err(StepGraphError::CrossWorkNode);
        }
        if node.binding.contract_root_id != contract_root_id {
            return Err(StepGraphError::CrossContractRootNode);
        }
        if node.binding.contract_generation_id != contract_generation_id {
            return Err(StepGraphError::CrossContractGenerationNode);
        }
        if !exact.insert(node.binding) {
            return Err(StepGraphError::DuplicateNode);
        }
        if !stable_ids.insert(node.binding.step_id) {
            return Err(StepGraphError::DuplicateStepId);
        }
    }
    Ok(())
}

fn validate_edges(
    scope: StepScopeV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    nodes: &[StepGraphNodeV1],
    edges: &[StepGraphEdgeV1],
) -> Result<(), StepGraphError> {
    let exact_nodes: BTreeSet<_> = nodes.iter().map(|node| node.binding).collect();
    let mut exact_edges = BTreeSet::new();
    for edge in edges {
        for endpoint in [edge.predecessor, edge.dependent] {
            if endpoint.scope != scope {
                return Err(StepGraphError::CrossWorkEdge);
            }
            if endpoint.contract_root_id != contract_root_id {
                return Err(StepGraphError::CrossContractRootEdge);
            }
            if endpoint.contract_generation_id != contract_generation_id {
                return Err(StepGraphError::CrossContractGenerationEdge);
            }
            if !exact_nodes.contains(&endpoint) {
                return Err(classify_missing_endpoint(nodes, endpoint));
            }
        }
        if edge.predecessor == edge.dependent {
            return Err(StepGraphError::SelfEdge);
        }
        if !exact_edges.insert(*edge) {
            return Err(StepGraphError::DuplicateEdge);
        }
    }
    Ok(())
}

fn classify_missing_endpoint(nodes: &[StepGraphNodeV1], endpoint: StepBindingV1) -> StepGraphError {
    if nodes
        .iter()
        .any(|node| node.binding.step_id == endpoint.step_id)
    {
        StepGraphError::StaleRevisionEndpoint
    } else {
        StepGraphError::DanglingEndpoint
    }
}

fn validate_acyclic(
    nodes: &[StepGraphNodeV1],
    edges: &[StepGraphEdgeV1],
) -> Result<(), StepGraphError> {
    let mut positions = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        positions.insert(node.binding, index);
    }
    let mut indegrees = vec![0_usize; nodes.len()];
    let mut successors = vec![Vec::new(); nodes.len()];
    for edge in edges {
        let predecessor = positions[&edge.predecessor];
        let dependent = positions[&edge.dependent];
        indegrees[dependent] += 1;
        successors[predecessor].push(dependent);
    }
    let mut ready: VecDeque<_> = indegrees
        .iter()
        .enumerate()
        .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
        .collect();
    let mut visited = 0;
    while let Some(index) = ready.pop_front() {
        visited += 1;
        for successor in &successors[index] {
            indegrees[*successor] -= 1;
            if indegrees[*successor] == 0 {
                ready.push_back(*successor);
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        Err(StepGraphError::Cycle)
    }
}

fn canonical_graph_value(
    scope: StepScopeV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    nodes: &[StepGraphNodeV1],
    edges: &[StepGraphEdgeV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(STEP_GRAPH_SNAPSHOT_VERSION_V1),
        scope.canonical_value(),
        CborValue::Bytes(contract_generation_id.as_bytes().to_vec()),
        CborValue::Bytes(contract_root_id.as_bytes().to_vec()),
        CborValue::Array(nodes.iter().map(StepGraphNodeV1::canonical_value).collect()),
        CborValue::Array(edges.iter().map(StepGraphEdgeV1::canonical_value).collect()),
    ])
}
