use thiserror::Error;

use crate::domain::identity::{
    DecisionMaterializationIdV1, DecisionResolutionIdV1, DesignClosureRequirementIdV1,
    DesignRevisionIdV1, DesignSourceBindingIdV1, NoDesignExemptionIdV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::materialization::DecisionMaterializationPreimageCommitmentV1;

pub const MAX_DESIGN_SLOT_TAG: u64 = 65_535;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentProvenanceV1 {
    DesignSlot(DesignSlotProvenanceV1),
    AuthorizedNoDesign(AuthorizedNoDesignProvenanceV1),
    DecisionMaterialization(DecisionMaterializationProvenanceV1),
    DecisionMaterializationPreimage(DecisionMaterializationPreimageProvenanceV1),
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

    pub(crate) fn decision_materialization(
        resolution_id: DecisionResolutionIdV1,
        materialization_id: DecisionMaterializationIdV1,
    ) -> Self {
        Self::DecisionMaterialization(DecisionMaterializationProvenanceV1 {
            resolution_id,
            materialization_id,
        })
    }

    pub(crate) fn decision_materialization_preimage(
        resolution_id: DecisionResolutionIdV1,
        commitment: DecisionMaterializationPreimageCommitmentV1,
    ) -> Self {
        Self::DecisionMaterializationPreimage(DecisionMaterializationPreimageProvenanceV1 {
            resolution_id,
            commitment,
        })
    }

    pub const fn variant_tag(&self) -> u64 {
        match self {
            Self::DesignSlot(_) => 1,
            Self::AuthorizedNoDesign(_) => 2,
            Self::DecisionMaterialization(_) => 3,
            Self::DecisionMaterializationPreimage(_) => 4,
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
            Self::DecisionMaterializationPreimage(value) => CborValue::Array(vec![
                CborValue::Unsigned(4),
                CborValue::Bytes(value.resolution_id.as_bytes().to_vec()),
                CborValue::Bytes(value.commitment.as_bytes().to_vec()),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionMaterializationPreimageProvenanceV1 {
    resolution_id: DecisionResolutionIdV1,
    commitment: DecisionMaterializationPreimageCommitmentV1,
}

impl DecisionMaterializationPreimageProvenanceV1 {
    pub fn resolution_id(&self) -> &DecisionResolutionIdV1 {
        &self.resolution_id
    }

    pub const fn commitment(&self) -> DecisionMaterializationPreimageCommitmentV1 {
        self.commitment
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contract::assembly::{
        candidate_root_schema_closure_v1, facet_schema_id_v1, fixture_facet_value_v1,
    };
    use crate::domain::contract::component::CandidateContractComponentV1;
    use crate::domain::contract::component_kind::ContractComponentKindV1;
    use crate::domain::identity::{
        decision_materialization_identity, decision_resolution_identity,
        design_closure_requirement_identity, design_revision_identity,
        design_source_binding_identity, no_design_exemption_identity,
    };

    #[test]
    fn decision_materialization_tag_three_remains_identity_bearing_but_not_publicly_mintable() {
        let schemas = candidate_root_schema_closure_v1().expect("candidate root schemas");
        let kind = ContractComponentKindV1::IntendedOutcome;
        let schema_id = facet_schema_id_v1(&schemas, kind).expect("facet schema");
        let value = fixture_facet_value_v1(kind, [7; 32], vec![[8; 32]]);
        let design_revision_id =
            design_revision_identity(&CborValue::Unsigned(1)).expect("design revision");
        let source_binding_id =
            design_source_binding_identity(&CborValue::Unsigned(2)).expect("source binding");
        let closure_requirement_id = design_closure_requirement_identity(&CborValue::Unsigned(3))
            .expect("closure requirement");
        let exemption_id =
            no_design_exemption_identity(&CborValue::Unsigned(4)).expect("no-design exemption");
        let resolution_id =
            decision_resolution_identity(&CborValue::Unsigned(5)).expect("Decision resolution");
        let materialization_id = decision_materialization_identity(&CborValue::Unsigned(6))
            .expect("Decision materialization");

        let provenance = [
            ComponentProvenanceV1::design_slot(design_revision_id, kind.tag(), source_binding_id)
                .expect("design provenance"),
            ComponentProvenanceV1::authorized_no_design(
                closure_requirement_id,
                exemption_id,
                source_binding_id,
            ),
            ComponentProvenanceV1::decision_materialization(resolution_id, materialization_id),
        ];
        assert_eq!(provenance[2].variant_tag(), 3);
        assert_eq!(
            provenance[2].canonical_value(),
            CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Bytes(resolution_id.as_bytes().to_vec()),
                CborValue::Bytes(materialization_id.as_bytes().to_vec()),
            ])
        );

        let component_ids = provenance.map(|provenance| {
            *CandidateContractComponentV1::new(
                &schemas,
                kind,
                schema_id,
                value.clone(),
                vec![],
                provenance,
            )
            .expect("candidate component")
            .component_id()
        });
        assert_ne!(component_ids[0], component_ids[1]);
        assert_ne!(component_ids[0], component_ids[2]);
        assert_ne!(component_ids[1], component_ids[2]);
    }
}
