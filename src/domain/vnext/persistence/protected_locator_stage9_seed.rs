use sha2::{Digest, Sha256};

use crate::domain::vnext::distribution::runtime::{
    CanonicalTargetIdentityV1, DistributionDomainKindV1, DistributionTransactionV1,
};
use crate::domain::vnext::execution::{
    CeremonyRequestModeV1, HomeTokenV1, ProtectedCeremonyCarrierAnchorV1,
    ProtectedCeremonyEffectCarrierV1, ProtectedCeremonyEffectErrorV1,
    ProtectedCeremonyEffectOutcomeV1, ProtectedCeremonyEffectPhaseV1,
    ProtectedCeremonyEffectStoreV1, ProtectedCeremonyOwnerAuthorityV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::protected_locator_lease::{
    ProtectedLocatorAcquisitionRequestV2, ProtectedLocatorBackendV2,
    ProtectedLocatorCandidateInputV2, ProtectedLocatorCandidateStateV2,
    ProtectedLocatorDispatchOccurrenceV2, ProtectedLocatorFinalReadbackV2,
    ProtectedLocatorLeaseErrorV2, ProtectedLocatorLeaseV2, ProtectedLocatorObservedStateV2,
    v2_owner_sealed,
};
#[cfg(test)]
use super::protected_locator_lease::{
    ProtectedLocatorBackendV1, ProtectedLocatorCandidateTransitionV1,
    ProtectedLocatorDispatchOccurrenceV1, ProtectedLocatorFinalReadbackV1,
    ProtectedLocatorLeaseErrorV1, ProtectedLocatorLeaseV1, ProtectedLocatorObservedStateV1,
    ProtectedLocatorOperationRequestV1, owner_sealed,
};

pub(in crate::domain::vnext) struct Stage9ProtectedLocatorBackendSeedV2<'locator> {
    store: &'locator ProtectedCeremonyEffectStoreV1,
    anchor: &'locator ProtectedCeremonyCarrierAnchorV1,
    owner_authority: &'locator ProtectedCeremonyOwnerAuthorityV1,
    installation: [u8; 32],
    installation_domain: [u8; 32],
    operation: [u8; 32],
    ceremony_spec: [u8; 32],
    attempt: [u8; 32],
    source_carrier: [u8; 32],
    target: [u8; 32],
    invocation: [u8; 32],
    target_manager_realm: [u8; 32],
    target_security_realm: [u8; 32],
    acquisition_cas_version: u64,
    expected_candidate_root: [u8; 32],
    expected_candidate_seal: [u8; 32],
    acquisition_carrier: ProtectedCeremonyEffectCarrierV1,
    idempotency_key: HomeTokenV1,
    dispatched_candidate: Option<Stage9DispatchedCandidateV2>,
    dispatch_outcome: Option<ProtectedCeremonyEffectOutcomeV1>,
    dispatch_attempted: bool,
}

#[derive(Clone, Copy)]
struct Stage9DispatchedCandidateV2 {
    association: [u8; 32],
    root: [u8; 32],
    carrier: [u8; 32],
    seal: [u8; 32],
    postcondition: [u8; 32],
    transition_commitment: [u8; 32],
}

impl v2_owner_sealed::Sealed for Stage9ProtectedLocatorBackendSeedV2<'_> {}

