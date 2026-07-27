use thiserror::Error;

use crate::domain::identity::{
    BuildHandoffIdV1, ContractComponentIdV1, ContractRootIdV1, DesignFinalizationManifestIdV1,
    IdentityError, build_handoff_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::component_kind::ContractComponentKindV1;
use super::finalization::{DesignFinalizationManifestV1, PinnedFinalizationInputV1};
use super::root::CandidateContractRootV1;

pub const BUILD_HANDOFF_VERSION_V1: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildHandoffComponentV1 {
    kind: ContractComponentKindV1,
    component_id: ContractComponentIdV1,
}

impl BuildHandoffComponentV1 {
    pub fn kind(&self) -> ContractComponentKindV1 {
        self.kind
    }

    pub fn component_id(&self) -> &ContractComponentIdV1 {
        &self.component_id
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.tag()),
            CborValue::Bytes(self.component_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Clone, Debug)]
pub struct CanonicalBuildHandoffV1 {
    finalization_manifest_id: DesignFinalizationManifestIdV1,
    candidate_contract_root_id: ContractRootIdV1,
    components: Vec<BuildHandoffComponentV1>,
    pinned_inputs: Vec<PinnedFinalizationInputV1>,
    handoff_id: BuildHandoffIdV1,
}

impl CanonicalBuildHandoffV1 {
    pub fn project(
        finalization: &DesignFinalizationManifestV1,
        candidate_contract_root: &CandidateContractRootV1,
    ) -> Result<Self, BuildHandoffError> {
        if finalization.candidate_contract_root_id() != candidate_contract_root.root_id() {
            return Err(BuildHandoffError::CandidateRootMismatch);
        }
        let components: Vec<BuildHandoffComponentV1> = candidate_contract_root
            .components()
            .iter()
            .map(|component| BuildHandoffComponentV1 {
                kind: component.kind(),
                component_id: *component.component_id(),
            })
            .collect();
        let pinned_inputs = finalization.pinned_inputs().to_vec();
        let finalization_manifest_id = *finalization.manifest_id();
        let candidate_contract_root_id = *candidate_contract_root.root_id();
        let canonical_value = handoff_value(
            &finalization_manifest_id,
            &candidate_contract_root_id,
            &components,
            &pinned_inputs,
        );
        let handoff_id = build_handoff_identity(&canonical_value)?;
        Ok(Self {
            finalization_manifest_id,
            candidate_contract_root_id,
            components,
            pinned_inputs,
            handoff_id,
        })
    }

    pub fn finalization_manifest_id(&self) -> &DesignFinalizationManifestIdV1 {
        &self.finalization_manifest_id
    }

    pub fn candidate_contract_root_id(&self) -> &ContractRootIdV1 {
        &self.candidate_contract_root_id
    }

    pub fn components(&self) -> &[BuildHandoffComponentV1] {
        &self.components
    }

    pub fn pinned_inputs(&self) -> &[PinnedFinalizationInputV1] {
        &self.pinned_inputs
    }

    pub fn handoff_id(&self) -> &BuildHandoffIdV1 {
        &self.handoff_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, BuildHandoffError> {
        Ok(deterministic_cbor::encode(&handoff_value(
            &self.finalization_manifest_id,
            &self.candidate_contract_root_id,
            &self.components,
            &self.pinned_inputs,
        ))?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BuildHandoffError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("Build Handoff projection root does not match the Finalization Manifest")]
    CandidateRootMismatch,
}

fn handoff_value(
    finalization_manifest_id: &DesignFinalizationManifestIdV1,
    candidate_contract_root_id: &ContractRootIdV1,
    components: &[BuildHandoffComponentV1],
    pinned_inputs: &[PinnedFinalizationInputV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(BUILD_HANDOFF_VERSION_V1),
        CborValue::Bytes(finalization_manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_contract_root_id.as_bytes().to_vec()),
        CborValue::Array(
            components
                .iter()
                .map(BuildHandoffComponentV1::canonical_value)
                .collect(),
        ),
        CborValue::Array(
            pinned_inputs
                .iter()
                .map(|input| {
                    CborValue::Array(vec![
                        CborValue::Unsigned(input.kind().tag()),
                        CborValue::Bytes(input.schema_id().as_bytes().to_vec()),
                        CborValue::Bytes(input.input_id().as_bytes().to_vec()),
                    ])
                })
                .collect(),
        ),
    ])
}
