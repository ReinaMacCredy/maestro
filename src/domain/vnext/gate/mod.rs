//! Pure Gate definitions, snapshots, and evaluator contracts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::contract::runtime::ContractGenerationIdV1;
use crate::domain::vnext::identity::{ContractComponentIdV1, ContractRootIdV1};
use crate::domain::vnext::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const MAX_GATE_NODES_V1: usize = 4_096;
const MAX_GATE_CHILDREN_V1: usize = 1_024;

macro_rules! gate_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, GateError> {
                require_nonzero(bytes, stringify!($name))?;
                Ok(Self(bytes))
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub fn render(&self) -> String {
                let mut rendered = String::with_capacity(71);
                rendered.push_str("sha256:");
                for byte in self.0 {
                    use std::fmt::Write;
                    write!(&mut rendered, "{byte:02x}")
                        .expect("invariant: writing hexadecimal into String cannot fail");
                }
                rendered
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.render())
                    .finish()
            }
        }
    };
}

gate_identity!(GateNodeIdV1);
gate_identity!(GateSnapshotIdV1);
gate_identity!(GateEvaluatorContractIdV1);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GateInputClassV1 {
    Evidence,
    Authority,
    Mixed,
    Composite,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GateScopeV1 {
    Work,
    Step,
}

impl GateScopeV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Work => 1,
            Self::Step => 2,
        }
    }
}

