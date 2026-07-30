use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::authority::{ActionRequestIdV1, IdempotencyKeyIdV1, RepositoryActionLeafV1};
use crate::domain::contract::runtime::ContractGenerationIdV1;
use crate::domain::evidence::SubmissionClaimSetV1;
use crate::domain::evidence::{ClaimSubjectV1, SubmissionRefV1};
use crate::domain::execution::{ExecutionRuntimeErrorV1, StepSubmissionExecutionFenceV1};
use crate::domain::identity::{ContractRootIdV1, StoreDomainIdV1};
use crate::domain::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{StepBindingV1, StepIdV1, StepRevisionIdV1, StepScopeV1, StepSubmissionIdV1};

const STEP_SUBMISSION_RECORD_DOMAIN_V1: &str = "maestro.vnext.step-submission-record.v1";
const STEP_SUBMISSION_RECORD_HASH_DOMAIN_V1: &str = "maestro.vnext.step-submission-record-hash.v1";
const STEP_SUBMIT_ACTION_REQUEST_DOMAIN_V1: &str = "maestro.vnext.step-submit-action-request.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepSubmissionV1 {
    id: StepSubmissionIdV1,
    binding: StepBindingV1,
    execution_fence: StepSubmissionExecutionFenceV1,
    execution_carrier_commitment: [u8; 32],
    claim_set_digest: [u8; 32],
    record_hash: [u8; 32],
}

impl StepSubmissionV1 {
    pub fn new(
        id: StepSubmissionIdV1,
        binding: StepBindingV1,
        execution_fence: StepSubmissionExecutionFenceV1,
        execution_carrier_commitment: [u8; 32],
        claim_set: &SubmissionClaimSetV1,
    ) -> Result<Self, StepSubmissionErrorV1> {
        if execution_carrier_commitment == [0; 32]
            || execution_fence.binding_commitment() != hash(&step_binding_value(binding))?
        {
            return Err(StepSubmissionErrorV1::ExecutionFenceMismatch);
        }
        if !claim_set.is_authoritative()
            || claim_set.submission_ref() != Some(SubmissionRefV1::Step(id))
            || claim_set.digest() == &[0; 32]
            || claim_set.claim_subjects().is_none_or(|subjects| {
                subjects.iter().any(|subject| {
                    !matches!(
                    subject,
                    ClaimSubjectV1::Step {
                        binding: claim_binding,
                        lease_fence,
                    } if *claim_binding == binding && *lease_fence == execution_fence.fence()
                    )
                })
            })
        {
            return Err(StepSubmissionErrorV1::ClaimBindingMismatch);
        }
        Self::from_parts(
            id,
            binding,
            execution_fence,
            execution_carrier_commitment,
            *claim_set.digest(),
        )
    }

    fn from_parts(
        id: StepSubmissionIdV1,
        binding: StepBindingV1,
        execution_fence: StepSubmissionExecutionFenceV1,
        execution_carrier_commitment: [u8; 32],
        claim_set_digest: [u8; 32],
    ) -> Result<Self, StepSubmissionErrorV1> {
        if execution_carrier_commitment == [0; 32]
            || claim_set_digest == [0; 32]
            || execution_fence.binding_commitment() != hash(&step_binding_value(binding))?
        {
            return Err(StepSubmissionErrorV1::ExecutionFenceMismatch);
        }
        let mut submission = Self {
            id,
            binding,
            execution_fence,
            execution_carrier_commitment,
            claim_set_digest,
            record_hash: [0; 32],
        };
        submission.record_hash = hash(&CborValue::Array(vec![
            CborValue::text(STEP_SUBMISSION_RECORD_HASH_DOMAIN_V1)?,
            submission.canonical_value()?,
        ]))?;
        Ok(submission)
    }

    pub const fn id(&self) -> StepSubmissionIdV1 {
        self.id
    }

    pub const fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub const fn execution_fence(&self) -> StepSubmissionExecutionFenceV1 {
        self.execution_fence
    }

    pub const fn execution_carrier_commitment(&self) -> [u8; 32] {
        self.execution_carrier_commitment
    }

