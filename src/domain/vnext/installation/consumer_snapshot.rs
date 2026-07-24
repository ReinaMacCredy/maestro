use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::integration::consumer_closure::{
    ConsumerClosureErrorV1, ConsumerClosureProviderV1, HostConsumerAdmissionGuardV1,
};
use crate::domain::vnext::persistence::consumer_snapshot::{
    ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentViewLeaseV1,
    ConsumerSnapshotCurrentViewProviderV1, ConsumerSnapshotCurrentnessErrorV1,
};

mod stage_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext) trait ConsumerClosureStageV1:
    stage_sealed::Sealed
{
    const TAG: u8;
}

pub(in crate::domain::vnext) struct PreCurrentnessConsumerStageV1;
pub(in crate::domain::vnext) struct ProtectedRetentionConsumerStageV1;
pub(in crate::domain::vnext) struct PhysicalPruningConsumerStageV1;

impl stage_sealed::Sealed for PreCurrentnessConsumerStageV1 {}
impl stage_sealed::Sealed for ProtectedRetentionConsumerStageV1 {}
impl stage_sealed::Sealed for PhysicalPruningConsumerStageV1 {}

impl ConsumerClosureStageV1 for PreCurrentnessConsumerStageV1 {
    const TAG: u8 = 1;
}
impl ConsumerClosureStageV1 for ProtectedRetentionConsumerStageV1 {
    const TAG: u8 = 2;
}
impl ConsumerClosureStageV1 for PhysicalPruningConsumerStageV1 {
    const TAG: u8 = 3;
}

