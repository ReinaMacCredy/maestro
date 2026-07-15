use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::contract::component_kind::ContractComponentKindV1;
use crate::domain::vnext::identity::{
    ContractComponentIdV1, ContractRootIdV1, DecisionClosureIdV1, DecisionMaterializationIdV1,
    DesignFinalizationManifestIdV1, IdentityError, decision_closure_identity,
    decision_materialization_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::finalization::DesignFinalizationManifestV1;
use super::materialization::{DecisionMaterializationResolutionV1, MaterializationBaseV1};
use super::provenance::ComponentProvenanceV1;
use super::root::CandidateContractRootV1;

pub const EXTERNAL_DESIGN_AUTHORITY_CLOSURE_VERSION_V1: u64 = 1;
pub const DECISION_CLOSURE_VERSION_V1: u64 = 1;
pub const EXTERNAL_DESIGN_AUTHORITY_CLOSURE_DOMAIN_V1: &str =
    "maestro.vnext.external-design-authority-closure.v1";
pub const MAX_EXTERNAL_DECISION_RECORDS_V1: usize = 65_536;
pub const MAX_DECISION_MATERIALIZATIONS_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum TerminalDecisionStatusV1 {
    Locked = 1,
    Superseded = 2,
}

impl TerminalDecisionStatusV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::Superseded => "superseded",
        }
    }
}

impl TryFrom<&str> for TerminalDecisionStatusV1 {
    type Error = DecisionClosureError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "locked" => Ok(Self::Locked),
            "superseded" => Ok(Self::Superseded),
            "open" => Err(DecisionClosureError::OpenDecision),
            _ => Err(DecisionClosureError::InvalidTerminalStatus),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalLineageDispositionV1 {
    None,
    OneToOne { successor: String },
    CompositeExternalAuthoring,
    UnilateralRawClaim,
    ExternalHead,
}

impl ExternalLineageDispositionV1 {
    pub fn from_literals(
        disposition: &str,
        normalized_successor: Option<String>,
    ) -> Result<Self, DecisionClosureError> {
        match (disposition, normalized_successor) {
            ("none", None) => Ok(Self::None),
            ("one_to_one", Some(successor)) => Ok(Self::OneToOne { successor }),
            ("composite_external_authoring", None) => Ok(Self::CompositeExternalAuthoring),
            ("unilateral_raw_claim", None) => Ok(Self::UnilateralRawClaim),
            ("external_head", None) => Ok(Self::ExternalHead),
            _ => Err(DecisionClosureError::InvalidLineageDisposition),
        }
    }

    fn normalized_successor(&self) -> Option<&str> {
        match self {
            Self::OneToOne { successor } => Some(successor),
            Self::None
            | Self::CompositeExternalAuthoring
            | Self::UnilateralRawClaim
            | Self::ExternalHead => None,
        }
    }

    const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::OneToOne { .. } => "one_to_one",
            Self::CompositeExternalAuthoring => "composite_external_authoring",
            Self::UnilateralRawClaim => "unilateral_raw_claim",
            Self::ExternalHead => "external_head",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionConsequenceClassificationV1 {
    RationaleOnly { disposition: String },
    Material,
}

impl DecisionConsequenceClassificationV1 {
    pub fn from_literals(
        classification: &str,
        rationale_disposition: Option<String>,
    ) -> Result<Self, DecisionClosureError> {
        match (classification, rationale_disposition) {
            ("material", None) => Ok(Self::Material),
            ("rationale_only", Some(disposition)) if !disposition.is_empty() => {
                Ok(Self::RationaleOnly { disposition })
            }
            _ => Err(DecisionClosureError::InvalidConsequenceClassification),
        }
    }

    const fn as_str(&self) -> &'static str {
        match self {
            Self::RationaleOnly { .. } => "rationale_only",
            Self::Material => "material",
        }
    }