impl Stage9ProtectedLocatorBackendSeedV2<'_> {
    fn request(
        &self,
    ) -> Result<ProtectedLocatorAcquisitionRequestV2, ProtectedLocatorLeaseErrorV2> {
        ProtectedLocatorAcquisitionRequestV2::from_stage9_owner(
            self.installation,
            *self.store.realm().as_bytes(),
            self.installation_domain,
            self.operation,
            self.ceremony_spec,
            self.attempt,
            self.source_carrier,
            self.target,
            self.invocation,
        )
    }

    fn observe_live(
        &self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
        let carrier = self.store.current().map_err(map_observation_error)?;
        self.state_for(carrier)
    }

    fn state_for(
        &self,
        carrier: ProtectedCeremonyEffectCarrierV1,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
        let request = self.request()?;
        if !anchor_matches_store(self.anchor, self.store) {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        let anchor_bytes = self
            .anchor
            .canonical_bytes()
            .map_err(|_| ProtectedLocatorLeaseErrorV2::BackendUnavailable)?;
        let realm = *self.store.realm().as_bytes();
        let facility = tuple_commitment(
            b"maestro.vnext.stage9-protected-facility.v2",
            &[&anchor_bytes, &realm],
        );
        let provider_incarnation = tuple_commitment(
            b"maestro.vnext.stage9-protected-provider-incarnation.v2",
            &[&anchor_bytes],
        );
        let external_anchor: [u8; 32] = Sha256::digest(&anchor_bytes).into();
        let backend_capability = tuple_commitment(
            b"maestro.vnext.stage9-protected-backend-capability.v2",
            &[&facility, &provider_incarnation],
        );
        let locator_identity = tuple_commitment(
            b"maestro.vnext.stage9-protected-locator-identity.v2",
            &[&external_anchor, &self.target],
        );
        let custody_class = tuple_commitment(
            b"maestro.vnext.stage9-protected-custody-class.v2",
            &[&self.target_manager_realm, &self.target_security_realm],
        );
        let cas_incarnation = tuple_commitment(
            b"maestro.vnext.stage9-protected-cas-incarnation.v2",
            &[&provider_incarnation, &external_anchor],
        );
        let publication_incarnation = tuple_commitment(
            b"maestro.vnext.stage9-protected-publication-incarnation.v2",
            &[&self.operation, &self.attempt, &self.invocation],
        );
        let restore_incarnation = tuple_commitment(
            b"maestro.vnext.stage9-protected-restore-incarnation.v2",
            &[&anchor_bytes, &realm],
        );
        let currentness = tuple_commitment(
            b"maestro.vnext.stage9-protected-currentness.v2",
            &[
                &facility,
                &provider_incarnation,
                &external_anchor,
                &self.source_carrier,
                &self.acquisition_cas_version.to_be_bytes(),
            ],
        );
        let state_token = tuple_commitment(
            b"maestro.vnext.stage9-protected-state-token.v2",
            &[&currentness, &self.source_carrier],
        );
        let fence = tuple_commitment(
            b"maestro.vnext.stage9-protected-fence.v2",
            &[&self.operation, &self.attempt, &state_token],
        );
        let revocation_revision = if carrier.revision() < self.acquisition_cas_version {
            carrier.revision()
        } else {
            self.acquisition_cas_version
        };
        ProtectedLocatorObservedStateV2::from_stage9_owner(
            &request,
            facility,
            provider_incarnation,
            external_anchor,
            backend_capability,
            locator_identity,
            *carrier.current_token().as_bytes(),
            custody_class,
            *carrier.current_token().as_bytes(),
            carrier.revision(),
            cas_incarnation,
            publication_incarnation,
            restore_incarnation,
            currentness,
            state_token,
            fence,
            revocation_revision,
        )
    }

    fn verify_pre_dispatch_owner_binding(
        &self,
        carrier: ProtectedCeremonyEffectCarrierV1,
    ) -> Result<(), ProtectedLocatorLeaseErrorV2> {
        if carrier != self.acquisition_carrier
            || carrier.current_token().as_bytes() != &self.source_carrier
            || !matches!(
                carrier.phase(),
                ProtectedCeremonyEffectPhaseV1::Sealed { attempt, seal }
                    if attempt.as_bytes() == &self.attempt
                        && seal.as_bytes() == &self.expected_candidate_seal
            )
        {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        self.owner_authority
            .issue_request(
                self.store,
                CeremonyRequestModeV1::ResolveResult,
                carrier.current_token(),
                HomeTokenV1::new(self.expected_candidate_seal),
                self.idempotency_key,
            )
            .map(|_| ())
            .map_err(map_pre_dispatch_error)
    }

    fn candidate_input(
        &self,
        candidate: Stage9DispatchedCandidateV2,
    ) -> Result<ProtectedLocatorCandidateInputV2, ProtectedLocatorLeaseErrorV2> {
        ProtectedLocatorCandidateInputV2::from_installation_owner(
            self.installation,
            *self.store.realm().as_bytes(),
            self.ceremony_spec,
            self.attempt,
            candidate.association,
            candidate.root,
            candidate.carrier,
            candidate.seal,
            candidate.postcondition,
            self.invocation,
        )
    }
}

impl ProtectedLocatorBackendV2 for Stage9ProtectedLocatorBackendSeedV2<'_> {
    fn acquire_pre_candidate(
        &mut self,
    ) -> Result<
        (
            ProtectedLocatorAcquisitionRequestV2,
            ProtectedLocatorObservedStateV2,
        ),
        ProtectedLocatorLeaseErrorV2,
    > {
        if self.dispatch_attempted {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        let request = self.request()?;
        self.verify_pre_dispatch_owner_binding(self.acquisition_carrier)?;
        let observed = self.observe_live()?;
        if observed != self.state_for(self.acquisition_carrier)? {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        Ok((request, observed))
    }

    fn acquisition_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
        if self.dispatch_attempted {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        let current = self.store.current().map_err(map_observation_error)?;
        self.verify_pre_dispatch_owner_binding(current)?;
        self.state_for(current)
    }

    fn prepare_candidate(
        &mut self,
        request: &ProtectedLocatorAcquisitionRequestV2,
        acquisition: &ProtectedLocatorObservedStateV2,
        candidate: ProtectedLocatorCandidateInputV2,
    ) -> Result<ProtectedLocatorCandidateStateV2, ProtectedLocatorLeaseErrorV2> {
        if self.dispatch_attempted || self.dispatched_candidate.is_some() {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        ProtectedLocatorCandidateStateV2::from_stage9_owner(request, acquisition, candidate)
    }

    fn pre_dispatch_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
        if self.dispatch_attempted {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        let current = self.store.current().map_err(map_observation_error)?;
        self.verify_pre_dispatch_owner_binding(current)?;
        self.state_for(current)
    }

    fn dispatch_exact_transition(
        &mut self,
        _expected_old: &ProtectedLocatorObservedStateV2,
        candidate: &ProtectedLocatorCandidateStateV2,
    ) -> Result<ProtectedLocatorDispatchOccurrenceV2, ProtectedLocatorLeaseErrorV2> {
        if self.dispatch_attempted || self.dispatched_candidate.is_some() {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        let projection = candidate.consume_stage9_dispatch_projection()?;
        let dispatched = Stage9DispatchedCandidateV2 {
            association: *projection.candidate_association(),
            root: *projection.candidate_root(),
            carrier: *projection.candidate_carrier(),
            seal: *projection.candidate_seal(),
            postcondition: *projection.candidate_postcondition(),
            transition_commitment: *projection.transition_commitment(),
        };
        let expected_postcondition = tuple_commitment(
            b"maestro.vnext.stage9-protected-postcondition.v2",
            &[
                &dispatched.association,
                &dispatched.root,
                &dispatched.carrier,
                &dispatched.seal,
            ],
        );
        let expected_transition = tuple_commitment(
            b"maestro.persistence.protected-locator-candidate-transition.v2\0",
            &[
                &self.invocation,
                &self.source_carrier,
                &dispatched.association,
                &dispatched.root,
                &dispatched.carrier,
                &dispatched.seal,
                &dispatched.postcondition,
            ],
        );
        if !projection.matches_exact_owner_effect(
            &dispatched.association,
            &self.expected_candidate_root,
            &self.expected_candidate_root,
            &self.expected_candidate_seal,
            &expected_postcondition,
            &expected_transition,
        ) || dispatched.transition_commitment != expected_transition
        {
            return Err(ProtectedLocatorLeaseErrorV2::InvalidCandidate);
        }
        let current = self.store.current().map_err(map_observation_error)?;
        if current != self.acquisition_carrier
            || current.current_token().as_bytes() != &self.source_carrier
        {
            return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
        }
        let request = self
            .owner_authority
            .issue_request(
                self.store,
                CeremonyRequestModeV1::ResolveResult,
                current.current_token(),
                HomeTokenV1::new(dispatched.seal),
                self.idempotency_key,
            )
            .map_err(map_pre_dispatch_error)?;
        self.dispatched_candidate = Some(dispatched);
        self.dispatch_attempted = true;
        match self.store.publish(request) {
            Ok(outcome) => {
                self.dispatch_outcome = Some(outcome);
                Ok(ProtectedLocatorDispatchOccurrenceV2::Definite)
            }
            Err(_) => Ok(ProtectedLocatorDispatchOccurrenceV2::Unknown),
        }
    }

    fn final_readback(
        &mut self,
    ) -> Result<ProtectedLocatorFinalReadbackV2, ProtectedLocatorLeaseErrorV2> {
        if !self.dispatch_attempted {
            return Err(ProtectedLocatorLeaseErrorV2::Replay);
        }
        let candidate = self
            .dispatched_candidate
            .ok_or(ProtectedLocatorLeaseErrorV2::InvalidCandidate)?;
        let current = self.store.current().map_err(map_observation_error)?;
        let committed = matches!(
            current.phase(),
            ProtectedCeremonyEffectPhaseV1::Resolved { result, .. }
                  if result.as_bytes() == &candidate.seal
        ) && self.dispatch_outcome.is_none_or(|outcome| {
            outcome.current_token() == current.current_token()
                && outcome.revision() == current.revision()
                && outcome.phase() == current.phase()
        });
        let request = self.request()?;
        let acquisition = self.state_for(self.acquisition_carrier)?;
        let observed = self.state_for(current)?;
        if committed {
            let prepared = ProtectedLocatorCandidateStateV2::from_stage9_owner(
                &request,
                &acquisition,
                self.candidate_input(candidate)?,
            )?;
            ProtectedLocatorFinalReadbackV2::exact_candidate_from_stage9_owner(
                &request, observed, &prepared,
            )
        } else {
            ProtectedLocatorFinalReadbackV2::observed_non_candidate_from_stage9_owner(
                &request, observed,
            )
        }
    }
}

pub(in crate::domain::vnext) fn acquire_stage9_backend_v2<'locator>(
    store: &'locator ProtectedCeremonyEffectStoreV1,
    anchor: &'locator ProtectedCeremonyCarrierAnchorV1,
    owner_authority: &'locator ProtectedCeremonyOwnerAuthorityV1,
    transaction: &DistributionTransactionV1,
    target: &CanonicalTargetIdentityV1,
) -> Result<Stage9ProtectedLocatorBackendSeedV2<'locator>, ProtectedLocatorLeaseErrorV2> {
    let domain = transaction.plan().domain();
    if domain.kind() != DistributionDomainKindV1::InstallationDomain
        || target.domain() != domain
        || !transaction
            .plan()
            .targets()
            .iter()
            .any(|planned| planned.target_identity == target.identity())
    {
        return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
    }
    let planned = transaction
        .plan()
        .targets()
        .iter()
        .find(|planned| planned.target_identity == target.identity())
        .ok_or(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)?;
    let expected_candidate_root = planned
        .candidate_commitment
        .ok_or(ProtectedLocatorLeaseErrorV2::InvalidCandidate)?;
    let carrier = store.current().map_err(map_observation_error)?;
    let ProtectedCeremonyEffectPhaseV1::Sealed { attempt, seal } = carrier.phase() else {
        return Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch);
    };
    let checkpoint = transaction
        .checkpoint_commitment()
        .map_err(|_| ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)?;
    let installation = *domain.domain_id().as_bytes();
    let installation_domain = tuple_commitment(
        b"maestro.vnext.stage9-protected-installation-domain.v2",
        &[
            domain.domain_id().as_bytes(),
            domain.store_generation_id().as_bytes(),
            domain.authority_epoch_id().as_bytes(),
        ],
    );
    let operation = *checkpoint.as_bytes();
    let ceremony_spec = tuple_commitment(
        b"maestro.vnext.stage9-protected-ceremony.v2",
        &[b"InstallationV1Cutover"],
    );
    let attempt = *attempt.as_bytes();
    let source_carrier = *carrier.current_token().as_bytes();
    let target_identity = *target.identity().as_bytes();
    let invocation = tuple_commitment(
        b"maestro.persistence.protected-locator-invocation.v2\0",
        &[
            &installation,
            &operation,
            &ceremony_spec,
            &attempt,
            &target_identity,
        ],
    );
    let idempotency_key = HomeTokenV1::new(tuple_commitment(
        b"maestro.vnext.stage9-protected-locator-idempotency.v2",
        &[&operation, &attempt, &invocation],
    ));
    let backend = Stage9ProtectedLocatorBackendSeedV2 {
        store,
        anchor,
        owner_authority,
        installation,
        installation_domain,
        operation,
        ceremony_spec,
        attempt,
        source_carrier,
        target: target_identity,
        invocation,
        target_manager_realm: *target.parts().manager_realm_id.as_bytes(),
        target_security_realm: *target.parts().security_realm_id.as_bytes(),
        acquisition_cas_version: carrier.revision(),
        expected_candidate_root: *expected_candidate_root.as_bytes(),
        expected_candidate_seal: *seal.as_bytes(),
        acquisition_carrier: carrier,
        idempotency_key,
        dispatched_candidate: None,
        dispatch_outcome: None,
        dispatch_attempted: false,
    };
    let _request = backend.request()?;
    backend.verify_pre_dispatch_owner_binding(carrier)?;
    backend.state_for(carrier)?;
    Ok(backend)
}

pub(in crate::domain::vnext::persistence) fn acquire_protected_locator_lease_v2<
    'lease,
    'provider,
>(
    backend: &'lease mut Stage9ProtectedLocatorBackendSeedV2<'provider>,
) -> Result<ProtectedLocatorLeaseV2<'lease>, ProtectedLocatorLeaseErrorV2> {
    ProtectedLocatorLeaseV2::acquire(backend)
}

