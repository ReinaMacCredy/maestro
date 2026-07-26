use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

use crate::domain::vnext::persistence::protected_locator_lease::{
    ProtectedLocatorCeremonyContinuationV1, ProtectedLocatorFinalityDispositionV1,
    ProtectedLocatorLeaseV1, ProtectedLocatorOperationJoinV1, ProtectedLocatorOwnerJoinV1,
    ProtectedLocatorPreStoreJoinV1,
};

pub(in crate::domain::vnext) mod owner_sealed {
    pub trait Sealed {}
}

pub(super) trait InstallationFinalityVariantV1: owner_sealed::Sealed {}

pub(super) struct ActiveStoreFinalityV1;
pub(super) struct PreStoreFinalityV1;

impl owner_sealed::Sealed for ActiveStoreFinalityV1 {}
impl owner_sealed::Sealed for PreStoreFinalityV1 {}
impl InstallationFinalityVariantV1 for ActiveStoreFinalityV1 {}
impl InstallationFinalityVariantV1 for PreStoreFinalityV1 {}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain::vnext) struct InstallationFinalityCurrentnessV1 {
    pub(super) installation: [u8; 32],
    pub(super) tenant: [u8; 32],
    pub(super) principal: [u8; 32],
    pub(super) authority: [u8; 32],
    pub(super) realm: [u8; 32],
    pub(super) domain: [u8; 32],
    pub(super) store_instance: [u8; 32],
    pub(super) activation_incarnation: [u8; 32],
    pub(super) head: [u8; 32],
    pub(super) head_revision: u64,
    pub(super) generation: [u8; 32],
    pub(super) generation_ordinal: u64,
    pub(super) store_cas: [u8; 32],
    pub(super) host_connection: [u8; 32],
    pub(super) host_currentness: [u8; 32],
    pub(super) currentness: [u8; 32],
    pub(super) fence: [u8; 32],
    pub(super) revocation_revision: u64,
}

impl InstallationFinalityCurrentnessV1 {
    fn validate(self) -> Result<(), DurableInstallationFinalityErrorV1> {
        if [
            self.installation,
            self.tenant,
            self.principal,
            self.authority,
            self.realm,
            self.domain,
            self.store_instance,
            self.activation_incarnation,
            self.head,
            self.generation,
            self.store_cas,
            self.host_connection,
            self.host_currentness,
            self.currentness,
            self.fence,
        ]
        .contains(&[0; 32])
            || self.head_revision == 0
            || self.generation_ordinal == 0
            || self.revocation_revision == 0
        {
            return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
        }
        Ok(())
    }
}

pub(in crate::domain::vnext) struct ActiveStoreFinalityRequestV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: ActiveStoreDecisionTupleV1,
}

