use std::fmt;
use std::marker::PhantomData;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::authority::{
    ActionRequestIdV1, AuthorizationReceiptV1, IdempotencyKeyIdV1, PrincipalIdV1,
};
use crate::domain::vnext::contract::runtime::ContractGenerationIdV1;
use crate::domain::vnext::identity::{ContractRootIdV1, StoreDomainIdV1, StoreGenerationIdV1};
use crate::domain::vnext::step::{StepBindingV1, StepIdV1, StepRevisionIdV1, StepScopeV1};
use crate::domain::vnext::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const EXECUTION_ID_SEED_LIMIT_V1: usize = 512;
const MAX_RUNS_PER_ATTEMPT_V1: usize = 4_096;
const MAX_RUN_SEGMENTS_V1: usize = 16_384;

mod private {
    pub trait Sealed {}
}

pub trait ExecutionIdentityKindV1: private::Sealed {
    const DOMAIN: &'static str;
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionIdV1<K: ExecutionIdentityKindV1> {
    bytes: [u8; 32],
    marker: PhantomData<K>,
}

impl<K: ExecutionIdentityKindV1> ExecutionIdV1<K> {
    pub fn derive(seed: &str) -> Result<Self, ExecutionRuntimeErrorV1> {
        if seed.is_empty() || seed.len() > EXECUTION_ID_SEED_LIMIT_V1 || !seed.is_ascii() {
            return Err(ExecutionRuntimeErrorV1::InvalidIdentitySeed);
        }
        Self::from_value(&CborValue::Text(seed.to_owned()))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ExecutionRuntimeErrorV1> {
        if bytes == [0; 32] {
            return Err(ExecutionRuntimeErrorV1::MissingCommitment);
        }
        Ok(Self {
            bytes,
            marker: PhantomData,
        })
    }

    fn from_value(value: &CborValue) -> Result<Self, ExecutionRuntimeErrorV1> {
        Self::from_bytes(hash(&CborValue::Array(vec![
            CborValue::text(K::DOMAIN)?,
            value.clone(),
        ]))?)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn render(&self) -> String {
        render_digest(self.bytes)
    }
}

impl<K: ExecutionIdentityKindV1> fmt::Debug for ExecutionIdV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple(std::any::type_name::<K>())
            .field(&self.render())
            .finish()
    }
}

macro_rules! execution_identity {
    ($kind:ident, $alias:ident, $domain:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $kind {}

        impl private::Sealed for $kind {}

        impl ExecutionIdentityKindV1 for $kind {
            const DOMAIN: &'static str = $domain;
        }

        pub type $alias = ExecutionIdV1<$kind>;
    };
}

execution_identity!(
    StepLeaseIdentityKindV1,
    StepLeaseIdV1,
    "maestro.vnext.step-lease.v1"
);
execution_identity!(
    StepAttemptIdentityKindV1,
    StepAttemptIdV1,
    "maestro.vnext.step-attempt.v1"
);
execution_identity!(
    LeaseTermIdentityKindV1,
    LeaseTermIdV1,
    "maestro.vnext.lease-term.v1"
);
execution_identity!(
    DispatchAttemptIdentityKindV1,
    DispatchAttemptIdV1,
    "maestro.vnext.dispatch-attempt.v1"
);
execution_identity!(
    ReconciliationAttemptIdentityKindV1,
    ReconciliationAttemptIdV1,
    "maestro.vnext.reconciliation-attempt.v1"
);
execution_identity!(RunIdentityKindV1, RunIdV1, "maestro.vnext.run.v1");
execution_identity!(
    EffectIntentIdentityKindV1,
    EffectIntentIdV1,
    "maestro.vnext.effect-intent.v1"
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionActionV1 {
    AcquireStepExecution,
    RenewStepLeaseTerm,
    AbandonStepAttempt,
    OriginateEffectIntent,
    OriginateCoordinationDelivery,
    RecordDispatchOutcome,
    ReconcileEffectIntent,
    ReserveBootstrapMandateInteractionEffect,
    PublishBootstrapMandateInteractionOutcome,
    ReconcileBootstrapMandateInteractionEffect,
    ReserveContinuityMaintenanceEffect,
    PublishContinuityMaintenanceEffectOutcome,
    ReconcileContinuityMaintenanceEffect,
    WithdrawEffectIntent,
    WithdrawBootstrapMandateInteractionEffect,
    WithdrawContinuityMaintenanceEffect,
}

impl ExecutionActionV1 {
    pub const ALL: [Self; 16] = [
        Self::AcquireStepExecution,
        Self::RenewStepLeaseTerm,
        Self::AbandonStepAttempt,
        Self::OriginateEffectIntent,
        Self::OriginateCoordinationDelivery,
        Self::RecordDispatchOutcome,
        Self::ReconcileEffectIntent,
        Self::ReserveBootstrapMandateInteractionEffect,
        Self::PublishBootstrapMandateInteractionOutcome,
        Self::ReconcileBootstrapMandateInteractionEffect,
        Self::ReserveContinuityMaintenanceEffect,
        Self::PublishContinuityMaintenanceEffectOutcome,
        Self::ReconcileContinuityMaintenanceEffect,
        Self::WithdrawEffectIntent,
        Self::WithdrawBootstrapMandateInteractionEffect,
        Self::WithdrawContinuityMaintenanceEffect,
    ];

    pub const fn global_tag(self) -> u64 {
        match self {
            Self::AcquireStepExecution => 23,
            Self::RenewStepLeaseTerm => 24,
            Self::AbandonStepAttempt => 25,
            Self::OriginateEffectIntent => 26,
            Self::OriginateCoordinationDelivery => 27,
            Self::RecordDispatchOutcome => 28,
            Self::ReconcileEffectIntent => 29,
            Self::ReserveBootstrapMandateInteractionEffect => 30,
            Self::PublishBootstrapMandateInteractionOutcome => 31,
            Self::ReconcileBootstrapMandateInteractionEffect => 32,
            Self::ReserveContinuityMaintenanceEffect => 33,
            Self::PublishContinuityMaintenanceEffectOutcome => 34,
            Self::ReconcileContinuityMaintenanceEffect => 35,
            Self::WithdrawEffectIntent => 36,
            Self::WithdrawBootstrapMandateInteractionEffect => 37,
            Self::WithdrawContinuityMaintenanceEffect => 38,
        }
    }

    pub const fn local_tag(self) -> u64 {
        self.global_tag() - 22
    }

    pub const fn literal(self) -> &'static str {
        match self {
            Self::AcquireStepExecution => "AcquireStepExecution",
            Self::RenewStepLeaseTerm => "RenewStepLeaseTerm",
            Self::AbandonStepAttempt => "AbandonStepAttempt",
            Self::OriginateEffectIntent => "OriginateEffectIntent",
            Self::OriginateCoordinationDelivery => "OriginateCoordinationDelivery",
            Self::RecordDispatchOutcome => "RecordDispatchOutcome",
            Self::ReconcileEffectIntent => "ReconcileEffectIntent",
            Self::ReserveBootstrapMandateInteractionEffect => {
                "ReserveBootstrapMandateInteractionEffect"
            }
            Self::PublishBootstrapMandateInteractionOutcome => {
                "PublishBootstrapMandateInteractionOutcome"
            }
            Self::ReconcileBootstrapMandateInteractionEffect => {
                "ReconcileBootstrapMandateInteractionEffect"
            }
            Self::ReserveContinuityMaintenanceEffect => "ReserveContinuityMaintenanceEffect",
            Self::PublishContinuityMaintenanceEffectOutcome => {
                "PublishContinuityMaintenanceEffectOutcome"
            }
            Self::ReconcileContinuityMaintenanceEffect => "ReconcileContinuityMaintenanceEffect",
            Self::WithdrawEffectIntent => "WithdrawEffectIntent",
            Self::WithdrawBootstrapMandateInteractionEffect => {
                "WithdrawBootstrapMandateInteractionEffect"
            }
            Self::WithdrawContinuityMaintenanceEffect => "WithdrawContinuityMaintenanceEffect",
        }
    }

    pub const fn descriptor_id(self) -> &'static str {
        match self {
            Self::AcquireStepExecution => {
                "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2"
            }
            Self::RenewStepLeaseTerm => {
                "24abfd7630a5d743f5793319ade1ad3b6017a1a31cca632be4ba2a68fb4edf0b"
            }
            Self::AbandonStepAttempt => {
                "4ef55b490996ea62eedd4ef62a58db9f17a62d2b040ea4243a4b331eb04953da"
            }
            Self::OriginateEffectIntent => {
                "a2cf705f5d7ba987ae47efd9a8f9a8033e794b9858aa33f3812e4433c1350e26"
            }
            Self::OriginateCoordinationDelivery => {
                "f4d4592e5de4084bcbb1b28487919011aae9a7f3f5f60e2bb3751900d3c26700"
            }
            Self::RecordDispatchOutcome => {
                "568be4ebcfcf121a7d0c7b6aa956dbd281bd17a20c414bf07656923d30cc69d3"
            }
            Self::ReconcileEffectIntent => {
                "5d0b53b85e408badf53e310ca7c619ae0c3a0e3113be26d94dfafe5bf6d2a745"
            }
            Self::ReserveBootstrapMandateInteractionEffect => {
                "b4af8370f69b1aa4c8d93964b34e5e952c4a1e2a764d5f944680527c0430d782"
            }
            Self::PublishBootstrapMandateInteractionOutcome => {
                "946ecf3e7c06a8fd5104f23776a8e19cfd3ad6b3325a76abf949c4a08e0ab0d0"
            }
            Self::ReconcileBootstrapMandateInteractionEffect => {
                "7e885d84c662dbbf928c74d8607975719c4e2851ca475f26855fb4e91ea15d36"
            }
            Self::ReserveContinuityMaintenanceEffect => {
                "4d67ac86d16c81fc135effaa27662f74b373054aee804e9b3b6ae8ba26323bb2"
            }
            Self::PublishContinuityMaintenanceEffectOutcome => {
                "e9197d302312aaf18f576aed38358c5e39854e12a297dac37fcab3e1f53c8460"
            }
            Self::ReconcileContinuityMaintenanceEffect => {
                "2dc583f57f23f12026cef748bb61c0db72150d3ed31f4e6e31be77f8f63e1fa1"
            }
            Self::WithdrawEffectIntent => {
                "6df3b90a4963ffef04865a9e70b57f2040b2b2159b35cfb58347866ed6afe2f9"
            }
            Self::WithdrawBootstrapMandateInteractionEffect => {
                "00ae32e979c74e12f6ebc2f31da11890043495cb8042708edd3f5063c72f2a29"
            }
            Self::WithdrawContinuityMaintenanceEffect => {
                "d5ed8273857101d805748d83023ad067c909427223ae81ba4f9a77f770227d47"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExecutionActionRequestV1 {
    action: ExecutionActionV1,
    request_id: ActionRequestIdV1,
    subject_commitment: [u8; 32],
    expected_state_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    idempotency_key_id: IdempotencyKeyIdV1,
}

impl CanonicalExecutionActionRequestV1 {
    pub(crate) fn new(
        action: ExecutionActionV1,
        subject_commitment: [u8; 32],
        expected_state_commitment: [u8; 32],
        payload_commitment: [u8; 32],
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        require_nonzero(subject_commitment)?;
        require_nonzero(expected_state_commitment)?;
        require_nonzero(payload_commitment)?;
        let value = execution_action_request_value(
            action,
            subject_commitment,
            expected_state_commitment,
            payload_commitment,
            idempotency_key_id,
        )?;
        Ok(Self {
            action,
            request_id: ActionRequestIdV1::from_digest(hash(&value)?),
            subject_commitment,
            expected_state_commitment,
            payload_commitment,
            idempotency_key_id,
        })
    }

    pub fn from_values(
        action: ExecutionActionV1,
        subject: &CborValue,
        expected_state: &CborValue,
        payload: &CborValue,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        Self::new(
            action,
            hash(subject)?,
            hash(expected_state)?,
            hash(payload)?,
            idempotency_key_id,
        )
    }

    pub const fn action(&self) -> ExecutionActionV1 {
        self.action
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

    pub fn canonical_value(&self) -> Result<CborValue, ExecutionRuntimeErrorV1> {
        Ok(execution_action_request_value(
            self.action,
            self.subject_commitment,
            self.expected_state_commitment,
            self.payload_commitment,
            self.idempotency_key_id,
        )?)
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, ExecutionRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let [
            CborValue::Text(domain),
            CborValue::Unsigned(global_tag),
            CborValue::Unsigned(local_tag),
            CborValue::Text(literal),
            CborValue::Text(descriptor_id),
            subject,
            expected_state,
            payload,
            idempotency_key,
        ] = fields.as_slice()
        else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let action = ExecutionActionV1::ALL
            .into_iter()
            .find(|action| action.global_tag() == *global_tag)
            .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
        if domain != "maestro.vnext.execution-action-request.v1"
            || action.local_tag() != *local_tag
            || action.literal() != literal
            || action.descriptor_id() != descriptor_id
        {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let request = Self::new(
            action,
            exact_runtime_digest(subject)?,
            exact_runtime_digest(expected_state)?,
            exact_runtime_digest(payload)?,
            IdempotencyKeyIdV1::from_digest(exact_runtime_digest(idempotency_key)?),
        )?;
        if request.canonical_value()? != *value {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        Ok(request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedExecutionActionV1 {
    request: CanonicalExecutionActionRequestV1,
    receipt: AuthorizationReceiptV1,
}

impl AuthorizedExecutionActionV1 {
    pub(crate) fn new(
        request: CanonicalExecutionActionRequestV1,
        receipt: AuthorizationReceiptV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        if receipt.request_id() != request.request_id() {
            return Err(ExecutionRuntimeErrorV1::AuthorizationRequestMismatch);
        }
        Ok(Self { request, receipt })
    }

    pub const fn action(&self) -> ExecutionActionV1 {
        self.request.action()
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request.request_id()
    }

    pub const fn request(&self) -> &CanonicalExecutionActionRequestV1 {
        &self.request
    }

    pub const fn receipt(&self) -> &AuthorizationReceiptV1 {
        &self.receipt
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepAttemptTerminalV1 {
    Submitted,
    Yielded,
    Failed,
    Cancelled,
    TimedOut,
    Lost,
    Fenced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepAttemptStateV1 {
    Live,
    Terminal(StepAttemptTerminalV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepAttemptV1 {
    id: StepAttemptIdV1,
    lease_id: StepLeaseIdV1,
    binding: StepBindingV1,
    fence: u64,
    executor: PrincipalIdV1,
    store_generation_id: StoreGenerationIdV1,
    authority_epoch: u64,
    fixed_envelope_commitment: [u8; 32],
    run_limit: u32,
    state: StepAttemptStateV1,
}

impl StepAttemptV1 {
    pub const fn id(&self) -> StepAttemptIdV1 {
        self.id
    }

    pub const fn lease_id(&self) -> StepLeaseIdV1 {
        self.lease_id
    }

    pub const fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub const fn fence(&self) -> u64 {
        self.fence
    }

    pub const fn executor(&self) -> PrincipalIdV1 {
        self.executor
    }

    pub const fn store_generation_id(&self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub const fn run_limit(&self) -> u32 {
        self.run_limit
    }

    pub const fn state(&self) -> StepAttemptStateV1 {
        self.state
    }

    pub const fn is_live(&self) -> bool {
        matches!(self.state, StepAttemptStateV1::Live)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepLeaseV1 {
    id: StepLeaseIdV1,
    attempt_id: StepAttemptIdV1,
    binding: StepBindingV1,
    fence: u64,
    current_term_id: LeaseTermIdV1,
    state: StepAttemptStateV1,
}

impl StepLeaseV1 {
    pub const fn id(&self) -> StepLeaseIdV1 {
        self.id
    }

    pub const fn attempt_id(&self) -> StepAttemptIdV1 {
        self.attempt_id
    }

    pub const fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub const fn fence(&self) -> u64 {
        self.fence
    }

    pub const fn current_term_id(&self) -> LeaseTermIdV1 {
        self.current_term_id
    }

    pub const fn state(&self) -> StepAttemptStateV1 {
        self.state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseTermV1 {
    id: LeaseTermIdV1,
    lease_id: StepLeaseIdV1,
    attempt_id: StepAttemptIdV1,
    fence: u64,
    ordinal: u64,
    prior_term_id: Option<LeaseTermIdV1>,
    issued_at: u64,
    expires_at: u64,
    hard_deadline: u64,
    action_request_id: ActionRequestIdV1,
}

impl LeaseTermV1 {
    pub const fn id(&self) -> LeaseTermIdV1 {
        self.id
    }

    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub const fn prior_term_id(&self) -> Option<LeaseTermIdV1> {
        self.prior_term_id
    }

    pub const fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub const fn hard_deadline(&self) -> u64 {
        self.hard_deadline
    }

    pub const fn action_request_id(&self) -> ActionRequestIdV1 {
        self.action_request_id
    }

    pub const fn is_live_at(&self, as_of: u64) -> bool {
        self.issued_at <= as_of && as_of < self.expires_at && as_of < self.hard_deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionAcquisitionV1 {
    pub binding: StepBindingV1,
    pub next_fence: u64,
    pub executor: PrincipalIdV1,
    pub store_generation_id: StoreGenerationIdV1,
    pub authority_epoch: u64,
    pub fixed_envelope_commitment: [u8; 32],
    pub run_limit: u32,
    pub issued_at: u64,
    pub expires_at: u64,
    pub hard_deadline: u64,
    pub authority: AuthorizedExecutionActionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionTenureV1 {
    lease: StepLeaseV1,
    attempt: StepAttemptV1,
    terms: Vec<LeaseTermV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionCarrierV1 {
    tenure: StepExecutionTenureV1,
    run_set: RunSetV1,
}

impl StepExecutionCarrierV1 {
    pub(crate) fn acquire(
        acquisition: StepExecutionAcquisitionV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        let tenure = StepExecutionTenureV1::acquire(acquisition)?;
        let run_set = RunSetV1::new(&ExecutionAttemptV1::Step(Box::new(
            tenure.attempt().clone(),
        )));
        Ok(Self { tenure, run_set })
    }

    pub const fn tenure(&self) -> &StepExecutionTenureV1 {
        &self.tenure
    }

    pub const fn run_set(&self) -> &RunSetV1 {
        &self.run_set
    }

    pub(crate) fn renew(
        &mut self,
        expected_term_id: LeaseTermIdV1,
        issued_at: u64,
        expires_at: u64,
        authority: AuthorizedExecutionActionV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.tenure
            .renew(expected_term_id, issued_at, expires_at, authority)?;
        Ok(())
    }

    pub(crate) fn abandon(
        &mut self,
        terminal: StepAttemptTerminalV1,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        expected_run_set_revision: u64,
        authority: AuthorizedExecutionActionV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.tenure.abandon(
            terminal,
            expected_term_id,
            as_of,
            &mut self.run_set,
            expected_run_set_revision,
            authority,
        )
    }

    pub(crate) fn reserve_run(
        &mut self,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        reservation: RunReservationV1,
    ) -> Result<RunIdV1, ExecutionRuntimeErrorV1> {
        self.tenure.reserve_run(
            &mut self.run_set,
            expected_run_set_revision,
            expected_term_id,
            as_of,
            reservation,
        )
    }

    pub(crate) fn transition_run(
        &mut self,
        run_id: RunIdV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        next: RunStateV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.tenure.transition_run(
            &mut self.run_set,
            run_id,
            expected_run_set_revision,
            expected_term_id,
            as_of,
            next,
        )
    }

    pub(crate) fn mark_run_definitely_not_started(
        &mut self,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        receipt: RunNoStartReceiptV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.tenure.mark_run_definitely_not_started(
            &mut self.run_set,
            expected_run_set_revision,
            expected_term_id,
            as_of,
            receipt,
        )
    }

    pub(crate) fn append_run_segment(
        &mut self,
        append: RunSegmentAppendV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.tenure.append_run_segment(&mut self.run_set, append)
    }

    pub(crate) fn retry_run(
        &mut self,
        predecessor_run_id: RunIdV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        deadline: u64,
    ) -> Result<RunIdV1, ExecutionRuntimeErrorV1> {
        self.tenure.retry_run(
            &mut self.run_set,
            predecessor_run_id,
            expected_run_set_revision,
            expected_term_id,
            as_of,
            deadline,
        )
    }

    pub fn submission_fence(
        &self,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
    ) -> Result<StepSubmissionExecutionFenceV1, ExecutionRuntimeErrorV1> {
        self.tenure
            .submission_fence(expected_term_id, as_of, &self.run_set)
    }

    pub(crate) fn close_for_submission(
        &mut self,
        expected: StepSubmissionExecutionFenceV1,
        as_of: u64,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        let current = self.submission_fence(expected.term_id(), as_of)?;
        if current != expected {
            return Err(ExecutionRuntimeErrorV1::SubmissionFenceUnavailable);
        }
        let state = StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted);
        self.tenure.attempt.state = state;
        self.tenure.lease.state = state;
        self.tenure.validate_pair()
    }

    pub fn canonical_value(&self) -> Result<CborValue, ExecutionRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.step-execution-carrier.v1")?,
            self.tenure.canonical_value()?,
            self.run_set.canonical_value()?,
        ]))
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, ExecutionRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let [CborValue::Text(domain), tenure_value, run_set_value] = fields.as_slice() else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        if domain != "maestro.vnext.step-execution-carrier.v1" {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let tenure = parse_step_execution_tenure(tenure_value)
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
        let run_set = parse_step_run_set(run_set_value, &tenure)
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
        let carrier = Self { tenure, run_set };
        if carrier
            .canonical_value()
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?
            != *value
        {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        Ok(carrier)
    }
}

impl StepExecutionTenureV1 {
    pub(crate) fn acquire(
        acquisition: StepExecutionAcquisitionV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        if acquisition.authority.action() != ExecutionActionV1::AcquireStepExecution {
            return Err(ExecutionRuntimeErrorV1::WrongExecutionAction);
        }
        validate_fence_and_time(
            acquisition.next_fence,
            acquisition.authority_epoch,
            acquisition.issued_at,
            acquisition.expires_at,
            acquisition.hard_deadline,
        )?;
        require_nonzero(acquisition.fixed_envelope_commitment)?;
        if acquisition.run_limit == 0
            || usize::try_from(acquisition.run_limit)
                .ok()
                .is_none_or(|limit| limit > MAX_RUNS_PER_ATTEMPT_V1)
        {
            return Err(ExecutionRuntimeErrorV1::InvalidRunLimit);
        }
        let binding_commitment = step_binding_commitment(acquisition.binding)?;
        let pair_commitment = CborValue::Array(vec![
            bytes(&binding_commitment),
            CborValue::Unsigned(acquisition.next_fence),
            bytes(acquisition.executor.as_bytes()),
            bytes(acquisition.store_generation_id.as_bytes()),
            CborValue::Unsigned(acquisition.authority_epoch),
            bytes(&acquisition.fixed_envelope_commitment),
            CborValue::Unsigned(u64::from(acquisition.run_limit)),
            bytes(acquisition.authority.request_id().as_bytes()),
        ]);
        let lease_id = StepLeaseIdV1::from_value(&CborValue::Array(vec![
            CborValue::text("lease")?,
            pair_commitment.clone(),
        ]))?;
        let attempt_id = StepAttemptIdV1::from_value(&CborValue::Array(vec![
            CborValue::text("attempt")?,
            pair_commitment,
            bytes(lease_id.as_bytes()),
        ]))?;
        let term = new_term(LeaseTermPartsV1 {
            lease_id,
            attempt_id,
            fence: acquisition.next_fence,
            ordinal: 1,
            prior_term_id: None,
            issued_at: acquisition.issued_at,
            expires_at: acquisition.expires_at,
            hard_deadline: acquisition.hard_deadline,
            action_request_id: acquisition.authority.request_id(),
        })?;
        let lease = StepLeaseV1 {
            id: lease_id,
            attempt_id,
            binding: acquisition.binding,
            fence: acquisition.next_fence,
            current_term_id: term.id(),
            state: StepAttemptStateV1::Live,
        };
        let attempt = StepAttemptV1 {
            id: attempt_id,
            lease_id,
            binding: acquisition.binding,
            fence: acquisition.next_fence,
            executor: acquisition.executor,
            store_generation_id: acquisition.store_generation_id,
            authority_epoch: acquisition.authority_epoch,
            fixed_envelope_commitment: acquisition.fixed_envelope_commitment,
            run_limit: acquisition.run_limit,
            state: StepAttemptStateV1::Live,
        };
        let tenure = Self {
            lease,
            attempt,
            terms: vec![term],
        };
        tenure.validate_pair()?;
        Ok(tenure)
    }

    pub const fn lease(&self) -> &StepLeaseV1 {
        &self.lease
    }

    pub const fn attempt(&self) -> &StepAttemptV1 {
        &self.attempt
    }

    pub fn terms(&self) -> &[LeaseTermV1] {
        &self.terms
    }

    pub fn current_term(&self) -> &LeaseTermV1 {
        self.terms
            .last()
            .expect("invariant: a Step execution tenure always has an initial term")
    }

    pub(crate) fn renew(
        &mut self,
        expected_term_id: LeaseTermIdV1,
        issued_at: u64,
        expires_at: u64,
        authority: AuthorizedExecutionActionV1,
    ) -> Result<&LeaseTermV1, ExecutionRuntimeErrorV1> {
        if authority.action() != ExecutionActionV1::RenewStepLeaseTerm {
            return Err(ExecutionRuntimeErrorV1::WrongExecutionAction);
        }
        if !self.attempt.is_live() || self.lease.state != StepAttemptStateV1::Live {
            return Err(ExecutionRuntimeErrorV1::TerminalAttempt);
        }
        let current = self.current_term();
        if current.id() != expected_term_id {
            return Err(ExecutionRuntimeErrorV1::StaleLeaseTerm);
        }
        if issued_at >= current.expires_at()
            || issued_at < current.issued_at()
            || expires_at <= issued_at
            || expires_at > current.hard_deadline()
        {
            return Err(ExecutionRuntimeErrorV1::InvalidLeaseTime);
        }
        let ordinal = current
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionRuntimeErrorV1::LeaseTermOverflow)?;
        let successor = new_term(LeaseTermPartsV1 {
            lease_id: self.lease.id,
            attempt_id: self.attempt.id,
            fence: self.lease.fence,
            ordinal,
            prior_term_id: Some(current.id()),
            issued_at,
            expires_at,
            hard_deadline: current.hard_deadline(),
            action_request_id: authority.request_id(),
        })?;
        self.lease.current_term_id = successor.id();
        self.terms.push(successor);
        self.validate_pair()?;
        Ok(self.current_term())
    }

    pub(crate) fn abandon(
        &mut self,
        terminal: StepAttemptTerminalV1,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        run_set: &mut RunSetV1,
        expected_run_set_revision: u64,
        authority: AuthorizedExecutionActionV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        if terminal == StepAttemptTerminalV1::Submitted {
            return Err(ExecutionRuntimeErrorV1::SubmissionOwnedByStep);
        }
        if authority.action() != ExecutionActionV1::AbandonStepAttempt {
            return Err(ExecutionRuntimeErrorV1::WrongExecutionAction);
        }
        if !self.attempt.is_live() || self.current_term().id() != expected_term_id {
            return Err(ExecutionRuntimeErrorV1::StaleLeaseTerm);
        }
        if as_of < self.current_term().issued_at() {
            return Err(ExecutionRuntimeErrorV1::InvalidLeaseTime);
        }
        let expired = as_of >= self.current_term().expires_at();
        if (terminal == StepAttemptTerminalV1::TimedOut) != expired {
            return Err(ExecutionRuntimeErrorV1::InvalidLeaseTime);
        }
        if run_set.owner() != ExecutionAttemptOwnerV1::Step(self.attempt.id())
            || run_set.revision() != expected_run_set_revision
        {
            return Err(ExecutionRuntimeErrorV1::StaleRunSetRevision);
        }
        match terminal {
            StepAttemptTerminalV1::Cancelled => {
                run_set.close_open(expected_run_set_revision, RunStateV1::Cancelled)?;
            }
            StepAttemptTerminalV1::TimedOut => {
                run_set.close_open(expected_run_set_revision, RunStateV1::TimedOut)?;
            }
            StepAttemptTerminalV1::Lost => {
                run_set.close_open(expected_run_set_revision, RunStateV1::Lost)?;
            }
            StepAttemptTerminalV1::Fenced => {
                run_set.close_open(expected_run_set_revision, RunStateV1::Fenced)?;
            }
            StepAttemptTerminalV1::Yielded | StepAttemptTerminalV1::Failed => {
                if run_set.runs().iter().any(|run| !run.state().is_terminal()) {
                    return Err(ExecutionRuntimeErrorV1::OpenRunsAtAttemptTerminal);
                }
            }
            StepAttemptTerminalV1::Submitted => {
                return Err(ExecutionRuntimeErrorV1::SubmissionOwnedByStep);
            }
        }
        let state = StepAttemptStateV1::Terminal(terminal);
        self.attempt.state = state;
        self.lease.state = state;
        self.validate_pair()
    }

    pub(crate) fn reserve_run(
        &self,
        run_set: &mut RunSetV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        mut reservation: RunReservationV1,
    ) -> Result<RunIdV1, ExecutionRuntimeErrorV1> {
        self.validate_live_run_basis(run_set, expected_run_set_revision, expected_term_id, as_of)?;
        if reservation.fixed_envelope_commitment()? != self.attempt.fixed_envelope_commitment {
            return Err(ExecutionRuntimeErrorV1::RunOutsideFixedEnvelope);
        }
        if run_set.runs().len()
            >= usize::try_from(self.attempt.run_limit)
                .map_err(|_| ExecutionRuntimeErrorV1::InvalidRunLimit)?
        {
            return Err(ExecutionRuntimeErrorV1::RunBudgetExhausted);
        }
        if reservation.launch_ordinal != 1
            || run_set
                .runs()
                .iter()
                .any(|run| run.semantic_operation_hash == reservation.semantic_operation_hash)
        {
            return Err(ExecutionRuntimeErrorV1::InvalidRunLaunchChain);
        }
        let term = self.current_term();
        reservation.current_step_term = Some(term.id());
        if reservation.deadline <= as_of
            || reservation.deadline > term.expires_at()
            || reservation.deadline > term.hard_deadline()
        {
            return Err(ExecutionRuntimeErrorV1::RunDeadlineOutsideTerm);
        }
        let attempt = ExecutionAttemptV1::Step(Box::new(self.attempt.clone()));
        let run = RunV1::reserve(&attempt, reservation)?;
        let id = run.id();
        run_set.insert_initial(run)?;
        Ok(id)
    }

    pub(crate) fn transition_run(
        &self,
        run_set: &mut RunSetV1,
        run_id: RunIdV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        next: RunStateV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.validate_live_run_basis(run_set, expected_run_set_revision, expected_term_id, as_of)?;
        let run = run_set
            .runs()
            .iter()
            .find(|run| run.id() == run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        if next == RunStateV1::DefinitelyNotStarted {
            return Err(ExecutionRuntimeErrorV1::RunNoStartProofRequired);
        }
        if next == RunStateV1::TimedOut && as_of < run.deadline {
            return Err(ExecutionRuntimeErrorV1::RunDeadlineNotReached);
        }
        if as_of >= run.deadline
            && matches!(
                next,
                RunStateV1::Active | RunStateV1::Succeeded | RunStateV1::Failed
            )
        {
            return Err(ExecutionRuntimeErrorV1::RunDeadlineExpired);
        }
        run_set.transition(run_id, expected_run_set_revision, next)
    }

    pub(crate) fn mark_run_definitely_not_started(
        &self,
        run_set: &mut RunSetV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        receipt: RunNoStartReceiptV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.validate_live_run_basis(run_set, expected_run_set_revision, expected_term_id, as_of)?;
        let run = run_set
            .runs()
            .iter()
            .find(|run| run.id() == receipt.run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        receipt.validate(run, as_of)?;
        run_set.transition(
            receipt.run_id,
            expected_run_set_revision,
            RunStateV1::DefinitelyNotStarted,
        )
    }

    pub(crate) fn append_run_segment(
        &self,
        run_set: &mut RunSetV1,
        append: RunSegmentAppendV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.validate_live_run_basis(
            run_set,
            append.expected_run_set_revision,
            append.expected_term_id,
            append.as_of,
        )?;
        let run = run_set
            .runs()
            .iter()
            .find(|run| run.id() == append.run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        if append.as_of >= run.deadline {
            return Err(ExecutionRuntimeErrorV1::RunDeadlineExpired);
        }
        run_set.append_segment(
            append.run_id,
            append.expected_run_set_revision,
            append.process_or_job_identity,
            append.segment_commitment,
        )
    }

    pub(crate) fn retry_run(
        &self,
        run_set: &mut RunSetV1,
        predecessor_run_id: RunIdV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        deadline: u64,
    ) -> Result<RunIdV1, ExecutionRuntimeErrorV1> {
        self.validate_live_run_basis(run_set, expected_run_set_revision, expected_term_id, as_of)?;
        let predecessor = run_set
            .runs()
            .iter()
            .find(|run| run.id() == predecessor_run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        let attempt = ExecutionAttemptV1::Step(Box::new(self.attempt.clone()));
        let mut reservation = predecessor.retry_reservation(&attempt, deadline)?;
        if reservation.fixed_envelope_commitment()? != self.attempt.fixed_envelope_commitment {
            return Err(ExecutionRuntimeErrorV1::RunOutsideFixedEnvelope);
        }
        reservation.current_step_term = Some(self.current_term().id());
        if run_set.runs().len()
            >= usize::try_from(self.attempt.run_limit)
                .map_err(|_| ExecutionRuntimeErrorV1::InvalidRunLimit)?
        {
            return Err(ExecutionRuntimeErrorV1::RunBudgetExhausted);
        }
        let term = self.current_term();
        if reservation.deadline <= as_of
            || reservation.deadline > term.expires_at()
            || reservation.deadline > term.hard_deadline()
        {
            return Err(ExecutionRuntimeErrorV1::RunDeadlineOutsideTerm);
        }
        let run = RunV1::reserve(&attempt, reservation)?;
        let run_id = run.id();
        run_set.insert_retry(predecessor_run_id, run)?;
        Ok(run_id)
    }

    fn validate_live_run_basis(
        &self,
        run_set: &RunSetV1,
        expected_run_set_revision: u64,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.validate_pair()?;
        let term = self.current_term();
        if !self.attempt.is_live()
            || term.id() != expected_term_id
            || !term.is_live_at(as_of)
            || run_set.owner() != ExecutionAttemptOwnerV1::Step(self.attempt.id())
            || run_set.revision() != expected_run_set_revision
        {
            return Err(ExecutionRuntimeErrorV1::StaleRunOrLeaseBasis);
        }
        Ok(())
    }

    pub fn submission_fence(
        &self,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        run_set: &RunSetV1,
    ) -> Result<StepSubmissionExecutionFenceV1, ExecutionRuntimeErrorV1> {
        self.validate_pair()?;
        let term = self.current_term();
        if !self.attempt.is_live()
            || term.id() != expected_term_id
            || !term.is_live_at(as_of)
            || run_set.owner() != ExecutionAttemptOwnerV1::Step(self.attempt.id())
            || !run_set.is_submission_quiescent()
        {
            return Err(ExecutionRuntimeErrorV1::SubmissionFenceUnavailable);
        }
        Ok(StepSubmissionExecutionFenceV1 {
            binding_commitment: step_binding_commitment(self.attempt.binding)?,
            lease_id: self.lease.id,
            attempt_id: self.attempt.id,
            fence: self.attempt.fence,
            term_id: term.id(),
            term_ordinal: term.ordinal(),
            run_set_revision: run_set.revision(),
            authority_epoch: self.attempt.authority_epoch,
            store_generation_id: self.attempt.store_generation_id,
        })
    }

    pub fn canonical_value(&self) -> Result<CborValue, ExecutionRuntimeErrorV1> {
        self.validate_pair()?;
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.step-execution-tenure.v1")?,
            CborValue::Array(vec![
                bytes(self.lease.id.as_bytes()),
                bytes(self.lease.attempt_id.as_bytes()),
                step_binding_value(self.lease.binding),
                CborValue::Unsigned(self.lease.fence),
                bytes(self.lease.current_term_id.as_bytes()),
                step_attempt_state_value(self.lease.state),
            ]),
            CborValue::Array(vec![
                bytes(self.attempt.id.as_bytes()),
                bytes(self.attempt.lease_id.as_bytes()),
                step_binding_value(self.attempt.binding),
                CborValue::Unsigned(self.attempt.fence),
                bytes(self.attempt.executor.as_bytes()),
                bytes(self.attempt.store_generation_id.as_bytes()),
                CborValue::Unsigned(self.attempt.authority_epoch),
                bytes(&self.attempt.fixed_envelope_commitment),
                CborValue::Unsigned(u64::from(self.attempt.run_limit)),
                step_attempt_state_value(self.attempt.state),
            ]),
            CborValue::Array(
                self.terms
                    .iter()
                    .map(|term| {
                        CborValue::Array(vec![
                            bytes(term.id.as_bytes()),
                            bytes(term.lease_id.as_bytes()),
                            bytes(term.attempt_id.as_bytes()),
                            CborValue::Unsigned(term.fence),
                            CborValue::Unsigned(term.ordinal),
                            CborValue::optional(
                                term.prior_term_id.map(|prior| bytes(prior.as_bytes())),
                            ),
                            CborValue::Unsigned(term.issued_at),
                            CborValue::Unsigned(term.expires_at),
                            CborValue::Unsigned(term.hard_deadline),
                            bytes(term.action_request_id.as_bytes()),
                        ])
                    })
                    .collect(),
            ),
        ]))
    }

    fn validate_pair(&self) -> Result<(), ExecutionRuntimeErrorV1> {
        if self.lease.attempt_id != self.attempt.id
            || self.attempt.lease_id != self.lease.id
            || self.lease.binding != self.attempt.binding
            || self.lease.fence != self.attempt.fence
            || self.lease.state != self.attempt.state
            || self.terms.is_empty()
            || self.lease.current_term_id != self.current_term().id()
            || self.terms.len() > MAX_RUN_SEGMENTS_V1
        {
            return Err(ExecutionRuntimeErrorV1::BrokenLeaseAttemptPair);
        }
        for (index, term) in self.terms.iter().enumerate() {
            let expected_ordinal = u64::try_from(index)
                .map_err(|_| ExecutionRuntimeErrorV1::LeaseTermOverflow)?
                .checked_add(1)
                .ok_or(ExecutionRuntimeErrorV1::LeaseTermOverflow)?;
            let expected_prior = index.checked_sub(1).map(|prior| self.terms[prior].id());
            if term.lease_id != self.lease.id
                || term.attempt_id != self.attempt.id
                || term.fence != self.lease.fence
                || term.ordinal != expected_ordinal
                || term.prior_term_id != expected_prior
            {
                return Err(ExecutionRuntimeErrorV1::BrokenLeaseAttemptPair);
            }
            if let Some(prior_index) = index.checked_sub(1) {
                let prior = &self.terms[prior_index];
                if term.issued_at < prior.issued_at
                    || term.issued_at >= prior.expires_at
                    || term.expires_at <= term.issued_at
                    || term.expires_at > prior.hard_deadline
                    || term.hard_deadline != prior.hard_deadline
                {
                    return Err(ExecutionRuntimeErrorV1::BrokenLeaseAttemptPair);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepSubmissionExecutionFenceV1 {
    binding_commitment: [u8; 32],
    lease_id: StepLeaseIdV1,
    attempt_id: StepAttemptIdV1,
    fence: u64,
    term_id: LeaseTermIdV1,
    term_ordinal: u64,
    run_set_revision: u64,
    authority_epoch: u64,
    store_generation_id: StoreGenerationIdV1,
}

impl StepSubmissionExecutionFenceV1 {
    pub const fn binding_commitment(self) -> [u8; 32] {
        self.binding_commitment
    }

    pub const fn lease_id(self) -> StepLeaseIdV1 {
        self.lease_id
    }

    pub const fn attempt_id(self) -> StepAttemptIdV1 {
        self.attempt_id
    }

    pub const fn fence(self) -> u64 {
        self.fence
    }

    pub const fn term_id(self) -> LeaseTermIdV1 {
        self.term_id
    }

    pub const fn run_set_revision(self) -> u64 {
        self.run_set_revision
    }

    pub const fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub const fn store_generation_id(self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            bytes(&self.binding_commitment),
            bytes(self.lease_id.as_bytes()),
            bytes(self.attempt_id.as_bytes()),
            CborValue::Unsigned(self.fence),
            bytes(self.term_id.as_bytes()),
            CborValue::Unsigned(self.term_ordinal),
            CborValue::Unsigned(self.run_set_revision),
            CborValue::Unsigned(self.authority_epoch),
            bytes(self.store_generation_id.as_bytes()),
        ])
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, ExecutionRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let [
            binding_commitment,
            lease_id,
            attempt_id,
            CborValue::Unsigned(fence),
            term_id,
            CborValue::Unsigned(term_ordinal),
            CborValue::Unsigned(run_set_revision),
            CborValue::Unsigned(authority_epoch),
            store_generation_id,
        ] = fields.as_slice()
        else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        if *fence == 0 || *term_ordinal == 0 || *authority_epoch == 0 {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let decoded = Self {
            binding_commitment: exact_runtime_digest(binding_commitment)?,
            lease_id: StepLeaseIdV1::from_bytes(exact_runtime_digest(lease_id)?)?,
            attempt_id: StepAttemptIdV1::from_bytes(exact_runtime_digest(attempt_id)?)?,
            fence: *fence,
            term_id: LeaseTermIdV1::from_bytes(exact_runtime_digest(term_id)?)?,
            term_ordinal: *term_ordinal,
            run_set_revision: *run_set_revision,
            authority_epoch: *authority_epoch,
            store_generation_id: StoreGenerationIdV1::from_digest(exact_runtime_digest(
                store_generation_id,
            )?),
        };
        if decoded.canonical_value() != *value {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        Ok(decoded)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchAttemptV1 {
    id: DispatchAttemptIdV1,
    effect_intent_id: EffectIntentIdV1,
    dispatch_fence: u64,
    use_fence_commitment: [u8; 32],
    originating_step_provenance: Option<[u8; 32]>,
}

impl DispatchAttemptV1 {
    pub fn new(
        effect_intent_id: EffectIntentIdV1,
        dispatch_fence: u64,
        use_fence_commitment: [u8; 32],
        originating_step_provenance: Option<StepBindingV1>,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        if dispatch_fence == 0 {
            return Err(ExecutionRuntimeErrorV1::InvalidFence);
        }
        require_nonzero(use_fence_commitment)?;
        let provenance = originating_step_provenance
            .map(step_binding_commitment)
            .transpose()?;
        let value = CborValue::Array(vec![
            bytes(effect_intent_id.as_bytes()),
            CborValue::Unsigned(dispatch_fence),
            bytes(&use_fence_commitment),
            CborValue::optional(provenance.map(|value| bytes(&value))),
        ]);
        Ok(Self {
            id: DispatchAttemptIdV1::from_value(&value)?,
            effect_intent_id,
            dispatch_fence,
            use_fence_commitment,
            originating_step_provenance: provenance,
        })
    }

    pub(crate) fn from_persisted(
        id: DispatchAttemptIdV1,
        effect_intent_id: EffectIntentIdV1,
        dispatch_fence: u64,
        use_fence_commitment: [u8; 32],
        originating_step_provenance: Option<StepBindingV1>,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        let rebuilt = Self::new(
            effect_intent_id,
            dispatch_fence,
            use_fence_commitment,
            originating_step_provenance,
        )?;
        if rebuilt.id != id {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        Ok(rebuilt)
    }

    pub const fn id(self) -> DispatchAttemptIdV1 {
        self.id
    }

    pub const fn effect_intent_id(self) -> EffectIntentIdV1 {
        self.effect_intent_id
    }

    pub const fn dispatch_fence(self) -> u64 {
        self.dispatch_fence
    }

    pub const fn has_step_lease_authority(self) -> bool {
        false
    }

    pub const fn may_mutate_originating_step(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationAttemptV1 {
    id: ReconciliationAttemptIdV1,
    effect_intent_id: EffectIntentIdV1,
    action_request_id: ActionRequestIdV1,
    use_fence_commitment: [u8; 32],
    read_plan_commitment: [u8; 32],
    originating_step_provenance: Option<[u8; 32]>,
}

impl ReconciliationAttemptV1 {
    pub fn new(
        effect_intent_id: EffectIntentIdV1,
        use_fence_commitment: [u8; 32],
        read_plan_commitment: [u8; 32],
        originating_step_provenance: Option<StepBindingV1>,
        authority: &AuthorizedExecutionActionV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        if !matches!(
            authority.action(),
            ExecutionActionV1::ReconcileEffectIntent
                | ExecutionActionV1::ReconcileBootstrapMandateInteractionEffect
                | ExecutionActionV1::ReconcileContinuityMaintenanceEffect
        ) {
            return Err(ExecutionRuntimeErrorV1::WrongExecutionAction);
        }
        require_nonzero(use_fence_commitment)?;
        require_nonzero(read_plan_commitment)?;
        let provenance = originating_step_provenance
            .map(step_binding_commitment)
            .transpose()?;
        let value = CborValue::Array(vec![
            bytes(effect_intent_id.as_bytes()),
            bytes(authority.request_id().as_bytes()),
            bytes(&use_fence_commitment),
            bytes(&read_plan_commitment),
            CborValue::optional(provenance.map(|value| bytes(&value))),
        ]);
        Ok(Self {
            id: ReconciliationAttemptIdV1::from_value(&value)?,
            effect_intent_id,
            action_request_id: authority.request_id(),
            use_fence_commitment,
            read_plan_commitment,
            originating_step_provenance: provenance,
        })
    }

    pub(crate) fn from_persisted(
        id: ReconciliationAttemptIdV1,
        effect_intent_id: EffectIntentIdV1,
        action_request_id: ActionRequestIdV1,
        use_fence_commitment: [u8; 32],
        read_plan_commitment: [u8; 32],
        originating_step_provenance: Option<StepBindingV1>,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        require_nonzero(use_fence_commitment)?;
        require_nonzero(read_plan_commitment)?;
        let provenance = originating_step_provenance
            .map(step_binding_commitment)
            .transpose()?;
        let expected = ReconciliationAttemptIdV1::from_value(&CborValue::Array(vec![
            bytes(effect_intent_id.as_bytes()),
            bytes(action_request_id.as_bytes()),
            bytes(&use_fence_commitment),
            bytes(&read_plan_commitment),
            CborValue::optional(provenance.map(|value| bytes(&value))),
        ]))?;
        if id != expected {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        Ok(Self {
            id,
            effect_intent_id,
            action_request_id,
            use_fence_commitment,
            read_plan_commitment,
            originating_step_provenance: provenance,
        })
    }

    pub const fn id(self) -> ReconciliationAttemptIdV1 {
        self.id
    }

    pub const fn effect_intent_id(self) -> EffectIntentIdV1 {
        self.effect_intent_id
    }

    pub const fn action_request_id(self) -> ActionRequestIdV1 {
        self.action_request_id
    }

    pub const fn has_step_lease_authority(self) -> bool {
        false
    }

    pub const fn may_mutate_originating_step(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionAttemptV1 {
    Step(Box<StepAttemptV1>),
    Dispatch(DispatchAttemptV1),
    Reconciliation(ReconciliationAttemptV1),
}

impl ExecutionAttemptV1 {
    pub const fn owner(&self) -> ExecutionAttemptOwnerV1 {
        match self {
            Self::Step(attempt) => ExecutionAttemptOwnerV1::Step(attempt.id()),
            Self::Dispatch(attempt) => ExecutionAttemptOwnerV1::Dispatch(attempt.id()),
            Self::Reconciliation(attempt) => ExecutionAttemptOwnerV1::Reconciliation(attempt.id()),
        }
    }

    pub const fn has_step_lease_authority(&self) -> bool {
        matches!(self, Self::Step(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionAttemptOwnerV1 {
    Step(StepAttemptIdV1),
    Dispatch(DispatchAttemptIdV1),
    Reconciliation(ReconciliationAttemptIdV1),
}

impl ExecutionAttemptOwnerV1 {
    fn canonical_value(self) -> CborValue {
        match self {
            Self::Step(id) => CborValue::Array(vec![CborValue::Unsigned(1), bytes(id.as_bytes())]),
            Self::Dispatch(id) => {
                CborValue::Array(vec![CborValue::Unsigned(2), bytes(id.as_bytes())])
            }
            Self::Reconciliation(id) => {
                CborValue::Array(vec![CborValue::Unsigned(3), bytes(id.as_bytes())])
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStateV1 {
    Reserved,
    Active,
    DefinitelyNotStarted,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Lost,
    Fenced,
}

impl RunStateV1 {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Reserved | Self::Active)
    }

    pub const fn is_submission_certain(self) -> bool {
        matches!(
            self,
            Self::DefinitelyNotStarted | Self::Succeeded | Self::Failed | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunReservationV1 {
    pub semantic_operation_hash: [u8; 32],
    pub inputs_commitment: [u8; 32],
    pub environment_commitment: [u8; 32],
    pub target_commitment: [u8; 32],
    pub execution_boundary_commitment: [u8; 32],
    pub deadline: u64,
    pub launch_ordinal: u32,
    pub current_step_term: Option<LeaseTermIdV1>,
}

impl RunReservationV1 {
    pub fn fixed_envelope_commitment(&self) -> Result<[u8; 32], ExecutionRuntimeErrorV1> {
        for commitment in [
            self.semantic_operation_hash,
            self.inputs_commitment,
            self.environment_commitment,
            self.target_commitment,
            self.execution_boundary_commitment,
        ] {
            require_nonzero(commitment)?;
        }
        Ok(hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.run-fixed-envelope.v1")?,
            bytes(&self.semantic_operation_hash),
            bytes(&self.inputs_commitment),
            bytes(&self.environment_commitment),
            bytes(&self.target_commitment),
            bytes(&self.execution_boundary_commitment),
        ]))?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunNoStartReceiptV1 {
    run_id: RunIdV1,
    owner: ExecutionAttemptOwnerV1,
    step_term_id: LeaseTermIdV1,
    execution_boundary_commitment: [u8; 32],
    deadline: u64,
    observed_at: u64,
    observer_commitment: [u8; 32],
    observation_commitment: [u8; 32],
    proof_commitment: [u8; 32],
}

impl RunNoStartReceiptV1 {
    pub(crate) fn from_validated_boundary_observation(
        run: &RunV1,
        observed_at: u64,
        observer_commitment: [u8; 32],
        observation_commitment: [u8; 32],
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        let step_term_id = run
            .current_step_term
            .ok_or(ExecutionRuntimeErrorV1::RunNoStartProofRequired)?;
        if run.state != RunStateV1::Reserved {
            return Err(ExecutionRuntimeErrorV1::RunNoStartProofRequired);
        }
        require_nonzero(observer_commitment)?;
        require_nonzero(observation_commitment)?;
        let mut receipt = Self {
            run_id: run.id,
            owner: run.owner,
            step_term_id,
            execution_boundary_commitment: run.execution_boundary_commitment,
            deadline: run.deadline,
            observed_at,
            observer_commitment,
            observation_commitment,
            proof_commitment: [0; 32],
        };
        receipt.proof_commitment = receipt.compute_commitment()?;
        Ok(receipt)
    }

    pub const fn run_id(&self) -> RunIdV1 {
        self.run_id
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.run-no-start-receipt.v1")
                .expect("invariant: static receipt domain is valid CBOR text"),
            bytes(self.run_id.as_bytes()),
            self.owner.canonical_value(),
            bytes(self.step_term_id.as_bytes()),
            bytes(&self.execution_boundary_commitment),
            CborValue::Unsigned(self.deadline),
            CborValue::Unsigned(self.observed_at),
            bytes(&self.observer_commitment),
            bytes(&self.observation_commitment),
            bytes(&self.proof_commitment),
        ])
    }

    fn validate(&self, run: &RunV1, as_of: u64) -> Result<(), ExecutionRuntimeErrorV1> {
        if run.state != RunStateV1::Reserved
            || self.run_id != run.id
            || self.owner != run.owner
            || run.current_step_term != Some(self.step_term_id)
            || self.execution_boundary_commitment != run.execution_boundary_commitment
            || self.deadline != run.deadline
            || self.observed_at != as_of
            || self.observer_commitment == [0; 32]
            || self.observation_commitment == [0; 32]
            || self.proof_commitment != self.compute_commitment()?
        {
            return Err(ExecutionRuntimeErrorV1::RunNoStartProofRequired);
        }
        Ok(())
    }

    fn compute_commitment(&self) -> Result<[u8; 32], ExecutionRuntimeErrorV1> {
        Ok(hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.run-no-start-proof.v1")?,
            bytes(self.run_id.as_bytes()),
            self.owner.canonical_value(),
            bytes(self.step_term_id.as_bytes()),
            bytes(&self.execution_boundary_commitment),
            CborValue::Unsigned(self.deadline),
            CborValue::Unsigned(self.observed_at),
            bytes(&self.observer_commitment),
            bytes(&self.observation_commitment),
        ]))?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunSegmentAppendV1 {
    pub(crate) run_id: RunIdV1,
    pub(crate) expected_run_set_revision: u64,
    pub(crate) expected_term_id: LeaseTermIdV1,
    pub(crate) as_of: u64,
    pub(crate) process_or_job_identity: [u8; 32],
    pub(crate) segment_commitment: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunV1 {
    id: RunIdV1,
    owner: ExecutionAttemptOwnerV1,
    semantic_operation_hash: [u8; 32],
    inputs_commitment: [u8; 32],
    environment_commitment: [u8; 32],
    target_commitment: [u8; 32],
    execution_boundary_commitment: [u8; 32],
    deadline: u64,
    launch_ordinal: u32,
    current_step_term: Option<LeaseTermIdV1>,
    state: RunStateV1,
    segments: Vec<RunSegmentV1>,
}

impl RunV1 {
    fn reserve(
        attempt: &ExecutionAttemptV1,
        reservation: RunReservationV1,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        for commitment in [
            reservation.semantic_operation_hash,
            reservation.inputs_commitment,
            reservation.environment_commitment,
            reservation.target_commitment,
            reservation.execution_boundary_commitment,
        ] {
            require_nonzero(commitment)?;
        }
        if reservation.deadline == 0 || reservation.launch_ordinal == 0 {
            return Err(ExecutionRuntimeErrorV1::InvalidRunReservation);
        }
        match attempt {
            ExecutionAttemptV1::Step(step) => {
                if !step.is_live() || reservation.current_step_term.is_none() {
                    return Err(ExecutionRuntimeErrorV1::StepRunRequiresLiveTerm);
                }
            }
            ExecutionAttemptV1::Dispatch(_) | ExecutionAttemptV1::Reconciliation(_) => {
                if reservation.current_step_term.is_some() {
                    return Err(ExecutionRuntimeErrorV1::NonStepRunHasLeaseTerm);
                }
            }
        }
        let owner = attempt.owner();
        let value = CborValue::Array(vec![
            owner.canonical_value(),
            bytes(&reservation.semantic_operation_hash),
            bytes(&reservation.inputs_commitment),
            bytes(&reservation.environment_commitment),
            bytes(&reservation.target_commitment),
            bytes(&reservation.execution_boundary_commitment),
            CborValue::Unsigned(reservation.deadline),
            CborValue::Unsigned(u64::from(reservation.launch_ordinal)),
            CborValue::optional(
                reservation
                    .current_step_term
                    .map(|term| bytes(term.as_bytes())),
            ),
        ]);
        Ok(Self {
            id: RunIdV1::from_value(&value)?,
            owner,
            semantic_operation_hash: reservation.semantic_operation_hash,
            inputs_commitment: reservation.inputs_commitment,
            environment_commitment: reservation.environment_commitment,
            target_commitment: reservation.target_commitment,
            execution_boundary_commitment: reservation.execution_boundary_commitment,
            deadline: reservation.deadline,
            launch_ordinal: reservation.launch_ordinal,
            current_step_term: reservation.current_step_term,
            state: RunStateV1::Reserved,
            segments: Vec::new(),
        })
    }

    pub const fn id(&self) -> RunIdV1 {
        self.id
    }

    pub const fn owner(&self) -> ExecutionAttemptOwnerV1 {
        self.owner
    }

    pub const fn state(&self) -> RunStateV1 {
        self.state
    }

    pub const fn launch_ordinal(&self) -> u32 {
        self.launch_ordinal
    }

    pub const fn reservation(&self) -> RunReservationV1 {
        RunReservationV1 {
            semantic_operation_hash: self.semantic_operation_hash,
            inputs_commitment: self.inputs_commitment,
            environment_commitment: self.environment_commitment,
            target_commitment: self.target_commitment,
            execution_boundary_commitment: self.execution_boundary_commitment,
            deadline: self.deadline,
            launch_ordinal: self.launch_ordinal,
            current_step_term: self.current_step_term,
        }
    }

    pub fn segments(&self) -> &[RunSegmentV1] {
        &self.segments
    }

    fn transition(&mut self, next: RunStateV1) -> Result<(), ExecutionRuntimeErrorV1> {
        let legal = match self.state {
            RunStateV1::Reserved => matches!(
                next,
                RunStateV1::Active
                    | RunStateV1::DefinitelyNotStarted
                    | RunStateV1::Cancelled
                    | RunStateV1::TimedOut
                    | RunStateV1::Lost
                    | RunStateV1::Fenced
            ),
            RunStateV1::Active => matches!(
                next,
                RunStateV1::Succeeded
                    | RunStateV1::Failed
                    | RunStateV1::Cancelled
                    | RunStateV1::TimedOut
                    | RunStateV1::Lost
                    | RunStateV1::Fenced
            ),
            _ => false,
        };
        if !legal {
            return Err(ExecutionRuntimeErrorV1::IllegalRunTransition);
        }
        self.state = next;
        Ok(())
    }

    fn retry_reservation(
        &self,
        attempt: &ExecutionAttemptV1,
        deadline: u64,
    ) -> Result<RunReservationV1, ExecutionRuntimeErrorV1> {
        if self.state != RunStateV1::DefinitelyNotStarted || attempt.owner() != self.owner {
            return Err(ExecutionRuntimeErrorV1::RunRetryBoundaryUnknown);
        }
        let launch_ordinal = self
            .launch_ordinal
            .checked_add(1)
            .ok_or(ExecutionRuntimeErrorV1::RunLaunchOrdinalOverflow)?;
        Ok(RunReservationV1 {
            semantic_operation_hash: self.semantic_operation_hash,
            inputs_commitment: self.inputs_commitment,
            environment_commitment: self.environment_commitment,
            target_commitment: self.target_commitment,
            execution_boundary_commitment: self.execution_boundary_commitment,
            deadline,
            launch_ordinal,
            current_step_term: self.current_step_term,
        })
    }

    fn append_segment(
        &mut self,
        process_or_job_identity: [u8; 32],
        segment_commitment: [u8; 32],
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        if self.state != RunStateV1::Active || self.segments.len() >= MAX_RUN_SEGMENTS_V1 {
            return Err(ExecutionRuntimeErrorV1::RunSegmentUnavailable);
        }
        require_nonzero(process_or_job_identity)?;
        require_nonzero(segment_commitment)?;
        if self
            .segments
            .first()
            .is_some_and(|segment| segment.process_or_job_identity != process_or_job_identity)
        {
            return Err(ExecutionRuntimeErrorV1::RunIdentityChanged);
        }
        let ordinal = u64::try_from(self.segments.len())
            .map_err(|_| ExecutionRuntimeErrorV1::RunSegmentUnavailable)?
            .checked_add(1)
            .ok_or(ExecutionRuntimeErrorV1::RunSegmentUnavailable)?;
        self.segments.push(RunSegmentV1 {
            ordinal,
            process_or_job_identity,
            segment_commitment,
        });
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunSegmentV1 {
    ordinal: u64,
    process_or_job_identity: [u8; 32],
    segment_commitment: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSetV1 {
    owner: ExecutionAttemptOwnerV1,
    max_runs: usize,
    revision: u64,
    runs: Vec<RunV1>,
}

impl RunSetV1 {
    pub(crate) fn new(attempt: &ExecutionAttemptV1) -> Self {
        let max_runs = match attempt {
            ExecutionAttemptV1::Step(attempt) => attempt.run_limit as usize,
            ExecutionAttemptV1::Dispatch(_) | ExecutionAttemptV1::Reconciliation(_) => {
                MAX_RUNS_PER_ATTEMPT_V1
            }
        };
        Self {
            owner: attempt.owner(),
            max_runs,
            revision: 1,
            runs: Vec::new(),
        }
    }

    pub(crate) fn reserve_non_step_run_at_revision(
        attempt: &ExecutionAttemptV1,
        reservation: RunReservationV1,
        current_revision: u64,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        if matches!(attempt, ExecutionAttemptV1::Step(_)) || reservation.current_step_term.is_some()
        {
            return Err(ExecutionRuntimeErrorV1::NonStepRunHasLeaseTerm);
        }
        if current_revision == 0 {
            return Err(ExecutionRuntimeErrorV1::StaleRunSetRevision);
        }
        let mut run_set = Self::new(attempt);
        run_set.revision = current_revision;
        let run = RunV1::reserve(attempt, reservation)?;
        run_set.insert_initial(run)?;
        Ok(run_set)
    }

    pub(crate) fn from_non_step_canonical_value_at_revision(
        value: &CborValue,
        attempt: &ExecutionAttemptV1,
        initial_revision: u64,
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        if matches!(attempt, ExecutionAttemptV1::Step(_)) {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let CborValue::Array(fields) = value else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let [
            CborValue::Text(domain),
            owner,
            CborValue::Unsigned(max_runs),
            CborValue::Unsigned(revision),
            CborValue::Array(runs),
        ] = fields.as_slice()
        else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let owner = parse_attempt_owner(owner)?;
        let expected_owner = attempt.owner();
        let max_runs = usize::try_from(*max_runs)
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
        if domain != "maestro.vnext.run-set.v1"
            || owner != expected_owner
            || max_runs != MAX_RUNS_PER_ATTEMPT_V1
            || runs.is_empty()
            || runs.len() > max_runs
            || initial_revision == 0
            || *revision <= initial_revision
        {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let mut parsed_runs = Vec::with_capacity(runs.len());
        let mut expected_revision = initial_revision;
        for value in runs {
            let CborValue::Array(run) = value else {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            };
            let [
                run_id,
                run_owner,
                semantic,
                inputs,
                environment,
                target,
                boundary,
                CborValue::Unsigned(deadline),
                CborValue::Unsigned(launch_ordinal),
                current_term,
                CborValue::Unsigned(state),
                CborValue::Array(segments),
            ] = run.as_slice()
            else {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            };
            let launch_ordinal = u32::try_from(*launch_ordinal)
                .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            if parse_attempt_owner(run_owner)? != expected_owner
                || parse_optional_runtime_digest(current_term)?.is_some()
            {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            }
            let reservation = RunReservationV1 {
                semantic_operation_hash: exact_runtime_digest(semantic)?,
                inputs_commitment: exact_runtime_digest(inputs)?,
                environment_commitment: exact_runtime_digest(environment)?,
                target_commitment: exact_runtime_digest(target)?,
                execution_boundary_commitment: exact_runtime_digest(boundary)?,
                deadline: *deadline,
                launch_ordinal,
                current_step_term: None,
            };
            let mut rebuilt = RunV1::reserve(attempt, reservation)?;
            if rebuilt.id() != RunIdV1::from_bytes(exact_runtime_digest(run_id)?)? {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            }
            expected_revision = expected_revision
                .checked_add(1)
                .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            let target_state = parse_run_state(*state)?;
            let active_path = !segments.is_empty()
                || matches!(
                    target_state,
                    RunStateV1::Active | RunStateV1::Succeeded | RunStateV1::Failed
                );
            if active_path {
                rebuilt.transition(RunStateV1::Active)?;
                expected_revision = expected_revision
                    .checked_add(1)
                    .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            }
            for (index, segment) in segments.iter().enumerate() {
                let CborValue::Array(segment) = segment else {
                    return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
                };
                let [CborValue::Unsigned(ordinal), process_or_job, commitment] = segment.as_slice()
                else {
                    return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
                };
                let expected_ordinal = u64::try_from(index)
                    .ok()
                    .and_then(|value| value.checked_add(1))
                    .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
                if *ordinal != expected_ordinal {
                    return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
                }
                rebuilt.append_segment(
                    exact_runtime_digest(process_or_job)?,
                    exact_runtime_digest(commitment)?,
                )?;
                expected_revision = expected_revision
                    .checked_add(1)
                    .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            }
            if target_state != rebuilt.state() {
                rebuilt.transition(target_state)?;
                expected_revision = expected_revision
                    .checked_add(1)
                    .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            }
            parsed_runs.push(rebuilt);
        }
        validate_run_launch_chains(&parsed_runs)?;
        if expected_revision != *revision {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        Ok(Self {
            owner,
            max_runs,
            revision: *revision,
            runs: parsed_runs,
        })
    }

    pub(crate) fn transition_non_step_run(
        &mut self,
        run_id: RunIdV1,
        expected_revision: u64,
        next: RunStateV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.transition(run_id, expected_revision, next)
    }

    pub(crate) fn close_non_step_runs(
        &mut self,
        expected_revision: u64,
        terminal: RunStateV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        self.close_open(expected_revision, terminal)
    }

    pub const fn owner(&self) -> ExecutionAttemptOwnerV1 {
        self.owner
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn runs(&self) -> &[RunV1] {
        &self.runs
    }

    fn insert_initial(&mut self, run: RunV1) -> Result<(), ExecutionRuntimeErrorV1> {
        if run.owner() != self.owner
            || self.runs.len() >= self.max_runs
            || self.runs.iter().any(|current| current.id() == run.id())
            || run.launch_ordinal != 1
            || self
                .runs
                .iter()
                .any(|current| current.semantic_operation_hash == run.semantic_operation_hash)
        {
            return Err(ExecutionRuntimeErrorV1::InvalidRunLaunchChain);
        }
        self.runs.push(run);
        self.bump_revision()
    }

    fn insert_retry(
        &mut self,
        predecessor_run_id: RunIdV1,
        run: RunV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        let predecessor = self
            .runs
            .iter()
            .find(|candidate| candidate.id() == predecessor_run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        let expected_ordinal = predecessor
            .launch_ordinal
            .checked_add(1)
            .ok_or(ExecutionRuntimeErrorV1::RunLaunchOrdinalOverflow)?;
        let latest_ordinal = self
            .runs
            .iter()
            .filter(|candidate| {
                candidate.semantic_operation_hash == predecessor.semantic_operation_hash
            })
            .map(|candidate| candidate.launch_ordinal)
            .max()
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        if predecessor.state != RunStateV1::DefinitelyNotStarted
            || run.owner() != self.owner
            || self.runs.len() >= self.max_runs
            || self.runs.iter().any(|current| current.id() == run.id())
            || run.semantic_operation_hash != predecessor.semantic_operation_hash
            || run.inputs_commitment != predecessor.inputs_commitment
            || run.environment_commitment != predecessor.environment_commitment
            || run.target_commitment != predecessor.target_commitment
            || run.execution_boundary_commitment != predecessor.execution_boundary_commitment
            || run.launch_ordinal != expected_ordinal
            || latest_ordinal != predecessor.launch_ordinal
        {
            return Err(ExecutionRuntimeErrorV1::InvalidRunLaunchChain);
        }
        self.runs.push(run);
        self.bump_revision()
    }

    fn transition(
        &mut self,
        run_id: RunIdV1,
        expected_revision: u64,
        next: RunStateV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        if self.revision != expected_revision {
            return Err(ExecutionRuntimeErrorV1::StaleRunSetRevision);
        }
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.id() == run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        run.transition(next)?;
        self.bump_revision()
    }

    fn append_segment(
        &mut self,
        run_id: RunIdV1,
        expected_revision: u64,
        process_or_job_identity: [u8; 32],
        segment_commitment: [u8; 32],
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        if self.revision != expected_revision {
            return Err(ExecutionRuntimeErrorV1::StaleRunSetRevision);
        }
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.id() == run_id)
            .ok_or(ExecutionRuntimeErrorV1::UnknownRun)?;
        run.append_segment(process_or_job_identity, segment_commitment)?;
        self.bump_revision()
    }

    fn close_open(
        &mut self,
        expected_revision: u64,
        terminal: RunStateV1,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        if self.revision != expected_revision || !terminal.is_terminal() {
            return Err(ExecutionRuntimeErrorV1::StaleRunSetRevision);
        }
        for run in &mut self.runs {
            if matches!(run.state(), RunStateV1::Reserved | RunStateV1::Active) {
                run.transition(terminal)?;
            }
        }
        self.bump_revision()
    }

    pub fn is_submission_quiescent(&self) -> bool {
        self.runs
            .iter()
            .all(|run| run.state().is_submission_certain())
    }

    pub fn all_terminal(&self) -> bool {
        self.runs.iter().all(|run| run.state().is_terminal())
    }

    pub fn canonical_value(&self) -> Result<CborValue, ExecutionRuntimeErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.run-set.v1")?,
            self.owner.canonical_value(),
            CborValue::Unsigned(
                u64::try_from(self.max_runs)
                    .map_err(|_| ExecutionRuntimeErrorV1::InvalidRunLimit)?,
            ),
            CborValue::Unsigned(self.revision),
            CborValue::Array(
                self.runs
                    .iter()
                    .map(|run| {
                        CborValue::Array(vec![
                            bytes(run.id.as_bytes()),
                            run.owner.canonical_value(),
                            bytes(&run.semantic_operation_hash),
                            bytes(&run.inputs_commitment),
                            bytes(&run.environment_commitment),
                            bytes(&run.target_commitment),
                            bytes(&run.execution_boundary_commitment),
                            CborValue::Unsigned(run.deadline),
                            CborValue::Unsigned(u64::from(run.launch_ordinal)),
                            CborValue::optional(
                                run.current_step_term.map(|term| bytes(term.as_bytes())),
                            ),
                            CborValue::Unsigned(run_state_tag(run.state)),
                            CborValue::Array(
                                run.segments
                                    .iter()
                                    .map(|segment| {
                                        CborValue::Array(vec![
                                            CborValue::Unsigned(segment.ordinal),
                                            bytes(&segment.process_or_job_identity),
                                            bytes(&segment.segment_commitment),
                                        ])
                                    })
                                    .collect(),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ]))
    }

    fn bump_revision(&mut self) -> Result<(), ExecutionRuntimeErrorV1> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(ExecutionRuntimeErrorV1::RunSetRevisionOverflow)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TakeoverSafetyMechanismV1 {
    PinnedQuiescenceEvidence,
    RevocableSandboxFence,
    AttemptIsolatedResources,
}

impl TakeoverSafetyMechanismV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::PinnedQuiescenceEvidence => 1,
            Self::RevocableSandboxFence => 2,
            Self::AttemptIsolatedResources => 3,
        }
    }

    #[cfg(test)]
    fn from_tag(tag: u64) -> Result<Self, ExecutionRuntimeErrorV1> {
        match tag {
            1 => Ok(Self::PinnedQuiescenceEvidence),
            2 => Ok(Self::RevocableSandboxFence),
            3 => Ok(Self::AttemptIsolatedResources),
            _ => Err(ExecutionRuntimeErrorV1::TakeoverSafetyUnknown),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeoverSafetyV1 {
    mechanism: TakeoverSafetyMechanismV1,
    binding_commitment: [u8; 32],
    predecessor_attempt_id: StepAttemptIdV1,
    predecessor_lease_id: StepLeaseIdV1,
    predecessor_term_id: LeaseTermIdV1,
    predecessor_fence: u64,
    successor_fence: u64,
    valid_at: u64,
    owner_receipt_commitment: [u8; 32],
    proof_commitment: [u8; 32],
}

impl TakeoverSafetyV1 {
    #[cfg(test)]
    fn from_validated_owner_receipt(
        predecessor: &StepExecutionCarrierV1,
        binding: StepBindingV1,
        successor_fence: u64,
        valid_at: u64,
        mechanism: TakeoverSafetyMechanismV1,
        owner_receipt_commitment: [u8; 32],
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        require_nonzero(owner_receipt_commitment)?;
        let attempt = predecessor.tenure().attempt();
        let lease = predecessor.tenure().lease();
        let term = predecessor.tenure().current_term();
        if attempt.binding() != binding
            || lease.binding() != binding
            || attempt.id() != lease.attempt_id()
            || attempt.lease_id() != lease.id()
            || attempt.fence() != lease.fence()
            || term.id() != lease.current_term_id()
            || successor_fence
                != attempt
                    .fence()
                    .checked_add(1)
                    .ok_or(ExecutionRuntimeErrorV1::InvalidFence)?
        {
            return Err(ExecutionRuntimeErrorV1::TakeoverSafetyUnknown);
        }
        let mut proof = Self {
            mechanism,
            binding_commitment: step_binding_commitment(binding)?,
            predecessor_attempt_id: attempt.id(),
            predecessor_lease_id: lease.id(),
            predecessor_term_id: term.id(),
            predecessor_fence: attempt.fence(),
            successor_fence,
            valid_at,
            owner_receipt_commitment,
            proof_commitment: [0; 32],
        };
        proof.proof_commitment = proof.compute_commitment()?;
        Ok(proof)
    }

    #[cfg(test)]
    pub(crate) fn from_owner_receipt(
        predecessor: &StepExecutionCarrierV1,
        binding: StepBindingV1,
        successor_fence: u64,
        valid_at: u64,
        mechanism: TakeoverSafetyMechanismV1,
        owner_receipt_commitment: [u8; 32],
    ) -> Result<Self, ExecutionRuntimeErrorV1> {
        Self::from_validated_owner_receipt(
            predecessor,
            binding,
            successor_fence,
            valid_at,
            mechanism,
            owner_receipt_commitment,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, ExecutionRuntimeErrorV1> {
        let CborValue::Array(fields) = value else {
            return Err(ExecutionRuntimeErrorV1::TakeoverSafetyUnknown);
        };
        let [
            CborValue::Unsigned(mechanism),
            binding,
            predecessor_attempt,
            predecessor_lease,
            predecessor_term,
            CborValue::Unsigned(predecessor_fence),
            CborValue::Unsigned(successor_fence),
            CborValue::Unsigned(valid_at),
            owner_receipt,
            proof_commitment,
        ] = fields.as_slice()
        else {
            return Err(ExecutionRuntimeErrorV1::TakeoverSafetyUnknown);
        };
        let proof = Self {
            mechanism: TakeoverSafetyMechanismV1::from_tag(*mechanism)?,
            binding_commitment: exact_runtime_digest(binding)?,
            predecessor_attempt_id: StepAttemptIdV1::from_bytes(exact_runtime_digest(
                predecessor_attempt,
            )?)?,
            predecessor_lease_id: StepLeaseIdV1::from_bytes(exact_runtime_digest(
                predecessor_lease,
            )?)?,
            predecessor_term_id: LeaseTermIdV1::from_bytes(exact_runtime_digest(
                predecessor_term,
            )?)?,
            predecessor_fence: *predecessor_fence,
            successor_fence: *successor_fence,
            valid_at: *valid_at,
            owner_receipt_commitment: exact_runtime_digest(owner_receipt)?,
            proof_commitment: exact_runtime_digest(proof_commitment)?,
        };
        if proof.predecessor_fence == 0
            || proof.successor_fence == 0
            || proof.owner_receipt_commitment == [0; 32]
            || proof.proof_commitment != proof.compute_commitment()?
            || proof.canonical_value() != *value
        {
            return Err(ExecutionRuntimeErrorV1::TakeoverSafetyUnknown);
        }
        Ok(proof)
    }

    pub(crate) fn validate(
        &self,
        predecessor: &StepExecutionCarrierV1,
        binding: StepBindingV1,
        successor_fence: u64,
        valid_at: u64,
    ) -> Result<(), ExecutionRuntimeErrorV1> {
        let attempt = predecessor.tenure().attempt();
        let lease = predecessor.tenure().lease();
        let term = predecessor.tenure().current_term();
        if self.binding_commitment != step_binding_commitment(binding)?
            || attempt.binding() != binding
            || lease.binding() != binding
            || self.predecessor_attempt_id != attempt.id()
            || self.predecessor_lease_id != lease.id()
            || self.predecessor_term_id != term.id()
            || self.predecessor_fence != attempt.fence()
            || self.successor_fence != successor_fence
            || self.valid_at != valid_at
            || self.owner_receipt_commitment == [0; 32]
            || self.proof_commitment != self.compute_commitment()?
            || (self.mechanism == TakeoverSafetyMechanismV1::PinnedQuiescenceEvidence
                && !predecessor.run_set().all_terminal())
        {
            return Err(ExecutionRuntimeErrorV1::TakeoverSafetyUnknown);
        }
        Ok(())
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.mechanism.tag()),
            bytes(&self.binding_commitment),
            bytes(self.predecessor_attempt_id.as_bytes()),
            bytes(self.predecessor_lease_id.as_bytes()),
            bytes(self.predecessor_term_id.as_bytes()),
            CborValue::Unsigned(self.predecessor_fence),
            CborValue::Unsigned(self.successor_fence),
            CborValue::Unsigned(self.valid_at),
            bytes(&self.owner_receipt_commitment),
            bytes(&self.proof_commitment),
        ])
    }

    fn compute_commitment(&self) -> Result<[u8; 32], ExecutionRuntimeErrorV1> {
        Ok(
            Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
                CborValue::text("maestro.vnext.step-takeover-safety-owner-receipt.v1")?,
                CborValue::Unsigned(self.mechanism.tag()),
                bytes(&self.binding_commitment),
                bytes(self.predecessor_attempt_id.as_bytes()),
                bytes(self.predecessor_lease_id.as_bytes()),
                bytes(self.predecessor_term_id.as_bytes()),
                CborValue::Unsigned(self.predecessor_fence),
                CborValue::Unsigned(self.successor_fence),
                CborValue::Unsigned(self.valid_at),
                bytes(&self.owner_receipt_commitment),
            ]))?)
            .into(),
        )
    }
}

fn parse_step_execution_tenure(
    value: &CborValue,
) -> Result<StepExecutionTenureV1, ExecutionRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let [
        CborValue::Text(domain),
        CborValue::Array(lease),
        CborValue::Array(attempt),
        CborValue::Array(terms),
    ] = fields.as_slice()
    else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    if domain != "maestro.vnext.step-execution-tenure.v1" || terms.is_empty() {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    let [
        lease_id,
        lease_attempt_id,
        lease_binding,
        CborValue::Unsigned(lease_fence),
        current_term_id,
        lease_state,
    ] = lease.as_slice()
    else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let [
        attempt_id,
        attempt_lease_id,
        attempt_binding,
        CborValue::Unsigned(attempt_fence),
        executor,
        store_generation,
        CborValue::Unsigned(authority_epoch),
        envelope,
        CborValue::Unsigned(run_limit),
        attempt_state,
    ] = attempt.as_slice()
    else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let lease_id = StepLeaseIdV1::from_bytes(exact_runtime_digest(lease_id)?)?;
    let lease_attempt_id = StepAttemptIdV1::from_bytes(exact_runtime_digest(lease_attempt_id)?)?;
    let attempt_id = StepAttemptIdV1::from_bytes(exact_runtime_digest(attempt_id)?)?;
    let attempt_lease_id = StepLeaseIdV1::from_bytes(exact_runtime_digest(attempt_lease_id)?)?;
    let binding = parse_step_binding(lease_binding)?;
    let parsed_attempt_binding = parse_step_binding(attempt_binding)?;
    let lease_state = parse_step_attempt_state(lease_state)?;
    let attempt_state = parse_step_attempt_state(attempt_state)?;
    let current_term_id = LeaseTermIdV1::from_bytes(exact_runtime_digest(current_term_id)?)?;
    let executor = PrincipalIdV1::from_digest(exact_runtime_digest(executor)?);
    let store_generation_id =
        StoreGenerationIdV1::from_digest(exact_runtime_digest(store_generation)?);
    let fixed_envelope_commitment = exact_runtime_digest(envelope)?;
    let run_limit = u32::try_from(*run_limit)
        .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
    if binding != parsed_attempt_binding
        || lease_id != attempt_lease_id
        || lease_attempt_id != attempt_id
        || lease_fence != attempt_fence
        || lease_state != attempt_state
    {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    let mut parsed_terms = Vec::with_capacity(terms.len());
    for term in terms {
        let CborValue::Array(term) = term else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let [
            term_id,
            term_lease_id,
            term_attempt_id,
            CborValue::Unsigned(fence),
            CborValue::Unsigned(ordinal),
            prior,
            CborValue::Unsigned(issued_at),
            CborValue::Unsigned(expires_at),
            CborValue::Unsigned(hard_deadline),
            request_id,
        ] = term.as_slice()
        else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let prior_term_id = parse_optional_runtime_digest(prior)?
            .map(LeaseTermIdV1::from_bytes)
            .transpose()?;
        let action_request_id = ActionRequestIdV1::from_digest(exact_runtime_digest(request_id)?);
        let rebuilt = new_term(LeaseTermPartsV1 {
            lease_id: StepLeaseIdV1::from_bytes(exact_runtime_digest(term_lease_id)?)?,
            attempt_id: StepAttemptIdV1::from_bytes(exact_runtime_digest(term_attempt_id)?)?,
            fence: *fence,
            ordinal: *ordinal,
            prior_term_id,
            issued_at: *issued_at,
            expires_at: *expires_at,
            hard_deadline: *hard_deadline,
            action_request_id,
        })?;
        if rebuilt.id() != LeaseTermIdV1::from_bytes(exact_runtime_digest(term_id)?)? {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        parsed_terms.push(rebuilt);
    }
    let initial_request = parsed_terms
        .first()
        .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?
        .action_request_id();
    let binding_commitment = step_binding_commitment(binding)?;
    let pair_commitment = CborValue::Array(vec![
        bytes(&binding_commitment),
        CborValue::Unsigned(*attempt_fence),
        bytes(executor.as_bytes()),
        bytes(store_generation_id.as_bytes()),
        CborValue::Unsigned(*authority_epoch),
        bytes(&fixed_envelope_commitment),
        CborValue::Unsigned(u64::from(run_limit)),
        bytes(initial_request.as_bytes()),
    ]);
    let expected_lease = StepLeaseIdV1::from_value(&CborValue::Array(vec![
        CborValue::text("lease")?,
        pair_commitment.clone(),
    ]))?;
    let expected_attempt = StepAttemptIdV1::from_value(&CborValue::Array(vec![
        CborValue::text("attempt")?,
        pair_commitment,
        bytes(expected_lease.as_bytes()),
    ]))?;
    if lease_id != expected_lease || attempt_id != expected_attempt {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    let tenure = StepExecutionTenureV1 {
        lease: StepLeaseV1 {
            id: lease_id,
            attempt_id,
            binding,
            fence: *lease_fence,
            current_term_id,
            state: lease_state,
        },
        attempt: StepAttemptV1 {
            id: attempt_id,
            lease_id,
            binding,
            fence: *attempt_fence,
            executor,
            store_generation_id,
            authority_epoch: *authority_epoch,
            fixed_envelope_commitment,
            run_limit,
            state: attempt_state,
        },
        terms: parsed_terms,
    };
    tenure.validate_pair()?;
    Ok(tenure)
}

fn parse_step_run_set(
    value: &CborValue,
    tenure: &StepExecutionTenureV1,
) -> Result<RunSetV1, ExecutionRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let [
        CborValue::Text(domain),
        owner,
        CborValue::Unsigned(max_runs),
        CborValue::Unsigned(revision),
        CborValue::Array(runs),
    ] = fields.as_slice()
    else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    if domain != "maestro.vnext.run-set.v1" || *revision == 0 {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    let owner = parse_attempt_owner(owner)?;
    let expected_owner = ExecutionAttemptOwnerV1::Step(tenure.attempt().id());
    let max_runs = usize::try_from(*max_runs)
        .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
    if owner != expected_owner
        || max_runs != tenure.attempt().run_limit() as usize
        || runs.len() > max_runs
    {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    let mut reconstruction_attempt = tenure.attempt().clone();
    reconstruction_attempt.state = StepAttemptStateV1::Live;
    let attempt = ExecutionAttemptV1::Step(Box::new(reconstruction_attempt));
    let mut parsed_runs = Vec::with_capacity(runs.len());
    for value in runs {
        let CborValue::Array(run) = value else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let [
            run_id,
            run_owner,
            semantic,
            inputs,
            environment,
            target,
            boundary,
            CborValue::Unsigned(deadline),
            CborValue::Unsigned(launch_ordinal),
            current_term,
            CborValue::Unsigned(state),
            CborValue::Array(segments),
        ] = run.as_slice()
        else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let run_owner = parse_attempt_owner(run_owner)?;
        let launch_ordinal = u32::try_from(*launch_ordinal)
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
        let current_step_term = parse_optional_runtime_digest(current_term)?
            .map(LeaseTermIdV1::from_bytes)
            .transpose()?;
        let Some(current_step_term_id) = current_step_term else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        let Some(bound_term) = tenure
            .terms()
            .iter()
            .find(|term| term.id() == current_step_term_id)
        else {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        };
        if *deadline <= bound_term.issued_at()
            || *deadline > bound_term.expires_at()
            || *deadline > bound_term.hard_deadline()
        {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let reservation = RunReservationV1 {
            semantic_operation_hash: exact_runtime_digest(semantic)?,
            inputs_commitment: exact_runtime_digest(inputs)?,
            environment_commitment: exact_runtime_digest(environment)?,
            target_commitment: exact_runtime_digest(target)?,
            execution_boundary_commitment: exact_runtime_digest(boundary)?,
            deadline: *deadline,
            launch_ordinal,
            current_step_term,
        };
        if reservation.fixed_envelope_commitment()? != tenure.attempt().fixed_envelope_commitment {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let mut rebuilt = RunV1::reserve(&attempt, reservation)?;
        if rebuilt.id() != RunIdV1::from_bytes(exact_runtime_digest(run_id)?)?
            || run_owner != expected_owner
        {
            return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
        }
        let target_state = parse_run_state(*state)?;
        let active_path = !segments.is_empty()
            || matches!(
                target_state,
                RunStateV1::Active | RunStateV1::Succeeded | RunStateV1::Failed
            );
        if active_path {
            rebuilt.transition(RunStateV1::Active)?;
        }
        for (index, segment) in segments.iter().enumerate() {
            let CborValue::Array(segment) = segment else {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            };
            let [CborValue::Unsigned(ordinal), process_or_job, commitment] = segment.as_slice()
            else {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            };
            let expected_ordinal = u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            if *ordinal != expected_ordinal {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            }
            rebuilt.append_segment(
                exact_runtime_digest(process_or_job)?,
                exact_runtime_digest(commitment)?,
            )?;
        }
        if target_state != rebuilt.state() {
            rebuilt.transition(target_state)?;
        }
        parsed_runs.push(rebuilt);
    }
    validate_run_launch_chains(&parsed_runs)?;
    if !tenure.attempt().is_live() && parsed_runs.iter().any(|run| !run.state().is_terminal()) {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    Ok(RunSetV1 {
        owner,
        max_runs,
        revision: *revision,
        runs: parsed_runs,
    })
}

fn validate_run_launch_chains(runs: &[RunV1]) -> Result<(), ExecutionRuntimeErrorV1> {
    let mut chains = std::collections::BTreeMap::<[u8; 32], Vec<&RunV1>>::new();
    for run in runs {
        chains
            .entry(run.semantic_operation_hash)
            .or_default()
            .push(run);
    }
    for chain in chains.values_mut() {
        chain.sort_unstable_by_key(|run| run.launch_ordinal);
        for (index, run) in chain.iter().enumerate() {
            let expected_ordinal = u32::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?;
            if run.launch_ordinal != expected_ordinal {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            }
            let Some(predecessor_index) = index.checked_sub(1) else {
                continue;
            };
            let predecessor = chain[predecessor_index];
            if predecessor.state != RunStateV1::DefinitelyNotStarted
                || predecessor.inputs_commitment != run.inputs_commitment
                || predecessor.environment_commitment != run.environment_commitment
                || predecessor.target_commitment != run.target_commitment
                || predecessor.execution_boundary_commitment != run.execution_boundary_commitment
            {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            }
        }
    }
    let unique = runs
        .iter()
        .map(|run| run.id())
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != runs.len() {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    }
    Ok(())
}

pub(crate) fn parse_step_binding(
    value: &CborValue,
) -> Result<StepBindingV1, ExecutionRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let [repository, work, generation, root, step, revision] = fields.as_slice() else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let scope = StepScopeV1::new(
        StoreDomainIdV1::from_digest(exact_runtime_digest(repository)?),
        WorkIdV1::parse(&render_digest(exact_runtime_digest(work)?))
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?,
    );
    StepBindingV1::new(
        scope,
        ContractGenerationIdV1::parse(&render_digest(exact_runtime_digest(generation)?))
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?,
        ContractRootIdV1::from_digest(exact_runtime_digest(root)?),
        StepIdV1::from_bytes(scope, exact_runtime_digest(step)?)
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?,
        StepRevisionIdV1::from_bytes(exact_runtime_digest(revision)?)
            .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)?,
    )
    .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)
}

fn parse_step_attempt_state(
    value: &CborValue,
) -> Result<StepAttemptStateV1, ExecutionRuntimeErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(1)] => {
            Ok(StepAttemptStateV1::Live)
        }
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(2), CborValue::Unsigned(tag)] = fields.as_slice() else {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            };
            let terminal = match tag {
                1 => StepAttemptTerminalV1::Submitted,
                2 => StepAttemptTerminalV1::Yielded,
                3 => StepAttemptTerminalV1::Failed,
                4 => StepAttemptTerminalV1::Cancelled,
                5 => StepAttemptTerminalV1::TimedOut,
                6 => StepAttemptTerminalV1::Lost,
                7 => StepAttemptTerminalV1::Fenced,
                _ => return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier),
            };
            Ok(StepAttemptStateV1::Terminal(terminal))
        }
        _ => Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier),
    }
}

pub(crate) fn parse_attempt_owner(
    value: &CborValue,
) -> Result<ExecutionAttemptOwnerV1, ExecutionRuntimeErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    let [CborValue::Unsigned(tag), identity] = fields.as_slice() else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    match tag {
        1 => Ok(ExecutionAttemptOwnerV1::Step(StepAttemptIdV1::from_bytes(
            exact_runtime_digest(identity)?,
        )?)),
        2 => Ok(ExecutionAttemptOwnerV1::Dispatch(
            DispatchAttemptIdV1::from_bytes(exact_runtime_digest(identity)?)?,
        )),
        3 => Ok(ExecutionAttemptOwnerV1::Reconciliation(
            ReconciliationAttemptIdV1::from_bytes(exact_runtime_digest(identity)?)?,
        )),
        _ => Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier),
    }
}

fn parse_run_state(tag: u64) -> Result<RunStateV1, ExecutionRuntimeErrorV1> {
    match tag {
        1 => Ok(RunStateV1::Reserved),
        2 => Ok(RunStateV1::Active),
        3 => Ok(RunStateV1::DefinitelyNotStarted),
        4 => Ok(RunStateV1::Succeeded),
        5 => Ok(RunStateV1::Failed),
        6 => Ok(RunStateV1::Cancelled),
        7 => Ok(RunStateV1::TimedOut),
        8 => Ok(RunStateV1::Lost),
        9 => Ok(RunStateV1::Fenced),
        _ => Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier),
    }
}

fn parse_optional_runtime_digest(
    value: &CborValue,
) -> Result<Option<[u8; 32]>, ExecutionRuntimeErrorV1> {
    match value {
        CborValue::Array(fields) if fields.as_slice() == [CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => {
            let [CborValue::Unsigned(1), value] = fields.as_slice() else {
                return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
            };
            Ok(Some(exact_runtime_digest(value)?))
        }
        _ => Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier),
    }
}

pub(crate) fn exact_runtime_digest(value: &CborValue) -> Result<[u8; 32], ExecutionRuntimeErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LeaseTermPartsV1 {
    lease_id: StepLeaseIdV1,
    attempt_id: StepAttemptIdV1,
    fence: u64,
    ordinal: u64,
    prior_term_id: Option<LeaseTermIdV1>,
    issued_at: u64,
    expires_at: u64,
    hard_deadline: u64,
    action_request_id: ActionRequestIdV1,
}

fn new_term(parts: LeaseTermPartsV1) -> Result<LeaseTermV1, ExecutionRuntimeErrorV1> {
    let LeaseTermPartsV1 {
        lease_id,
        attempt_id,
        fence,
        ordinal,
        prior_term_id,
        issued_at,
        expires_at,
        hard_deadline,
        action_request_id,
    } = parts;
    if ordinal == 0 || fence == 0 || issued_at >= expires_at || expires_at > hard_deadline {
        return Err(ExecutionRuntimeErrorV1::InvalidLeaseTime);
    }
    let value = CborValue::Array(vec![
        bytes(lease_id.as_bytes()),
        bytes(attempt_id.as_bytes()),
        CborValue::Unsigned(fence),
        CborValue::Unsigned(ordinal),
        CborValue::optional(prior_term_id.map(|term| bytes(term.as_bytes()))),
        CborValue::Unsigned(issued_at),
        CborValue::Unsigned(expires_at),
        CborValue::Unsigned(hard_deadline),
        bytes(action_request_id.as_bytes()),
    ]);
    Ok(LeaseTermV1 {
        id: LeaseTermIdV1::from_value(&value)?,
        lease_id,
        attempt_id,
        fence,
        ordinal,
        prior_term_id,
        issued_at,
        expires_at,
        hard_deadline,
        action_request_id,
    })
}

fn validate_fence_and_time(
    fence: u64,
    authority_epoch: u64,
    issued_at: u64,
    expires_at: u64,
    hard_deadline: u64,
) -> Result<(), ExecutionRuntimeErrorV1> {
    if fence == 0 || authority_epoch == 0 {
        return Err(ExecutionRuntimeErrorV1::InvalidFence);
    }
    if issued_at >= expires_at || expires_at > hard_deadline {
        return Err(ExecutionRuntimeErrorV1::InvalidLeaseTime);
    }
    Ok(())
}

fn step_binding_commitment(binding: StepBindingV1) -> Result<[u8; 32], ExecutionRuntimeErrorV1> {
    hash(&step_binding_value(binding)).map_err(Into::into)
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

fn step_attempt_state_value(state: StepAttemptStateV1) -> CborValue {
    match state {
        StepAttemptStateV1::Live => CborValue::Array(vec![CborValue::Unsigned(1)]),
        StepAttemptStateV1::Terminal(terminal) => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Unsigned(match terminal {
                StepAttemptTerminalV1::Submitted => 1,
                StepAttemptTerminalV1::Yielded => 2,
                StepAttemptTerminalV1::Failed => 3,
                StepAttemptTerminalV1::Cancelled => 4,
                StepAttemptTerminalV1::TimedOut => 5,
                StepAttemptTerminalV1::Lost => 6,
                StepAttemptTerminalV1::Fenced => 7,
            }),
        ]),
    }
}

fn run_state_tag(state: RunStateV1) -> u64 {
    match state {
        RunStateV1::Reserved => 1,
        RunStateV1::Active => 2,
        RunStateV1::DefinitelyNotStarted => 3,
        RunStateV1::Succeeded => 4,
        RunStateV1::Failed => 5,
        RunStateV1::Cancelled => 6,
        RunStateV1::TimedOut => 7,
        RunStateV1::Lost => 8,
        RunStateV1::Fenced => 9,
    }
}

fn execution_action_request_value(
    action: ExecutionActionV1,
    subject_commitment: [u8; 32],
    expected_state_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    idempotency_key_id: IdempotencyKeyIdV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.execution-action-request.v1")?,
        CborValue::Unsigned(action.global_tag()),
        CborValue::Unsigned(action.local_tag()),
        CborValue::text(action.literal())?,
        CborValue::text(action.descriptor_id())?,
        bytes(&subject_commitment),
        bytes(&expected_state_commitment),
        bytes(&payload_commitment),
        bytes(idempotency_key_id.as_bytes()),
    ]))
}

fn require_nonzero(commitment: [u8; 32]) -> Result<(), ExecutionRuntimeErrorV1> {
    if commitment == [0; 32] {
        Err(ExecutionRuntimeErrorV1::MissingCommitment)
    } else {
        Ok(())
    }
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
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

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExecutionRuntimeErrorV1 {
    #[error("Execution identity seed must be non-empty bounded ASCII")]
    InvalidIdentitySeed,
    #[error("Execution commitment must not be the all-zero missing value")]
    MissingCommitment,
    #[error("authorization Receipt does not bind the Action Request")]
    AuthorizationRequestMismatch,
    #[error("the authorized Action is not valid for this Execution transition")]
    WrongExecutionAction,
    #[error("Execution fence and Authority Epoch must be positive")]
    InvalidFence,
    #[error("Lease issue, expiry, and hard-deadline ordering is invalid")]
    InvalidLeaseTime,
    #[error("StepAttempt Run limit is invalid")]
    InvalidRunLimit,
    #[error("StepLease and StepAttempt are not a coherent 1:1 pair")]
    BrokenLeaseAttemptPair,
    #[error("stored Step execution carrier is malformed or violates the closed Execution model")]
    InvalidStoredExecutionCarrier,
    #[error("LeaseTerm is stale")]
    StaleLeaseTerm,
    #[error("LeaseTerm ordinal overflow")]
    LeaseTermOverflow,
    #[error("terminal StepAttempt cannot be renewed or reopened")]
    TerminalAttempt,
    #[error("Execution cannot own or mutate the Step-owned Submission")]
    SubmissionOwnedByStep,
    #[error("Step Submission execution fence is unavailable")]
    SubmissionFenceUnavailable,
    #[error("Run reservation is invalid")]
    InvalidRunReservation,
    #[error("Step-owned Run requires one current live LeaseTerm")]
    StepRunRequiresLiveTerm,
    #[error("Dispatch or reconciliation Run must not carry a LeaseTerm")]
    NonStepRunHasLeaseTerm,
    #[error("Run transition is outside the closed state graph")]
    IllegalRunTransition,
    #[error("same-Attempt retry requires conclusive definitely_not_started")]
    RunRetryBoundaryUnknown,
    #[error("definitely_not_started requires a validated execution-boundary no-start Receipt")]
    RunNoStartProofRequired,
    #[error("Run launch ordinal overflow")]
    RunLaunchOrdinalOverflow,
    #[error("Run segment is unavailable")]
    RunSegmentUnavailable,
    #[error("Run segment does not continue the same proven process or provider job")]
    RunIdentityChanged,
    #[error("Run owner, uniqueness, or finite cardinality is invalid")]
    RunOwnerOrCardinalityMismatch,
    #[error("Run launch chain must start at one and advance only from definitely_not_started")]
    InvalidRunLaunchChain,
    #[error("StepAttempt Run budget is exhausted")]
    RunBudgetExhausted,
    #[error("Run deadline is outside the current LeaseTerm")]
    RunDeadlineOutsideTerm,
    #[error("Run deadline has expired for activation or segment append")]
    RunDeadlineExpired,
    #[error("Run cannot time out before its exact deadline")]
    RunDeadlineNotReached,
    #[error("Run immutable fields do not match the StepAttempt fixed execution envelope")]
    RunOutsideFixedEnvelope,
    #[error("Run or Lease expected-current basis is stale")]
    StaleRunOrLeaseBasis,
    #[error("terminal StepAttempt has an open Run")]
    OpenRunsAtAttemptTerminal,
    #[error("Run-set revision is stale")]
    StaleRunSetRevision,
    #[error("Run-set revision overflow")]
    RunSetRevisionOverflow,
    #[error("Run is not present in the exact owner set")]
    UnknownRun,
    #[error("takeover is blocked because physical overlap is unknown")]
    TakeoverSafetyUnknown,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::domain::vnext::authority::{
        ActionAuthorityBasisKindV1, AuthorityContextIdV1, StateTokenIdV1,
    };
    use crate::domain::vnext::contract::runtime::ContractGenerationIdV1;
    use crate::domain::vnext::identity::{ContractRootIdV1, StoreDomainIdV1};
    use crate::domain::vnext::step::{StepIdV1, StepRevisionIdV1, StepScopeV1};
    use crate::domain::vnext::work::WorkIdV1;

    fn token(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn binding() -> StepBindingV1 {
        let repository = StoreDomainIdV1::from_digest(token(1));
        let work = WorkIdV1::derive("work").unwrap();
        let scope = StepScopeV1::new(repository, work);
        StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest(token(3))).unwrap(),
            ContractRootIdV1::from_digest(token(4)),
            StepIdV1::from_bytes(scope, token(5)).unwrap(),
            StepRevisionIdV1::from_bytes(token(6)).unwrap(),
        )
        .unwrap()
    }

    fn authority(action: ExecutionActionV1, seed: &str) -> AuthorizedExecutionActionV1 {
        let request = CanonicalExecutionActionRequestV1::new(
            action,
            token(30),
            token(31),
            token(32),
            IdempotencyKeyIdV1::derive(seed).unwrap(),
        )
        .unwrap();
        let request_id = request.request_id();
        let receipt = AuthorizationReceiptV1::new(
            request_id,
            AuthorityContextIdV1::derive("context").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive("old-state").unwrap(),
            StateTokenIdV1::derive("new-state").unwrap(),
        )
        .unwrap();
        AuthorizedExecutionActionV1::new(request, receipt).unwrap()
    }

    fn tenure() -> StepExecutionTenureV1 {
        let fixed_envelope_commitment = RunReservationV1 {
            semantic_operation_hash: token(11),
            inputs_commitment: token(12),
            environment_commitment: token(13),
            target_commitment: token(14),
            execution_boundary_commitment: token(15),
            deadline: 1,
            launch_ordinal: 1,
            current_step_term: None,
        }
        .fixed_envelope_commitment()
        .unwrap();
        StepExecutionTenureV1::acquire(StepExecutionAcquisitionV1 {
            binding: binding(),
            next_fence: 7,
            executor: PrincipalIdV1::derive("executor").unwrap(),
            store_generation_id: StoreGenerationIdV1::from_digest(token(8)),
            authority_epoch: 9,
            fixed_envelope_commitment,
            run_limit: 4,
            issued_at: 100,
            expires_at: 150,
            hard_deadline: 200,
            authority: authority(ExecutionActionV1::AcquireStepExecution, "acquire"),
        })
        .unwrap()
    }

    #[test]
    fn lease_attempt_pair_is_one_to_one_and_terms_are_contiguous() {
        let mut tenure = tenure();
        assert_eq!(tenure.lease().attempt_id(), tenure.attempt().id());
        assert_eq!(tenure.attempt().lease_id(), tenure.lease().id());
        let first = tenure.current_term().id();
        let second = tenure
            .renew(
                first,
                120,
                180,
                authority(ExecutionActionV1::RenewStepLeaseTerm, "renew"),
            )
            .unwrap();
        assert_eq!(second.ordinal(), 2);
        assert_eq!(second.prior_term_id(), Some(first));
        assert!(
            tenure
                .renew(
                    first,
                    130,
                    190,
                    authority(ExecutionActionV1::RenewStepLeaseTerm, "stale-renew")
                )
                .is_err()
        );
    }

    #[test]
    fn run_graph_is_exact_and_terminal_never_reopens() {
        let tenure = tenure();
        let attempt = ExecutionAttemptV1::Step(Box::new(tenure.attempt().clone()));
        let mut run_set = RunSetV1::new(&attempt);
        let revision = run_set.revision();
        let run_id = tenure
            .reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                RunReservationV1 {
                    semantic_operation_hash: token(11),
                    inputs_commitment: token(12),
                    environment_commitment: token(13),
                    target_commitment: token(14),
                    execution_boundary_commitment: token(15),
                    deadline: 140,
                    launch_ordinal: 1,
                    current_step_term: None,
                },
            )
            .unwrap();
        let revision = run_set.revision();
        assert_eq!(
            tenure.transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                114,
                RunStateV1::Succeeded,
            ),
            Err(ExecutionRuntimeErrorV1::IllegalRunTransition),
            "Reserved -> Succeeded must remain illegal"
        );
        let revision = run_set.revision();
        assert_eq!(
            tenure.transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                119,
                RunStateV1::TimedOut,
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineNotReached)
        );
        let revision = run_set.revision();
        tenure
            .transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                115,
                RunStateV1::Active,
            )
            .unwrap();
        let revision = run_set.revision();
        assert!(
            tenure
                .transition_run(
                    &mut run_set,
                    run_id,
                    revision,
                    tenure.current_term().id(),
                    120,
                    RunStateV1::DefinitelyNotStarted,
                )
                .is_err()
        );
        let revision = run_set.revision();
        tenure
            .transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                RunStateV1::Succeeded,
            )
            .unwrap();
        let revision = run_set.revision();
        assert!(
            tenure
                .transition_run(
                    &mut run_set,
                    run_id,
                    revision,
                    tenure.current_term().id(),
                    125,
                    RunStateV1::Active,
                )
                .is_err()
        );
    }

    #[test]
    fn retry_requires_conclusive_no_start() {
        let tenure = tenure();
        let attempt = ExecutionAttemptV1::Step(Box::new(tenure.attempt().clone()));
        let mut run_set = RunSetV1::new(&attempt);
        let revision = run_set.revision();
        let run_id = tenure
            .reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                RunReservationV1 {
                    semantic_operation_hash: token(11),
                    inputs_commitment: token(12),
                    environment_commitment: token(13),
                    target_commitment: token(14),
                    execution_boundary_commitment: token(15),
                    deadline: 140,
                    launch_ordinal: 1,
                    current_step_term: None,
                },
            )
            .unwrap();
        let revision = run_set.revision();
        assert!(
            tenure
                .retry_run(
                    &mut run_set,
                    run_id,
                    revision,
                    tenure.current_term().id(),
                    115,
                    145,
                )
                .is_err()
        );
        let revision = run_set.revision();
        assert_eq!(
            tenure.transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                115,
                RunStateV1::DefinitelyNotStarted,
            ),
            Err(ExecutionRuntimeErrorV1::RunNoStartProofRequired)
        );
        let revision = run_set.revision();
        let receipt = RunNoStartReceiptV1::from_validated_boundary_observation(
            &run_set.runs()[0],
            115,
            token(16),
            token(17),
        )
        .unwrap();
        tenure
            .mark_run_definitely_not_started(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                115,
                receipt,
            )
            .unwrap();
        let revision = run_set.revision();
        assert_eq!(
            tenure.retry_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                151,
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineOutsideTerm)
        );
        let revision = run_set.revision();
        let retry_id = tenure
            .retry_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                145,
            )
            .unwrap();
        let retry = run_set
            .runs()
            .iter()
            .find(|run| run.id() == retry_id)
            .unwrap();
        assert_eq!(retry.launch_ordinal(), 2);
        assert_eq!(retry.semantic_operation_hash, token(11));
    }

    #[test]
    fn ordinary_reservation_cannot_bypass_the_retry_chain() {
        let tenure = tenure();
        let attempt = ExecutionAttemptV1::Step(Box::new(tenure.attempt().clone()));
        let mut run_set = RunSetV1::new(&attempt);
        let reservation = |launch_ordinal| RunReservationV1 {
            semantic_operation_hash: token(11),
            inputs_commitment: token(12),
            environment_commitment: token(13),
            target_commitment: token(14),
            execution_boundary_commitment: token(15),
            deadline: 140,
            launch_ordinal,
            current_step_term: None,
        };
        let revision = run_set.revision();
        assert_eq!(
            tenure.reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                reservation(99),
            ),
            Err(ExecutionRuntimeErrorV1::InvalidRunLaunchChain)
        );
        let revision = run_set.revision();
        let run_id = tenure
            .reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                reservation(1),
            )
            .unwrap();
        let revision = run_set.revision();
        tenure
            .transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                115,
                RunStateV1::Active,
            )
            .unwrap();
        let revision = run_set.revision();
        tenure
            .transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                RunStateV1::Failed,
            )
            .unwrap();
        let revision = run_set.revision();
        assert_eq!(
            tenure.reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                125,
                reservation(2),
            ),
            Err(ExecutionRuntimeErrorV1::InvalidRunLaunchChain)
        );
    }

    #[test]
    fn run_fixed_envelope_and_deadline_are_enforced() {
        let tenure = tenure();
        let attempt = ExecutionAttemptV1::Step(Box::new(tenure.attempt().clone()));
        let mut run_set = RunSetV1::new(&attempt);
        let mut outside_envelope = RunReservationV1 {
            semantic_operation_hash: token(11),
            inputs_commitment: token(12),
            environment_commitment: token(13),
            target_commitment: token(99),
            execution_boundary_commitment: token(15),
            deadline: 120,
            launch_ordinal: 1,
            current_step_term: None,
        };
        let revision = run_set.revision();
        let mut correct_envelope = outside_envelope.clone();
        correct_envelope.target_commitment = token(14);
        correct_envelope.deadline = 110;
        assert_eq!(
            tenure.reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                correct_envelope.clone(),
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineOutsideTerm)
        );
        correct_envelope.deadline = 151;
        assert_eq!(
            tenure.reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                correct_envelope,
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineOutsideTerm)
        );
        assert_eq!(
            tenure.reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                outside_envelope.clone(),
            ),
            Err(ExecutionRuntimeErrorV1::RunOutsideFixedEnvelope)
        );
        outside_envelope.target_commitment = token(14);
        assert_eq!(
            tenure.reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                99,
                outside_envelope.clone(),
            ),
            Err(ExecutionRuntimeErrorV1::StaleRunOrLeaseBasis)
        );
        assert_eq!(
            tenure.submission_fence(tenure.current_term().id(), 99, &run_set),
            Err(ExecutionRuntimeErrorV1::SubmissionFenceUnavailable)
        );
        let revision = run_set.revision();
        let run_id = tenure
            .reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                outside_envelope,
            )
            .unwrap();
        let revision = run_set.revision();
        assert_eq!(
            tenure.transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                RunStateV1::Active,
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineExpired)
        );
        let revision = run_set.revision();
        tenure
            .transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                119,
                RunStateV1::Active,
            )
            .unwrap();
        let revision = run_set.revision();
        assert_eq!(
            tenure.transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                RunStateV1::Succeeded,
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineExpired)
        );
        let revision = run_set.revision();
        assert_eq!(
            tenure.transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                RunStateV1::Failed,
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineExpired)
        );
        let revision = run_set.revision();
        assert_eq!(
            tenure.append_run_segment(
                &mut run_set,
                RunSegmentAppendV1 {
                    run_id,
                    expected_run_set_revision: revision,
                    expected_term_id: tenure.current_term().id(),
                    as_of: 120,
                    process_or_job_identity: token(21),
                    segment_commitment: token(22),
                },
            ),
            Err(ExecutionRuntimeErrorV1::RunDeadlineExpired)
        );
        let revision = run_set.revision();
        tenure
            .transition_run(
                &mut run_set,
                run_id,
                revision,
                tenure.current_term().id(),
                120,
                RunStateV1::TimedOut,
            )
            .unwrap();
    }

    #[test]
    fn stored_carrier_rejects_cross_invariant_mutants() {
        let mut carrier = StepExecutionCarrierV1::acquire(StepExecutionAcquisitionV1 {
            binding: binding(),
            next_fence: 7,
            executor: PrincipalIdV1::derive("executor").unwrap(),
            store_generation_id: StoreGenerationIdV1::from_digest(token(8)),
            authority_epoch: 9,
            fixed_envelope_commitment: RunReservationV1 {
                semantic_operation_hash: token(11),
                inputs_commitment: token(12),
                environment_commitment: token(13),
                target_commitment: token(14),
                execution_boundary_commitment: token(15),
                deadline: 1,
                launch_ordinal: 1,
                current_step_term: None,
            }
            .fixed_envelope_commitment()
            .unwrap(),
            run_limit: 4,
            issued_at: 100,
            expires_at: 150,
            hard_deadline: 200,
            authority: authority(ExecutionActionV1::AcquireStepExecution, "carrier-acquire"),
        })
        .unwrap();
        let initial_term_id = carrier.tenure().current_term().id();
        carrier
            .renew(
                initial_term_id,
                120,
                180,
                authority(ExecutionActionV1::RenewStepLeaseTerm, "carrier-renew"),
            )
            .unwrap();
        let revision = carrier.run_set().revision();
        let term_id = carrier.tenure().current_term().id();
        let run_id = carrier
            .reserve_run(
                revision,
                term_id,
                121,
                RunReservationV1 {
                    semantic_operation_hash: token(11),
                    inputs_commitment: token(12),
                    environment_commitment: token(13),
                    target_commitment: token(14),
                    execution_boundary_commitment: token(15),
                    deadline: 170,
                    launch_ordinal: 1,
                    current_step_term: None,
                },
            )
            .unwrap();
        let revision = carrier.run_set().revision();
        let receipt = RunNoStartReceiptV1::from_validated_boundary_observation(
            &carrier.run_set().runs()[0],
            122,
            token(16),
            token(17),
        )
        .unwrap();
        carrier
            .mark_run_definitely_not_started(revision, term_id, 122, receipt)
            .unwrap();
        let revision = carrier.run_set().revision();
        let retry_id = carrier
            .retry_run(run_id, revision, term_id, 123, 175)
            .unwrap();
        assert_eq!(
            StepExecutionCarrierV1::from_canonical_value(&carrier.canonical_value().unwrap())
                .unwrap(),
            carrier
        );

        let mut noncontiguous_retry = carrier.canonical_value().unwrap();
        let CborValue::Array(carrier_fields) = &mut noncontiguous_retry else {
            unreachable!()
        };
        let CborValue::Array(run_set_fields) = &mut carrier_fields[2] else {
            unreachable!()
        };
        let CborValue::Array(runs) = &mut run_set_fields[4] else {
            unreachable!()
        };
        let CborValue::Array(retry_fields) = &mut runs[1] else {
            unreachable!()
        };
        let gap = RunV1::reserve(
            &ExecutionAttemptV1::Step(Box::new(carrier.tenure().attempt().clone())),
            RunReservationV1 {
                semantic_operation_hash: token(11),
                inputs_commitment: token(12),
                environment_commitment: token(13),
                target_commitment: token(14),
                execution_boundary_commitment: token(15),
                deadline: 175,
                launch_ordinal: 3,
                current_step_term: Some(term_id),
            },
        )
        .unwrap();
        retry_fields[0] = bytes(gap.id().as_bytes());
        retry_fields[8] = CborValue::Unsigned(3);
        assert_eq!(
            StepExecutionCarrierV1::from_canonical_value(&noncontiguous_retry),
            Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)
        );

        let mut broken_terms = carrier.tenure.clone();
        broken_terms.terms[1].issued_at = broken_terms.terms[0].issued_at - 1;
        assert_eq!(
            broken_terms.validate_pair(),
            Err(ExecutionRuntimeErrorV1::BrokenLeaseAttemptPair)
        );

        let mut foreign_term = carrier.canonical_value().unwrap();
        let CborValue::Array(carrier_fields) = &mut foreign_term else {
            unreachable!()
        };
        let CborValue::Array(run_set_fields) = &mut carrier_fields[2] else {
            unreachable!()
        };
        let CborValue::Array(runs) = &mut run_set_fields[4] else {
            unreachable!()
        };
        let CborValue::Array(run_fields) = &mut runs[0] else {
            unreachable!()
        };
        run_fields[9] = CborValue::optional(Some(bytes(&token(99))));
        assert_eq!(
            StepExecutionCarrierV1::from_canonical_value(&foreign_term),
            Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)
        );

        let mut terminal_with_open_run = carrier.clone();
        terminal_with_open_run.tenure.attempt.state =
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Yielded);
        terminal_with_open_run.tenure.lease.state =
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Yielded);
        let malformed = terminal_with_open_run.canonical_value().unwrap();
        assert_eq!(
            StepExecutionCarrierV1::from_canonical_value(&malformed),
            Err(ExecutionRuntimeErrorV1::InvalidStoredExecutionCarrier)
        );
        assert_eq!(carrier.run_set().runs()[0].id(), run_id);
        assert_eq!(carrier.run_set().runs()[1].id(), retry_id);
    }

    #[test]
    fn non_step_attempts_cannot_receive_lease_terms_or_step_power() {
        let dispatch = DispatchAttemptV1::new(
            EffectIntentIdV1::derive("intent").unwrap(),
            1,
            token(17),
            Some(binding()),
        )
        .unwrap();
        assert!(!dispatch.has_step_lease_authority());
        assert!(!dispatch.may_mutate_originating_step());
        let attempt = ExecutionAttemptV1::Dispatch(dispatch);
        assert!(
            RunV1::reserve(
                &attempt,
                RunReservationV1 {
                    semantic_operation_hash: token(18),
                    inputs_commitment: token(19),
                    environment_commitment: token(20),
                    target_commitment: token(21),
                    execution_boundary_commitment: token(22),
                    deadline: 100,
                    launch_ordinal: 1,
                    current_step_term: Some(LeaseTermIdV1::derive("forged").unwrap()),
                }
            )
            .is_err()
        );
    }

    #[test]
    fn execution_cannot_close_pair_as_submitted() {
        let mut tenure = tenure();
        let attempt = ExecutionAttemptV1::Step(Box::new(tenure.attempt().clone()));
        let mut run_set = RunSetV1::new(&attempt);
        let revision = run_set.revision();
        assert_eq!(
            tenure.abandon(
                StepAttemptTerminalV1::Submitted,
                tenure.current_term().id(),
                110,
                &mut run_set,
                revision,
                authority(ExecutionActionV1::AbandonStepAttempt, "submit-smuggle"),
            ),
            Err(ExecutionRuntimeErrorV1::SubmissionOwnedByStep)
        );
        assert!(tenure.attempt().is_live());
    }

    #[test]
    fn terminal_pair_closes_open_runs_atomically() {
        let mut tenure = tenure();
        let attempt = ExecutionAttemptV1::Step(Box::new(tenure.attempt().clone()));
        let mut run_set = RunSetV1::new(&attempt);
        let revision = run_set.revision();
        tenure
            .reserve_run(
                &mut run_set,
                revision,
                tenure.current_term().id(),
                110,
                RunReservationV1 {
                    semantic_operation_hash: token(11),
                    inputs_commitment: token(12),
                    environment_commitment: token(13),
                    target_commitment: token(14),
                    execution_boundary_commitment: token(15),
                    deadline: 140,
                    launch_ordinal: 1,
                    current_step_term: None,
                },
            )
            .unwrap();
        let revision = run_set.revision();
        tenure
            .abandon(
                StepAttemptTerminalV1::Fenced,
                tenure.current_term().id(),
                120,
                &mut run_set,
                revision,
                authority(ExecutionActionV1::AbandonStepAttempt, "fence"),
            )
            .unwrap();
        assert_eq!(run_set.runs()[0].state(), RunStateV1::Fenced);
        assert_eq!(
            tenure.attempt().state(),
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Fenced)
        );
    }

    #[test]
    fn execution_action_catalog_is_dense_and_exact() {
        assert_eq!(
            ExecutionActionV1::ALL.map(ExecutionActionV1::global_tag),
            [
                23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38
            ]
        );
        assert_eq!(
            ExecutionActionV1::ALL
                .into_iter()
                .map(ExecutionActionV1::descriptor_id)
                .collect::<BTreeSet<_>>()
                .len(),
            16
        );
    }
}
