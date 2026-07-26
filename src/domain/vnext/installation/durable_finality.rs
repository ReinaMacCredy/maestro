use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

use crate::domain::vnext::persistence::protected_locator_lease::ProtectedLocatorLeaseV1;

pub(super) mod owner_sealed {
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
pub(super) struct InstallationFinalityCurrentnessV1 {
    pub(super) installation: [u8; 32],
    pub(super) realm: [u8; 32],
    pub(super) domain: [u8; 32],
    pub(super) store_instance: [u8; 32],
    pub(super) head: [u8; 32],
    pub(super) generation: [u8; 32],
    pub(super) store_cas: [u8; 32],
    pub(super) host_connection: [u8; 32],
    pub(super) host_currentness: [u8; 32],
    pub(super) revocation_revision: u64,
}

impl InstallationFinalityCurrentnessV1 {
    fn validate(self) -> Result<(), DurableInstallationFinalityErrorV1> {
        if [
            self.installation,
            self.realm,
            self.domain,
            self.store_instance,
            self.head,
            self.generation,
            self.store_cas,
            self.host_connection,
            self.host_currentness,
        ]
        .contains(&[0; 32])
            || self.revocation_revision == 0
        {
            return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
        }
        Ok(())
    }
}

pub(super) struct ActiveStoreFinalityRequestV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: ActiveStoreDecisionTupleV1,
}

