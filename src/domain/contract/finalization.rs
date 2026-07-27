use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::identity::{
    ContractRootIdV1, DecisionClosureIdV1, DesignClosureRequirementIdV1,
    DesignFinalizationManifestIdV1, DesignRevisionIdV1, DesignSourceBindingIdV1,
    FinalizationInputIdV1, IdentityError, NoDesignExemptionIdV1, SchemaClosureV1, SchemaError,
    SchemaIdV1, design_finalization_manifest_identity, finalization_input_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::root::CandidateContractRootV1;

pub const DESIGN_FINALIZATION_VERSION_V1: u64 = 1;
pub const MAX_FINALIZATION_INPUTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum FinalizationInputKindV1 {
    ClosureRequirement = 1,
    DeterministicSynthesis = 2,
    ScopeAndExclusions = 3,
    CapabilityCensusAndJourneys = 4,
    MigrationRollbackRemoval = 5,
    StageProofMatrix = 6,
    ReviewEvidence = 7,
    EdgeSweepEvidence = 8,
    RiskRecovery = 9,
    FreshnessReferences = 10,
    CanonicalizationPolicy = 11,
}

impl FinalizationInputKindV1 {
    pub const ALL: [Self; 11] = [
        Self::ClosureRequirement,
        Self::DeterministicSynthesis,
        Self::ScopeAndExclusions,
        Self::CapabilityCensusAndJourneys,
        Self::MigrationRollbackRemoval,
        Self::StageProofMatrix,
        Self::ReviewEvidence,
        Self::EdgeSweepEvidence,
        Self::RiskRecovery,
        Self::FreshnessReferences,
        Self::CanonicalizationPolicy,
    ];

    pub const fn tag(self) -> u64 {
        self as u64
    }

    pub const fn schema_name(self) -> &'static str {
        match self {
            Self::ClosureRequirement => "ClosureRequirementFinalizationInputV1",
            Self::DeterministicSynthesis => "DeterministicSynthesisFinalizationInputV1",
            Self::ScopeAndExclusions => "ScopeAndExclusionsFinalizationInputV1",
            Self::CapabilityCensusAndJourneys => "CapabilityCensusAndJourneysFinalizationInputV1",
            Self::MigrationRollbackRemoval => "MigrationRollbackRemovalFinalizationInputV1",
            Self::StageProofMatrix => "StageProofMatrixFinalizationInputV1",
            Self::ReviewEvidence => "ReviewEvidenceFinalizationInputV1",
            Self::EdgeSweepEvidence => "EdgeSweepEvidenceFinalizationInputV1",
            Self::RiskRecovery => "RiskRecoveryFinalizationInputV1",
            Self::FreshnessReferences => "FreshnessReferencesFinalizationInputV1",
            Self::CanonicalizationPolicy => "CanonicalizationPolicyFinalizationInputV1",
        }
    }
}

