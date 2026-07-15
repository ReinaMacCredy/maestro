use thiserror::Error;

use crate::domain::vnext::identity::{
    DecisionMaterializationIdV1, DecisionResolutionIdV1, DesignClosureRequirementIdV1,
    DesignRevisionIdV1, DesignSourceBindingIdV1, NoDesignExemptionIdV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

pub const MAX_DESIGN_SLOT_TAG: u64 = 65_535;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentProvenanceV1 {
    DesignSlot(DesignSlotProvenanceV1),
    AuthorizedNoDesign(AuthorizedNoDesignProvenanceV1),
    DecisionMaterialization(DecisionMaterializationProvenanceV1),
}

impl ComponentProvenanceV1 {
    pub fn design_slot(
        design_revision_id: DesignRevisionIdV1,
        slot_tag: u64,
        source_binding_id: DesignSourceBindingIdV1,
    ) -> Result<Self, ProvenanceError> {
        if slot_tag == 0 || slot_tag > MAX_DESIGN_SLOT_TAG {
            return Err(ProvenanceError::InvalidDesignSlotTag);
        }
        Ok(Self::DesignSlot(DesignSlotProvenanceV1 {
            design_revision_id,
            slot_tag,
            source_binding_id,
        }))
    }

    pub fn authorized_no_design(
        closure_requirement_id: DesignClosureRequirementIdV1,
        exemption_id: NoDesignExemptionIdV1,
        source_binding_id: DesignSourceBindingIdV1,
    ) -> Self {
        Self::AuthorizedNoDesign(AuthorizedNoDesignProvenanceV1 {
            closure_requirement_id,
            exemption_id,
            source_binding_id,
        })
    }

    pub fn decision_materialization(
        resolution_id: DecisionResolutionIdV1,
        materialization_id: DecisionMaterializationIdV1,
    ) -> Self {
        Self::DecisionMaterialization(DecisionMaterializationProvenanceV1 {
            resolution_id,
            materialization_id,
        })
    }

    pub const fn variant_tag(&self) -> u64 {
        match self {
            Self::DesignSlot(_) => 1,
            Self::AuthorizedNoDesign(_) => 2,
            Self::DecisionMaterialization(_) => 3,
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::DesignSlot(value) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(value.design_revision_id.as_bytes().to_vec()),
                CborValue::Unsigned(value.slot_tag),
                CborValue::Bytes(value.source_binding_id.as_bytes().to_vec()),
            ]),
            Self::AuthorizedNoDesign(value) => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Bytes(value.closure_requirement_id.as_bytes().to_vec()),
                CborValue::Bytes(value.exemption_id.as_bytes().to_vec()),
                CborValue::Bytes(value.source_binding_id.as_bytes().to_vec()),
            ]),
            Self::DecisionMaterialization(value) => CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Bytes(value.resolution_id.as_bytes().to_vec()),
                CborValue::Bytes(value.materialization_id.as_bytes().to_vec()),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignSlotProvenanceV1 {
    design_revision_id: DesignRevisionIdV1,
    slot_tag: u64,
    source_binding_id: DesignSourceBindingIdV1,
}

impl DesignSlotProvenanceV1 {
    pub fn design_revision_id(&self) -> &DesignRevisionIdV1 {
        &self.design_revision_id
    }

    pub fn slot_tag(&self) -> u64 {
        self.slot_tag
    }

    pub fn source_binding_id(&self) -> &DesignSourceBindingIdV1 {
        &self.source_binding_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedNoDesignProvenanceV1 {
    closure_requirement_id: DesignClosureRequirementIdV1,
    exemption_id: NoDesignExemptionIdV1,
    source_binding_id: DesignSourceBindingIdV1,
}

impl AuthorizedNoDesignProvenanceV1 {
    pub fn closure_requirement_id(&self) -> &DesignClosureRequirementIdV1 {
        &self.closure_requirement_id
    }

    pub fn exemption_id(&self) -> &NoDesignExemptionIdV1 {
        &self.exemption_id
    }

    pub fn source_binding_id(&self) -> &DesignSourceBindingIdV1 {
        &self.source_binding_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionMaterializationProvenanceV1 {
    resolution_id: DecisionResolutionIdV1,
    materialization_id: DecisionMaterializationIdV1,
}

impl DecisionMaterializationProvenanceV1 {
    pub fn resolution_id(&self) -> &DecisionResolutionIdV1 {
        &self.resolution_id
    }

    pub fn materialization_id(&self) -> &DecisionMaterializationIdV1 {
        &self.materialization_id
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProvenanceError {
    #[error("Design slot tag must be positive and within the finite v1 bound")]
    InvalidDesignSlotTag,
}
