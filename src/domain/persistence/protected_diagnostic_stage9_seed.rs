#![allow(
    dead_code,
    reason = "Stage 9 supplies the provider before the Stage 10 host consumer integrates"
)]

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::domain::identity::{
    ContractRootIdV1, StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1,
};
use crate::domain::installation::Stage9ActiveConsumerMaterializationV1;

use super::consumer_snapshot::{
    ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentViewLeasePortV1,
    ConsumerSnapshotCurrentViewProviderV1, ConsumerSnapshotCurrentnessErrorV1,
    consumer_currentness_sealed,
};
use super::{StoreCompatibilityV1, StoreError, StoreRoleV1, StoreStateV1, StoreV1};

use super::protected_diagnostic::{
    ProtectedDiagnosticCurrentViewProviderV1, ProtectedDiagnosticObservedCurrentViewV1,
    ProtectedDiagnosticProviderCurrentnessV1, sealed,
};

#[derive(Clone)]
pub(crate) struct Stage9OwnerLocalCurrentViewProviderV1 {
    expected_state_revision: u64,
    expected_domain_id: StoreDomainIdV1,
    expected_role: StoreRoleV1,
    expected_head_id: StoreHeadIdV1,
    expected_head_revision: u64,
    expected_generation_id: StoreGenerationIdV1,
    expected_generation_ordinal: u64,
    expected_contract_root_id: ContractRootIdV1,
    expected_publication_clock: Option<u64>,
    live_store_instance: [u8; 32],
    activation_carrier_identity: [u8; 32],
    activation_carrier_token: [u8; 32],
    activation_attempt_identity: [u8; 32],
    activation_destination_seal: [u8; 32],
    activation_restore_incarnation: [u8; 32],
    activation_carrier_revision: u64,
    provider_currentness_revision: u64,
    bound: bool,
    consumed: bool,
}

impl Stage9OwnerLocalCurrentViewProviderV1 {
    fn from_active_materialization(
        store: &StoreV1,
        materialization: &Stage9ActiveConsumerMaterializationV1,
    ) -> Result<Option<Self>, StoreError> {
        let (state, state_revision) = store.state()?;
        let (coherent_state, head, generation, objects) = store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active
            || coherent_state != StoreStateV1::Active
            || state_revision == 0
            || materialization.activation_carrier_revision() == 0
            || store.role() != StoreRoleV1::Installation
            || store.domain().id().as_bytes() != &materialization.installation_id()
            || materialization
                .expected_head()
                .is_some_and(|expected| expected != *head.id().as_bytes())
            || !generation.compatibility().is_stage0_successor()
            || !materialization
                .required_current_objects()
                .iter()
                .all(|required| objects.iter().any(|object| object.id() == *required))
        {
            return Ok(None);
        }
        let provider_currentness_revision = next_provider_currentness_revision()?;
        let process_incarnation = process_incarnation();
        let entropy = random_entropy(b"maestro.vnext.stage9-currentness-activation.v1");
        if process_incarnation == [0; 32] || entropy == [0; 32] {
            return Ok(None);
        }
        let live_store_instance = tuple_commitment(
            b"maestro.vnext.stage9-store-instance.v1",
            &[
                &process_incarnation,
                &entropy,
                store.domain().id().as_bytes(),
                head.id().as_bytes(),
                generation.id().as_bytes(),
                &provider_currentness_revision.to_be_bytes(),
            ],
        );
        let activation_carrier_token = tuple_commitment(
            b"maestro.vnext.stage9-activation-carrier-token.v1",
            &[
                &live_store_instance,
                &materialization.activation_carrier_identity(),
                &materialization.activation_carrier_revision().to_be_bytes(),
                &materialization.activation_attempt_identity(),
                &materialization.activation_destination_seal(),
            ],
        );
        let activation_restore_incarnation = tuple_commitment(
            b"maestro.vnext.stage9-activation-restore-incarnation.v1",
            &[
                &live_store_instance,
                &entropy,
                activation_carrier_token.as_slice(),
            ],
        );
        if [
            live_store_instance,
            activation_carrier_token,
            activation_restore_incarnation,
        ]
        .contains(&[0; 32])
        {
            return Ok(None);
        }
        Ok(Some(Self {
            expected_state_revision: state_revision,
            expected_domain_id: store.domain().id(),
            expected_role: store.role(),
            expected_head_id: head.id(),
            expected_head_revision: head.revision(),
            expected_generation_id: generation.id(),
            expected_generation_ordinal: generation.ordinal(),
            expected_contract_root_id: generation.contract_root_id(),
            expected_publication_clock: None,
            live_store_instance,
            activation_carrier_identity: materialization.activation_carrier_identity(),
            activation_carrier_token,
            activation_attempt_identity: materialization.activation_attempt_identity(),
            activation_destination_seal: materialization.activation_destination_seal(),
            activation_restore_incarnation,
            activation_carrier_revision: materialization.activation_carrier_revision(),
            provider_currentness_revision,
            bound: false,
            consumed: false,
        }))
    }

