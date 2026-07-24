use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ConsumerSnapshotCurrentFactsV1 {
    ActiveStore {
        owner_operation: [u8; 32],
        owner_stage_tag: u8,
        store_instance: [u8; 32],
        active_state_revision: [u8; 32],
        activation_incarnation: [u8; 32],
        restore_incarnation: [u8; 32],
        head_identity: [u8; 32],
        head_revision: [u8; 32],
        generation_identity: [u8; 32],
        generation_ordinal: u64,
        publication_clock: [u8; 32],
        currentness: [u8; 32],
        installation_id: [u8; 32],
        realm: [u8; 32],
        domain: [u8; 32],
        census_identity: [u8; 32],
        census_rows: Vec<[u8; 32]>,
        release_identity: [u8; 32],
        writer_protocol_epoch: u64,
        schema_epoch: u64,
        migration_epoch: u64,
        declared_consumer_root_manifest: [u8; 32],
        public_resource_closure: [u8; 32],
        public_bundle_closure: [u8; 32],
        public_release_closure: [u8; 32],
        alias_roots: [u8; 32],
        manager_roots: [u8; 32],
        target_roots: [u8; 32],
        claims_catalog_descriptors: [u8; 32],
    },
    PreStore {
        owner_operation: [u8; 32],
        inactive_candidate_state: [u8; 32],
        pre_association_root_closure: [u8; 32],
        installation_id: [u8; 32],
        realm: [u8; 32],
        domain: [u8; 32],
        candidate_census_identity: [u8; 32],
        candidate_census_rows: Vec<[u8; 32]>,
        candidate_release_identity: [u8; 32],
        writer_protocol_epoch: u64,
        schema_epoch: u64,
        migration_epoch: u64,
        declared_consumer_root_manifest: [u8; 32],
        public_resource_closure: [u8; 32],
        public_bundle_closure: [u8; 32],
        public_release_closure: [u8; 32],
        alias_roots: [u8; 32],
        manager_roots: [u8; 32],
        target_roots: [u8; 32],
        ceremony_spec: [u8; 32],
        attempt: [u8; 32],
        protected_source_carrier: [u8; 32],
        candidate_carrier: [u8; 32],
        expected_old_locator_root: [u8; 32],
        expected_old_cas: [u8; 32],
    },
}

impl ConsumerSnapshotCurrentFactsV1 {
    pub(in crate::domain::vnext) const fn owner_operation(&self) -> [u8; 32] {
        match self {
            Self::ActiveStore {
                owner_operation, ..
            }
            | Self::PreStore {
                owner_operation, ..
            } => *owner_operation,
        }
    }

    pub(in crate::domain::vnext) const fn owner_stage_tag(&self) -> u8 {
        match self {
            Self::ActiveStore {
                owner_stage_tag, ..
            } => *owner_stage_tag,
            Self::PreStore { .. } => 1,
        }
    }

    pub(in crate::domain::vnext) fn census_rows(&self) -> &[[u8; 32]] {
        match self {
            Self::ActiveStore { census_rows, .. } => census_rows,
            Self::PreStore {
                candidate_census_rows,
                ..
            } => candidate_census_rows,
        }
    }

    pub(in crate::domain::vnext) fn canonical_commitments(&self) -> Vec<[u8; 32]> {
        match self {
            Self::ActiveStore {
                owner_operation,
                store_instance,
                active_state_revision,
                activation_incarnation,
                restore_incarnation,
                head_identity,
                head_revision,
                generation_identity,
                publication_clock,
                currentness,
                installation_id,
                realm,
                domain,
                census_identity,
                census_rows,
                release_identity,
                declared_consumer_root_manifest,
                public_resource_closure,
                public_bundle_closure,
                public_release_closure,
                alias_roots,
                manager_roots,
                target_roots,
                claims_catalog_descriptors,
                ..
            } => {
                let mut values = vec![
                    *owner_operation,
                    *store_instance,
                    *active_state_revision,
                    *activation_incarnation,
                    *restore_incarnation,
                    *head_identity,
                    *head_revision,
                    *generation_identity,
                    *publication_clock,
                    *currentness,
                    *installation_id,
                    *realm,
                    *domain,
                    *census_identity,
                    *release_identity,
                    *declared_consumer_root_manifest,
                    *public_resource_closure,
                    *public_bundle_closure,
                    *public_release_closure,
                    *alias_roots,
                    *manager_roots,
                    *target_roots,
                    *claims_catalog_descriptors,
                ];
                values.extend(census_rows);
                values
            }
            Self::PreStore {
                owner_operation,
                inactive_candidate_state,
                pre_association_root_closure,
                installation_id,
                realm,
                domain,
                candidate_census_identity,
                candidate_census_rows,
                candidate_release_identity,
                declared_consumer_root_manifest,
                public_resource_closure,
                public_bundle_closure,
                public_release_closure,
                alias_roots,
                manager_roots,
                target_roots,
                ceremony_spec,
                attempt,
                protected_source_carrier,
                candidate_carrier,
                expected_old_locator_root,
                expected_old_cas,
                ..
            } => {
                let mut values = vec![
                    *owner_operation,
                    *inactive_candidate_state,
                    *pre_association_root_closure,
                    *installation_id,
                    *realm,
                    *domain,
                    *candidate_census_identity,
                    *candidate_release_identity,
                    *declared_consumer_root_manifest,
                    *public_resource_closure,
                    *public_bundle_closure,
                    *public_release_closure,
                    *alias_roots,
                    *manager_roots,
                    *target_roots,
                    *ceremony_spec,
                    *attempt,
                    *protected_source_carrier,
                    *candidate_carrier,
                    *expected_old_locator_root,
                    *expected_old_cas,
                ];
                values.extend(candidate_census_rows);
                values
            }
        }
    }

