use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::domain::vnext) struct AdmittedConsumerIdentityV1 {
    kind_tag: u8,
    identity_commitment: [u8; 32],
    descriptor_commitment: [u8; 32],
    binary_commitment: [u8; 32],
    protocol_commitment: [u8; 32],
    release_commitment: [u8; 32],
    currentness_commitment: [u8; 32],
    required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct ConsumerClosureFactsV1 {
    host_process_incarnation: [u8; 32],
    connection_incarnation: [u8; 32],
    discovery_revision: [u8; 32],
    autoload_revision: [u8; 32],
    dispatch_revision: [u8; 32],
    old_reader_revision: [u8; 32],
    writer_revision: [u8; 32],
    alias_revision: [u8; 32],
    legacy_reader_revision: [u8; 32],
    declared_slot_revision: [u8; 32],
    admission_set_epoch: [u8; 32],
    monotonic_currentness_fence: [u8; 32],
    revocation_fence: [u8; 32],
    consumers: Vec<AdmittedConsumerIdentityV1>,
}

impl ConsumerClosureFactsV1 {
    fn is_valid(&self) -> bool {
        let anchors = [
            self.host_process_incarnation,
            self.connection_incarnation,
            self.discovery_revision,
            self.autoload_revision,
            self.dispatch_revision,
            self.old_reader_revision,
            self.writer_revision,
            self.alias_revision,
            self.legacy_reader_revision,
            self.declared_slot_revision,
            self.admission_set_epoch,
            self.monotonic_currentness_fence,
            self.revocation_fence,
        ];
        if anchors.contains(&[0; 32]) || self.consumers.is_empty() {
            return false;
        }
        let mut ordered = self.consumers.clone();
        ordered.sort();
        ordered == self.consumers
            && ordered.windows(2).all(|pair| {
                pair[0].identity_commitment != pair[1].identity_commitment
                    && (pair[0].kind_tag, pair[0].identity_commitment)
                        < (pair[1].kind_tag, pair[1].identity_commitment)
            })
            && ordered.iter().all(|consumer| {
                consumer.kind_tag != 0
                    && [
                        consumer.identity_commitment,
                        consumer.descriptor_commitment,
                        consumer.binary_commitment,
                        consumer.protocol_commitment,
                        consumer.release_commitment,
                        consumer.currentness_commitment,
                    ]
                    .iter()
                    .all(|value| *value != [0; 32])
            })
    }
}

pub(in crate::domain::vnext::integration) mod consumer_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext) trait ConsumerClosureProviderV1:
    consumer_sealed::Sealed
{
    fn current_facts(&self) -> Option<ConsumerClosureFactsV1>;
}

