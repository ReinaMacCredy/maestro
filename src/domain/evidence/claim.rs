use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::contract::runtime::ContractGenerationIdV1;
use crate::domain::identity::{ContractRootIdV1, StoreDomainIdV1};
use crate::domain::step::{
    StepBindingV1, StepIdV1, StepRevisionIdV1, StepScopeV1, StepSubmissionIdV1,
};
use crate::domain::work::{WorkIdV1, WorkSubmissionIdV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::{
    ClaimIdV1, EvidenceIdentityError, ObservationRecordIdV1, derive_claim_id, domain_hash,
    require_nonzero,
};
use super::observation::{ObservationSubjectKindV1, ObservationSubjectV1, ObservationV1};
use super::submission_claim::{SubmissionClaimSetError, SubmissionClaimSetV1};

pub const CLAIM_RECORD_VERSION_V1: u64 = 1;
pub const CLAIM_RECORD_DOMAIN_V1: &str = "maestro.vnext.evidence.claim-record.v1";
const MAX_CLAIM_OBSERVATION_REFERENCES_V1: usize = 4_096;
const MAX_WORK_CLAIM_STEP_SUBMISSION_REFERENCES_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SubmissionRefKindV1 {
    Work,
    Step,
}

impl SubmissionRefKindV1 {
    pub const fn tag(self) -> u64 {
        match self {
            Self::Work => 1,
            Self::Step => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SubmissionRefV1 {
    Work(WorkSubmissionIdV1),
    Step(StepSubmissionIdV1),
}

impl SubmissionRefV1 {
    pub fn for_work(id: WorkSubmissionIdV1) -> Result<Self, ClaimError> {
        require_nonzero(*id.as_bytes(), "Work Submission")?;
        Ok(Self::Work(id))
    }

    pub fn for_step(id: StepSubmissionIdV1) -> Result<Self, ClaimError> {
        require_nonzero(*id.as_bytes(), "Step Submission")?;
        Ok(Self::Step(id))
    }

    pub const fn kind(self) -> SubmissionRefKindV1 {
        match self {
            Self::Work(_) => SubmissionRefKindV1::Work,
            Self::Step(_) => SubmissionRefKindV1::Step,
        }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        match self {
            Self::Work(id) => id.as_bytes(),
            Self::Step(id) => id.as_bytes(),
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Work(id) => id.render(),
            Self::Step(id) => id.render(),
        }
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind().tag()),
            CborValue::Bytes(self.as_bytes().to_vec()),
        ])
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ClaimError> {
        Ok(deterministic_cbor::encode(&self.canonical_value())?)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClaimSubjectV1 {
    Work {
        work_id: WorkIdV1,
        contract_root_id: ContractRootIdV1,
        current_step_submissions: Vec<StepSubmissionIdV1>,
    },
    Step {
        binding: StepBindingV1,
        lease_fence: u64,
    },
}

impl ClaimSubjectV1 {
    pub fn for_work(
        work_id: WorkIdV1,
        contract_root_id: ContractRootIdV1,
        mut current_step_submissions: Vec<StepSubmissionIdV1>,
    ) -> Result<Self, ClaimError> {
        require_nonzero(*work_id.as_bytes(), "Work Claim Work")?;
        require_nonzero(*contract_root_id.as_bytes(), "Work Claim Contract Root")?;
        if current_step_submissions.len() > MAX_WORK_CLAIM_STEP_SUBMISSION_REFERENCES_V1 {
            return Err(ClaimError::TooManyStepSubmissionReferences);
        }
        current_step_submissions.sort();
        if current_step_submissions
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(ClaimError::DuplicateStepSubmissionReference);
        }
        Ok(Self::Work {
            work_id,
            contract_root_id,
            current_step_submissions,
        })
    }

    pub fn for_step(binding: StepBindingV1, lease_fence: u64) -> Result<Self, ClaimError> {
        validate_step_binding(binding)?;
        if lease_fence == 0 {
            return Err(ClaimError::ZeroLeaseFence);
        }
        Ok(Self::Step {
            binding,
            lease_fence,
        })
    }

    pub fn submission_kind(&self) -> SubmissionRefKindV1 {
        match self {
            Self::Work { .. } => SubmissionRefKindV1::Work,
            Self::Step { .. } => SubmissionRefKindV1::Step,
        }
    }

    pub fn canonical_value(&self) -> CborValue {
        match self {
            Self::Work {
                work_id,
                contract_root_id,
                current_step_submissions,
            } => CborValue::Array(vec![
                CborValue::Unsigned(SubmissionRefKindV1::Work.tag()),
                CborValue::Bytes(work_id.as_bytes().to_vec()),
                CborValue::Bytes(contract_root_id.as_bytes().to_vec()),
                CborValue::Array(
                    current_step_submissions
                        .iter()
                        .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                        .collect(),
                ),
            ]),
            Self::Step {
                binding,
                lease_fence,
            } => CborValue::Array(vec![
                CborValue::Unsigned(SubmissionRefKindV1::Step.tag()),
                CborValue::Bytes(binding.scope().repository_id().as_bytes().to_vec()),
                CborValue::Bytes(binding.scope().work_id().as_bytes().to_vec()),
                CborValue::Bytes(binding.contract_generation_id().as_bytes().to_vec()),
                CborValue::Bytes(binding.contract_root_id().as_bytes().to_vec()),
                CborValue::Bytes(binding.step_id().as_bytes().to_vec()),
                CborValue::Bytes(binding.revision_id().as_bytes().to_vec()),
                CborValue::Unsigned(*lease_fence),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimV1 {
    claim_id: ClaimIdV1,
    submission: SubmissionRefV1,
    subject: ClaimSubjectV1,
    normalized_proposition_hash: [u8; 32],
    observation_refs: Vec<ObservationRecordIdV1>,
    record_hash: [u8; 32],
}

impl ClaimV1 {
    pub fn new(
        submission: SubmissionRefV1,
        subject: ClaimSubjectV1,
        normalized_proposition_hash: [u8; 32],
        mut observation_refs: Vec<ObservationRecordIdV1>,
    ) -> Result<Self, ClaimError> {
        if submission.kind() != subject.submission_kind() {
            return Err(ClaimError::SubmissionSubjectMismatch);
        }
        require_nonzero(normalized_proposition_hash, "normalized Claim proposition")?;
        if observation_refs.is_empty() {
            return Err(ClaimError::EmptyObservationReferences);
        }
        if observation_refs.len() > MAX_CLAIM_OBSERVATION_REFERENCES_V1 {
            return Err(ClaimError::TooManyObservationReferences);
        }
        observation_refs.sort();
        let unique: BTreeSet<_> = observation_refs.iter().collect();
        if unique.len() != observation_refs.len() {
            return Err(ClaimError::DuplicateObservationReference);
        }

        let identity_value = claim_identity_value(
            submission,
            &subject,
            normalized_proposition_hash,
            &observation_refs,
        );
        let claim_id = derive_claim_id(&identity_value)?;
        let record_value = claim_record_value(claim_id, &identity_value);
        let record_hash = domain_hash(CLAIM_RECORD_DOMAIN_V1, &record_value)?;
        Ok(Self {
            claim_id,
            submission,
            subject,
            normalized_proposition_hash,
            observation_refs,
            record_hash,
        })
    }

    pub fn claim_id(&self) -> ClaimIdV1 {
        self.claim_id
    }

    pub fn submission(&self) -> SubmissionRefV1 {
        self.submission
    }

    pub fn subject(&self) -> &ClaimSubjectV1 {
        &self.subject
    }

    pub fn normalized_proposition_hash(&self) -> &[u8; 32] {
        &self.normalized_proposition_hash
    }

    pub fn observation_refs(&self) -> &[ObservationRecordIdV1] {
        &self.observation_refs
    }

    pub fn record_hash(&self) -> &[u8; 32] {
        &self.record_hash
    }

    pub fn canonical_value(&self) -> CborValue {
        claim_record_value(
            self.claim_id,
            &claim_identity_value(
                self.submission,
                &self.subject,
                self.normalized_proposition_hash,
                &self.observation_refs,
            ),
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ClaimError> {
        Ok(deterministic_cbor::encode(&self.canonical_value())?)
    }

    pub fn from_canonical_bytes(value: &[u8]) -> Result<Self, ClaimError> {
        let decoded = deterministic_cbor::decode(value)?;
        let claim = Self::from_canonical_value(&decoded)?;
        if claim.canonical_bytes()? != value {
            return Err(ClaimError::InvalidStoredClaim);
        }
        Ok(claim)
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, ClaimError> {
        let CborValue::Array(fields) = value else {
            return Err(ClaimError::InvalidStoredClaim);
        };
        let [
            CborValue::Unsigned(version),
            claim_id,
            submission,
            subject,
            normalized_proposition_hash,
            CborValue::Array(observation_refs),
        ] = fields.as_slice()
        else {
            return Err(ClaimError::InvalidStoredClaim);
        };
        if *version != CLAIM_RECORD_VERSION_V1 {
            return Err(ClaimError::InvalidStoredClaim);
        }
        let claim_id = ClaimIdV1::from_bytes(exact_claim_digest(claim_id)?)
            .map_err(|_| ClaimError::InvalidStoredClaim)?;
        let submission = parse_submission_ref(submission)?;
        let subject = parse_claim_subject(subject)?;
        let normalized_proposition_hash = exact_claim_digest(normalized_proposition_hash)?;
        let observation_refs = observation_refs
            .iter()
            .map(|reference| {
                ObservationRecordIdV1::from_bytes(exact_claim_digest(reference)?)
                    .map_err(|_| ClaimError::InvalidStoredClaim)
            })
            .collect::<Result<Vec<_>, ClaimError>>()?;
        let rebuilt = Self::new(
            submission,
            subject,
            normalized_proposition_hash,
            observation_refs,
        )?;
        if rebuilt.claim_id != claim_id || rebuilt.canonical_value() != *value {
            return Err(ClaimError::InvalidStoredClaim);
        }
        Ok(rebuilt)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceClaimPublicationV1 {
    claims: Vec<ClaimV1>,
    observations: Vec<ObservationV1>,
    claim_set: SubmissionClaimSetV1,
}

impl EvidenceClaimPublicationV1 {
    pub fn new(
        submission: SubmissionRefV1,
        mut claims: Vec<ClaimV1>,
        mut observations: Vec<ObservationV1>,
    ) -> Result<Self, ClaimError> {
        claims.sort_by(|left, right| {
            (left.normalized_proposition_hash(), left.claim_id())
                .cmp(&(right.normalized_proposition_hash(), right.claim_id()))
        });
        observations.sort_unstable_by_key(ObservationV1::id);
        if observations
            .windows(2)
            .any(|pair| pair[0].id() == pair[1].id())
        {
            return Err(ClaimError::DuplicateObservationRecord);
        }
        let resolved = observations
            .iter()
            .map(ObservationV1::id)
            .collect::<BTreeSet<_>>();
        let referenced = claims
            .iter()
            .flat_map(|claim| claim.observation_refs().iter().copied())
            .collect::<BTreeSet<_>>();
        if referenced.iter().any(|id| !resolved.contains(id)) {
            return Err(ClaimError::UnresolvedObservationReference);
        }
        if resolved.iter().any(|id| !referenced.contains(id)) {
            return Err(ClaimError::UnreferencedObservationRecord);
        }
        let observations_by_id = observations
            .iter()
            .map(|observation| (observation.id(), observation))
            .collect::<std::collections::BTreeMap<_, _>>();
        for claim in &claims {
            for observation_id in claim.observation_refs() {
                validate_claim_observation_subjects(
                    claim.submission(),
                    claim.subject(),
                    observations_by_id
                        .get(observation_id)
                        .expect("invariant: all Claim Observation references were resolved")
                        .store_domain_id(),
                    observations_by_id
                        .get(observation_id)
                        .expect("invariant: all Claim Observation references were resolved")
                        .subjects(),
                )?;
            }
        }
        let claim_set = SubmissionClaimSetV1::from_claims(submission, &claims)?;
        Ok(Self {
            claims,
            observations,
            claim_set,
        })
    }

    pub fn claims(&self) -> &[ClaimV1] {
        &self.claims
    }

    pub const fn claim_set(&self) -> &SubmissionClaimSetV1 {
        &self.claim_set
    }

    pub fn observations(&self) -> &[ObservationV1] {
        &self.observations
    }

    pub fn claim_set_value(&self) -> Result<CborValue, ClaimError> {
        Ok(self.claim_set.schema_value()?)
    }

    pub fn canonical_value(&self) -> Result<CborValue, ClaimError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence.claim-publication.v1")?,
            self.claim_set_value()?,
            CborValue::Array(self.claims.iter().map(ClaimV1::canonical_value).collect()),
            CborValue::Array(
                self.observations
                    .iter()
                    .map(ObservationV1::canonical_value)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        ]))
    }
}

pub(super) fn validate_claim_observation_subjects(
    submission: SubmissionRefV1,
    claim_subject: &ClaimSubjectV1,
    observation_store_domain_id: StoreDomainIdV1,
    observation_subjects: &[ObservationSubjectV1],
) -> Result<(), ClaimError> {
    let subject_kind_count = |kind: ObservationSubjectKindV1| {
        observation_subjects
            .iter()
            .filter(|subject| subject.kind() == kind)
            .count()
    };
    let exact_subject_count =
        |kind: ObservationSubjectKindV1, subject_id: &[u8; 32], revision_id: &[u8; 32]| {
            observation_subjects
                .iter()
                .filter(|subject| subject.kind() == kind)
                .filter(|subject| {
                    subject.subject_id() == subject_id && subject.revision_id() == revision_id
                })
                .count()
        };
    match claim_subject {
        ClaimSubjectV1::Work {
            work_id,
            contract_root_id,
            ..
        } => {
            let work_subjects = observation_subjects
                .iter()
                .filter(|subject| subject.kind() == ObservationSubjectKindV1::Work)
                .filter(|subject| {
                    subject.subject_id() == work_id.as_bytes()
                        && subject.revision_id() == contract_root_id.as_bytes()
                })
                .collect::<Vec<_>>();
            let Some(work_subject) = work_subjects.first() else {
                return Err(ClaimError::ObservationSubjectMismatch);
            };
            let Some(contract_generation_id) = work_subject.contract_generation_id() else {
                return Err(ClaimError::ObservationSubjectMismatch);
            };
            if work_subjects.len() != 1
                || subject_kind_count(ObservationSubjectKindV1::Work) != 1
                || exact_subject_count(
                    ObservationSubjectKindV1::Repository,
                    observation_store_domain_id.as_bytes(),
                    contract_generation_id.as_bytes(),
                ) != 1
                || subject_kind_count(ObservationSubjectKindV1::Repository) != 1
                || observation_subjects.iter().any(|subject| {
                    matches!(
                        subject.kind(),
                        ObservationSubjectKindV1::Step
                            | ObservationSubjectKindV1::Submission
                            | ObservationSubjectKindV1::Run
                    )
                })
            {
                return Err(ClaimError::ObservationSubjectMismatch);
            }
        }
        ClaimSubjectV1::Step { binding, .. } => {
            if observation_store_domain_id != binding.scope().repository_id()
                || exact_subject_count(
                    ObservationSubjectKindV1::Repository,
                    binding.scope().repository_id().as_bytes(),
                    binding.contract_generation_id().as_bytes(),
                ) != 1
                || observation_subjects
                    .iter()
                    .filter(|subject| subject.kind() == ObservationSubjectKindV1::Work)
                    .filter(|subject| {
                        subject.subject_id() == binding.scope().work_id().as_bytes()
                            && subject.contract_generation_id()
                                == Some(binding.contract_generation_id())
                            && subject.revision_id() == binding.contract_root_id().as_bytes()
                    })
                    .count()
                    != 1
                || exact_subject_count(
                    ObservationSubjectKindV1::Step,
                    binding.step_id().as_bytes(),
                    binding.revision_id().as_bytes(),
                ) != 1
                || exact_subject_count(
                    ObservationSubjectKindV1::Submission,
                    submission.as_bytes(),
                    binding.contract_generation_id().as_bytes(),
                ) != 1
                || subject_kind_count(ObservationSubjectKindV1::Work) != 1
                || subject_kind_count(ObservationSubjectKindV1::Step) != 1
                || subject_kind_count(ObservationSubjectKindV1::Submission) != 1
                || subject_kind_count(ObservationSubjectKindV1::Repository) != 1
                || observation_subjects.iter().any(|subject| {
                    subject.kind() == ObservationSubjectKindV1::Run
                        || (subject.kind() == ObservationSubjectKindV1::Repository
                            && subject.subject_id() != binding.scope().repository_id().as_bytes())
                })
            {
                return Err(ClaimError::ObservationSubjectMismatch);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClaimError {
    #[error(transparent)]
    Identity(#[from] EvidenceIdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    SubmissionClaimSet(#[from] SubmissionClaimSetError),
    #[error("Claim Submission kind does not match its immutable subject binding")]
    SubmissionSubjectMismatch,
    #[error("Step Claim Lease fence must be positive")]
    ZeroLeaseFence,
    #[error("Claim must cite at least one direct Observation")]
    EmptyObservationReferences,
    #[error(
        "Claim exceeds the finite v1 limit of {MAX_CLAIM_OBSERVATION_REFERENCES_V1} Observation references"
    )]
    TooManyObservationReferences,
    #[error("Claim contains a duplicate Observation reference")]
    DuplicateObservationReference,
    #[error(
        "Work Claim exceeds the finite v1 limit of {MAX_WORK_CLAIM_STEP_SUBMISSION_REFERENCES_V1} current Step Submission references"
    )]
    TooManyStepSubmissionReferences,
    #[error("Work Claim contains a duplicate current Step Submission reference")]
    DuplicateStepSubmissionReference,
    #[error("stored Claim carrier is malformed or non-canonical")]
    InvalidStoredClaim,
    #[error("Claim publication repeats an Observation record")]
    DuplicateObservationRecord,
    #[error("Claim publication contains an unresolved Observation reference")]
    UnresolvedObservationReference,
    #[error("Claim publication contains an Observation not cited by any Claim")]
    UnreferencedObservationRecord,
    #[error("Claim Observation subjects do not exactly match the Work/Step/Submission scope")]
    ObservationSubjectMismatch,
    #[error(transparent)]
    Observation(#[from] super::observation::ObservationError),
}

pub(super) fn parse_submission_ref(value: &CborValue) -> Result<SubmissionRefV1, ClaimError> {
    let CborValue::Array(fields) = value else {
        return Err(ClaimError::InvalidStoredClaim);
    };
    let [CborValue::Unsigned(kind), identity] = fields.as_slice() else {
        return Err(ClaimError::InvalidStoredClaim);
    };
    let identity = exact_claim_digest(identity)?;
    match *kind {
        1 => WorkSubmissionIdV1::parse(&render_claim_digest(identity))
            .map(SubmissionRefV1::Work)
            .map_err(|_| ClaimError::InvalidStoredClaim),
        2 => StepSubmissionIdV1::from_bytes(identity)
            .map(SubmissionRefV1::Step)
            .map_err(|_| ClaimError::InvalidStoredClaim),
        _ => Err(ClaimError::InvalidStoredClaim),
    }
}

pub(super) fn parse_claim_subject(value: &CborValue) -> Result<ClaimSubjectV1, ClaimError> {
    let CborValue::Array(fields) = value else {
        return Err(ClaimError::InvalidStoredClaim);
    };
    match fields.as_slice() {
        [
            CborValue::Unsigned(1),
            work_id,
            contract_root_id,
            CborValue::Array(current_step_submissions),
        ] => {
            let work_id = WorkIdV1::parse(&render_claim_digest(exact_claim_digest(work_id)?))
                .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let contract_root_id = ContractRootIdV1::parse(&render_claim_digest(
                exact_claim_digest(contract_root_id)?,
            ))
            .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let current_step_submissions = current_step_submissions
                .iter()
                .map(|submission| {
                    StepSubmissionIdV1::from_bytes(exact_claim_digest(submission)?)
                        .map_err(|_| ClaimError::InvalidStoredClaim)
                })
                .collect::<Result<Vec<_>, ClaimError>>()?;
            ClaimSubjectV1::for_work(work_id, contract_root_id, current_step_submissions)
        }
        [
            CborValue::Unsigned(2),
            repository_id,
            work_id,
            contract_generation_id,
            contract_root_id,
            step_id,
            revision_id,
            CborValue::Unsigned(lease_fence),
        ] => {
            let repository_id =
                StoreDomainIdV1::parse(&render_claim_digest(exact_claim_digest(repository_id)?))
                    .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let work_id = WorkIdV1::parse(&render_claim_digest(exact_claim_digest(work_id)?))
                .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let scope = StepScopeV1::new(repository_id, work_id);
            let contract_generation_id = ContractGenerationIdV1::parse(&render_claim_digest(
                exact_claim_digest(contract_generation_id)?,
            ))
            .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let contract_root_id = ContractRootIdV1::parse(&render_claim_digest(
                exact_claim_digest(contract_root_id)?,
            ))
            .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let step_id = StepIdV1::from_bytes(scope, exact_claim_digest(step_id)?)
                .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let revision_id = StepRevisionIdV1::from_bytes(exact_claim_digest(revision_id)?)
                .map_err(|_| ClaimError::InvalidStoredClaim)?;
            let binding = StepBindingV1::new(
                scope,
                contract_generation_id,
                contract_root_id,
                step_id,
                revision_id,
            )
            .map_err(|_| ClaimError::InvalidStoredClaim)?;
            ClaimSubjectV1::for_step(binding, *lease_fence)
        }
        _ => Err(ClaimError::InvalidStoredClaim),
    }
}

fn exact_claim_digest(value: &CborValue) -> Result<[u8; 32], ClaimError> {
    let CborValue::Bytes(bytes) = value else {
        return Err(ClaimError::InvalidStoredClaim);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| ClaimError::InvalidStoredClaim)
}

fn render_claim_digest(bytes: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

fn claim_identity_value(
    submission: SubmissionRefV1,
    subject: &ClaimSubjectV1,
    normalized_proposition_hash: [u8; 32],
    observation_refs: &[ObservationRecordIdV1],
) -> CborValue {
    CborValue::Array(vec![
        submission.canonical_value(),
        subject.canonical_value(),
        CborValue::Bytes(normalized_proposition_hash.to_vec()),
        CborValue::Array(
            observation_refs
                .iter()
                .map(|reference| CborValue::Bytes(reference.as_bytes().to_vec()))
                .collect(),
        ),
    ])
}

fn claim_record_value(claim_id: ClaimIdV1, identity_value: &CborValue) -> CborValue {
    let CborValue::Array(identity_fields) = identity_value else {
        unreachable!("invariant: Claim identity material is an array")
    };
    let mut fields = Vec::with_capacity(identity_fields.len() + 2);
    fields.push(CborValue::Unsigned(CLAIM_RECORD_VERSION_V1));
    fields.push(CborValue::Bytes(claim_id.as_bytes().to_vec()));
    fields.extend(identity_fields.iter().cloned());
    CborValue::Array(fields)
}

fn validate_step_binding(binding: StepBindingV1) -> Result<(), ClaimError> {
    for (label, bytes) in [
        (
            "Step Claim repository",
            *binding.scope().repository_id().as_bytes(),
        ),
        ("Step Claim Work", *binding.scope().work_id().as_bytes()),
        (
            "Step Claim Contract Generation",
            *binding.contract_generation_id().as_bytes(),
        ),
        (
            "Step Claim Contract Root",
            *binding.contract_root_id().as_bytes(),
        ),
        ("Step Claim Step", *binding.step_id().as_bytes()),
        ("Step Claim revision", *binding.revision_id().as_bytes()),
    ] {
        require_nonzero(bytes, label)?;
    }
    Ok(())
}