pub(super) struct PreStoreFinalityRequestV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: PreStoreDecisionTupleV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ActiveStoreDecisionTupleV1 {
    pub(super) operation: [u8; 32],
    pub(super) attempt: [u8; 32],
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
    pub(super) association_identity: [u8; 32],
    pub(super) association_meaning: [u8; 32],
    pub(super) distribution_commit: [u8; 32],
    pub(super) expected_old_owner_state: [u8; 32],
    pub(super) invocation: [u8; 32],
    pub(super) carrier: [u8; 32],
    pub(super) host_guard: [u8; 32],
    pub(super) postcondition: [u8; 32],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct PreStoreDecisionTupleV1 {
    pub(super) operation: [u8; 32],
    pub(super) attempt: [u8; 32],
    pub(super) release: [u8; 32],
    pub(super) facility: [u8; 32],
    pub(super) locator_identity: [u8; 32],
    pub(super) candidate_association: [u8; 32],
    pub(super) association_meaning: [u8; 32],
    pub(super) candidate_root: [u8; 32],
    pub(super) candidate_seal: [u8; 32],
    pub(super) distribution_commit: [u8; 32],
    pub(super) source_carrier: [u8; 32],
    pub(super) candidate_carrier: [u8; 32],
    pub(super) inert_marker: [u8; 32],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ActiveStoreFinalityReadbackV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: ActiveStoreDecisionTupleV1,
    pub(super) durable_effect: [u8; 32],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct PreStoreFinalityReadbackV1 {
    pub(super) currentness: InstallationFinalityCurrentnessV1,
    pub(super) decision: PreStoreDecisionTupleV1,
    pub(super) write_count: u64,
}

pub(super) trait DurableInstallationOwnerEffectV1<K>: owner_sealed::Sealed
where
    K: InstallationFinalityVariantV1,
{
    type Readback;

    fn linearize(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        request: &DurableInstallationFinalityRequestV1<K>,
    ) -> Result<Self::Readback, DurableInstallationFinalityErrorV1>;
}

pub(super) enum DurableInstallationFinalityRequestV1<K> {
    Active(ActiveStoreFinalityRequestV1, PhantomData<K>),
    PreStore(PreStoreFinalityRequestV1, PhantomData<K>),
}

struct DurableInstallationFinalityBackendV1<'effect, B, K>
where
    K: InstallationFinalityVariantV1,
    B: DurableInstallationOwnerEffectV1<K>,
{
    backend: &'effect mut B,
    consumed: Cell<bool>,
    _variant: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'effect, B, K> DurableInstallationFinalityBackendV1<'effect, B, K>
where
    K: InstallationFinalityVariantV1,
    B: DurableInstallationOwnerEffectV1<K>,
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

impl<'effect, B> DurableInstallationFinalityBackendV1<'effect, B, ActiveStoreFinalityV1>
where
    B: DurableInstallationOwnerEffectV1<
            ActiveStoreFinalityV1,
            Readback = ActiveStoreFinalityReadbackV1,
        >,
{
    fn consume_active(
        self,
        request: ActiveStoreFinalityRequestV1,
    ) -> Result<DurableInstallationFinalityV1<'effect>, DurableInstallationFinalityErrorV1> {
        validate_active_request(&request)?;
        if self.consumed.replace(true) {
            return Err(DurableInstallationFinalityErrorV1::Replay);
        }
        let expected = request.currentness;
        let wrapped = DurableInstallationFinalityRequestV1::Active(
            request,
            PhantomData::<ActiveStoreFinalityV1>,
        );
        let readback = self.backend.linearize(expected, &wrapped)?;
        let DurableInstallationFinalityRequestV1::Active(request, _) = &wrapped else {
            unreachable!("invariant: nominal ActiveStore request");
        };
        if readback.currentness != expected
            || readback.decision != request.decision
            || readback.durable_effect == [0; 32]
        {
            return Err(DurableInstallationFinalityErrorV1::PostconditionMismatch);
        }
        Ok(DurableInstallationFinalityV1 {
            effect: readback.durable_effect,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

impl<'effect, B> DurableInstallationFinalityBackendV1<'effect, B, PreStoreFinalityV1>
where
    B: DurableInstallationOwnerEffectV1<PreStoreFinalityV1, Readback = PreStoreFinalityReadbackV1>,
{
    fn consume_pre_store<'locator>(
        self,
        request: PreStoreFinalityRequestV1,
        locator_lease: ProtectedLocatorLeaseV1<'locator>,
    ) -> Result<DurableInstallationFinalityV1<'effect>, DurableInstallationFinalityErrorV1> {
        validate_pre_store_request(&request)?;
        if self.consumed.replace(true) {
            return Err(DurableInstallationFinalityErrorV1::Replay);
        }
        let expected = request.currentness;
        let wrapped = DurableInstallationFinalityRequestV1::PreStore(
            request,
            PhantomData::<PreStoreFinalityV1>,
        );
        let readback = self.backend.linearize(expected, &wrapped)?;
        let DurableInstallationFinalityRequestV1::PreStore(request, _) = &wrapped else {
            unreachable!("invariant: nominal PreStore request");
        };
        if readback.currentness != expected
            || readback.decision != request.decision
            || readback.write_count != 0
        {
            return Err(DurableInstallationFinalityErrorV1::PostconditionMismatch);
        }
        let _locator_validation = locator_lease
            .consume_pre_store_inert(
                request.currentness.installation,
                request.currentness.realm,
                request.currentness.domain,
                request.decision.operation,
                request.decision.attempt,
                request.decision.facility,
                request.decision.locator_identity,
                request.decision.candidate_association,
                request.decision.source_carrier,
                request.decision.candidate_carrier,
                request.decision.candidate_root,
                request.decision.candidate_seal,
            )
            .map_err(|_| DurableInstallationFinalityErrorV1::CurrentnessMismatch)?;
        Ok(DurableInstallationFinalityV1 {
            effect: request.decision.inert_marker,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

struct DurableInstallationFinalityV1<'effect> {
    effect: [u8; 32],
    _lifetime: PhantomData<&'effect mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) fn execute_active_from_stage9_owner(
    request: ActiveStoreFinalityRequestV1,
) -> Result<(), DurableInstallationFinalityErrorV1> {
    let mut backend = super::durable_finality_stage9_seed::acquire();
    DurableInstallationFinalityBackendV1::capture(&mut backend)
        .consume_active(request)
        .map(|_| ())
}

pub(super) fn execute_pre_store_from_stage11_owner<'locator>(
    request: PreStoreFinalityRequestV1,
    locator_lease: ProtectedLocatorLeaseV1<'locator>,
) -> Result<(), DurableInstallationFinalityErrorV1> {
    let mut backend = super::durable_finality_stage11_seed::acquire();
    DurableInstallationFinalityBackendV1::capture(&mut backend)
        .consume_pre_store(request, locator_lease)
        .map(|_| ())
}

fn validate_active_request(
    request: &ActiveStoreFinalityRequestV1,
) -> Result<(), DurableInstallationFinalityErrorV1> {
    request.currentness.validate()?;
    if [
        request.decision.operation,
        request.decision.attempt,
        request.decision.release,
        request.decision.rows,
        request.decision.successor_head,
        request.decision.result,
        request.decision.idempotency_key,
        request.decision.idempotency_meaning,
        request.decision.census,
        request.decision.consumer_set,
        request.decision.association_identity,
        request.decision.association_meaning,
        request.decision.distribution_commit,
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
        request.decision.attempt,
        request.decision.release,
        request.decision.facility,
        request.decision.locator_identity,
        request.decision.candidate_association,
        request.decision.association_meaning,
        request.decision.candidate_root,
        request.decision.candidate_seal,
        request.decision.distribution_commit,
        request.decision.source_carrier,
        request.decision.candidate_carrier,
        request.decision.inert_marker,
    ]
    .contains(&[0; 32])
    {
        return Err(DurableInstallationFinalityErrorV1::InvalidRequest);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(super) enum DurableInstallationFinalityErrorV1 {
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
    readback: PreStoreFinalityReadbackV1,
}

#[cfg(test)]
impl owner_sealed::Sealed for PreStoreConformanceBackendV1 {}

#[cfg(test)]
impl DurableInstallationOwnerEffectV1<PreStoreFinalityV1> for PreStoreConformanceBackendV1 {
    type Readback = PreStoreFinalityReadbackV1;

    fn linearize(
        &mut self,
        expected: InstallationFinalityCurrentnessV1,
        _request: &DurableInstallationFinalityRequestV1<PreStoreFinalityV1>,
    ) -> Result<Self::Readback, DurableInstallationFinalityErrorV1> {
        if expected != self.expected {
            return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
        }
        Ok(self.readback)
    }
}

#[cfg(test)]
pub(in crate::domain::vnext) fn consume_pre_store_with_test_owner<'locator>(
    locator_lease: ProtectedLocatorLeaseV1<'locator>,
    write_count: u64,
) -> bool {
    let currentness = InstallationFinalityCurrentnessV1 {
        installation: [1; 32],
        realm: [2; 32],
        domain: [3; 32],
        store_instance: [44; 32],
        head: [45; 32],
        generation: [46; 32],
        store_cas: [47; 32],
        host_connection: [48; 32],
        host_currentness: [49; 32],
        revocation_revision: 50,
    };
    let decision = PreStoreDecisionTupleV1 {
        operation: [4; 32],
        attempt: [6; 32],
        release: [53; 32],
        facility: [11; 32],
        locator_identity: [15; 32],
        candidate_association: [9; 32],
        association_meaning: [56; 32],
        candidate_root: [29; 32],
        candidate_seal: [30; 32],
        distribution_commit: [57; 32],
        source_carrier: [7; 32],
        candidate_carrier: [8; 32],
        inert_marker: [58; 32],
    };
    let request = PreStoreFinalityRequestV1 {
        currentness,
        decision,
    };
    let mut backend = PreStoreConformanceBackendV1 {
        expected: currentness,
        readback: PreStoreFinalityReadbackV1 {
            currentness,
            decision,
            write_count,
        },
    };
    DurableInstallationFinalityBackendV1::capture(&mut backend)
        .consume_pre_store(request, locator_lease)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ActiveBackendV1 {
        expected: InstallationFinalityCurrentnessV1,
        readback: ActiveStoreFinalityReadbackV1,
        effects: u64,
    }

    impl owner_sealed::Sealed for ActiveBackendV1 {}

    impl DurableInstallationOwnerEffectV1<ActiveStoreFinalityV1> for ActiveBackendV1 {
        type Readback = ActiveStoreFinalityReadbackV1;

        fn linearize(
            &mut self,
            expected: InstallationFinalityCurrentnessV1,
            _request: &DurableInstallationFinalityRequestV1<ActiveStoreFinalityV1>,
        ) -> Result<Self::Readback, DurableInstallationFinalityErrorV1> {
            if expected != self.expected {
                return Err(DurableInstallationFinalityErrorV1::CurrentnessMismatch);
            }
            self.effects += 1;
            Ok(self.readback)
        }
    }

    fn currentness() -> InstallationFinalityCurrentnessV1 {
        InstallationFinalityCurrentnessV1 {
            installation: [1; 32],
            realm: [2; 32],
            domain: [3; 32],
            store_instance: [4; 32],
            head: [5; 32],
            generation: [6; 32],
            store_cas: [7; 32],
            host_connection: [8; 32],
            host_currentness: [9; 32],
            revocation_revision: 11,
        }
    }

    fn active_decision() -> ActiveStoreDecisionTupleV1 {
        ActiveStoreDecisionTupleV1 {
            operation: [12; 32],
            attempt: [13; 32],
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
            association_identity: [25; 32],
            association_meaning: [26; 32],
            distribution_commit: [27; 32],
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

    fn active_readback() -> ActiveStoreFinalityReadbackV1 {
        ActiveStoreFinalityReadbackV1 {
            currentness: currentness(),
            decision: active_decision(),
            durable_effect: [33; 32],
        }
    }

    #[test]
    fn active_store_owner_effect_and_readback_are_one_typed_operation() {
        let mut backend = ActiveBackendV1 {
            expected: currentness(),
            readback: active_readback(),
            effects: 0,
        };
        let finality = DurableInstallationFinalityBackendV1::capture(&mut backend)
            .consume_active(active_request())
            .unwrap();
        assert_eq!(finality.effect, [33; 32]);
        assert_eq!(backend.effects, 1);
    }

    #[test]
    fn false_success_and_partial_readback_cannot_mint_finality() {
        let mut backend = ActiveBackendV1 {
            expected: currentness(),
            readback: ActiveStoreFinalityReadbackV1 {
                durable_effect: [0; 32],
                ..active_readback()
            },
            effects: 0,
        };
        assert!(matches!(
            DurableInstallationFinalityBackendV1::capture(&mut backend)
                .consume_active(active_request()),
            Err(DurableInstallationFinalityErrorV1::PostconditionMismatch)
        ));
    }

    // PreStore success and refusal are exercised from Persistence with a real
    // lifetime-bound ProtectedLocatorLeaseV1; no digest-like substitute exists.

    #[test]
    fn production_owner_entry_points_are_frozen_for_stage9_and_stage11() {
        let _ = execute_active_from_stage9_owner;
        let _ = execute_pre_store_from_stage11_owner;
    }
}