    fn matches_observed(&self, observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>) -> bool {
        self.expected_state_revision == observed.state_revision
            && self.expected_domain_id == observed.domain.id()
            && self.expected_role == observed.role
            && self.expected_head_id == observed.head.id()
            && self.expected_head_revision == observed.head_revision
            && self.expected_generation_id == observed.generation.id()
            && self.expected_generation_ordinal == observed.generation.ordinal()
            && self.expected_contract_root_id == observed.generation.contract_root_id()
            && self
                .expected_publication_clock
                .is_none_or(|clock| clock == observed.publication_clock)
    }

    fn currentness(&self) -> Option<ProtectedDiagnosticProviderCurrentnessV1> {
        ProtectedDiagnosticProviderCurrentnessV1::from_live_provider(
            self.live_store_instance,
            self.activation_carrier_identity,
            self.activation_carrier_token,
            self.activation_carrier_revision,
            self.activation_attempt_identity,
            self.activation_destination_seal,
            self.activation_restore_incarnation,
            self.provider_currentness_revision,
        )
    }

    fn rearmed(&self) -> Self {
        let mut provider = self.clone();
        provider.bound = false;
        provider.consumed = false;
        provider
    }

    fn current_facts(
        &self,
        materialization: &Stage9ActiveConsumerMaterializationV1,
        currentness: [u8; 32],
    ) -> Option<ConsumerSnapshotCurrentFactsV1> {
        let publication_clock = self.expected_publication_clock?;
        let compatibility = StoreCompatibilityV1::stage0_successor().ok()?;
        if currentness == [0; 32]
            || [
                materialization.owner_operation(),
                materialization.installation_id(),
                materialization.realm(),
                materialization.domain(),
                materialization.census_identity(),
                materialization.release_identity(),
            ]
            .contains(&[0; 32])
            || !compatibility.is_stage0_successor()
        {
            return None;
        }
        Some(ConsumerSnapshotCurrentFactsV1::ActiveStore {
            owner_operation: materialization.owner_operation(),
            owner_stage_tag: materialization.owner_stage_tag(),
            store_instance: self.live_store_instance,
            active_state_revision: scalar_commitment(
                b"maestro.vnext.stage9-active-state-revision.v1",
                self.expected_state_revision,
            ),
            activation_incarnation: self.activation_carrier_token,
            restore_incarnation: self.activation_restore_incarnation,
            head_identity: *self.expected_head_id.as_bytes(),
            head_revision: scalar_commitment(
                b"maestro.vnext.stage9-head-revision.v1",
                self.expected_head_revision,
            ),
            generation_identity: *self.expected_generation_id.as_bytes(),
            generation_ordinal: self.expected_generation_ordinal,
            publication_clock: scalar_commitment(
                b"maestro.vnext.stage9-publication-clock.v1",
                publication_clock,
            ),
            currentness,
            installation_id: materialization.installation_id(),
            realm: materialization.realm(),
            domain: materialization.domain(),
            census_identity: materialization.census_identity(),
            census_rows: materialization.census_rows().to_vec(),
            release_identity: materialization.release_identity(),
            writer_protocol_epoch: 1,
            schema_epoch: 1,
            migration_epoch: 1,
            declared_consumer_root_manifest: materialization.declared_consumer_root_manifest(),
            public_resource_closure: materialization.public_resource_closure(),
            public_bundle_closure: materialization.public_bundle_closure(),
            public_release_closure: materialization.public_release_closure(),
            alias_roots: materialization.alias_roots(),
            manager_roots: materialization.manager_roots(),
            target_roots: materialization.target_roots(),
            claims_catalog_descriptors: materialization.claims_catalog_descriptors(),
        })
    }
}

