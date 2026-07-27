use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::evidence::{ClaimError, ClaimSubjectV1, ClaimV1, SubmissionRefV1};
use crate::domain::evidence::{SubmissionClaimSetError, SubmissionClaimSetV1};
use crate::domain::identity::ContractRootIdV1;
use crate::domain::step::StepSubmissionIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{WorkIdV1, WorkSubmissionIdV1};

pub const WORK_SUBMISSION_VERSION_V1: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkRecordWriterV1 {
    Work,
    Contract,
    Step,
    Execution,
    Evidence,
    GatePolicy,
}

impl WorkRecordWriterV1 {
    pub const ALL: [Self; 6] = [
        Self::Work,
        Self::Contract,
        Self::Step,
        Self::Execution,
        Self::Evidence,
        Self::GatePolicy,
    ];

    pub const fn tag(self) -> u64 {
        match self {
            Self::Work => 1,
            Self::Contract => 2,
            Self::Step => 3,
            Self::Execution => 4,
            Self::Evidence => 5,
            Self::GatePolicy => 6,
        }
    }

    pub fn from_tag(tag: u64) -> Result<Self, WorkSubmissionError> {
        match tag {
            1 => Ok(Self::Work),
            2 => Ok(Self::Contract),
            3 => Ok(Self::Step),
            4 => Ok(Self::Execution),
            5 => Ok(Self::Evidence),
            6 => Ok(Self::GatePolicy),
            _ => Err(WorkSubmissionError::UnknownWriterTag(tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkSubmissionSubjectV1 {
    work_id: WorkIdV1,
    contract_root: ContractRootIdV1,
    current_step_submissions: Vec<StepSubmissionIdV1>,
}

impl WorkSubmissionSubjectV1 {
    pub fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub fn contract_root(&self) -> ContractRootIdV1 {
        self.contract_root
    }

    pub fn current_step_submissions(&self) -> &[StepSubmissionIdV1] {
        &self.current_step_submissions
    }

    fn canonical_value(&self) -> CborValue {
        ClaimSubjectV1::Work {
            work_id: self.work_id,
            contract_root_id: self.contract_root,
            current_step_submissions: self.current_step_submissions.clone(),
        }
        .canonical_value()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkSubmissionV1 {
    id: WorkSubmissionIdV1,
    subject: WorkSubmissionSubjectV1,
    expected_work_revision: u64,
    claim_set: SubmissionClaimSetV1,
    digest: [u8; 32],
}

impl WorkSubmissionV1 {
    pub fn publish_from_claims(
        writer: WorkRecordWriterV1,
        id: WorkSubmissionIdV1,
        work_id: WorkIdV1,
        contract_root: ContractRootIdV1,
        expected_work_revision: u64,
        claims: &[ClaimV1],
    ) -> Result<Self, WorkSubmissionError> {
        validate_work_submission_header(writer, contract_root, expected_work_revision)?;
        let submission_ref = SubmissionRefV1::for_work(id)?;
        let claim_set = SubmissionClaimSetV1::from_claims(submission_ref, claims)?;
        Self::publish(
            writer,
            id,
            work_id,
            contract_root,
            expected_work_revision,
            claim_set,
        )
    }

    pub fn publish(
        writer: WorkRecordWriterV1,
        id: WorkSubmissionIdV1,
        work_id: WorkIdV1,
        contract_root: ContractRootIdV1,
        expected_work_revision: u64,
        claim_set: SubmissionClaimSetV1,
    ) -> Result<Self, WorkSubmissionError> {
        validate_work_submission_header(writer, contract_root, expected_work_revision)?;
        if claim_set.claim_count() == 0 {
            return Err(WorkSubmissionError::EmptyClaimSet);
        }
        let expected_submission = SubmissionRefV1::for_work(id)?;
        let Some(actual_submission) = claim_set.submission_ref() else {
            return Err(WorkSubmissionError::NonAuthoritativeClaimSet);
        };
        if actual_submission != expected_submission
            || claim_set.submission_id() != id.render().as_bytes()
        {
            return Err(WorkSubmissionError::ClaimSetSubmissionMismatch);
        }
        let Some(claim_subjects) = claim_set.claim_subjects() else {
            return Err(WorkSubmissionError::NonAuthoritativeClaimSet);
        };
        let subject = validate_work_claim_subjects(claim_subjects, work_id, contract_root)?;
        let digest = compute_digest(id, &subject, expected_work_revision, &claim_set)?;
        Ok(Self {
            id,
            subject,
            expected_work_revision,
            claim_set,
            digest,
        })
    }

    pub fn id(&self) -> WorkSubmissionIdV1 {
        self.id
    }

    pub fn work_id(&self) -> WorkIdV1 {
        self.subject.work_id()
    }

    pub fn contract_root(&self) -> ContractRootIdV1 {
        self.subject.contract_root()
    }

    pub fn current_step_submissions(&self) -> &[StepSubmissionIdV1] {
        self.subject.current_step_submissions()
    }

    pub fn subject(&self) -> &WorkSubmissionSubjectV1 {
        &self.subject
    }

    pub fn expected_work_revision(&self) -> u64 {
        self.expected_work_revision
    }

    pub fn claim_set(&self) -> &SubmissionClaimSetV1 {
        &self.claim_set
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn canonical_value(&self) -> Result<CborValue, WorkSubmissionError> {
        submission_value(
            self.id,
            &self.subject,
            self.expected_work_revision,
            &self.claim_set,
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, WorkSubmissionError> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    pub fn from_canonical_bytes(
        value: &[u8],
        claims: &[ClaimV1],
    ) -> Result<Self, WorkSubmissionError> {
        let decoded = deterministic_cbor::decode(value)?;
        let CborValue::Array(fields) = &decoded else {
            return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
        };
        let [
            CborValue::Unsigned(version),
            id,
            subject,
            CborValue::Unsigned(expected_work_revision),
            claim_set,
        ] = fields.as_slice()
        else {
            return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
        };
        if *version != WORK_SUBMISSION_VERSION_V1 {
            return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
        }
        let id = WorkSubmissionIdV1::parse(&render_digest(exact_digest(id)?))
            .map_err(|_| WorkSubmissionError::InvalidStoredWorkSubmission)?;
        let CborValue::Array(subject_fields) = subject else {
            return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
        };
        let [
            CborValue::Unsigned(1),
            work_id,
            contract_root,
            CborValue::Array(current_step_submissions),
        ] = subject_fields.as_slice()
        else {
            return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
        };
        let work_id = WorkIdV1::parse(&render_digest(exact_digest(work_id)?))
            .map_err(|_| WorkSubmissionError::InvalidStoredWorkSubmission)?;
        let contract_root = ContractRootIdV1::parse(&render_digest(exact_digest(contract_root)?))
            .map_err(|_| WorkSubmissionError::InvalidStoredWorkSubmission)?;
        let current_step_submissions = current_step_submissions
            .iter()
            .map(|id| {
                StepSubmissionIdV1::from_bytes(exact_digest(id)?)
                    .map_err(|_| WorkSubmissionError::InvalidStoredWorkSubmission)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rebuilt = Self::publish_from_claims(
            WorkRecordWriterV1::Work,
            id,
            work_id,
            contract_root,
            *expected_work_revision,
            claims,
        )?;
        if rebuilt.current_step_submissions() != current_step_submissions
            || rebuilt.claim_set().schema_value()? != *claim_set
            || rebuilt.canonical_bytes()? != value
        {
            return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
        }
        Ok(rebuilt)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkSubmissionError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    ClaimSet(#[from] SubmissionClaimSetError),
    #[error(transparent)]
    Claim(#[from] ClaimError),
    #[error("unknown Work record writer tag {0}")]
    UnknownWriterTag(u64),
    #[error("{0:?} cannot publish a Work-owned Submission")]
    ForeignWriter(WorkRecordWriterV1),
    #[error("Work Submission expected revision must be positive")]
    InvalidExpectedRevision,
    #[error("Work Submission Contract Root must not be all-zero")]
    EmptyContractRoot,
    #[error("Work Submission requires one finite non-empty SubmissionClaimSetV1")]
    EmptyClaimSet,
    #[error("Work Submission refuses a non-authoritative Stage0 ClaimSet carrier")]
    NonAuthoritativeClaimSet,
    #[error("SubmissionClaimSetV1 submission_id must equal the Work Submission identity")]
    ClaimSetSubmissionMismatch,
    #[error(
        "every Work Submission Claim must bind one exact Work, Contract Root, and current Step Submission closure"
    )]
    ClaimSubjectMismatch,
    #[error("stored Work Submission V1 is malformed, substituted, or non-canonical")]
    InvalidStoredWorkSubmission,
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], WorkSubmissionError> {
    let CborValue::Bytes(value) = value else {
        return Err(WorkSubmissionError::InvalidStoredWorkSubmission);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| WorkSubmissionError::InvalidStoredWorkSubmission)
}

fn render_digest(value: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in value {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

fn validate_work_claim_subjects<'a>(
    subjects: impl IntoIterator<Item = &'a ClaimSubjectV1>,
    work_id: WorkIdV1,
    contract_root: ContractRootIdV1,
) -> Result<WorkSubmissionSubjectV1, WorkSubmissionError> {
    let mut subjects = subjects.into_iter();
    let first = subjects.next().ok_or(WorkSubmissionError::EmptyClaimSet)?;
    let ClaimSubjectV1::Work {
        work_id: claim_work_id,
        contract_root_id,
        current_step_submissions,
    } = first
    else {
        return Err(WorkSubmissionError::ClaimSubjectMismatch);
    };
    if *claim_work_id != work_id
        || *contract_root_id != contract_root
        || subjects.any(|subject| subject != first)
    {
        return Err(WorkSubmissionError::ClaimSubjectMismatch);
    }
    Ok(WorkSubmissionSubjectV1 {
        work_id,
        contract_root,
        current_step_submissions: current_step_submissions.clone(),
    })
}

fn validate_work_submission_header(
    writer: WorkRecordWriterV1,
    contract_root: ContractRootIdV1,
    expected_work_revision: u64,
) -> Result<(), WorkSubmissionError> {
    if writer != WorkRecordWriterV1::Work {
        return Err(WorkSubmissionError::ForeignWriter(writer));
    }
    if expected_work_revision == 0 {
        return Err(WorkSubmissionError::InvalidExpectedRevision);
    }
    if *contract_root.as_bytes() == [0_u8; 32] {
        return Err(WorkSubmissionError::EmptyContractRoot);
    }
    Ok(())
}

fn compute_digest(
    id: WorkSubmissionIdV1,
    subject: &WorkSubmissionSubjectV1,
    expected_work_revision: u64,
    claim_set: &SubmissionClaimSetV1,
) -> Result<[u8; 32], WorkSubmissionError> {
    Ok(Sha256::digest(encode_submission(
        id,
        subject,
        expected_work_revision,
        claim_set,
    )?)
    .into())
}

fn encode_submission(
    id: WorkSubmissionIdV1,
    subject: &WorkSubmissionSubjectV1,
    expected_work_revision: u64,
    claim_set: &SubmissionClaimSetV1,
) -> Result<Vec<u8>, WorkSubmissionError> {
    Ok(deterministic_cbor::encode(&submission_value(
        id,
        subject,
        expected_work_revision,
        claim_set,
    )?)?)
}

fn submission_value(
    id: WorkSubmissionIdV1,
    subject: &WorkSubmissionSubjectV1,
    expected_work_revision: u64,
    claim_set: &SubmissionClaimSetV1,
) -> Result<CborValue, WorkSubmissionError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(WORK_SUBMISSION_VERSION_V1),
        CborValue::Bytes(id.as_bytes().to_vec()),
        subject.canonical_value(),
        CborValue::Unsigned(expected_work_revision),
        claim_set.schema_value()?,
    ]))
}