    pub const fn claim_set_digest(&self) -> [u8; 32] {
        self.claim_set_digest
    }

    pub const fn record_hash(&self) -> [u8; 32] {
        self.record_hash
    }

    pub fn canonical_value(&self) -> Result<CborValue, StepSubmissionErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text(STEP_SUBMISSION_RECORD_DOMAIN_V1)?,
            CborValue::Unsigned(1),
            bytes(self.id.as_bytes()),
            step_binding_value(self.binding),
            self.execution_fence.canonical_value(),
            bytes(&self.execution_carrier_commitment),
            bytes(&self.claim_set_digest),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StepSubmissionErrorV1> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, StepSubmissionErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
        };
        let [
            CborValue::Text(domain),
            CborValue::Unsigned(1),
            id,
            binding,
            execution_fence,
            execution_carrier_commitment,
            claim_set_digest,
        ] = fields.as_slice()
        else {
            return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
        };
        if domain != STEP_SUBMISSION_RECORD_DOMAIN_V1 {
            return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
        }
        let id = StepSubmissionIdV1::from_bytes(exact_digest(id)?)
            .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
        let binding = parse_step_binding(binding)?;
        let execution_fence =
            StepSubmissionExecutionFenceV1::from_canonical_value(execution_fence)?;
        let execution_carrier_commitment = exact_digest(execution_carrier_commitment)?;
        let rebuilt = Self::from_parts(
            id,
            binding,
            execution_fence,
            execution_carrier_commitment,
            exact_digest(claim_set_digest)?,
        )?;
        if rebuilt.canonical_value()? != *value {
            return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
        }
        Ok(rebuilt)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStepSubmissionActionRequestV1 {
    request_id: ActionRequestIdV1,
    subject_commitment: [u8; 32],
    expected_state_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    idempotency_key_id: IdempotencyKeyIdV1,
}

