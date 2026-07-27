use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{WorkIdV1, WorkRecordWriterV1, WorkSubmissionIdV1, WorkSubmissionV1};

pub const WORK_LIFECYCLE_VERSION_V1: u64 = 1;
pub const MAX_WORK_HISTORY_FACTS_V1: usize = 4_096;
pub const MAX_WORK_TRANSITION_REASON_BYTES_V1: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkRevisionV1(u64);

impl WorkRevisionV1 {
    pub fn new(value: u64) -> Result<Self, WorkLifecycleError> {
        if value == 0 {
            return Err(WorkLifecycleError::InvalidRevision);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkTransitionReasonV1(String);

impl WorkTransitionReasonV1 {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkLifecycleError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_WORK_TRANSITION_REASON_BYTES_V1
            || !value.is_ascii()
        {
            return Err(WorkLifecycleError::InvalidReason);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkLifecycleStateV1 {
    Draft,
    Ready,
    Active,
    AwaitingAcceptance,
    Completed,
    Cancelled,
    Superseded { successor: WorkIdV1 },
}

impl WorkLifecycleStateV1 {
    pub const fn tag(&self) -> u64 {
        match self {
            Self::Draft => 1,
            Self::Ready => 2,
            Self::Active => 3,
            Self::AwaitingAcceptance => 4,
            Self::Completed => 5,
            Self::Cancelled => 6,
            Self::Superseded { .. } => 7,
        }
    }

    pub fn from_tag(tag: u64, successor: Option<WorkIdV1>) -> Result<Self, WorkLifecycleError> {
        match (tag, successor) {
            (1, None) => Ok(Self::Draft),
            (2, None) => Ok(Self::Ready),
            (3, None) => Ok(Self::Active),
            (4, None) => Ok(Self::AwaitingAcceptance),
            (5, None) => Ok(Self::Completed),
            (6, None) => Ok(Self::Cancelled),
            (7, Some(successor)) => Ok(Self::Superseded { successor }),
            (1..=6, Some(_)) | (7, None) => Err(WorkLifecycleError::InvalidStatePayload),
            _ => Err(WorkLifecycleError::UnknownStateTag(tag)),
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Superseded { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkTransitionKindV1 {
    CreateDraftWork,
    PublishInitialContract,
    AcquireFirstStepExecution,
    SubmitWorkCompletion,
    CompleteWork,
    RejectWorkCompletion,
    ReturnWorkForRepair,
    AmendContract,
    CancelWork,
    AbsorbWork,
}

impl WorkTransitionKindV1 {
    pub const ALL: [Self; 10] = [
        Self::CreateDraftWork,
        Self::PublishInitialContract,
        Self::AcquireFirstStepExecution,
        Self::SubmitWorkCompletion,
        Self::CompleteWork,
        Self::RejectWorkCompletion,
        Self::ReturnWorkForRepair,
        Self::AmendContract,
        Self::CancelWork,
        Self::AbsorbWork,
    ];

    pub const fn tag(self) -> u64 {
        match self {
            Self::CreateDraftWork => 1,
            Self::PublishInitialContract => 2,
            Self::AcquireFirstStepExecution => 3,
            Self::SubmitWorkCompletion => 4,
            Self::CompleteWork => 5,
            Self::RejectWorkCompletion => 6,
            Self::ReturnWorkForRepair => 7,
            Self::AmendContract => 8,
            Self::CancelWork => 9,
            Self::AbsorbWork => 10,
        }
    }

    pub fn from_tag(tag: u64) -> Result<Self, WorkLifecycleError> {
        match tag {
            1 => Ok(Self::CreateDraftWork),
            2 => Ok(Self::PublishInitialContract),
            3 => Ok(Self::AcquireFirstStepExecution),
            4 => Ok(Self::SubmitWorkCompletion),
            5 => Ok(Self::CompleteWork),
            6 => Ok(Self::RejectWorkCompletion),
            7 => Ok(Self::ReturnWorkForRepair),
            8 => Ok(Self::AmendContract),
            9 => Ok(Self::CancelWork),
            10 => Ok(Self::AbsorbWork),
            _ => Err(WorkLifecycleError::UnknownTransitionTag(tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkTransitionV1 {
    PublishInitialContract,
    AcquireFirstStepExecution,
    SubmitWorkCompletion {
        submission: Box<WorkSubmissionV1>,
    },
    CompleteWork {
        submission_id: WorkSubmissionIdV1,
    },
    RejectWorkCompletion {
        submission_id: WorkSubmissionIdV1,
        reason: WorkTransitionReasonV1,
    },
    ReturnWorkForRepair {
        submission_id: WorkSubmissionIdV1,
        reason: WorkTransitionReasonV1,
    },
    AmendContract {
        invalidated_submission_id: Option<WorkSubmissionIdV1>,
        reason: WorkTransitionReasonV1,
    },
    CancelWork {
        reason: WorkTransitionReasonV1,
    },
}

impl WorkTransitionV1 {
    pub const fn kind(&self) -> WorkTransitionKindV1 {
        match self {
            Self::PublishInitialContract => WorkTransitionKindV1::PublishInitialContract,
            Self::AcquireFirstStepExecution => WorkTransitionKindV1::AcquireFirstStepExecution,
            Self::SubmitWorkCompletion { .. } => WorkTransitionKindV1::SubmitWorkCompletion,
            Self::CompleteWork { .. } => WorkTransitionKindV1::CompleteWork,
            Self::RejectWorkCompletion { .. } => WorkTransitionKindV1::RejectWorkCompletion,
            Self::ReturnWorkForRepair { .. } => WorkTransitionKindV1::ReturnWorkForRepair,
            Self::AmendContract { .. } => WorkTransitionKindV1::AmendContract,
            Self::CancelWork { .. } => WorkTransitionKindV1::CancelWork,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkHistoryFactV1 {
    revision: WorkRevisionV1,
    prior_state: Option<WorkLifecycleStateV1>,
    state: WorkLifecycleStateV1,
    transition: WorkTransitionKindV1,
    submission_id: Option<WorkSubmissionIdV1>,
    reason: Option<WorkTransitionReasonV1>,
}

impl WorkHistoryFactV1 {
    pub fn revision(&self) -> WorkRevisionV1 {
        self.revision
    }

    pub fn prior_state(&self) -> Option<&WorkLifecycleStateV1> {
        self.prior_state.as_ref()
    }

    pub fn state(&self) -> &WorkLifecycleStateV1 {
        &self.state
    }

    pub fn transition(&self) -> WorkTransitionKindV1 {
        self.transition
    }

    pub fn submission_id(&self) -> Option<WorkSubmissionIdV1> {
        self.submission_id
    }

    pub fn reason(&self) -> Option<&WorkTransitionReasonV1> {
        self.reason.as_ref()
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, WorkLifecycleError> {
        Ok(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(WORK_LIFECYCLE_VERSION_V1),
            CborValue::Unsigned(self.revision.get()),
            CborValue::optional(self.prior_state.as_ref().map(encode_state).transpose()?),
            encode_state(&self.state)?,
            CborValue::Unsigned(self.transition.tag()),
            CborValue::optional(
                self.submission_id
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec())),
            ),
            CborValue::optional(
                self.reason
                    .as_ref()
                    .map(|reason| CborValue::text(reason.as_str()))
                    .transpose()?,
            ),
        ]))?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkRecordV1 {
    id: WorkIdV1,
    revision: WorkRevisionV1,
    state: WorkLifecycleStateV1,
    history: Vec<WorkHistoryFactV1>,
    submissions: Vec<WorkSubmissionV1>,
    current_submission_id: Option<WorkSubmissionIdV1>,
}

impl WorkRecordV1 {
    pub fn create_draft(
        writer: WorkRecordWriterV1,
        id: WorkIdV1,
    ) -> Result<Self, WorkLifecycleError> {
        validate_work_writer(writer)?;
        let revision = WorkRevisionV1::new(1)?;
        let state = WorkLifecycleStateV1::Draft;
        Ok(Self {
            id,
            revision,
            state: state.clone(),
            history: vec![WorkHistoryFactV1 {
                revision,
                prior_state: None,
                state,
                transition: WorkTransitionKindV1::CreateDraftWork,
                submission_id: None,
                reason: None,
            }],
            submissions: Vec::new(),
            current_submission_id: None,
        })
    }

    pub fn id(&self) -> WorkIdV1 {
        self.id
    }

    pub fn revision(&self) -> WorkRevisionV1 {
        self.revision
    }

    pub fn state(&self) -> &WorkLifecycleStateV1 {
        &self.state
    }

    pub fn history(&self) -> &[WorkHistoryFactV1] {
        &self.history
    }

    pub fn submissions(&self) -> &[WorkSubmissionV1] {
        &self.submissions
    }

    pub fn current_submission(&self) -> Option<&WorkSubmissionV1> {
        let current = self.current_submission_id?;
        self.submissions.iter().find(|item| item.id() == current)
    }

    pub fn apply(
        &self,
        writer: WorkRecordWriterV1,
        expected_revision: WorkRevisionV1,
        transition: WorkTransitionV1,
    ) -> Result<Self, WorkLifecycleError> {
        self.apply_inner(writer, expected_revision, transition, false)
    }

    pub(crate) fn apply_verified_completion(
        &self,
        writer: WorkRecordWriterV1,
        expected_revision: WorkRevisionV1,
        submission: WorkSubmissionV1,
    ) -> Result<Self, WorkLifecycleError> {
        self.apply_inner(
            writer,
            expected_revision,
            WorkTransitionV1::SubmitWorkCompletion {
                submission: Box::new(submission),
            },
            true,
        )
    }

    fn apply_inner(
        &self,
        writer: WorkRecordWriterV1,
        expected_revision: WorkRevisionV1,
        transition: WorkTransitionV1,
        completion_basis_verified: bool,
    ) -> Result<Self, WorkLifecycleError> {
        validate_work_writer(writer)?;
        if expected_revision != self.revision {
            return Err(WorkLifecycleError::StaleRevision {
                expected: expected_revision.get(),
                actual: self.revision.get(),
            });
        }
        if self.history.len() >= MAX_WORK_HISTORY_FACTS_V1 {
            return Err(WorkLifecycleError::HistoryLimitExceeded);
        }

        let prior_state = self.state.clone();
        let (state, submission, submission_id, reason) =
            self.evaluate(&transition, completion_basis_verified)?;
        let next_revision = WorkRevisionV1::new(
            self.revision
                .get()
                .checked_add(1)
                .ok_or(WorkLifecycleError::RevisionOverflow)?,
        )?;

        let mut next = self.clone();
        next.revision = next_revision;
        next.state = state.clone();
        if let Some(submission) = submission {
            if next
                .submissions
                .iter()
                .any(|existing| existing.id() == submission.id())
            {
                return Err(WorkLifecycleError::DuplicateSubmissionId);
            }
            next.current_submission_id = Some(submission.id());
            next.submissions.push(submission);
        } else if matches!(
            transition,
            WorkTransitionV1::RejectWorkCompletion { .. }
                | WorkTransitionV1::ReturnWorkForRepair { .. }
                | WorkTransitionV1::AmendContract { .. }
                | WorkTransitionV1::CancelWork { .. }
        ) {
            next.current_submission_id = None;
        }
        next.history.push(WorkHistoryFactV1 {
            revision: next_revision,
            prior_state: Some(prior_state),
            state,
            transition: transition.kind(),
            submission_id,
            reason,
        });
        Ok(next)
    }

    fn evaluate(
        &self,
        transition: &WorkTransitionV1,
        completion_basis_verified: bool,
    ) -> Result<TransitionEvaluation, WorkLifecycleError> {
        let result = match (&self.state, transition) {
            (WorkLifecycleStateV1::Draft, WorkTransitionV1::PublishInitialContract) => {
                (WorkLifecycleStateV1::Ready, None, None, None)
            }
            (WorkLifecycleStateV1::Ready, WorkTransitionV1::AcquireFirstStepExecution) => {
                (WorkLifecycleStateV1::Active, None, None, None)
            }
            (
                WorkLifecycleStateV1::Active,
                WorkTransitionV1::SubmitWorkCompletion { submission },
            ) => {
                if !completion_basis_verified {
                    return Err(WorkLifecycleError::UnverifiedCompletionBasis);
                }
                if submission.work_id() != self.id {
                    return Err(WorkLifecycleError::SubmissionWorkMismatch);
                }
                if submission.expected_work_revision() != self.revision.get() {
                    return Err(WorkLifecycleError::SubmissionRevisionMismatch);
                }
                (
                    WorkLifecycleStateV1::AwaitingAcceptance,
                    Some(submission.as_ref().clone()),
                    Some(submission.id()),
                    None,
                )
            }
            (
                WorkLifecycleStateV1::AwaitingAcceptance,
                WorkTransitionV1::CompleteWork { submission_id },
            ) => {
                self.validate_current_submission(*submission_id)?;
                (
                    WorkLifecycleStateV1::Completed,
                    None,
                    Some(*submission_id),
                    None,
                )
            }
            (
                WorkLifecycleStateV1::AwaitingAcceptance,
                WorkTransitionV1::RejectWorkCompletion {
                    submission_id,
                    reason,
                }
                | WorkTransitionV1::ReturnWorkForRepair {
                    submission_id,
                    reason,
                },
            ) => {
                self.validate_current_submission(*submission_id)?;
                (
                    WorkLifecycleStateV1::Active,
                    None,
                    Some(*submission_id),
                    Some(reason.clone()),
                )
            }
            (
                WorkLifecycleStateV1::Ready,
                WorkTransitionV1::AmendContract {
                    invalidated_submission_id: None,
                    reason,
                },
            ) => (
                WorkLifecycleStateV1::Ready,
                None,
                None,
                Some(reason.clone()),
            ),
            (
                WorkLifecycleStateV1::Active,
                WorkTransitionV1::AmendContract {
                    invalidated_submission_id: None,
                    reason,
                },
            ) => (
                WorkLifecycleStateV1::Active,
                None,
                None,
                Some(reason.clone()),
            ),
            (
                WorkLifecycleStateV1::AwaitingAcceptance,
                WorkTransitionV1::AmendContract {
                    invalidated_submission_id: Some(submission_id),
                    reason,
                },
            ) => {
                self.validate_current_submission(*submission_id)?;
                (
                    WorkLifecycleStateV1::Active,
                    None,
                    Some(*submission_id),
                    Some(reason.clone()),
                )
            }
            (
                WorkLifecycleStateV1::Draft
                | WorkLifecycleStateV1::Ready
                | WorkLifecycleStateV1::Active
                | WorkLifecycleStateV1::AwaitingAcceptance,
                WorkTransitionV1::CancelWork { reason },
            ) => (
                WorkLifecycleStateV1::Cancelled,
                None,
                self.current_submission_id,
                Some(reason.clone()),
            ),
            _ => {
                return Err(WorkLifecycleError::IllegalTransition {
                    state_tag: self.state.tag(),
                    transition_tag: transition.kind().tag(),
                });
            }
        };
        Ok(result)
    }

    fn validate_current_submission(
        &self,
        submission_id: WorkSubmissionIdV1,
    ) -> Result<(), WorkLifecycleError> {
        if self.current_submission_id != Some(submission_id) {
            return Err(WorkLifecycleError::SubmissionIsNotCurrent);
        }
        Ok(())
    }
}

type TransitionEvaluation = (
    WorkLifecycleStateV1,
    Option<WorkSubmissionV1>,
    Option<WorkSubmissionIdV1>,
    Option<WorkTransitionReasonV1>,
);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkLifecycleError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("unknown Work lifecycle state tag {0}")]
    UnknownStateTag(u64),
    #[error("Work lifecycle state tag has an invalid payload")]
    InvalidStatePayload,
    #[error("unknown Work transition tag {0}")]
    UnknownTransitionTag(u64),
    #[error("Work revision must be positive")]
    InvalidRevision,
    #[error("Work revision overflow")]
    RevisionOverflow,
    #[error("Work transition reason must contain 1..=1024 ASCII bytes")]
    InvalidReason,
    #[error("{0:?} cannot mutate Work-owned lifecycle or history")]
    ForeignWriter(WorkRecordWriterV1),
    #[error("stale Work revision: expected {expected}, actual {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("Work history exceeds its finite v1 bound")]
    HistoryLimitExceeded,
    #[error("illegal Work lifecycle transition {transition_tag} from state {state_tag}")]
    IllegalTransition { state_tag: u64, transition_tag: u64 },
    #[error("Work Submission belongs to another Work")]
    SubmissionWorkMismatch,
    #[error("Work Submission does not bind the exact expected Work revision")]
    SubmissionRevisionMismatch,
    #[error(
        "Work completion requires the Repository facade's exact current Contract and Step basis"
    )]
    UnverifiedCompletionBasis,
    #[error("Work Submission is not the current awaiting-acceptance Submission")]
    SubmissionIsNotCurrent,
    #[error("Work Submission identity already exists in immutable history")]
    DuplicateSubmissionId,
    #[error("Work cannot supersede itself")]
    SelfSupersession,
}

fn validate_work_writer(writer: WorkRecordWriterV1) -> Result<(), WorkLifecycleError> {
    if writer != WorkRecordWriterV1::Work {
        return Err(WorkLifecycleError::ForeignWriter(writer));
    }
    Ok(())
}

fn encode_state(state: &WorkLifecycleStateV1) -> Result<CborValue, WorkLifecycleError> {
    let successor = match state {
        WorkLifecycleStateV1::Superseded { successor } => {
            Some(CborValue::Bytes(successor.as_bytes().to_vec()))
        }
        _ => None,
    };
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(state.tag()),
        CborValue::optional(successor),
    ]))
}
