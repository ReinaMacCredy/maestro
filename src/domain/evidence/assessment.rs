use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::authority::{
    ActionRequestIdV1, AdmittedRepositoryActionV1, AuthorizationReceiptIdV1,
    AuthorizationReceiptV1, RepositoryActionLeafV1, TrustedTimeV1,
};
use crate::domain::contract::runtime::ContractGenerationIdV1;
use crate::domain::gate::{
    GateError, GateEvaluationInputV1, GateEvaluationResultV1, GateEvaluatorContractIdV1,
    GateEvaluatorContractV1, GateEvaluatorDefinitionV1, GateInputClassV1, GateLeafRuleV1,
    GateNodeIdV1, GateOperatorV1, GateScopeV1, GateSnapshotIdV1, GateSnapshotV1,
    PureGateEvaluatorV1,
};
use crate::domain::identity::{
    ContractRootIdV1, StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};
use crate::domain::step::{StepBindingV1, StepIdV1, StepRevisionIdV1, StepScopeV1};
use crate::domain::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::claim::{
    ClaimSubjectV1, ClaimV1, parse_claim_subject, parse_submission_ref,
    validate_claim_observation_subjects,
};
use super::identity::{
    AssessmentIdV1, AssessmentInvalidationIdV1, ClaimIdV1, EvidenceIdentityError,
    ObservationRecordIdV1, domain_hash, require_nonzero,
};
use super::observation::{
    ObservationError, ObservationKindV1, ObservationSubjectKindV1, ObservationV1,
};

pub const ASSESSMENT_RECORD_VERSION_V1: u64 = 1;
pub const ASSESSMENT_RECORD_DOMAIN_V1: &str = "maestro.vnext.evidence.assessment-record.v1";
const MAX_ASSESSMENT_INPUTS_V1: usize = 8_192;

#[derive(Debug, Eq, PartialEq)]
struct SupportBindingsV1 {
    contributors: Vec<[u8; 32]>,
    roots: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservationAssessmentInputV1 {
    observation_id: ObservationRecordIdV1,
    record_hash: [u8; 32],
    kind: ObservationKindV1,
    payload_semantic_hash: [u8; 32],
    store_domain_id: StoreDomainIdV1,
    observed_at: u64,
    payload_object_id: crate::domain::identity::StoreObjectIdV1,
    subjects: Vec<super::observation::ObservationSubjectV1>,
    contributor_hash: [u8; 32],
    support_roots: Vec<[u8; 32]>,
}

impl ObservationAssessmentInputV1 {
    pub fn from_observation(observation: &ObservationV1) -> Result<Self, AssessmentError> {
        validate_observation_subject_cardinality(observation.subjects())?;
        let contributor_hash = domain_hash(
            "maestro.vnext.evidence.observation-contributor.v1",
            &observation.producer().canonical_value(),
        )?;
        let mut support_roots = observation
            .lineage()
            .iter()
            .map(|id| *id.as_bytes())
            .collect::<Vec<_>>();
        support_roots.push(*observation.id().as_bytes());
        support_roots.sort_unstable();
        support_roots.dedup();
        Ok(Self {
            observation_id: observation.id(),
            record_hash: *observation.record_hash(),
            kind: observation.kind(),
            payload_semantic_hash: *observation.payload().semantic_hash(),
            store_domain_id: observation.store_domain_id(),
            observed_at: observation.observed_at(),
            payload_object_id: observation.payload().object_id(),
            subjects: observation.subjects().to_vec(),
            contributor_hash,
            support_roots,
        })
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.observation_id.as_bytes()),
            bytes(&self.record_hash),
            CborValue::Unsigned(self.kind.tag()),
            bytes(&self.payload_semantic_hash),
            bytes(self.store_domain_id.as_bytes()),
            CborValue::Unsigned(self.observed_at),
            bytes(self.payload_object_id.as_bytes()),
            CborValue::Array(
                self.subjects
                    .iter()
                    .map(|subject| subject.canonical_value())
                    .collect(),
            ),
            bytes(&self.contributor_hash),
            CborValue::Array(self.support_roots.iter().map(bytes).collect()),
        ])
    }

    fn step_subject(&self) -> Option<([u8; 32], StepRevisionIdV1)> {
        self.subjects
            .iter()
            .find(|subject| subject.kind() == ObservationSubjectKindV1::Step)
            .and_then(|subject| {
                StepRevisionIdV1::from_bytes(*subject.revision_id())
                    .ok()
                    .map(|revision| (*subject.subject_id(), revision))
            })
    }

    fn work_subject(
        &self,
    ) -> Option<(
        StoreDomainIdV1,
        [u8; 32],
        ContractGenerationIdV1,
        ContractRootIdV1,
    )> {
        let work = self
            .subjects
            .iter()
            .find(|subject| subject.kind() == ObservationSubjectKindV1::Work)
            .and_then(|subject| {
                Some((
                    *subject.subject_id(),
                    subject.contract_generation_id()?,
                    ContractRootIdV1::from_digest(*subject.revision_id()),
                ))
            })?;
        let repository_matches = self
            .subjects
            .iter()
            .filter(|subject| subject.kind() == ObservationSubjectKindV1::Repository)
            .filter(|subject| {
                subject.subject_id() == self.store_domain_id.as_bytes()
                    && subject.revision_id() == work.1.as_bytes()
            })
            .count();
        (repository_matches == 1).then_some((self.store_domain_id, work.0, work.1, work.2))
    }

    pub(crate) const fn observation_id(&self) -> ObservationRecordIdV1 {
        self.observation_id
    }
}

fn validate_observation_subject_cardinality(
    subjects: &[super::observation::ObservationSubjectV1],
) -> Result<(), AssessmentError> {
    let count = |kind| {
        subjects
            .iter()
            .filter(|subject| subject.kind() == kind)
            .count()
    };
    if count(ObservationSubjectKindV1::Step) > 1 {
        return Err(AssessmentError::AmbiguousStepScope);
    }
    if count(ObservationSubjectKindV1::Work) > 1 {
        return Err(AssessmentError::AmbiguousWorkScope);
    }
    if count(ObservationSubjectKindV1::Submission) > 1
        || count(ObservationSubjectKindV1::Repository) > 1
    {
        return Err(AssessmentError::InvalidInputs);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ClaimObservationScopeV1 {
    observation_id: ObservationRecordIdV1,
    subjects: Vec<super::observation::ObservationSubjectV1>,
}

impl ClaimObservationScopeV1 {
    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.observation_id.as_bytes()),
            CborValue::Array(
                self.subjects
                    .iter()
                    .map(|subject| subject.canonical_value())
                    .collect(),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClaimAssessmentInputV1 {
    claim_id: ClaimIdV1,
    record_hash: [u8; 32],
    store_domain_id: StoreDomainIdV1,
    submission: super::claim::SubmissionRefV1,
    observation_ids: Vec<ObservationRecordIdV1>,
    oldest_observed_at: u64,
    payload_object_ids: Vec<crate::domain::identity::StoreObjectIdV1>,
    observation_scopes: Vec<ClaimObservationScopeV1>,
    contributor_hashes: Vec<[u8; 32]>,
    support_roots: Vec<[u8; 32]>,
    subject: ClaimSubjectV1,
}

impl ClaimAssessmentInputV1 {
    pub fn from_claim(
        claim: &ClaimV1,
        observations: &[&ObservationV1],
    ) -> Result<Self, AssessmentError> {
        let mut resolved = observations
            .iter()
            .map(|observation| (observation.id(), observation.observed_at()))
            .collect::<Vec<_>>();
        resolved.sort_unstable_by_key(|(id, _)| *id);
        let resolved_ids = resolved.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        if resolved_ids != claim.observation_refs() {
            return Err(AssessmentError::UnresolvedInput);
        }
        for observation in observations {
            validate_claim_observation_subjects(
                claim.submission(),
                claim.subject(),
                observation.store_domain_id(),
                observation.subjects(),
            )
            .map_err(|_| AssessmentError::WorkScopeMismatch)?;
        }
        let oldest_observed_at = resolved
            .iter()
            .map(|(_, observed_at)| *observed_at)
            .min()
            .ok_or(AssessmentError::UnresolvedInput)?;
        let store_domains = observations
            .iter()
            .map(|observation| observation.store_domain_id())
            .collect::<BTreeSet<_>>();
        if store_domains.len() != 1 {
            return Err(AssessmentError::CrossStoreInput);
        }
        let mut payload_object_ids = observations
            .iter()
            .map(|observation| observation.payload().object_id())
            .collect::<Vec<_>>();
        payload_object_ids.sort_unstable();
        payload_object_ids.dedup();
        let mut observation_scopes = observations
            .iter()
            .map(|observation| ClaimObservationScopeV1 {
                observation_id: observation.id(),
                subjects: observation.subjects().to_vec(),
            })
            .collect::<Vec<_>>();
        observation_scopes.sort_unstable_by_key(|scope| scope.observation_id);
        let mut contributor_hashes = Vec::with_capacity(observations.len());
        let mut support_roots = Vec::new();
        for observation in observations {
            let input = ObservationAssessmentInputV1::from_observation(observation)?;
            contributor_hashes.push(input.contributor_hash);
            support_roots.extend(input.support_roots);
        }
        contributor_hashes.sort_unstable();
        contributor_hashes.dedup();
        support_roots.sort_unstable();
        support_roots.dedup();
        let value = Self {
            claim_id: claim.claim_id(),
            record_hash: *claim.record_hash(),
            store_domain_id: *store_domains
                .first()
                .expect("invariant: one Store Domain exists"),
            submission: claim.submission(),
            observation_ids: resolved_ids,
            oldest_observed_at,
            payload_object_ids,
            observation_scopes,
            contributor_hashes,
            support_roots,
            subject: claim.subject().clone(),
        };
        value.validate_scope_commitments()?;
        Ok(value)
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.claim_id.as_bytes()),
            bytes(&self.record_hash),
            bytes(self.store_domain_id.as_bytes()),
            self.submission.canonical_value(),
            CborValue::Array(
                self.observation_ids
                    .iter()
                    .map(|id| bytes(id.as_bytes()))
                    .collect(),
            ),
            CborValue::Unsigned(self.oldest_observed_at),
            CborValue::Array(
                self.payload_object_ids
                    .iter()
                    .map(|id| bytes(id.as_bytes()))
                    .collect(),
            ),
            CborValue::Array(
                self.observation_scopes
                    .iter()
                    .map(ClaimObservationScopeV1::canonical_value)
                    .collect(),
            ),
            CborValue::Array(self.contributor_hashes.iter().map(bytes).collect()),
            CborValue::Array(self.support_roots.iter().map(bytes).collect()),
            self.subject.canonical_value(),
        ])
    }

    fn validate_scope_commitments(&self) -> Result<(), AssessmentError> {
        if self
            .observation_scopes
            .iter()
            .map(|scope| scope.observation_id)
            .collect::<Vec<_>>()
            != self.observation_ids
        {
            return Err(AssessmentError::InvalidInputs);
        }
        validate_support_set(&self.contributor_hashes)?;
        validate_support_set(&self.support_roots)?;
        for scope in &self.observation_scopes {
            validate_claim_observation_subjects(
                self.submission,
                &self.subject,
                self.store_domain_id,
                &scope.subjects,
            )
            .map_err(|_| AssessmentError::WorkScopeMismatch)?;
        }
        Ok(())
    }

    pub(crate) const fn claim_id(&self) -> ClaimIdV1 {
        self.claim_id
    }

    fn work_subject(
        &self,
    ) -> Option<(
        StoreDomainIdV1,
        [u8; 32],
        ContractGenerationIdV1,
        ContractRootIdV1,
    )> {
        let mut subjects = self
            .observation_scopes
            .iter()
            .flat_map(|scope| scope.subjects.iter())
            .filter(|subject| subject.kind() == super::observation::ObservationSubjectKindV1::Work)
            .filter_map(|subject| {
                Some((
                    self.store_domain_id,
                    *subject.subject_id(),
                    subject.contract_generation_id()?,
                    ContractRootIdV1::parse(&render_assessment_digest(*subject.revision_id()))
                        .ok()?,
                ))
            })
            .collect::<BTreeSet<_>>();
        if subjects.len() == 1 {
            subjects.pop_first()
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AuthorizationAssessmentInputV1 {
    receipt_id: crate::domain::authority::AuthorizationReceiptIdV1,
    receipt_hash: [u8; 32],
    request_id: ActionRequestIdV1,
    context_id: crate::domain::authority::AuthorityContextIdV1,
    basis_kind: u8,
    prior_state_token: crate::domain::authority::StateTokenIdV1,
    resulting_state_token: crate::domain::authority::StateTokenIdV1,
    authority_snapshot_hash: [u8; 32],
    subject_hash: [u8; 32],
    validated_at: u64,
    valid_until: u64,
    step_binding: Option<StepBindingV1>,
}

impl AuthorizationAssessmentInputV1 {
    pub(crate) fn from_validated_receipt(
        receipt: &AuthorizationReceiptV1,
        authority_snapshot_hash: [u8; 32],
        subject_hash: [u8; 32],
        validated_at: u64,
        valid_until: u64,
        step_binding: Option<StepBindingV1>,
    ) -> Result<Self, AssessmentError> {
        require_nonzero(authority_snapshot_hash, "Assessment Authority snapshot")?;
        require_nonzero(subject_hash, "Assessment Authority subject")?;
        if validated_at == 0 || valid_until <= validated_at {
            return Err(AssessmentError::InvalidTimeWindow);
        }
        let receipt_hash = domain_hash(
            "maestro.vnext.evidence.authorization-assessment-input.v1",
            &CborValue::Bytes(receipt.canonical_bytes()?),
        )?;
        Ok(Self {
            receipt_id: receipt.id(),
            receipt_hash,
            request_id: receipt.request_id(),
            context_id: receipt.context_id(),
            basis_kind: receipt.basis_kind() as u8,
            prior_state_token: receipt.prior_state_token(),
            resulting_state_token: receipt.resulting_state_token(),
            authority_snapshot_hash,
            subject_hash,
            validated_at,
            valid_until,
            step_binding,
        })
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.receipt_id.as_bytes()),
            bytes(&self.receipt_hash),
            bytes(self.request_id.as_bytes()),
            bytes(self.context_id.as_bytes()),
            CborValue::Unsigned(self.basis_kind.into()),
            bytes(self.prior_state_token.as_bytes()),
            bytes(self.resulting_state_token.as_bytes()),
            bytes(&self.authority_snapshot_hash),
            bytes(&self.subject_hash),
            CborValue::Unsigned(self.validated_at),
            CborValue::Unsigned(self.valid_until),
            CborValue::optional(
                self.step_binding
                    .map(|binding| AssessmentScopeV1::Step(binding).canonical_value()),
            ),
        ])
    }

    pub(crate) fn exact_receipt(&self) -> Result<AuthorizationReceiptV1, AssessmentError> {
        let basis_kind =
            crate::domain::authority::ActionAuthorityBasisKindV1::try_from(self.basis_kind)
                .map_err(|_| AssessmentError::InvalidStoredAssessment)?;
        let receipt = AuthorizationReceiptV1::new(
            self.request_id,
            self.context_id,
            basis_kind,
            self.prior_state_token,
            self.resulting_state_token,
        )?;
        let receipt_hash = domain_hash(
            "maestro.vnext.evidence.authorization-assessment-input.v1",
            &CborValue::Bytes(receipt.canonical_bytes()?),
        )?;
        if receipt.id() != self.receipt_id || receipt_hash != self.receipt_hash {
            return Err(AssessmentError::InvalidStoredAssessment);
        }
        Ok(receipt)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ChildAssessmentResolutionV1 {
    gate_id: GateNodeIdV1,
    assessment_ids: Vec<AssessmentIdV1>,
    resolution_hash: [u8; 32],
    result: GateEvaluationResultV1,
    valid_until: Option<u64>,
    as_of: u64,
    time_basis_hash: [u8; 32],
    contributor_hashes: Vec<[u8; 32]>,
    support_roots: Vec<[u8; 32]>,
}

impl ChildAssessmentResolutionV1 {
    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.gate_id.as_bytes()),
            CborValue::Array(
                self.assessment_ids
                    .iter()
                    .map(|id| bytes(id.as_bytes()))
                    .collect(),
            ),
            bytes(&self.resolution_hash),
            CborValue::Unsigned(self.result.tag()),
            CborValue::optional(self.valid_until.map(CborValue::Unsigned)),
            CborValue::Unsigned(self.as_of),
            bytes(&self.time_basis_hash),
            CborValue::Array(self.contributor_hashes.iter().map(bytes).collect()),
            CborValue::Array(self.support_roots.iter().map(bytes).collect()),
        ])
    }

    pub(crate) fn assessment_ids(&self) -> &[AssessmentIdV1] {
        &self.assessment_ids
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AssessmentInputRefV1 {
    Observation(ObservationAssessmentInputV1),
    Claim(ClaimAssessmentInputV1),
    AuthorizationReceipt(AuthorizationAssessmentInputV1),
    ChildResolution(ChildAssessmentResolutionV1),
}

impl AssessmentInputRefV1 {
    fn tag(&self) -> u64 {
        match self {
            Self::Observation(_) => 1,
            Self::Claim(_) => 2,
            Self::AuthorizationReceipt(_) => 3,
            Self::ChildResolution(_) => 4,
        }
    }

    fn canonical_value(&self) -> CborValue {
        let value = match self {
            Self::Observation(value) => value.canonical_value(),
            Self::Claim(value) => value.canonical_value(),
            Self::AuthorizationReceipt(value) => value.canonical_value(),
            Self::ChildResolution(value) => value.canonical_value(),
        };
        CborValue::Array(vec![CborValue::Unsigned(self.tag()), value])
    }

    fn step_binding(&self) -> Option<StepBindingV1> {
        match self {
            Self::Observation(_) => None,
            Self::Claim(value) => match &value.subject {
                ClaimSubjectV1::Step { binding, .. } => Some(*binding),
                ClaimSubjectV1::Work { .. } => None,
            },
            Self::AuthorizationReceipt(value) => value.step_binding,
            Self::ChildResolution(_) => None,
        }
    }

    fn observation_step_subject(&self) -> Option<([u8; 32], StepRevisionIdV1)> {
        match self {
            Self::Observation(value) => value.step_subject(),
            Self::Claim(_) | Self::AuthorizationReceipt(_) | Self::ChildResolution(_) => None,
        }
    }

    fn observation_work_subject(
        &self,
    ) -> Option<(
        StoreDomainIdV1,
        [u8; 32],
        ContractGenerationIdV1,
        ContractRootIdV1,
    )> {
        match self {
            Self::Observation(value) => value.work_subject(),
            Self::Claim(_) | Self::AuthorizationReceipt(_) | Self::ChildResolution(_) => None,
        }
    }

    fn freshness_anchor(&self) -> Option<u64> {
        match self {
            Self::Observation(value) => Some(value.observed_at),
            Self::Claim(value) => Some(value.oldest_observed_at),
            Self::AuthorizationReceipt(_) | Self::ChildResolution(_) => None,
        }
    }

    fn independent_valid_until(&self) -> Option<u64> {
        match self {
            Self::AuthorizationReceipt(value) => Some(value.valid_until),
            Self::ChildResolution(value) => value.valid_until,
            Self::Observation(_) | Self::Claim(_) => None,
        }
    }

    fn store_domain_id(&self) -> Option<StoreDomainIdV1> {
        match self {
            Self::Observation(value) => Some(value.store_domain_id),
            Self::Claim(value) => Some(value.store_domain_id),
            Self::AuthorizationReceipt(_) | Self::ChildResolution(_) => None,
        }
    }

    fn references_payload(
        &self,
        payload_object_id: crate::domain::identity::StoreObjectIdV1,
    ) -> bool {
        match self {
            Self::Observation(value) => value.payload_object_id == payload_object_id,
            Self::Claim(value) => value.payload_object_ids.contains(&payload_object_id),
            Self::AuthorizationReceipt(_) | Self::ChildResolution(_) => false,
        }
    }

    pub(crate) fn child_assessment_ids(&self) -> &[AssessmentIdV1] {
        match self {
            Self::ChildResolution(value) => &value.assessment_ids,
            Self::Observation(_) | Self::Claim(_) | Self::AuthorizationReceipt(_) => &[],
        }
    }

    fn support_bindings(&self) -> Result<SupportBindingsV1, AssessmentError> {
        let (contributors, roots) = match self {
            Self::Observation(value) => (vec![value.contributor_hash], value.support_roots.clone()),
            Self::Claim(value) => (
                value.contributor_hashes.clone(),
                value.support_roots.clone(),
            ),
            Self::AuthorizationReceipt(value) => (
                vec![domain_hash(
                    "maestro.vnext.evidence.authorization-contributor.v1",
                    &CborValue::Array(vec![
                        bytes(value.context_id.as_bytes()),
                        bytes(value.receipt_id.as_bytes()),
                    ]),
                )?],
                vec![domain_hash(
                    "maestro.vnext.evidence.authorization-support-root.v1",
                    &value.canonical_value(),
                )?],
            ),
            Self::ChildResolution(value) => (
                value.contributor_hashes.clone(),
                value.support_roots.clone(),
            ),
        };
        validate_support_set(&contributors)?;
        validate_support_set(&roots)?;
        Ok(SupportBindingsV1 {
            contributors,
            roots,
        })
    }
}

fn validate_support_set(values: &[[u8; 32]]) -> Result<(), AssessmentError> {
    if values.is_empty()
        || values.contains(&[0; 32])
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(AssessmentError::InvalidSupportIndependence);
    }
    Ok(())
}