impl CanonicalStepSubmissionActionRequestV1 {
    pub fn from_values(
        subject: &CborValue,
        expected_state: &CborValue,
        payload: &CborValue,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, StepSubmissionErrorV1> {
        Self::new(
            hash(subject)?,
            hash(expected_state)?,
            hash(payload)?,
            idempotency_key_id,
        )
    }

    fn new(
        subject_commitment: [u8; 32],
        expected_state_commitment: [u8; 32],
        payload_commitment: [u8; 32],
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, StepSubmissionErrorV1> {
        if [
            subject_commitment,
            expected_state_commitment,
            payload_commitment,
        ]
        .contains(&[0; 32])
        {
            return Err(StepSubmissionErrorV1::MissingCommitment);
        }
        let value = step_submit_action_request_value(
            subject_commitment,
            expected_state_commitment,
            payload_commitment,
            idempotency_key_id,
        )?;
        Ok(Self {
            request_id: ActionRequestIdV1::from_digest(hash(&value)?),
            subject_commitment,
            expected_state_commitment,
            payload_commitment,
            idempotency_key_id,
        })
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn expected_state_commitment(&self) -> [u8; 32] {
        self.expected_state_commitment
    }

    pub const fn payload_commitment(&self) -> [u8; 32] {
        self.payload_commitment
    }

    pub const fn idempotency_key_id(&self) -> IdempotencyKeyIdV1 {
        self.idempotency_key_id
    }

    pub fn canonical_value(&self) -> Result<CborValue, StepSubmissionErrorV1> {
        step_submit_action_request_value(
            self.subject_commitment,
            self.expected_state_commitment,
            self.payload_commitment,
            self.idempotency_key_id,
        )
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, StepSubmissionErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(StepSubmissionErrorV1::InvalidStoredActionRequest);
        };
        let [
            CborValue::Text(domain),
            CborValue::Unsigned(global_tag),
            CborValue::Unsigned(owner_tag),
            CborValue::Unsigned(local_tag),
            CborValue::Text(literal),
            CborValue::Text(owner_descriptor_id),
            subject,
            expected_state,
            payload,
            idempotency_key,
        ] = fields.as_slice()
        else {
            return Err(StepSubmissionErrorV1::InvalidStoredActionRequest);
        };
        let action = RepositoryActionLeafV1::SubmitStep;
        if domain != STEP_SUBMIT_ACTION_REQUEST_DOMAIN_V1
            || *global_tag != action.global_tag()
            || *owner_tag != action.owner_tag()
            || *local_tag != action.local_tag()
            || literal != action.literal()
            || owner_descriptor_id != action.owner_descriptor_id()
        {
            return Err(StepSubmissionErrorV1::InvalidStoredActionRequest);
        }
        let rebuilt = Self::new(
            exact_digest(subject)?,
            exact_digest(expected_state)?,
            exact_digest(payload)?,
            IdempotencyKeyIdV1::from_digest(exact_digest(idempotency_key)?),
        )?;
        if rebuilt.canonical_value()? != *value {
            return Err(StepSubmissionErrorV1::InvalidStoredActionRequest);
        }
        Ok(rebuilt)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepSubmissionErrorV1 {
    #[error("Step Submission execution fence does not bind the exact Step execution carrier")]
    ExecutionFenceMismatch,
    #[error("Step Submission Claim belongs to another Submission, Step Binding, or Lease fence")]
    ClaimBindingMismatch,
    #[error("Step Submission Action Request contains a missing commitment")]
    MissingCommitment,
    #[error("stored Step Submission is malformed or non-canonical")]
    InvalidStoredSubmission,
    #[error("stored SubmitStep Action Request is malformed or non-canonical")]
    InvalidStoredActionRequest,
    #[error(transparent)]
    Execution(#[from] ExecutionRuntimeErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

fn step_submit_action_request_value(
    subject_commitment: [u8; 32],
    expected_state_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    idempotency_key_id: IdempotencyKeyIdV1,
) -> Result<CborValue, StepSubmissionErrorV1> {
    let action = RepositoryActionLeafV1::SubmitStep;
    Ok(CborValue::Array(vec![
        CborValue::text(STEP_SUBMIT_ACTION_REQUEST_DOMAIN_V1)?,
        CborValue::Unsigned(action.global_tag()),
        CborValue::Unsigned(action.owner_tag()),
        CborValue::Unsigned(action.local_tag()),
        CborValue::Text(action.literal().to_owned()),
        CborValue::Text(action.owner_descriptor_id().to_owned()),
        bytes(&subject_commitment),
        bytes(&expected_state_commitment),
        bytes(&payload_commitment),
        bytes(idempotency_key_id.as_bytes()),
    ]))
}

fn step_binding_value(binding: StepBindingV1) -> CborValue {
    CborValue::Array(vec![
        bytes(binding.scope().repository_id().as_bytes()),
        bytes(binding.scope().work_id().as_bytes()),
        bytes(binding.contract_generation_id().as_bytes()),
        bytes(binding.contract_root_id().as_bytes()),
        bytes(binding.step_id().as_bytes()),
        bytes(binding.revision_id().as_bytes()),
    ])
}

fn parse_step_binding(value: &CborValue) -> Result<StepBindingV1, StepSubmissionErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
    };
    let [repository, work, generation, root, step, revision] = fields.as_slice() else {
        return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
    };
    let repository = StoreDomainIdV1::parse(&render_digest(exact_digest(repository)?))
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
    let work = WorkIdV1::parse(&render_digest(exact_digest(work)?))
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
    let scope = StepScopeV1::new(repository, work);
    let generation = ContractGenerationIdV1::parse(&render_digest(exact_digest(generation)?))
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
    let root = ContractRootIdV1::parse(&render_digest(exact_digest(root)?))
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
    let step = StepIdV1::from_bytes(scope, exact_digest(step)?)
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
    let revision = StepRevisionIdV1::from_bytes(exact_digest(revision)?)
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)?;
    StepBindingV1::new(scope, generation, root, step, revision)
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], StepSubmissionErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(StepSubmissionErrorV1::InvalidStoredSubmission);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| StepSubmissionErrorV1::InvalidStoredSubmission)
}

fn render_digest(bytes: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}