impl sealed::Sealed for Stage9OwnerLocalCurrentViewProviderV1 {}

impl ProtectedDiagnosticCurrentViewProviderV1 for Stage9OwnerLocalCurrentViewProviderV1 {
    fn bind_current_view(
        &mut self,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> Option<ProtectedDiagnosticProviderCurrentnessV1> {
        if self.bound || self.consumed || !self.matches_observed(observed) {
            return None;
        }
        self.expected_publication_clock = Some(observed.publication_clock);
        self.bound = true;
        self.currentness()
    }

    fn final_recheck_current_view(
        &mut self,
        initial_currentness: &ProtectedDiagnosticProviderCurrentnessV1,
        observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
    ) -> bool {
        let valid = self.bound
            && !self.consumed
            && self.matches_observed(observed)
            && self
                .currentness()
                .as_ref()
                .is_some_and(|current| initial_currentness.matches(current));
        self.bound = false;
        self.consumed = true;
        valid
    }

    fn abandon_current_view(&mut self) {
        self.bound = false;
        self.consumed = true;
    }
}

pub(crate) struct Stage9ConsumerCurrentViewProviderV1<'store> {
    store: &'store mut StoreV1,
    materialization: Stage9ActiveConsumerMaterializationV1,
    verifier: Stage9OwnerLocalCurrentViewProviderV1,
    leased: bool,
}

impl<'store> Stage9ConsumerCurrentViewProviderV1<'store> {
    pub(in crate::domain) fn from_installation_materialization(
        store: &'store mut StoreV1,
        materialization: Stage9ActiveConsumerMaterializationV1,
    ) -> Result<Self, ConsumerSnapshotCurrentnessErrorV1> {
        let verifier = Stage9OwnerLocalCurrentViewProviderV1::from_active_materialization(
            store,
            &materialization,
        )
        .map_err(|_| ConsumerSnapshotCurrentnessErrorV1::Unavailable)?
        .ok_or(ConsumerSnapshotCurrentnessErrorV1::InvalidCurrentView)?;
        Ok(Self {
            store,
            materialization,
            verifier,
            leased: false,
        })
    }

    fn observe_current(
        &mut self,
    ) -> Result<ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentnessErrorV1> {
        let mut verifier = self.verifier.rearmed();
        let materialization = &self.materialization;
        let facts = self
            .store
            .with_serialized_active_view(|view| {
                let anchor = view
                    .protected_diagnostic_current_view_anchor(&mut verifier)
                    .map_err(|_| ConsumerSnapshotCurrentnessErrorV1::Changed)?;
                let currentness = anchor.commitment();
                view.consume_protected_diagnostic_current_view_anchor(anchor)
                    .map_err(|_| ConsumerSnapshotCurrentnessErrorV1::Changed)?;
                verifier
                    .current_facts(materialization, currentness)
                    .ok_or(ConsumerSnapshotCurrentnessErrorV1::InvalidCurrentView)
            })
            .map_err(|error| match error {
                super::PreparedPublicationError::Store(_) => {
                    ConsumerSnapshotCurrentnessErrorV1::Changed
                }
                super::PreparedPublicationError::Prepare(error) => error,
            })?;
        self.verifier = verifier.rearmed();
        Ok(facts)
    }
}

