use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

pub(super) mod v2_owner_sealed {
    pub trait Sealed {}
}

#[derive(Eq, PartialEq)]
pub(in crate::domain::vnext) struct ProtectedLocatorAcquisitionRequestV2 {
    installation: [u8; 32],
    realm: [u8; 32],
    installation_domain: [u8; 32],
    operation: [u8; 32],
    ceremony_spec: [u8; 32],
    attempt: [u8; 32],
    source_carrier: [u8; 32],
    target: [u8; 32],
    invocation: [u8; 32],
}

impl ProtectedLocatorAcquisitionRequestV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Persistence owner must bind the complete pre-candidate acquisition tuple"
    )]
    pub(in crate::domain::vnext::persistence) fn from_stage9_owner(
        installation: [u8; 32],
        realm: [u8; 32],
        installation_domain: [u8; 32],
        operation: [u8; 32],
        ceremony_spec: [u8; 32],
        attempt: [u8; 32],
        source_carrier: [u8; 32],
        target: [u8; 32],
        invocation: [u8; 32],
    ) -> Result<Self, ProtectedLocatorLeaseErrorV2> {
        let request = Self {
            installation,
            realm,
            installation_domain,
            operation,
            ceremony_spec,
            attempt,
            source_carrier,
            target,
            invocation,
        };
        if [
            request.installation,
            request.realm,
            request.installation_domain,
            request.operation,
            request.ceremony_spec,
            request.attempt,
            request.source_carrier,
            request.target,
            request.invocation,
        ]
        .contains(&[0; 32])
        {
            return Err(ProtectedLocatorLeaseErrorV2::InvalidAcquisition);
        }
        Ok(request)
    }
}

#[derive(Eq, PartialEq)]
pub(in crate::domain::vnext) struct ProtectedLocatorObservedStateV2 {
    facility: [u8; 32],
    provider_incarnation: [u8; 32],
    external_anchor: [u8; 32],
    backend_capability: [u8; 32],
    installation: [u8; 32],
    realm: [u8; 32],
    installation_domain: [u8; 32],
    locator_identity: [u8; 32],
    locator_root: [u8; 32],
    custody_class: [u8; 32],
    observed_carrier: [u8; 32],
    observed_cas: [u8; 32],
    cas_version: u64,
    cas_incarnation: [u8; 32],
    publication_incarnation: [u8; 32],
    restore_incarnation: [u8; 32],
    currentness: [u8; 32],
    state_token: [u8; 32],
    fence: [u8; 32],
    revocation_revision: u64,
}

impl ProtectedLocatorObservedStateV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Persistence owner observation binds the complete locator currentness tuple"
    )]
    pub(in crate::domain::vnext::persistence) fn from_stage9_owner(
        request: &ProtectedLocatorAcquisitionRequestV2,
        facility: [u8; 32],
        provider_incarnation: [u8; 32],
        external_anchor: [u8; 32],
        backend_capability: [u8; 32],
        locator_identity: [u8; 32],
        locator_root: [u8; 32],
        custody_class: [u8; 32],
        observed_cas: [u8; 32],
        cas_version: u64,
        cas_incarnation: [u8; 32],
        publication_incarnation: [u8; 32],
        restore_incarnation: [u8; 32],
        currentness: [u8; 32],
        state_token: [u8; 32],
        fence: [u8; 32],
        revocation_revision: u64,
    ) -> Result<Self, ProtectedLocatorLeaseErrorV2> {
        let observed = Self {
            facility,
            provider_incarnation,
            external_anchor,
            backend_capability,
            installation: request.installation,
            realm: request.realm,
            installation_domain: request.installation_domain,
            locator_identity,
            locator_root,
            custody_class,
            observed_carrier: request.source_carrier,
            observed_cas,
            cas_version,
            cas_incarnation,
            publication_incarnation,
            restore_incarnation,
            currentness,
            state_token,
            fence,
            revocation_revision,
        };
        validate_acquisition(request, &observed)?;
        Ok(observed)
    }
}

