use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::contract::component::CandidateContractComponentV1;
use crate::domain::vnext::contract::materialization::{
    ContractConsequencePlanIdV1, ContractConsequencePlanV1,
    DecisionMaterializationPreimageCommitmentV1, MaterializationBaseV1,
    PlannedContractDependencyV1, PlannedContractSlotV1,
};
use crate::domain::vnext::contract::provenance::ComponentProvenanceV1;
use crate::domain::vnext::contract::root::CandidateContractRootV1;
use crate::domain::vnext::identity::{
    ContractComponentIdV1, ContractRootIdV1, DecisionMaterializationIdV1, DecisionResolutionIdV1,
    IdentityError, SchemaClosureV1, StoreObjectIdV1, decision_materialization_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{AdmittedCommittedActionV1, CommittedActionAuditV1, ResolutionV1};

const DECISION_MATERIALIZATION_VERSION_V1: u64 = 2;
const DECISION_MATERIALIZATION_PREIMAGE_VERSION_V1: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractComponentDeltaV1 {
    added: Vec<ContractComponentIdV1>,
    removed: Vec<ContractComponentIdV1>,
    retained: Vec<ContractComponentIdV1>,
}

impl ContractComponentDeltaV1 {
    pub fn added(&self) -> &[ContractComponentIdV1] {
        &self.added
    }

    pub fn removed(&self) -> &[ContractComponentIdV1] {
        &self.removed
    }

    pub fn retained(&self) -> &[ContractComponentIdV1] {
        &self.retained
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            component_ids_value(&self.added),
            component_ids_value(&self.removed),
            component_ids_value(&self.retained),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentInvalidationReceiptV1 {
    component_id: ContractComponentIdV1,
    receipt_object_id: StoreObjectIdV1,
}

impl ComponentInvalidationReceiptV1 {
    pub const fn component_id(&self) -> &ContractComponentIdV1 {
        &self.component_id
    }

    pub const fn receipt_object_id(&self) -> StoreObjectIdV1 {
        self.receipt_object_id
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.component_id.as_bytes().to_vec()),
            CborValue::Bytes(self.receipt_object_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEquivalenceReceiptV1 {
    base_root_id: ContractRootIdV1,
    candidate_root_id: ContractRootIdV1,
    receipt_object_id: StoreObjectIdV1,
}

impl ExactEquivalenceReceiptV1 {
    pub const fn base_root_id(&self) -> &ContractRootIdV1 {
        &self.base_root_id
    }

    pub const fn candidate_root_id(&self) -> &ContractRootIdV1 {
        &self.candidate_root_id
    }

    pub const fn receipt_object_id(&self) -> StoreObjectIdV1 {
        self.receipt_object_id
    }

    pub const fn is_bearer_authority(&self) -> bool {
        false
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.base_root_id.as_bytes().to_vec()),
            CborValue::Bytes(self.candidate_root_id.as_bytes().to_vec()),
            CborValue::Bytes(self.receipt_object_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Debug)]
pub struct AdmittedMaterializationAuthorityV1 {
    action: AdmittedCommittedActionV1,
    preflight_binding: [u8; 32],
    invalidation_receipts: Vec<ComponentInvalidationReceiptV1>,
}

impl AdmittedMaterializationAuthorityV1 {
    pub(crate) fn from_store_commit(
        action: AdmittedCommittedActionV1,
        preflight: &DecisionMaterializationPreflightV1,
        invalidation_receipt_refs: Vec<(ContractComponentIdV1, StoreObjectIdV1)>,
    ) -> Result<Self, MaterializationV1Error> {
        if preflight.equal_root {
            return Err(MaterializationV1Error::EqualRootRejectsAuthority);
        }
        let mut invalidation_receipts = invalidation_receipt_refs
            .into_iter()
            .map(
                |(component_id, receipt_object_id)| ComponentInvalidationReceiptV1 {
                    component_id,
                    receipt_object_id,
                },
            )
            .collect::<Vec<_>>();
        invalidation_receipts.sort_by_key(|receipt| *receipt.component_id());
        if invalidation_receipts
            .windows(2)
            .any(|pair| pair[0].component_id() == pair[1].component_id())
        {
            return Err(MaterializationV1Error::DuplicateInvalidationReceipt);
        }
        let invalidated = invalidation_receipts
            .iter()
            .map(|receipt| *receipt.component_id())
            .collect::<Vec<_>>();
        if invalidated != preflight.delta.removed {
            return Err(MaterializationV1Error::IncompleteInvalidationReceipts);
        }
        Ok(Self {
            action,
            preflight_binding: preflight.binding,
            invalidation_receipts,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionMaterializationDispositionV1 {
    Changed {
        invalidation_receipts: Vec<ComponentInvalidationReceiptV1>,
    },
    NoOpEqualRoot,
    NoOpEquivalent {
        equivalence_receipt: ExactEquivalenceReceiptV1,
    },
}

impl DecisionMaterializationDispositionV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Changed {
                invalidation_receipts,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Array(
                    invalidation_receipts
                        .iter()
                        .map(ComponentInvalidationReceiptV1::canonical_value)
                        .collect(),
                ),
            ]),
            Self::NoOpEqualRoot => CborValue::Array(vec![CborValue::Unsigned(2)]),
            Self::NoOpEquivalent {
                equivalence_receipt,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                equivalence_receipt.canonical_value(),
            ]),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DecisionMaterializationV1 {
    resolution_id: DecisionResolutionIdV1,
    plan_id: ContractConsequencePlanIdV1,
    preimage_commitment: DecisionMaterializationPreimageCommitmentV1,
    materialization_base: MaterializationBaseV1,
    base_root: CandidateContractRootV1,
    candidate_root: CandidateContractRootV1,
    delta: ContractComponentDeltaV1,
    disposition: DecisionMaterializationDispositionV1,
    authorization: Option<CommittedActionAuditV1>,
    materialization_id: DecisionMaterializationIdV1,
}

impl PartialEq for DecisionMaterializationV1 {
    fn eq(&self, other: &Self) -> bool {
        self.materialization_id == other.materialization_id
    }
}

impl Eq for DecisionMaterializationV1 {}

impl DecisionMaterializationV1 {
    pub fn preflight(
        resolution: &ResolutionV1,
        schemas: &SchemaClosureV1,
        base_root: &CandidateContractRootV1,
    ) -> Result<DecisionMaterializationPreflightV1, MaterializationV1Error> {
        DecisionMaterializationPreflightV1::evaluate(resolution, schemas, base_root)
    }

    pub fn resolution_id(&self) -> &DecisionResolutionIdV1 {
        &self.resolution_id
    }

    pub const fn plan_id(&self) -> ContractConsequencePlanIdV1 {
        self.plan_id
    }

    pub const fn preimage_commitment(&self) -> DecisionMaterializationPreimageCommitmentV1 {
        self.preimage_commitment
    }

    pub fn materialization_base(&self) -> &MaterializationBaseV1 {
        &self.materialization_base
    }

    pub fn base_root(&self) -> &CandidateContractRootV1 {
        &self.base_root
    }

    pub fn base_root_id(&self) -> &ContractRootIdV1 {
        self.base_root.root_id()
    }

    pub fn candidate_root(&self) -> &CandidateContractRootV1 {
        &self.candidate_root
    }

    pub fn candidate_root_id(&self) -> &ContractRootIdV1 {
        self.candidate_root.root_id()
    }

    pub const fn delta(&self) -> &ContractComponentDeltaV1 {
        &self.delta
    }

    pub const fn disposition(&self) -> &DecisionMaterializationDispositionV1 {
        &self.disposition
    }

    pub const fn authorization(&self) -> Option<&CommittedActionAuditV1> {
        self.authorization.as_ref()
    }

    pub fn materialization_id(&self) -> &DecisionMaterializationIdV1 {
        &self.materialization_id
    }

    pub fn verify_exact_transformation(
        &self,
        resolution: &ResolutionV1,
        schemas: &SchemaClosureV1,
    ) -> Result<(), MaterializationV1Error> {
        let expected =
            DecisionMaterializationPreflightV1::evaluate(resolution, schemas, &self.base_root)?;
        if expected.binding
            != materialization_preflight_binding(
                &self.resolution_id,
                self.plan_id,
                &self.preimage_commitment,
                &self.base_root,
                &self.candidate_root,
                &self.delta,
            )?
            || expected
                .candidate_root
                .canonical_bytes()
                .map_err(|_| MaterializationV1Error::InvalidCandidateRoot)?
                != self
                    .candidate_root
                    .canonical_bytes()
                    .map_err(|_| MaterializationV1Error::InvalidCandidateRoot)?
            || expected.delta != self.delta
            || expected.preimage_commitment != self.preimage_commitment
        {
            return Err(MaterializationV1Error::TransformationJoinMismatch);
        }
        let expected_id = decision_materialization_identity(&materialization_value(
            &self.resolution_id,
            self.plan_id,
            &self.preimage_commitment,
            &self.base_root,
            &self.candidate_root,
            &self.delta,
            &self.disposition,
            self.authorization.as_ref(),
        ))?;
        if expected_id != self.materialization_id {
            return Err(MaterializationV1Error::TransformationJoinMismatch);
        }
        Ok(())
    }

    pub fn verified_component_provenance(
        &self,
        resolution: &ResolutionV1,
        schemas: &SchemaClosureV1,
    ) -> Result<ComponentProvenanceV1, MaterializationV1Error> {
        self.verify_exact_transformation(resolution, schemas)?;
        Ok(ComponentProvenanceV1::decision_materialization(
            self.resolution_id,
            self.materialization_id,
        ))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, MaterializationV1Error> {
        Ok(deterministic_cbor::encode(&materialization_value(
            &self.resolution_id,
            self.plan_id,
            &self.preimage_commitment,
            &self.base_root,
            &self.candidate_root,
            &self.delta,
            &self.disposition,
            self.authorization.as_ref(),
        ))?)
    }
}

#[derive(Clone, Debug)]
pub struct DecisionMaterializationPreflightV1 {
    resolution_id: DecisionResolutionIdV1,
    plan_id: ContractConsequencePlanIdV1,
    preimage_commitment: DecisionMaterializationPreimageCommitmentV1,
    base_root: CandidateContractRootV1,
    candidate_root: CandidateContractRootV1,
    delta: ContractComponentDeltaV1,
    equal_root: bool,
    binding: [u8; 32],
}

impl DecisionMaterializationPreflightV1 {
    fn evaluate(
        resolution: &ResolutionV1,
        schemas: &SchemaClosureV1,
        base_root: &CandidateContractRootV1,
    ) -> Result<Self, MaterializationV1Error> {
        let plan = resolution
            .selected_consequence()
            .typed_plan_value()
            .ok_or(MaterializationV1Error::NoContractEffectResolution)?;
        if resolution.base_contract_root_id() != base_root.root_id()
            || plan.base_root_id() != base_root.root_id()
        {
            return Err(MaterializationV1Error::StaleBase);
        }
        let preimage_commitment = materialization_preimage_commitment(resolution, plan)?;
        let candidate_root =
            evaluate_plan(resolution, plan, schemas, base_root, preimage_commitment)?;
        let delta = derive_delta(base_root, &candidate_root);
        let equal_root = base_root
            .canonical_bytes()
            .map_err(|_| MaterializationV1Error::InvalidBaseRoot)?
            == candidate_root
                .canonical_bytes()
                .map_err(|_| MaterializationV1Error::InvalidCandidateRoot)?;
        let binding = materialization_preflight_binding(
            resolution.resolution_id(),
            plan.plan_id(),
            &preimage_commitment,
            base_root,
            &candidate_root,
            &delta,
        )?;
        Ok(Self {
            resolution_id: *resolution.resolution_id(),
            plan_id: plan.plan_id(),
            preimage_commitment,
            base_root: base_root.clone(),
            candidate_root,
            delta,
            equal_root,
            binding,
        })
    }

    pub fn base_root(&self) -> &CandidateContractRootV1 {
        &self.base_root
    }

    pub fn candidate_root(&self) -> &CandidateContractRootV1 {
        &self.candidate_root
    }

    pub const fn delta(&self) -> &ContractComponentDeltaV1 {
        &self.delta
    }

    pub const fn is_equal_root(&self) -> bool {
        self.equal_root
    }

    pub fn complete_equal_root(self) -> Result<DecisionMaterializationV1, MaterializationV1Error> {
        if !self.equal_root {
            return Err(MaterializationV1Error::DifferentRootRequiresAuthority);
        }
        self.complete(DecisionMaterializationDispositionV1::NoOpEqualRoot, None)
    }

    pub fn complete_with_authority(
        self,
        authority: AdmittedMaterializationAuthorityV1,
    ) -> Result<DecisionMaterializationV1, MaterializationV1Error> {
        if self.equal_root {
            return Err(MaterializationV1Error::EqualRootRejectsAuthority);
        }
        if authority.preflight_binding != self.binding {
            return Err(MaterializationV1Error::AuthorityTransformationMismatch);
        }
        let authorization = Some(authority.action.audit().clone());
        let disposition = DecisionMaterializationDispositionV1::Changed {
            invalidation_receipts: authority.invalidation_receipts,
        };
        self.complete(disposition, authorization)
    }

    pub(crate) fn complete_exact_equivalent(
        self,
        receipt_object_id: StoreObjectIdV1,
    ) -> Result<DecisionMaterializationV1, MaterializationV1Error> {
        if self.equal_root {
            return Err(MaterializationV1Error::EqualRootRejectsEquivalenceReceipt);
        }
        let equivalence_receipt = ExactEquivalenceReceiptV1 {
            base_root_id: *self.base_root.root_id(),
            candidate_root_id: *self.candidate_root.root_id(),
            receipt_object_id,
        };
        self.complete(
            DecisionMaterializationDispositionV1::NoOpEquivalent {
                equivalence_receipt,
            },
            None,
        )
    }

    fn complete(
        self,
        disposition: DecisionMaterializationDispositionV1,
        authorization: Option<CommittedActionAuditV1>,
    ) -> Result<DecisionMaterializationV1, MaterializationV1Error> {
        let materialization_base =
            MaterializationBaseV1::prior_contract_root(*self.base_root.root_id());
        let materialization_id = decision_materialization_identity(&materialization_value(
            &self.resolution_id,
            self.plan_id,
            &self.preimage_commitment,
            &self.base_root,
            &self.candidate_root,
            &self.delta,
            &disposition,
            authorization.as_ref(),
        ))?;
        Ok(DecisionMaterializationV1 {
            resolution_id: self.resolution_id,
            plan_id: self.plan_id,
            preimage_commitment: self.preimage_commitment,
            materialization_base,
            base_root: self.base_root,
            candidate_root: self.candidate_root,
            delta: self.delta,
            disposition,
            authorization,
            materialization_id,
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MaterializationV1Error {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("no-contract-effect Resolution cannot be materialized")]
    NoContractEffectResolution,
    #[error("Decision Materialization base is stale")]
    StaleBase,
    #[error("typed consequence plan references an unavailable retained component")]
    MissingRetainedComponent,
    #[error("typed consequence plan contains a cyclic or unresolved planned dependency")]
    UnresolvedPlannedDependency,
    #[error("typed consequence plan produced an invalid candidate component")]
    InvalidCandidateComponent,
    #[error("typed consequence plan produced an invalid base Contract root")]
    InvalidBaseRoot,
    #[error("typed consequence plan produced an invalid candidate Contract root")]
    InvalidCandidateRoot,
    #[error("different-root Decision Materialization requires admitted Store authority")]
    DifferentRootRequiresAuthority,
    #[error("equal-root no-op is detected before authority and rejects authority evidence")]
    EqualRootRejectsAuthority,
    #[error("equal-root no-op rejects an exact-equivalence Receipt")]
    EqualRootRejectsEquivalenceReceipt,
    #[error("Decision Materialization repeats an invalidation receipt")]
    DuplicateInvalidationReceipt,
    #[error(
        "changed Decision Materialization lacks an exact invalidation receipt for every removal"
    )]
    IncompleteInvalidationReceipts,
    #[error("admitted materialization authority does not bind the evaluated transformation")]
    AuthorityTransformationMismatch,
    #[error("Resolution, typed plan, materialization, and exact roots do not join")]
    TransformationJoinMismatch,
}

fn evaluate_plan(
    resolution: &ResolutionV1,
    plan: &ContractConsequencePlanV1,
    schemas: &SchemaClosureV1,
    base_root: &CandidateContractRootV1,
    preimage: DecisionMaterializationPreimageCommitmentV1,
) -> Result<CandidateContractRootV1, MaterializationV1Error> {
    let base_by_id = base_root
        .components()
        .iter()
        .map(|component| (*component.component_id(), component.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut components = plan
        .retained_component_ids()
        .iter()
        .map(|component_id| {
            base_by_id
                .get(component_id)
                .cloned()
                .ok_or(MaterializationV1Error::MissingRetainedComponent)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut unresolved = plan.additions().to_vec();
    let mut planned_ids = BTreeMap::<PlannedContractSlotV1, ContractComponentIdV1>::new();
    while !unresolved.is_empty() {
        let mut progressed = false;
        let mut deferred = Vec::new();
        for addition in unresolved {
            let mut dependencies = Vec::with_capacity(addition.dependencies().len());
            let mut ready = true;
            for dependency in addition.dependencies() {
                match dependency {
                    PlannedContractDependencyV1::Retained(component_id) => {
                        dependencies.push(*component_id);
                    }
                    PlannedContractDependencyV1::Planned(slot) => {
                        if let Some(component_id) = planned_ids.get(slot) {
                            dependencies.push(*component_id);
                        } else {
                            ready = false;
                            break;
                        }
                    }
                }
            }
            if !ready {
                deferred.push(addition);
                continue;
            }
            dependencies.sort_unstable();
            let component = CandidateContractComponentV1::new(
                schemas,
                addition.kind(),
                *addition.schema_id(),
                addition.value().clone(),
                dependencies,
                ComponentProvenanceV1::decision_materialization_preimage(
                    *resolution.resolution_id(),
                    preimage,
                ),
            )
            .map_err(|_| MaterializationV1Error::InvalidCandidateComponent)?;
            planned_ids.insert(addition.slot(), *component.component_id());
            components.push(component);
            progressed = true;
        }
        if !progressed {
            return Err(MaterializationV1Error::UnresolvedPlannedDependency);
        }
        unresolved = deferred;
    }
    CandidateContractRootV1::new(schemas, components)
        .map_err(|_| MaterializationV1Error::InvalidCandidateRoot)
}

fn derive_delta(
    base_root: &CandidateContractRootV1,
    candidate_root: &CandidateContractRootV1,
) -> ContractComponentDeltaV1 {
    let base = base_root
        .components()
        .iter()
        .map(|component| *component.component_id())
        .collect::<BTreeSet<_>>();
    let candidate = candidate_root
        .components()
        .iter()
        .map(|component| *component.component_id())
        .collect::<BTreeSet<_>>();
    ContractComponentDeltaV1 {
        added: candidate.difference(&base).copied().collect(),
        removed: base.difference(&candidate).copied().collect(),
        retained: base.intersection(&candidate).copied().collect(),
    }
}

fn materialization_preimage_commitment(
    resolution: &ResolutionV1,
    plan: &ContractConsequencePlanV1,
) -> Result<DecisionMaterializationPreimageCommitmentV1, MaterializationV1Error> {
    let value = CborValue::Array(vec![
        CborValue::Unsigned(DECISION_MATERIALIZATION_PREIMAGE_VERSION_V1),
        CborValue::Bytes(resolution.resolution_id().as_bytes().to_vec()),
        plan.canonical_value(),
    ]);
    Ok(DecisionMaterializationPreimageCommitmentV1::from_digest(
        Sha256::digest(deterministic_cbor::encode(&value)?).into(),
    ))
}

fn materialization_preflight_binding(
    resolution_id: &DecisionResolutionIdV1,
    plan_id: ContractConsequencePlanIdV1,
    preimage: &DecisionMaterializationPreimageCommitmentV1,
    base_root: &CandidateContractRootV1,
    candidate_root: &CandidateContractRootV1,
    delta: &ContractComponentDeltaV1,
) -> Result<[u8; 32], MaterializationV1Error> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Bytes(resolution_id.as_bytes().to_vec()),
            CborValue::Bytes(plan_id.as_bytes().to_vec()),
            CborValue::Bytes(preimage.as_bytes().to_vec()),
            root_components_value(base_root),
            root_components_value(candidate_root),
            delta.canonical_value(),
        ]))?)
        .into(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "immutable materialization identity commits every exact transformation and authority input"
)]
fn materialization_value(
    resolution_id: &DecisionResolutionIdV1,
    plan_id: ContractConsequencePlanIdV1,
    preimage: &DecisionMaterializationPreimageCommitmentV1,
    base_root: &CandidateContractRootV1,
    candidate_root: &CandidateContractRootV1,
    delta: &ContractComponentDeltaV1,
    disposition: &DecisionMaterializationDispositionV1,
    authorization: Option<&CommittedActionAuditV1>,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DECISION_MATERIALIZATION_VERSION_V1),
        CborValue::Bytes(resolution_id.as_bytes().to_vec()),
        CborValue::Bytes(plan_id.as_bytes().to_vec()),
        CborValue::Bytes(preimage.as_bytes().to_vec()),
        root_components_value(base_root),
        root_components_value(candidate_root),
        delta.canonical_value(),
        disposition.canonical_value(),
        CborValue::optional(authorization.map(CommittedActionAuditV1::canonical_value)),
    ])
}

fn root_components_value(root: &CandidateContractRootV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Bytes(root.root_id().as_bytes().to_vec()),
        CborValue::Array(
            root.components()
                .iter()
                .map(CandidateContractComponentV1::canonical_record_value)
                .collect(),
        ),
    ])
}

fn component_ids_value(ids: &[ContractComponentIdV1]) -> CborValue {
    CborValue::Array(
        ids.iter()
            .map(|component_id| CborValue::Bytes(component_id.as_bytes().to_vec()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use crate::domain::vnext::contract::assembly::{
        candidate_root_schema_closure_v1, facet_schema_id_v1, fixture_facet_value_v1,
        normative_inputs_schema_id_v1,
    };
    use crate::domain::vnext::contract::component_kind::ContractComponentKindV1;
    use crate::domain::vnext::contract::materialization::{
        ContractConsequencePlanV1, PlannedContractComponentV1, PlannedContractSlotV1,
    };
    use crate::domain::vnext::identity::{
        DesignRevisionIdV1, DesignSourceBindingIdV1, StoreDomainIdV1,
    };
    use crate::domain::vnext::work::WorkIdV1;

    use super::*;
    use crate::domain::vnext::design::{
        AdmittedCommittedActionV1, AlternativeConsequenceV1, AlternativeRejectionV1, AlternativeV1,
        DecisionClosureEffectStatusV1, DecisionClosureManifestV1, DecisionIdV1, DecisionLineageV1,
        DecisionRevisionV1, DecisionV1, ExactRecordRefV1, WorkDecisionEligibilityV1,
    };

    fn rendered(value: u8) -> String {
        format!("sha256:{}", format!("{value:02x}").repeat(32))
    }

    fn schema_and_root(seed: u8) -> (SchemaClosureV1, CandidateContractRootV1) {
        let schemas = candidate_root_schema_closure_v1().expect("schema closure");
        let design_revision_id =
            DesignRevisionIdV1::parse(&rendered(seed)).expect("Design Revision");
        let source_binding_id = DesignSourceBindingIdV1::parse(&rendered(seed.saturating_add(1)))
            .expect("source binding");
        let components = ContractComponentKindV1::ALL
            .into_iter()
            .map(|kind| {
                let (schema_id, value) = if kind == ContractComponentKindV1::NormativeInputs {
                    (
                        normative_inputs_schema_id_v1(&schemas).expect("normative schema"),
                        CborValue::Array(vec![
                            CborValue::Unsigned(1),
                            CborValue::Bytes([seed; 32].to_vec()),
                            CborValue::Array(Vec::new()),
                        ]),
                    )
                } else {
                    (
                        facet_schema_id_v1(&schemas, kind).expect("facet schema"),
                        fixture_facet_value_v1(kind, [seed; 32], vec![[seed; 32]]),
                    )
                };
                CandidateContractComponentV1::new(
                    &schemas,
                    kind,
                    schema_id,
                    value,
                    vec![],
                    ComponentProvenanceV1::design_slot(
                        design_revision_id,
                        kind.tag(),
                        source_binding_id,
                    )
                    .expect("provenance"),
                )
                .expect("component")
            })
            .collect();
        let root = CandidateContractRootV1::new(&schemas, components).expect("root");
        (schemas, root)
    }

    fn changed_plan(
        schemas: &SchemaClosureV1,
        base: &CandidateContractRootV1,
    ) -> ContractConsequencePlanV1 {
        let replaced = base
            .components()
            .iter()
            .find(|component| component.kind() == ContractComponentKindV1::IntendedOutcome)
            .expect("replaced component");
        let retained = base
            .components()
            .iter()
            .filter(|component| component.component_id() != replaced.component_id())
            .map(|component| *component.component_id())
            .collect();
        let addition = PlannedContractComponentV1::new(
            PlannedContractSlotV1::new(1).expect("slot"),
            ContractComponentKindV1::IntendedOutcome,
            facet_schema_id_v1(schemas, ContractComponentKindV1::IntendedOutcome)
                .expect("facet schema"),
            fixture_facet_value_v1(ContractComponentKindV1::IntendedOutcome, [99; 32], vec![]),
            vec![],
        )
        .expect("addition");
        ContractConsequencePlanV1::new(7, base, retained, vec![addition]).expect("changed plan")
    }

    fn resolved_decision(plan: ContractConsequencePlanV1) -> DecisionV1 {
        resolved_decision_with_id("materialization-decision", plan)
    }

    fn resolved_decision_with_id(id: &str, plan: ContractConsequencePlanV1) -> DecisionV1 {
        let decision_id = DecisionIdV1::new(id).expect("Decision id");
        let alternatives = vec![
            AlternativeV1::new(
                b"no effect".to_vec(),
                b"no Contract effect".to_vec(),
                AlternativeConsequenceV1::NoContractEffect,
            )
            .expect("no-effect alternative"),
            AlternativeV1::new(
                b"apply exact plan".to_vec(),
                b"derive candidate closure".to_vec(),
                AlternativeConsequenceV1::typed_plan(plan.clone()),
            )
            .expect("plan alternative"),
        ];
        let revision = DecisionRevisionV1::new(
            decision_id.clone(),
            1,
            None,
            b"which exact transformation?".to_vec(),
            ExactRecordRefV1::from_digest([31; 32]),
            *plan.base_root_id(),
            alternatives,
        )
        .expect("revision");
        let selected = *revision.alternatives()[1].alternative_id();
        let rejected = AlternativeRejectionV1::new(
            *revision.alternatives()[0].alternative_id(),
            b"the exact transformation is required".to_vec(),
        )
        .expect("rejection");
        let decision = DecisionV1::new(
            StoreDomainIdV1::parse(&rendered(200)).expect("Store domain"),
            WorkIdV1::derive("materialization-work").expect("Work"),
            decision_id,
            revision,
        )
        .expect("Decision");
        let authorization = AdmittedCommittedActionV1::fixture("resolution");
        decision
            .resolve(
                decision.head().revision_id(),
                &selected,
                b"apply the selected exact plan".to_vec(),
                vec![rejected],
                &authorization,
                WorkDecisionEligibilityV1::Eligible,
            )
            .expect("Resolution")
    }

    #[test]
    fn equal_root_is_detected_before_authority_and_needs_none() {
        let (schemas, base) = schema_and_root(11);
        let retained = base
            .components()
            .iter()
            .map(|component| *component.component_id())
            .collect();
        let plan = ContractConsequencePlanV1::new(7, &base, retained, vec![]).expect("equal plan");
        let decision = resolved_decision(plan);
        let preflight = DecisionMaterializationV1::preflight(
            decision.resolution().expect("Resolution"),
            &schemas,
            &base,
        )
        .expect("preflight");
        assert!(preflight.is_equal_root());
        let materialization = preflight.complete_equal_root().expect("equal no-op");
        assert!(materialization.authorization().is_none());
        assert!(materialization.delta().added().is_empty());
        assert!(materialization.delta().removed().is_empty());
    }

    #[test]
    fn exact_plan_derives_delta_candidate_root_and_preimage_provenance() {
        let (schemas, base) = schema_and_root(21);
        let decision = resolved_decision(changed_plan(&schemas, &base));
        let resolution = decision.resolution().expect("Resolution");
        let preflight =
            DecisionMaterializationV1::preflight(resolution, &schemas, &base).expect("preflight");
        assert!(!preflight.is_equal_root());
        assert_eq!(preflight.delta().added().len(), 1);
        assert_eq!(preflight.delta().removed().len(), 1);

        let missing = AdmittedMaterializationAuthorityV1::from_store_commit(
            AdmittedCommittedActionV1::fixture("amendment-missing"),
            &preflight,
            vec![],
        );
        assert!(matches!(
            missing,
            Err(MaterializationV1Error::IncompleteInvalidationReceipts)
        ));

        let removed = preflight.delta().removed()[0];
        let authority = AdmittedMaterializationAuthorityV1::from_store_commit(
            AdmittedCommittedActionV1::fixture("amendment"),
            &preflight,
            vec![(
                removed,
                StoreObjectIdV1::parse(&rendered(77)).expect("invalidation receipt Object"),
            )],
        )
        .expect("admitted amendment authority");
        let materialization = preflight
            .complete_with_authority(authority)
            .expect("materialization");
        materialization
            .verify_exact_transformation(resolution, &schemas)
            .expect("exact transformation join");
        let completed_provenance = materialization
            .verified_component_provenance(resolution, &schemas)
            .expect("verified completed provenance");
        assert_eq!(completed_provenance.variant_tag(), 3);
        assert!(matches!(
            &completed_provenance,
            ComponentProvenanceV1::DecisionMaterialization(provenance)
                if provenance.resolution_id() == resolution.resolution_id()
                    && provenance.materialization_id() == materialization.materialization_id()
        ));
        let mut expected_provenance_bytes = vec![0x83, 0x03, 0x58, 0x20];
        expected_provenance_bytes.extend_from_slice(resolution.resolution_id().as_bytes());
        expected_provenance_bytes.extend_from_slice(&[0x58, 0x20]);
        expected_provenance_bytes
            .extend_from_slice(materialization.materialization_id().as_bytes());
        assert_eq!(
            deterministic_cbor::encode(&completed_provenance.canonical_value())
                .expect("completed provenance CBOR"),
            expected_provenance_bytes
        );
        let unrelated_decision = resolved_decision_with_id(
            "unrelated-materialization-decision",
            changed_plan(&schemas, &base),
        );
        assert!(matches!(
            materialization.verified_component_provenance(
                unrelated_decision.resolution().expect("Resolution"),
                &schemas,
            ),
            Err(MaterializationV1Error::TransformationJoinMismatch)
        ));
        assert!(materialization.authorization().is_some());
        assert_ne!(
            materialization.base_root_id(),
            materialization.candidate_root_id()
        );
        assert!(
            materialization
                .candidate_root()
                .components()
                .iter()
                .any(|component| {
                    matches!(
                        component.provenance(),
                        ComponentProvenanceV1::DecisionMaterializationPreimage(provenance)
                            if provenance.resolution_id() == resolution.resolution_id()
                                && provenance.commitment() == materialization.preimage_commitment()
                    )
                })
        );

        let closure = DecisionClosureManifestV1::close(
            WorkIdV1::derive("materialization-work").expect("Work"),
            vec![decision.decision_id().clone()],
            vec![decision],
            &DecisionLineageV1::new(WorkIdV1::derive("materialization-work").expect("Work")),
            vec![materialization.clone()],
            &schemas,
            &base,
        )
        .expect("recomputed Decision closure");
        assert_eq!(
            closure.candidate_contract_root_id(),
            materialization.candidate_root_id()
        );
    }

    #[test]
    fn equivalence_only_materialization_stays_on_the_exact_base_root() {
        let (schemas, base) = schema_and_root(41);
        let decision = resolved_decision_with_id(
            "equivalent-materialization-decision",
            changed_plan(&schemas, &base),
        );
        let resolution = decision.resolution().expect("Resolution");
        let preflight =
            DecisionMaterializationV1::preflight(resolution, &schemas, &base).expect("preflight");
        let materialization = preflight
            .complete_exact_equivalent(
                StoreObjectIdV1::parse(&rendered(78)).expect("Equivalence Receipt Object"),
            )
            .expect("equivalent materialization");
        assert!(materialization.authorization().is_none());
        let decision_id = decision.decision_id().clone();

        let closure = DecisionClosureManifestV1::close(
            WorkIdV1::derive("materialization-work").expect("Work"),
            vec![decision_id.clone()],
            vec![decision],
            &DecisionLineageV1::new(WorkIdV1::derive("materialization-work").expect("Work")),
            vec![materialization],
            &schemas,
            &base,
        )
        .expect("equivalence-only Decision closure");

        assert_eq!(closure.candidate_contract_root_id(), base.root_id());
        assert_eq!(
            closure.effect_status(&decision_id),
            Some(DecisionClosureEffectStatusV1::AppliedEquivalentRoot)
        );
    }

    #[test]
    fn equivalence_no_op_is_consumed_before_a_real_change_from_the_same_base() {
        let (schemas, base) = schema_and_root(51);
        let equivalent_decision =
            resolved_decision_with_id("equivalent-before-change", changed_plan(&schemas, &base));
        let changed_decision =
            resolved_decision_with_id("real-change", changed_plan(&schemas, &base));

        let equivalent_preflight = DecisionMaterializationV1::preflight(
            equivalent_decision.resolution().expect("Resolution"),
            &schemas,
            &base,
        )
        .expect("equivalence preflight");
        let equivalent_materialization = equivalent_preflight
            .complete_exact_equivalent(
                StoreObjectIdV1::parse(&rendered(79)).expect("Equivalence Receipt Object"),
            )
            .expect("equivalent materialization");
        assert!(equivalent_materialization.authorization().is_none());

        let changed_preflight = DecisionMaterializationV1::preflight(
            changed_decision.resolution().expect("Resolution"),
            &schemas,
            &base,
        )
        .expect("changed preflight");
        let removed = changed_preflight.delta().removed()[0];
        let changed_authority = AdmittedMaterializationAuthorityV1::from_store_commit(
            AdmittedCommittedActionV1::fixture("real-change"),
            &changed_preflight,
            vec![(
                removed,
                StoreObjectIdV1::parse(&rendered(80)).expect("invalidation receipt Object"),
            )],
        )
        .expect("admitted change authority");
        let changed_materialization = changed_preflight
            .complete_with_authority(changed_authority)
            .expect("changed materialization");
        let changed_root_id = *changed_materialization.candidate_root_id();
        let equivalent_decision_id = equivalent_decision.decision_id().clone();
        let changed_decision_id = changed_decision.decision_id().clone();

        let closure = DecisionClosureManifestV1::close(
            WorkIdV1::derive("materialization-work").expect("Work"),
            vec![equivalent_decision_id.clone(), changed_decision_id.clone()],
            vec![equivalent_decision, changed_decision],
            &DecisionLineageV1::new(WorkIdV1::derive("materialization-work").expect("Work")),
            vec![equivalent_materialization, changed_materialization],
            &schemas,
            &base,
        )
        .expect("equivalence-plus-change Decision closure");

        assert_eq!(closure.candidate_contract_root_id(), &changed_root_id);
        assert_eq!(
            closure.effect_status(&equivalent_decision_id),
            Some(DecisionClosureEffectStatusV1::Historical)
        );
        assert_eq!(
            closure.effect_status(&changed_decision_id),
            Some(DecisionClosureEffectStatusV1::AppliedExactRoot)
        );
    }
}