    fn rationale_disposition(&self) -> Option<&str> {
        match self {
            Self::RationaleOnly { disposition } => Some(disposition),
            Self::Material => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum DerivedDecisionEffectStatusV1 {
    NoContractEffect = 1,
    Unapplied = 2,
    SupersededButEffectLive = 3,
}

impl DerivedDecisionEffectStatusV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NoContractEffect => "no_contract_effect",
            Self::Unapplied => "unapplied",
            Self::SupersededButEffectLive => "superseded_but_effect_live",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawExternalDecisionRecordV1 {
    id: String,
    terminal_status: TerminalDecisionStatusV1,
    raw_record: Vec<u8>,
    raw_supersedes: Vec<String>,
    raw_superseded_by: Vec<String>,
    raw_record_hash: [u8; 32],
    raw_body_hash: [u8; 32],
}

impl RawExternalDecisionRecordV1 {
    pub fn new(
        id: impl Into<String>,
        terminal_status: TerminalDecisionStatusV1,
        raw_record: Vec<u8>,
        raw_body: Vec<u8>,
        raw_supersedes: Vec<String>,
        raw_superseded_by: Vec<String>,
    ) -> Result<Self, DecisionClosureError> {
        let id = id.into();
        validate_ascii_identifier(&id)?;
        for predecessor in &raw_supersedes {
            validate_ascii_identifier(predecessor)?;
        }
        for successor in &raw_superseded_by {
            validate_ascii_identifier(successor)?;
        }
        if terminal_status == TerminalDecisionStatusV1::Superseded && raw_superseded_by.is_empty() {
            return Err(DecisionClosureError::RawLineageOmission);
        }
        Ok(Self {
            id,
            terminal_status,
            raw_record_hash: sha256(&raw_record),
            raw_body_hash: sha256(&raw_body),
            raw_record,
            raw_supersedes,
            raw_superseded_by,
        })
    }

    pub fn from_committed_hashes(
        id: impl Into<String>,
        terminal_status: TerminalDecisionStatusV1,
        raw_record: Vec<u8>,
        raw_record_hash: [u8; 32],
        raw_body_hash: [u8; 32],
        raw_supersedes: Vec<String>,
        raw_superseded_by: Vec<String>,
    ) -> Result<Self, DecisionClosureError> {
        let id = id.into();
        validate_ascii_identifier(&id)?;
        for predecessor in &raw_supersedes {
            validate_ascii_identifier(predecessor)?;
        }
        for successor in &raw_superseded_by {
            validate_ascii_identifier(successor)?;
        }
        if terminal_status == TerminalDecisionStatusV1::Superseded && raw_superseded_by.is_empty() {
            return Err(DecisionClosureError::RawLineageOmission);
        }
        if sha256(&raw_record) != raw_record_hash {
            return Err(DecisionClosureError::RawRecordHashMismatch);
        }
        Ok(Self {
            id,
            terminal_status,
            raw_record,
            raw_supersedes,
            raw_superseded_by,
            raw_record_hash,
            raw_body_hash,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn terminal_status(&self) -> TerminalDecisionStatusV1 {
        self.terminal_status
    }

    pub fn raw_record(&self) -> &[u8] {
        &self.raw_record
    }

    pub const fn raw_record_hash(&self) -> &[u8; 32] {
        &self.raw_record_hash
    }

    pub const fn raw_body_hash(&self) -> &[u8; 32] {
        &self.raw_body_hash
    }

    pub fn raw_supersedes(&self) -> &[String] {
        &self.raw_supersedes
    }

    pub fn raw_superseded_by(&self) -> &[String] {
        &self.raw_superseded_by
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalDecisionClosureRecordV1 {
    raw: RawExternalDecisionRecordV1,
    lineage: ExternalLineageDispositionV1,
    consequence: DecisionConsequenceClassificationV1,
}

impl ExternalDecisionClosureRecordV1 {
    pub fn new(
        raw: RawExternalDecisionRecordV1,
        lineage: ExternalLineageDispositionV1,
        consequence: DecisionConsequenceClassificationV1,
    ) -> Result<Self, DecisionClosureError> {
        if let ExternalLineageDispositionV1::OneToOne { successor } = &lineage {
            validate_ascii_identifier(successor)?;
        }
        if let DecisionConsequenceClassificationV1::RationaleOnly { disposition } = &consequence {
            validate_ascii_identifier(disposition)?;
        }
        Ok(Self {
            raw,
            lineage,
            consequence,
        })
    }

    pub fn raw(&self) -> &RawExternalDecisionRecordV1 {
        &self.raw
    }

    pub fn lineage(&self) -> &ExternalLineageDispositionV1 {
        &self.lineage
    }

    pub fn consequence(&self) -> &DecisionConsequenceClassificationV1 {
        &self.consequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionMaterializationSourceV1 {
    decision_id: String,
    raw_body_hash: [u8; 32],
}

impl DecisionMaterializationSourceV1 {
    pub fn new(
        decision_id: impl Into<String>,
        raw_body_hash: [u8; 32],
    ) -> Result<Self, DecisionClosureError> {
        let decision_id = decision_id.into();
        validate_ascii_identifier(&decision_id)?;
        Ok(Self {
            decision_id,
            raw_body_hash,
        })
    }

    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    pub const fn raw_body_hash(&self) -> &[u8; 32] {
        &self.raw_body_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredDecisionMaterializationV1 {
    artifact_id: String,
    component_kind: ContractComponentKindV1,
    sources: Vec<DecisionMaterializationSourceV1>,
    materialization_id: DecisionMaterializationIdV1,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IgnoredUnilateralClaimV1 {
    source: String,
    claimed_predecessor: String,
}

impl IgnoredUnilateralClaimV1 {
    pub fn new(
        source: impl Into<String>,
        claimed_predecessor: impl Into<String>,
    ) -> Result<Self, DecisionClosureError> {
        let source = source.into();
        let claimed_predecessor = claimed_predecessor.into();
        validate_ascii_identifier(&source)?;
        validate_ascii_identifier(&claimed_predecessor)?;
        Ok(Self {
            source,
            claimed_predecessor,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn claimed_predecessor(&self) -> &str {
        &self.claimed_predecessor
    }

    fn canonical_value(&self) -> Result<CborValue, DecisionClosureError> {
        Ok(CborValue::Array(vec![
            CborValue::text(&self.source)?,
            CborValue::text(&self.claimed_predecessor)?,
        ]))
    }
}

impl RequiredDecisionMaterializationV1 {
    pub fn new(
        artifact_id: impl Into<String>,
        component_kind: ContractComponentKindV1,
        sources: Vec<DecisionMaterializationSourceV1>,
    ) -> Result<Self, DecisionClosureError> {
        let artifact_id = artifact_id.into();
        validate_ascii_identifier(&artifact_id)?;
        validate_strict_sources(&sources)?;
        let materialization_id = decision_materialization_identity(
            &materialization_identity_value(&artifact_id, component_kind)?,
        )?;
        Ok(Self {
            artifact_id,
            component_kind,
            sources,
            materialization_id,
        })
    }

    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    pub const fn component_kind(&self) -> ContractComponentKindV1 {
        self.component_kind
    }

    pub fn sources(&self) -> &[DecisionMaterializationSourceV1] {
        &self.sources
    }

    pub const fn materialization_id(&self) -> &DecisionMaterializationIdV1 {
        &self.materialization_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalDesignAuthorityClosureV1 {
    records: Vec<ExternalDecisionClosureRecordV1>,
    materializations: Vec<RequiredDecisionMaterializationV1>,
    ignored_unilateral_claims: Vec<IgnoredUnilateralClaimV1>,
    recognized_external_composite_heads: Vec<String>,
    external_closure_id: [u8; 32],
}

impl ExternalDesignAuthorityClosureV1 {
    pub fn new(
        records: Vec<ExternalDecisionClosureRecordV1>,
        materializations: Vec<RequiredDecisionMaterializationV1>,
        expected_record_ids: &[String],
        ignored_unilateral_claims: Vec<IgnoredUnilateralClaimV1>,
        recognized_external_composite_heads: Vec<String>,
    ) -> Result<Self, DecisionClosureError> {
        if records.len() > MAX_EXTERNAL_DECISION_RECORDS_V1 {
            return Err(DecisionClosureError::TooManyRecords);
        }
        if materializations.len() > MAX_DECISION_MATERIALIZATIONS_V1 {
            return Err(DecisionClosureError::TooManyMaterializations);
        }
        validate_expected_records(&records, expected_record_ids)?;
        validate_materializations(&records, &materializations)?;
        validate_normalized_lineage(&records)?;
        validate_ignored_unilateral_claims(&records, &ignored_unilateral_claims)?;
        validate_recognized_external_composite_heads(
            &records,
            &recognized_external_composite_heads,
        )?;
        let value = external_closure_value(
            &records,
            &materializations,
            &ignored_unilateral_claims,
            &recognized_external_composite_heads,
        )?;
        let external_closure_id = sha256(&deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text(EXTERNAL_DESIGN_AUTHORITY_CLOSURE_DOMAIN_V1)?,
            value,
        ]))?);
        Ok(Self {
            records,
            materializations,
            ignored_unilateral_claims,
            recognized_external_composite_heads,
            external_closure_id,
        })
    }

    pub fn records(&self) -> &[ExternalDecisionClosureRecordV1] {
        &self.records
    }

    pub fn materializations(&self) -> &[RequiredDecisionMaterializationV1] {
        &self.materializations
    }

    pub fn recognized_external_composite_heads(&self) -> &[String] {
        &self.recognized_external_composite_heads
    }

    pub fn ignored_unilateral_claims(&self) -> &[IgnoredUnilateralClaimV1] {
        &self.ignored_unilateral_claims
    }

    pub const fn external_closure_id(&self) -> &[u8; 32] {
        &self.external_closure_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DecisionClosureError> {
        Ok(deterministic_cbor::encode(&external_closure_value(
            &self.records,
            &self.materializations,
            &self.ignored_unilateral_claims,
            &self.recognized_external_composite_heads,
        )?)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDecisionSuccessorEdgeV1 {
    predecessor: String,
    successor: String,
}

impl CanonicalDecisionSuccessorEdgeV1 {
    pub fn predecessor(&self) -> &str {
        &self.predecessor
    }

    pub fn successor(&self) -> &str {
        &self.successor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionClosureV1 {
    records: Vec<ExternalDecisionClosureRecordV1>,
    materializations: Vec<RequiredDecisionMaterializationV1>,
    ignored_unilateral_claims: Vec<IgnoredUnilateralClaimV1>,
    normalized_successor_edges: Vec<CanonicalDecisionSuccessorEdgeV1>,
    closure_id: DecisionClosureIdV1,
}

impl DecisionClosureV1 {
    pub fn from_external(
        external: &ExternalDesignAuthorityClosureV1,
    ) -> Result<Self, DecisionClosureError> {
        let normalized_successor_edges = external
            .records
            .iter()
            .filter_map(|record| match record.lineage() {
                ExternalLineageDispositionV1::OneToOne { successor } => {
                    Some(CanonicalDecisionSuccessorEdgeV1 {
                        predecessor: record.raw().id().to_owned(),
                        successor: successor.clone(),
                    })
                }
                ExternalLineageDispositionV1::None
                | ExternalLineageDispositionV1::CompositeExternalAuthoring
                | ExternalLineageDispositionV1::UnilateralRawClaim
                | ExternalLineageDispositionV1::ExternalHead => None,
            })
            .collect();
        let value = decision_closure_value(
            &external.records,
            &external.materializations,
            &external.ignored_unilateral_claims,
        )?;
        let closure_id = decision_closure_identity(&value)?;
        Ok(Self {
            records: external.records.clone(),
            materializations: external.materializations.clone(),
            ignored_unilateral_claims: external.ignored_unilateral_claims.clone(),
            normalized_successor_edges,
            closure_id,
        })
    }

    pub fn records(&self) -> &[ExternalDecisionClosureRecordV1] {
        &self.records
    }

    pub fn materializations(&self) -> &[RequiredDecisionMaterializationV1] {
        &self.materializations
    }

    pub fn normalized_successor_edges(&self) -> &[CanonicalDecisionSuccessorEdgeV1] {
        &self.normalized_successor_edges
    }

    pub fn derived_effect_status(
        &self,
        decision_id: &str,
    ) -> Option<DerivedDecisionEffectStatusV1> {
        self.records
            .iter()
            .find(|record| record.raw().id() == decision_id)
            .map(derived_effect_status)
    }

    pub const fn closure_id(&self) -> &DecisionClosureIdV1 {
        &self.closure_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DecisionClosureError> {
        Ok(deterministic_cbor::encode(&decision_closure_value(
            &self.records,
            &self.materializations,
            &self.ignored_unilateral_claims,
        )?)?)
    }

    pub fn root_binding_requirements(&self) -> DecisionRootBindingRequirementsV1 {
        DecisionRootBindingRequirementsV1 {
            closure_id: self.closure_id,
            materializations: self.materializations.clone(),
            materialization_ids: self
                .materializations
                .iter()
                .map(|materialization| *materialization.materialization_id())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionRootBindingRequirementsV1 {
    closure_id: DecisionClosureIdV1,
    materializations: Vec<RequiredDecisionMaterializationV1>,
    materialization_ids: Vec<DecisionMaterializationIdV1>,
}

impl DecisionRootBindingRequirementsV1 {
    pub fn closure_id(&self) -> &DecisionClosureIdV1 {
        &self.closure_id
    }

    pub fn materialization_ids(&self) -> &[DecisionMaterializationIdV1] {
        &self.materialization_ids
    }

    pub fn resolve(
        &self,
        bindings: Vec<ExactDecisionRootBindingV1>,
        candidate_root: &CandidateContractRootV1,
        finalization: &DesignFinalizationManifestV1,
    ) -> Result<ResolvedDecisionRootBindingsV1, DecisionClosureError> {
        if finalization.decision_closure_id() != &self.closure_id
            || finalization.candidate_contract_root_id() != candidate_root.root_id()
        {
            return Err(DecisionClosureError::BindingFinalizationMismatch);
        }
        let expected_materializations: BTreeSet<[u8; 32]> = self
            .materialization_ids
            .iter()
            .map(|id| *id.as_bytes())
            .collect();
        let actual_materializations: BTreeSet<[u8; 32]> = bindings
            .iter()
            .map(|binding| *binding.materialization_id.as_bytes())
            .collect();
        if expected_materializations != actual_materializations
            || expected_materializations.len() != bindings.len()
        {
            return Err(DecisionClosureError::IncompleteExactRootResolution);
        }
        if bindings.windows(2).any(|pair| {
            pair[0].materialization_id.as_bytes() >= pair[1].materialization_id.as_bytes()
        }) {
            return Err(DecisionClosureError::BindingsNotStrictlySorted);
        }
        let normative_components = candidate_root
            .components()
            .iter()
            .filter(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
            .collect::<Vec<_>>();
        if normative_components.len() != self.materializations.len() {
            return Err(DecisionClosureError::NormativeComponentSetMismatch);
        }
        let required_by_id = self
            .materializations
            .iter()
            .map(|materialization| {
                (
                    *materialization.materialization_id().as_bytes(),
                    materialization,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected_base = MaterializationBaseV1::initial_external_design_closure(self.closure_id);
        let mut expected_components = BTreeMap::new();
        for component in normative_components {
            let provenance = match component.provenance() {
                ComponentProvenanceV1::DecisionMaterialization(provenance) => provenance,
                ComponentProvenanceV1::DesignSlot(_)
                | ComponentProvenanceV1::AuthorizedNoDesign(_)
                | ComponentProvenanceV1::DecisionMaterializationPreimage(_) => {
                    return Err(DecisionClosureError::NormativeComponentProvenanceMismatch);
                }
            };
            if expected_components.contains_key(provenance.materialization_id().as_bytes()) {
                return Err(DecisionClosureError::DuplicateNormativeMaterialization);
            }
            let materialization = required_by_id
                .get(provenance.materialization_id().as_bytes())
                .ok_or(DecisionClosureError::NormativeComponentSetMismatch)?;
            let resolution = DecisionMaterializationResolutionV1::new(
                self.closure_id,
                expected_base.clone(),
                *materialization.materialization_id(),
            )
            .map_err(|_| DecisionClosureError::NormativeResolutionMismatch)?;
            if resolution.resolution_id() != provenance.resolution_id() {
                return Err(DecisionClosureError::NormativeResolutionMismatch);
            }
            if component.value() != &normative_inputs_value(materialization)? {
                return Err(DecisionClosureError::NormativeComponentValueMismatch);
            }
            expected_components.insert(
                *provenance.materialization_id().as_bytes(),
                *component.component_id().as_bytes(),
            );
        }
        if expected_components.len() != expected_materializations.len()
            || expected_components.keys().copied().collect::<BTreeSet<_>>()
                != expected_materializations
        {
            return Err(DecisionClosureError::NormativeComponentSetMismatch);
        }
        for binding in &bindings {
            if expected_components.get(binding.materialization_id.as_bytes())
                != Some(binding.component_id.as_bytes())
            {
                return Err(DecisionClosureError::NormativeComponentSetMismatch);
            }
            if binding.materialization_base != expected_base {
                return Err(DecisionClosureError::BindingMaterializationBaseMismatch);
            }
            if binding.after_root_id != *candidate_root.root_id()
                || binding.finalization_manifest_id != *finalization.manifest_id()
            {
                return Err(DecisionClosureError::BindingFinalizationMismatch);
            }
        }
        Ok(ResolvedDecisionRootBindingsV1 {
            closure_id: self.closure_id,
            bindings,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDecisionRootBindingV1 {
    materialization_id: DecisionMaterializationIdV1,
    component_id: ContractComponentIdV1,
    materialization_base: MaterializationBaseV1,
    after_root_id: ContractRootIdV1,
    finalization_manifest_id: DesignFinalizationManifestIdV1,
}

impl ExactDecisionRootBindingV1 {
    pub fn new(
        materialization_id: DecisionMaterializationIdV1,
        component_id: ContractComponentIdV1,
        materialization_base: MaterializationBaseV1,
        after_root_id: ContractRootIdV1,
        finalization_manifest_id: DesignFinalizationManifestIdV1,
    ) -> Self {
        Self {
            materialization_id,
            component_id,
            materialization_base,
            after_root_id,
            finalization_manifest_id,
        }
    }

    pub fn materialization_id(&self) -> &DecisionMaterializationIdV1 {
        &self.materialization_id
    }

    pub fn component_id(&self) -> &ContractComponentIdV1 {
        &self.component_id
    }

    pub fn materialization_base(&self) -> &MaterializationBaseV1 {
        &self.materialization_base
    }

    pub fn after_root_id(&self) -> &ContractRootIdV1 {
        &self.after_root_id
    }

    pub fn finalization_manifest_id(&self) -> &DesignFinalizationManifestIdV1 {
        &self.finalization_manifest_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDecisionRootBindingsV1 {
    closure_id: DecisionClosureIdV1,
    bindings: Vec<ExactDecisionRootBindingV1>,
}

impl ResolvedDecisionRootBindingsV1 {
    pub fn closure_id(&self) -> &DecisionClosureIdV1 {
        &self.closure_id
    }

    pub fn bindings(&self) -> &[ExactDecisionRootBindingV1] {
        &self.bindings
    }
}

#[derive(Debug, Error)]
pub enum DecisionClosureError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("Decision closure identifiers and dispositions must be ASCII and nonempty")]
    InvalidIdentifier,
    #[error("a vNext Decision closure cannot contain an open Decision")]
    OpenDecision,
    #[error("Decision terminal status is invalid")]
    InvalidTerminalStatus,
    #[error("external Decision lineage disposition and normalized successor are inconsistent")]
    InvalidLineageDisposition,
    #[error("Decision consequence classification and rationale disposition are inconsistent")]
    InvalidConsequenceClassification,
    #[error("a superseded external Decision omitted its raw superseded_by lineage")]
    RawLineageOmission,
    #[error("external Decision raw bytes do not match their committed record hash")]
    RawRecordHashMismatch,
    #[error("Decision closure record input is not strictly sorted by id")]
    RecordsNotStrictlySorted,
    #[error("Decision closure omitted or duplicated an expected Decision id")]
    ExpectedRecordSetMismatch,
    #[error("Decision closure references an unknown raw Decision id")]
    UnknownRawDecisionId,
    #[error("a normalized one-to-one successor does not match both raw directions")]
    InvalidOneToOneLineage,
    #[error(
        "a composite external authoring head cannot be promoted to a canonical vNext supersession"
    )]
    CompositePromotion,
    #[error("a unilateral raw claim cannot be repaired into a canonical vNext supersession")]
    UnilateralRepair,
    #[error("normalized canonical Decision successor edges are cyclic")]
    NormalizedSuccessorCycle,
    #[error("Decision materializations exceed the finite v1 limit")]
    TooManyMaterializations,
    #[error("Decision records exceed the finite v1 limit")]
    TooManyRecords,
    #[error("Decision materialization sources must be strictly sorted by Decision id")]
    MaterializationSourcesNotStrictlySorted,
    #[error("Decision materialization source body hash is stale")]
    StaleMaterialization,
    #[error("a material Decision omitted an explicit component-slot materialization")]
    MissingMaterialization,
    #[error(
        "a rationale-only Decision must have a rationale disposition and no component materialization"
    )]
    InvalidRationaleDisposition,
    #[error("Decision materializations must be strictly sorted by artifact id")]
    MaterializationsNotStrictlySorted,
    #[error("Decision materialization identities must be unique")]
    DuplicateMaterializationIdentity,
    #[error("a Decision may belong to exactly one materialization source set")]
    DuplicateDecisionMaterializationSource,
    #[error("ignored unilateral claims do not equal the raw asymmetric lineage set")]
    IgnoredUnilateralClaimSetMismatch,
    #[error("recognized external composite heads must be strictly sorted and unique")]
    RecognizedCompositeHeadsNotStrictlySorted,
    #[error("a recognized external composite head is not a raw multi-predecessor head")]
    UnknownRecognizedCompositeHead,
    #[error(
        "exact component/root/finalization bindings are required before Decision effects are resolved"
    )]
    IncompleteExactRootResolution,
    #[error("a NormativeInputs component is not owned by Decision materialization provenance")]
    NormativeComponentProvenanceMismatch,
    #[error("multiple NormativeInputs components claim the same Decision materialization")]
    DuplicateNormativeMaterialization,
    #[error("NormativeInputs Decision materialization resolution identity is stale or fabricated")]
    NormativeResolutionMismatch,
    #[error("NormativeInputs value does not equal the frozen materialization and source rows")]
    NormativeComponentValueMismatch,
    #[error("Decision-root bindings do not equal the candidate root NormativeInputs components")]
    NormativeComponentSetMismatch,
    #[error("Decision-root bindings must be strictly sorted by materialization identity")]
    BindingsNotStrictlySorted,
    #[error("a Decision-root binding does not use the frozen initial materialization base")]
    BindingMaterializationBaseMismatch,
    #[error("a Decision-root binding does not match the exact root and finalization manifest")]
    BindingFinalizationMismatch,
}

fn validate_ascii_identifier(value: &str) -> Result<(), DecisionClosureError> {
    if value.is_empty() || !value.is_ascii() {
        return Err(DecisionClosureError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_strict_sources(
    sources: &[DecisionMaterializationSourceV1],
) -> Result<(), DecisionClosureError> {
    let ids: Vec<&str> = sources
        .iter()
        .map(DecisionMaterializationSourceV1::decision_id)
        .collect();
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DecisionClosureError::MaterializationSourcesNotStrictlySorted);
    }
    Ok(())
}

fn validate_expected_records(
    records: &[ExternalDecisionClosureRecordV1],
    expected_record_ids: &[String],
) -> Result<(), DecisionClosureError> {
    let actual: Vec<&str> = records.iter().map(|record| record.raw().id()).collect();
    if actual.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DecisionClosureError::RecordsNotStrictlySorted);
    }
    let expected: Vec<&str> = expected_record_ids.iter().map(String::as_str).collect();
    if expected.windows(2).any(|pair| pair[0] >= pair[1]) || actual != expected {
        return Err(DecisionClosureError::ExpectedRecordSetMismatch);
    }
    Ok(())
}

fn validate_normalized_lineage(
    records: &[ExternalDecisionClosureRecordV1],
) -> Result<(), DecisionClosureError> {
    let by_id: BTreeMap<&str, &ExternalDecisionClosureRecordV1> = records
        .iter()
        .map(|record| (record.raw().id(), record))
        .collect();
    for record in records {
        for raw_id in record
            .raw()
            .raw_supersedes()
            .iter()
            .chain(record.raw().raw_superseded_by())
        {
            if !by_id.contains_key(raw_id.as_str()) {
                return Err(DecisionClosureError::UnknownRawDecisionId);
            }
        }
        match record.lineage() {
            ExternalLineageDispositionV1::OneToOne { successor } => {
                let successor_record = by_id
                    .get(successor.as_str())
                    .ok_or(DecisionClosureError::UnknownRawDecisionId)?;
                if record.raw().raw_superseded_by() != [successor.clone()]
                    || successor_record.raw().raw_supersedes() != [record.raw().id().to_owned()]
                {
                    return Err(DecisionClosureError::InvalidOneToOneLineage);
                }
            }
            ExternalLineageDispositionV1::CompositeExternalAuthoring => {
                if record.lineage().normalized_successor().is_some() {
                    return Err(DecisionClosureError::CompositePromotion);
                }
            }
            ExternalLineageDispositionV1::UnilateralRawClaim => {
                if record.lineage().normalized_successor().is_some() {
                    return Err(DecisionClosureError::UnilateralRepair);
                }
            }
            ExternalLineageDispositionV1::None | ExternalLineageDispositionV1::ExternalHead => {}
        }
    }
    for record in records {
        let mut visited = BTreeSet::new();
        let mut current = Some(record.raw().id());
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(DecisionClosureError::NormalizedSuccessorCycle);
            }
            current = by_id
                .get(id)
                .and_then(|item| item.lineage().normalized_successor());
        }
    }
    Ok(())
}

fn validate_materializations(
    records: &[ExternalDecisionClosureRecordV1],
    materializations: &[RequiredDecisionMaterializationV1],
) -> Result<(), DecisionClosureError> {
    let artifact_ids: Vec<&str> = materializations
        .iter()
        .map(RequiredDecisionMaterializationV1::artifact_id)
        .collect();
    if artifact_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DecisionClosureError::MaterializationsNotStrictlySorted);
    }
    let unique_materialization_ids = materializations
        .iter()
        .map(|materialization| *materialization.materialization_id().as_bytes())
        .collect::<BTreeSet<_>>();
    if unique_materialization_ids.len() != materializations.len() {
        return Err(DecisionClosureError::DuplicateMaterializationIdentity);
    }
    let records_by_id: BTreeMap<&str, &ExternalDecisionClosureRecordV1> = records
        .iter()
        .map(|record| (record.raw().id(), record))
        .collect();
    let mut materialized = BTreeSet::new();
    for materialization in materializations {
        for source in materialization.sources() {
            let record = records_by_id
                .get(source.decision_id())
                .ok_or(DecisionClosureError::UnknownRawDecisionId)?;
            if source.raw_body_hash() != record.raw().raw_body_hash() {
                return Err(DecisionClosureError::StaleMaterialization);
            }
            if !materialized.insert(source.decision_id()) {
                return Err(DecisionClosureError::DuplicateDecisionMaterializationSource);
            }
        }
    }
    for record in records {
        match record.consequence() {
            DecisionConsequenceClassificationV1::Material => {
                if !materialized.contains(record.raw().id()) {
                    return Err(DecisionClosureError::MissingMaterialization);
                }
            }
            DecisionConsequenceClassificationV1::RationaleOnly { disposition } => {
                if disposition.is_empty() || materialized.contains(record.raw().id()) {
                    return Err(DecisionClosureError::InvalidRationaleDisposition);
                }
            }
        }
    }
    Ok(())
}

fn validate_ignored_unilateral_claims(
    records: &[ExternalDecisionClosureRecordV1],
    ignored: &[IgnoredUnilateralClaimV1],
) -> Result<(), DecisionClosureError> {
    let by_id: BTreeMap<&str, &ExternalDecisionClosureRecordV1> = records
        .iter()
        .map(|record| (record.raw().id(), record))
        .collect();
    let mut actual = BTreeSet::new();
    for record in records {
        for predecessor in record.raw().raw_supersedes() {
            let predecessor_record = by_id
                .get(predecessor.as_str())
                .ok_or(DecisionClosureError::UnknownRawDecisionId)?;
            if !predecessor_record
                .raw()
                .raw_superseded_by()
                .iter()
                .any(|successor| successor == record.raw().id())
            {
                actual.insert((record.raw().id(), predecessor.as_str()));
            }
        }
    }
    let supplied = ignored
        .iter()
        .map(|claim| (claim.source(), claim.claimed_predecessor()))
        .collect::<BTreeSet<_>>();
    if supplied.len() != ignored.len() || supplied != actual {
        return Err(DecisionClosureError::IgnoredUnilateralClaimSetMismatch);
    }
    Ok(())
}

fn validate_recognized_external_composite_heads(
    records: &[ExternalDecisionClosureRecordV1],
    recognized: &[String],
) -> Result<(), DecisionClosureError> {
    if recognized
        .windows(2)
        .any(|pair| pair[0].as_str() >= pair[1].as_str())
    {
        return Err(DecisionClosureError::RecognizedCompositeHeadsNotStrictlySorted);
    }
    let composite_heads = records
        .iter()
        .filter(|record| record.raw().raw_supersedes().len() > 1)
        .map(|record| record.raw().id())
        .collect::<BTreeSet<_>>();
    if recognized
        .iter()
        .any(|identifier| !composite_heads.contains(identifier.as_str()))
    {
        return Err(DecisionClosureError::UnknownRecognizedCompositeHead);
    }
    Ok(())
}

fn external_closure_value(
    records: &[ExternalDecisionClosureRecordV1],
    materializations: &[RequiredDecisionMaterializationV1],
    ignored_unilateral_claims: &[IgnoredUnilateralClaimV1],
    recognized_external_composite_heads: &[String],
) -> Result<CborValue, DecisionClosureError> {
    closure_value(
        records,
        materializations,
        ignored_unilateral_claims,
        Some(recognized_external_composite_heads),
    )
}

fn decision_closure_value(
    records: &[ExternalDecisionClosureRecordV1],
    materializations: &[RequiredDecisionMaterializationV1],
    ignored_unilateral_claims: &[IgnoredUnilateralClaimV1],
) -> Result<CborValue, DecisionClosureError> {
    closure_value(records, materializations, ignored_unilateral_claims, None)
}

fn closure_value(
    records: &[ExternalDecisionClosureRecordV1],
    materializations: &[RequiredDecisionMaterializationV1],
    ignored_unilateral_claims: &[IgnoredUnilateralClaimV1],
    recognized_external_composite_heads: Option<&[String]>,
) -> Result<CborValue, DecisionClosureError> {
    let materialization_ids = materialization_ids_by_decision(materializations);
    let external = recognized_external_composite_heads.is_some();
    let mut value = vec![
        CborValue::Unsigned(if external {
            EXTERNAL_DESIGN_AUTHORITY_CLOSURE_VERSION_V1
        } else {
            DECISION_CLOSURE_VERSION_V1
        }),
        CborValue::Array(
            records
                .iter()
                .map(|record| {
                    closure_record_value(
                        record,
                        materialization_ids
                            .get(record.raw().id())
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        external,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CborValue::Array(
            materializations
                .iter()
                .map(required_materialization_record_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CborValue::Array(
            ignored_unilateral_claims
                .iter()
                .map(IgnoredUnilateralClaimV1::canonical_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        composite_external_heads_value(records)?,
    ];
    if let Some(heads) = recognized_external_composite_heads {
        value.push(text_array(heads)?);
    }
    Ok(CborValue::Array(value))
}

fn materialization_ids_by_decision(
    materializations: &[RequiredDecisionMaterializationV1],
) -> BTreeMap<&str, Vec<&DecisionMaterializationIdV1>> {
    let mut by_decision: BTreeMap<&str, Vec<&DecisionMaterializationIdV1>> = BTreeMap::new();
    for materialization in materializations {
        for source in materialization.sources() {
            by_decision
                .entry(source.decision_id())
                .or_default()
                .push(materialization.materialization_id());
        }
    }
    by_decision
}

fn closure_record_value(
    record: &ExternalDecisionClosureRecordV1,
    materialization_ids: &[&DecisionMaterializationIdV1],
    include_raw_record: bool,
) -> Result<CborValue, DecisionClosureError> {
    let mut fields = vec![
        CborValue::text(record.raw().id())?,
        CborValue::text(record.raw().terminal_status().as_str())?,
        CborValue::Bytes(record.raw().raw_record_hash().to_vec()),
        CborValue::Bytes(record.raw().raw_body_hash().to_vec()),
        text_array(record.raw().raw_supersedes())?,
        text_array(record.raw().raw_superseded_by())?,
        CborValue::text(record.lineage().as_str())?,
        optional_text(record.lineage().normalized_successor())?,
        CborValue::text(record.consequence().as_str())?,
        optional_text(record.consequence().rationale_disposition())?,
        CborValue::Array(
            materialization_ids
                .iter()
                .map(|identifier| CborValue::Bytes(identifier.as_bytes().to_vec()))
                .collect(),
        ),
        CborValue::text(derived_effect_status(record).as_str())?,
    ];
    if include_raw_record {
        fields.push(CborValue::Bytes(record.raw().raw_record().to_vec()));
    }
    Ok(CborValue::Array(fields))
}

fn required_materialization_record_value(
    materialization: &RequiredDecisionMaterializationV1,
) -> Result<CborValue, DecisionClosureError> {
    Ok(CborValue::Array(vec![
        CborValue::Bytes(materialization.materialization_id().as_bytes().to_vec()),
        CborValue::text(materialization.artifact_id())?,
        CborValue::Unsigned(materialization.component_kind().tag()),
        CborValue::Unsigned(0),
        CborValue::Array(
            materialization
                .sources()
                .iter()
                .map(|source| {
                    Ok(CborValue::Array(vec![
                        CborValue::text(source.decision_id())?,
                        CborValue::Bytes(source.raw_body_hash().to_vec()),
                    ]))
                })
                .collect::<Result<Vec<_>, DecisionClosureError>>()?,
        ),
    ]))
}

fn normative_inputs_value(
    materialization: &RequiredDecisionMaterializationV1,
) -> Result<CborValue, DecisionClosureError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Bytes(materialization.materialization_id().as_bytes().to_vec()),
        CborValue::Array(
            materialization
                .sources()
                .iter()
                .map(|source| {
                    Ok(CborValue::Array(vec![
                        CborValue::text(source.decision_id())?,
                        CborValue::Bytes(source.raw_body_hash().to_vec()),
                    ]))
                })
                .collect::<Result<Vec<_>, DecisionClosureError>>()?,
        ),
    ]))
}

fn materialization_identity_value(
    artifact_id: &str,
    component_kind: ContractComponentKindV1,
) -> Result<CborValue, DecisionClosureError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::text(artifact_id)?,
        CborValue::Unsigned(component_kind.tag()),
    ]))
}

fn composite_external_heads_value(
    records: &[ExternalDecisionClosureRecordV1],
) -> Result<CborValue, DecisionClosureError> {
    Ok(CborValue::Array(
        records
            .iter()
            .filter(|record| record.raw().raw_supersedes().len() > 1)
            .map(|record| {
                Ok(CborValue::Array(vec![
                    CborValue::text(record.raw().id())?,
                    text_array(record.raw().raw_supersedes())?,
                ]))
            })
            .collect::<Result<Vec<_>, DecisionClosureError>>()?,
    ))
}

fn derived_effect_status(
    record: &ExternalDecisionClosureRecordV1,
) -> DerivedDecisionEffectStatusV1 {
    match record.consequence() {
        DecisionConsequenceClassificationV1::RationaleOnly { .. } => {
            DerivedDecisionEffectStatusV1::NoContractEffect
        }
        DecisionConsequenceClassificationV1::Material
            if record.raw().terminal_status() == TerminalDecisionStatusV1::Locked =>
        {
            DerivedDecisionEffectStatusV1::Unapplied
        }
        DecisionConsequenceClassificationV1::Material => {
            DerivedDecisionEffectStatusV1::SupersededButEffectLive
        }
    }
}

fn optional_text(value: Option<&str>) -> Result<CborValue, DecisionClosureError> {
    match value {
        None => Ok(CborValue::Array(vec![CborValue::Unsigned(0)])),
        Some(value) => Ok(CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::text(value)?,
        ])),
    }
}

fn text_array(values: &[String]) -> Result<CborValue, DecisionClosureError> {
    Ok(CborValue::Array(
        values
            .iter()
            .map(|value| Ok(CborValue::text(value)?))
            .collect::<Result<Vec<_>, DecisionClosureError>>()?,
    ))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::contract::assembly::{
        candidate_root_schema_closure_v1, facet_schema_id_v1, finalization_facet_kinds_v1,
        fixture_facet_value_v1, normative_inputs_schema_id_v1,
    };
    use crate::domain::vnext::contract::component::CandidateContractComponentV1;
    use crate::domain::vnext::contract::finalization::{
        DesignBasisV1, FinalizationInputKindV1, PinnedFinalizationInputV1,
    };
    use crate::domain::vnext::identity::{
        DesignRevisionIdV1, SchemaClosureV1, decision_resolution_identity,
        design_revision_identity, design_source_binding_identity,
    };

    struct RootBindingFixture {
        schemas: SchemaClosureV1,
        decision_closure: DecisionClosureV1,
        design_revision_id: DesignRevisionIdV1,
        root: CandidateContractRootV1,
        finalization: DesignFinalizationManifestV1,
    }

    impl RootBindingFixture {
        fn new() -> Self {
            let schemas = candidate_root_schema_closure_v1().expect("candidate root schemas");
            let decision_closure = fixture_decision_closure(2);
            let closure_id = *decision_closure.closure_id();
            let design_revision_id = design_revision_identity(&CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(closure_id.as_bytes().to_vec()),
            ]))
            .expect("design revision");
            let normative_schema_id =
                normative_inputs_schema_id_v1(&schemas).expect("NormativeInputs schema");
            let mut components = decision_closure
                .materializations()
                .iter()
                .map(|materialization| {
                    let resolution = DecisionMaterializationResolutionV1::new(
                        closure_id,
                        MaterializationBaseV1::initial_external_design_closure(closure_id),
                        *materialization.materialization_id(),
                    )
                    .expect("Decision materialization resolution");
                    CandidateContractComponentV1::new(
                        &schemas,
                        ContractComponentKindV1::NormativeInputs,
                        normative_schema_id,
                        normative_inputs_value(materialization).expect("NormativeInputs value"),
                        vec![],
                        ComponentProvenanceV1::decision_materialization(
                            *resolution.resolution_id(),
                            *materialization.materialization_id(),
                        ),
                    )
                    .expect("NormativeInputs component")
                })
                .collect::<Vec<_>>();
            let mut normative_ids = components
                .iter()
                .map(|component| *component.component_id())
                .collect::<Vec<_>>();
            normative_ids.sort_by_key(|identifier| *identifier.as_bytes());
            for kind in ContractComponentKindV1::ALL {
                if kind == ContractComponentKindV1::NormativeInputs {
                    continue;
                }
                let source_binding_id = design_source_binding_identity(&CborValue::Array(vec![
                    CborValue::Unsigned(kind.tag()),
                    CborValue::Bytes(closure_id.as_bytes().to_vec()),
                ]))
                .expect("source binding");
                components.push(
                    CandidateContractComponentV1::new(
                        &schemas,
                        kind,
                        facet_schema_id_v1(&schemas, kind).expect("facet schema"),
                        fixture_facet_value_v1(kind, [kind.tag() as u8; 32], vec![[3; 32]]),
                        normative_ids.clone(),
                        ComponentProvenanceV1::design_slot(
                            design_revision_id,
                            kind.tag(),
                            source_binding_id,
                        )
                        .expect("facet provenance"),
                    )
                    .expect("aggregate facet"),
                );
            }
            let root = CandidateContractRootV1::new(&schemas, components).expect("candidate root");
            let finalization =
                fixture_finalization(&schemas, design_revision_id, closure_id, &root);
            Self {
                schemas,
                decision_closure,
                design_revision_id,
                root,
                finalization,
            }
        }

        fn rebuilt_root(
            &self,
            mut normative_components: Vec<CandidateContractComponentV1>,
        ) -> CandidateContractRootV1 {
            let mut normative_ids = normative_components
                .iter()
                .map(|component| *component.component_id())
                .collect::<Vec<_>>();
            normative_ids.sort_by_key(|identifier| *identifier.as_bytes());
            for component in
                self.root.components().iter().filter(|component| {
                    component.kind() != ContractComponentKindV1::NormativeInputs
                })
            {
                normative_components.push(
                    CandidateContractComponentV1::new(
                        &self.schemas,
                        component.kind(),
                        *component.schema_id(),
                        component.value().clone(),
                        normative_ids.clone(),
                        component.provenance().clone(),
                    )
                    .expect("rebuilt aggregate component"),
                );
            }
            CandidateContractRootV1::new(&self.schemas, normative_components)
                .expect("rebuilt candidate root")
        }

        fn finalization_for(&self, root: &CandidateContractRootV1) -> DesignFinalizationManifestV1 {
            fixture_finalization(
                &self.schemas,
                self.design_revision_id,
                *self.decision_closure.closure_id(),
                root,
            )
        }

        fn bindings_for(
            &self,
            root: &CandidateContractRootV1,
            finalization: &DesignFinalizationManifestV1,
        ) -> Vec<ExactDecisionRootBindingV1> {
            fixture_bindings(root, finalization, *self.decision_closure.closure_id())
        }
    }

    #[test]
    fn exact_decision_root_binding_rejects_every_stale_or_fabricated_join() {
        let fixture = RootBindingFixture::new();
        let bindings = fixture.bindings_for(&fixture.root, &fixture.finalization);
        fixture
            .decision_closure
            .root_binding_requirements()
            .resolve(bindings.clone(), &fixture.root, &fixture.finalization)
            .expect("valid exact Decision-root bindings");

        let mut unordered = bindings.clone();
        unordered.reverse();
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(unordered, &fixture.root, &fixture.finalization,),
            Err(DecisionClosureError::BindingsNotStrictlySorted)
        ));

        let first_materialization = &fixture.decision_closure.materializations()[0];
        let second_materialization = &fixture.decision_closure.materializations()[1];
        let first_component =
            normative_component(&fixture.root, first_materialization.materialization_id());
        let second_component =
            normative_component(&fixture.root, second_materialization.materialization_id());
        let original_normatives = fixture
            .root
            .components()
            .iter()
            .filter(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
            .cloned()
            .collect::<Vec<_>>();

        let fabricated_resolution = decision_resolution_identity(&CborValue::Unsigned(9_001))
            .expect("fabricated resolution identity");
        let wrong_resolution_component = CandidateContractComponentV1::new(
            &fixture.schemas,
            ContractComponentKindV1::NormativeInputs,
            *first_component.schema_id(),
            first_component.value().clone(),
            first_component.dependencies().to_vec(),
            ComponentProvenanceV1::decision_materialization(
                fabricated_resolution,
                *first_materialization.materialization_id(),
            ),
        )
        .expect("wrong-resolution component");
        let wrong_resolution_root = fixture.rebuilt_root(replace_component(
            &original_normatives,
            first_component,
            wrong_resolution_component,
        ));
        let wrong_resolution_finalization = fixture.finalization_for(&wrong_resolution_root);
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(
                    fixture.bindings_for(&wrong_resolution_root, &wrong_resolution_finalization,),
                    &wrong_resolution_root,
                    &wrong_resolution_finalization,
                ),
            Err(DecisionClosureError::NormativeResolutionMismatch)
        ));

        let first_source = &first_materialization.sources()[0];
        let stale_value_component = CandidateContractComponentV1::new(
            &fixture.schemas,
            ContractComponentKindV1::NormativeInputs,
            *first_component.schema_id(),
            CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(
                    first_materialization
                        .materialization_id()
                        .as_bytes()
                        .to_vec(),
                ),
                CborValue::Array(vec![CborValue::Array(vec![
                    CborValue::Text(first_source.decision_id().to_owned()),
                    CborValue::Bytes([0xff; 32].to_vec()),
                ])]),
            ]),
            first_component.dependencies().to_vec(),
            first_component.provenance().clone(),
        )
        .expect("stale-value component");
        let stale_value_root = fixture.rebuilt_root(replace_component(
            &original_normatives,
            first_component,
            stale_value_component,
        ));
        let stale_value_finalization = fixture.finalization_for(&stale_value_root);
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(
                    fixture.bindings_for(&stale_value_root, &stale_value_finalization),
                    &stale_value_root,
                    &stale_value_finalization,
                ),
            Err(DecisionClosureError::NormativeComponentValueMismatch)
        ));

        let duplicate_provenance_component = CandidateContractComponentV1::new(
            &fixture.schemas,
            ContractComponentKindV1::NormativeInputs,
            *first_component.schema_id(),
            first_component.value().clone(),
            vec![*first_component.component_id()],
            first_component.provenance().clone(),
        )
        .expect("duplicate-provenance component");
        let duplicate_provenance_root = fixture.rebuilt_root(replace_component(
            &original_normatives,
            second_component,
            duplicate_provenance_component,
        ));
        let duplicate_provenance_finalization =
            fixture.finalization_for(&duplicate_provenance_root);
        let duplicate_replacement = duplicate_provenance_root
            .components()
            .iter()
            .find(|component| {
                component.kind() == ContractComponentKindV1::NormativeInputs
                    && !component.dependencies().is_empty()
            })
            .expect("duplicate-provenance replacement");
        let mut duplicate_provenance_bindings = vec![
            ExactDecisionRootBindingV1::new(
                *first_materialization.materialization_id(),
                *normative_component(
                    &duplicate_provenance_root,
                    first_materialization.materialization_id(),
                )
                .component_id(),
                MaterializationBaseV1::initial_external_design_closure(
                    *fixture.decision_closure.closure_id(),
                ),
                *duplicate_provenance_root.root_id(),
                *duplicate_provenance_finalization.manifest_id(),
            ),
            ExactDecisionRootBindingV1::new(
                *second_materialization.materialization_id(),
                *duplicate_replacement.component_id(),
                MaterializationBaseV1::initial_external_design_closure(
                    *fixture.decision_closure.closure_id(),
                ),
                *duplicate_provenance_root.root_id(),
                *duplicate_provenance_finalization.manifest_id(),
            ),
        ];
        duplicate_provenance_bindings
            .sort_by_key(|binding| *binding.materialization_id().as_bytes());
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(
                    duplicate_provenance_bindings,
                    &duplicate_provenance_root,
                    &duplicate_provenance_finalization,
                ),
            Err(DecisionClosureError::DuplicateNormativeMaterialization)
        ));

        let aggregate_component = fixture
            .root
            .components()
            .iter()
            .find(|component| component.kind() != ContractComponentKindV1::NormativeInputs)
            .expect("aggregate component");
        let first_binding = bindings[0].clone();
        let mut wrong_component = bindings.clone();
        wrong_component[0] = ExactDecisionRootBindingV1::new(
            *first_binding.materialization_id(),
            *aggregate_component.component_id(),
            first_binding.materialization_base().clone(),
            *fixture.root.root_id(),
            *fixture.finalization.manifest_id(),
        );
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(wrong_component, &fixture.root, &fixture.finalization,),
            Err(DecisionClosureError::NormativeComponentSetMismatch)
        ));

        let mut wrong_base = bindings.clone();
        wrong_base[0] = ExactDecisionRootBindingV1::new(
            *first_binding.materialization_id(),
            *first_binding.component_id(),
            MaterializationBaseV1::prior_contract_root(*fixture.root.root_id()),
            *fixture.root.root_id(),
            *fixture.finalization.manifest_id(),
        );
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(wrong_base, &fixture.root, &fixture.finalization,),
            Err(DecisionClosureError::BindingMaterializationBaseMismatch)
        ));

        let wrong_root = ContractRootIdV1::parse(&format!("sha256:{}", "00".repeat(32)))
            .expect("different root identity");
        let mut wrong_root_binding = bindings.clone();
        wrong_root_binding[0] = ExactDecisionRootBindingV1::new(
            *first_binding.materialization_id(),
            *first_binding.component_id(),
            first_binding.materialization_base().clone(),
            wrong_root,
            *fixture.finalization.manifest_id(),
        );
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(wrong_root_binding, &fixture.root, &fixture.finalization,),
            Err(DecisionClosureError::BindingFinalizationMismatch)
        ));

        let wrong_finalization =
            DesignFinalizationManifestIdV1::parse(&format!("sha256:{}", "11".repeat(32)))
                .expect("different finalization identity");
        let mut wrong_finalization_binding = bindings;
        wrong_finalization_binding[0] = ExactDecisionRootBindingV1::new(
            *first_binding.materialization_id(),
            *first_binding.component_id(),
            first_binding.materialization_base().clone(),
            *fixture.root.root_id(),
            wrong_finalization,
        );
        assert!(matches!(
            fixture
                .decision_closure
                .root_binding_requirements()
                .resolve(
                    wrong_finalization_binding,
                    &fixture.root,
                    &fixture.finalization,
                ),
            Err(DecisionClosureError::BindingFinalizationMismatch)
        ));
    }

    fn fixture_decision_closure(materialization_count: usize) -> DecisionClosureV1 {
        let records = (0..materialization_count)
            .map(|index| {
                let identifier = format!("decision-{index:04}");
                let raw = RawExternalDecisionRecordV1::new(
                    &identifier,
                    TerminalDecisionStatusV1::Locked,
                    format!("raw:{identifier}").into_bytes(),
                    format!("body:{identifier}").into_bytes(),
                    vec![],
                    vec![],
                )
                .expect("raw fixture Decision");
                ExternalDecisionClosureRecordV1::new(
                    raw,
                    ExternalLineageDispositionV1::None,
                    DecisionConsequenceClassificationV1::Material,
                )
                .expect("fixture Decision")
            })
            .collect::<Vec<_>>();
        let materializations = records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                RequiredDecisionMaterializationV1::new(
                    format!("maestro.vnext.candidate-contract.normative-inputs.v1/{index:04}"),
                    ContractComponentKindV1::NormativeInputs,
                    vec![
                        DecisionMaterializationSourceV1::new(
                            record.raw().id(),
                            *record.raw().raw_body_hash(),
                        )
                        .expect("fixture materialization source"),
                    ],
                )
                .expect("fixture materialization")
            })
            .collect::<Vec<_>>();
        let expected_ids = records
            .iter()
            .map(|record| record.raw().id().to_owned())
            .collect::<Vec<_>>();
        let external = ExternalDesignAuthorityClosureV1::new(
            records,
            materializations,
            &expected_ids,
            vec![],
            vec![],
        )
        .expect("fixture external closure");
        DecisionClosureV1::from_external(&external).expect("fixture Decision closure")
    }

    fn fixture_finalization(
        schemas: &SchemaClosureV1,
        design_revision_id: DesignRevisionIdV1,
        decision_closure_id: DecisionClosureIdV1,
        root: &CandidateContractRootV1,
    ) -> DesignFinalizationManifestV1 {
        let stage_proof_binding_fields = root
            .components()
            .iter()
            .find(|component| component.kind() == ContractComponentKindV1::StageProofMatrix)
            .map(|component| match component.value() {
                CborValue::Array(fields) => fields[1..].to_vec(),
                _ => panic!("StageProofMatrix facet value must be an array"),
            })
            .expect("StageProofMatrix facet component");
        let inputs = FinalizationInputKindV1::ALL
            .into_iter()
            .map(|kind| {
                let owner_facet_ids = finalization_facet_kinds_v1(kind)
                    .iter()
                    .map(|facet_kind| {
                        *root
                            .components()
                            .iter()
                            .find(|component| component.kind() == *facet_kind)
                            .expect("owner facet component")
                            .component_id()
                    })
                    .collect::<Vec<_>>();
                let mut fields = vec![
                    CborValue::Unsigned(1),
                    CborValue::Unsigned(kind.tag()),
                    CborValue::Bytes(design_revision_id.as_bytes().to_vec()),
                    CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
                    CborValue::Bytes(root.root_id().as_bytes().to_vec()),
                    CborValue::Array(
                        owner_facet_ids
                            .iter()
                            .map(|identifier| CborValue::Bytes(identifier.as_bytes().to_vec()))
                            .collect(),
                    ),
                ];
                if kind == FinalizationInputKindV1::StageProofMatrix {
                    fields.extend(stage_proof_binding_fields.clone());
                }
                let value = CborValue::Array(fields);
                match kind {
                    FinalizationInputKindV1::ClosureRequirement => {
                        PinnedFinalizationInputV1::closure_requirement(schemas, value)
                    }
                    FinalizationInputKindV1::DeterministicSynthesis => {
                        PinnedFinalizationInputV1::deterministic_synthesis(schemas, value)
                    }
                    FinalizationInputKindV1::ScopeAndExclusions => {
                        PinnedFinalizationInputV1::scope_and_exclusions(schemas, value)
                    }
                    FinalizationInputKindV1::CapabilityCensusAndJourneys => {
                        PinnedFinalizationInputV1::capability_census_and_journeys(schemas, value)
                    }
                    FinalizationInputKindV1::MigrationRollbackRemoval => {
                        PinnedFinalizationInputV1::migration_rollback_removal(schemas, value)
                    }
                    FinalizationInputKindV1::StageProofMatrix => {
                        PinnedFinalizationInputV1::stage_proof_matrix(schemas, value)
                    }
                    FinalizationInputKindV1::ReviewEvidence => {
                        PinnedFinalizationInputV1::review_evidence(schemas, value)
                    }
                    FinalizationInputKindV1::EdgeSweepEvidence => {
                        PinnedFinalizationInputV1::edge_sweep_evidence(schemas, value)
                    }
                    FinalizationInputKindV1::RiskRecovery => {
                        PinnedFinalizationInputV1::risk_recovery(schemas, value)
                    }
                    FinalizationInputKindV1::FreshnessReferences => {
                        PinnedFinalizationInputV1::freshness_references(schemas, value)
                    }
                    FinalizationInputKindV1::CanonicalizationPolicy => {
                        PinnedFinalizationInputV1::canonicalization_policy(schemas, value)
                    }
                }
                .expect("pinned finalization input")
            })
            .collect();
        DesignFinalizationManifestV1::new(
            schemas,
            DesignBasisV1::design_revision(design_revision_id),
            decision_closure_id,
            root,
            inputs,
        )
        .expect("design finalization manifest")
    }

    fn fixture_bindings(
        root: &CandidateContractRootV1,
        finalization: &DesignFinalizationManifestV1,
        decision_closure_id: DecisionClosureIdV1,
    ) -> Vec<ExactDecisionRootBindingV1> {
        let mut bindings = root
            .components()
            .iter()
            .filter(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
            .map(|component| {
                let ComponentProvenanceV1::DecisionMaterialization(provenance) =
                    component.provenance()
                else {
                    panic!("NormativeInputs fixture must use Decision materialization provenance");
                };
                ExactDecisionRootBindingV1::new(
                    *provenance.materialization_id(),
                    *component.component_id(),
                    MaterializationBaseV1::initial_external_design_closure(decision_closure_id),
                    *root.root_id(),
                    *finalization.manifest_id(),
                )
            })
            .collect::<Vec<_>>();
        bindings.sort_by_key(|binding| *binding.materialization_id().as_bytes());
        bindings
    }

    fn normative_component<'a>(
        root: &'a CandidateContractRootV1,
        materialization_id: &DecisionMaterializationIdV1,
    ) -> &'a CandidateContractComponentV1 {
        root.components()
            .iter()
            .find(|component| {
                matches!(
                    component.provenance(),
                    ComponentProvenanceV1::DecisionMaterialization(provenance)
                        if provenance.materialization_id() == materialization_id
                )
            })
            .expect("fixture NormativeInputs component")
    }

    fn replace_component(
        components: &[CandidateContractComponentV1],
        replaced: &CandidateContractComponentV1,
        replacement: CandidateContractComponentV1,
    ) -> Vec<CandidateContractComponentV1> {
        components
            .iter()
            .map(|component| {
                if component.component_id() == replaced.component_id() {
                    replacement.clone()
                } else {
                    component.clone()
                }
            })
            .collect()
    }
}
