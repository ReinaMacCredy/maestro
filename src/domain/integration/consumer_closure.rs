use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::domain) enum AdmittedConsumerKindV1 {
    PublicCli,
    PublicSkill,
    GlobalMcp,
    HostActivation,
    LegacyReader,
    OldReader,
}

impl AdmittedConsumerKindV1 {
    const ALL: [Self; 6] = [
        Self::PublicCli,
        Self::PublicSkill,
        Self::GlobalMcp,
        Self::HostActivation,
        Self::LegacyReader,
        Self::OldReader,
    ];

    const fn tag(self) -> u8 {
        match self {
            Self::PublicCli => 1,
            Self::PublicSkill => 2,
            Self::GlobalMcp => 3,
            Self::HostActivation => 4,
            Self::LegacyReader => 5,
            Self::OldReader => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::domain) struct AdmittedConsumerIdentityV1 {
    kind: AdmittedConsumerKindV1,
    identity_commitment: [u8; 32],
    descriptor_commitment: [u8; 32],
    binary_commitment: [u8; 32],
    protocol_commitment: [u8; 32],
    release_commitment: [u8; 32],
    currentness_commitment: [u8; 32],
    required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::domain) struct ConsumerClosureFactsV1 {
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
                    && (pair[0].kind, pair[0].identity_commitment)
                        < (pair[1].kind, pair[1].identity_commitment)
            })
            && AdmittedConsumerKindV1::ALL.iter().all(|kind| {
                ordered
                    .iter()
                    .any(|consumer| consumer.kind == *kind && consumer.required)
            })
            && ordered.iter().all(|consumer| {
                [
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

pub(in crate::domain::integration) mod consumer_sealed {
    pub trait ProviderSealed {}
    pub trait LeaseSealed {}
}

pub(in crate::domain) trait ConsumerClosureProviderV1:
    consumer_sealed::ProviderSealed
{
    type Lease<'connection>: ConsumerClosureLeasePortV1
    where
        Self: 'connection;

    fn acquire_authenticated_complete_closure(
        &mut self,
    ) -> Result<Self::Lease<'_>, ConsumerClosureErrorV1>;
}

pub(in crate::domain) trait ConsumerClosureLeasePortV1:
    consumer_sealed::LeaseSealed
{
    fn initial(&self) -> &ConsumerClosureFactsV1;
    fn recheck_current(&mut self) -> Result<(), ConsumerClosureErrorV1>;
}

pub(in crate::domain) struct HostConsumerAdmissionGuardV1<'connection, L> {
    lease: L,
    initial: ConsumerClosureFactsV1,
    _exclusive: PhantomData<&'connection mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'connection, L: ConsumerClosureLeasePortV1> HostConsumerAdmissionGuardV1<'connection, L> {
    pub(in crate::domain::integration) fn bind(lease: L) -> Result<Self, ConsumerClosureErrorV1> {
        let initial = lease.initial().clone();
        if !initial.is_valid() {
            return Err(ConsumerClosureErrorV1::InvalidClosure);
        }
        Ok(Self {
            lease,
            initial,
            _exclusive: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain) fn closure_commitments(&self) -> Vec<[u8; 32]> {
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
                [consumer.kind.tag(); 32],
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

    pub(in crate::domain) fn recheck_current(&mut self) -> Result<(), ConsumerClosureErrorV1> {
        self.lease.recheck_current()
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain) enum ConsumerClosureErrorV1 {
    #[error("authenticated complete consumer universe is unavailable")]
    Unavailable,
    #[error("authenticated consumer closure is invalid")]
    InvalidClosure,
    #[error("authenticated consumer closure changed before finality")]
    Changed,
}

#[cfg(test)]
pub(in crate::domain) mod test_seed {
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

    pub struct TestLeaseV1<'connection> {
        provider: &'connection mut TestProviderV1,
        initial: ConsumerClosureFactsV1,
    }

    impl consumer_sealed::LeaseSealed for TestLeaseV1<'_> {}

    impl ConsumerClosureLeasePortV1 for TestLeaseV1<'_> {
        fn initial(&self) -> &ConsumerClosureFactsV1 {
            &self.initial
        }

        fn recheck_current(&mut self) -> Result<(), ConsumerClosureErrorV1> {
            if self.provider.facts.borrow().as_ref() != Some(&self.initial) {
                return Err(ConsumerClosureErrorV1::Changed);
            }
            Ok(())
        }
    }

    impl consumer_sealed::ProviderSealed for TestProviderV1 {}

    impl ConsumerClosureProviderV1 for TestProviderV1 {
        type Lease<'connection> = TestLeaseV1<'connection>;

        fn acquire_authenticated_complete_closure(
            &mut self,
        ) -> Result<Self::Lease<'_>, ConsumerClosureErrorV1> {
            let initial = self
                .facts
                .borrow()
                .clone()
                .ok_or(ConsumerClosureErrorV1::Unavailable)?;
            Ok(TestLeaseV1 {
                provider: self,
                initial,
            })
        }
    }

    pub fn bind(
        provider: &mut TestProviderV1,
    ) -> Result<HostConsumerAdmissionGuardV1<'_, TestLeaseV1<'_>>, ConsumerClosureErrorV1> {
        HostConsumerAdmissionGuardV1::bind(provider.acquire_authenticated_complete_closure()?)
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
            consumers: AdmittedConsumerKindV1::ALL
                .into_iter()
                .enumerate()
                .map(|(index, kind)| {
                    let base = 43 + (index as u8 * 6);
                    AdmittedConsumerIdentityV1 {
                        kind,
                        identity_commitment: [base; 32],
                        descriptor_commitment: [base + 1; 32],
                        binary_commitment: [base + 2; 32],
                        protocol_commitment: [base + 3; 32],
                        release_commitment: [base + 4; 32],
                        currentness_commitment: [base + 5; 32],
                        required: true,
                    }
                })
                .collect(),
        }
    }
}