pub(in crate::domain::vnext) struct InstallationConsumerSnapshotV1<'view, 'connection, S, H, K> {
    store: ConsumerSnapshotCurrentViewLeaseV1<'view, S>,
    host: HostConsumerAdmissionGuardV1<'connection, H>,
    consumer_set_id: [u8; 32],
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<
    'view,
    'connection,
    S: ConsumerSnapshotCurrentViewProviderV1,
    H: ConsumerClosureProviderV1,
    K: ConsumerClosureStageV1,
> InstallationConsumerSnapshotV1<'view, 'connection, S, H, K>
{
    pub(in crate::domain::vnext::installation) fn issue(
        store: ConsumerSnapshotCurrentViewLeaseV1<'view, S>,
        host: HostConsumerAdmissionGuardV1<'connection, H>,
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1> {
        let current = store.initial();
        if current.owner_stage_tag() != K::TAG
            || (matches!(current, ConsumerSnapshotCurrentFactsV1::PreStore { .. })
                && K::TAG != PreCurrentnessConsumerStageV1::TAG)
        {
            return Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.vnext.installation-consumer-set.v1\0");
        hasher.update([K::TAG]);
        for commitment in current.canonical_commitments() {
            hasher.update(commitment);
        }
        hasher.update((current.census_rows().len() as u64).to_be_bytes());
        for commitment in host.closure_commitments() {
            hasher.update(commitment);
        }
        let consumer_set_id = hasher.finalize().into();
        Ok(Self {
            store,
            host,
            consumer_set_id,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext::installation) fn consume_finality(
        self,
        exact_consumer_gate_result_id: [u8; 32],
    ) -> Result<ConsumerClosureReceiptV1<K>, InstallationConsumerSnapshotErrorV1> {
        if exact_consumer_gate_result_id != self.consumer_set_id {
            return Err(InstallationConsumerSnapshotErrorV1::GateMismatch);
        }
        self.host.consume_final_recheck()?;
        let current = self.store.consume_final_recheck()?;
        if current.owner_stage_tag() != K::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch);
        }
        Ok(ConsumerClosureReceiptV1 {
            owner_operation: current.owner_operation(),
            consumer_set_id: self.consumer_set_id,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain::vnext) struct ConsumerClosureReceiptV1<K> {
    owner_operation: [u8; 32],
    consumer_set_id: [u8; 32],
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Debug, Error)]
pub(in crate::domain::vnext) enum InstallationConsumerSnapshotErrorV1 {
    #[error(transparent)]
    Store(#[from] ConsumerSnapshotCurrentnessErrorV1),
    #[error(transparent)]
    Host(#[from] ConsumerClosureErrorV1),
    #[error("consumer stage does not match the sealed owning operation")]
    OwnerStageMismatch,
    #[error("consumer gate result does not bind the owner-issued consumer set")]
    GateMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vnext::integration::consumer_closure::test_seed as integration_seed;
    use crate::domain::vnext::persistence::consumer_snapshot::{
        ConsumerSnapshotCurrentFactsV1, test_seed as persistence_seed,
    };

    fn active_store_facts(stage: u8) -> ConsumerSnapshotCurrentFactsV1 {
        ConsumerSnapshotCurrentFactsV1::ActiveStore {
            owner_operation: [1; 32],
            owner_stage_tag: stage,
            store_instance: [2; 32],
            active_state_revision: [3; 32],
            activation_incarnation: [4; 32],
            restore_incarnation: [5; 32],
            head_identity: [6; 32],
            head_revision: [7; 32],
            generation_identity: [8; 32],
            generation_ordinal: 9,
            publication_clock: [10; 32],
            currentness: [11; 32],
            installation_id: [12; 32],
            realm: [13; 32],
            domain: [14; 32],
            census_identity: [15; 32],
            census_rows: vec![[16; 32], [17; 32]],
            release_identity: [18; 32],
            writer_protocol_epoch: 19,
            schema_epoch: 20,
            migration_epoch: 21,
            declared_consumer_root_manifest: [22; 32],
            public_resource_closure: [23; 32],
            public_bundle_closure: [24; 32],
            public_release_closure: [25; 32],
            alias_roots: [26; 32],
            manager_roots: [27; 32],
            target_roots: [28; 32],
            claims_catalog_descriptors: [29; 32],
        }
    }

    fn pre_store_facts() -> ConsumerSnapshotCurrentFactsV1 {
        ConsumerSnapshotCurrentFactsV1::PreStore {
            owner_operation: [1; 32],
            inactive_candidate_state: [2; 32],
            pre_association_root_closure: [3; 32],
            installation_id: [4; 32],
            realm: [5; 32],
            domain: [6; 32],
            candidate_census_identity: [7; 32],
            candidate_census_rows: vec![[8; 32], [9; 32]],
            candidate_release_identity: [10; 32],
            writer_protocol_epoch: 11,
            schema_epoch: 12,
            migration_epoch: 13,
            declared_consumer_root_manifest: [14; 32],
            public_resource_closure: [15; 32],
            public_bundle_closure: [16; 32],
            public_release_closure: [17; 32],
            alias_roots: [18; 32],
            manager_roots: [19; 32],
            target_roots: [20; 32],
            ceremony_spec: [21; 32],
            attempt: [22; 32],
            protected_source_carrier: [23; 32],
            candidate_carrier: [24; 32],
            expected_old_locator_root: [25; 32],
            expected_old_cas: [26; 32],
        }
    }

    fn issue<'store, 'host, K: ConsumerClosureStageV1>(
        store: &'store mut persistence_seed::TestProviderV1,
        host: &'host mut integration_seed::TestProviderV1,
    ) -> Result<
        InstallationConsumerSnapshotV1<
            'store,
            'host,
            persistence_seed::TestProviderV1,
            integration_seed::TestProviderV1,
            K,
        >,
        InstallationConsumerSnapshotErrorV1,
    > {
        InstallationConsumerSnapshotV1::issue(
            persistence_seed::bind(store).unwrap(),
            integration_seed::bind(host).unwrap(),
        )
    }

    #[test]
    fn owner_issued_snapshot_joins_store_and_host_until_both_final_rechecks() {
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        let expected = snapshot.consumer_set_id;
        let receipt = snapshot.consume_finality(expected).unwrap();
        assert_eq!(receipt.owner_operation, [1; 32]);
        assert_eq!(receipt.consumer_set_id, expected);
    }

    #[test]
    fn caller_gate_stage_and_post_issue_currentness_substitution_refuse() {
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        assert!(matches!(
            snapshot.consume_finality([99; 32]),
            Err(InstallationConsumerSnapshotErrorV1::GateMismatch)
        ));

        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        assert!(matches!(
            issue::<ProtectedRetentionConsumerStageV1>(&mut store, &mut host),
            Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch)
        ));
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PhysicalPruningConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        assert!(issue::<PhysicalPruningConsumerStageV1>(&mut store, &mut host).is_ok());

        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let host_control = host.control();
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        let expected = snapshot.consumer_set_id;
        integration_seed::change_admission_epoch(&host_control, [88; 32]);
        assert!(matches!(
            snapshot.consume_finality(expected),
            Err(InstallationConsumerSnapshotErrorV1::Host(
                ConsumerClosureErrorV1::Changed
            ))
        ));

        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let store_control = store.control();
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        let expected = snapshot.consumer_set_id;
        let mut changed = store_control.borrow().clone().unwrap();
        if let ConsumerSnapshotCurrentFactsV1::ActiveStore {
            restore_incarnation,
            ..
        } = &mut changed
        {
            *restore_incarnation = [77; 32];
        }
        *store_control.borrow_mut() = Some(changed);
        assert!(matches!(
            snapshot.consume_finality(expected),
            Err(InstallationConsumerSnapshotErrorV1::Store(
                ConsumerSnapshotCurrentnessErrorV1::Changed
            ))
        ));
    }

    #[test]
    fn pre_store_is_pre_currentness_only_and_has_no_final_root_or_candidate_seal() {
        let mut store = persistence_seed::TestProviderV1::new(pre_store_facts());
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        let expected = snapshot.consumer_set_id;
        snapshot.consume_finality(expected).unwrap();

        let source = include_str!("../persistence/consumer_snapshot.rs");
        assert!(!source.contains("final_candidate_root"));
        assert!(!source.contains("candidate_seal"));
    }
}
