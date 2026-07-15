use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::vnext::identity::{
    ContractRootIdV1, IdentityError, SchemaClosureV1, SchemaError, contract_root_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::component::CandidateContractComponentV1;
use super::component_kind::ContractComponentKindV1;

pub const CONTRACT_ROOT_VERSION_V1: u64 = 1;
pub const MAX_CONTRACT_COMPONENTS: usize = 65_536;

#[derive(Clone, Debug)]
pub struct CandidateContractRootV1 {
    components: Vec<CandidateContractComponentV1>,
    root_id: ContractRootIdV1,
}

impl CandidateContractRootV1 {
    pub fn new(
        schema_closure: &SchemaClosureV1,
        components: Vec<CandidateContractComponentV1>,
    ) -> Result<Self, ContractRootError> {
        if components.len() > MAX_CONTRACT_COMPONENTS {
            return Err(ContractRootError::TooManyComponents);
        }
        for component in &components {
            schema_closure.validate_value(component.schema_id(), component.value())?;
        }
        validate_complete_kinds(&components)?;
        let components = deterministic_topological_order(components)?;
        let canonical_value = root_value(&components);
        let root_id = contract_root_identity(&canonical_value)?;
        Ok(Self {
            components,
            root_id,
        })
    }

    pub fn root_id(&self) -> &ContractRootIdV1 {
        &self.root_id
    }

    pub fn components(&self) -> &[CandidateContractComponentV1] {
        &self.components
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ContractRootError> {
        Ok(deterministic_cbor::encode(&root_value(&self.components))?)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ContractRootError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("candidate Contract root exceeds the finite v1 component limit")]
    TooManyComponents,
    #[error("candidate Contract closure is missing component kind {0:?}")]
    MissingComponentKind(ContractComponentKindV1),
    #[error("candidate Contract closure repeats aggregate component kind {0:?}")]
    DuplicateAggregateComponentKind(ContractComponentKindV1),
    #[error("candidate Contract closure repeats a component identity")]
    DuplicateComponentIdentity,
    #[error("candidate Contract component references an unknown dependency")]
    UnknownComponentDependency,
    #[error("candidate Contract component graph is cyclic")]
    CyclicComponentDependency,
    #[error("candidate Contract component dependency is not strictly backward")]
    NonBackwardComponentDependency,
}

fn validate_complete_kinds(
    components: &[CandidateContractComponentV1],
) -> Result<(), ContractRootError> {
    let mut counts = BTreeMap::new();
    for component in components {
        *counts.entry(component.kind()).or_insert(0_usize) += 1;
    }
    for required in ContractComponentKindV1::ALL {
        let count = counts.get(&required).copied().unwrap_or_default();
        if count == 0 {
            return Err(ContractRootError::MissingComponentKind(required));
        }
        if required != ContractComponentKindV1::NormativeInputs && count != 1 {
            return Err(ContractRootError::DuplicateAggregateComponentKind(required));
        }
    }
    Ok(())
}

fn deterministic_topological_order(
    components: Vec<CandidateContractComponentV1>,
) -> Result<Vec<CandidateContractComponentV1>, ContractRootError> {
    let mut indices = BTreeMap::new();
    for (index, component) in components.iter().enumerate() {
        if indices
            .insert(*component.component_id().as_bytes(), index)
            .is_some()
        {
            return Err(ContractRootError::DuplicateComponentIdentity);
        }
    }

    let mut indegrees = vec![0_usize; components.len()];
    let mut dependents = vec![Vec::new(); components.len()];
    for (index, component) in components.iter().enumerate() {
        for dependency in component.dependencies() {
            let dependency_index = indices
                .get(dependency.as_bytes())
                .copied()
                .ok_or(ContractRootError::UnknownComponentDependency)?;
            indegrees[index] = indegrees[index]
                .checked_add(1)
                .ok_or(ContractRootError::TooManyComponents)?;
            dependents[dependency_index].push(index);
        }
    }

    let mut ready = BTreeSet::new();
    for (index, component) in components.iter().enumerate() {
        if indegrees[index] == 0 {
            ready.insert((
                component.kind().tag(),
                *component.component_id().as_bytes(),
                index,
            ));
        }
    }

    let mut ordered_indices = Vec::with_capacity(components.len());
    while let Some((_, _, index)) = ready.pop_first() {
        ordered_indices.push(index);
        for dependent in &dependents[index] {
            indegrees[*dependent] -= 1;
            if indegrees[*dependent] == 0 {
                let component = &components[*dependent];
                ready.insert((
                    component.kind().tag(),
                    *component.component_id().as_bytes(),
                    *dependent,
                ));
            }
        }
    }
    if ordered_indices.len() != components.len() {
        return Err(ContractRootError::CyclicComponentDependency);
    }

    let positions: BTreeMap<[u8; 32], usize> = ordered_indices
        .iter()
        .enumerate()
        .map(|(position, index)| (*components[*index].component_id().as_bytes(), position))
        .collect();
    for (position, index) in ordered_indices.iter().enumerate() {
        for dependency in components[*index].dependencies() {
            let dependency_position = positions
                .get(dependency.as_bytes())
                .copied()
                .ok_or(ContractRootError::UnknownComponentDependency)?;
            if dependency_position >= position {
                return Err(ContractRootError::NonBackwardComponentDependency);
            }
        }
    }

    Ok(ordered_indices
        .into_iter()
        .map(|index| components[index].clone())
        .collect())
}

fn root_value(components: &[CandidateContractComponentV1]) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(CONTRACT_ROOT_VERSION_V1),
        CborValue::Unsigned(components.len() as u64),
        CborValue::Array(
            components
                .iter()
                .map(CandidateContractComponentV1::canonical_record_value)
                .collect(),
        ),
    ])
}
