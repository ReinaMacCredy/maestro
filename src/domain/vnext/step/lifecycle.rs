use thiserror::Error;

use super::graph::StepBindingV1;
use super::identity::{StepIdentityError, StepSubmissionIdV1, require_nonzero};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepLifecycleKindV1 {
    Open,
    Submitted,
    Satisfied,
    Cancelled,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOpenBasisV1 {
    Fresh,
    RejectedSubmission {
        submission_id: StepSubmissionIdV1,
        submission_record_hash: [u8; 32],
        rejection_receipt_hash: [u8; 32],
    },
    RecoveredSubmission {
        submission_id: StepSubmissionIdV1,
        submission_record_hash: [u8; 32],
        recovery_receipt_hash: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepLifecycleV1 {
    Open {
        basis: StepOpenBasisV1,
    },
    Submitted {
        submission_id: StepSubmissionIdV1,
        submission_record_hash: [u8; 32],
    },
    Satisfied {
        submission_record_hash: [u8; 32],
        satisfaction_basis_hash: [u8; 32],
    },
    Cancelled {
        amendment_receipt_hash: [u8; 32],
    },
    Superseded {
        successor: StepBindingV1,
        amendment_receipt_hash: [u8; 32],
    },
}

impl StepLifecycleV1 {
    pub fn kind(&self) -> StepLifecycleKindV1 {
        match self {
            Self::Open { .. } => StepLifecycleKindV1::Open,
            Self::Submitted { .. } => StepLifecycleKindV1::Submitted,
            Self::Satisfied { .. } => StepLifecycleKindV1::Satisfied,
            Self::Cancelled { .. } => StepLifecycleKindV1::Cancelled,
            Self::Superseded { .. } => StepLifecycleKindV1::Superseded,
        }
    }

    pub fn satisfaction_basis_hash(&self) -> Option<&[u8; 32]> {
        match self {
            Self::Satisfied {
                satisfaction_basis_hash,
                ..
            } => Some(satisfaction_basis_hash),
            _ => None,
        }
    }

    pub fn submission_record_hash(&self) -> Option<&[u8; 32]> {
        match self {
            Self::Submitted {
                submission_record_hash,
                ..
            }
            | Self::Satisfied {
                submission_record_hash,
                ..
            } => Some(submission_record_hash),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepStateV1 {
    binding: StepBindingV1,
    lifecycle: StepLifecycleV1,
}

impl StepStateV1 {
    pub fn new_open(binding: StepBindingV1) -> Self {
        Self {
            binding,
            lifecycle: StepLifecycleV1::Open {
                basis: StepOpenBasisV1::Fresh,
            },
        }
    }

    pub fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub fn lifecycle(&self) -> StepLifecycleV1 {
        self.lifecycle
    }

    pub fn cancel(&self, amendment_receipt_hash: [u8; 32]) -> Result<Self, StepLifecycleError> {
        require_nonzero(
            amendment_receipt_hash,
            "Step cancellation amendment receipt",
        )?;
        if !matches!(
            self.lifecycle,
            StepLifecycleV1::Open { .. } | StepLifecycleV1::Submitted { .. }
        ) {
            return Err(StepLifecycleError::CancelRequiresOpenOrSubmitted);
        }
        Ok(Self {
            binding: self.binding,
            lifecycle: StepLifecycleV1::Cancelled {
                amendment_receipt_hash,
            },
        })
    }

    pub fn supersede(
        &self,
        successor: StepBindingV1,
        amendment_receipt_hash: [u8; 32],
    ) -> Result<Self, StepLifecycleError> {
        require_nonzero(
            amendment_receipt_hash,
            "Step supersession amendment receipt",
        )?;
        if !matches!(
            self.lifecycle,
            StepLifecycleV1::Open { .. } | StepLifecycleV1::Submitted { .. }
        ) {
            return Err(StepLifecycleError::SupersedeRequiresOpenOrSubmitted);
        }
        if successor == self.binding
            || successor.scope() != self.binding.scope()
            || successor.step_id() != self.binding.step_id()
        {
            return Err(StepLifecycleError::InvalidSuccessor);
        }
        Ok(Self {
            binding: self.binding,
            lifecycle: StepLifecycleV1::Superseded {
                successor,
                amendment_receipt_hash,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StepLifecycleError {
    #[error(transparent)]
    Identity(#[from] StepIdentityError),
    #[error("Step Submission binds a different generation-scoped Step")]
    SubmissionBindingMismatch,
    #[error("SubmitStep refuses reuse of the immediately rejected or recovered Submission")]
    ReusedSubmission,
    #[error("SubmitStep requires Step lifecycle open")]
    SubmitRequiresOpen,
    #[error("SatisfyStep requires Step lifecycle submitted")]
    SatisfyRequiresSubmitted,
    #[error("RejectStepSubmission requires Step lifecycle submitted")]
    RejectRequiresSubmitted,
    #[error("RecoverStepSubmission requires Step lifecycle submitted")]
    RecoverRequiresSubmitted,
    #[error("Step cancellation requires lifecycle open or submitted")]
    CancelRequiresOpenOrSubmitted,
    #[error("Step supersession requires lifecycle open or submitted")]
    SupersedeRequiresOpenOrSubmitted,
    #[error("Step supersession successor must be a distinct binding of the same stable Step")]
    InvalidSuccessor,
}
