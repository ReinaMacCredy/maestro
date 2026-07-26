use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Copy)]
pub(in crate::domain::vnext) struct ProtectedLocatorOperationRequestV1 {
    installation: [u8; 32],
    realm: [u8; 32],
    installation_domain: [u8; 32],
    operation: [u8; 32],
    ceremony: [u8; 32],
    attempt: [u8; 32],
    source_carrier: [u8; 32],
    candidate_carrier: [u8; 32],
    candidate_association: [u8; 32],
    target: [u8; 32],
    invocation: [u8; 32],
    source_carrier_identity: [u8; 32],
    candidate_carrier_identity: [u8; 32],
}

impl ProtectedLocatorOperationRequestV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Installation request is a closed nominal tuple and deliberately carries no path, root or CAS evidence"
    )]
    pub(in crate::domain::vnext) fn from_installation_operation(
        installation: [u8; 32],
        realm: [u8; 32],
        installation_domain: [u8; 32],
        operation: [u8; 32],
        ceremony: [u8; 32],
        attempt: [u8; 32],
        source_carrier: [u8; 32],
        candidate_carrier: [u8; 32],
        candidate_association: [u8; 32],
        target: [u8; 32],
    ) -> Result<Self, ProtectedLocatorLeaseErrorV1> {
        let values = [
            installation,
            realm,
            installation_domain,
            operation,
            ceremony,
            attempt,
            source_carrier,
            candidate_carrier,
            candidate_association,
            target,
        ];
        if values.contains(&[0; 32]) {
            return Err(ProtectedLocatorLeaseErrorV1::InvalidRequest);
        }
        let invocation = protected_locator_commitment(
            b"maestro.persistence.protected-locator-invocation.v1\0",
            &[&installation, &operation, &ceremony, &attempt, &target],
        );
        let source_carrier_identity = protected_locator_commitment(
            b"maestro.persistence.protected-locator-source-carrier.v1\0",
            &[&source_carrier],
        );
        let candidate_carrier_identity = protected_locator_commitment(
            b"maestro.persistence.protected-locator-candidate-carrier.v1\0",
            &[&candidate_carrier],
        );
        Ok(Self {
            installation,
            realm,
            installation_domain,
            operation,
            ceremony,
            attempt,
            source_carrier,
            candidate_carrier,
            candidate_association,
            target,
            invocation,
            source_carrier_identity,
            candidate_carrier_identity,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ProtectedLocatorObservedStateV1 {
    pub(super) facility: [u8; 32],
    pub(super) provider_incarnation: [u8; 32],
    pub(super) external_anchor: [u8; 32],
    pub(super) backend_handle: [u8; 32],
    pub(super) installation: [u8; 32],
    pub(super) realm: [u8; 32],
    pub(super) installation_domain: [u8; 32],
    pub(super) locator_identity: [u8; 32],
    pub(super) locator_slot: [u8; 32],
    pub(super) custody_class: [u8; 32],
    pub(super) protected_root: [u8; 32],
    pub(super) observed_cas: [u8; 32],
    pub(super) cas_version: u64,
    pub(super) cas_incarnation: [u8; 32],
    pub(super) source_carrier: [u8; 32],
    pub(super) source_carrier_identity: [u8; 32],
    pub(super) candidate_carrier: [u8; 32],
    pub(super) candidate_carrier_identity: [u8; 32],
    pub(super) operation: [u8; 32],
    pub(super) invocation: [u8; 32],
    pub(super) ceremony: [u8; 32],
    pub(super) attempt: [u8; 32],
    pub(super) candidate_association: [u8; 32],
    pub(super) publication_incarnation: [u8; 32],
    pub(super) restore_incarnation: [u8; 32],
    pub(super) currentness: [u8; 32],
    pub(super) anti_rollback_epoch: u64,
    pub(super) state_token: [u8; 32],
    pub(super) fence: [u8; 32],
    pub(super) revocation_revision: u64,
    pub(super) target: [u8; 32],
}

impl ProtectedLocatorObservedStateV1 {
    fn validate(
        self,
        request: ProtectedLocatorOperationRequestV1,
    ) -> Result<(), ProtectedLocatorLeaseErrorV1> {
        let commitments = [
            self.facility,
            self.provider_incarnation,
            self.external_anchor,
            self.backend_handle,
            self.installation,
            self.realm,
            self.installation_domain,
            self.locator_identity,
            self.locator_slot,
            self.custody_class,
            self.protected_root,
            self.observed_cas,
            self.cas_incarnation,
            self.source_carrier,
            self.source_carrier_identity,
            self.candidate_carrier,
            self.candidate_carrier_identity,
            self.operation,
            self.invocation,
            self.ceremony,
            self.attempt,
            self.candidate_association,
            self.publication_incarnation,
            self.restore_incarnation,
            self.currentness,
            self.state_token,
            self.fence,
            self.target,
        ];
        if commitments.contains(&[0; 32])
            || self.cas_version == 0
            || self.anti_rollback_epoch == 0
            || self.revocation_revision == 0
            || self.installation != request.installation
            || self.realm != request.realm
            || self.installation_domain != request.installation_domain
            || request.operation == [0; 32]
            || self.source_carrier != request.source_carrier
            || self.source_carrier_identity != request.source_carrier_identity
            || self.candidate_carrier != request.candidate_carrier
            || self.candidate_carrier_identity != request.candidate_carrier_identity
            || self.operation != request.operation
            || self.invocation != request.invocation
            || self.ceremony != request.ceremony
            || self.attempt != request.attempt
            || self.candidate_association != request.candidate_association
            || self.target != request.target
        {
            return Err(ProtectedLocatorLeaseErrorV1::CurrentnessMismatch);
        }
        Ok(())
    }
}

pub(super) mod owner_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext::persistence) trait ProtectedLocatorBackendV1:
    owner_sealed::Sealed
{
    fn observe_no_follow(
        &mut self,
        request: ProtectedLocatorOperationRequestV1,
    ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1>;

    fn pre_dispatch_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1>;

    fn dispatch_expected_old(
        &mut self,
        expected_old: [u8; 32],
        candidate: &ProtectedLocatorCandidateTransitionV1,
    ) -> Result<ProtectedLocatorDispatchOccurrenceV1, ProtectedLocatorLeaseErrorV1>;

    fn prepare_candidate_transition(
        &mut self,
        request: ProtectedLocatorOperationRequestV1,
    ) -> Result<ProtectedLocatorCandidateTransitionV1, ProtectedLocatorLeaseErrorV1>;

    fn final_readback(
        &mut self,
    ) -> Result<ProtectedLocatorFinalReadbackV1, ProtectedLocatorLeaseErrorV1>;
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ProtectedLocatorDispatchOccurrenceV1 {
    Definite,
    Unknown,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ProtectedLocatorFinalReadbackV1 {
    pub(super) state: ProtectedLocatorObservedStateV1,
    pub(super) root: [u8; 32],
    pub(super) carrier: [u8; 32],
    pub(super) seal_valid: bool,
    pub(super) definite_non_occurrence: bool,
    pub(super) no_late_apply: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ProtectedLocatorCandidateTransitionV1 {
    pub(super) root: [u8; 32],
    pub(super) seal: [u8; 32],
    pub(super) carrier_identity: [u8; 32],
    pub(super) carrier_commitment: [u8; 32],
    pub(super) association: [u8; 32],
    pub(super) ceremony: [u8; 32],
    pub(super) attempt: [u8; 32],
    pub(super) invocation: [u8; 32],
}

pub(in crate::domain::vnext) struct ProtectedLocatorLeaseV1<'locator> {
    backend: &'locator mut dyn ProtectedLocatorBackendV1,
    request: ProtectedLocatorOperationRequestV1,
    observed: ProtectedLocatorObservedStateV1,
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'locator> ProtectedLocatorLeaseV1<'locator> {
    pub(in crate::domain::vnext::persistence) fn acquire(
        backend: &'locator mut dyn ProtectedLocatorBackendV1,
        request: ProtectedLocatorOperationRequestV1,
    ) -> Result<Self, ProtectedLocatorLeaseErrorV1> {
        let observed = backend.observe_no_follow(request)?;
        observed.validate(request)?;
        Ok(Self {
            backend,
            request,
            observed,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }

    fn consume_finality(
        self,
    ) -> Result<ProtectedLocatorFinalityDispositionV1, ProtectedLocatorLeaseErrorV1> {
        let join = ProtectedLocatorPreStoreJoinV1::from_lease(&self);
        self.begin_pre_store(join)?.dispatch()
    }

    pub(in crate::domain::vnext) fn begin_pre_store(
        self,
        join: ProtectedLocatorPreStoreJoinV1,
    ) -> Result<ProtectedLocatorCeremonyContinuationV1<'locator>, ProtectedLocatorLeaseErrorV1>
    {
        if self.consumed.get() || !join.matches(&self) {
            return Err(ProtectedLocatorLeaseErrorV1::CapabilityMismatch);
        }
        let before_dispatch = self.backend.pre_dispatch_recheck()?;
        if before_dispatch != self.observed {
            return Err(ProtectedLocatorLeaseErrorV1::Changed);
        }
        Ok(ProtectedLocatorCeremonyContinuationV1 {
            lease: self,
            _not_send_or_sync: PhantomData,
        })
    }

    fn stable_binding_matches(&self, current: ProtectedLocatorObservedStateV1) -> bool {
        current.facility == self.observed.facility
            && current.provider_incarnation == self.observed.provider_incarnation
            && current.external_anchor == self.observed.external_anchor
            && current.backend_handle == self.observed.backend_handle
            && current.installation == self.observed.installation
            && current.realm == self.observed.realm
            && current.installation_domain == self.observed.installation_domain
            && current.locator_identity == self.observed.locator_identity
            && current.locator_slot == self.observed.locator_slot
            && current.custody_class == self.observed.custody_class
            && current.source_carrier == self.observed.source_carrier
            && current.source_carrier_identity == self.observed.source_carrier_identity
            && current.candidate_carrier == self.observed.candidate_carrier
            && current.candidate_carrier_identity == self.observed.candidate_carrier_identity
            && current.operation == self.observed.operation
            && current.invocation == self.observed.invocation
            && current.ceremony == self.observed.ceremony
            && current.attempt == self.observed.attempt
            && current.candidate_association == self.observed.candidate_association
            && current.publication_incarnation == self.observed.publication_incarnation
            && current.restore_incarnation == self.observed.restore_incarnation
            && current.currentness == self.observed.currentness
            && current.anti_rollback_epoch == self.observed.anti_rollback_epoch
            && current.state_token == self.observed.state_token
            && current.fence == self.observed.fence
            && current.revocation_revision == self.observed.revocation_revision
            && current.target == self.observed.target
    }
}

#[derive(Clone, Copy)]
pub(in crate::domain::vnext) struct ProtectedLocatorPreStoreJoinV1 {
    installation: [u8; 32],
    realm: [u8; 32],
    installation_domain: [u8; 32],
    operation: [u8; 32],
    ceremony: [u8; 32],
    attempt: [u8; 32],
    facility: [u8; 32],
    locator_identity: [u8; 32],
    candidate_association: [u8; 32],
    invocation: [u8; 32],
}

pub(in crate::domain::vnext) struct ProtectedLocatorOperationJoinV1 {
    operation: [u8; 32],
    ceremony: [u8; 32],
    attempt: [u8; 32],
    candidate_association: [u8; 32],
    target: [u8; 32],
}

impl ProtectedLocatorOperationJoinV1 {
    pub(in crate::domain::vnext) fn new(
        operation: [u8; 32],
        ceremony: [u8; 32],
        attempt: [u8; 32],
        candidate_association: [u8; 32],
        target: [u8; 32],
    ) -> Self {
        Self {
            operation,
            ceremony,
            attempt,
            candidate_association,
            target,
        }
    }
}

pub(in crate::domain::vnext) struct ProtectedLocatorOwnerJoinV1 {
    installation: [u8; 32],
    realm: [u8; 32],
    installation_domain: [u8; 32],
    facility: [u8; 32],
    locator_identity: [u8; 32],
}

impl ProtectedLocatorOwnerJoinV1 {
    pub(in crate::domain::vnext) fn new(
        installation: [u8; 32],
        realm: [u8; 32],
        installation_domain: [u8; 32],
        facility: [u8; 32],
        locator_identity: [u8; 32],
    ) -> Self {
        Self {
            installation,
            realm,
            installation_domain,
            facility,
            locator_identity,
        }
    }
}

impl ProtectedLocatorPreStoreJoinV1 {
    fn from_lease(lease: &ProtectedLocatorLeaseV1<'_>) -> Self {
        Self {
            installation: lease.request.installation,
            realm: lease.request.realm,
            installation_domain: lease.request.installation_domain,
            operation: lease.request.operation,
            ceremony: lease.request.ceremony,
            attempt: lease.request.attempt,
            facility: lease.observed.facility,
            locator_identity: lease.observed.locator_identity,
            candidate_association: lease.request.candidate_association,
            invocation: lease.request.invocation,
        }
    }

    pub(in crate::domain::vnext) fn from_installation_owner(
        operation: ProtectedLocatorOperationJoinV1,
        owner: ProtectedLocatorOwnerJoinV1,
    ) -> Result<Self, ProtectedLocatorLeaseErrorV1> {
        let invocation = protected_locator_commitment(
            b"maestro.persistence.protected-locator-invocation.v1\0",
            &[
                &owner.installation,
                &operation.operation,
                &operation.ceremony,
                &operation.attempt,
                &operation.target,
            ],
        );
        let value = Self {
            installation: owner.installation,
            realm: owner.realm,
            installation_domain: owner.installation_domain,
            operation: operation.operation,
            ceremony: operation.ceremony,
            attempt: operation.attempt,
            facility: owner.facility,
            locator_identity: owner.locator_identity,
            candidate_association: operation.candidate_association,
            invocation,
        };
        if [
            operation.operation,
            operation.ceremony,
            operation.attempt,
            operation.candidate_association,
            operation.target,
            owner.installation,
            owner.realm,
            owner.installation_domain,
            owner.facility,
            owner.locator_identity,
        ]
        .contains(&[0; 32])
        {
            return Err(ProtectedLocatorLeaseErrorV1::CapabilityMismatch);
        }
        Ok(value)
    }

    fn matches(self, lease: &ProtectedLocatorLeaseV1<'_>) -> bool {
        self == Self::from_lease(lease)
    }
}

impl Eq for ProtectedLocatorPreStoreJoinV1 {}

impl PartialEq for ProtectedLocatorPreStoreJoinV1 {
    fn eq(&self, other: &Self) -> bool {
        self.installation == other.installation
            && self.realm == other.realm
            && self.installation_domain == other.installation_domain
            && self.operation == other.operation
            && self.ceremony == other.ceremony
            && self.attempt == other.attempt
            && self.facility == other.facility
            && self.locator_identity == other.locator_identity
            && self.candidate_association == other.candidate_association
            && self.invocation == other.invocation
    }
}

pub(in crate::domain::vnext) struct ProtectedLocatorCeremonyContinuationV1<'locator> {
    lease: ProtectedLocatorLeaseV1<'locator>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ProtectedLocatorCeremonyContinuationV1<'_> {
    pub(in crate::domain::vnext) fn dispatch(
        self,
    ) -> Result<ProtectedLocatorFinalityDispositionV1, ProtectedLocatorLeaseErrorV1> {
        let lease = self.lease;
        if lease.consumed.replace(true) {
            return Err(ProtectedLocatorLeaseErrorV1::CapabilityMismatch);
        }
        let candidate = lease.backend.prepare_candidate_transition(lease.request)?;
        if candidate.root == [0; 32]
            || candidate.seal == [0; 32]
            || candidate.carrier_identity != lease.request.candidate_carrier_identity
            || candidate.carrier_commitment != lease.request.candidate_carrier
            || candidate.association != lease.request.candidate_association
            || candidate.ceremony != lease.request.ceremony
            || candidate.attempt != lease.request.attempt
            || candidate.invocation != lease.request.invocation
        {
            return Err(ProtectedLocatorLeaseErrorV1::CapabilityMismatch);
        }
        let before_dispatch = lease.backend.pre_dispatch_recheck()?;
        if before_dispatch != lease.observed {
            return Err(ProtectedLocatorLeaseErrorV1::Changed);
        }
        let occurrence = lease
            .backend
            .dispatch_expected_old(lease.observed.observed_cas, &candidate)?;
        let readback = lease.backend.final_readback()?;
        if !lease.stable_binding_matches(readback.state) {
            return Ok(ProtectedLocatorFinalityDispositionV1::IntegrityBlocked);
        }
        if readback.root == candidate.root
            && readback.carrier == candidate.carrier_commitment
            && readback.seal_valid
        {
            return Ok(ProtectedLocatorFinalityDispositionV1::Committed);
        }
        if readback.root != lease.observed.protected_root
            || readback.carrier != lease.request.source_carrier
            || readback.seal_valid
        {
            return Ok(ProtectedLocatorFinalityDispositionV1::IntegrityBlocked);
        }
        if occurrence == ProtectedLocatorDispatchOccurrenceV1::Unknown {
            return Ok(ProtectedLocatorFinalityDispositionV1::InDoubt);
        }
        if readback.definite_non_occurrence && readback.no_late_apply {
            return Ok(ProtectedLocatorFinalityDispositionV1::RecoveryRequired);
        }
        Ok(ProtectedLocatorFinalityDispositionV1::IntegrityBlocked)
    }
}

fn protected_locator_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ProtectedLocatorFinalityDispositionV1 {
    Committed,
    RecoveryRequired,
    InDoubt,
    IntegrityBlocked,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ProtectedLocatorLeaseErrorV1 {
    #[error("protected locator request is invalid")]
    InvalidRequest,
    #[error("protected locator currentness does not match the owner operation")]
    CurrentnessMismatch,
    #[error("protected locator changed")]
    Changed,
    #[error("protected locator capability does not match the finality operation")]
    CapabilityMismatch,
    #[error("protected locator provider is unavailable")]
    ProviderUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBackendV1 {
        acquisition: ProtectedLocatorObservedStateV1,
        before_dispatch: ProtectedLocatorObservedStateV1,
        final_readback: ProtectedLocatorFinalReadbackV1,
        occurrence: ProtectedLocatorDispatchOccurrenceV1,
        candidate: ProtectedLocatorCandidateTransitionV1,
        dispatches: u64,
    }

    impl owner_sealed::Sealed for TestBackendV1 {}

    impl ProtectedLocatorBackendV1 for TestBackendV1 {
        fn observe_no_follow(
            &mut self,
            _request: ProtectedLocatorOperationRequestV1,
        ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1> {
            Ok(self.acquisition)
        }

        fn pre_dispatch_recheck(
            &mut self,
        ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1> {
            Ok(self.before_dispatch)
        }

        fn dispatch_expected_old(
            &mut self,
            expected_old: [u8; 32],
            candidate: &ProtectedLocatorCandidateTransitionV1,
        ) -> Result<ProtectedLocatorDispatchOccurrenceV1, ProtectedLocatorLeaseErrorV1> {
            if expected_old != self.acquisition.observed_cas || candidate != &self.candidate {
                return Err(ProtectedLocatorLeaseErrorV1::CurrentnessMismatch);
            }
            self.dispatches += 1;
            Ok(self.occurrence)
        }

        fn prepare_candidate_transition(
            &mut self,
            _request: ProtectedLocatorOperationRequestV1,
        ) -> Result<ProtectedLocatorCandidateTransitionV1, ProtectedLocatorLeaseErrorV1> {
            Ok(self.candidate)
        }

        fn final_readback(
            &mut self,
        ) -> Result<ProtectedLocatorFinalReadbackV1, ProtectedLocatorLeaseErrorV1> {
            Ok(self.final_readback)
        }
    }

    fn request() -> ProtectedLocatorOperationRequestV1 {
        ProtectedLocatorOperationRequestV1::from_installation_operation(
            [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32],
            [10; 32],
        )
        .unwrap()
    }

    fn observed() -> ProtectedLocatorObservedStateV1 {
        ProtectedLocatorObservedStateV1 {
            facility: [11; 32],
            provider_incarnation: [12; 32],
            external_anchor: [13; 32],
            backend_handle: [14; 32],
            installation: [1; 32],
            realm: [2; 32],
            installation_domain: [3; 32],
            locator_identity: [15; 32],
            locator_slot: [16; 32],
            custody_class: [17; 32],
            protected_root: [18; 32],
            observed_cas: [19; 32],
            cas_version: 20,
            cas_incarnation: [21; 32],
            source_carrier: [7; 32],
            source_carrier_identity: request().source_carrier_identity,
            candidate_carrier: [8; 32],
            candidate_carrier_identity: request().candidate_carrier_identity,
            operation: [4; 32],
            invocation: request().invocation,
            ceremony: [5; 32],
            attempt: [6; 32],
            candidate_association: [9; 32],
            publication_incarnation: [22; 32],
            restore_incarnation: [23; 32],
            currentness: [24; 32],
            anti_rollback_epoch: 25,
            state_token: [26; 32],
            fence: [27; 32],
            revocation_revision: 28,
            target: [10; 32],
        }
    }

    fn backend() -> TestBackendV1 {
        let state = observed();
        TestBackendV1 {
            acquisition: state,
            before_dispatch: state,
            final_readback: ProtectedLocatorFinalReadbackV1 {
                state,
                root: [29; 32],
                carrier: [8; 32],
                seal_valid: true,
                definite_non_occurrence: false,
                no_late_apply: false,
            },
            occurrence: ProtectedLocatorDispatchOccurrenceV1::Definite,
            candidate: ProtectedLocatorCandidateTransitionV1 {
                root: [29; 32],
                seal: [30; 32],
                carrier_identity: request().candidate_carrier_identity,
                carrier_commitment: [8; 32],
                association: [9; 32],
                ceremony: [5; 32],
                attempt: [6; 32],
                invocation: request().invocation,
            },
            dispatches: 0,
        }
    }

    #[test]
    fn owner_observes_and_rereads_the_same_locator_through_finality() {
        let mut backend = backend();
        backend.final_readback.state.protected_root = [29; 32];
        backend.final_readback.state.observed_cas = [41; 32];
        backend.final_readback.state.cas_version += 1;
        let lease = ProtectedLocatorLeaseV1::acquire(&mut backend, request()).unwrap();
        assert!(matches!(
            lease.consume_finality(),
            Ok(ProtectedLocatorFinalityDispositionV1::Committed)
        ));
        assert_eq!(backend.dispatches, 1);
    }

    #[test]
    fn stale_pre_dispatch_tuple_refuses_without_cas() {
        let mut backend = backend();
        backend.before_dispatch.restore_incarnation = [31; 32];
        let lease = ProtectedLocatorLeaseV1::acquire(&mut backend, request()).unwrap();
        assert!(matches!(
            lease.consume_finality(),
            Err(ProtectedLocatorLeaseErrorV1::Changed)
        ));
        assert_eq!(backend.dispatches, 0);
    }

    #[test]
    fn unknown_and_old_root_readback_preserve_recovery_laws() {
        let mut unknown = backend();
        unknown.occurrence = ProtectedLocatorDispatchOccurrenceV1::Unknown;
        unknown.final_readback.root = unknown.acquisition.protected_root;
        unknown.final_readback.carrier = unknown.acquisition.source_carrier;
        unknown.final_readback.seal_valid = false;
        let lease = ProtectedLocatorLeaseV1::acquire(&mut unknown, request()).unwrap();
        assert!(matches!(
            lease.consume_finality(),
            Ok(ProtectedLocatorFinalityDispositionV1::InDoubt)
        ));

        let mut not_applied = backend();
        not_applied.final_readback.root = not_applied.acquisition.protected_root;
        not_applied.final_readback.carrier = not_applied.acquisition.source_carrier;
        not_applied.final_readback.seal_valid = false;
        not_applied.final_readback.definite_non_occurrence = true;
        not_applied.final_readback.no_late_apply = true;
        let lease = ProtectedLocatorLeaseV1::acquire(&mut not_applied, request()).unwrap();
        assert!(matches!(
            lease.consume_finality(),
            Ok(ProtectedLocatorFinalityDispositionV1::RecoveryRequired)
        ));
    }

    #[test]
    fn pre_store_hands_the_live_lease_to_the_ceremony_cas_continuation() {
        let mut backend = backend();
        let lease = ProtectedLocatorLeaseV1::acquire(&mut backend, request()).unwrap();
        assert!(
            crate::domain::vnext::installation::durable_finality::consume_pre_store_with_test_owner(
                lease, 0
            )
        );
        assert_eq!(backend.dispatches, 1);
    }

    #[test]
    fn pre_store_false_success_with_a_write_is_rejected_through_the_same_live_lease() {
        let mut backend = backend();
        let lease = ProtectedLocatorLeaseV1::acquire(&mut backend, request()).unwrap();
        assert!(
            !crate::domain::vnext::installation::durable_finality::consume_pre_store_with_test_owner(
                lease, 1
            )
        );
        assert_eq!(backend.dispatches, 0);
    }
}