fn map_observation_error(error: ProtectedCeremonyEffectErrorV1) -> ProtectedLocatorLeaseErrorV2 {
    match error {
        ProtectedCeremonyEffectErrorV1::StaleExpectedCarrier
        | ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch
        | ProtectedCeremonyEffectErrorV1::UnsafePersistentCarrierPath => {
            ProtectedLocatorLeaseErrorV2::CurrentnessMismatch
        }
        _ => ProtectedLocatorLeaseErrorV2::BackendUnavailable,
    }
}

fn map_pre_dispatch_error(error: ProtectedCeremonyEffectErrorV1) -> ProtectedLocatorLeaseErrorV2 {
    match error {
        ProtectedCeremonyEffectErrorV1::StaleExpectedCarrier
        | ProtectedCeremonyEffectErrorV1::CarrierIdentityMismatch => {
            ProtectedLocatorLeaseErrorV2::CurrentnessMismatch
        }
        _ => ProtectedLocatorLeaseErrorV2::BackendUnavailable,
    }
}

fn anchor_matches_store(
    anchor: &ProtectedCeremonyCarrierAnchorV1,
    store: &ProtectedCeremonyEffectStoreV1,
) -> bool {
    let Ok(CborValue::Array(fields)) = anchor.canonical_value() else {
        return false;
    };
    matches!(
        fields.get(6),
        Some(CborValue::Bytes(realm)) if realm.as_slice() == store.realm().as_bytes()
    )
}