impl GateInputClassV1 {
    pub const fn tag(self) -> u64 {
        match self {
            Self::Evidence => 1,
            Self::Authority => 2,
            Self::Mixed => 3,
            Self::Composite => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateOperatorV1 {
    Leaf,
    All,
    Any,
    Quorum { required: u32 },
    Veto,
    DenyOverrides,
}

impl GateOperatorV1 {
    fn canonical_value(self) -> CborValue {
        match self {
            Self::Leaf => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::All => CborValue::Array(vec![CborValue::Unsigned(2)]),
            Self::Any => CborValue::Array(vec![CborValue::Unsigned(3)]),
            Self::Quorum { required } => CborValue::Array(vec![
                CborValue::Unsigned(4),
                CborValue::Unsigned(u64::from(required)),
            ]),
            Self::Veto => CborValue::Array(vec![CborValue::Unsigned(5)]),
            Self::DenyOverrides => CborValue::Array(vec![CborValue::Unsigned(6)]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GateEvaluationResultV1 {
    Pass,
    Fail,
    Indeterminate,
    Error,
}

impl GateEvaluationResultV1 {
    pub const ALL: [Self; 4] = [Self::Pass, Self::Fail, Self::Indeterminate, Self::Error];

    pub const fn tag(self) -> u64 {
        match self {
            Self::Pass => 1,
            Self::Fail => 2,
            Self::Indeterminate => 3,
            Self::Error => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateEvaluatorContractV1 {
    id: GateEvaluatorContractIdV1,
    version: u64,
    algorithm_hash: [u8; 32],
    implementation_hash: [u8; 32],
    canonicalization_hash: [u8; 32],
    redaction_hash: [u8; 32],
    trust_root_snapshot_hash: [u8; 32],
    definition: GateEvaluatorDefinitionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateLeafRuleV1 {
    EvidenceSetPresent,
    AuthoritySetPresent,
    MixedSetPresent,
    ExactInputSet,
    EvidenceSemanticMatch,
    AuthoritySemanticMatch,
    MixedSemanticMatch,
}

impl GateLeafRuleV1 {
    pub const fn tag(self) -> u64 {
        match self {
            Self::EvidenceSetPresent => 1,
            Self::AuthoritySetPresent => 2,
            Self::MixedSetPresent => 3,
            Self::ExactInputSet => 4,
            Self::EvidenceSemanticMatch => 5,
            Self::AuthoritySemanticMatch => 6,
            Self::MixedSemanticMatch => 7,
        }
    }

    const fn required_input_class(self) -> Option<GateInputClassV1> {
        match self {
            Self::EvidenceSetPresent => Some(GateInputClassV1::Evidence),
            Self::AuthoritySetPresent => Some(GateInputClassV1::Authority),
            Self::MixedSetPresent => Some(GateInputClassV1::Mixed),
            Self::EvidenceSemanticMatch => Some(GateInputClassV1::Evidence),
            Self::AuthoritySemanticMatch => Some(GateInputClassV1::Authority),
            Self::MixedSemanticMatch => Some(GateInputClassV1::Mixed),
            Self::ExactInputSet => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateEvaluatorDefinitionV1 {
    Leaf(GateLeafRuleV1),
    Composite(GateOperatorV1),
}

impl GateEvaluatorDefinitionV1 {
    fn canonical_value(self) -> CborValue {
        match self {
            Self::Leaf(rule) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(rule.tag()),
            ]),
            Self::Composite(operator) => {
                CborValue::Array(vec![CborValue::Unsigned(2), operator.canonical_value()])
            }
        }
    }
}

impl GateEvaluatorContractV1 {
    pub fn leaf(
        rule: GateLeafRuleV1,
        trust_root_snapshot_hash: [u8; 32],
    ) -> Result<Self, GateError> {
        Self::closed(
            GateEvaluatorDefinitionV1::Leaf(rule),
            trust_root_snapshot_hash,
        )
    }

    pub fn composite(
        operator: GateOperatorV1,
        trust_root_snapshot_hash: [u8; 32],
    ) -> Result<Self, GateError> {
        if operator == GateOperatorV1::Leaf {
            return Err(GateError::EvaluatorDefinitionMismatch);
        }
        Self::closed(
            GateEvaluatorDefinitionV1::Composite(operator),
            trust_root_snapshot_hash,
        )
    }

    fn closed(
        definition: GateEvaluatorDefinitionV1,
        trust_root_snapshot_hash: [u8; 32],
    ) -> Result<Self, GateError> {
        let version = 1;
        let definition_value = definition.canonical_value();
        let algorithm_hash = domain_hash(
            "maestro.vnext.gate-evaluator-algorithm.v1",
            &definition_value,
        )?;
        let implementation_hash = domain_hash(
            "maestro.vnext.gate-evaluator-implementation.v1",
            &definition_value,
        )?;
        let canonicalization_hash = domain_hash(
            "maestro.vnext.gate-evaluator-canonicalization.v1",
            &definition_value,
        )?;
        let redaction_hash = domain_hash(
            "maestro.vnext.gate-evaluator-redaction.v1",
            &definition_value,
        )?;
        for (label, value) in [
            ("Gate evaluator algorithm", algorithm_hash),
            ("Gate evaluator implementation", implementation_hash),
            ("Gate canonicalization", canonicalization_hash),
            ("Gate redaction", redaction_hash),
            ("Gate trust-root snapshot", trust_root_snapshot_hash),
        ] {
            require_nonzero(value, label)?;
        }
        let value = CborValue::Array(vec![
            CborValue::Unsigned(version),
            bytes(&algorithm_hash),
            bytes(&implementation_hash),
            bytes(&canonicalization_hash),
            bytes(&redaction_hash),
            bytes(&trust_root_snapshot_hash),
            definition_value,
        ]);
        Ok(Self {
            id: GateEvaluatorContractIdV1::from_bytes(domain_hash(
                "maestro.vnext.gate-evaluator-contract.v1",
                &value,
            )?)?,
            version,
            algorithm_hash,
            implementation_hash,
            canonicalization_hash,
            redaction_hash,
            trust_root_snapshot_hash,
            definition,
        })
    }

    pub const fn id(&self) -> GateEvaluatorContractIdV1 {
        self.id
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub const fn trust_root_snapshot_hash(&self) -> &[u8; 32] {
        &self.trust_root_snapshot_hash
    }

    pub const fn definition(&self) -> GateEvaluatorDefinitionV1 {
        self.definition
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            CborValue::Unsigned(self.version),
            bytes(&self.algorithm_hash),
            bytes(&self.implementation_hash),
            bytes(&self.canonicalization_hash),
            bytes(&self.redaction_hash),
            bytes(&self.trust_root_snapshot_hash),
            self.definition.canonical_value(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateNodeV1 {
    id: GateNodeIdV1,
    scope: GateScopeV1,
    input_class: GateInputClassV1,
    operator: GateOperatorV1,
    evaluator: GateEvaluatorContractV1,
    parameters_hash: [u8; 32],
    freshness_limit: Option<u64>,
    children: Vec<GateNodeIdV1>,
}

impl GateNodeV1 {
    pub fn new(
        scope: GateScopeV1,
        input_class: GateInputClassV1,
        operator: GateOperatorV1,
        evaluator: GateEvaluatorContractV1,
        parameters_hash: [u8; 32],
        freshness_limit: Option<u64>,
        mut children: Vec<GateNodeIdV1>,
    ) -> Result<Self, GateError> {
        require_nonzero(parameters_hash, "Gate parameters")?;
        if freshness_limit == Some(0) {
            return Err(GateError::ZeroFreshnessLimit);
        }
        if children.len() > MAX_GATE_CHILDREN_V1 {
            return Err(GateError::TooManyChildren);
        }
        children.sort_unstable();
        if children.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GateError::DuplicateChild);
        }
        match operator {
            GateOperatorV1::Leaf => {
                if input_class == GateInputClassV1::Composite || !children.is_empty() {
                    return Err(GateError::InvalidLeafShape);
                }
                let GateEvaluatorDefinitionV1::Leaf(rule) = evaluator.definition() else {
                    return Err(GateError::EvaluatorDefinitionMismatch);
                };
                if rule
                    .required_input_class()
                    .is_some_and(|required| required != input_class)
                {
                    return Err(GateError::EvaluatorDefinitionMismatch);
                }
            }
            GateOperatorV1::Quorum { required } => {
                if input_class != GateInputClassV1::Composite
                    || children.is_empty()
                    || required == 0
                    || usize::try_from(required).map_or(true, |value| value > children.len())
                {
                    return Err(GateError::InvalidCompositeShape);
                }
                if evaluator.definition() != GateEvaluatorDefinitionV1::Composite(operator) {
                    return Err(GateError::EvaluatorDefinitionMismatch);
                }
            }
            _ => {
                if input_class != GateInputClassV1::Composite || children.is_empty() {
                    return Err(GateError::InvalidCompositeShape);
                }
                if evaluator.definition() != GateEvaluatorDefinitionV1::Composite(operator) {
                    return Err(GateError::EvaluatorDefinitionMismatch);
                }
            }
        }
        let identity_value = gate_node_identity_value(
            scope,
            input_class,
            operator,
            &evaluator,
            parameters_hash,
            freshness_limit,
            &children,
        );
        Ok(Self {
            id: GateNodeIdV1::from_bytes(domain_hash(
                "maestro.vnext.gate-node.v1",
                &identity_value,
            )?)?,
            scope,
            input_class,
            operator,
            evaluator,
            parameters_hash,
            freshness_limit,
            children,
        })
    }

    pub const fn id(&self) -> GateNodeIdV1 {
        self.id
    }

    pub const fn input_class(&self) -> GateInputClassV1 {
        self.input_class
    }

    pub const fn scope(&self) -> GateScopeV1 {
        self.scope
    }

    pub const fn operator(&self) -> GateOperatorV1 {
        self.operator
    }

    pub fn evaluator(&self) -> &GateEvaluatorContractV1 {
        &self.evaluator
    }

    pub fn children(&self) -> &[GateNodeIdV1] {
        &self.children
    }

    pub const fn freshness_limit(&self) -> Option<u64> {
        self.freshness_limit
    }

    pub const fn parameters_hash(&self) -> &[u8; 32] {
        &self.parameters_hash
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            gate_node_identity_value(
                self.scope,
                self.input_class,
                self.operator,
                &self.evaluator,
                self.parameters_hash,
                self.freshness_limit,
                &self.children,
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateSnapshotV1 {
    id: GateSnapshotIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    contract_component_id: ContractComponentIdV1,
    expansion_engine_hash: [u8; 32],
    profile_provenance_hash: [u8; 32],
    roots: Vec<GateNodeIdV1>,
    nodes: Vec<GateNodeV1>,
}

impl GateSnapshotV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "a Gate Snapshot identity binds the exact Work, Contract component, evaluator provenance, roots, and nodes"
    )]
    pub fn new(
        work_id: WorkIdV1,
        contract_generation_id: ContractGenerationIdV1,
        contract_root_id: ContractRootIdV1,
        contract_component_id: ContractComponentIdV1,
        expansion_engine_hash: [u8; 32],
        profile_provenance_hash: [u8; 32],
        mut roots: Vec<GateNodeIdV1>,
        mut nodes: Vec<GateNodeV1>,
    ) -> Result<Self, GateError> {
        require_nonzero(*work_id.as_bytes(), "Gate Snapshot Work")?;
        require_nonzero(
            *contract_generation_id.as_bytes(),
            "Gate Snapshot Contract Generation",
        )?;
        require_nonzero(*contract_root_id.as_bytes(), "Gate Snapshot Contract Root")?;
        require_nonzero(
            *contract_component_id.as_bytes(),
            "Gate Snapshot Contract Component",
        )?;
        require_nonzero(expansion_engine_hash, "Gate expansion engine")?;
        require_nonzero(profile_provenance_hash, "Gate profile provenance")?;
        if nodes.is_empty() || nodes.len() > MAX_GATE_NODES_V1 {
            return Err(GateError::InvalidNodeCount);
        }
        roots.sort_unstable();
        if roots.is_empty() || roots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GateError::InvalidRoots);
        }
        nodes.sort_unstable_by_key(GateNodeV1::id);
        if nodes.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(GateError::DuplicateNode);
        }
        let by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
        if roots.iter().any(|root| !by_id.contains_key(root))
            || nodes
                .iter()
                .flat_map(|node| node.children.iter())
                .any(|child| !by_id.contains_key(child))
        {
            return Err(GateError::MissingNodeReference);
        }
        if nodes.iter().any(|node| {
            node.children.iter().any(|child| {
                by_id
                    .get(child)
                    .is_some_and(|child| child.scope != node.scope)
            })
        }) {
            return Err(GateError::CrossScopeEdge);
        }
        let mut visiting = BTreeSet::new();
        let mut reachable = BTreeSet::new();
        for root in &roots {
            visit_gate(*root, &by_id, &mut visiting, &mut reachable)?;
        }
        if reachable.len() != nodes.len() {
            return Err(GateError::DetachedNode);
        }
        let identity_value = gate_snapshot_identity_value(
            work_id,
            contract_generation_id,
            contract_root_id,
            contract_component_id,
            expansion_engine_hash,
            profile_provenance_hash,
            &roots,
            &nodes,
        );
        Ok(Self {
            id: GateSnapshotIdV1::from_bytes(domain_hash(
                "maestro.vnext.gate-snapshot.v1",
                &identity_value,
            )?)?,
            work_id,
            contract_generation_id,
            contract_root_id,
            contract_component_id,
            expansion_engine_hash,
            profile_provenance_hash,
            roots,
            nodes,
        })
    }

    pub const fn id(&self) -> GateSnapshotIdV1 {
        self.id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub const fn contract_generation_id(&self) -> ContractGenerationIdV1 {
        self.contract_generation_id
    }

    pub const fn contract_root_id(&self) -> ContractRootIdV1 {
        self.contract_root_id
    }

    pub const fn contract_component_id(&self) -> ContractComponentIdV1 {
        self.contract_component_id
    }

    pub fn roots(&self) -> &[GateNodeIdV1] {
        &self.roots
    }

    pub fn nodes(&self) -> &[GateNodeV1] {
        &self.nodes
    }

    pub fn node(&self, id: GateNodeIdV1) -> Option<&GateNodeV1> {
        self.nodes
            .binary_search_by_key(&id, GateNodeV1::id)
            .ok()
            .map(|index| &self.nodes[index])
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, GateError> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            gate_snapshot_identity_value(
                self.work_id,
                self.contract_generation_id,
                self.contract_root_id,
                self.contract_component_id,
                self.expansion_engine_hash,
                self.profile_provenance_hash,
                &self.roots,
                &self.nodes,
            ),
        ]))?)
    }

    pub fn from_canonical_bytes(value: &[u8]) -> Result<Self, GateError> {
        let decoded = deterministic_cbor::decode(value)?;
        let CborValue::Array(record) = &decoded else {
            return Err(GateError::InvalidStoredSnapshot);
        };
        let [stored_id, CborValue::Array(fields)] = record.as_slice() else {
            return Err(GateError::InvalidStoredSnapshot);
        };
        let [
            work_id,
            contract_generation_id,
            contract_root_id,
            contract_component_id,
            expansion_engine_hash,
            profile_provenance_hash,
            CborValue::Array(roots),
            CborValue::Array(nodes),
        ] = fields.as_slice()
        else {
            return Err(GateError::InvalidStoredSnapshot);
        };
        let snapshot = Self::new(
            WorkIdV1::parse(&render_gate_digest(exact_gate_digest(work_id)?))
                .map_err(|_| GateError::InvalidStoredSnapshot)?,
            ContractGenerationIdV1::parse(&render_gate_digest(exact_gate_digest(
                contract_generation_id,
            )?))
            .map_err(|_| GateError::InvalidStoredSnapshot)?,
            ContractRootIdV1::from_digest(exact_gate_digest(contract_root_id)?),
            ContractComponentIdV1::parse(&render_gate_digest(exact_gate_digest(
                contract_component_id,
            )?))
            .map_err(|_| GateError::InvalidStoredSnapshot)?,
            exact_gate_digest(expansion_engine_hash)?,
            exact_gate_digest(profile_provenance_hash)?,
            roots
                .iter()
                .map(|root| GateNodeIdV1::from_bytes(exact_gate_digest(root)?))
                .collect::<Result<Vec<_>, _>>()?,
            nodes
                .iter()
                .map(parse_gate_node)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        if exact_gate_digest(stored_id)? != *snapshot.id().as_bytes()
            || snapshot.canonical_bytes()? != value
        {
            return Err(GateError::InvalidStoredSnapshot);
        }
        Ok(snapshot)
    }
}

fn parse_gate_node(value: &CborValue) -> Result<GateNodeV1, GateError> {
    let CborValue::Array(record) = value else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    let [stored_id, CborValue::Array(fields)] = record.as_slice() else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    let [
        CborValue::Unsigned(scope),
        CborValue::Unsigned(input_class),
        operator,
        evaluator,
        parameters_hash,
        freshness_limit,
        CborValue::Array(children),
    ] = fields.as_slice()
    else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    let node = GateNodeV1::new(
        match scope {
            1 => GateScopeV1::Work,
            2 => GateScopeV1::Step,
            _ => return Err(GateError::InvalidStoredSnapshot),
        },
        match input_class {
            1 => GateInputClassV1::Evidence,
            2 => GateInputClassV1::Authority,
            3 => GateInputClassV1::Mixed,
            4 => GateInputClassV1::Composite,
            _ => return Err(GateError::InvalidStoredSnapshot),
        },
        parse_gate_operator(operator)?,
        parse_gate_evaluator(evaluator)?,
        exact_gate_digest(parameters_hash)?,
        parse_gate_optional_u64(freshness_limit)?,
        children
            .iter()
            .map(|child| GateNodeIdV1::from_bytes(exact_gate_digest(child)?))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    if exact_gate_digest(stored_id)? != *node.id().as_bytes() || &node.canonical_value() != value {
        return Err(GateError::InvalidStoredSnapshot);
    }
    Ok(node)
}

fn parse_gate_evaluator(value: &CborValue) -> Result<GateEvaluatorContractV1, GateError> {
    let CborValue::Array(fields) = value else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    let [
        stored_id,
        CborValue::Unsigned(version),
        algorithm_hash,
        implementation_hash,
        canonicalization_hash,
        redaction_hash,
        trust_root_snapshot_hash,
        definition,
    ] = fields.as_slice()
    else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    if *version != 1 {
        return Err(GateError::InvalidStoredSnapshot);
    }
    let trust_root_snapshot_hash = exact_gate_digest(trust_root_snapshot_hash)?;
    let CborValue::Array(definition) = definition else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    let evaluator = match definition.as_slice() {
        [CborValue::Unsigned(1), CborValue::Unsigned(rule)] => {
            GateEvaluatorContractV1::leaf(parse_gate_leaf_rule(*rule)?, trust_root_snapshot_hash)?
        }
        [CborValue::Unsigned(2), operator] => GateEvaluatorContractV1::composite(
            parse_gate_operator(operator)?,
            trust_root_snapshot_hash,
        )?,
        _ => return Err(GateError::InvalidStoredSnapshot),
    };
    if exact_gate_digest(stored_id)? != *evaluator.id().as_bytes()
        || exact_gate_digest(algorithm_hash)? != evaluator.algorithm_hash
        || exact_gate_digest(implementation_hash)? != evaluator.implementation_hash
        || exact_gate_digest(canonicalization_hash)? != evaluator.canonicalization_hash
        || exact_gate_digest(redaction_hash)? != evaluator.redaction_hash
        || &evaluator.canonical_value() != value
    {
        return Err(GateError::InvalidStoredSnapshot);
    }
    Ok(evaluator)
}

fn parse_gate_operator(value: &CborValue) -> Result<GateOperatorV1, GateError> {
    let CborValue::Array(fields) = value else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(1)] => Ok(GateOperatorV1::Leaf),
        [CborValue::Unsigned(2)] => Ok(GateOperatorV1::All),
        [CborValue::Unsigned(3)] => Ok(GateOperatorV1::Any),
        [CborValue::Unsigned(4), CborValue::Unsigned(required)] => Ok(GateOperatorV1::Quorum {
            required: u32::try_from(*required).map_err(|_| GateError::InvalidStoredSnapshot)?,
        }),
        [CborValue::Unsigned(5)] => Ok(GateOperatorV1::Veto),
        [CborValue::Unsigned(6)] => Ok(GateOperatorV1::DenyOverrides),
        _ => Err(GateError::InvalidStoredSnapshot),
    }
}

fn parse_gate_leaf_rule(tag: u64) -> Result<GateLeafRuleV1, GateError> {
    match tag {
        1 => Ok(GateLeafRuleV1::EvidenceSetPresent),
        2 => Ok(GateLeafRuleV1::AuthoritySetPresent),
        3 => Ok(GateLeafRuleV1::MixedSetPresent),
        4 => Ok(GateLeafRuleV1::ExactInputSet),
        5 => Ok(GateLeafRuleV1::EvidenceSemanticMatch),
        6 => Ok(GateLeafRuleV1::AuthoritySemanticMatch),
        7 => Ok(GateLeafRuleV1::MixedSemanticMatch),
        _ => Err(GateError::InvalidStoredSnapshot),
    }
}

fn parse_gate_optional_u64(value: &CborValue) -> Result<Option<u64>, GateError> {
    let CborValue::Array(fields) = value else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), CborValue::Unsigned(value)] => Ok(Some(*value)),
        _ => Err(GateError::InvalidStoredSnapshot),
    }
}

fn exact_gate_digest(value: &CborValue) -> Result<[u8; 32], GateError> {
    let CborValue::Bytes(value) = value else {
        return Err(GateError::InvalidStoredSnapshot);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| GateError::InvalidStoredSnapshot)
}

fn render_gate_digest(bytes: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GateEvaluationInputV1 {
    source_gate_id: Option<GateNodeIdV1>,
    input_commitment: [u8; 32],
    result: GateEvaluationResultV1,
}

impl GateEvaluationInputV1 {
    pub fn leaf(
        input_commitment: [u8; 32],
        result: GateEvaluationResultV1,
    ) -> Result<Self, GateError> {
        require_nonzero(input_commitment, "Gate leaf input")?;
        Ok(Self {
            source_gate_id: None,
            input_commitment,
            result,
        })
    }

    pub fn child(
        source_gate_id: GateNodeIdV1,
        assessment_commitment: [u8; 32],
        result: GateEvaluationResultV1,
    ) -> Result<Self, GateError> {
        require_nonzero(assessment_commitment, "Gate child Assessment")?;
        Ok(Self {
            source_gate_id: Some(source_gate_id),
            input_commitment: assessment_commitment,
            result,
        })
    }

    pub const fn result(self) -> GateEvaluationResultV1 {
        self.result
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::optional(self.source_gate_id.map(|id| bytes(id.as_bytes()))),
            bytes(&self.input_commitment),
            CborValue::Unsigned(self.result.tag()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateEvaluationV1 {
    gate_id: GateNodeIdV1,
    evaluator_contract_id: GateEvaluatorContractIdV1,
    input_set_hash: [u8; 32],
    result: GateEvaluationResultV1,
}

impl GateEvaluationV1 {
    pub const fn gate_id(&self) -> GateNodeIdV1 {
        self.gate_id
    }

    pub const fn evaluator_contract_id(&self) -> GateEvaluatorContractIdV1 {
        self.evaluator_contract_id
    }

    pub const fn input_set_hash(&self) -> &[u8; 32] {
        &self.input_set_hash
    }

    pub const fn result(&self) -> GateEvaluationResultV1 {
        self.result
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PureGateEvaluatorV1;

impl PureGateEvaluatorV1 {
    pub fn evaluate(
        &self,
        snapshot: &GateSnapshotV1,
        gate_id: GateNodeIdV1,
        mut inputs: Vec<GateEvaluationInputV1>,
    ) -> Result<GateEvaluationV1, GateError> {
        let node = snapshot
            .node(gate_id)
            .ok_or(GateError::MissingNodeReference)?;
        match node.operator {
            GateOperatorV1::Leaf => return Err(GateError::LeafRequiresPinnedEvaluator),
            _ => {
                inputs.sort_unstable_by_key(|input| input.source_gate_id);
                if inputs.len() != node.children.len()
                    || inputs.iter().map(|input| input.source_gate_id).ne(node
                        .children
                        .iter()
                        .copied()
                        .map(Some))
                {
                    return Err(GateError::InvalidEvaluationInputs);
                }
            }
        }
        let result = evaluate_results(node.operator, &inputs)?;
        let input_set_hash = domain_hash(
            "maestro.vnext.gate-evaluation-input-set.v1",
            &CborValue::Array(
                inputs
                    .iter()
                    .copied()
                    .map(GateEvaluationInputV1::canonical_value)
                    .collect(),
            ),
        )?;
        Ok(GateEvaluationV1 {
            gate_id,
            evaluator_contract_id: node.evaluator.id,
            input_set_hash,
            result,
        })
    }
}

fn evaluate_results(
    operator: GateOperatorV1,
    inputs: &[GateEvaluationInputV1],
) -> Result<GateEvaluationResultV1, GateError> {
    let count = |result| inputs.iter().filter(|input| input.result == result).count();
    let pass = count(GateEvaluationResultV1::Pass);
    let fail = count(GateEvaluationResultV1::Fail);
    let indeterminate = count(GateEvaluationResultV1::Indeterminate);
    let error = count(GateEvaluationResultV1::Error);
    Ok(match operator {
        GateOperatorV1::Leaf => return Err(GateError::LeafRequiresPinnedEvaluator),
        GateOperatorV1::All => {
            if error > 0 {
                GateEvaluationResultV1::Error
            } else if fail > 0 {
                GateEvaluationResultV1::Fail
            } else if indeterminate > 0 {
                GateEvaluationResultV1::Indeterminate
            } else {
                GateEvaluationResultV1::Pass
            }
        }
        GateOperatorV1::Any => {
            if pass > 0 {
                GateEvaluationResultV1::Pass
            } else if error > 0 {
                GateEvaluationResultV1::Error
            } else if indeterminate > 0 {
                GateEvaluationResultV1::Indeterminate
            } else {
                GateEvaluationResultV1::Fail
            }
        }
        GateOperatorV1::Quorum { required } => {
            let required =
                usize::try_from(required).map_err(|_| GateError::InvalidCompositeShape)?;
            if pass >= required {
                GateEvaluationResultV1::Pass
            } else if pass + indeterminate + error < required {
                GateEvaluationResultV1::Fail
            } else if error > 0 {
                GateEvaluationResultV1::Error
            } else {
                GateEvaluationResultV1::Indeterminate
            }
        }
        GateOperatorV1::Veto | GateOperatorV1::DenyOverrides => {
            if fail > 0 {
                GateEvaluationResultV1::Fail
            } else if error > 0 {
                GateEvaluationResultV1::Error
            } else if indeterminate > 0 {
                GateEvaluationResultV1::Indeterminate
            } else {
                GateEvaluationResultV1::Pass
            }
        }
    })
}

fn visit_gate(
    id: GateNodeIdV1,
    nodes: &BTreeMap<GateNodeIdV1, &GateNodeV1>,
    visiting: &mut BTreeSet<GateNodeIdV1>,
    visited: &mut BTreeSet<GateNodeIdV1>,
) -> Result<(), GateError> {
    if visited.contains(&id) {
        return Ok(());
    }
    if !visiting.insert(id) {
        return Err(GateError::CyclicGraph);
    }
    let node = nodes.get(&id).ok_or(GateError::MissingNodeReference)?;
    for child in &node.children {
        visit_gate(*child, nodes, visiting, visited)?;
    }
    visiting.remove(&id);
    visited.insert(id);
    Ok(())
}

fn gate_node_identity_value(
    scope: GateScopeV1,
    input_class: GateInputClassV1,
    operator: GateOperatorV1,
    evaluator: &GateEvaluatorContractV1,
    parameters_hash: [u8; 32],
    freshness_limit: Option<u64>,
    children: &[GateNodeIdV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(scope.tag()),
        CborValue::Unsigned(input_class.tag()),
        operator.canonical_value(),
        evaluator.canonical_value(),
        bytes(&parameters_hash),
        CborValue::optional(freshness_limit.map(CborValue::Unsigned)),
        CborValue::Array(children.iter().map(|id| bytes(id.as_bytes())).collect()),
    ])
}

#[expect(
    clippy::too_many_arguments,
    reason = "the canonical Gate Snapshot identity must receive every identity-bearing field explicitly"
)]
fn gate_snapshot_identity_value(
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    contract_component_id: ContractComponentIdV1,
    expansion_engine_hash: [u8; 32],
    profile_provenance_hash: [u8; 32],
    roots: &[GateNodeIdV1],
    nodes: &[GateNodeV1],
) -> CborValue {
    CborValue::Array(vec![
        bytes(work_id.as_bytes()),
        bytes(contract_generation_id.as_bytes()),
        bytes(contract_root_id.as_bytes()),
        bytes(contract_component_id.as_bytes()),
        bytes(&expansion_engine_hash),
        bytes(&profile_provenance_hash),
        CborValue::Array(roots.iter().map(|id| bytes(id.as_bytes())).collect()),
        CborValue::Array(nodes.iter().map(GateNodeV1::canonical_value).collect()),
    ])
}

fn domain_hash(domain: &str, value: &CborValue) -> Result<[u8; 32], GateError> {
    let canonical = deterministic_cbor::encode(&CborValue::Array(vec![
        CborValue::text(domain)?,
        value.clone(),
    ]))?;
    let digest: [u8; 32] = Sha256::digest(canonical).into();
    require_nonzero(digest, "derived Gate digest")?;
    Ok(digest)
}

fn require_nonzero(bytes: [u8; 32], label: &'static str) -> Result<(), GateError> {
    if bytes == [0; 32] {
        Err(GateError::MissingReference(label))
    } else {
        Ok(())
    }
}

fn bytes(value: &[u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GateError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Gate material {0} must not be the all-zero missing reference")]
    MissingReference(&'static str),
    #[error("Gate protocol version must be positive")]
    ZeroVersion,
    #[error("Gate freshness limit must be positive when present")]
    ZeroFreshnessLimit,
    #[error("Gate node has too many children")]
    TooManyChildren,
    #[error("Gate node repeats a child")]
    DuplicateChild,
    #[error("Gate leaf shape is invalid")]
    InvalidLeafShape,
    #[error("Gate composite shape is invalid")]
    InvalidCompositeShape,
    #[error("Gate Snapshot node count is outside the admitted bound")]
    InvalidNodeCount,
    #[error("Gate Snapshot roots are empty or duplicated")]
    InvalidRoots,
    #[error("Gate Snapshot repeats a node")]
    DuplicateNode,
    #[error("Gate Snapshot references a missing node")]
    MissingNodeReference,
    #[error("Gate Snapshot contains a parent/child edge across Work and Step scopes")]
    CrossScopeEdge,
    #[error("Gate Snapshot graph is cyclic")]
    CyclicGraph,
    #[error("Gate Snapshot contains a node outside its root-reachable closure")]
    DetachedNode,
    #[error("Gate evaluation inputs do not exactly match the pinned node")]
    InvalidEvaluationInputs,
    #[error("Gate leaves require their pinned evaluator and cannot accept a proposed result")]
    LeafRequiresPinnedEvaluator,
    #[error("Gate evaluator definition does not match the exact leaf rule or composite operator")]
    EvaluatorDefinitionMismatch,
    #[error("stored Gate Snapshot is malformed, non-canonical, or identity-inconsistent")]
    InvalidStoredSnapshot,
}