pub(in crate::domain::vnext) struct PreStoreFinalityRequestV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: PreStoreDecisionTupleV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain::vnext) struct ActiveStoreDecisionTupleV1 {
    pub(super) operation: [u8; 32],
    pub(super) attempt: [u8; 32],
    pub(super) action: [u8; 32],
    pub(super) request: [u8; 32],
    pub(super) release: [u8; 32],
    pub(super) rows: [u8; 32],
    pub(super) successor_head: [u8; 32],
    pub(super) result: [u8; 32],
    pub(super) idempotency_key: [u8; 32],
    pub(super) idempotency_meaning: [u8; 32],
    pub(super) writer_protocol_epoch: u64,
    pub(super) schema_epoch: u64,
    pub(super) migration_epoch: u64,
    pub(super) census: [u8; 32],
    pub(super) consumer_set: [u8; 32],
    pub(super) consumer_gate_result: [u8; 32],
    pub(super) association_identity: [u8; 32],
    pub(super) association_meaning: [u8; 32],
    pub(super) distribution_commit: [u8; 32],
    pub(super) receipt: [u8; 32],
    pub(super) expected_old_owner_state: [u8; 32],
    pub(super) invocation: [u8; 32],
    pub(super) carrier: [u8; 32],
    pub(super) host_guard: [u8; 32],
    pub(super) postcondition: [u8; 32],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain::vnext) struct PreStoreDecisionTupleV1 {
    pub(super) operation: [u8; 32],
    pub(super) ceremony_spec: [u8; 32],
    pub(super) attempt: [u8; 32],
    pub(super) protected_attempt_currentness: [u8; 32],
    pub(super) release: [u8; 32],
    pub(super) facility: [u8; 32],
    pub(super) locator_identity: [u8; 32],
    pub(super) candidate_association: [u8; 32],
    pub(super) association_meaning: [u8; 32],
    pub(super) candidate_store_lineage: [u8; 32],
    pub(super) target: [u8; 32],
    pub(super) distribution_commit: [u8; 32],
    pub(super) source_carrier: [u8; 32],
    pub(super) candidate_carrier: [u8; 32],
    pub(super) writer_protocol_epoch: u64,
    pub(super) schema_epoch: u64,
    pub(super) migration_epoch: u64,
    pub(super) census: [u8; 32],
    pub(super) consumer_set: [u8; 32],
    pub(super) invocation: [u8; 32],
    pub(super) idempotency_key: [u8; 32],
    pub(super) idempotency_meaning: [u8; 32],
    pub(super) host_guard: [u8; 32],
    pub(super) currentness_fence: [u8; 32],
    pub(super) candidate_postcondition: [u8; 32],
    pub(super) inert_marker: [u8; 32],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain::vnext) struct ActiveStoreCommittedReadbackV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: ActiveStoreDecisionTupleV1,
    pub(super) association: [u8; 32],
    pub(super) consumer_gate_result: [u8; 32],
    pub(super) receipt: [u8; 32],
    pub(super) distribution_commit: [u8; 32],
    pub(super) committed_head: [u8; 32],
    pub(super) result: [u8; 32],
    pub(super) idempotency_rows: [u8; 32],
}

pub(in crate::domain::vnext) enum ActiveStoreOwnerOutcomeV1 {
    PreCommitRefused,
    Committed(ActiveStoreCommittedReadbackV1),
    AcknowledgementLost(Option<ActiveStoreCommittedReadbackV1>),
    UnknownOccurrence,
    IntegrityBlocked,
}

pub(in crate::domain::vnext) struct PreStoreOwnerValidationV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: PreStoreDecisionTupleV1,
    pub(super) write_count: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(in crate::domain::vnext) trait ActiveStoreFinalityOwnerV1:
    owner_sealed::Sealed
{
    fn prepare_request(
        &mut self,
    ) -> Result<ActiveStoreFinalityRequestV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn commit_and_readback(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &ActiveStoreFinalityRequestV1,
    ) -> Result<ActiveStoreOwnerOutcomeV1, DurableInstallationFinalityErrorV1>;
}

pub(in crate::domain::vnext) trait PreStoreFinalityOwnerV1:
    owner_sealed::Sealed
{
    fn prepare_request(
        &mut self,
    ) -> Result<PreStoreFinalityRequestV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn validate_inactive_candidate(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &PreStoreFinalityRequestV1,
    ) -> Result<PreStoreOwnerValidationV1, DurableInstallationFinalityErrorV1>;

    fn pre_dispatch_recheck(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &PreStoreFinalityRequestV1,
    ) -> Result<(), DurableInstallationFinalityErrorV1>;

    fn final_recheck(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &PreStoreFinalityRequestV1,
        outcome: ProtectedLocatorFinalityDispositionV1,
    ) -> Result<(), DurableInstallationFinalityErrorV1>;
}