fn tuple_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[cfg(test)]
pub(in crate::domain::vnext::persistence) struct Stage9ProtectedLocatorBackendSeedV1 {
    _private: (),
}

#[cfg(test)]
impl owner_sealed::Sealed for Stage9ProtectedLocatorBackendSeedV1 {}

#[cfg(test)]
impl ProtectedLocatorBackendV1 for Stage9ProtectedLocatorBackendSeedV1 {
    fn observe_no_follow(
        &mut self,
        _request: ProtectedLocatorOperationRequestV1,
    ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn pre_dispatch_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn dispatch_expected_old(
        &mut self,
        _expected_old: [u8; 32],
        _candidate: &ProtectedLocatorCandidateTransitionV1,
    ) -> Result<ProtectedLocatorDispatchOccurrenceV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn prepare_candidate_transition(
        &mut self,
        _request: ProtectedLocatorOperationRequestV1,
    ) -> Result<ProtectedLocatorCandidateTransitionV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn final_readback(
        &mut self,
    ) -> Result<ProtectedLocatorFinalReadbackV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }
}

#[cfg(test)]
pub(in crate::domain::vnext::persistence) fn acquire_stage9_backend()
-> Stage9ProtectedLocatorBackendSeedV1 {
    Stage9ProtectedLocatorBackendSeedV1 { _private: () }
}