pub(crate) struct Stage9ConsumerCurrentViewLeaseV1<'view, 'store> {
    provider: &'view mut Stage9ConsumerCurrentViewProviderV1<'store>,
    initial: ConsumerSnapshotCurrentFactsV1,
}

impl consumer_currentness_sealed::LeaseSealed for Stage9ConsumerCurrentViewLeaseV1<'_, '_> {}

impl ConsumerSnapshotCurrentViewLeasePortV1 for Stage9ConsumerCurrentViewLeaseV1<'_, '_> {
    fn initial(&self) -> &ConsumerSnapshotCurrentFactsV1 {
        &self.initial
    }

    fn recheck_current(
        &mut self,
    ) -> Result<ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentnessErrorV1> {
        let current = self.provider.observe_current()?;
        if current != self.initial {
            return Err(ConsumerSnapshotCurrentnessErrorV1::Changed);
        }
        Ok(current)
    }
}

impl consumer_currentness_sealed::ProviderSealed for Stage9ConsumerCurrentViewProviderV1<'_> {}

impl<'store> ConsumerSnapshotCurrentViewProviderV1 for Stage9ConsumerCurrentViewProviderV1<'store> {
    type Lease<'view>
        = Stage9ConsumerCurrentViewLeaseV1<'view, 'store>
    where
        Self: 'view;

    fn acquire_current_view(
        &mut self,
    ) -> Result<Self::Lease<'_>, ConsumerSnapshotCurrentnessErrorV1> {
        if self.leased {
            return Err(ConsumerSnapshotCurrentnessErrorV1::Unavailable);
        }
        let initial = self.observe_current()?;
        self.leased = true;
        Ok(Stage9ConsumerCurrentViewLeaseV1 {
            provider: self,
            initial,
        })
    }
}

fn next_provider_currentness_revision() -> Result<u64, StoreError> {
    static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
    NEXT_REVISION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |revision| {
            revision.checked_add(1)
        })
        .map_err(|_| StoreError::ProtectedDiagnosticCurrentnessRefused)
}

fn process_incarnation() -> [u8; 32] {
    static PROCESS_INCARNATION: OnceLock<[u8; 32]> = OnceLock::new();
    *PROCESS_INCARNATION
        .get_or_init(|| random_entropy(b"maestro.vnext.stage9-process-incarnation.v1"))
}

fn random_entropy(domain: &[u8]) -> [u8; 32] {
    let state = RandomState::new();
    let mut entropy = [0u8; 32];
    for lane in 0..4u64 {
        let mut hasher = state.build_hasher();
        hasher.write(domain);
        hasher.write_u64(lane);
        entropy[(lane as usize) * 8..(lane as usize + 1) * 8]
            .copy_from_slice(&hasher.finish().to_be_bytes());
    }
    entropy
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

fn scalar_commitment(domain: &[u8], value: u64) -> [u8; 32] {
    tuple_commitment(domain, &[&value.to_be_bytes()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage9_owner_local_descendant_can_mint_only_structured_live_currentness() {
        fn require_provider<T: ProtectedDiagnosticCurrentViewProviderV1>() {}
        require_provider::<Stage9OwnerLocalCurrentViewProviderV1>();
    }

    #[test]
    fn stage9_consumer_provider_is_persistence_sealed_and_nominally_v1() {
        fn require_provider<T: ConsumerSnapshotCurrentViewProviderV1>() {}
        require_provider::<Stage9ConsumerCurrentViewProviderV1<'static>>();
        assert!(
            StoreCompatibilityV1::stage0_successor()
                .expect("frozen successor compatibility")
                .is_stage0_successor()
        );
    }
}
