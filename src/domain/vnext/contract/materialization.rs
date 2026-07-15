use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::identity::{
    ContractComponentIdV1, ContractRootIdV1, DecisionClosureIdV1, DecisionMaterializationIdV1,
    DecisionResolutionIdV1, IdentityError, SchemaIdV1, decision_resolution_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::component_kind::ContractComponentKindV1;
use super::root::CandidateContractRootV1;

pub const MATERIALIZATION_BASE_VERSION_V1: u64 = 1;
pub const DECISION_MATERIALIZATION_RESOLUTION_VERSION_V1: u64 = 1;
const CONTRACT_CONSEQUENCE_PLAN_VERSION_V1: u64 = 1;
const MAX_PLANNED_COMPONENTS_V1: usize = 65_536;
const MAX_PLANNED_DEPENDENCIES_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaterializationBaseV1 {
    InitialExternalDesignClosure(DecisionClosureIdV1),
    PriorContractRoot(ContractRootIdV1),
}

impl MaterializationBaseV1 {
    pub const fn initial_external_design_closure(decision_closure_id: DecisionClosureIdV1) -> Self {
        Self::InitialExternalDesignClosure(decision_closure_id)
    }

    pub const fn prior_contract_root(root_id: ContractRootIdV1) -> Self {
        Self::PriorContractRoot(root_id)
    }

    pub const fn initial_decision_closure_id(&self) -> Option<&DecisionClosureIdV1> {
        match self {
            Self::InitialExternalDesignClosure(decision_closure_id) => Some(decision_closure_id),
            Self::PriorContractRoot(_) => None,
        }
    }

    pub const fn prior_root_id(&self) -> Option<&ContractRootIdV1> {
        match self {
            Self::InitialExternalDesignClosure(_) => None,
            Self::PriorContractRoot(root_id) => Some(root_id),
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::InitialExternalDesignClosure(decision_closure_id) => CborValue::Array(vec![
                CborValue::Unsigned(MATERIALIZATION_BASE_VERSION_V1),
                CborValue::Unsigned(1),
                CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
            ]),
            Self::PriorContractRoot(root_id) => CborValue::Array(vec![
                CborValue::Unsigned(MATERIALIZATION_BASE_VERSION_V1),
                CborValue::Unsigned(2),
                CborValue::Bytes(root_id.as_bytes().to_vec()),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionMaterializationResolutionV1 {
    decision_closure_id: DecisionClosureIdV1,
    materialization_base: MaterializationBaseV1,
    materialization_id: DecisionMaterializationIdV1,
    resolution_id: DecisionResolutionIdV1,
}

impl DecisionMaterializationResolutionV1 {
    pub fn new(
        decision_closure_id: DecisionClosureIdV1,
        materialization_base: MaterializationBaseV1,
        materialization_id: DecisionMaterializationIdV1,
    ) -> Result<Self, MaterializationError> {
        if let Some(initial_closure_id) = materialization_base.initial_decision_closure_id()
            && initial_closure_id != &decision_closure_id
        {
            return Err(MaterializationError::InitialBaseDoesNotMatchDecisionClosure);
        }
        let canonical_value = resolution_value(
            &decision_closure_id,
            &materialization_base,
            &materialization_id,
        );
        let resolution_id = decision_resolution_identity(&canonical_value)?;
        Ok(Self {
            decision_closure_id,
            materialization_base,
            materialization_id,
            resolution_id,
        })
    }

    pub const fn decision_closure_id(&self) -> &DecisionClosureIdV1 {
        &self.decision_closure_id
    }

    pub const fn materialization_base(&self) -> &MaterializationBaseV1 {
        &self.materialization_base
    }

    pub const fn materialization_id(&self) -> &DecisionMaterializationIdV1 {
        &self.materialization_id
    }

    pub const fn resolution_id(&self) -> &DecisionResolutionIdV1 {
        &self.resolution_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, MaterializationError> {
        Ok(deterministic_cbor::encode(&resolution_value(
            &self.decision_closure_id,
            &self.materialization_base,
            &self.materialization_id,
        ))?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MaterializationError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(
        "the initial external design closure materialization base must match the decision closure"
    )]
    InitialBaseDoesNotMatchDecisionClosure,
}

fn resolution_value(
    decision_closure_id: &DecisionClosureIdV1,
    materialization_base: &MaterializationBaseV1,
    materialization_id: &DecisionMaterializationIdV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DECISION_MATERIALIZATION_RESOLUTION_VERSION_V1),
        materialization_base.canonical_value(),
        CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
        CborValue::Bytes(materialization_id.as_bytes().to_vec()),
    ])
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContractConsequencePlanIdV1([u8; 32]);

impl ContractConsequencePlanIdV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecisionMaterializationPreimageCommitmentV1([u8; 32]);

impl DecisionMaterializationPreimageCommitmentV1 {
    pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlannedContractSlotV1(u32);

impl PlannedContractSlotV1 {
    pub fn new(value: u32) -> Result<Self, ContractConsequencePlanError> {
        if value == 0 {
            return Err(ContractConsequencePlanError::InvalidPlannedSlot);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlannedContractDependencyV1 {
    Retained(ContractComponentIdV1),
    Planned(PlannedContractSlotV1),
}

impl PlannedContractDependencyV1 {
    fn canonical_value(self) -> CborValue {
        match self {
            Self::Retained(component_id) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(component_id.as_bytes().to_vec()),
            ]),
            Self::Planned(slot) => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Unsigned(u64::from(slot.get())),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedContractComponentV1 {
    slot: PlannedContractSlotV1,
    kind: ContractComponentKindV1,
    schema_id: SchemaIdV1,
    value: CborValue,
    dependencies: Vec<PlannedContractDependencyV1>,
}

impl PlannedContractComponentV1 {
    pub fn new(
        slot: PlannedContractSlotV1,
        kind: ContractComponentKindV1,
        schema_id: SchemaIdV1,
        value: CborValue,
        mut dependencies: Vec<PlannedContractDependencyV1>,
    ) -> Result<Self, ContractConsequencePlanError> {
        if dependencies.len() > MAX_PLANNED_DEPENDENCIES_V1 {
            return Err(ContractConsequencePlanError::TooManyPlannedDependencies);
        }
        dependencies.sort_unstable();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ContractConsequencePlanError::DuplicatePlannedDependency);
        }
        Ok(Self {
            slot,
            kind,
            schema_id,
            value,
            dependencies,
        })
    }

    pub const fn slot(&self) -> PlannedContractSlotV1 {
        self.slot
    }

    pub const fn kind(&self) -> ContractComponentKindV1 {
        self.kind
    }

    pub const fn schema_id(&self) -> &SchemaIdV1 {
        &self.schema_id
    }

    pub const fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn dependencies(&self) -> &[PlannedContractDependencyV1] {
        &self.dependencies
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(u64::from(self.slot.get())),
            CborValue::Unsigned(self.kind.tag()),
            CborValue::Bytes(self.schema_id.as_bytes().to_vec()),
            self.value.clone(),
            CborValue::Array(
                self.dependencies
                    .iter()
                    .copied()
                    .map(PlannedContractDependencyV1::canonical_value)
                    .collect(),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractConsequencePlanV1 {
    plan_kind: u16,
    base_root_id: ContractRootIdV1,
    retained_component_ids: Vec<ContractComponentIdV1>,
    additions: Vec<PlannedContractComponentV1>,
    plan_id: ContractConsequencePlanIdV1,
}

impl ContractConsequencePlanV1 {
    pub fn new(
        plan_kind: u16,
        base_root: &CandidateContractRootV1,
        mut retained_component_ids: Vec<ContractComponentIdV1>,
        mut additions: Vec<PlannedContractComponentV1>,
    ) -> Result<Self, ContractConsequencePlanError> {
        if plan_kind == 0 {
            return Err(ContractConsequencePlanError::InvalidPlanKind);
        }
        if additions.len() > MAX_PLANNED_COMPONENTS_V1 {
            return Err(ContractConsequencePlanError::TooManyPlannedComponents);
        }
        retained_component_ids.sort_unstable();
        if retained_component_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(ContractConsequencePlanError::DuplicateRetainedComponent);
        }
        let base_ids = base_root
            .components()
            .iter()
            .map(|component| *component.component_id())
            .collect::<BTreeSet<_>>();
        if retained_component_ids
            .iter()
            .any(|component_id| !base_ids.contains(component_id))
        {
            return Err(ContractConsequencePlanError::UnknownRetainedComponent);
        }
        additions.sort_by_key(PlannedContractComponentV1::slot);
        if additions
            .windows(2)
            .any(|pair| pair[0].slot() == pair[1].slot())
        {
            return Err(ContractConsequencePlanError::DuplicatePlannedSlot);
        }
        let slots = additions
            .iter()
            .map(PlannedContractComponentV1::slot)
            .collect::<BTreeSet<_>>();
        let retained = retained_component_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for addition in &additions {
            for dependency in addition.dependencies() {
                match dependency {
                    PlannedContractDependencyV1::Retained(component_id)
                        if !retained.contains(component_id) =>
                    {
                        return Err(ContractConsequencePlanError::UnknownRetainedDependency);
                    }
                    PlannedContractDependencyV1::Planned(slot)
                        if slot == &addition.slot() || !slots.contains(slot) =>
                    {
                        return Err(ContractConsequencePlanError::UnknownOrSelfPlannedDependency);
                    }
                    PlannedContractDependencyV1::Retained(_)
                    | PlannedContractDependencyV1::Planned(_) => {}
                }
            }
        }
        let value = contract_consequence_plan_value(
            plan_kind,
            base_root.root_id(),
            &retained_component_ids,
            &additions,
        );
        let plan_id =
            ContractConsequencePlanIdV1(Sha256::digest(deterministic_cbor::encode(&value)?).into());
        Ok(Self {
            plan_kind,
            base_root_id: *base_root.root_id(),
            retained_component_ids,
            additions,
            plan_id,
        })
    }

    pub const fn plan_kind(&self) -> u16 {
        self.plan_kind
    }

    pub const fn base_root_id(&self) -> &ContractRootIdV1 {
        &self.base_root_id
    }

    pub fn retained_component_ids(&self) -> &[ContractComponentIdV1] {
        &self.retained_component_ids
    }

    pub fn additions(&self) -> &[PlannedContractComponentV1] {
        &self.additions
    }

    pub const fn plan_id(&self) -> ContractConsequencePlanIdV1 {
        self.plan_id
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        contract_consequence_plan_value(
            self.plan_kind,
            &self.base_root_id,
            &self.retained_component_ids,
            &self.additions,
        )
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ContractConsequencePlanError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("typed consequence-plan kind must be positive")]
    InvalidPlanKind,
    #[error("planned Contract component slot must be positive")]
    InvalidPlannedSlot,
    #[error("typed consequence plan exceeds the finite component limit")]
    TooManyPlannedComponents,
    #[error("planned Contract component exceeds the finite dependency limit")]
    TooManyPlannedDependencies,
    #[error("typed consequence plan repeats a retained component")]
    DuplicateRetainedComponent,
    #[error("typed consequence plan retains a component outside its exact base closure")]
    UnknownRetainedComponent,
    #[error("typed consequence plan repeats a planned component slot")]
    DuplicatePlannedSlot,
    #[error("planned Contract component repeats a dependency")]
    DuplicatePlannedDependency,
    #[error("planned Contract component depends on a base component that is not retained")]
    UnknownRetainedDependency,
    #[error("planned Contract component has an unknown or self-referential planned dependency")]
    UnknownOrSelfPlannedDependency,
}

fn contract_consequence_plan_value(
    plan_kind: u16,
    base_root_id: &ContractRootIdV1,
    retained_component_ids: &[ContractComponentIdV1],
    additions: &[PlannedContractComponentV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(CONTRACT_CONSEQUENCE_PLAN_VERSION_V1),
        CborValue::Unsigned(u64::from(plan_kind)),
        CborValue::Bytes(base_root_id.as_bytes().to_vec()),
        CborValue::Array(
            retained_component_ids
                .iter()
                .map(|component_id| CborValue::Bytes(component_id.as_bytes().to_vec()))
                .collect(),
        ),
        CborValue::Array(
            additions
                .iter()
                .map(PlannedContractComponentV1::canonical_value)
                .collect(),
        ),
    ])
}
