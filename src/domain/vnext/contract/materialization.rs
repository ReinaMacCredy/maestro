use thiserror::Error;

use crate::domain::vnext::identity::{
    ContractRootIdV1, DecisionClosureIdV1, DecisionMaterializationIdV1, DecisionResolutionIdV1,
    IdentityError, decision_resolution_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub const MATERIALIZATION_BASE_VERSION_V1: u64 = 1;
pub const DECISION_MATERIALIZATION_RESOLUTION_VERSION_V1: u64 = 1;

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