struct DurableInstallationFinalityBackendV1<'effect, B: ?Sized, K>
where
    K: InstallationFinalityVariantV1,
{
    backend: &'effect mut B,
    consumed: Cell<bool>,
    _variant: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'effect, B: ?Sized, K> DurableInstallationFinalityBackendV1<'effect, B, K>
where
    K: InstallationFinalityVariantV1,
{
    fn capture(backend: &'effect mut B) -> Self {
        Self {
            backend,
            consumed: Cell::new(false),
            _variant: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }
}

impl<'effect, B: ?Sized> DurableInstallationFinalityBackendV1<'effect, B, ActiveStoreFinalityV1>
where
    B: ActiveStoreFinalityOwnerV1,
{
    fn consume_active(
        self,
        request: ActiveStoreFinalityRequestV1,
    ) -> Result<DurableInstallationFinalityOutcomeV1, DurableInstallationFinalityErrorV1> {
        validate_active_request(&request)?;
        if self.consumed.replace(true) {
            return Err(DurableInstallationFinalityErrorV1::Replay);
        }
        let expected = request.currentness;
        match self.backend.commit_and_readback(expected, &request)? {
            ActiveStoreOwnerOutcomeV1::PreCommitRefused => {
                Err(DurableInstallationFinalityErrorV1::PostconditionMismatch)
            }
            ActiveStoreOwnerOutcomeV1::Committed(readback)
            | ActiveStoreOwnerOutcomeV1::AcknowledgementLost(Some(readback)) => {
                if active_readback_matches(expected, request.decision, readback) {
                    Ok(DurableInstallationFinalityOutcomeV1::Committed)
                } else {
                    Ok(DurableInstallationFinalityOutcomeV1::IntegrityBlocked)
                }
            }
            ActiveStoreOwnerOutcomeV1::AcknowledgementLost(None) => {
                Ok(DurableInstallationFinalityOutcomeV1::RecoveryRequired)
            }
            ActiveStoreOwnerOutcomeV1::UnknownOccurrence => {
                Ok(DurableInstallationFinalityOutcomeV1::InDoubt)
            }
            ActiveStoreOwnerOutcomeV1::IntegrityBlocked => {
                Ok(DurableInstallationFinalityOutcomeV1::IntegrityBlocked)
            }
        }
    }
}

impl<'effect, B: ?Sized> DurableInstallationFinalityBackendV1<'effect, B, PreStoreFinalityV1>
where
    B: PreStoreFinalityOwnerV1,
{
    fn consume_pre_store<'locator>(
        self,
        request: PreStoreFinalityRequestV1,
        locator_lease: ProtectedLocatorLeaseV1<'locator>,
    ) -> Result<
        PreStoreCeremonyContinuationV1<'effect, 'locator, B>,
        DurableInstallationFinalityErrorV1,
    > {
        validate_pre_store_request(&request)?;
        if self.consumed.replace(true) {
            return Err(DurableInstallationFinalityErrorV1::Replay);
        }
        let expected = request.currentness;
        let readback = self
            .backend
            .validate_inactive_candidate(expected, &request)?;
        if readback.currentness != expected
            || readback.decision != request.decision
            || readback.write_count != 0
        {
            return Err(DurableInstallationFinalityErrorV1::PostconditionMismatch);
        }
        let join = ProtectedLocatorPreStoreJoinV1::from_installation_owner(
            ProtectedLocatorOperationJoinV1::new(
                request.decision.operation,
                request.decision.ceremony_spec,
                request.decision.attempt,
                request.decision.candidate_association,
                request.decision.target,
            ),
            ProtectedLocatorOwnerJoinV1::new(
                request.currentness.installation,
                request.currentness.realm,
                request.currentness.domain,
                request.decision.facility,
                request.decision.locator_identity,
            ),
        )
        .map_err(|_| DurableInstallationFinalityErrorV1::CurrentnessMismatch)?;
        let locator = locator_lease
            .begin_pre_store(join)
            .map_err(|_| DurableInstallationFinalityErrorV1::CurrentnessMismatch)?;
        Ok(PreStoreCeremonyContinuationV1 {
            backend: self.backend,
            request,
            locator,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(super) struct PreStoreCeremonyContinuationV1<
    'effect,
    'locator,
    B: PreStoreFinalityOwnerV1 + ?Sized,
> {
    backend: &'effect mut B,
    request: PreStoreFinalityRequestV1,
    locator: ProtectedLocatorCeremonyContinuationV1<'locator>,
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<B: PreStoreFinalityOwnerV1 + ?Sized> PreStoreCeremonyContinuationV1<'_, '_, B> {
    fn dispatch(
        self,
    ) -> Result<DurableInstallationFinalityOutcomeV1, DurableInstallationFinalityErrorV1> {
        if self.consumed.replace(true) {
            return Err(DurableInstallationFinalityErrorV1::Replay);
        }
        self.backend
            .pre_dispatch_recheck(self.request.currentness, &self.request)?;
        let locator_outcome = self
            .locator
            .dispatch()
            .map_err(|_| DurableInstallationFinalityErrorV1::PostconditionMismatch)?;
        if self
            .backend
            .final_recheck(self.request.currentness, &self.request, locator_outcome)
            .is_err()
        {
            return Ok(match locator_outcome {
                ProtectedLocatorFinalityDispositionV1::InDoubt => {
                    DurableInstallationFinalityOutcomeV1::InDoubt
                }
                _ => DurableInstallationFinalityOutcomeV1::IntegrityBlocked,
            });
        }
        Ok(locator_outcome.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DurableInstallationFinalityOutcomeV1 {
    Committed,
    RecoveryRequired,
    InDoubt,
    IntegrityBlocked,
}

impl From<ProtectedLocatorFinalityDispositionV1> for DurableInstallationFinalityOutcomeV1 {
    fn from(value: ProtectedLocatorFinalityDispositionV1) -> Self {
        match value {
            ProtectedLocatorFinalityDispositionV1::Committed => Self::Committed,
            ProtectedLocatorFinalityDispositionV1::RecoveryRequired => Self::RecoveryRequired,
            ProtectedLocatorFinalityDispositionV1::InDoubt => Self::InDoubt,
            ProtectedLocatorFinalityDispositionV1::IntegrityBlocked => Self::IntegrityBlocked,
        }
    }
}

fn active_readback_matches(
    expected: InstallationFinalityCurrentnessV1,
    decision: ActiveStoreDecisionTupleV1,
    readback: ActiveStoreCommittedReadbackV1,
) -> bool {
    readback.currentness == expected
        && readback.decision == decision
        && readback.association == decision.association_identity
        && readback.consumer_gate_result == decision.consumer_gate_result
        && readback.receipt == decision.receipt
        && readback.distribution_commit == decision.distribution_commit
        && readback.committed_head == decision.successor_head
        && readback.result == decision.result
        && readback.idempotency_rows == decision.idempotency_meaning
}

pub(super) struct Stage9ActiveStoreFinalityOperationV1<'effect> {
    backend: &'effect mut dyn ActiveStoreFinalityOwnerV1,
    request: ActiveStoreFinalityRequestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) struct Stage9ActiveStoreFinalityOutcomeV1 {
    class: Stage9ActiveStoreFinalityOutcomeClassV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Stage9ActiveStoreFinalityOutcomeClassV1 {
    Committed,
    RecoveryRequired,
    InDoubt,
    IntegrityBlocked,
}

#[derive(Debug, Error, Eq, PartialEq)]
#[error("the Stage 9 ActiveStore finality operation was refused")]
pub(super) struct Stage9ActiveStoreFinalityErrorV1;

impl Stage9ActiveStoreFinalityOutcomeV1 {
    pub(super) fn into_class(self) -> Stage9ActiveStoreFinalityOutcomeClassV1 {
        self.class
    }
}

pub(super) fn prepare_active_from_stage9_owner(
    backend: &mut dyn ActiveStoreFinalityOwnerV1,
) -> Result<Stage9ActiveStoreFinalityOperationV1<'_>, Stage9ActiveStoreFinalityErrorV1> {
    let request = backend
        .prepare_request()
        .map_err(|_| Stage9ActiveStoreFinalityErrorV1)?;
    validate_active_request(&request).map_err(|_| Stage9ActiveStoreFinalityErrorV1)?;
    Ok(Stage9ActiveStoreFinalityOperationV1 {
        backend,
        request,
        _not_send_or_sync: PhantomData,
    })
}

pub(super) fn execute_active_from_stage9_owner(
    operation: Stage9ActiveStoreFinalityOperationV1<'_>,
) -> Result<Stage9ActiveStoreFinalityOutcomeV1, Stage9ActiveStoreFinalityErrorV1> {
    DurableInstallationFinalityBackendV1::capture(operation.backend)
        .consume_active(operation.request)
        .map_err(|_| Stage9ActiveStoreFinalityErrorV1)
        .map(|outcome| Stage9ActiveStoreFinalityOutcomeV1 {
            class: match outcome {
                DurableInstallationFinalityOutcomeV1::Committed => {
                    Stage9ActiveStoreFinalityOutcomeClassV1::Committed
                }
                DurableInstallationFinalityOutcomeV1::RecoveryRequired => {
                    Stage9ActiveStoreFinalityOutcomeClassV1::RecoveryRequired
                }
                DurableInstallationFinalityOutcomeV1::InDoubt => {
                    Stage9ActiveStoreFinalityOutcomeClassV1::InDoubt
                }
                DurableInstallationFinalityOutcomeV1::IntegrityBlocked => {
                    Stage9ActiveStoreFinalityOutcomeClassV1::IntegrityBlocked
                }
            },
            _not_send_or_sync: PhantomData,
        })
}

pub(super) struct Stage11PreStoreFinalityOperationV1<'effect> {
    backend: &'effect mut dyn PreStoreFinalityOwnerV1,
    request: PreStoreFinalityRequestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) struct Stage11PreStoreFinalityOutcomeV1 {
    class: Stage11PreStoreFinalityOutcomeClassV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Stage11PreStoreFinalityOutcomeClassV1 {
    Committed,
    RecoveryRequired,
    InDoubt,
    IntegrityBlocked,
}

#[derive(Debug, Error, Eq, PartialEq)]
#[error("the Stage 11 PreStore finality operation was refused")]
pub(super) struct Stage11PreStoreFinalityErrorV1;

impl Stage11PreStoreFinalityOutcomeV1 {
    pub(super) fn into_class(self) -> Stage11PreStoreFinalityOutcomeClassV1 {
        self.class
    }
}

pub(super) fn prepare_pre_store_from_stage11_owner(
    backend: &mut dyn PreStoreFinalityOwnerV1,
) -> Result<Stage11PreStoreFinalityOperationV1<'_>, Stage11PreStoreFinalityErrorV1> {
    let request = backend
        .prepare_request()
        .map_err(|_| Stage11PreStoreFinalityErrorV1)?;
    validate_pre_store_request(&request).map_err(|_| Stage11PreStoreFinalityErrorV1)?;
    Ok(Stage11PreStoreFinalityOperationV1 {
        backend,
        request,
        _not_send_or_sync: PhantomData,
    })
}

pub(super) fn execute_pre_store_from_stage11_owner<'locator>(
    operation: Stage11PreStoreFinalityOperationV1<'_>,
    locator_lease: ProtectedLocatorLeaseV1<'locator>,
) -> Result<Stage11PreStoreFinalityOutcomeV1, Stage11PreStoreFinalityErrorV1> {
    DurableInstallationFinalityBackendV1::capture(operation.backend)
        .consume_pre_store(operation.request, locator_lease)
        .and_then(PreStoreCeremonyContinuationV1::dispatch)
        .map_err(|_| Stage11PreStoreFinalityErrorV1)
        .map(|outcome| Stage11PreStoreFinalityOutcomeV1 {
            class: match outcome {
                DurableInstallationFinalityOutcomeV1::Committed => {
                    Stage11PreStoreFinalityOutcomeClassV1::Committed
                }
                DurableInstallationFinalityOutcomeV1::RecoveryRequired => {
                    Stage11PreStoreFinalityOutcomeClassV1::RecoveryRequired
                }
                DurableInstallationFinalityOutcomeV1::InDoubt => {
                    Stage11PreStoreFinalityOutcomeClassV1::InDoubt
                }
                DurableInstallationFinalityOutcomeV1::IntegrityBlocked => {
                    Stage11PreStoreFinalityOutcomeClassV1::IntegrityBlocked
                }
            },
            _not_send_or_sync: PhantomData,
        })
}

fn validate_active_request(
    request: &ActiveStoreFinalityRequestV1,
) -> Result<(), DurableInstallationFinalityErrorV1> {
    request.currentness.validate()?;
    if [
        request.decision.operation,
        request.decision.attempt,
        request.decision.action,
        request.decision.request,
        request.decision.release,
        request.decision.rows,
        request.decision.successor_head,
        request.decision.result,
        request.decision.idempotency_key,
        request.decision.idempotency_meaning,
        request.decision.census,
        request.decision.consumer_set,
        request.decision.consumer_gate_result,
        request.decision.association_identity,
        request.decision.association_meaning,
        request.decision.distribution_commit,
        request.decision.receipt,
        request.decision.expected_old_owner_state,
        request.decision.invocation,
        request.decision.carrier,
        request.decision.host_guard,
        request.decision.postcondition,
    ]
    .contains(&[0; 32])
    {
        return Err(DurableInstallationFinalityErrorV1::InvalidRequest);
    }
    if request.decision.writer_protocol_epoch == 0
        || request.decision.schema_epoch == 0
        || request.decision.migration_epoch == 0
    {
        return Err(DurableInstallationFinalityErrorV1::InvalidRequest);
    }
    Ok(())
}

fn validate_pre_store_request(
    request: &PreStoreFinalityRequestV1,
) -> Result<(), DurableInstallationFinalityErrorV1> {
    request.currentness.validate()?;
    if [
        request.decision.operation,
        request.decision.ceremony_spec,
        request.decision.attempt,
        request.decision.protected_attempt_currentness,
        request.decision.release,
        request.decision.facility,
        request.decision.locator_identity,
        request.decision.candidate_association,
        request.decision.association_meaning,
        request.decision.candidate_store_lineage,
        request.decision.target,
        request.decision.distribution_commit,
        request.decision.source_carrier,
        request.decision.candidate_carrier,
        request.decision.census,
        request.decision.consumer_set,
        request.decision.invocation,
        request.decision.idempotency_key,
        request.decision.idempotency_meaning,
        request.decision.host_guard,
        request.decision.currentness_fence,
        request.decision.candidate_postcondition,
        request.decision.inert_marker,
    ]
    .contains(&[0; 32])
    {
        return Err(DurableInstallationFinalityErrorV1::InvalidRequest);
    }
    if request.decision.writer_protocol_epoch == 0
        || request.decision.schema_epoch == 0
        || request.decision.migration_epoch == 0
    {
        return Err(DurableInstallationFinalityErrorV1::InvalidRequest);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum DurableInstallationFinalityErrorV1 {
    #[error("installation finality request is invalid")]
    InvalidRequest,
    #[error("installation finality currentness does not match")]
    CurrentnessMismatch,
    #[error("installation finality capability was replayed")]
    Replay,
    #[error("installation finality postcondition does not match")]
    PostconditionMismatch,
    #[error("installation finality owner backend is unavailable")]
    BackendUnavailable,
}

#[cfg(test)]
struct PreStoreConformanceBackendV1 {
    expected: InstallationFinalityCurrentnessV1,
    decision: PreStoreDecisionTupleV1,
    write_count: u64,
    rechecks: u64,
}

#[cfg(test)]
impl owner_sealed::Sealed for PreStoreConformanceBackendV1 {}

#[cfg(test)]
impl PreStoreFinalityOwnerV1 for PreStoreConformanceBackendV1 {
    fn validate_inactive_candidate(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &PreStoreFinalityRequestV1,
    ) -> Result<PreStoreOwnerValidationV1, DurableInstallationFinalityErrorV1> {
        Ok(PreStoreOwnerValidationV1 {
            currentness: expected,
            decision: request.decision,
            write_count: self.write_count,
            _not_send_or_sync: PhantomData,
        })
    }

    fn pre_dispatch_recheck(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &PreStoreFinalityRequestV1,
    ) -> Result<(), DurableInstallationFinalityErrorV1> {
        if expected != self.expected || request.decision != self.decision {
            return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
        }
        self.rechecks += 1;
        Ok(())
    }

    fn final_recheck(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &PreStoreFinalityRequestV1,
        _outcome: ProtectedLocatorFinalityDispositionV1,
    ) -> Result<(), DurableInstallationFinalityErrorV1> {
        if expected != self.expected || request.decision != self.decision {
            return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
        }
        self.rechecks += 1;
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::domain::vnext) fn consume_pre_store_with_test_owner<'locator>(
    locator_lease: ProtectedLocatorLeaseV1<'locator>,
    write_count: u64,
) -> bool {
    let currentness = InstallationFinalityCurrentnessV1 {
        installation: [1; 32],
        tenant: [60; 32],
        principal: [61; 32],
        authority: [62; 32],
        realm: [2; 32],
        domain: [3; 32],
        store_instance: [44; 32],
        activation_incarnation: [63; 32],
        head: [45; 32],
        head_revision: 64,
        generation: [46; 32],
        generation_ordinal: 65,
        store_cas: [47; 32],
        host_connection: [48; 32],
        host_currentness: [49; 32],
        currentness: [66; 32],
        fence: [67; 32],
        revocation_revision: 50,
    };
    let decision = PreStoreDecisionTupleV1 {
        operation: [4; 32],
        ceremony_spec: [5; 32],
        attempt: [6; 32],
        protected_attempt_currentness: [51; 32],
        release: [53; 32],
        facility: [11; 32],
        locator_identity: [15; 32],
        candidate_association: [9; 32],
        association_meaning: [56; 32],
        candidate_store_lineage: [68; 32],
        target: [10; 32],
        distribution_commit: [57; 32],
        source_carrier: [7; 32],
        candidate_carrier: [8; 32],
        writer_protocol_epoch: 69,
        schema_epoch: 70,
        migration_epoch: 71,
        census: [72; 32],
        consumer_set: [73; 32],
        invocation: [74; 32],
        idempotency_key: [75; 32],
        idempotency_meaning: [76; 32],
        host_guard: [77; 32],
        currentness_fence: [78; 32],
        candidate_postcondition: [79; 32],
        inert_marker: [58; 32],
    };
    let request = PreStoreFinalityRequestV1 {
        currentness,
        decision,
    };
    let mut backend = PreStoreConformanceBackendV1 {
        expected: currentness,
        decision,
        write_count,
        rechecks: 0,
    };
    DurableInstallationFinalityBackendV1::capture(&mut backend)
        .consume_pre_store(request, locator_lease)
        .and_then(PreStoreCeremonyContinuationV1::dispatch)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ActiveBackendV1 {
        expected: InstallationFinalityCurrentnessV1,
        outcome: Option<ActiveStoreOwnerOutcomeV1>,
        effects: u64,
    }

    impl owner_sealed::Sealed for ActiveBackendV1 {}

    impl ActiveStoreFinalityOwnerV1 for ActiveBackendV1 {
        fn commit_and_readback(
            &mut self,
            expected: InstallationFinalityCurrentnessV1,
            _request: &ActiveStoreFinalityRequestV1,
        ) -> Result<ActiveStoreOwnerOutcomeV1, DurableInstallationFinalityErrorV1> {
            if expected != self.expected {
                return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
            }
            let outcome = self
                .outcome
                .take()
                .ok_or(DurableInstallationFinalityErrorV1::Replay)?;
            if !matches!(outcome, ActiveStoreOwnerOutcomeV1::PreCommitRefused) {
                self.effects += 1;
            }
            Ok(outcome)
        }
    }

    fn currentness() -> InstallationFinalityCurrentnessV1 {
        InstallationFinalityCurrentnessV1 {
            installation: [1; 32],
            tenant: [40; 32],
            principal: [41; 32],
            authority: [42; 32],
            realm: [2; 32],
            domain: [3; 32],
            store_instance: [4; 32],
            activation_incarnation: [43; 32],
            head: [5; 32],
            head_revision: 44,
            generation: [6; 32],
            generation_ordinal: 45,
            store_cas: [7; 32],
            host_connection: [8; 32],
            host_currentness: [9; 32],
            currentness: [46; 32],
            fence: [47; 32],
            revocation_revision: 11,
        }
    }

    fn active_decision() -> ActiveStoreDecisionTupleV1 {
        ActiveStoreDecisionTupleV1 {
            operation: [12; 32],
            attempt: [13; 32],
            action: [48; 32],
            request: [49; 32],
            release: [14; 32],
            rows: [15; 32],
            successor_head: [16; 32],
            result: [17; 32],
            idempotency_key: [18; 32],
            idempotency_meaning: [19; 32],
            writer_protocol_epoch: 20,
            schema_epoch: 21,
            migration_epoch: 22,
            census: [23; 32],
            consumer_set: [24; 32],
            consumer_gate_result: [50; 32],
            association_identity: [25; 32],
            association_meaning: [26; 32],
            distribution_commit: [27; 32],
            receipt: [51; 32],
            expected_old_owner_state: [28; 32],
            invocation: [29; 32],
            carrier: [30; 32],
            host_guard: [31; 32],
            postcondition: [32; 32],
        }
    }

    fn active_request() -> ActiveStoreFinalityRequestV1 {
        ActiveStoreFinalityRequestV1 {
            currentness: currentness(),
            decision: active_decision(),
        }
    }

    fn active_readback() -> ActiveStoreCommittedReadbackV1 {
        ActiveStoreCommittedReadbackV1 {
            currentness: currentness(),
            decision: active_decision(),
            association: active_decision().association_identity,
            consumer_gate_result: active_decision().consumer_gate_result,
            receipt: active_decision().receipt,
            distribution_commit: active_decision().distribution_commit,
            committed_head: active_decision().successor_head,
            result: active_decision().result,
            idempotency_rows: active_decision().idempotency_meaning,
        }
    }

    #[test]
    fn active_store_owner_effect_and_readback_are_one_typed_operation() {
        let mut backend = ActiveBackendV1 {
            expected: currentness(),
            outcome: Some(ActiveStoreOwnerOutcomeV1::Committed(active_readback())),
            effects: 0,
        };
        let finality = DurableInstallationFinalityBackendV1::capture(&mut backend)
            .consume_active(active_request())
            .unwrap();
        assert_eq!(finality, DurableInstallationFinalityOutcomeV1::Committed);
        assert_eq!(backend.effects, 1);
    }

    #[test]
    fn active_store_no_op_cannot_mint_finality_from_an_echoed_nonzero_digest() {
        let mut backend = ActiveBackendV1 {
            expected: currentness(),
            outcome: Some(ActiveStoreOwnerOutcomeV1::PreCommitRefused),
            effects: 0,
        };
        assert!(matches!(
            DurableInstallationFinalityBackendV1::capture(&mut backend)
                .consume_active(active_request()),
            Err(DurableInstallationFinalityErrorV1::PostconditionMismatch)
        ));
        assert_eq!(backend.effects, 0);
    }

    #[test]
    fn false_success_and_partial_readback_cannot_mint_finality() {
        let mut backend = ActiveBackendV1 {
            expected: currentness(),
            outcome: Some(ActiveStoreOwnerOutcomeV1::Committed(
                ActiveStoreCommittedReadbackV1 {
                    receipt: [0; 32],
                    ..active_readback()
                },
            )),
            effects: 0,
        };
        assert!(matches!(
            DurableInstallationFinalityBackendV1::capture(&mut backend)
                .consume_active(active_request()),
            Ok(DurableInstallationFinalityOutcomeV1::IntegrityBlocked)
        ));
    }

    #[test]
    fn post_write_outcomes_are_never_reported_as_ordinary_refusal() {
        for (owner_outcome, expected) in [
            (
                ActiveStoreOwnerOutcomeV1::AcknowledgementLost(None),
                DurableInstallationFinalityOutcomeV1::RecoveryRequired,
            ),
            (
                ActiveStoreOwnerOutcomeV1::UnknownOccurrence,
                DurableInstallationFinalityOutcomeV1::InDoubt,
            ),
            (
                ActiveStoreOwnerOutcomeV1::IntegrityBlocked,
                DurableInstallationFinalityOutcomeV1::IntegrityBlocked,
            ),
        ] {
            let mut backend = ActiveBackendV1 {
                expected: currentness(),
                outcome: Some(owner_outcome),
                effects: 0,
            };
            assert_eq!(
                DurableInstallationFinalityBackendV1::capture(&mut backend)
                    .consume_active(active_request()),
                Ok(expected)
            );
        }
    }

    // PreStore success and refusal are exercised from Persistence with a real
    // lifetime-bound ProtectedLocatorLeaseV1; no digest-like substitute exists.

    #[test]
    fn production_owner_entry_points_are_frozen_for_stage9_and_stage11() {
        let _ = execute_active_from_stage9_owner;
        let _ = execute_pre_store_from_stage11_owner;
    }
}