impl TryFrom<u64> for FinalizationInputKindV1 {
    type Error = DesignFinalizationError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.tag() == value)
            .ok_or(DesignFinalizationError::UnknownInputKind(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinnedFinalizationInputV1 {
    kind: FinalizationInputKindV1,
    schema_id: SchemaIdV1,
    value: CborValue,
    input_id: FinalizationInputIdV1,
}

impl PinnedFinalizationInputV1 {
    pub fn closure_requirement(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::ClosureRequirement,
            value,
        )
    }

    pub fn deterministic_synthesis(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::DeterministicSynthesis,
            value,
        )
    }

    pub fn scope_and_exclusions(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::ScopeAndExclusions,
            value,
        )
    }

    pub fn capability_census_and_journeys(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::CapabilityCensusAndJourneys,
            value,
        )
    }

    pub fn migration_rollback_removal(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::MigrationRollbackRemoval,
            value,
        )
    }

    pub fn stage_proof_matrix(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::StageProofMatrix,
            value,
        )
    }

    pub fn review_evidence(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::ReviewEvidence,
            value,
        )
    }

    pub fn edge_sweep_evidence(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::EdgeSweepEvidence,
            value,
        )
    }

    pub fn risk_recovery(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(schema_closure, FinalizationInputKindV1::RiskRecovery, value)
    }

    pub fn freshness_references(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::FreshnessReferences,
            value,
        )
    }

    pub fn canonicalization_policy(
        schema_closure: &SchemaClosureV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        Self::from_schema_closure(
            schema_closure,
            FinalizationInputKindV1::CanonicalizationPolicy,
            value,
        )
    }

    fn from_schema_closure(
        schema_closure: &SchemaClosureV1,
        kind: FinalizationInputKindV1,
        value: CborValue,
    ) -> Result<Self, DesignFinalizationError> {
        let schema_id = *schema_closure
            .schema_id(kind.schema_name(), 1)
            .ok_or(DesignFinalizationError::MissingInputSchema(kind))?;
        schema_closure.validate_value(&schema_id, &value)?;
        let input_id =
            finalization_input_identity(&input_identity_value(kind, &schema_id, &value))?;
        Ok(Self {
            kind,
            schema_id,
            value,
            input_id,
        })
    }

    pub fn kind(&self) -> FinalizationInputKindV1 {
        self.kind
    }

    pub fn input_id(&self) -> &FinalizationInputIdV1 {
        &self.input_id
    }

    pub fn schema_id(&self) -> &SchemaIdV1 {
        &self.schema_id
    }

    pub fn value(&self) -> &CborValue {
        &self.value
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.tag()),
            CborValue::Bytes(self.schema_id.as_bytes().to_vec()),
            CborValue::Bytes(self.input_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesignBasisV1 {
    DesignRevision(DesignRevisionIdV1),
    AuthorizedNoDesign {
        closure_requirement_id: DesignClosureRequirementIdV1,
        exemption_id: NoDesignExemptionIdV1,
        source_binding_id: DesignSourceBindingIdV1,
    },
}

impl DesignBasisV1 {
    pub fn design_revision(design_revision_id: DesignRevisionIdV1) -> Self {
        Self::DesignRevision(design_revision_id)
    }

    pub fn authorized_no_design(
        closure_requirement_id: DesignClosureRequirementIdV1,
        exemption_id: NoDesignExemptionIdV1,
        source_binding_id: DesignSourceBindingIdV1,
    ) -> Self {
        Self::AuthorizedNoDesign {
            closure_requirement_id,
            exemption_id,
            source_binding_id,
        }
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        match self {
            Self::DesignRevision(design_revision_id) => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(design_revision_id.as_bytes().to_vec()),
            ]),
            Self::AuthorizedNoDesign {
                closure_requirement_id,
                exemption_id,
                source_binding_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Bytes(closure_requirement_id.as_bytes().to_vec()),
                CborValue::Bytes(exemption_id.as_bytes().to_vec()),
                CborValue::Bytes(source_binding_id.as_bytes().to_vec()),
            ]),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DesignFinalizationManifestV1 {
    design_basis: DesignBasisV1,
    decision_closure_id: DecisionClosureIdV1,
    candidate_contract_root_id: ContractRootIdV1,
    pinned_inputs: Vec<PinnedFinalizationInputV1>,
    manifest_id: DesignFinalizationManifestIdV1,
}

impl DesignFinalizationManifestV1 {
    pub fn new(
        schema_closure: &SchemaClosureV1,
        design_basis: DesignBasisV1,
        decision_closure_id: DecisionClosureIdV1,
        candidate_contract_root: &CandidateContractRootV1,
        mut pinned_inputs: Vec<PinnedFinalizationInputV1>,
    ) -> Result<Self, DesignFinalizationError> {
        if pinned_inputs.len() > MAX_FINALIZATION_INPUTS {
            return Err(DesignFinalizationError::TooManyInputs);
        }
        for input in &pinned_inputs {
            let expected_schema_id = schema_closure
                .schema_id(input.kind.schema_name(), 1)
                .ok_or(DesignFinalizationError::MissingInputSchema(input.kind))?;
            if expected_schema_id != &input.schema_id {
                return Err(DesignFinalizationError::InputSchemaMismatch(input.kind));
            }
            schema_closure.validate_value(&input.schema_id, &input.value)?;
            let recomputed = finalization_input_identity(&input_identity_value(
                input.kind,
                &input.schema_id,
                &input.value,
            ))?;
            if recomputed != input.input_id {
                return Err(DesignFinalizationError::InputIdentityMismatch(input.kind));
            }
        }
        pinned_inputs.sort_by_key(PinnedFinalizationInputV1::kind);
        validate_complete_inputs(&pinned_inputs)?;
        let candidate_contract_root_id = *candidate_contract_root.root_id();
        let canonical_value = finalization_value(
            &design_basis,
            &decision_closure_id,
            &candidate_contract_root_id,
            &pinned_inputs,
        );
        let manifest_id = design_finalization_manifest_identity(&canonical_value)?;
        Ok(Self {
            design_basis,
            decision_closure_id,
            candidate_contract_root_id,
            pinned_inputs,
            manifest_id,
        })
    }

    pub fn design_basis(&self) -> &DesignBasisV1 {
        &self.design_basis
    }

    pub fn decision_closure_id(&self) -> &DecisionClosureIdV1 {
        &self.decision_closure_id
    }

    pub fn candidate_contract_root_id(&self) -> &ContractRootIdV1 {
        &self.candidate_contract_root_id
    }

    pub fn pinned_inputs(&self) -> &[PinnedFinalizationInputV1] {
        &self.pinned_inputs
    }

    pub fn manifest_id(&self) -> &DesignFinalizationManifestIdV1 {
        &self.manifest_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DesignFinalizationError> {
        Ok(deterministic_cbor::encode(&finalization_value(
            &self.design_basis,
            &self.decision_closure_id,
            &self.candidate_contract_root_id,
            &self.pinned_inputs,
        ))?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DesignFinalizationError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("Design Finalization Manifest exceeds the finite v1 input limit")]
    TooManyInputs,
    #[error("Design Finalization Manifest is missing input kind {0:?}")]
    MissingInputKind(FinalizationInputKindV1),
    #[error("Design Finalization Manifest repeats input kind {0:?}")]
    DuplicateInputKind(FinalizationInputKindV1),
    #[error("unknown Design Finalization input kind tag {0}")]
    UnknownInputKind(u64),
    #[error("SchemaClosureV1 is missing the exact schema for finalization input {0:?}")]
    MissingInputSchema(FinalizationInputKindV1),
    #[error("finalization input {0:?} is bound to the wrong SchemaIdV1")]
    InputSchemaMismatch(FinalizationInputKindV1),
    #[error("finalization input {0:?} identity does not match its schema-bound value")]
    InputIdentityMismatch(FinalizationInputKindV1),
}

fn validate_complete_inputs(
    inputs: &[PinnedFinalizationInputV1],
) -> Result<(), DesignFinalizationError> {
    let mut present = BTreeSet::new();
    for input in inputs {
        if !present.insert(input.kind) {
            return Err(DesignFinalizationError::DuplicateInputKind(input.kind));
        }
    }
    for required in FinalizationInputKindV1::ALL {
        if !present.contains(&required) {
            return Err(DesignFinalizationError::MissingInputKind(required));
        }
    }
    Ok(())
}

fn finalization_value(
    design_basis: &DesignBasisV1,
    decision_closure_id: &DecisionClosureIdV1,
    candidate_contract_root_id: &ContractRootIdV1,
    pinned_inputs: &[PinnedFinalizationInputV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DESIGN_FINALIZATION_VERSION_V1),
        design_basis.canonical_value(),
        CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_contract_root_id.as_bytes().to_vec()),
        CborValue::Array(
            pinned_inputs
                .iter()
                .map(PinnedFinalizationInputV1::canonical_value)
                .collect(),
        ),
    ])
}

fn input_identity_value(
    kind: FinalizationInputKindV1,
    schema_id: &SchemaIdV1,
    value: &CborValue,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(kind.tag()),
        CborValue::Bytes(schema_id.as_bytes().to_vec()),
        value.clone(),
    ])
}
