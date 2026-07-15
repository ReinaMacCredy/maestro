use thiserror::Error;

use crate::domain::vnext::identity::{
    ContractComponentIdV1, IdentityError, SchemaClosureV1, SchemaError, SchemaIdV1,
    contract_component_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::component_kind::ContractComponentKindV1;
use super::provenance::ComponentProvenanceV1;

pub const CONTRACT_COMPONENT_VERSION_V1: u64 = 1;
pub const MAX_COMPONENT_DEPENDENCIES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateContractComponentV1 {
    kind: ContractComponentKindV1,
    schema_id: SchemaIdV1,
    value: CborValue,
    dependencies: Vec<ContractComponentIdV1>,
    provenance: ComponentProvenanceV1,
    component_id: ContractComponentIdV1,
}

impl CandidateContractComponentV1 {
    pub fn new(
        schema_closure: &SchemaClosureV1,
        kind: ContractComponentKindV1,
        schema_id: SchemaIdV1,
        value: CborValue,
        dependencies: Vec<ContractComponentIdV1>,
        provenance: ComponentProvenanceV1,
    ) -> Result<Self, ContractComponentError> {
        if dependencies.len() > MAX_COMPONENT_DEPENDENCIES {
            return Err(ContractComponentError::TooManyDependencies);
        }
        if dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ContractComponentError::DependenciesNotStrictlySorted);
        }
        schema_closure.validate_value(&schema_id, &value)?;
        let canonical_value = component_value(kind, &schema_id, &value, &dependencies, &provenance);
        let component_id = contract_component_identity(&canonical_value)?;
        Ok(Self {
            kind,
            schema_id,
            value,
            dependencies,
            provenance,
            component_id,
        })
    }

    pub fn kind(&self) -> ContractComponentKindV1 {
        self.kind
    }

    pub fn schema_id(&self) -> &SchemaIdV1 {
        &self.schema_id
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub fn dependencies(&self) -> &[ContractComponentIdV1] {
        &self.dependencies
    }

    pub fn provenance(&self) -> &ComponentProvenanceV1 {
        &self.provenance
    }

    pub fn component_id(&self) -> &ContractComponentIdV1 {
        &self.component_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ContractComponentError> {
        Ok(deterministic_cbor::encode(&self.canonical_value())?)
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        component_value(
            self.kind,
            &self.schema_id,
            &self.value,
            &self.dependencies,
            &self.provenance,
        )
    }

    pub(crate) fn canonical_record_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.component_id.as_bytes().to_vec()),
            self.canonical_value(),
        ])
    }
}

#[derive(Debug, Error)]
pub enum ContractComponentError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("component dependency set exceeds the finite v1 limit")]
    TooManyDependencies,
    #[error("component dependencies must be strictly sorted by raw identity bytes")]
    DependenciesNotStrictlySorted,
}

fn component_value(
    kind: ContractComponentKindV1,
    schema_id: &SchemaIdV1,
    value: &CborValue,
    dependencies: &[ContractComponentIdV1],
    provenance: &ComponentProvenanceV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(CONTRACT_COMPONENT_VERSION_V1),
        CborValue::Unsigned(kind.tag()),
        CborValue::Bytes(schema_id.as_bytes().to_vec()),
        value.clone(),
        CborValue::Array(
            dependencies
                .iter()
                .map(|dependency| CborValue::Bytes(dependency.as_bytes().to_vec()))
                .collect(),
        ),
        provenance.canonical_value(),
    ])
}