#[derive(Eq, PartialEq)]
pub(in crate::domain::vnext) struct ProtectedLocatorCandidateInputV2 {
    installation: [u8; 32],
    realm: [u8; 32],
    ceremony_spec: [u8; 32],
    attempt: [u8; 32],
    candidate_association: [u8; 32],
    candidate_root: [u8; 32],
    candidate_carrier: [u8; 32],
    candidate_seal: [u8; 32],
    candidate_postcondition: [u8; 32],
    invocation: [u8; 32],
}

impl ProtectedLocatorCandidateInputV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the inert Installation candidate tuple is exact and grants no locator or CAS authority"
    )]
    pub(in crate::domain::vnext) fn from_installation_owner(
        installation: [u8; 32],
        realm: [u8; 32],
        ceremony_spec: [u8; 32],
        attempt: [u8; 32],
        candidate_association: [u8; 32],
        candidate_root: [u8; 32],
        candidate_carrier: [u8; 32],
        candidate_seal: [u8; 32],
        candidate_postcondition: [u8; 32],
        invocation: [u8; 32],
    ) -> Result<Self, ProtectedLocatorLeaseErrorV2> {
        if [
            installation,
            realm,
            ceremony_spec,
            attempt,
            candidate_association,
            candidate_root,
            candidate_carrier,
            candidate_seal,
            candidate_postcondition,
            invocation,
        ]
        .contains(&[0; 32])
        {
            return Err(ProtectedLocatorLeaseErrorV2::InvalidCandidate);
        }
        Ok(Self {
            installation,
            realm,
            ceremony_spec,
            attempt,
            candidate_association,
            candidate_root,
            candidate_carrier,
            candidate_seal,
            candidate_postcondition,
            invocation,
        })
    }
}

#[derive(Eq, PartialEq)]
pub(in crate::domain::vnext) struct ProtectedLocatorCandidateStateV2 {
    candidate: ProtectedLocatorCandidateInputV2,
    transition_commitment: [u8; 32],
    dispatch_projection_consumed: Cell<bool>,
}

impl ProtectedLocatorCandidateStateV2 {
    pub(in crate::domain::vnext::persistence) fn from_stage9_owner(
        request: &ProtectedLocatorAcquisitionRequestV2,
        acquisition: &ProtectedLocatorObservedStateV2,
        candidate: ProtectedLocatorCandidateInputV2,
    ) -> Result<Self, ProtectedLocatorLeaseErrorV2> {
        validate_acquisition(request, acquisition)?;
        let transition_commitment = protected_locator_commitment(
            b"maestro.persistence.protected-locator-candidate-transition.v2\0",
            &[
                &request.invocation,
                &acquisition.observed_cas,
                &candidate.candidate_association,
                &candidate.candidate_root,
                &candidate.candidate_carrier,
                &candidate.candidate_seal,
                &candidate.candidate_postcondition,
            ],
        );
        let prepared = Self {
            candidate,
            transition_commitment,
            dispatch_projection_consumed: Cell::new(false),
        };
        validate_candidate(request, acquisition, &prepared)?;
        Ok(prepared)
    }

