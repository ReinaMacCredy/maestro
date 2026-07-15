use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::vnext::identity::ContractRootIdV1;
use crate::domain::vnext::step::{StepBindingV1, StepSubmissionIdV1};
use crate::domain::vnext::work::{WorkIdV1, WorkSubmissionIdV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::{
    ClaimIdV1, EvidenceIdentityError, ObservationRecordIdV1, derive_claim_id, domain_hash,
    require_nonzero,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClaimError {
    #[error(transparent)]
    Identity(#[from] EvidenceIdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
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