pub(in crate::domain::vnext) struct HostConsumerAdmissionGuardV1<'connection, P> {
    provider: &'connection mut P,
    initial: ConsumerClosureFactsV1,
    _exclusive: PhantomData<&'connection mut P>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'connection, P: ConsumerClosureProviderV1> HostConsumerAdmissionGuardV1<'connection, P> {
    pub(in crate::domain::vnext::integration) fn bind(
        provider: &'connection mut P,
    ) -> Result<Self, ConsumerClosureErrorV1> {
        let initial = provider
            .current_facts()
            .ok_or(ConsumerClosureErrorV1::Unavailable)?;
        if !initial.is_valid() {
            return Err(ConsumerClosureErrorV1::InvalidClosure);
        }
        Ok(Self {
            provider,
            initial,
            _exclusive: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext) fn closure_commitments(&self) -> Vec<[u8; 32]> {
        let mut commitments = vec![
            self.initial.host_process_incarnation,
            self.initial.connection_incarnation,
            self.initial.discovery_revision,
            self.initial.autoload_revision,
            self.initial.dispatch_revision,
            self.initial.old_reader_revision,
            self.initial.writer_revision,
            self.initial.alias_revision,
            self.initial.legacy_reader_revision,
            self.initial.declared_slot_revision,
            self.initial.admission_set_epoch,
            self.initial.monotonic_currentness_fence,
            self.initial.revocation_fence,
        ];
        for consumer in &self.initial.consumers {
            commitments.extend([
                [consumer.kind_tag; 32],
                [u8::from(consumer.required); 32],
                consumer.identity_commitment,
                consumer.descriptor_commitment,
                consumer.binary_commitment,
                consumer.protocol_commitment,
                consumer.release_commitment,
                consumer.currentness_commitment,
            ]);
        }
        commitments
    }

    pub(in crate::domain::vnext) fn consume_final_recheck(
        self,
    ) -> Result<(), ConsumerClosureErrorV1> {
        if self.provider.current_facts() != Some(self.initial) {
            return Err(ConsumerClosureErrorV1::Changed);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum ConsumerClosureErrorV1 {
    #[error("authenticated complete consumer universe is unavailable")]
    Unavailable,
    #[error("authenticated consumer closure is invalid")]
    InvalidClosure,
    #[error("authenticated consumer closure changed before finality")]
    Changed,
}

#[cfg(test)]
pub(in crate::domain::vnext) mod test_seed {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    pub struct TestProviderV1 {
        facts: Rc<RefCell<Option<ConsumerClosureFactsV1>>>,
    }

    impl TestProviderV1 {
        pub fn new(facts: ConsumerClosureFactsV1) -> Self {
            Self {
                facts: Rc::new(RefCell::new(Some(facts))),
            }
        }

        pub fn control(&self) -> Rc<RefCell<Option<ConsumerClosureFactsV1>>> {
            Rc::clone(&self.facts)
        }
    }

    impl consumer_sealed::Sealed for TestProviderV1 {}

    impl ConsumerClosureProviderV1 for TestProviderV1 {
        fn current_facts(&self) -> Option<ConsumerClosureFactsV1> {
            self.facts.borrow().clone()
        }
    }

    pub fn bind(
        provider: &mut TestProviderV1,
    ) -> Result<HostConsumerAdmissionGuardV1<'_, TestProviderV1>, ConsumerClosureErrorV1> {
        HostConsumerAdmissionGuardV1::bind(provider)
    }

    pub fn change_admission_epoch(
        control: &Rc<RefCell<Option<ConsumerClosureFactsV1>>>,
        value: [u8; 32],
    ) {
        if let Some(facts) = control.borrow_mut().as_mut() {
            facts.admission_set_epoch = value;
        }
    }

    pub fn standard_facts() -> ConsumerClosureFactsV1 {
        ConsumerClosureFactsV1 {
            host_process_incarnation: [30; 32],
            connection_incarnation: [31; 32],
            discovery_revision: [32; 32],
            autoload_revision: [33; 32],
            dispatch_revision: [34; 32],
            old_reader_revision: [35; 32],
            writer_revision: [36; 32],
            alias_revision: [37; 32],
            legacy_reader_revision: [38; 32],
            declared_slot_revision: [39; 32],
            admission_set_epoch: [40; 32],
            monotonic_currentness_fence: [41; 32],
            revocation_fence: [42; 32],
            consumers: vec![
                AdmittedConsumerIdentityV1 {
                    kind_tag: 1,
                    identity_commitment: [43; 32],
                    descriptor_commitment: [44; 32],
                    binary_commitment: [45; 32],
                    protocol_commitment: [46; 32],
                    release_commitment: [47; 32],
                    currentness_commitment: [48; 32],
                    required: true,
                },
                AdmittedConsumerIdentityV1 {
                    kind_tag: 2,
                    identity_commitment: [49; 32],
                    descriptor_commitment: [50; 32],
                    binary_commitment: [51; 32],
                    protocol_commitment: [52; 32],
                    release_commitment: [53; 32],
                    currentness_commitment: [54; 32],
                    required: false,
                },
            ],
        }
    }
}