    pub(in crate::domain::vnext::persistence) fn consume_stage9_dispatch_projection(
        &self,
    ) -> Result<ProtectedLocatorStage9DispatchProjectionV2<'_>, ProtectedLocatorLeaseErrorV2> {
        if self.dispatch_projection_consumed.replace(true) {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        Ok(ProtectedLocatorStage9DispatchProjectionV2 {
            candidate_association: &self.candidate.candidate_association,
            candidate_root: &self.candidate.candidate_root,
            candidate_carrier: &self.candidate.candidate_carrier,
            candidate_seal: &self.candidate.candidate_seal,
            candidate_postcondition: &self.candidate.candidate_postcondition,
            transition_commitment: &self.transition_commitment,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain::vnext::persistence) struct ProtectedLocatorStage9DispatchProjectionV2<
    'candidate,
> {
    candidate_association: &'candidate [u8; 32],
    candidate_root: &'candidate [u8; 32],
    candidate_carrier: &'candidate [u8; 32],
    candidate_seal: &'candidate [u8; 32],
    candidate_postcondition: &'candidate [u8; 32],
    transition_commitment: &'candidate [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ProtectedLocatorStage9DispatchProjectionV2<'_> {
    pub(in crate::domain::vnext::persistence) fn candidate_association(&self) -> &[u8; 32] {
        self.candidate_association
    }

    pub(in crate::domain::vnext::persistence) fn candidate_root(&self) -> &[u8; 32] {
        self.candidate_root
    }

    pub(in crate::domain::vnext::persistence) fn candidate_carrier(&self) -> &[u8; 32] {
        self.candidate_carrier
    }

    pub(in crate::domain::vnext::persistence) fn candidate_seal(&self) -> &[u8; 32] {
        self.candidate_seal
    }

    pub(in crate::domain::vnext::persistence) fn candidate_postcondition(&self) -> &[u8; 32] {
        self.candidate_postcondition
    }

    pub(in crate::domain::vnext::persistence) fn transition_commitment(&self) -> &[u8; 32] {
        self.transition_commitment
    }

    pub(in crate::domain::vnext::persistence) fn matches_exact_owner_effect(
        &self,
        candidate_association: &[u8; 32],
        candidate_root: &[u8; 32],
        candidate_carrier: &[u8; 32],
        candidate_seal: &[u8; 32],
        candidate_postcondition: &[u8; 32],
        transition_commitment: &[u8; 32],
    ) -> bool {
        self.candidate_association == candidate_association
            && self.candidate_root == candidate_root
            && self.candidate_carrier == candidate_carrier
            && self.candidate_seal == candidate_seal
            && self.candidate_postcondition == candidate_postcondition
            && self.transition_commitment == transition_commitment
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ProtectedLocatorDispatchOccurrenceV2 {
    Definite,
    Unknown,
}

#[derive(Eq, PartialEq)]
pub(in crate::domain::vnext) struct ProtectedLocatorFinalReadbackV2 {
    observed: ProtectedLocatorObservedStateV2,
    candidate_root: [u8; 32],
    candidate_carrier: [u8; 32],
    candidate_seal: [u8; 32],
    candidate_postcondition: [u8; 32],
}

impl ProtectedLocatorFinalReadbackV2 {
    pub(in crate::domain::vnext::persistence) fn exact_candidate_from_stage9_owner(
        request: &ProtectedLocatorAcquisitionRequestV2,
        observed: ProtectedLocatorObservedStateV2,
        prepared: &ProtectedLocatorCandidateStateV2,
    ) -> Result<Self, ProtectedLocatorLeaseErrorV2> {
        validate_acquisition(request, &observed)?;
        Ok(Self {
            observed,
            candidate_root: prepared.candidate.candidate_root,
            candidate_carrier: prepared.candidate.candidate_carrier,
            candidate_seal: prepared.candidate.candidate_seal,
            candidate_postcondition: prepared.candidate.candidate_postcondition,
        })
    }
}

pub(in crate::domain::vnext) trait ProtectedLocatorBackendV2:
    v2_owner_sealed::Sealed
{
    fn acquire_pre_candidate(
        &mut self,
    ) -> Result<
        (
            ProtectedLocatorAcquisitionRequestV2,
            ProtectedLocatorObservedStateV2,
        ),
        ProtectedLocatorLeaseErrorV2,
    >;

    fn acquisition_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2>;

    fn prepare_candidate(
        &mut self,
        request: &ProtectedLocatorAcquisitionRequestV2,
        acquisition: &ProtectedLocatorObservedStateV2,
        candidate: ProtectedLocatorCandidateInputV2,
    ) -> Result<ProtectedLocatorCandidateStateV2, ProtectedLocatorLeaseErrorV2>;

    fn pre_dispatch_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2>;

    fn dispatch_exact_transition(
        &mut self,
        expected_old: &ProtectedLocatorObservedStateV2,
        candidate: &ProtectedLocatorCandidateStateV2,
    ) -> Result<ProtectedLocatorDispatchOccurrenceV2, ProtectedLocatorLeaseErrorV2>;

    fn final_readback(
        &mut self,
    ) -> Result<ProtectedLocatorFinalReadbackV2, ProtectedLocatorLeaseErrorV2>;
}

pub(in crate::domain::vnext) struct ProtectedLocatorLeaseV2<'locator> {
    backend: &'locator mut dyn ProtectedLocatorBackendV2,
    request: ProtectedLocatorAcquisitionRequestV2,
    acquisition: ProtectedLocatorObservedStateV2,
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'locator> ProtectedLocatorLeaseV2<'locator> {
    pub(in crate::domain::vnext::persistence) fn acquire(
        backend: &'locator mut dyn ProtectedLocatorBackendV2,
    ) -> Result<Self, ProtectedLocatorLeaseErrorV2> {
        let (request, acquisition) = backend.acquire_pre_candidate()?;
        validate_acquisition(&request, &acquisition)?;
        let rechecked = backend.acquisition_recheck()?;
        if rechecked != acquisition {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        Ok(Self {
            backend,
            request,
            acquisition,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext) fn bind_inert_candidate(
        self,
        candidate: ProtectedLocatorCandidateInputV2,
    ) -> Result<ProtectedLocatorCandidateTransitionV2<'locator>, ProtectedLocatorLeaseErrorV2> {
        if self.consumed.replace(true)
            || candidate.installation != self.request.installation
            || candidate.realm != self.request.realm
            || candidate.ceremony_spec != self.request.ceremony_spec
            || candidate.attempt != self.request.attempt
            || candidate.invocation != self.request.invocation
        {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        let prepared =
            self.backend
                .prepare_candidate(&self.request, &self.acquisition, candidate)?;
        validate_candidate(&self.request, &self.acquisition, &prepared)?;
        Ok(ProtectedLocatorCandidateTransitionV2 {
            backend: self.backend,
            acquisition: self.acquisition,
            prepared,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain::vnext) struct ProtectedLocatorCandidateTransitionV2<'locator> {
    backend: &'locator mut dyn ProtectedLocatorBackendV2,
    acquisition: ProtectedLocatorObservedStateV2,
    prepared: ProtectedLocatorCandidateStateV2,
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ProtectedLocatorCandidateTransitionV2<'_> {
    pub(in crate::domain::vnext) fn dispatch(
        self,
    ) -> Result<ProtectedLocatorFinalityDispositionV2, ProtectedLocatorLeaseErrorV2> {
        if self.consumed.replace(true) {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        let before = self.backend.pre_dispatch_recheck()?;
        if before != self.acquisition {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        let occurrence = self
            .backend
            .dispatch_exact_transition(&self.acquisition, &self.prepared)?;
        let readback = self.backend.final_readback()?;
        classify_finality(&self.acquisition, &self.prepared, occurrence, &readback)
    }
}

fn validate_acquisition(
    request: &ProtectedLocatorAcquisitionRequestV2,
    observed: &ProtectedLocatorObservedStateV2,
) -> Result<(), ProtectedLocatorLeaseErrorV2> {
    if [
        request.installation,
        request.realm,
        request.installation_domain,
        request.operation,
        request.ceremony_spec,
        request.attempt,
        request.source_carrier,
        request.target,
        request.invocation,
        observed.facility,
        observed.provider_incarnation,
        observed.external_anchor,
        observed.backend_capability,
        observed.locator_identity,
        observed.locator_root,
        observed.custody_class,
        observed.observed_carrier,
        observed.observed_cas,
        observed.cas_incarnation,
        observed.publication_incarnation,
        observed.restore_incarnation,
        observed.currentness,
        observed.state_token,
        observed.fence,
    ]
    .contains(&[0; 32])
        || observed.cas_version == 0
        || observed.revocation_revision == 0
    {
        return Err(ProtectedLocatorLeaseErrorV2::InvalidAcquisition);
    }
    if observed.installation != request.installation
        || observed.realm != request.realm
        || observed.installation_domain != request.installation_domain
        || observed.observed_carrier != request.source_carrier
    {
        return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
    }
    Ok(())
}

fn validate_candidate(
    request: &ProtectedLocatorAcquisitionRequestV2,
    acquisition: &ProtectedLocatorObservedStateV2,
    prepared: &ProtectedLocatorCandidateStateV2,
) -> Result<(), ProtectedLocatorLeaseErrorV2> {
    let expected = protected_locator_commitment(
        b"maestro.persistence.protected-locator-candidate-transition.v2\0",
        &[
            &request.invocation,
            &acquisition.observed_cas,
            &prepared.candidate.candidate_association,
            &prepared.candidate.candidate_root,
            &prepared.candidate.candidate_carrier,
            &prepared.candidate.candidate_seal,
            &prepared.candidate.candidate_postcondition,
        ],
    );
    if prepared.candidate.installation != request.installation
        || prepared.candidate.realm != request.realm
        || prepared.candidate.ceremony_spec != request.ceremony_spec
        || prepared.candidate.attempt != request.attempt
        || prepared.candidate.invocation != request.invocation
        || prepared.transition_commitment != expected
    {
        return Err(ProtectedLocatorLeaseErrorV2::InvalidCandidate);
    }
    Ok(())
}

fn classify_finality(
    acquisition: &ProtectedLocatorObservedStateV2,
    prepared: &ProtectedLocatorCandidateStateV2,
    occurrence: ProtectedLocatorDispatchOccurrenceV2,
    readback: &ProtectedLocatorFinalReadbackV2,
) -> Result<ProtectedLocatorFinalityDispositionV2, ProtectedLocatorLeaseErrorV2> {
    if readback.observed.installation != acquisition.installation
        || readback.observed.realm != acquisition.realm
        || readback.observed.locator_identity != acquisition.locator_identity
        || readback.observed.provider_incarnation != acquisition.provider_incarnation
        || readback.observed.external_anchor != acquisition.external_anchor
        || readback.observed.backend_capability != acquisition.backend_capability
    {
        return Ok(ProtectedLocatorFinalityDispositionV2::IntegrityBlocked);
    }
    let exact_candidate = readback.candidate_root == prepared.candidate.candidate_root
        && readback.candidate_carrier == prepared.candidate.candidate_carrier
        && readback.candidate_seal == prepared.candidate.candidate_seal
        && readback.candidate_postcondition == prepared.candidate.candidate_postcondition;
    if exact_candidate {
        return Ok(match occurrence {
            ProtectedLocatorDispatchOccurrenceV2::Definite => {
                ProtectedLocatorFinalityDispositionV2::Committed
            }
            ProtectedLocatorDispatchOccurrenceV2::Unknown => {
                ProtectedLocatorFinalityDispositionV2::RecoveryRequired
            }
        });
    }
    if readback.observed == *acquisition
        && matches!(occurrence, ProtectedLocatorDispatchOccurrenceV2::Definite)
    {
        return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
    }
    Ok(ProtectedLocatorFinalityDispositionV2::InDoubt)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ProtectedLocatorFinalityDispositionV2 {
    Committed,
    RecoveryRequired,
    InDoubt,
    IntegrityBlocked,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ProtectedLocatorLeaseErrorV2 {
    #[error("the protected locator acquisition was invalid")]
    InvalidAcquisition,
    #[error("the protected locator candidate was invalid")]
    InvalidCandidate,
    #[error("the protected locator currentness changed")]
    CurrentnessMismatch,
    #[error("the protected locator capability was replayed")]
    Replay,
    #[error("the protected locator owner backend is unavailable")]
    BackendUnavailable,
}

#[cfg(test)]
pub(in crate::domain::vnext) mod v2_tests {
    use super::*;

    struct BackendV2 {
        substitute_observed_carrier: bool,
        prepared_commitment: Option<[u8; 32]>,
        occurrence: ProtectedLocatorDispatchOccurrenceV2,
        readback: Option<ProtectedLocatorFinalReadbackV2>,
        writes: u64,
    }

    impl v2_owner_sealed::Sealed for BackendV2 {}

    impl ProtectedLocatorBackendV2 for BackendV2 {
        fn acquire_pre_candidate(
            &mut self,
        ) -> Result<
            (
                ProtectedLocatorAcquisitionRequestV2,
                ProtectedLocatorObservedStateV2,
            ),
            ProtectedLocatorLeaseErrorV2,
        > {
            let mut observed = observed_v2();
            if self.substitute_observed_carrier {
                observed.observed_carrier = [99; 32];
            }
            Ok((request_v2(), observed))
        }

        fn acquisition_recheck(
            &mut self,
        ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
            let mut observed = observed_v2();
            if self.substitute_observed_carrier {
                observed.observed_carrier = [99; 32];
            }
            Ok(observed)
        }

        fn prepare_candidate(
            &mut self,
            request: &ProtectedLocatorAcquisitionRequestV2,
            acquisition: &ProtectedLocatorObservedStateV2,
            candidate: ProtectedLocatorCandidateInputV2,
        ) -> Result<ProtectedLocatorCandidateStateV2, ProtectedLocatorLeaseErrorV2> {
            let transition_commitment = protected_locator_commitment(
                b"maestro.persistence.protected-locator-candidate-transition.v2\0",
                &[
                    &request.invocation,
                    &acquisition.observed_cas,
                    &candidate.candidate_association,
                    &candidate.candidate_root,
                    &candidate.candidate_carrier,
                    &candidate.candidate_seal,
                    &candidate.candidate_postcondition,
                ],
            );
            self.prepared_commitment = Some(transition_commitment);
            Ok(ProtectedLocatorCandidateStateV2 {
                candidate,
                transition_commitment,
                dispatch_projection_consumed: Cell::new(false),
            })
        }

        fn pre_dispatch_recheck(
            &mut self,
        ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
            Ok(observed_v2())
        }

        fn dispatch_exact_transition(
            &mut self,
            expected_old: &ProtectedLocatorObservedStateV2,
            candidate: &ProtectedLocatorCandidateStateV2,
        ) -> Result<ProtectedLocatorDispatchOccurrenceV2, ProtectedLocatorLeaseErrorV2> {
            if expected_old != &observed_v2()
                || self.prepared_commitment != Some(candidate.transition_commitment)
            {
                return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
            }
            self.writes += 1;
            Ok(self.occurrence)
        }

        fn final_readback(
            &mut self,
        ) -> Result<ProtectedLocatorFinalReadbackV2, ProtectedLocatorLeaseErrorV2> {
            self.readback
                .take()
                .ok_or(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)
        }
    }

    fn request_v2() -> ProtectedLocatorAcquisitionRequestV2 {
        ProtectedLocatorAcquisitionRequestV2 {
            installation: [1; 32],
            realm: [2; 32],
            installation_domain: [3; 32],
            operation: [4; 32],
            ceremony_spec: [5; 32],
            attempt: [6; 32],
            source_carrier: [7; 32],
            target: [8; 32],
            invocation: [9; 32],
        }
    }

    fn observed_v2() -> ProtectedLocatorObservedStateV2 {
        ProtectedLocatorObservedStateV2 {
            facility: [10; 32],
            provider_incarnation: [11; 32],
            external_anchor: [12; 32],
            backend_capability: [13; 32],
            installation: [1; 32],
            realm: [2; 32],
            installation_domain: [3; 32],
            locator_identity: [14; 32],
            locator_root: [15; 32],
            custody_class: [16; 32],
            observed_carrier: [7; 32],
            observed_cas: [17; 32],
            cas_version: 18,
            cas_incarnation: [19; 32],
            publication_incarnation: [20; 32],
            restore_incarnation: [21; 32],
            currentness: [22; 32],
            state_token: [23; 32],
            fence: [24; 32],
            revocation_revision: 25,
        }
    }

    fn candidate_v2() -> ProtectedLocatorCandidateInputV2 {
        ProtectedLocatorCandidateInputV2::from_installation_owner(
            [1; 32], [2; 32], [5; 32], [6; 32], [26; 32], [27; 32], [28; 32], [29; 32], [30; 32],
            [9; 32],
        )
        .unwrap()
    }

    fn backend_v2(occurrence: ProtectedLocatorDispatchOccurrenceV2) -> BackendV2 {
        let candidate = candidate_v2();
        BackendV2 {
            substitute_observed_carrier: false,
            prepared_commitment: None,
            occurrence,
            readback: Some(ProtectedLocatorFinalReadbackV2 {
                observed: observed_v2(),
                candidate_root: candidate.candidate_root,
                candidate_carrier: candidate.candidate_carrier,
                candidate_seal: candidate.candidate_seal,
                candidate_postcondition: candidate.candidate_postcondition,
            }),
            writes: 0,
        }
    }

    pub(in crate::domain::vnext) fn with_test_lease_v2<R>(
        callback: impl for<'locator> FnOnce(ProtectedLocatorLeaseV2<'locator>) -> R,
    ) -> (R, u64) {
        let mut backend = backend_v2(ProtectedLocatorDispatchOccurrenceV2::Definite);
        let lease = ProtectedLocatorLeaseV2::acquire(&mut backend).unwrap();
        let output = callback(lease);
        (output, backend.writes)
    }

    #[test]
    pub(super) fn v2_acquires_before_candidate_and_retains_the_backend_through_finality() {
        let mut backend = backend_v2(ProtectedLocatorDispatchOccurrenceV2::Definite);
        let result = ProtectedLocatorLeaseV2::acquire(&mut backend)
            .and_then(|lease| lease.bind_inert_candidate(candidate_v2()))
            .and_then(ProtectedLocatorCandidateTransitionV2::dispatch);
        assert_eq!(result, Ok(ProtectedLocatorFinalityDispositionV2::Committed));
        assert_eq!(backend.writes, 1);
    }

    #[test]
    pub(super) fn v2_rejects_pre_candidate_substitution_and_classifies_lost_acknowledgement() {
        let mut substituted = backend_v2(ProtectedLocatorDispatchOccurrenceV2::Definite);
        substituted.substitute_observed_carrier = true;
        assert!(matches!(
            ProtectedLocatorLeaseV2::acquire(&mut substituted),
            Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)
        ));
        assert_eq!(substituted.writes, 0);

        let mut unknown = backend_v2(ProtectedLocatorDispatchOccurrenceV2::Unknown);
        assert_eq!(
            ProtectedLocatorLeaseV2::acquire(&mut unknown)
                .and_then(|lease| lease.bind_inert_candidate(candidate_v2()))
                .and_then(ProtectedLocatorCandidateTransitionV2::dispatch),
            Ok(ProtectedLocatorFinalityDispositionV2::RecoveryRequired)
        );
    }
}

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
    #[cfg(test)]
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
        super::v2_tests::v2_acquires_before_candidate_and_retains_the_backend_through_finality();
        super::v2_tests::v2_rejects_pre_candidate_substitution_and_classifies_lost_acknowledgement(
        );
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
        assert!(crate::domain::vnext::installation::consume_pre_store_with_test_owner(lease, 0));
        assert_eq!(backend.dispatches, 1);
    }

    #[test]
    fn pre_store_false_success_with_a_write_is_rejected_through_the_same_live_lease() {
        let mut backend = backend();
        let lease = ProtectedLocatorLeaseV1::acquire(&mut backend, request()).unwrap();
        assert!(!crate::domain::vnext::installation::consume_pre_store_with_test_owner(lease, 1));
        assert_eq!(backend.dispatches, 0);
    }
}