#[cfg(test)]
pub(in crate::domain::vnext::persistence) fn acquire_protected_locator_lease<'locator>(
    backend: &'locator mut Stage9ProtectedLocatorBackendSeedV1,
    request: ProtectedLocatorOperationRequestV1,
) -> Result<ProtectedLocatorLeaseV1<'locator>, ProtectedLocatorLeaseErrorV1> {
    ProtectedLocatorLeaseV1::acquire(backend, request)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::vnext::execution::CeremonySpecV1;
    use crate::domain::vnext::persistence::protected_locator_lease::{
        ProtectedLocatorCandidateInputV2, ProtectedLocatorFinalityDispositionV2,
    };

    fn seed_root(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("stage9 locator test clock")
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!(
                "maestro-vnext-stage9-v4-locator-{}-{nanos}",
                std::process::id()
            ))
            .join(label);
        std::fs::create_dir_all(&root).expect("stage9 locator test root");
        std::fs::canonicalize(root).expect("stage9 locator test root canonicalization")
    }

    fn sealed_fixture(
        label: &str,
        basis: u8,
    ) -> (
        ProtectedCeremonyEffectStoreV1,
        ProtectedCeremonyCarrierAnchorV1,
        ProtectedCeremonyOwnerAuthorityV1,
    ) {
        let owner =
            ProtectedCeremonyOwnerAuthorityV1::from_owner_basis(HomeTokenV1::new([basis; 32]))
                .expect("stage9 locator owner");
        let (store, anchor) = ProtectedCeremonyEffectStoreV1::initialize(
            seed_root(label),
            CeremonySpecV1::InstallationV1Cutover,
            HomeTokenV1::new([basis.wrapping_add(1); 32]),
            &owner,
        )
        .expect("stage9 locator store");
        let empty = store.current().expect("stage9 empty carrier");
        let initiate = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::Initiate,
                empty.current_token(),
                HomeTokenV1::new([basis.wrapping_add(2); 32]),
                HomeTokenV1::new([basis.wrapping_add(3); 32]),
            )
            .expect("stage9 initiate request");
        store.publish(initiate).expect("stage9 initiate");
        let reserved = store.current().expect("stage9 reserved carrier");
        let seal = [basis.wrapping_add(4); 32];
        let recover = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::RecoverReserved,
                reserved.current_token(),
                HomeTokenV1::new(seal),
                HomeTokenV1::new([basis.wrapping_add(5); 32]),
            )
            .expect("stage9 recover request");
        store.publish(recover).expect("stage9 recover");
        (store, anchor, owner)
    }

    fn seeded_backend<'locator>(
        store: &'locator ProtectedCeremonyEffectStoreV1,
        anchor: &'locator ProtectedCeremonyCarrierAnchorV1,
        owner: &'locator ProtectedCeremonyOwnerAuthorityV1,
    ) -> Stage9ProtectedLocatorBackendSeedV2<'locator> {
        let carrier = store.current().expect("stage9 sealed carrier");
        let ProtectedCeremonyEffectPhaseV1::Sealed { attempt, seal } = carrier.phase() else {
            panic!("stage9 fixture must be sealed");
        };
        Stage9ProtectedLocatorBackendSeedV2 {
            store,
            anchor,
            owner_authority: owner,
            installation: [1; 32],
            installation_domain: [2; 32],
            operation: [3; 32],
            ceremony_spec: [4; 32],
            attempt: *attempt.as_bytes(),
            source_carrier: *carrier.current_token().as_bytes(),
            target: [5; 32],
            invocation: [6; 32],
            target_manager_realm: [7; 32],
            target_security_realm: [8; 32],
            acquisition_cas_version: carrier.revision(),
            expected_candidate_root: [30; 32],
            expected_candidate_seal: *seal.as_bytes(),
            acquisition_carrier: carrier,
            idempotency_key: HomeTokenV1::new([33; 32]),
            dispatched_candidate: None,
            dispatch_outcome: None,
            dispatch_attempted: false,
        }
    }

    fn candidate(
        backend: &Stage9ProtectedLocatorBackendSeedV2<'_>,
    ) -> ProtectedLocatorCandidateInputV2 {
        let association = [40; 32];
        let postcondition = tuple_commitment(
            b"maestro.vnext.stage9-protected-postcondition.v2",
            &[
                &association,
                &backend.expected_candidate_root,
                &backend.expected_candidate_root,
                &backend.expected_candidate_seal,
            ],
        );
        ProtectedLocatorCandidateInputV2::from_installation_owner(
            backend.installation,
            *backend.store.realm().as_bytes(),
            backend.ceremony_spec,
            backend.attempt,
            association,
            backend.expected_candidate_root,
            backend.expected_candidate_root,
            backend.expected_candidate_seal,
            postcondition,
            backend.invocation,
        )
        .expect("stage9 locator candidate")
    }

    #[test]
    fn real_v2_ceremony_provider_acquires_before_candidate_and_commits_once() {
        let (store, anchor, owner) = sealed_fixture("commit", 41);
        let mut backend = seeded_backend(&store, &anchor, &owner);
        let candidate = candidate(&backend);
        let lease = acquire_protected_locator_lease_v2(&mut backend).expect("stage9 locator lease");
        let disposition = lease
            .bind_inert_candidate(candidate)
            .and_then(|transition| transition.dispatch())
            .expect("stage9 locator dispatch");
        assert_eq!(
            disposition,
            ProtectedLocatorFinalityDispositionV2::Committed
        );
        assert!(matches!(
            store.current().expect("stage9 committed carrier").phase(),
            ProtectedCeremonyEffectPhaseV1::Resolved { result, .. }
                if result.as_bytes() == &backend.expected_candidate_seal
        ));
    }

    #[test]
    fn stale_carrier_is_refused_before_the_owner_dispatches_a_second_cas() {
        let (store, anchor, owner) = sealed_fixture("stale", 51);
        let mut backend = seeded_backend(&store, &anchor, &owner);
        let expected_seal = backend.expected_candidate_seal;
        let candidate = candidate(&backend);
        let lease = acquire_protected_locator_lease_v2(&mut backend).expect("stage9 locator lease");
        let transition = lease
            .bind_inert_candidate(candidate)
            .expect("stage9 locator transition");
        let current = store.current().expect("stage9 current carrier");
        let competing_result = HomeTokenV1::new([99; 32]);
        let competing = owner
            .issue_request(
                &store,
                CeremonyRequestModeV1::ResolveResult,
                current.current_token(),
                competing_result,
                HomeTokenV1::new([98; 32]),
            )
            .expect("stage9 competing request");
        store.publish(competing).expect("stage9 competing publish");
        assert!(matches!(
            transition.dispatch(),
            Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)
        ));
        assert!(matches!(
            store.current().expect("stage9 competing carrier").phase(),
            ProtectedCeremonyEffectPhaseV1::Resolved { result, .. }
                if result == competing_result && result.as_bytes() != &expected_seal
        ));
    }

    #[test]
    fn anchor_and_owner_substitution_are_rejected_during_acquisition() {
        let (store, anchor, owner) = sealed_fixture("genuine", 61);
        let (_other_store, other_anchor, other_owner) = sealed_fixture("other", 71);

        let mut wrong_anchor = seeded_backend(&store, &other_anchor, &owner);
        assert!(matches!(
            acquire_protected_locator_lease_v2(&mut wrong_anchor),
            Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)
        ));

        let mut wrong_owner = seeded_backend(&store, &anchor, &other_owner);
        assert!(matches!(
            acquire_protected_locator_lease_v2(&mut wrong_owner),
            Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
                | Err(ProtectedLocatorLeaseErrorV2::CurrentnessMismatch)
        ));
    }

    #[test]
    fn revision_regression_changes_the_recomputed_live_binding() {
        let (store, anchor, owner) = sealed_fixture("rollback", 81);
        let mut backend = seeded_backend(&store, &anchor, &owner);
        let carrier = store.current().expect("stage9 current carrier");
        let baseline = backend.state_for(carrier).expect("stage9 baseline");
        backend.acquisition_cas_version = carrier.revision() + 1;
        let regressed = backend.state_for(carrier).expect("stage9 regressed");
        assert!(baseline != regressed);
    }

    #[test]
    fn tuple_commitment_is_domain_and_field_separated() {
        let one = tuple_commitment(b"maestro.test.domain", &[b"a", b"bc"]);
        let two = tuple_commitment(b"maestro.test.domain", &[b"ab", b"c"]);
        let three = tuple_commitment(b"maestro.test.other", &[b"a", b"bc"]);
        assert_ne!(one, two);
        assert_ne!(one, three);
        assert_eq!(
            one,
            tuple_commitment(b"maestro.test.domain", &[b"a", b"bc"])
        );
    }

    #[test]
    fn v1_historical_seed_is_test_only_and_fail_closed() {
        let request = ProtectedLocatorOperationRequestV1::from_installation_operation(
            [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32],
            [10; 32],
        )
        .expect("historical V1 request");
        let mut backend = acquire_stage9_backend();
        assert!(matches!(
            acquire_protected_locator_lease(&mut backend, request),
            Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
        ));
    }
}