fn quorum_supports_are_independent(
    inputs: &[AssessmentInputRefV1],
) -> Result<bool, AssessmentError> {
    let mut contributors = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for input in inputs {
        let AssessmentInputRefV1::ChildResolution(_) = input else {
            return Err(AssessmentError::InvalidInputClass);
        };
        let child_support = input.support_bindings()?;
        if child_support
            .contributors
            .into_iter()
            .any(|contributor| !contributors.insert(contributor))
            || child_support
                .roots
                .into_iter()
                .any(|root| !roots.insert(root))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the closed scope value keeps exact inline contract identity without heap-backed semantics"
)]
pub enum AssessmentScopeV1 {
    Work,
    Step(StepBindingV1),
}

impl AssessmentScopeV1 {
    fn canonical_value(self) -> CborValue {
        match self {
            Self::Work => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::Step(binding) => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(binding.scope().repository_id().as_bytes()),
                bytes(binding.scope().work_id().as_bytes()),
                bytes(binding.contract_generation_id().as_bytes()),
                bytes(binding.contract_root_id().as_bytes()),
                bytes(binding.step_id().as_bytes()),
                bytes(binding.revision_id().as_bytes()),
            ]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssessmentTimeBasisV1 {
    input_store_generation_id: StoreGenerationIdV1,
    lower_bound: u64,
    upper_bound: u64,
    freshness_basis_hash: [u8; 32],
    evidence_input_cut_hash: [u8; 32],
    complete_input_cut_hash: [u8; 32],
}

impl AssessmentTimeBasisV1 {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Evidence Store assessment admission is added in this Stage 5 slice"
        )
    )]
    pub(crate) fn from_evidence_cut(
        cut: &EvidenceCutV1,
        trusted_time: TrustedTimeV1,
        freshness_basis_hash: [u8; 32],
    ) -> Result<Self, AssessmentError> {
        require_nonzero(freshness_basis_hash, "Assessment freshness basis")?;
        let TrustedTimeV1::Verified {
            lower_bound,
            upper_bound,
        } = trusted_time
        else {
            return Err(AssessmentError::TrustedTimeUnavailable);
        };
        if lower_bound == 0 || upper_bound < lower_bound {
            return Err(AssessmentError::InvalidTimeWindow);
        }
        Ok(Self {
            input_store_generation_id: cut.store_generation_id,
            lower_bound,
            upper_bound,
            freshness_basis_hash,
            evidence_input_cut_hash: cut.evidence_input_cut_hash,
            complete_input_cut_hash: cut.complete_cut_hash,
        })
    }

    pub const fn as_of(self) -> u64 {
        self.upper_bound
    }

    fn time_basis_hash(self) -> Result<[u8; 32], AssessmentError> {
        assessment_applicability_time_basis_hash(
            self.lower_bound,
            self.upper_bound,
            self.freshness_basis_hash,
        )
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.input_store_generation_id.as_bytes()),
            CborValue::Unsigned(self.lower_bound),
            CborValue::Unsigned(self.upper_bound),
            bytes(&self.freshness_basis_hash),
            bytes(&self.evidence_input_cut_hash),
            bytes(&self.complete_input_cut_hash),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentBasisV1 {
    pub store_domain_id: StoreDomainIdV1,
    pub scope: AssessmentScopeV1,
    pub inputs: Vec<AssessmentInputRefV1>,
    pub time: AssessmentTimeBasisV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeafGateEvaluationOutputV1 {
    result: GateEvaluationResultV1,
    diagnostic_hash: [u8; 32],
}

impl LeafGateEvaluationOutputV1 {
    pub fn new(
        result: GateEvaluationResultV1,
        diagnostic_hash: [u8; 32],
    ) -> Result<Self, AssessmentError> {
        require_nonzero(diagnostic_hash, "leaf Gate diagnostic")?;
        Ok(Self {
            result,
            diagnostic_hash,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LeafGateEvaluationContextV1<'a> {
    pub gate_id: GateNodeIdV1,
    pub parameters_hash: &'a [u8; 32],
    pub input_set_hash: &'a [u8; 32],
    pub inputs: &'a [AssessmentInputRefV1],
    pub as_of: u64,
    pub evidence_cut_hash: &'a [u8; 32],
}

mod sealed {
    pub trait PinnedLeafGateEvaluatorSealedV1 {}
}

pub trait PinnedLeafGateEvaluatorV1: sealed::PinnedLeafGateEvaluatorSealedV1 {
    fn contract_id(&self) -> GateEvaluatorContractIdV1;
    fn evaluate(
        &self,
        context: LeafGateEvaluationContextV1<'_>,
    ) -> Result<LeafGateEvaluationOutputV1, AssessmentError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedLeafGateEvaluatorV1 {
    contract: GateEvaluatorContractV1,
}

impl ClosedLeafGateEvaluatorV1 {
    pub fn new(contract: GateEvaluatorContractV1) -> Result<Self, AssessmentError> {
        if !matches!(contract.definition(), GateEvaluatorDefinitionV1::Leaf(_)) {
            return Err(AssessmentError::EvaluatorMismatch);
        }
        Ok(Self { contract })
    }

    pub fn semantic_parameters_hash(
        rule: GateLeafRuleV1,
        inputs: &[AssessmentInputRefV1],
    ) -> Result<[u8; 32], AssessmentError> {
        let expected_class = match rule {
            GateLeafRuleV1::EvidenceSemanticMatch => GateInputClassV1::Evidence,
            GateLeafRuleV1::AuthoritySemanticMatch => GateInputClassV1::Authority,
            GateLeafRuleV1::MixedSemanticMatch => GateInputClassV1::Mixed,
            GateLeafRuleV1::EvidenceSetPresent
            | GateLeafRuleV1::AuthoritySetPresent
            | GateLeafRuleV1::MixedSetPresent
            | GateLeafRuleV1::ExactInputSet => return Err(AssessmentError::EvaluatorMismatch),
        };
        let inputs = validate_inputs(expected_class, inputs.to_vec())?;
        Ok(domain_hash(
            "maestro.vnext.evidence.closed-leaf-semantic-parameters.v1",
            &CborValue::Array(vec![
                CborValue::Unsigned(rule.tag()),
                CborValue::Array(
                    inputs
                        .iter()
                        .map(AssessmentInputRefV1::canonical_value)
                        .collect(),
                ),
            ]),
        )?)
    }
}

impl sealed::PinnedLeafGateEvaluatorSealedV1 for ClosedLeafGateEvaluatorV1 {}

impl PinnedLeafGateEvaluatorV1 for ClosedLeafGateEvaluatorV1 {
    fn contract_id(&self) -> GateEvaluatorContractIdV1 {
        self.contract.id()
    }

    fn evaluate(
        &self,
        context: LeafGateEvaluationContextV1<'_>,
    ) -> Result<LeafGateEvaluationOutputV1, AssessmentError> {
        let GateEvaluatorDefinitionV1::Leaf(rule) = self.contract.definition() else {
            return Err(AssessmentError::EvaluatorMismatch);
        };
        let evidence = context.inputs.iter().any(|input| {
            matches!(
                input,
                AssessmentInputRefV1::Observation(_) | AssessmentInputRefV1::Claim(_)
            )
        });
        let authority = context
            .inputs
            .iter()
            .any(|input| matches!(input, AssessmentInputRefV1::AuthorizationReceipt(_)));
        let result = match rule {
            GateLeafRuleV1::EvidenceSetPresent if evidence && !authority => {
                GateEvaluationResultV1::Indeterminate
            }
            GateLeafRuleV1::AuthoritySetPresent if authority && !evidence => {
                GateEvaluationResultV1::Indeterminate
            }
            GateLeafRuleV1::MixedSetPresent if evidence && authority => {
                GateEvaluationResultV1::Indeterminate
            }
            GateLeafRuleV1::ExactInputSet if context.input_set_hash == context.parameters_hash => {
                GateEvaluationResultV1::Pass
            }
            GateLeafRuleV1::ExactInputSet => GateEvaluationResultV1::Fail,
            GateLeafRuleV1::EvidenceSemanticMatch
            | GateLeafRuleV1::AuthoritySemanticMatch
            | GateLeafRuleV1::MixedSemanticMatch => {
                match Self::semantic_parameters_hash(rule, context.inputs) {
                    Ok(actual) if actual == *context.parameters_hash => {
                        GateEvaluationResultV1::Pass
                    }
                    Ok(_) => GateEvaluationResultV1::Fail,
                    Err(_) => GateEvaluationResultV1::Error,
                }
            }
            GateLeafRuleV1::EvidenceSetPresent
            | GateLeafRuleV1::AuthoritySetPresent
            | GateLeafRuleV1::MixedSetPresent => GateEvaluationResultV1::Error,
        };
        LeafGateEvaluationOutputV1::new(
            result,
            domain_hash(
                "maestro.vnext.evidence.closed-leaf-evaluator-diagnostic.v1",
                &CborValue::Array(vec![
                    bytes(context.gate_id.as_bytes()),
                    bytes(self.contract.id().as_bytes()),
                    bytes(context.parameters_hash),
                    bytes(context.input_set_hash),
                    CborValue::Unsigned(context.as_of),
                    CborValue::Unsigned(result.tag()),
                ]),
            )?,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentV1 {
    id: AssessmentIdV1,
    store_domain_id: StoreDomainIdV1,
    gate_snapshot_id: GateSnapshotIdV1,
    gate_id: GateNodeIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    scope: AssessmentScopeV1,
    evaluator_contract_id: GateEvaluatorContractIdV1,
    trust_root_snapshot_hash: [u8; 32],
    input_set_hash: [u8; 32],
    inputs: Vec<AssessmentInputRefV1>,
    time: AssessmentTimeBasisV1,
    valid_until: Option<u64>,
    result: GateEvaluationResultV1,
    diagnostic_hash: [u8; 32],
    record_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceMutationAuthorityV1 {
    action: RepositoryActionLeafV1,
    request_id: ActionRequestIdV1,
    receipt: AuthorizationReceiptV1,
    action_request_hash: [u8; 32],
    evidence_cut_hash: [u8; 32],
    accepted_h_time: u64,
    authority_basis_object_id: StoreObjectIdV1,
    authority_epoch: u64,
    authority_epoch_commitment: [u8; 32],
}

impl EvidenceMutationAuthorityV1 {
    pub(crate) fn from_admitted_action(
        admission: &AdmittedRepositoryActionV1,
        request_object: &crate::domain::persistence::StoreObjectV1,
        evidence_cut_hash: [u8; 32],
    ) -> Result<Self, AssessmentError> {
        require_nonzero(evidence_cut_hash, "Evidence mutation cut")?;
        if !matches!(
            admission.action(),
            RepositoryActionLeafV1::InvalidateAssessment
                | RepositoryActionLeafV1::SecurityEraseEvidencePayload
        ) || admission.request_id() != admission.authorization_receipt().request_id()
            || admission.accepted_h_time() == 0
        {
            return Err(AssessmentError::InvalidMutationAuthority);
        }
        Ok(Self {
            action: admission.action(),
            request_id: admission.request_id(),
            receipt: admission.authorization_receipt().clone(),
            action_request_hash: *request_object.id().as_bytes(),
            evidence_cut_hash,
            accepted_h_time: admission.accepted_h_time(),
            authority_basis_object_id: admission.basis_object().id(),
            authority_epoch: admission.authority_epoch(),
            authority_epoch_commitment: admission
                .authority_epoch_commitment()
                .map_err(|_| AssessmentError::InvalidMutationAuthority)?,
        })
    }

    #[cfg(test)]
    fn test_only(
        action: RepositoryActionLeafV1,
        receipt: AuthorizationReceiptV1,
        action_request_hash: [u8; 32],
        evidence_cut_hash: [u8; 32],
        accepted_h_time: u64,
    ) -> Result<Self, AssessmentError> {
        if !matches!(
            action,
            RepositoryActionLeafV1::InvalidateAssessment
                | RepositoryActionLeafV1::SecurityEraseEvidencePayload
        ) || accepted_h_time == 0
        {
            return Err(AssessmentError::InvalidMutationAuthority);
        }
        require_nonzero(action_request_hash, "Evidence mutation Action Request")?;
        require_nonzero(evidence_cut_hash, "Evidence mutation cut")?;
        Ok(Self {
            action,
            request_id: receipt.request_id(),
            receipt,
            action_request_hash,
            evidence_cut_hash,
            accepted_h_time,
            authority_basis_object_id: StoreObjectIdV1::from_digest(action_request_hash),
            authority_epoch: 1,
            authority_epoch_commitment: <sha2::Sha256 as sha2::Digest>::digest(
                deterministic_cbor::encode(&CborValue::Unsigned(1))?,
            )
            .into(),
        })
    }

    pub(crate) const fn action(&self) -> RepositoryActionLeafV1 {
        self.action
    }

    pub(crate) const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub(crate) const fn receipt(&self) -> &AuthorizationReceiptV1 {
        &self.receipt
    }

    pub(crate) const fn evidence_cut_hash(&self) -> &[u8; 32] {
        &self.evidence_cut_hash
    }

    pub(crate) const fn action_request_hash(&self) -> &[u8; 32] {
        &self.action_request_hash
    }

    pub(crate) const fn accepted_h_time(&self) -> u64 {
        self.accepted_h_time
    }

    pub(crate) const fn authority_basis_object_id(&self) -> StoreObjectIdV1 {
        self.authority_basis_object_id
    }

    pub(crate) const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub(crate) const fn authority_epoch_commitment(&self) -> [u8; 32] {
        self.authority_epoch_commitment
    }
}

impl AssessmentV1 {
    pub(crate) fn binds_gate_snapshot(&self, snapshot: &GateSnapshotV1) -> bool {
        snapshot.id() == self.gate_snapshot_id
            && snapshot.work_id() == self.work_id
            && snapshot.contract_generation_id() == self.contract_generation_id
            && snapshot.contract_root_id() == self.contract_root_id
            && snapshot.node(self.gate_id).is_some_and(|node| {
                node.evaluator().id() == self.evaluator_contract_id
                    && node.evaluator().trust_root_snapshot_hash() == &self.trust_root_snapshot_hash
            })
    }

    pub(crate) fn validate_recomputed(
        &self,
        snapshot: &GateSnapshotV1,
        input_cut: &EvidenceCutV1,
    ) -> Result<(), AssessmentError> {
        if self.gate_snapshot_id != snapshot.id()
            || self.work_id != snapshot.work_id()
            || self.contract_generation_id != snapshot.contract_generation_id()
            || self.contract_root_id != snapshot.contract_root_id()
            || self.store_domain_id != input_cut.store_domain_id
            || self.time.input_store_generation_id != input_cut.store_generation_id
            || self.time.evidence_input_cut_hash != input_cut.evidence_input_cut_hash
            || self.time.complete_input_cut_hash != input_cut.complete_cut_hash
        {
            return Err(AssessmentError::StaleEvidenceCut);
        }
        let node = snapshot
            .node(self.gate_id)
            .ok_or(AssessmentError::UnknownGate)?;
        if self.evaluator_contract_id != node.evaluator().id()
            || self.trust_root_snapshot_hash != *node.evaluator().trust_root_snapshot_hash()
        {
            return Err(AssessmentError::EvaluatorMismatch);
        }
        for input in &self.inputs {
            if let AssessmentInputRefV1::AuthorizationReceipt(value) = input {
                value.exact_receipt()?;
            }
        }
        let rebuilt = if node.operator() == GateOperatorV1::Leaf {
            let evaluator = ClosedLeafGateEvaluatorV1::new(node.evaluator().clone())?;
            Self::evaluate_leaf(
                snapshot,
                self.gate_id,
                AssessmentBasisV1 {
                    store_domain_id: self.store_domain_id,
                    scope: self.scope,
                    inputs: self.inputs.clone(),
                    time: self.time,
                },
                &evaluator,
            )?
        } else {
            let applicability = AssessmentApplicabilityV1::new(
                self.store_domain_id,
                input_cut.store_generation_id,
                snapshot,
                self.scope,
                TrustedTimeV1::Verified {
                    lower_bound: self.time.lower_bound,
                    upper_bound: self.time.upper_bound,
                },
                self.time,
            )?;
            let child_resolutions = node
                .children()
                .iter()
                .map(|child| resolve_gate_assessments(*child, &applicability, input_cut))
                .collect::<Result<Vec<_>, _>>()?;
            Self::evaluate_composite(
                snapshot,
                self.gate_id,
                self.store_domain_id,
                self.scope,
                self.time,
                child_resolutions,
            )?
        };
        if rebuilt != *self {
            return Err(AssessmentError::InvalidStoredAssessment);
        }
        Ok(())
    }

    pub(crate) fn validate_recomputed_from_persisted_snapshot(
        &self,
        snapshot: &GateSnapshotV1,
    ) -> Result<(), AssessmentError> {
        if !self.binds_gate_snapshot(snapshot) {
            return Err(AssessmentError::InvalidStoredAssessment);
        }
        for input in &self.inputs {
            if let AssessmentInputRefV1::AuthorizationReceipt(value) = input {
                value.exact_receipt()?;
            }
        }
        let node = snapshot
            .node(self.gate_id)
            .ok_or(AssessmentError::UnknownGate)?;
        let rebuilt = if node.operator() == GateOperatorV1::Leaf {
            let evaluator = ClosedLeafGateEvaluatorV1::new(node.evaluator().clone())?;
            Self::evaluate_leaf(
                snapshot,
                self.gate_id,
                AssessmentBasisV1 {
                    store_domain_id: self.store_domain_id,
                    scope: self.scope,
                    inputs: self.inputs.clone(),
                    time: self.time,
                },
                &evaluator,
            )?
        } else {
            let resolutions = node
                .children()
                .iter()
                .map(|child_id| {
                    let mut matching = self.inputs.iter().filter_map(|input| {
                        let AssessmentInputRefV1::ChildResolution(value) = input else {
                            return None;
                        };
                        (value.gate_id == *child_id).then_some(value)
                    });
                    let value = matching.next().ok_or(AssessmentError::InvalidInputs)?;
                    if matching.next().is_some() {
                        return Err(AssessmentError::InvalidInputs);
                    }
                    Ok(GateAssessmentResolutionV1 {
                        store_domain_id: self.store_domain_id,
                        store_generation_id: self.time.input_store_generation_id,
                        snapshot_id: self.gate_snapshot_id,
                        gate_id: value.gate_id,
                        work_id: self.work_id,
                        contract_generation_id: self.contract_generation_id,
                        scope: self.scope,
                        evidence_cut_hash: self.time.evidence_input_cut_hash,
                        result: value.result,
                        applicable_assessment_ids: value.assessment_ids.clone(),
                        valid_until: value.valid_until,
                        as_of: value.as_of,
                        time_basis_hash: value.time_basis_hash,
                        contributor_hashes: value.contributor_hashes.clone(),
                        support_roots: value.support_roots.clone(),
                        resolution_hash: value.resolution_hash,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Self::evaluate_composite(
                snapshot,
                self.gate_id,
                self.store_domain_id,
                self.scope,
                self.time,
                resolutions,
            )?
        };
        if rebuilt != *self {
            return Err(AssessmentError::InvalidStoredAssessment);
        }
        Ok(())
    }

    pub(crate) fn authorization_subject_value(&self) -> Result<CborValue, AssessmentError> {
        assessment_authorization_subject_value(
            self.store_domain_id,
            self.gate_snapshot_id,
            self.gate_id,
            self.work_id,
            self.contract_generation_id,
            self.scope,
        )
    }

    pub fn evaluate_leaf(
        snapshot: &GateSnapshotV1,
        gate_id: GateNodeIdV1,
        basis: AssessmentBasisV1,
        evaluator: &impl PinnedLeafGateEvaluatorV1,
    ) -> Result<Self, AssessmentError> {
        let node = snapshot.node(gate_id).ok_or(AssessmentError::UnknownGate)?;
        if node.operator() != GateOperatorV1::Leaf {
            return Err(AssessmentError::ExpectedLeaf);
        }
        validate_scope(
            snapshot,
            node.scope(),
            basis.store_domain_id,
            basis.scope,
            &basis.inputs,
        )?;
        let inputs = validate_inputs(node.input_class(), basis.inputs)?;
        let input_set_hash = assessment_input_set_hash(&inputs)?;
        if evaluator.contract_id() != node.evaluator().id() {
            return Err(AssessmentError::EvaluatorMismatch);
        }
        let valid_until = derive_valid_until(node.freshness_limit(), &inputs)?;
        let output = if valid_until.is_some_and(|limit| basis.time.as_of() >= limit) {
            LeafGateEvaluationOutputV1 {
                result: GateEvaluationResultV1::Indeterminate,
                diagnostic_hash: domain_hash(
                    "maestro.vnext.evidence.assessment-stale-input.v1",
                    &CborValue::Array(vec![
                        bytes(gate_id.as_bytes()),
                        bytes(&input_set_hash),
                        CborValue::Unsigned(basis.time.as_of()),
                    ]),
                )?,
            }
        } else {
            let evaluation = evaluator.evaluate(LeafGateEvaluationContextV1 {
                gate_id,
                parameters_hash: node.parameters_hash(),
                input_set_hash: &input_set_hash,
                inputs: &inputs,
                as_of: basis.time.as_of(),
                evidence_cut_hash: &basis.time.evidence_input_cut_hash,
            });
            evaluation.unwrap_or(LeafGateEvaluationOutputV1 {
                result: GateEvaluationResultV1::Error,
                diagnostic_hash: domain_hash(
                    "maestro.vnext.evidence.leaf-evaluator-error.v1",
                    &CborValue::Array(vec![
                        bytes(gate_id.as_bytes()),
                        bytes(&input_set_hash),
                        bytes(evaluator.contract_id().as_bytes()),
                        CborValue::Unsigned(basis.time.as_of()),
                    ]),
                )?,
            })
        };
        Self::new_record(
            snapshot,
            gate_id,
            AssessmentBasisV1 { inputs, ..basis },
            input_set_hash,
            valid_until,
            output,
        )
    }

    pub fn evaluate_composite(
        snapshot: &GateSnapshotV1,
        gate_id: GateNodeIdV1,
        store_domain_id: StoreDomainIdV1,
        scope: AssessmentScopeV1,
        time: AssessmentTimeBasisV1,
        child_resolutions: Vec<GateAssessmentResolutionV1>,
    ) -> Result<Self, AssessmentError> {
        let node = snapshot.node(gate_id).ok_or(AssessmentError::UnknownGate)?;
        if node.operator() == GateOperatorV1::Leaf {
            return Err(AssessmentError::ExpectedComposite);
        }
        let mut inputs = child_resolutions
            .into_iter()
            .map(|resolution| {
                if resolution.snapshot_id != snapshot.id()
                    || resolution.store_domain_id != store_domain_id
                    || resolution.store_generation_id != time.input_store_generation_id
                    || resolution.work_id != snapshot.work_id()
                    || resolution.contract_generation_id != snapshot.contract_generation_id()
                    || resolution.scope != scope
                    || resolution.evidence_cut_hash != time.evidence_input_cut_hash
                    || resolution.as_of > time.as_of()
                    || resolution.time_basis_hash != time.time_basis_hash()?
                {
                    return Err(AssessmentError::StaleEvidenceCut);
                }
                Ok(AssessmentInputRefV1::ChildResolution(
                    ChildAssessmentResolutionV1 {
                        gate_id: resolution.gate_id,
                        assessment_ids: resolution.applicable_assessment_ids,
                        resolution_hash: resolution.resolution_hash,
                        result: resolution.result,
                        valid_until: resolution.valid_until,
                        as_of: resolution.as_of,
                        time_basis_hash: resolution.time_basis_hash,
                        contributor_hashes: resolution.contributor_hashes,
                        support_roots: resolution.support_roots,
                    },
                ))
            })
            .collect::<Result<Vec<_>, AssessmentError>>()?;
        inputs = validate_inputs(GateInputClassV1::Composite, inputs)?;
        let gate_inputs = inputs
            .iter()
            .map(|input| match input {
                AssessmentInputRefV1::ChildResolution(value) => {
                    GateEvaluationInputV1::child(value.gate_id, value.resolution_hash, value.result)
                        .map_err(AssessmentError::from)
                }
                _ => Err(AssessmentError::InvalidInputClass),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let evaluation = PureGateEvaluatorV1.evaluate(snapshot, gate_id, gate_inputs)?;
        let independent = !matches!(node.operator(), GateOperatorV1::Quorum { .. })
            || quorum_supports_are_independent(&inputs)?;
        let input_set_hash = assessment_input_set_hash(&inputs)?;
        let valid_until = inputs
            .iter()
            .filter_map(AssessmentInputRefV1::independent_valid_until)
            .min();
        let output = LeafGateEvaluationOutputV1 {
            result: if !independent || valid_until.is_some_and(|limit| time.as_of() >= limit) {
                GateEvaluationResultV1::Indeterminate
            } else {
                evaluation.result()
            },
            diagnostic_hash: domain_hash(
                "maestro.vnext.evidence.composite-assessment-diagnostic.v1",
                &CborValue::Array(vec![
                    bytes(gate_id.as_bytes()),
                    bytes(&input_set_hash),
                    CborValue::Unsigned(evaluation.result().tag()),
                    CborValue::Bool(independent),
                ]),
            )?,
        };
        Self::new_record(
            snapshot,
            gate_id,
            AssessmentBasisV1 {
                store_domain_id,
                scope,
                inputs,
                time,
            },
            input_set_hash,
            valid_until,
            output,
        )
    }

    fn new_record(
        snapshot: &GateSnapshotV1,
        gate_id: GateNodeIdV1,
        basis: AssessmentBasisV1,
        input_set_hash: [u8; 32],
        valid_until: Option<u64>,
        output: LeafGateEvaluationOutputV1,
    ) -> Result<Self, AssessmentError> {
        require_nonzero(*basis.store_domain_id.as_bytes(), "Assessment Store Domain")?;
        let node = snapshot.node(gate_id).ok_or(AssessmentError::UnknownGate)?;
        validate_scope(
            snapshot,
            node.scope(),
            basis.store_domain_id,
            basis.scope,
            &basis.inputs,
        )?;
        let material = AssessmentIdentityMaterial {
            store_domain_id: basis.store_domain_id,
            gate_snapshot_id: snapshot.id(),
            gate_id,
            work_id: snapshot.work_id(),
            contract_generation_id: snapshot.contract_generation_id(),
            contract_root_id: snapshot.contract_root_id(),
            scope: basis.scope,
            evaluator_contract_id: node.evaluator().id(),
            trust_root_snapshot_hash: *node.evaluator().trust_root_snapshot_hash(),
            input_set_hash,
            inputs: &basis.inputs,
            time: basis.time,
            valid_until,
            result: output.result,
            diagnostic_hash: output.diagnostic_hash,
        };
        let identity_value = assessment_identity_value(&material);
        let id = AssessmentIdV1::from_bytes(domain_hash(
            "maestro.vnext.evidence.assessment-id.v1",
            &identity_value,
        )?)?;
        let record_value = CborValue::Array(vec![
            CborValue::Unsigned(ASSESSMENT_RECORD_VERSION_V1),
            bytes(id.as_bytes()),
            identity_value,
        ]);
        let record_hash = domain_hash(ASSESSMENT_RECORD_DOMAIN_V1, &record_value)?;
        Ok(Self {
            id,
            store_domain_id: basis.store_domain_id,
            gate_snapshot_id: snapshot.id(),
            gate_id,
            work_id: snapshot.work_id(),
            contract_generation_id: snapshot.contract_generation_id(),
            contract_root_id: snapshot.contract_root_id(),
            scope: basis.scope,
            evaluator_contract_id: node.evaluator().id(),
            trust_root_snapshot_hash: *node.evaluator().trust_root_snapshot_hash(),
            input_set_hash,
            inputs: basis.inputs,
            time: basis.time,
            valid_until,
            result: output.result,
            diagnostic_hash: output.diagnostic_hash,
            record_hash,
        })
    }

    pub const fn id(&self) -> AssessmentIdV1 {
        self.id
    }

    pub(crate) const fn store_domain_id(&self) -> StoreDomainIdV1 {
        self.store_domain_id
    }

    pub(crate) const fn evidence_input_cut_hash(&self) -> &[u8; 32] {
        &self.time.evidence_input_cut_hash
    }

    pub(crate) const fn complete_input_cut_hash(&self) -> &[u8; 32] {
        &self.time.complete_input_cut_hash
    }

    pub(crate) const fn evaluated_at(&self) -> u64 {
        self.time.as_of()
    }

    pub const fn gate_id(&self) -> GateNodeIdV1 {
        self.gate_id
    }

    pub const fn result(&self) -> GateEvaluationResultV1 {
        self.result
    }

    pub const fn valid_until(&self) -> Option<u64> {
        self.valid_until
    }

    pub const fn record_hash(&self) -> &[u8; 32] {
        &self.record_hash
    }

    pub(crate) fn references_payload(
        &self,
        payload_object_id: crate::domain::identity::StoreObjectIdV1,
    ) -> bool {
        self.inputs
            .iter()
            .any(|input| input.references_payload(payload_object_id))
    }

    fn is_applicable(
        &self,
        context: &AssessmentApplicabilityV1,
        invalidated: &BTreeSet<AssessmentIdV1>,
    ) -> bool {
        self.store_domain_id == context.store_domain_id
            && self.work_id == context.work_id
            && self.contract_generation_id == context.contract_generation_id
            && self.gate_snapshot_id == context.gate_snapshot_id
            && self.scope == context.scope
            && self.time.evidence_input_cut_hash == context.evidence_cut_hash
            && self
                .time
                .time_basis_hash()
                .is_ok_and(|hash| hash == context.time_basis_hash)
            && self.time.as_of() <= context.as_of
            && self.valid_until.is_none_or(|limit| context.as_of < limit)
            && !invalidated.contains(&self.id)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AssessmentError> {
        let material = AssessmentIdentityMaterial {
            store_domain_id: self.store_domain_id,
            gate_snapshot_id: self.gate_snapshot_id,
            gate_id: self.gate_id,
            work_id: self.work_id,
            contract_generation_id: self.contract_generation_id,
            contract_root_id: self.contract_root_id,
            scope: self.scope,
            evaluator_contract_id: self.evaluator_contract_id,
            trust_root_snapshot_hash: self.trust_root_snapshot_hash,
            input_set_hash: self.input_set_hash,
            inputs: &self.inputs,
            time: self.time,
            valid_until: self.valid_until,
            result: self.result,
            diagnostic_hash: self.diagnostic_hash,
        };
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(ASSESSMENT_RECORD_VERSION_V1),
            bytes(self.id.as_bytes()),
            assessment_identity_value(&material),
        ]))?)
    }

    pub(crate) fn from_canonical_bytes(value: &[u8]) -> Result<Self, AssessmentError> {
        let decoded = deterministic_cbor::decode(value)?;
        let CborValue::Array(record) = &decoded else {
            return Err(AssessmentError::InvalidStoredAssessment);
        };
        let [
            CborValue::Unsigned(ASSESSMENT_RECORD_VERSION_V1),
            id,
            CborValue::Array(material),
        ] = record.as_slice()
        else {
            return Err(AssessmentError::InvalidStoredAssessment);
        };
        let [
            store_domain_id,
            gate_snapshot_id,
            gate_id,
            work_id,
            contract_generation_id,
            contract_root_id,
            scope,
            evaluator_contract_id,
            trust_root_snapshot_hash,
            input_set_hash,
            CborValue::Array(inputs),
            time,
            valid_until,
            result,
            diagnostic_hash,
        ] = material.as_slice()
        else {
            return Err(AssessmentError::InvalidStoredAssessment);
        };
        let id = AssessmentIdV1::from_bytes(exact_assessment_digest(id)?)?;
        let store_domain_id = parse_store_domain(store_domain_id)?;
        let inputs = inputs
            .iter()
            .map(parse_assessment_input)
            .collect::<Result<Vec<_>, _>>()?;
        let time = parse_assessment_time(time)?;
        let assessment = Self {
            id,
            store_domain_id,
            gate_snapshot_id: GateSnapshotIdV1::from_bytes(exact_assessment_digest(
                gate_snapshot_id,
            )?)?,
            gate_id: GateNodeIdV1::from_bytes(exact_assessment_digest(gate_id)?)?,
            work_id: parse_work_id(work_id)?,
            contract_generation_id: parse_contract_generation(contract_generation_id)?,
            contract_root_id: ContractRootIdV1::from_digest(exact_assessment_digest(
                contract_root_id,
            )?),
            scope: parse_assessment_scope(scope)?,
            evaluator_contract_id: GateEvaluatorContractIdV1::from_bytes(exact_assessment_digest(
                evaluator_contract_id,
            )?)?,
            trust_root_snapshot_hash: exact_assessment_digest(trust_root_snapshot_hash)?,
            input_set_hash: exact_assessment_digest(input_set_hash)?,
            inputs,
            time,
            valid_until: parse_optional_u64(valid_until)?,
            result: parse_gate_result(result)?,
            diagnostic_hash: exact_assessment_digest(diagnostic_hash)?,
            record_hash: domain_hash(ASSESSMENT_RECORD_DOMAIN_V1, &decoded)?,
        };
        let input_class = inferred_input_class(&assessment.inputs)?;
        let validated_inputs = validate_inputs(input_class, assessment.inputs.clone())?;
        let gate_scope = match assessment.scope {
            AssessmentScopeV1::Work => GateScopeV1::Work,
            AssessmentScopeV1::Step(_) => GateScopeV1::Step,
        };
        require_nonzero(
            assessment.trust_root_snapshot_hash,
            "Assessment trust-root snapshot",
        )?;
        require_nonzero(
            assessment.time.freshness_basis_hash,
            "Assessment freshness basis",
        )?;
        require_nonzero(
            assessment.time.evidence_input_cut_hash,
            "Assessment Evidence input cut",
        )?;
        require_nonzero(
            assessment.time.complete_input_cut_hash,
            "Assessment complete input cut",
        )?;
        require_nonzero(assessment.diagnostic_hash, "Assessment diagnostic")?;
        let identity = assessment_identity_value(&AssessmentIdentityMaterial {
            store_domain_id: assessment.store_domain_id,
            gate_snapshot_id: assessment.gate_snapshot_id,
            gate_id: assessment.gate_id,
            work_id: assessment.work_id,
            contract_generation_id: assessment.contract_generation_id,
            contract_root_id: assessment.contract_root_id,
            scope: assessment.scope,
            evaluator_contract_id: assessment.evaluator_contract_id,
            trust_root_snapshot_hash: assessment.trust_root_snapshot_hash,
            input_set_hash: assessment.input_set_hash,
            inputs: &assessment.inputs,
            time: assessment.time,
            valid_until: assessment.valid_until,
            result: assessment.result,
            diagnostic_hash: assessment.diagnostic_hash,
        });
        if validated_inputs != assessment.inputs
            || validate_scope_semantics(
                assessment.store_domain_id,
                assessment.work_id,
                assessment.contract_generation_id,
                assessment.contract_root_id,
                gate_scope,
                assessment.scope,
                &assessment.inputs,
            )
            .is_err()
            || assessment_input_set_hash(&assessment.inputs)? != assessment.input_set_hash
            || identity != CborValue::Array(material.clone())
            || assessment.id
                != AssessmentIdV1::from_bytes(domain_hash(
                    "maestro.vnext.evidence.assessment-id.v1",
                    &identity,
                )?)?
            || assessment.canonical_bytes()? != value
        {
            return Err(AssessmentError::InvalidStoredAssessment);
        }
        Ok(assessment)
    }

    pub(crate) fn dependency_assessment_ids(&self) -> Vec<AssessmentIdV1> {
        let mut ids = self
            .inputs
            .iter()
            .flat_map(AssessmentInputRefV1::child_assessment_ids)
            .copied()
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub(crate) fn inputs(&self) -> &[AssessmentInputRefV1] {
        &self.inputs
    }

    pub const fn input_store_generation_id(&self) -> StoreGenerationIdV1 {
        self.time.input_store_generation_id
    }

    #[cfg(test)]
    pub(crate) const fn time_basis(&self) -> AssessmentTimeBasisV1 {
        self.time
    }
}

pub(crate) fn assessment_authorization_subject_value(
    store_domain_id: StoreDomainIdV1,
    gate_snapshot_id: GateSnapshotIdV1,
    gate_id: GateNodeIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    scope: AssessmentScopeV1,
) -> Result<CborValue, AssessmentError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-assessment-subject.v1")?,
        bytes(store_domain_id.as_bytes()),
        bytes(gate_snapshot_id.as_bytes()),
        bytes(gate_id.as_bytes()),
        bytes(work_id.as_bytes()),
        bytes(contract_generation_id.as_bytes()),
        scope.canonical_value(),
    ]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssessmentApplicabilityV1 {
    store_domain_id: StoreDomainIdV1,
    store_generation_id: StoreGenerationIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    gate_snapshot_id: GateSnapshotIdV1,
    scope: AssessmentScopeV1,
    as_of: u64,
    time_basis_hash: [u8; 32],
    evidence_cut_hash: [u8; 32],
}

impl AssessmentApplicabilityV1 {
    pub(crate) fn new(
        store_domain_id: StoreDomainIdV1,
        current_store_generation_id: StoreGenerationIdV1,
        gate_snapshot: &GateSnapshotV1,
        scope: AssessmentScopeV1,
        current_time: TrustedTimeV1,
        assessment_time_basis: AssessmentTimeBasisV1,
    ) -> Result<Self, AssessmentError> {
        let TrustedTimeV1::Verified {
            lower_bound: _,
            upper_bound: as_of,
        } = current_time
        else {
            return Err(AssessmentError::TrustedTimeUnavailable);
        };
        if as_of < assessment_time_basis.as_of() {
            return Err(AssessmentError::InvalidTimeWindow);
        }
        Ok(Self {
            store_domain_id,
            store_generation_id: current_store_generation_id,
            work_id: gate_snapshot.work_id(),
            contract_generation_id: gate_snapshot.contract_generation_id(),
            gate_snapshot_id: gate_snapshot.id(),
            scope,
            as_of,
            time_basis_hash: assessment_time_basis.time_basis_hash()?,
            evidence_cut_hash: assessment_time_basis.evidence_input_cut_hash,
        })
    }
}

fn assessment_applicability_time_basis_hash(
    lower_bound: u64,
    upper_bound: u64,
    freshness_basis_hash: [u8; 32],
) -> Result<[u8; 32], AssessmentError> {
    Ok(domain_hash(
        "maestro.vnext.evidence.assessment-applicability-time.v1",
        &CborValue::Array(vec![
            CborValue::Unsigned(lower_bound),
            CborValue::Unsigned(upper_bound),
            bytes(&freshness_basis_hash),
        ]),
    )?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssessmentInvalidationReasonV1 {
    WorkGenerationAdvanced,
    StepRevisionAdvanced,
    GateSnapshotChanged,
    EvaluatorChanged,
    InputTombstoned,
    InputCorrected,
    FreshnessExpired,
    IntegrityFailure,
    AuthorizationReceiptRevoked,
}

impl AssessmentInvalidationReasonV1 {
    pub(crate) const fn tag(self) -> u64 {
        match self {
            Self::WorkGenerationAdvanced => 1,
            Self::StepRevisionAdvanced => 2,
            Self::GateSnapshotChanged => 3,
            Self::EvaluatorChanged => 4,
            Self::InputTombstoned => 5,
            Self::InputCorrected => 6,
            Self::FreshnessExpired => 7,
            Self::IntegrityFailure => 8,
            Self::AuthorizationReceiptRevoked => 9,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentInvalidationV1 {
    id: AssessmentInvalidationIdV1,
    assessment_id: AssessmentIdV1,
    reason: AssessmentInvalidationReasonV1,
    source_revision_hash: [u8; 32],
    authority_receipt_id: crate::domain::authority::AuthorizationReceiptIdV1,
    authority_receipt_hash: [u8; 32],
    action_request_id: ActionRequestIdV1,
    action_request_hash: [u8; 32],
    invalidated_at: u64,
    evidence_cut_hash: [u8; 32],
    replacement_assessment_id: Option<AssessmentIdV1>,
}

impl AssessmentInvalidationV1 {
    pub fn authorized(
        assessment: &AssessmentV1,
        reason: AssessmentInvalidationReasonV1,
        source_revision_hash: [u8; 32],
        authority: &EvidenceMutationAuthorityV1,
        evidence_cut_hash: [u8; 32],
        replacement_assessment_id: Option<AssessmentIdV1>,
    ) -> Result<Self, AssessmentError> {
        require_nonzero(source_revision_hash, "Assessment invalidation source")?;
        require_nonzero(evidence_cut_hash, "Assessment invalidation Evidence cut")?;
        if !matches!(
            authority.action(),
            RepositoryActionLeafV1::InvalidateAssessment
                | RepositoryActionLeafV1::SecurityEraseEvidencePayload
        ) || authority.evidence_cut_hash != evidence_cut_hash
        {
            return Err(AssessmentError::StaleEvidenceCut);
        }
        if replacement_assessment_id == Some(assessment.id) {
            return Err(AssessmentError::SelfReplacement);
        }
        let authority_receipt_hash = domain_hash(
            "maestro.vnext.evidence.invalidation-authority-receipt.v1",
            &CborValue::Bytes(authority.receipt.canonical_bytes()?),
        )?;
        let value = CborValue::Array(vec![
            bytes(assessment.id.as_bytes()),
            CborValue::Unsigned(reason.tag()),
            bytes(&source_revision_hash),
            bytes(authority.receipt.id().as_bytes()),
            bytes(&authority_receipt_hash),
            bytes(authority.request_id.as_bytes()),
            bytes(&authority.action_request_hash),
            CborValue::Unsigned(authority.accepted_h_time()),
            bytes(&evidence_cut_hash),
            CborValue::optional(replacement_assessment_id.map(|id| bytes(id.as_bytes()))),
        ]);
        Ok(Self {
            id: AssessmentInvalidationIdV1::from_bytes(domain_hash(
                "maestro.vnext.evidence.assessment-invalidation.v1",
                &value,
            )?)?,
            assessment_id: assessment.id,
            reason,
            source_revision_hash,
            authority_receipt_id: authority.receipt.id(),
            authority_receipt_hash,
            action_request_id: authority.request_id,
            action_request_hash: authority.action_request_hash,
            invalidated_at: authority.accepted_h_time(),
            evidence_cut_hash,
            replacement_assessment_id,
        })
    }

    pub const fn id(&self) -> AssessmentInvalidationIdV1 {
        self.id
    }

    pub const fn assessment_id(&self) -> AssessmentIdV1 {
        self.assessment_id
    }

    pub(crate) const fn reason(&self) -> AssessmentInvalidationReasonV1 {
        self.reason
    }

    pub(crate) const fn source_revision_hash(&self) -> &[u8; 32] {
        &self.source_revision_hash
    }

    pub const fn evidence_cut_hash(&self) -> &[u8; 32] {
        &self.evidence_cut_hash
    }

    pub(crate) const fn authority_receipt_id(
        &self,
    ) -> crate::domain::authority::AuthorizationReceiptIdV1 {
        self.authority_receipt_id
    }

    pub(crate) const fn authority_receipt_hash(&self) -> &[u8; 32] {
        &self.authority_receipt_hash
    }

    pub(crate) const fn action_request_id(&self) -> ActionRequestIdV1 {
        self.action_request_id
    }

    pub(crate) const fn action_request_hash(&self) -> &[u8; 32] {
        &self.action_request_hash
    }

    pub(crate) const fn invalidated_at(&self) -> u64 {
        self.invalidated_at
    }

    pub(crate) const fn replacement_assessment_id(&self) -> Option<AssessmentIdV1> {
        self.replacement_assessment_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AssessmentError> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            bytes(self.id.as_bytes()),
            bytes(self.assessment_id.as_bytes()),
            CborValue::Unsigned(self.reason.tag()),
            bytes(&self.source_revision_hash),
            bytes(self.authority_receipt_id.as_bytes()),
            bytes(&self.authority_receipt_hash),
            bytes(self.action_request_id.as_bytes()),
            bytes(&self.action_request_hash),
            CborValue::Unsigned(self.invalidated_at),
            bytes(&self.evidence_cut_hash),
            CborValue::optional(
                self.replacement_assessment_id
                    .map(|id| bytes(id.as_bytes())),
            ),
        ]))?)
    }

    pub(crate) fn from_canonical_bytes(value: &[u8]) -> Result<Self, AssessmentError> {
        let decoded = deterministic_cbor::decode(value)?;
        let CborValue::Array(fields) = &decoded else {
            return Err(AssessmentError::InvalidStoredAssessment);
        };
        let [
            id,
            assessment_id,
            reason,
            source_revision_hash,
            authority_receipt_id,
            authority_receipt_hash,
            action_request_id,
            action_request_hash,
            CborValue::Unsigned(invalidated_at),
            evidence_cut_hash,
            replacement_assessment_id,
        ] = fields.as_slice()
        else {
            return Err(AssessmentError::InvalidStoredAssessment);
        };
        let invalidation = Self {
            id: AssessmentInvalidationIdV1::from_bytes(exact_assessment_digest(id)?)?,
            assessment_id: AssessmentIdV1::from_bytes(exact_assessment_digest(assessment_id)?)?,
            reason: parse_invalidation_reason(reason)?,
            source_revision_hash: exact_assessment_digest(source_revision_hash)?,
            authority_receipt_id: AuthorizationReceiptIdV1::from_digest(exact_assessment_digest(
                authority_receipt_id,
            )?),
            authority_receipt_hash: exact_assessment_digest(authority_receipt_hash)?,
            action_request_id: ActionRequestIdV1::from_digest(exact_assessment_digest(
                action_request_id,
            )?),
            action_request_hash: exact_assessment_digest(action_request_hash)?,
            invalidated_at: *invalidated_at,
            evidence_cut_hash: exact_assessment_digest(evidence_cut_hash)?,
            replacement_assessment_id: parse_optional_digest(replacement_assessment_id)?
                .map(AssessmentIdV1::from_bytes)
                .transpose()?,
        };
        let identity_value = CborValue::Array(vec![
            bytes(invalidation.assessment_id.as_bytes()),
            CborValue::Unsigned(invalidation.reason.tag()),
            bytes(&invalidation.source_revision_hash),
            bytes(invalidation.authority_receipt_id.as_bytes()),
            bytes(&invalidation.authority_receipt_hash),
            bytes(invalidation.action_request_id.as_bytes()),
            bytes(&invalidation.action_request_hash),
            CborValue::Unsigned(invalidation.invalidated_at),
            bytes(&invalidation.evidence_cut_hash),
            CborValue::optional(
                invalidation
                    .replacement_assessment_id
                    .map(|id| bytes(id.as_bytes())),
            ),
        ]);
        if invalidation.id
            != AssessmentInvalidationIdV1::from_bytes(domain_hash(
                "maestro.vnext.evidence.assessment-invalidation.v1",
                &identity_value,
            )?)?
            || invalidation.canonical_bytes()? != value
        {
            return Err(AssessmentError::InvalidStoredAssessment);
        }
        Ok(invalidation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateAssessmentResolutionV1 {
    store_domain_id: StoreDomainIdV1,
    store_generation_id: StoreGenerationIdV1,
    snapshot_id: GateSnapshotIdV1,
    gate_id: GateNodeIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    scope: AssessmentScopeV1,
    evidence_cut_hash: [u8; 32],
    result: GateEvaluationResultV1,
    applicable_assessment_ids: Vec<AssessmentIdV1>,
    valid_until: Option<u64>,
    as_of: u64,
    time_basis_hash: [u8; 32],
    contributor_hashes: Vec<[u8; 32]>,
    support_roots: Vec<[u8; 32]>,
    resolution_hash: [u8; 32],
}

impl GateAssessmentResolutionV1 {
    pub const fn result(&self) -> GateEvaluationResultV1 {
        self.result
    }

    pub fn applicable_assessment_ids(&self) -> &[AssessmentIdV1] {
        &self.applicable_assessment_ids
    }

    pub(crate) const fn gate_id(&self) -> GateNodeIdV1 {
        self.gate_id
    }

    pub(crate) const fn snapshot_id(&self) -> GateSnapshotIdV1 {
        self.snapshot_id
    }

    pub(crate) const fn scope(&self) -> AssessmentScopeV1 {
        self.scope
    }

    pub(crate) const fn as_of(&self) -> u64 {
        self.as_of
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            bytes(self.store_domain_id.as_bytes()),
            bytes(self.store_generation_id.as_bytes()),
            bytes(self.snapshot_id.as_bytes()),
            bytes(self.gate_id.as_bytes()),
            bytes(self.work_id.as_bytes()),
            bytes(self.contract_generation_id.as_bytes()),
            self.scope.canonical_value(),
            bytes(&self.evidence_cut_hash),
            CborValue::Unsigned(self.result.tag()),
            CborValue::Array(
                self.applicable_assessment_ids
                    .iter()
                    .map(|id| bytes(id.as_bytes()))
                    .collect(),
            ),
            CborValue::optional(self.valid_until.map(CborValue::Unsigned)),
            CborValue::Unsigned(self.as_of),
            bytes(&self.time_basis_hash),
            CborValue::Array(self.contributor_hashes.iter().map(bytes).collect()),
            CborValue::Array(self.support_roots.iter().map(bytes).collect()),
            bytes(&self.resolution_hash),
        ])
    }

    pub(crate) fn validate_recomputed(&self, cut: &EvidenceCutV1) -> Result<(), AssessmentError> {
        let context = AssessmentApplicabilityV1 {
            store_domain_id: self.store_domain_id,
            store_generation_id: self.store_generation_id,
            work_id: self.work_id,
            contract_generation_id: self.contract_generation_id,
            gate_snapshot_id: self.snapshot_id,
            scope: self.scope,
            as_of: self.as_of,
            time_basis_hash: self.time_basis_hash,
            evidence_cut_hash: self.evidence_cut_hash,
        };
        if resolve_gate_assessments(self.gate_id, &context, cut)? != *self {
            return Err(AssessmentError::StaleEvidenceCut);
        }
        Ok(())
    }

    pub const fn satisfaction(&self) -> DerivedGateSatisfactionV1 {
        match self.result {
            GateEvaluationResultV1::Pass => DerivedGateSatisfactionV1::Satisfied,
            result => DerivedGateSatisfactionV1::Blocked(result),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedGateSatisfactionV1 {
    Satisfied,
    Blocked(GateEvaluationResultV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceCutV1 {
    store_domain_id: StoreDomainIdV1,
    store_head_id: StoreHeadIdV1,
    store_generation_id: StoreGenerationIdV1,
    evidence_index_object_id: StoreObjectIdV1,
    evidence_input_cut_hash: [u8; 32],
    assessments: Vec<AssessmentV1>,
    invalidations: Vec<AssessmentInvalidationV1>,
    complete_cut_hash: [u8; 32],
}

impl EvidenceCutV1 {
    pub(crate) fn from_current_index(
        store_domain_id: StoreDomainIdV1,
        store_head_id: StoreHeadIdV1,
        store_generation_id: StoreGenerationIdV1,
        evidence_index_object_id: StoreObjectIdV1,
        evidence_input_cut_hash: [u8; 32],
        mut assessments: Vec<AssessmentV1>,
        mut invalidations: Vec<AssessmentInvalidationV1>,
    ) -> Result<Self, AssessmentError> {
        require_nonzero(evidence_input_cut_hash, "Evidence input cut")?;
        assessments.sort_unstable_by_key(|assessment| assessment.id);
        invalidations.sort_unstable_by_key(|invalidation| invalidation.id);
        if assessments.windows(2).any(|pair| pair[0].id == pair[1].id)
            || invalidations
                .windows(2)
                .any(|pair| pair[0].id == pair[1].id)
            || assessments
                .iter()
                .any(|assessment| assessment.store_domain_id != store_domain_id)
        {
            return Err(AssessmentError::IncompleteEvidenceCut);
        }
        let assessment_ids = assessments
            .iter()
            .map(|assessment| assessment.id)
            .collect::<BTreeSet<_>>();
        if invalidations
            .iter()
            .any(|invalidation| !assessment_ids.contains(&invalidation.assessment_id))
        {
            return Err(AssessmentError::UnknownInvalidationTarget);
        }
        let complete_cut_hash = domain_hash(
            "maestro.vnext.evidence.complete-cut.v1",
            &CborValue::Array(vec![
                bytes(store_domain_id.as_bytes()),
                bytes(store_head_id.as_bytes()),
                bytes(store_generation_id.as_bytes()),
                bytes(evidence_index_object_id.as_bytes()),
                bytes(&evidence_input_cut_hash),
                CborValue::Array(
                    assessments
                        .iter()
                        .map(|assessment| {
                            CborValue::Array(vec![
                                bytes(assessment.id.as_bytes()),
                                bytes(&assessment.record_hash),
                            ])
                        })
                        .collect(),
                ),
                CborValue::Array(
                    invalidations
                        .iter()
                        .map(|invalidation| bytes(invalidation.id.as_bytes()))
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            store_domain_id,
            store_head_id,
            store_generation_id,
            evidence_index_object_id,
            evidence_input_cut_hash,
            assessments,
            invalidations,
            complete_cut_hash,
        })
    }

    pub const fn evidence_input_cut_hash(&self) -> &[u8; 32] {
        &self.evidence_input_cut_hash
    }

    pub const fn complete_cut_hash(&self) -> &[u8; 32] {
        &self.complete_cut_hash
    }

    pub(crate) const fn store_generation_id(&self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub(crate) fn assessment(&self, id: AssessmentIdV1) -> Option<&AssessmentV1> {
        self.assessments
            .binary_search_by_key(&id, |assessment| assessment.id())
            .ok()
            .map(|index| &self.assessments[index])
    }

    pub(crate) fn assessments(&self) -> &[AssessmentV1] {
        &self.assessments
    }
}

pub fn resolve_gate_assessments(
    gate_id: GateNodeIdV1,
    context: &AssessmentApplicabilityV1,
    cut: &EvidenceCutV1,
) -> Result<GateAssessmentResolutionV1, AssessmentError> {
    if cut.store_domain_id != context.store_domain_id
        || cut.store_generation_id != context.store_generation_id
        || cut.evidence_input_cut_hash != context.evidence_cut_hash
    {
        return Err(AssessmentError::StaleEvidenceCut);
    }
    let by_assessment: BTreeMap<_, _> = cut
        .assessments
        .iter()
        .map(|assessment| (assessment.id, assessment))
        .collect();
    if cut
        .invalidations
        .iter()
        .any(|invalidation| !by_assessment.contains_key(&invalidation.assessment_id))
    {
        return Err(AssessmentError::UnknownInvalidationTarget);
    }
    let invalidated: BTreeSet<_> = cut
        .invalidations
        .iter()
        .map(AssessmentInvalidationV1::assessment_id)
        .collect();
    let mut applicable = cut
        .assessments
        .iter()
        .filter(|assessment| {
            assessment.gate_id == gate_id && assessment.is_applicable(context, &invalidated)
        })
        .collect::<Vec<_>>();
    applicable.sort_unstable_by_key(|assessment| assessment.id);
    let results = applicable
        .iter()
        .map(|assessment| assessment.result)
        .collect::<BTreeSet<_>>();
    let result = if results.len() == 1 {
        *results.first().expect("invariant: one result exists")
    } else {
        GateEvaluationResultV1::Indeterminate
    };
    let applicable_assessment_ids = applicable
        .iter()
        .map(|assessment| assessment.id)
        .collect::<Vec<_>>();
    let valid_until = applicable
        .iter()
        .filter_map(|assessment| assessment.valid_until)
        .min();
    let mut contributor_hashes = Vec::new();
    let mut support_roots = Vec::new();
    for assessment in &applicable {
        for input in &assessment.inputs {
            let support = input.support_bindings()?;
            contributor_hashes.extend(support.contributors);
            support_roots.extend(support.roots);
        }
    }
    contributor_hashes.sort_unstable();
    contributor_hashes.dedup();
    support_roots.sort_unstable();
    support_roots.dedup();
    if applicable.is_empty() {
        contributor_hashes.push(domain_hash(
            "maestro.vnext.evidence.no-applicable-assessment-contributor.v1",
            &CborValue::Array(vec![
                bytes(gate_id.as_bytes()),
                bytes(&context.evidence_cut_hash),
            ]),
        )?);
        support_roots.push(domain_hash(
            "maestro.vnext.evidence.no-applicable-assessment-support.v1",
            &CborValue::Array(vec![
                bytes(gate_id.as_bytes()),
                bytes(&cut.complete_cut_hash),
            ]),
        )?);
    }
    validate_support_set(&contributor_hashes)?;
    validate_support_set(&support_roots)?;
    let resolution_hash = domain_hash(
        "maestro.vnext.evidence.gate-assessment-resolution.v1",
        &CborValue::Array(vec![
            bytes(context.store_domain_id.as_bytes()),
            bytes(context.store_generation_id.as_bytes()),
            bytes(context.gate_snapshot_id.as_bytes()),
            bytes(gate_id.as_bytes()),
            bytes(context.work_id.as_bytes()),
            bytes(context.contract_generation_id.as_bytes()),
            context.scope.canonical_value(),
            bytes(&context.evidence_cut_hash),
            bytes(&cut.complete_cut_hash),
            CborValue::Unsigned(context.as_of),
            bytes(&context.time_basis_hash),
            CborValue::Array(
                applicable_assessment_ids
                    .iter()
                    .map(|id| bytes(id.as_bytes()))
                    .collect(),
            ),
            CborValue::Unsigned(result.tag()),
            CborValue::optional(valid_until.map(CborValue::Unsigned)),
            CborValue::Array(contributor_hashes.iter().map(bytes).collect()),
            CborValue::Array(support_roots.iter().map(bytes).collect()),
        ]),
    )?;
    Ok(GateAssessmentResolutionV1 {
        store_domain_id: context.store_domain_id,
        store_generation_id: context.store_generation_id,
        snapshot_id: context.gate_snapshot_id,
        gate_id,
        work_id: context.work_id,
        contract_generation_id: context.contract_generation_id,
        scope: context.scope,
        evidence_cut_hash: context.evidence_cut_hash,
        result,
        applicable_assessment_ids,
        valid_until,
        as_of: context.as_of,
        time_basis_hash: context.time_basis_hash,
        contributor_hashes,
        support_roots,
        resolution_hash,
    })
}

fn validate_scope(
    snapshot: &GateSnapshotV1,
    gate_scope: GateScopeV1,
    store_domain_id: StoreDomainIdV1,
    assessment_scope: AssessmentScopeV1,
    inputs: &[AssessmentInputRefV1],
) -> Result<(), AssessmentError> {
    validate_scope_semantics(
        store_domain_id,
        snapshot.work_id(),
        snapshot.contract_generation_id(),
        snapshot.contract_root_id(),
        gate_scope,
        assessment_scope,
        inputs,
    )
}

fn validate_scope_semantics(
    store_domain_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    gate_scope: GateScopeV1,
    assessment_scope: AssessmentScopeV1,
    inputs: &[AssessmentInputRefV1],
) -> Result<(), AssessmentError> {
    if inputs
        .iter()
        .filter_map(AssessmentInputRefV1::store_domain_id)
        .any(|domain| domain != store_domain_id)
    {
        return Err(AssessmentError::CrossStoreInput);
    }
    if let AssessmentScopeV1::Step(binding) = assessment_scope
        && (binding.scope().repository_id() != store_domain_id
            || binding.scope().work_id() != work_id
            || binding.contract_generation_id() != contract_generation_id
            || binding.contract_root_id() != contract_root_id)
    {
        return Err(AssessmentError::StepScopeMismatch);
    }
    validate_scope_identity(
        store_domain_id,
        work_id,
        contract_generation_id,
        contract_root_id,
        gate_scope,
        assessment_scope,
        inputs,
    )
}

fn validate_scope_identity(
    store_domain_id: StoreDomainIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    gate_scope: GateScopeV1,
    assessment_scope: AssessmentScopeV1,
    inputs: &[AssessmentInputRefV1],
) -> Result<(), AssessmentError> {
    if inputs.iter().any(|input| match input {
        AssessmentInputRefV1::Claim(value) => match &value.subject {
            ClaimSubjectV1::Work {
                work_id: claim_work_id,
                contract_root_id: claim_contract_root_id,
                ..
            } => {
                *claim_work_id != work_id
                    || *claim_contract_root_id != contract_root_id
                    || value.work_subject()
                        != Some((
                            store_domain_id,
                            *work_id.as_bytes(),
                            contract_generation_id,
                            contract_root_id,
                        ))
            }
            ClaimSubjectV1::Step { binding, .. } => {
                binding.scope().work_id() != work_id
                    || binding.contract_generation_id() != contract_generation_id
                    || binding.contract_root_id() != contract_root_id
            }
        },
        AssessmentInputRefV1::Observation(value) => {
            value.work_subject()
                != Some((
                    store_domain_id,
                    *work_id.as_bytes(),
                    contract_generation_id,
                    contract_root_id,
                ))
        }
        AssessmentInputRefV1::ChildResolution(_)
        | AssessmentInputRefV1::AuthorizationReceipt(_) => false,
    }) {
        return Err(AssessmentError::WorkScopeMismatch);
    }
    validate_scope_shape(gate_scope, assessment_scope, inputs)
}

fn validate_scope_shape(
    gate_scope: GateScopeV1,
    assessment_scope: AssessmentScopeV1,
    inputs: &[AssessmentInputRefV1],
) -> Result<(), AssessmentError> {
    match (gate_scope, assessment_scope) {
        (GateScopeV1::Work, AssessmentScopeV1::Work) => {
            if inputs.iter().any(|input| {
                input.step_binding().is_some()
                    || input.observation_step_subject().is_some()
                    || matches!(input, AssessmentInputRefV1::Observation(_))
                        && input.observation_work_subject().is_none()
            }) {
                return Err(AssessmentError::StepScopeMismatch);
            }
        }
        (GateScopeV1::Step, AssessmentScopeV1::Step(expected)) => {
            let composite_only = inputs
                .iter()
                .all(|input| matches!(input, AssessmentInputRefV1::ChildResolution(_)));
            if inputs.iter().any(|input| {
                input
                    .step_binding()
                    .is_some_and(|binding| binding != expected)
                    || matches!(input, AssessmentInputRefV1::Observation(_))
            }) || (!composite_only && inputs.iter().all(|input| input.step_binding().is_none()))
            {
                return Err(AssessmentError::StepScopeMismatch);
            }
        }
        _ => return Err(AssessmentError::StepScopeMismatch),
    }
    Ok(())
}

fn validate_inputs(
    input_class: GateInputClassV1,
    mut inputs: Vec<AssessmentInputRefV1>,
) -> Result<Vec<AssessmentInputRefV1>, AssessmentError> {
    inputs.sort_unstable();
    if inputs.is_empty()
        || inputs.len() > MAX_ASSESSMENT_INPUTS_V1
        || inputs.windows(2).any(|pair| pair[0] == pair[1])
        || inputs.iter().any(|input| input.support_bindings().is_err())
    {
        return Err(AssessmentError::InvalidInputs);
    }
    let has_evidence = inputs.iter().any(|input| {
        matches!(
            input,
            AssessmentInputRefV1::Observation(_) | AssessmentInputRefV1::Claim(_)
        )
    });
    let has_authority = inputs
        .iter()
        .any(|input| matches!(input, AssessmentInputRefV1::AuthorizationReceipt(_)));
    let has_child = inputs
        .iter()
        .any(|input| matches!(input, AssessmentInputRefV1::ChildResolution(_)));
    let valid = match input_class {
        GateInputClassV1::Evidence => has_evidence && !has_authority && !has_child,
        GateInputClassV1::Authority => !has_evidence && has_authority && !has_child,
        GateInputClassV1::Mixed => has_evidence && has_authority && !has_child,
        GateInputClassV1::Composite => !has_evidence && !has_authority && has_child,
    };
    if !valid {
        return Err(AssessmentError::InvalidInputClass);
    }
    Ok(inputs)
}

fn inferred_input_class(
    inputs: &[AssessmentInputRefV1],
) -> Result<GateInputClassV1, AssessmentError> {
    let has_evidence = inputs.iter().any(|input| {
        matches!(
            input,
            AssessmentInputRefV1::Observation(_) | AssessmentInputRefV1::Claim(_)
        )
    });
    let has_authority = inputs
        .iter()
        .any(|input| matches!(input, AssessmentInputRefV1::AuthorizationReceipt(_)));
    let has_child = inputs
        .iter()
        .any(|input| matches!(input, AssessmentInputRefV1::ChildResolution(_)));
    match (has_evidence, has_authority, has_child) {
        (true, false, false) => Ok(GateInputClassV1::Evidence),
        (false, true, false) => Ok(GateInputClassV1::Authority),
        (true, true, false) => Ok(GateInputClassV1::Mixed),
        (false, false, true) => Ok(GateInputClassV1::Composite),
        _ => Err(AssessmentError::InvalidInputClass),
    }
}

fn derive_valid_until(
    freshness_limit: Option<u64>,
    inputs: &[AssessmentInputRefV1],
) -> Result<Option<u64>, AssessmentError> {
    let evidence_deadlines = inputs
        .iter()
        .filter_map(AssessmentInputRefV1::freshness_anchor)
        .map(|anchor| {
            freshness_limit
                .map(|limit| {
                    anchor
                        .checked_add(limit)
                        .ok_or(AssessmentError::InvalidTimeWindow)
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(evidence_deadlines
        .into_iter()
        .flatten()
        .chain(
            inputs
                .iter()
                .filter_map(AssessmentInputRefV1::independent_valid_until),
        )
        .min())
}

fn assessment_input_set_hash(inputs: &[AssessmentInputRefV1]) -> Result<[u8; 32], AssessmentError> {
    Ok(domain_hash(
        "maestro.vnext.evidence.assessment-input-set.v1",
        &CborValue::Array(
            inputs
                .iter()
                .map(AssessmentInputRefV1::canonical_value)
                .collect(),
        ),
    )?)
}

struct AssessmentIdentityMaterial<'a> {
    store_domain_id: StoreDomainIdV1,
    gate_snapshot_id: GateSnapshotIdV1,
    gate_id: GateNodeIdV1,
    work_id: WorkIdV1,
    contract_generation_id: ContractGenerationIdV1,
    contract_root_id: ContractRootIdV1,
    scope: AssessmentScopeV1,
    evaluator_contract_id: GateEvaluatorContractIdV1,
    trust_root_snapshot_hash: [u8; 32],
    input_set_hash: [u8; 32],
    inputs: &'a [AssessmentInputRefV1],
    time: AssessmentTimeBasisV1,
    valid_until: Option<u64>,
    result: GateEvaluationResultV1,
    diagnostic_hash: [u8; 32],
}

fn assessment_identity_value(material: &AssessmentIdentityMaterial<'_>) -> CborValue {
    CborValue::Array(vec![
        bytes(material.store_domain_id.as_bytes()),
        bytes(material.gate_snapshot_id.as_bytes()),
        bytes(material.gate_id.as_bytes()),
        bytes(material.work_id.as_bytes()),
        bytes(material.contract_generation_id.as_bytes()),
        bytes(material.contract_root_id.as_bytes()),
        material.scope.canonical_value(),
        bytes(material.evaluator_contract_id.as_bytes()),
        bytes(&material.trust_root_snapshot_hash),
        bytes(&material.input_set_hash),
        CborValue::Array(
            material
                .inputs
                .iter()
                .map(AssessmentInputRefV1::canonical_value)
                .collect(),
        ),
        material.time.canonical_value(),
        CborValue::optional(material.valid_until.map(CborValue::Unsigned)),
        CborValue::Unsigned(material.result.tag()),
        bytes(&material.diagnostic_hash),
    ])
}

fn parse_assessment_input(value: &CborValue) -> Result<AssessmentInputRefV1, AssessmentError> {
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    let [CborValue::Unsigned(tag), value] = fields.as_slice() else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    match (*tag, fields.as_slice()) {
        (
            1,
            [
                observation_id,
                record_hash,
                CborValue::Unsigned(kind),
                payload_semantic_hash,
                store_domain_id,
                CborValue::Unsigned(observed_at),
                payload_object_id,
                CborValue::Array(subjects),
                contributor_hash,
                CborValue::Array(support_roots),
            ],
        ) => {
            let input = ObservationAssessmentInputV1 {
                observation_id: ObservationRecordIdV1::from_bytes(exact_assessment_digest(
                    observation_id,
                )?)?,
                record_hash: exact_assessment_digest(record_hash)?,
                kind: ObservationKindV1::from_tag(*kind)?,
                payload_semantic_hash: exact_assessment_digest(payload_semantic_hash)?,
                store_domain_id: parse_store_domain(store_domain_id)?,
                observed_at: *observed_at,
                payload_object_id: StoreObjectIdV1::from_digest(exact_assessment_digest(
                    payload_object_id,
                )?),
                subjects: subjects
                    .iter()
                    .map(parse_observation_subject)
                    .collect::<Result<Vec<_>, _>>()?,
                contributor_hash: exact_assessment_digest(contributor_hash)?,
                support_roots: support_roots
                    .iter()
                    .map(exact_assessment_digest)
                    .collect::<Result<Vec<_>, _>>()?,
            };
            validate_observation_subject_cardinality(&input.subjects)?;
            validate_support_set(&input.support_roots)?;
            require_nonzero(input.record_hash, "Observation Assessment record")?;
            require_nonzero(
                input.payload_semantic_hash,
                "Observation Assessment payload semantics",
            )?;
            require_nonzero(input.contributor_hash, "Observation Assessment contributor")?;
            Ok(AssessmentInputRefV1::Observation(input))
        }
        (
            2,
            [
                claim_id,
                record_hash,
                store_domain_id,
                submission,
                CborValue::Array(observation_ids),
                CborValue::Unsigned(oldest_observed_at),
                CborValue::Array(payload_object_ids),
                CborValue::Array(observation_scopes),
                CborValue::Array(contributor_hashes),
                CborValue::Array(support_roots),
                subject,
            ],
        ) => {
            let value = ClaimAssessmentInputV1 {
                claim_id: ClaimIdV1::from_bytes(exact_assessment_digest(claim_id)?)?,
                record_hash: exact_assessment_digest(record_hash)?,
                store_domain_id: parse_store_domain(store_domain_id)?,
                submission: parse_submission_ref(submission)
                    .map_err(|_| AssessmentError::InvalidStoredAssessment)?,
                observation_ids: observation_ids
                    .iter()
                    .map(|id| -> Result<ObservationRecordIdV1, AssessmentError> {
                        Ok(ObservationRecordIdV1::from_bytes(exact_assessment_digest(
                            id,
                        )?)?)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                oldest_observed_at: *oldest_observed_at,
                payload_object_ids: payload_object_ids
                    .iter()
                    .map(|id| Ok(StoreObjectIdV1::from_digest(exact_assessment_digest(id)?)))
                    .collect::<Result<Vec<_>, AssessmentError>>()?,
                observation_scopes: observation_scopes
                    .iter()
                    .map(parse_claim_observation_scope)
                    .collect::<Result<Vec<_>, _>>()?,
                contributor_hashes: contributor_hashes
                    .iter()
                    .map(exact_assessment_digest)
                    .collect::<Result<Vec<_>, _>>()?,
                support_roots: support_roots
                    .iter()
                    .map(exact_assessment_digest)
                    .collect::<Result<Vec<_>, _>>()?,
                subject: parse_claim_subject(subject)
                    .map_err(|_| AssessmentError::InvalidStoredAssessment)?,
            };
            value.validate_scope_commitments()?;
            Ok(AssessmentInputRefV1::Claim(value))
        }
        (
            3,
            [
                receipt_id,
                receipt_hash,
                request_id,
                context_id,
                CborValue::Unsigned(basis_kind),
                prior_state_token,
                resulting_state_token,
                authority_snapshot_hash,
                subject_hash,
                CborValue::Unsigned(validated_at),
                CborValue::Unsigned(valid_until),
                step_binding,
            ],
        ) => {
            let value = AuthorizationAssessmentInputV1 {
                receipt_id: AuthorizationReceiptIdV1::from_digest(exact_assessment_digest(
                    receipt_id,
                )?),
                receipt_hash: exact_assessment_digest(receipt_hash)?,
                request_id: ActionRequestIdV1::from_digest(exact_assessment_digest(request_id)?),
                context_id: crate::domain::authority::AuthorityContextIdV1::from_digest(
                    exact_assessment_digest(context_id)?,
                ),
                basis_kind: u8::try_from(*basis_kind)
                    .map_err(|_| AssessmentError::InvalidStoredAssessment)?,
                prior_state_token: crate::domain::authority::StateTokenIdV1::from_digest(
                    exact_assessment_digest(prior_state_token)?,
                ),
                resulting_state_token: crate::domain::authority::StateTokenIdV1::from_digest(
                    exact_assessment_digest(resulting_state_token)?,
                ),
                authority_snapshot_hash: exact_assessment_digest(authority_snapshot_hash)?,
                subject_hash: exact_assessment_digest(subject_hash)?,
                validated_at: *validated_at,
                valid_until: *valid_until,
                step_binding: parse_optional_value(step_binding)?
                    .map(parse_assessment_scope)
                    .transpose()?
                    .map(|scope| match scope {
                        AssessmentScopeV1::Step(binding) => Ok(binding),
                        AssessmentScopeV1::Work => Err(AssessmentError::InvalidStoredAssessment),
                    })
                    .transpose()?,
            };
            let receipt = value.exact_receipt()?;
            let rebuilt = AuthorizationAssessmentInputV1::from_validated_receipt(
                &receipt,
                value.authority_snapshot_hash,
                value.subject_hash,
                value.validated_at,
                value.valid_until,
                value.step_binding,
            )?;
            if rebuilt != value {
                return Err(AssessmentError::InvalidStoredAssessment);
            }
            Ok(AssessmentInputRefV1::AuthorizationReceipt(value))
        }
        (
            4,
            [
                gate_id,
                CborValue::Array(assessment_ids),
                resolution_hash,
                result,
                valid_until,
                CborValue::Unsigned(as_of),
                time_basis_hash,
                CborValue::Array(contributor_hashes),
                CborValue::Array(support_roots),
            ],
        ) => {
            let value = ChildAssessmentResolutionV1 {
                gate_id: GateNodeIdV1::from_bytes(exact_assessment_digest(gate_id)?)?,
                assessment_ids: assessment_ids
                    .iter()
                    .map(|id| -> Result<AssessmentIdV1, AssessmentError> {
                        Ok(AssessmentIdV1::from_bytes(exact_assessment_digest(id)?)?)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                resolution_hash: exact_assessment_digest(resolution_hash)?,
                result: parse_gate_result(result)?,
                valid_until: parse_optional_u64(valid_until)?,
                as_of: *as_of,
                time_basis_hash: exact_assessment_digest(time_basis_hash)?,
                contributor_hashes: contributor_hashes
                    .iter()
                    .map(exact_assessment_digest)
                    .collect::<Result<Vec<_>, _>>()?,
                support_roots: support_roots
                    .iter()
                    .map(exact_assessment_digest)
                    .collect::<Result<Vec<_>, _>>()?,
            };
            if value
                .assessment_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
                || value.as_of == 0
            {
                return Err(AssessmentError::InvalidStoredAssessment);
            }
            require_nonzero(value.resolution_hash, "child Assessment resolution")?;
            require_nonzero(value.time_basis_hash, "child Assessment time basis")?;
            validate_support_set(&value.contributor_hashes)?;
            validate_support_set(&value.support_roots)?;
            Ok(AssessmentInputRefV1::ChildResolution(value))
        }
        _ => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn parse_claim_observation_scope(
    value: &CborValue,
) -> Result<ClaimObservationScopeV1, AssessmentError> {
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    let [observation_id, CborValue::Array(subjects)] = fields.as_slice() else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    Ok(ClaimObservationScopeV1 {
        observation_id: ObservationRecordIdV1::from_bytes(exact_assessment_digest(
            observation_id,
        )?)?,
        subjects: subjects
            .iter()
            .map(parse_observation_subject)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_observation_subject(
    value: &CborValue,
) -> Result<super::observation::ObservationSubjectV1, AssessmentError> {
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    match fields.as_slice() {
        [
            CborValue::Unsigned(1),
            work_id,
            contract_generation_id,
            contract_root_id,
        ] => super::observation::ObservationSubjectV1::for_work(
            exact_assessment_digest(work_id)?,
            ContractGenerationIdV1::parse(&render_assessment_digest(exact_assessment_digest(
                contract_generation_id,
            )?))
            .map_err(|_| AssessmentError::InvalidStoredAssessment)?,
            exact_assessment_digest(contract_root_id)?,
        )
        .map_err(|_| AssessmentError::InvalidStoredAssessment),
        [CborValue::Unsigned(kind), subject_id, revision_id] => {
            let kind = match kind {
                2 => ObservationSubjectKindV1::Step,
                3 => ObservationSubjectKindV1::Submission,
                4 => ObservationSubjectKindV1::Run,
                5 => ObservationSubjectKindV1::Repository,
                6 => ObservationSubjectKindV1::Installation,
                7 => ObservationSubjectKindV1::Authority,
                8 => ObservationSubjectKindV1::Resource,
                9 => ObservationSubjectKindV1::External,
                _ => return Err(AssessmentError::InvalidStoredAssessment),
            };
            super::observation::ObservationSubjectV1::new(
                kind,
                exact_assessment_digest(subject_id)?,
                exact_assessment_digest(revision_id)?,
            )
            .map_err(|_| AssessmentError::InvalidStoredAssessment)
        }
        _ => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn parse_assessment_scope(value: &CborValue) -> Result<AssessmentScopeV1, AssessmentError> {
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(1)] => Ok(AssessmentScopeV1::Work),
        [
            CborValue::Unsigned(2),
            repository_id,
            work_id,
            contract_generation_id,
            contract_root_id,
            step_id,
            revision_id,
        ] => {
            let repository_id = parse_store_domain(repository_id)?;
            let work_id = parse_work_id(work_id)?;
            let scope = StepScopeV1::new(repository_id, work_id);
            Ok(AssessmentScopeV1::Step(
                StepBindingV1::new(
                    scope,
                    parse_contract_generation(contract_generation_id)?,
                    ContractRootIdV1::from_digest(exact_assessment_digest(contract_root_id)?),
                    StepIdV1::from_bytes(scope, exact_assessment_digest(step_id)?)?,
                    StepRevisionIdV1::from_bytes(exact_assessment_digest(revision_id)?)?,
                )
                .map_err(|_| AssessmentError::InvalidStoredAssessment)?,
            ))
        }
        _ => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn parse_assessment_time(value: &CborValue) -> Result<AssessmentTimeBasisV1, AssessmentError> {
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    let [
        input_store_generation_id,
        CborValue::Unsigned(lower_bound),
        CborValue::Unsigned(upper_bound),
        freshness_basis_hash,
        evidence_input_cut_hash,
        complete_input_cut_hash,
    ] = fields.as_slice()
    else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    if *lower_bound == 0 || upper_bound < lower_bound {
        return Err(AssessmentError::InvalidStoredAssessment);
    }
    Ok(AssessmentTimeBasisV1 {
        input_store_generation_id: StoreGenerationIdV1::from_digest(exact_assessment_digest(
            input_store_generation_id,
        )?),
        lower_bound: *lower_bound,
        upper_bound: *upper_bound,
        freshness_basis_hash: exact_assessment_digest(freshness_basis_hash)?,
        evidence_input_cut_hash: exact_assessment_digest(evidence_input_cut_hash)?,
        complete_input_cut_hash: exact_assessment_digest(complete_input_cut_hash)?,
    })
}

fn parse_gate_result(value: &CborValue) -> Result<GateEvaluationResultV1, AssessmentError> {
    let CborValue::Unsigned(tag) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    match tag {
        1 => Ok(GateEvaluationResultV1::Pass),
        2 => Ok(GateEvaluationResultV1::Fail),
        3 => Ok(GateEvaluationResultV1::Indeterminate),
        4 => Ok(GateEvaluationResultV1::Error),
        _ => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn parse_invalidation_reason(
    value: &CborValue,
) -> Result<AssessmentInvalidationReasonV1, AssessmentError> {
    let CborValue::Unsigned(tag) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    match tag {
        1 => Ok(AssessmentInvalidationReasonV1::WorkGenerationAdvanced),
        2 => Ok(AssessmentInvalidationReasonV1::StepRevisionAdvanced),
        3 => Ok(AssessmentInvalidationReasonV1::GateSnapshotChanged),
        4 => Ok(AssessmentInvalidationReasonV1::EvaluatorChanged),
        5 => Ok(AssessmentInvalidationReasonV1::InputTombstoned),
        6 => Ok(AssessmentInvalidationReasonV1::InputCorrected),
        7 => Ok(AssessmentInvalidationReasonV1::FreshnessExpired),
        8 => Ok(AssessmentInvalidationReasonV1::IntegrityFailure),
        9 => Ok(AssessmentInvalidationReasonV1::AuthorizationReceiptRevoked),
        _ => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn parse_store_domain(value: &CborValue) -> Result<StoreDomainIdV1, AssessmentError> {
    StoreDomainIdV1::parse(&render_assessment_digest(exact_assessment_digest(value)?))
        .map_err(|_| AssessmentError::InvalidStoredAssessment)
}

fn parse_work_id(value: &CborValue) -> Result<WorkIdV1, AssessmentError> {
    WorkIdV1::parse(&render_assessment_digest(exact_assessment_digest(value)?))
        .map_err(|_| AssessmentError::InvalidStoredAssessment)
}

fn parse_contract_generation(value: &CborValue) -> Result<ContractGenerationIdV1, AssessmentError> {
    ContractGenerationIdV1::parse(&render_assessment_digest(exact_assessment_digest(value)?))
        .map_err(|_| AssessmentError::InvalidStoredAssessment)
}

fn parse_optional_u64(value: &CborValue) -> Result<Option<u64>, AssessmentError> {
    match parse_optional_value(value)? {
        None => Ok(None),
        Some(CborValue::Unsigned(value)) => Ok(Some(*value)),
        Some(_) => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn parse_optional_digest(value: &CborValue) -> Result<Option<[u8; 32]>, AssessmentError> {
    parse_optional_value(value)?
        .map(exact_assessment_digest)
        .transpose()
}

fn parse_optional_value(value: &CborValue) -> Result<Option<&CborValue>, AssessmentError> {
    let CborValue::Array(fields) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), value] => Ok(Some(value)),
        _ => Err(AssessmentError::InvalidStoredAssessment),
    }
}

fn exact_assessment_digest(value: &CborValue) -> Result<[u8; 32], AssessmentError> {
    let CborValue::Bytes(value) = value else {
        return Err(AssessmentError::InvalidStoredAssessment);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| AssessmentError::InvalidStoredAssessment)
}

fn render_assessment_digest(bytes: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

fn bytes(value: &[u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AssessmentError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] EvidenceIdentityError),
    #[error(transparent)]
    Gate(#[from] GateError),
    #[error(transparent)]
    StepIdentity(#[from] crate::domain::step::StepIdentityError),
    #[error(transparent)]
    Observation(#[from] ObservationError),
    #[error("Assessment references an unknown Gate")]
    UnknownGate,
    #[error("Assessment expected an exact Gate leaf")]
    ExpectedLeaf,
    #[error("Assessment expected an exact composite Gate")]
    ExpectedComposite,
    #[error("Assessment inputs must be resolved, nonempty, unique, and bounded")]
    InvalidInputs,
    #[error("Assessment input records could not be resolved exactly")]
    UnresolvedInput,
    #[error("Assessment inputs do not match the pinned Gate input class")]
    InvalidInputClass,
    #[error("Assessment time or freshness window is invalid")]
    InvalidTimeWindow,
    #[error("trusted time is unavailable for Assessment evaluation")]
    TrustedTimeUnavailable,
    #[error("Assessment Gate scope and exact Step revision differ")]
    StepScopeMismatch,
    #[error("Assessment Work, Contract generation, or Contract Root scope differs")]
    WorkScopeMismatch,
    #[error("Assessment inputs cite more than one Step revision")]
    AmbiguousStepScope,
    #[error("Assessment inputs cite more than one Work revision")]
    AmbiguousWorkScope,
    #[error("Assessment contributor or source-support independence is incomplete or ambiguous")]
    InvalidSupportIndependence,
    #[error("Assessment Evidence inputs cross the selected Store Domain")]
    CrossStoreInput,
    #[error("pinned leaf evaluator identity differs from the Gate contract")]
    EvaluatorMismatch,
    #[error("Assessment invalidation cannot replace an Assessment with itself")]
    SelfReplacement,
    #[error("Assessment resolution received a stale or incoherent Evidence cut")]
    StaleEvidenceCut,
    #[error("Evidence mutation authority is not an exact admitted Evidence Action")]
    InvalidMutationAuthority,
    #[error("complete Evidence cut is duplicated, cross-Store, or otherwise incomplete")]
    IncompleteEvidenceCut,
    #[error("Assessment invalidation targets a record outside the supplied Evidence cut")]
    UnknownInvalidationTarget,
    #[error("stored Assessment or invalidation is malformed or non-canonical")]
    InvalidStoredAssessment,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::authority::{
        ActionAuthorityBasisKindV1, ActionRequestIdV1, AuthorityContextIdV1, PrincipalIdV1,
        RepositoryActionLeafV1, SessionIdV1, StateTokenIdV1,
    };
    use crate::domain::evidence::{
        EvidencePayloadManifestV1, EvidenceRedactionPolicyV1, EvidenceRetentionClassV1,
        EvidenceRetentionPolicyV1, EvidenceSecretScanReceiptV1, ObservationAcquisitionV1,
        ObservationDraftV1, ObservationKindV1, ObservationPayloadCommonV1,
        ObservationPayloadDetailV1, ObservationPayloadV1, ObservationPublicationRouteV1,
        ObservationSubjectKindV1, ObservationSubjectV1, SecurityErasureError,
        SecurityErasurePublicationV1, SubmissionRefV1,
    };
    use crate::domain::gate::{GateEvaluatorContractV1, GateLeafRuleV1, GateNodeV1};
    use crate::domain::identity::{
        CollectionPlanIdV1, ContractRootIdV1, LogicalTombstoneIdV1, StoreObjectIdV1,
    };
    use crate::domain::persistence::ControlledCopyErasurePlanV1;
    use crate::domain::step::{StepIdV1, StepScopeV1, StepSubmissionIdV1};
    use crate::domain::work::WorkSubmissionIdV1;

    fn token(byte: u8) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::digest([byte]).into()
    }

    fn rendered(byte: u8) -> String {
        format!("sha256:{}", format!("{byte:02x}").repeat(32))
    }

    fn store() -> StoreDomainIdV1 {
        StoreDomainIdV1::parse(&rendered(1)).unwrap()
    }

    fn leaf(
        scope: GateScopeV1,
        input_class: GateInputClassV1,
        seed: u8,
        freshness: Option<u64>,
    ) -> GateNodeV1 {
        GateNodeV1::new(
            scope,
            input_class,
            GateOperatorV1::Leaf,
            GateEvaluatorContractV1::leaf(
                match input_class {
                    GateInputClassV1::Evidence => GateLeafRuleV1::EvidenceSetPresent,
                    GateInputClassV1::Authority => GateLeafRuleV1::AuthoritySetPresent,
                    GateInputClassV1::Mixed => GateLeafRuleV1::MixedSetPresent,
                    GateInputClassV1::Composite => panic!("leaf input class cannot be composite"),
                },
                token(seed + 4),
            )
            .unwrap(),
            token(seed + 5),
            freshness,
            vec![],
        )
        .unwrap()
    }

    fn snapshot(roots: Vec<GateNodeIdV1>, nodes: Vec<GateNodeV1>) -> GateSnapshotV1 {
        GateSnapshotV1::new(
            WorkIdV1::derive("stage5-assessment-work").unwrap(),
            ContractGenerationIdV1::parse(&rendered(2)).unwrap(),
            ContractRootIdV1::parse(&rendered(3)).unwrap(),
            crate::domain::identity::ContractComponentIdV1::parse(&rendered(4)).unwrap(),
            token(4),
            token(5),
            roots,
            nodes,
        )
        .unwrap()
    }

    fn step_binding(seed: u8) -> StepBindingV1 {
        let scope = StepScopeV1::new(store(), WorkIdV1::derive("stage5-assessment-work").unwrap());
        StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&rendered(2)).unwrap(),
            ContractRootIdV1::parse(&rendered(3)).unwrap(),
            StepIdV1::from_bytes(scope, token(seed)).unwrap(),
            StepRevisionIdV1::from_bytes(token(seed + 1)).unwrap(),
        )
        .unwrap()
    }

    fn observation(seed: u8, binding: Option<StepBindingV1>, observed_at: u64) -> ObservationV1 {
        observation_with_step_submission(seed, binding, None, observed_at)
    }

    fn observation_with_step_submission(
        seed: u8,
        binding: Option<StepBindingV1>,
        submission_id: Option<StepSubmissionIdV1>,
        observed_at: u64,
    ) -> ObservationV1 {
        let kind = ObservationKindV1::DeterministicProcedure;
        let (store_domain_id, work_id, contract_generation_id, contract_root_id) = binding
            .map_or_else(
                || {
                    (
                        store(),
                        WorkIdV1::derive("stage5-assessment-work").unwrap(),
                        ContractGenerationIdV1::parse(&rendered(2)).unwrap(),
                        ContractRootIdV1::parse(&rendered(3)).unwrap(),
                    )
                },
                |binding| {
                    (
                        binding.scope().repository_id(),
                        binding.scope().work_id(),
                        binding.contract_generation_id(),
                        binding.contract_root_id(),
                    )
                },
            );
        let mut subjects = vec![
            ObservationSubjectV1::for_work(
                *work_id.as_bytes(),
                contract_generation_id,
                *contract_root_id.as_bytes(),
            )
            .unwrap(),
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Repository,
                *store_domain_id.as_bytes(),
                *contract_generation_id.as_bytes(),
            )
            .unwrap(),
        ];
        if let Some(binding) = binding {
            subjects.push(
                ObservationSubjectV1::new(
                    ObservationSubjectKindV1::Step,
                    *binding.step_id().as_bytes(),
                    *binding.revision_id().as_bytes(),
                )
                .unwrap(),
            );
            if let Some(submission_id) = submission_id {
                subjects.push(
                    ObservationSubjectV1::new(
                        ObservationSubjectKindV1::Submission,
                        *submission_id.as_bytes(),
                        *binding.contract_generation_id().as_bytes(),
                    )
                    .unwrap(),
                );
            }
        }
        let procedure_hash = token(seed + 2);
        let environment_hash = token(seed + 3);
        let toolchain_hash = token(seed + 4);
        let recorded_at = observed_at + 1;
        let clock_basis_hash = token(seed + 5);
        let typed_payload = ObservationPayloadV1::new(
            kind,
            ObservationPayloadCommonV1::new(
                &subjects,
                procedure_hash,
                environment_hash,
                toolchain_hash,
                observed_at,
                recorded_at,
                clock_basis_hash,
            )
            .unwrap(),
            ObservationPayloadDetailV1::Deterministic {
                executable_bytes_hash: token(seed + 10),
                executable_version_hash: token(seed + 11),
                arguments_hash: token(seed + 12),
                working_directory_hash: token(seed + 13),
                relevant_environment_hash: token(seed + 14),
                subject_revision_hash: token(seed + 15),
                dirty_state_hash: token(seed + 16),
                exit_status_hash: token(seed + 17),
                stdout_hash: token(seed + 18),
                stderr_hash: token(seed + 19),
            },
        )
        .unwrap();
        let producer = crate::domain::authority::ExecutionProducerV1::SessionBound {
            principal_id: PrincipalIdV1::derive(&format!("assessment-principal-{seed}")).unwrap(),
            session_id: SessionIdV1::derive(&format!("assessment-session-{seed}")).unwrap(),
        };
        let object_id = StoreObjectIdV1::parse(&rendered(seed + 6)).unwrap();
        let redaction = EvidenceRedactionPolicyV1::prohibit_secrets_v1(1_048_576).unwrap();
        let scan = EvidenceSecretScanReceiptV1::scan(
            object_id,
            &typed_payload,
            redaction,
            producer,
            recorded_at,
        )
        .unwrap();
        let retention = EvidenceRetentionPolicyV1::new(
            EvidenceRetentionClassV1::ExplicitSecurityErasureEligible,
            recorded_at + 1_000,
        )
        .unwrap();
        ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: store(),
            subjects,
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
            lineage: vec![],
            payload: EvidencePayloadManifestV1::new(
                kind,
                object_id,
                &typed_payload,
                "application/cbor",
                redaction,
                scan,
                retention,
            )
            .unwrap(),
            acquisition: ObservationAcquisitionV1::effect_free(token(seed + 10), token(seed + 11))
                .unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap()
    }

    fn authority_receipt(seed: &str) -> AuthorizationReceiptV1 {
        AuthorizationReceiptV1::new(
            ActionRequestIdV1::derive(&format!("{seed}-request")).unwrap(),
            AuthorityContextIdV1::derive(&format!("{seed}-context")).unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive(&format!("{seed}-prior")).unwrap(),
            StateTokenIdV1::derive(&format!("{seed}-result")).unwrap(),
        )
        .unwrap()
    }

    fn mutation_authority(
        seed: &str,
        cut: u8,
        action: RepositoryActionLeafV1,
    ) -> EvidenceMutationAuthorityV1 {
        EvidenceMutationAuthorityV1::test_only(
            action,
            authority_receipt(seed),
            token(cut),
            token(cut + 1),
            100,
        )
        .unwrap()
    }

    fn cut(
        assessments: Vec<AssessmentV1>,
        invalidations: Vec<AssessmentInvalidationV1>,
        seed: u8,
    ) -> EvidenceCutV1 {
        EvidenceCutV1::from_current_index(
            store(),
            StoreHeadIdV1::from_digest(token(seed + 10)),
            StoreGenerationIdV1::from_digest(token(seed + 11)),
            StoreObjectIdV1::from_digest(token(seed + 12)),
            token(seed + 1),
            assessments,
            invalidations,
        )
        .unwrap()
    }

    fn time(as_of: u64, cut: u8) -> AssessmentTimeBasisV1 {
        AssessmentTimeBasisV1::from_evidence_cut(
            &self::cut(vec![], vec![], cut),
            TrustedTimeV1::verified(as_of, as_of).unwrap(),
            token(cut),
        )
        .unwrap()
    }

    fn context(
        graph: &GateSnapshotV1,
        scope: AssessmentScopeV1,
        as_of: u64,
        assessment_basis_as_of: u64,
        cut: u8,
    ) -> AssessmentApplicabilityV1 {
        AssessmentApplicabilityV1::new(
            store(),
            StoreGenerationIdV1::from_digest(token(cut + 11)),
            graph,
            scope,
            TrustedTimeV1::verified(as_of, as_of).unwrap(),
            time(assessment_basis_as_of, cut),
        )
        .unwrap()
    }

    struct TestLeafEvaluatorV1 {
        contract_id: GateEvaluatorContractIdV1,
        result: GateEvaluationResultV1,
        diagnostic: [u8; 32],
    }

    impl sealed::PinnedLeafGateEvaluatorSealedV1 for TestLeafEvaluatorV1 {}

    impl PinnedLeafGateEvaluatorV1 for TestLeafEvaluatorV1 {
        fn contract_id(&self) -> GateEvaluatorContractIdV1 {
            self.contract_id
        }

        fn evaluate(
            &self,
            _context: LeafGateEvaluationContextV1<'_>,
        ) -> Result<LeafGateEvaluationOutputV1, AssessmentError> {
            LeafGateEvaluationOutputV1::new(self.result, self.diagnostic)
        }
    }

    fn evaluate(
        graph: &GateSnapshotV1,
        gate: &GateNodeV1,
        scope: AssessmentScopeV1,
        inputs: Vec<AssessmentInputRefV1>,
        as_of: u64,
        cut: u8,
        result: GateEvaluationResultV1,
    ) -> Result<AssessmentV1, AssessmentError> {
        AssessmentV1::evaluate_leaf(
            graph,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: store(),
                scope,
                inputs,
                time: time(as_of, cut),
            },
            &TestLeafEvaluatorV1 {
                contract_id: gate.evaluator().id(),
                result,
                diagnostic: token(cut + 2),
            },
        )
    }

    #[test]
    fn leaf_assessment_uses_pinned_evaluator_and_conservative_freshness() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, Some(10));
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let observation = observation(30, None, 100);
        let input = AssessmentInputRefV1::Observation(
            ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
        );
        let pass = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![input.clone()],
            109,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        assert_eq!(pass.result(), GateEvaluationResultV1::Pass);
        assert_eq!(pass.valid_until(), Some(110));

        let stale = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![input],
            110,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        assert_eq!(stale.result(), GateEvaluationResultV1::Indeterminate);

        let wrong = TestLeafEvaluatorV1 {
            contract_id: GateEvaluatorContractV1::leaf(
                GateLeafRuleV1::EvidenceSetPresent,
                token(94),
            )
            .unwrap()
            .id(),
            result: GateEvaluationResultV1::Pass,
            diagnostic: token(91),
        };
        assert_eq!(
            AssessmentV1::evaluate_leaf(
                &graph,
                gate.id(),
                AssessmentBasisV1 {
                    store_domain_id: store(),
                    scope: AssessmentScopeV1::Work,
                    inputs: vec![AssessmentInputRefV1::Observation(
                        ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
                    )],
                    time: time(105, 40),
                },
                &wrong,
            )
            .unwrap_err(),
            AssessmentError::EvaluatorMismatch
        );
    }

    #[test]
    fn closed_presence_rules_cannot_self_attest_gate_satisfaction() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let observation = observation(30, None, 100);
        let evaluator = ClosedLeafGateEvaluatorV1::new(gate.evaluator().clone()).unwrap();
        let assessment = AssessmentV1::evaluate_leaf(
            &graph,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: store(),
                scope: AssessmentScopeV1::Work,
                inputs: vec![AssessmentInputRefV1::Observation(
                    ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
                )],
                time: time(105, 40),
            },
            &evaluator,
        )
        .unwrap();
        assert_eq!(assessment.result(), GateEvaluationResultV1::Indeterminate);
    }

    #[test]
    fn trusted_time_and_store_domain_fail_closed() {
        assert_eq!(
            AssessmentTimeBasisV1::from_evidence_cut(
                &cut(vec![], vec![], 3),
                TrustedTimeV1::Unavailable,
                token(1),
            )
            .unwrap_err(),
            AssessmentError::TrustedTimeUnavailable
        );
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let observation = observation(30, None, 100);
        let other_store = StoreDomainIdV1::parse(&rendered(99)).unwrap();
        let result = AssessmentV1::evaluate_leaf(
            &graph,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: other_store,
                scope: AssessmentScopeV1::Work,
                inputs: vec![AssessmentInputRefV1::Observation(
                    ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
                )],
                time: time(105, 40),
            },
            &TestLeafEvaluatorV1 {
                contract_id: gate.evaluator().id(),
                result: GateEvaluationResultV1::Pass,
                diagnostic: token(42),
            },
        );
        assert_eq!(result.unwrap_err(), AssessmentError::CrossStoreInput);
    }

    #[test]
    fn foreign_work_and_contract_claims_are_rejected_before_evaluation() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let observation = observation(30, None, 100);
        let submission = SubmissionRefV1::for_work(
            WorkSubmissionIdV1::derive("stage5-foreign-work-submission").unwrap(),
        )
        .unwrap();
        for subject in [
            ClaimSubjectV1::for_work(
                WorkIdV1::derive("stage5-foreign-work").unwrap(),
                graph.contract_root_id(),
                vec![],
            )
            .unwrap(),
            ClaimSubjectV1::for_work(
                graph.work_id(),
                ContractRootIdV1::parse(&rendered(99)).unwrap(),
                vec![],
            )
            .unwrap(),
        ] {
            let claim =
                ClaimV1::new(submission, subject, token(80), vec![observation.id()]).unwrap();
            assert_eq!(
                ClaimAssessmentInputV1::from_claim(&claim, &[&observation]).unwrap_err(),
                AssessmentError::WorkScopeMismatch
            );
        }
    }

    #[test]
    fn step_scope_and_mixed_authority_inputs_are_exact() {
        let binding = step_binding(60);
        let gate = leaf(GateScopeV1::Step, GateInputClassV1::Mixed, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let submission_id = StepSubmissionIdV1::derive("stage5-mixed-submission").unwrap();
        let observation =
            observation_with_step_submission(30, Some(binding), Some(submission_id), 100);
        let claim = ClaimV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            ClaimSubjectV1::for_step(binding, 1).unwrap(),
            token(72),
            vec![observation.id()],
        )
        .unwrap();
        let receipt = authority_receipt("stage5-mixed");
        let raw_observation = AssessmentInputRefV1::Observation(
            ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
        );
        let evidence = AssessmentInputRefV1::Claim(
            ClaimAssessmentInputV1::from_claim(&claim, &[&observation]).unwrap(),
        );
        let authority = AssessmentInputRefV1::AuthorizationReceipt(
            AuthorizationAssessmentInputV1::from_validated_receipt(
                &receipt,
                token(70),
                token(71),
                90,
                120,
                Some(binding),
            )
            .unwrap(),
        );
        let assessment = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Step(binding),
            vec![evidence.clone(), authority.clone()],
            100,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        assert_eq!(assessment.result(), GateEvaluationResultV1::Pass);
        assert_eq!(
            evaluate(
                &graph,
                &gate,
                AssessmentScopeV1::Step(binding),
                vec![raw_observation, authority.clone()],
                100,
                40,
                GateEvaluationResultV1::Pass,
            )
            .unwrap_err(),
            AssessmentError::StepScopeMismatch
        );
        assert_eq!(
            evaluate(
                &graph,
                &gate,
                AssessmentScopeV1::Work,
                vec![evidence.clone()],
                100,
                40,
                GateEvaluationResultV1::Pass,
            )
            .unwrap_err(),
            AssessmentError::StepScopeMismatch
        );
        assert_eq!(
            evaluate(
                &graph,
                &gate,
                AssessmentScopeV1::Step(binding),
                vec![evidence],
                100,
                40,
                GateEvaluationResultV1::Pass,
            )
            .unwrap_err(),
            AssessmentError::InvalidInputClass
        );
    }

    #[test]
    fn conflict_invalidation_and_expiry_never_prefer_pass() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, Some(20));
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let first_observation = observation(30, None, 100);
        let second_observation = observation(50, None, 100);
        let first = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&first_observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        let second = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&second_observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Fail,
        )
        .unwrap();
        let current = context(&graph, AssessmentScopeV1::Work, 110, 105, 40);
        let conflict_cut = cut(vec![first.clone(), second.clone()], vec![], 40);
        let conflict = resolve_gate_assessments(gate.id(), &current, &conflict_cut).unwrap();
        assert_eq!(conflict.result(), GateEvaluationResultV1::Indeterminate);
        assert_eq!(
            conflict.satisfaction(),
            DerivedGateSatisfactionV1::Blocked(GateEvaluationResultV1::Indeterminate)
        );

        let invalidation = AssessmentInvalidationV1::authorized(
            &second,
            AssessmentInvalidationReasonV1::InputCorrected,
            token(80),
            &mutation_authority(
                "stage5-invalidate",
                40,
                RepositoryActionLeafV1::InvalidateAssessment,
            ),
            token(41),
            None,
        )
        .unwrap();
        let resolved_cut = cut(vec![first.clone(), second], vec![invalidation], 40);
        let resolved = resolve_gate_assessments(gate.id(), &current, &resolved_cut).unwrap();
        assert_eq!(resolved.result(), GateEvaluationResultV1::Pass);
        assert_eq!(
            resolved.satisfaction(),
            DerivedGateSatisfactionV1::Satisfied
        );

        let expired = context(&graph, AssessmentScopeV1::Work, 120, 105, 40);
        assert_eq!(
            resolve_gate_assessments(gate.id(), &expired, &cut(vec![first], vec![], 40))
                .unwrap()
                .result(),
            GateEvaluationResultV1::Indeterminate
        );
    }

    #[test]
    fn all_same_result_assessments_remain_applicable_without_newest_selection() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let assessments = [30_u8, 50]
            .into_iter()
            .map(|seed| {
                let observation = observation(seed, None, 100 + u64::from(seed));
                evaluate(
                    &graph,
                    &gate,
                    AssessmentScopeV1::Work,
                    vec![AssessmentInputRefV1::Observation(
                        ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
                    )],
                    200,
                    40,
                    GateEvaluationResultV1::Pass,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let resolution = resolve_gate_assessments(
            gate.id(),
            &context(&graph, AssessmentScopeV1::Work, 201, 200, 40),
            &cut(assessments, vec![], 40),
        )
        .unwrap();
        assert_eq!(resolution.result(), GateEvaluationResultV1::Pass);
        assert_eq!(resolution.applicable_assessment_ids().len(), 2);
    }

    #[test]
    fn applicability_binds_the_complete_historical_time_basis() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let observation = observation(30, None, 100);
        let assessment = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        let mismatched = [
            AssessmentTimeBasisV1::from_evidence_cut(
                &cut(vec![], vec![], 40),
                TrustedTimeV1::verified(104, 105).unwrap(),
                token(40),
            )
            .unwrap(),
            AssessmentTimeBasisV1::from_evidence_cut(
                &cut(vec![], vec![], 40),
                TrustedTimeV1::verified(105, 105).unwrap(),
                token(99),
            )
            .unwrap(),
        ];
        for basis in mismatched {
            let applicability = AssessmentApplicabilityV1::new(
                store(),
                StoreGenerationIdV1::from_digest(token(51)),
                &graph,
                AssessmentScopeV1::Work,
                TrustedTimeV1::verified(110, 110).unwrap(),
                basis,
            )
            .unwrap();
            let resolution = resolve_gate_assessments(
                gate.id(),
                &applicability,
                &cut(vec![assessment.clone()], vec![], 40),
            )
            .unwrap();
            assert_eq!(resolution.result(), GateEvaluationResultV1::Indeterminate);
            assert!(resolution.applicable_assessment_ids().is_empty());
        }
        let cross_generation = AssessmentApplicabilityV1::new(
            store(),
            StoreGenerationIdV1::from_digest(token(99)),
            &graph,
            AssessmentScopeV1::Work,
            TrustedTimeV1::verified(110, 110).unwrap(),
            time(105, 40),
        )
        .unwrap();
        assert_eq!(
            resolve_gate_assessments(
                gate.id(),
                &cross_generation,
                &cut(vec![assessment], vec![], 40),
            )
            .unwrap_err(),
            AssessmentError::StaleEvidenceCut
        );
    }

    #[test]
    fn composite_assessment_consumes_exact_child_resolutions() {
        let first = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let second = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 20, None);
        let composite = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Composite,
            GateOperatorV1::All,
            GateEvaluatorContractV1::composite(GateOperatorV1::All, token(74)).unwrap(),
            token(75),
            None,
            vec![first.id(), second.id()],
        )
        .unwrap();
        let graph = snapshot(
            vec![composite.id()],
            vec![first.clone(), second.clone(), composite.clone()],
        );
        let assess = |gate: &GateNodeV1, seed| {
            let observation = observation(seed, None, 100);
            evaluate(
                &graph,
                gate,
                AssessmentScopeV1::Work,
                vec![AssessmentInputRefV1::Observation(
                    ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
                )],
                105,
                40,
                GateEvaluationResultV1::Pass,
            )
            .unwrap()
        };
        let first_assessment = assess(&first, 30);
        let second_assessment = assess(&second, 50);
        let applicable = context(&graph, AssessmentScopeV1::Work, 105, 105, 40);
        let assessment_cut = cut(vec![first_assessment, second_assessment], vec![], 40);
        let first_resolution =
            resolve_gate_assessments(first.id(), &applicable, &assessment_cut).unwrap();
        let second_resolution =
            resolve_gate_assessments(second.id(), &applicable, &assessment_cut).unwrap();
        let wrong_freshness = AssessmentTimeBasisV1::from_evidence_cut(
            &cut(vec![], vec![], 40),
            TrustedTimeV1::verified(106, 106).unwrap(),
            token(41),
        )
        .unwrap();
        assert_eq!(
            AssessmentV1::evaluate_composite(
                &graph,
                composite.id(),
                store(),
                AssessmentScopeV1::Work,
                wrong_freshness,
                vec![first_resolution.clone(), second_resolution.clone()],
            )
            .unwrap_err(),
            AssessmentError::StaleEvidenceCut
        );
        let assessment = AssessmentV1::evaluate_composite(
            &graph,
            composite.id(),
            store(),
            AssessmentScopeV1::Work,
            time(105, 40),
            vec![second_resolution, first_resolution],
        )
        .unwrap();
        assert_eq!(assessment.result(), GateEvaluationResultV1::Pass);
    }

    #[test]
    fn closed_semantic_evaluator_can_pass_fail_and_derive_satisfaction() {
        let observation = observation(30, None, 100);
        let inputs = vec![AssessmentInputRefV1::Observation(
            ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
        )];
        let rule = GateLeafRuleV1::EvidenceSemanticMatch;
        let parameters =
            ClosedLeafGateEvaluatorV1::semantic_parameters_hash(rule, &inputs).unwrap();
        let gate = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Evidence,
            GateOperatorV1::Leaf,
            GateEvaluatorContractV1::leaf(rule, token(70)).unwrap(),
            parameters,
            None,
            vec![],
        )
        .unwrap();
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let evaluator = ClosedLeafGateEvaluatorV1::new(gate.evaluator().clone()).unwrap();
        let assessment = AssessmentV1::evaluate_leaf(
            &graph,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: store(),
                scope: AssessmentScopeV1::Work,
                inputs: inputs.clone(),
                time: time(105, 40),
            },
            &evaluator,
        )
        .unwrap();
        assert_eq!(assessment.result(), GateEvaluationResultV1::Pass);
        let resolution = resolve_gate_assessments(
            gate.id(),
            &context(&graph, AssessmentScopeV1::Work, 105, 105, 40),
            &cut(vec![assessment], vec![], 40),
        )
        .unwrap();
        assert_eq!(
            resolution.satisfaction(),
            DerivedGateSatisfactionV1::Satisfied
        );

        let failing_gate = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Evidence,
            GateOperatorV1::Leaf,
            GateEvaluatorContractV1::leaf(rule, token(71)).unwrap(),
            token(99),
            None,
            vec![],
        )
        .unwrap();
        let failing_graph = snapshot(vec![failing_gate.id()], vec![failing_gate.clone()]);
        let failing = AssessmentV1::evaluate_leaf(
            &failing_graph,
            failing_gate.id(),
            AssessmentBasisV1 {
                store_domain_id: store(),
                scope: AssessmentScopeV1::Work,
                inputs,
                time: time(105, 40),
            },
            &ClosedLeafGateEvaluatorV1::new(failing_gate.evaluator().clone()).unwrap(),
        )
        .unwrap();
        assert_eq!(failing.result(), GateEvaluationResultV1::Fail);
    }

    #[test]
    fn quorum_requires_pairwise_contributor_and_source_independence() {
        let first = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let second = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 20, None);
        let quorum = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Composite,
            GateOperatorV1::Quorum { required: 2 },
            GateEvaluatorContractV1::composite(GateOperatorV1::Quorum { required: 2 }, token(74))
                .unwrap(),
            token(75),
            None,
            vec![first.id(), second.id()],
        )
        .unwrap();
        let graph = snapshot(
            vec![quorum.id()],
            vec![first.clone(), second.clone(), quorum.clone()],
        );
        let first_observation = observation(30, None, 100);
        let second_observation = observation(50, None, 100);
        let first_input =
            ObservationAssessmentInputV1::from_observation(&first_observation).unwrap();
        let independent_second =
            ObservationAssessmentInputV1::from_observation(&second_observation).unwrap();
        let assess = |gate: &GateNodeV1, input: ObservationAssessmentInputV1| {
            evaluate(
                &graph,
                gate,
                AssessmentScopeV1::Work,
                vec![AssessmentInputRefV1::Observation(input)],
                105,
                40,
                GateEvaluationResultV1::Pass,
            )
            .unwrap()
        };
        let resolve_pair = |left_input: ObservationAssessmentInputV1,
                            right_input: ObservationAssessmentInputV1| {
            let left = assess(&first, left_input);
            let right = assess(&second, right_input);
            let current = context(&graph, AssessmentScopeV1::Work, 105, 105, 40);
            let evidence = cut(vec![left, right], vec![], 40);
            vec![
                resolve_gate_assessments(first.id(), &current, &evidence).unwrap(),
                resolve_gate_assessments(second.id(), &current, &evidence).unwrap(),
            ]
        };

        let independent = AssessmentV1::evaluate_composite(
            &graph,
            quorum.id(),
            store(),
            AssessmentScopeV1::Work,
            time(105, 40),
            resolve_pair(first_input.clone(), independent_second.clone()),
        )
        .unwrap();
        assert_eq!(independent.result(), GateEvaluationResultV1::Pass);

        let same_observation = AssessmentV1::evaluate_composite(
            &graph,
            quorum.id(),
            store(),
            AssessmentScopeV1::Work,
            time(105, 40),
            resolve_pair(first_input.clone(), first_input.clone()),
        )
        .unwrap();
        assert_eq!(
            same_observation.result(),
            GateEvaluationResultV1::Indeterminate
        );

        let mut same_reviewer = independent_second.clone();
        same_reviewer.contributor_hash = first_input.contributor_hash;
        let duplicate_reviewer = AssessmentV1::evaluate_composite(
            &graph,
            quorum.id(),
            store(),
            AssessmentScopeV1::Work,
            time(105, 40),
            resolve_pair(first_input.clone(), same_reviewer),
        )
        .unwrap();
        assert_eq!(
            duplicate_reviewer.result(),
            GateEvaluationResultV1::Indeterminate
        );

        let mut shared_lineage = independent_second;
        shared_lineage.support_roots = first_input.support_roots.clone();
        let common_source = AssessmentV1::evaluate_composite(
            &graph,
            quorum.id(),
            store(),
            AssessmentScopeV1::Work,
            time(105, 40),
            resolve_pair(first_input, shared_lineage),
        )
        .unwrap();
        assert_eq!(
            common_source.result(),
            GateEvaluationResultV1::Indeterminate
        );
    }

    #[test]
    fn claim_assessment_requires_exact_resolved_observations() {
        let resolved_observation = observation(30, None, 100);
        let other = observation(50, None, 100);
        let submission = SubmissionRefV1::for_work(
            WorkSubmissionIdV1::derive("stage5-claim-submission").unwrap(),
        )
        .unwrap();
        let claim = ClaimV1::new(
            submission,
            ClaimSubjectV1::for_work(
                WorkIdV1::derive("stage5-assessment-work").unwrap(),
                ContractRootIdV1::parse(&rendered(3)).unwrap(),
                vec![],
            )
            .unwrap(),
            token(80),
            vec![resolved_observation.id()],
        )
        .unwrap();
        ClaimAssessmentInputV1::from_claim(&claim, &[&resolved_observation]).unwrap();
        assert_eq!(
            ClaimAssessmentInputV1::from_claim(&claim, &[&other]).unwrap_err(),
            AssessmentError::UnresolvedInput
        );
    }

    #[test]
    fn security_erasure_is_authorized_and_couples_all_invalidations() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let composite = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Composite,
            GateOperatorV1::All,
            GateEvaluatorContractV1::composite(GateOperatorV1::All, token(74)).unwrap(),
            token(75),
            None,
            vec![gate.id()],
        )
        .unwrap();
        let graph = snapshot(vec![composite.id()], vec![gate.clone(), composite.clone()]);
        let observation = observation(30, None, 100);
        let assessment = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        let leaf_resolution = resolve_gate_assessments(
            gate.id(),
            &context(&graph, AssessmentScopeV1::Work, 105, 105, 40),
            &cut(vec![assessment.clone()], vec![], 40),
        )
        .unwrap();
        let parent = AssessmentV1::evaluate_composite(
            &graph,
            composite.id(),
            store(),
            AssessmentScopeV1::Work,
            time(105, 40),
            vec![leaf_resolution],
        )
        .unwrap();
        let payload = observation.payload().object_id();
        let authority = mutation_authority(
            "stage5-erasure",
            40,
            RepositoryActionLeafV1::SecurityEraseEvidencePayload,
        );
        let copy_plan = ControlledCopyErasurePlanV1::test_only(payload);
        let publication = SecurityErasurePublicationV1::begin(
            payload,
            &authority,
            vec![&assessment, &parent],
            copy_plan.clone(),
        )
        .unwrap();
        let mut expected_affected = vec![assessment.id(), parent.id()];
        expected_affected.sort_unstable();
        assert_eq!(
            publication.intent().affected_assessments(),
            expected_affected
        );
        assert_eq!(publication.invalidations().len(), 2);
        let receipt = publication
            .intent()
            .finalize(crate::domain::evidence::SecurityErasureFinalizationV1 {
                tombstone_id: LogicalTombstoneIdV1::from_digest(token(91)),
                collection_plan_id: CollectionPlanIdV1::from_digest(token(92)),
                destroyed_payload_hash: token(93),
                physical_absence_receipt_hash: token(94),
                controlled_copy_plan_id: copy_plan.plan_id(),
                controlled_copy_absence_receipt_hash: token(95),
                finalized_at: 101,
            })
            .unwrap();
        assert!(!receipt.is_general_garbage_collection());
        assert_eq!(
            SecurityErasurePublicationV1::begin(
                payload,
                &authority,
                vec![&assessment, &assessment, &parent],
                copy_plan,
            )
            .unwrap_err(),
            SecurityErasureError::DuplicateAssessment
        );
    }

    #[test]
    fn stored_assessment_decoder_rejects_self_consistent_duplicate_inputs() {
        let gate = leaf(GateScopeV1::Work, GateInputClassV1::Evidence, 10, None);
        let graph = snapshot(vec![gate.id()], vec![gate.clone()]);
        let observation = observation(30, None, 100);
        let mut malformed = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        malformed.inputs.push(malformed.inputs[0].clone());
        malformed.input_set_hash = assessment_input_set_hash(&malformed.inputs).unwrap();
        let identity = assessment_identity_value(&AssessmentIdentityMaterial {
            store_domain_id: malformed.store_domain_id,
            gate_snapshot_id: malformed.gate_snapshot_id,
            gate_id: malformed.gate_id,
            work_id: malformed.work_id,
            contract_generation_id: malformed.contract_generation_id,
            contract_root_id: malformed.contract_root_id,
            scope: malformed.scope,
            evaluator_contract_id: malformed.evaluator_contract_id,
            trust_root_snapshot_hash: malformed.trust_root_snapshot_hash,
            input_set_hash: malformed.input_set_hash,
            inputs: &malformed.inputs,
            time: malformed.time,
            valid_until: malformed.valid_until,
            result: malformed.result,
            diagnostic_hash: malformed.diagnostic_hash,
        });
        malformed.id = AssessmentIdV1::from_bytes(
            domain_hash("maestro.vnext.evidence.assessment-id.v1", &identity).unwrap(),
        )
        .unwrap();
        assert_eq!(
            AssessmentV1::from_canonical_bytes(&malformed.canonical_bytes().unwrap()).unwrap_err(),
            AssessmentError::InvalidInputs
        );

        let mut foreign_store = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        foreign_store.store_domain_id =
            StoreDomainIdV1::parse(&rendered(98)).expect("foreign Store identity is valid");
        let identity = assessment_identity_value(&AssessmentIdentityMaterial {
            store_domain_id: foreign_store.store_domain_id,
            gate_snapshot_id: foreign_store.gate_snapshot_id,
            gate_id: foreign_store.gate_id,
            work_id: foreign_store.work_id,
            contract_generation_id: foreign_store.contract_generation_id,
            contract_root_id: foreign_store.contract_root_id,
            scope: foreign_store.scope,
            evaluator_contract_id: foreign_store.evaluator_contract_id,
            trust_root_snapshot_hash: foreign_store.trust_root_snapshot_hash,
            input_set_hash: foreign_store.input_set_hash,
            inputs: &foreign_store.inputs,
            time: foreign_store.time,
            valid_until: foreign_store.valid_until,
            result: foreign_store.result,
            diagnostic_hash: foreign_store.diagnostic_hash,
        });
        foreign_store.id = AssessmentIdV1::from_bytes(
            domain_hash("maestro.vnext.evidence.assessment-id.v1", &identity).unwrap(),
        )
        .unwrap();
        assert_eq!(
            AssessmentV1::from_canonical_bytes(&foreign_store.canonical_bytes().unwrap())
                .unwrap_err(),
            AssessmentError::InvalidStoredAssessment
        );

        let mut recomputed_result = evaluate(
            &graph,
            &gate,
            AssessmentScopeV1::Work,
            vec![AssessmentInputRefV1::Observation(
                ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
            )],
            105,
            40,
            GateEvaluationResultV1::Pass,
        )
        .unwrap();
        recomputed_result.result = GateEvaluationResultV1::Fail;
        recomputed_result.diagnostic_hash = token(97);
        let identity = assessment_identity_value(&AssessmentIdentityMaterial {
            store_domain_id: recomputed_result.store_domain_id,
            gate_snapshot_id: recomputed_result.gate_snapshot_id,
            gate_id: recomputed_result.gate_id,
            work_id: recomputed_result.work_id,
            contract_generation_id: recomputed_result.contract_generation_id,
            contract_root_id: recomputed_result.contract_root_id,
            scope: recomputed_result.scope,
            evaluator_contract_id: recomputed_result.evaluator_contract_id,
            trust_root_snapshot_hash: recomputed_result.trust_root_snapshot_hash,
            input_set_hash: recomputed_result.input_set_hash,
            inputs: &recomputed_result.inputs,
            time: recomputed_result.time,
            valid_until: recomputed_result.valid_until,
            result: recomputed_result.result,
            diagnostic_hash: recomputed_result.diagnostic_hash,
        });
        recomputed_result.id = AssessmentIdV1::from_bytes(
            domain_hash("maestro.vnext.evidence.assessment-id.v1", &identity).unwrap(),
        )
        .unwrap();
        let self_consistent =
            AssessmentV1::from_canonical_bytes(&recomputed_result.canonical_bytes().unwrap())
                .unwrap();
        assert_eq!(
            self_consistent
                .validate_recomputed_from_persisted_snapshot(&graph)
                .unwrap_err(),
            AssessmentError::InvalidStoredAssessment
        );
    }

    #[test]
    fn step_claim_subject_helper_remains_generation_scoped() {
        let work = WorkIdV1::derive("stage5-step-work").unwrap();
        let scope = StepScopeV1::new(store(), work);
        let binding = crate::domain::step::StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&rendered(2)).unwrap(),
            ContractRootIdV1::parse(&rendered(3)).unwrap(),
            StepIdV1::from_bytes(scope, token(95)).unwrap(),
            StepRevisionIdV1::from_bytes(token(96)).unwrap(),
        )
        .unwrap();
        let submission_id = StepSubmissionIdV1::derive("stage5-step-submission").unwrap();
        let submission = SubmissionRefV1::for_step(submission_id).unwrap();
        let valid_observation =
            observation_with_step_submission(30, Some(binding), Some(submission_id), 100);
        let claim = ClaimV1::new(
            submission,
            ClaimSubjectV1::for_step(binding, 7).unwrap(),
            token(97),
            vec![valid_observation.id()],
        )
        .unwrap();
        assert_eq!(
            ClaimAssessmentInputV1::from_claim(&claim, &[&valid_observation])
                .unwrap()
                .subject,
            ClaimSubjectV1::for_step(binding, 7).unwrap()
        );

        let wrong_submission_id =
            StepSubmissionIdV1::derive("stage5-other-step-submission").unwrap();
        let wrong_submission =
            observation_with_step_submission(31, Some(binding), Some(wrong_submission_id), 100);
        let wrong_submission_claim = ClaimV1::new(
            submission,
            ClaimSubjectV1::for_step(binding, 7).unwrap(),
            token(98),
            vec![wrong_submission.id()],
        )
        .unwrap();
        assert_eq!(
            ClaimAssessmentInputV1::from_claim(&wrong_submission_claim, &[&wrong_submission])
                .unwrap_err(),
            AssessmentError::WorkScopeMismatch
        );

        let other_generation_binding = crate::domain::step::StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&rendered(4)).unwrap(),
            binding.contract_root_id(),
            binding.step_id(),
            binding.revision_id(),
        )
        .unwrap();
        let wrong_generation = observation_with_step_submission(
            32,
            Some(other_generation_binding),
            Some(submission_id),
            100,
        );
        let wrong_generation_claim = ClaimV1::new(
            submission,
            ClaimSubjectV1::for_step(binding, 7).unwrap(),
            token(99),
            vec![wrong_generation.id()],
        )
        .unwrap();
        assert_eq!(
            ClaimAssessmentInputV1::from_claim(&wrong_generation_claim, &[&wrong_generation])
                .unwrap_err(),
            AssessmentError::WorkScopeMismatch
        );

        let missing_submission = observation(33, Some(binding), 100);
        let missing_submission_claim = ClaimV1::new(
            submission,
            ClaimSubjectV1::for_step(binding, 7).unwrap(),
            token(100),
            vec![missing_submission.id()],
        )
        .unwrap();
        assert_eq!(
            ClaimAssessmentInputV1::from_claim(&missing_submission_claim, &[&missing_submission])
                .unwrap_err(),
            AssessmentError::WorkScopeMismatch
        );
    }
}