    fn is_valid(&self) -> bool {
        let epochs_valid = match self {
            Self::ActiveStore {
                owner_stage_tag,
                generation_ordinal,
                writer_protocol_epoch,
                schema_epoch,
                migration_epoch,
                ..
            } => {
                (1..=3).contains(owner_stage_tag)
                    && *generation_ordinal > 0
                    && *writer_protocol_epoch > 0
                    && *schema_epoch > 0
                    && *migration_epoch > 0
            }
            Self::PreStore {
                writer_protocol_epoch,
                schema_epoch,
                migration_epoch,
                ..
            } => *writer_protocol_epoch > 0 && *schema_epoch > 0 && *migration_epoch > 0,
        };
        let rows = self.census_rows();
        epochs_valid
            && !rows.is_empty()
            && !self.canonical_commitments().contains(&[0; 32])
            && rows.windows(2).all(|pair| pair[0] < pair[1])
    }
}

pub(in crate::domain::vnext::persistence) mod consumer_currentness_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext) trait ConsumerSnapshotCurrentViewProviderV1:
    consumer_currentness_sealed::Sealed
{
    fn current_facts(&self) -> Option<ConsumerSnapshotCurrentFactsV1>;
}

pub(in crate::domain::vnext) struct ConsumerSnapshotCurrentViewLeaseV1<'view, P> {
    provider: &'view mut P,
    initial: ConsumerSnapshotCurrentFactsV1,
    _exclusive: PhantomData<&'view mut P>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'view, P: ConsumerSnapshotCurrentViewProviderV1> ConsumerSnapshotCurrentViewLeaseV1<'view, P> {
    pub(in crate::domain::vnext::persistence) fn bind(
        provider: &'view mut P,
    ) -> Result<Self, ConsumerSnapshotCurrentnessErrorV1> {
        let initial = provider
            .current_facts()
            .ok_or(ConsumerSnapshotCurrentnessErrorV1::Unavailable)?;
        if !initial.is_valid() {
            return Err(ConsumerSnapshotCurrentnessErrorV1::InvalidCurrentView);
        }
        Ok(Self {
            provider,
            initial,
            _exclusive: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext) fn initial(&self) -> &ConsumerSnapshotCurrentFactsV1 {
        &self.initial
    }

    pub(in crate::domain::vnext) fn consume_final_recheck(
        self,
    ) -> Result<ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentnessErrorV1> {
        if self.provider.current_facts() != Some(self.initial.clone()) {
            return Err(ConsumerSnapshotCurrentnessErrorV1::Changed);
        }
        Ok(self.initial)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ConsumerSnapshotCurrentnessErrorV1 {
    #[error("consumer-snapshot current Store view is unavailable")]
    Unavailable,
    #[error("consumer-snapshot current Store view is invalid")]
    InvalidCurrentView,
    #[error("consumer-snapshot current Store view changed")]
    Changed,
}

#[cfg(test)]
pub(in crate::domain::vnext) mod test_seed {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    pub struct TestProviderV1 {
        facts: Rc<RefCell<Option<ConsumerSnapshotCurrentFactsV1>>>,
    }

    impl TestProviderV1 {
        pub fn new(facts: ConsumerSnapshotCurrentFactsV1) -> Self {
            Self {
                facts: Rc::new(RefCell::new(Some(facts))),
            }
        }

        pub fn control(&self) -> Rc<RefCell<Option<ConsumerSnapshotCurrentFactsV1>>> {
            Rc::clone(&self.facts)
        }
    }

    impl consumer_currentness_sealed::Sealed for TestProviderV1 {}

    impl ConsumerSnapshotCurrentViewProviderV1 for TestProviderV1 {
        fn current_facts(&self) -> Option<ConsumerSnapshotCurrentFactsV1> {
            self.facts.borrow().clone()
        }
    }

    pub fn bind(
        provider: &mut TestProviderV1,
    ) -> Result<
        ConsumerSnapshotCurrentViewLeaseV1<'_, TestProviderV1>,
        ConsumerSnapshotCurrentnessErrorV1,
    > {
        ConsumerSnapshotCurrentViewLeaseV1::bind(provider)
    }
}
