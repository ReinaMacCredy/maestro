use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::installation::Stage12ConsumerReaderHoldClosureV2;
use crate::domain::persistence::{PreparedPublicationError, StoreV1};

use super::facade::observe_legacy_removal_guard_currentness;

#[derive(Debug, Error, Eq, PartialEq)]
pub(in crate::domain) enum LegacyRemovalGuardAdmissionErrorV2 {
    #[error("the complete owner-issued legacy removal facts do not describe one exact cut")]
    OwnerFactsMismatch,
    #[error("the live Authority state is absent, inactive, or not an Installation authority")]
    InvalidAuthorityState,
    #[error(
        "the live Authority currentness or revocation state changed before pruning linearization"
    )]
    AuthorityCurrentnessDrift,
    #[error("Authority could not issue a unique process-local pruning invocation")]
    InvocationUnavailable,
    #[error("legacy removal requires Authority-bracketed expected-old linearization")]
    LinearizationRequired,
}

pub(super) struct LegacyRemovalGuardBindingV2 {
    _realm: [u8; 32],
    _release: [u8; 32],
    _store_generation: [u8; 32],
    _store_head: [u8; 32],
    _invocation: LegacyRemovalInvocationBindingV2,
    _source_case_manifest: [u8; 32],
    _sighting_manifest: [u8; 32],
    _classification_manifest: [u8; 32],
    _overlap_manifest: [u8; 32],
    _loss_manifest: [u8; 32],
    _quarantine_manifest: [u8; 32],
    _quarantine_epoch: [u8; 32],
    _replacement_activation: [u8; 32],
    _consumer_closure: [u8; 32],
    _reader_closure: [u8; 32],
    _hold_closure: [u8; 32],
    _rollback_rehearsal: [u8; 32],
    _deletion_plan: [u8; 32],
    _authority_snapshot: [u8; 32],
    _authority_epoch: u64,
    _trust_root: [u8; 32],
    _authority_fence: [u8; 32],
    _state_token: [u8; 32],
    _authority_currentness: [u8; 32],
    _revocation_revision: u64,
    _revocation_state: [u8; 32],
}

pub(super) struct LegacyRemovalInvocationBindingV2 {
    commitment: [u8; 32],
    process_incarnation: [u8; 32],
    entropy: [u8; 32],
    sequence: u64,
}

impl LegacyRemovalInvocationBindingV2 {
    pub(super) fn mint(
        process_incarnation: [u8; 32],
        entropy: [u8; 32],
        sequence: u64,
        current_head: &[u8; 32],
        epoch: &[u8; 32],
        activation: &[u8; 32],
        deletion_plan: &[u8; 32],
    ) -> Result<Self, LegacyRemovalGuardAdmissionErrorV2> {
        if process_incarnation == [0; 32] || entropy == [0; 32] || sequence == 0 {
            return Err(LegacyRemovalGuardAdmissionErrorV2::InvocationUnavailable);
        }
        Ok(Self {
            commitment: legacy_removal_invocation_commitment(
                &process_incarnation,
                &entropy,
                sequence,
                current_head,
                epoch,
                activation,
                deletion_plan,
            ),
            process_incarnation,
            entropy,
            sequence,
        })
    }

    fn recheck(
        &self,
        current_head: &[u8; 32],
        epoch: &[u8; 32],
        activation: &[u8; 32],
        deletion_plan: &[u8; 32],
    ) -> bool {
        self.process_incarnation != [0; 32]
            && self.entropy != [0; 32]
            && self.sequence != 0
            && self.commitment
                == legacy_removal_invocation_commitment(
                    &self.process_incarnation,
                    &self.entropy,
                    self.sequence,
                    current_head,
                    epoch,
                    activation,
                    deletion_plan,
                )
    }

    #[cfg(test)]
    fn corrupt_for_test(&mut self) {
        self.commitment[0] ^= 1;
    }
}

fn legacy_removal_invocation_commitment(
    process_incarnation: &[u8; 32],
    entropy: &[u8; 32],
    sequence: u64,
    current_head: &[u8; 32],
    epoch: &[u8; 32],
    activation: &[u8; 32],
    deletion_plan: &[u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    let domain = b"maestro.authority.legacy-removal-invocation.v2\0";
    let sequence_bytes = sequence.to_be_bytes();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in [
        process_incarnation.as_slice(),
        entropy.as_slice(),
        sequence_bytes.as_slice(),
        current_head.as_slice(),
        epoch.as_slice(),
        activation.as_slice(),
        deletion_plan.as_slice(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[derive(Clone, Copy)]
pub(super) struct LegacyRemovalGuardCurrentnessV2 {
    pub(super) realm: [u8; 32],
    pub(super) store_generation: [u8; 32],
    pub(super) store_head: [u8; 32],
    pub(super) authority_snapshot: [u8; 32],
    pub(super) authority_epoch: u64,
    pub(super) trust_root: [u8; 32],
    pub(super) authority_fence: [u8; 32],
    pub(super) state_token: [u8; 32],
    pub(super) authority_currentness: [u8; 32],
    pub(super) revocation_revision: u64,
    pub(super) revocation_state: [u8; 32],
}

#[derive(Clone, Copy)]
pub(super) struct LegacyRemovalConsumerBindingV2 {
    pub(super) consumer_closure: [u8; 32],
    pub(super) release: [u8; 32],
    pub(super) quarantine_epoch: [u8; 32],
    pub(super) sighting_manifest: [u8; 32],
    pub(super) replacement_activation: [u8; 32],
    pub(super) reader_closure: [u8; 32],
    pub(super) hold_closure: [u8; 32],
    pub(super) rollback_rehearsal: [u8; 32],
    pub(super) deletion_plan: [u8; 32],
}

impl LegacyRemovalConsumerBindingV2 {
    fn observe(closure: &Stage12ConsumerReaderHoldClosureV2) -> Self {
        Self {
            consumer_closure: *closure.identity().as_bytes(),
            release: *closure.release_id().as_bytes(),
            quarantine_epoch: *closure.legacy_quarantine_epoch_id().as_bytes(),
            sighting_manifest: *closure.sighting_manifest_id().as_bytes(),
            replacement_activation: *closure.replacement_activation_id().as_bytes(),
            reader_closure: *closure.physical_pruning_reader_zero_id().as_bytes(),
            hold_closure: *closure.physical_pruning_hold_zero_id().as_bytes(),
            rollback_rehearsal: *closure.rollback_rehearsal_id().as_bytes(),
            deletion_plan: *closure.deletion_plan_id().as_bytes(),
        }
    }
}

impl LegacyRemovalGuardBindingV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the removal guard binds every independently owner-issued pruning fact"
    )]
    pub(super) const fn new(
        realm: [u8; 32],
        release: [u8; 32],
        store_generation: [u8; 32],
        store_head: [u8; 32],
        invocation: LegacyRemovalInvocationBindingV2,
        source_case_manifest: [u8; 32],
        sighting_manifest: [u8; 32],
        classification_manifest: [u8; 32],
        overlap_manifest: [u8; 32],
        loss_manifest: [u8; 32],
        quarantine_manifest: [u8; 32],
        quarantine_epoch: [u8; 32],
        replacement_activation: [u8; 32],
        consumer_closure: [u8; 32],
        reader_closure: [u8; 32],
        hold_closure: [u8; 32],
        rollback_rehearsal: [u8; 32],
        deletion_plan: [u8; 32],
        authority_snapshot: [u8; 32],
        authority_epoch: u64,
        trust_root: [u8; 32],
        authority_fence: [u8; 32],
        state_token: [u8; 32],
        authority_currentness: [u8; 32],
        revocation_revision: u64,
        revocation_state: [u8; 32],
    ) -> Self {
        Self {
            _realm: realm,
            _release: release,
            _store_generation: store_generation,
            _store_head: store_head,
            _invocation: invocation,
            _source_case_manifest: source_case_manifest,
            _sighting_manifest: sighting_manifest,
            _classification_manifest: classification_manifest,
            _overlap_manifest: overlap_manifest,
            _loss_manifest: loss_manifest,
            _quarantine_manifest: quarantine_manifest,
            _quarantine_epoch: quarantine_epoch,
            _replacement_activation: replacement_activation,
            _consumer_closure: consumer_closure,
            _reader_closure: reader_closure,
            _hold_closure: hold_closure,
            _rollback_rehearsal: rollback_rehearsal,
            _deletion_plan: deletion_plan,
            _authority_snapshot: authority_snapshot,
            _authority_epoch: authority_epoch,
            _trust_root: trust_root,
            _authority_fence: authority_fence,
            _state_token: state_token,
            _authority_currentness: authority_currentness,
            _revocation_revision: revocation_revision,
            _revocation_state: revocation_state,
        }
    }

    fn recheck(
        &self,
        currentness: LegacyRemovalGuardCurrentnessV2,
        consumer: LegacyRemovalConsumerBindingV2,
    ) -> Result<(), LegacyRemovalGuardAdmissionErrorV2> {
        if !self._invocation.recheck(
            &self._store_head,
            &self._quarantine_epoch,
            &self._replacement_activation,
            &self._deletion_plan,
        ) {
            return Err(LegacyRemovalGuardAdmissionErrorV2::InvocationUnavailable);
        }
        if self._source_case_manifest == [0; 32]
            || self._classification_manifest == [0; 32]
            || self._overlap_manifest == [0; 32]
            || self._loss_manifest == [0; 32]
            || self._quarantine_manifest == [0; 32]
        {
            return Err(LegacyRemovalGuardAdmissionErrorV2::OwnerFactsMismatch);
        }
        if self._consumer_closure != consumer.consumer_closure
            || self._release != consumer.release
            || self._quarantine_epoch != consumer.quarantine_epoch
            || self._sighting_manifest != consumer.sighting_manifest
            || self._replacement_activation != consumer.replacement_activation
            || self._reader_closure != consumer.reader_closure
            || self._hold_closure != consumer.hold_closure
            || self._rollback_rehearsal != consumer.rollback_rehearsal
            || self._deletion_plan != consumer.deletion_plan
        {
            return Err(LegacyRemovalGuardAdmissionErrorV2::OwnerFactsMismatch);
        }
        if self._realm != currentness.realm
            || self._store_generation != currentness.store_generation
            || self._store_head != currentness.store_head
            || self._authority_snapshot != currentness.authority_snapshot
            || self._authority_epoch != currentness.authority_epoch
            || self._trust_root != currentness.trust_root
            || self._authority_fence != currentness.authority_fence
            || self._state_token != currentness.state_token
            || self._authority_currentness != currentness.authority_currentness
            || self._revocation_revision != currentness.revocation_revision
            || self._revocation_state != currentness.revocation_state
        {
            return Err(LegacyRemovalGuardAdmissionErrorV2::AuthorityCurrentnessDrift);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn test_from_currentness(
        currentness: LegacyRemovalGuardCurrentnessV2,
        consumer: LegacyRemovalConsumerBindingV2,
    ) -> Self {
        Self::new(
            currentness.realm,
            consumer.release,
            currentness.store_generation,
            currentness.store_head,
            LegacyRemovalInvocationBindingV2::mint(
                [1; 32],
                [2; 32],
                1,
                &currentness.store_head,
                &consumer.quarantine_epoch,
                &consumer.replacement_activation,
                &consumer.deletion_plan,
            )
            .expect("test invocation basis is valid"),
            [7; 32],
            consumer.sighting_manifest,
            [8; 32],
            [9; 32],
            [10; 32],
            [11; 32],
            consumer.quarantine_epoch,
            consumer.replacement_activation,
            consumer.consumer_closure,
            consumer.reader_closure,
            consumer.hold_closure,
            consumer.rollback_rehearsal,
            consumer.deletion_plan,
            currentness.authority_snapshot,
            currentness.authority_epoch,
            currentness.trust_root,
            currentness.authority_fence,
            currentness.state_token,
            currentness.authority_currentness,
            currentness.revocation_revision,
            currentness.revocation_state,
        )
    }

    #[cfg(test)]
    pub(super) fn corrupt_invocation_for_test(&mut self) {
        self._invocation.corrupt_for_test();
    }
}

pub(in crate::domain) struct LegacyRemovalGuardV2<'cut> {
    _binding: LegacyRemovalGuardBindingV2,
    _store: &'cut mut StoreV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'cut> LegacyRemovalGuardV2<'cut> {
    pub(super) const fn mint(
        binding: LegacyRemovalGuardBindingV2,
        store: &'cut mut StoreV1,
    ) -> Self {
        Self {
            _binding: binding,
            _store: store,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain) fn consume_for(
        self,
        closure: &Stage12ConsumerReaderHoldClosureV2,
    ) -> Result<(), LegacyRemovalGuardAdmissionErrorV2> {
        match self.consume_with_linearization(closure, || {
            Err::<std::convert::Infallible, _>(
                LegacyRemovalGuardAdmissionErrorV2::LinearizationRequired,
            )
        })? {
            Ok(never) => match never {},
            Err(error) => Err(error),
        }
    }

    pub(in crate::domain) fn consume_with_linearization<T, E>(
        self,
        closure: &Stage12ConsumerReaderHoldClosureV2,
        linearize_expected_old: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, LegacyRemovalGuardAdmissionErrorV2> {
        self.consume_bound_with_linearization(
            LegacyRemovalConsumerBindingV2::observe(closure),
            linearize_expected_old,
        )
    }

    fn consume_bound_with_linearization<T, E>(
        self,
        consumer: LegacyRemovalConsumerBindingV2,
        linearize_expected_old: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, LegacyRemovalGuardAdmissionErrorV2> {
        let Self {
            _binding: binding,
            _store: store,
            _not_send_or_sync: _,
        } = self;
        match store.with_serialized_active_view(|view| {
            let currentness = observe_legacy_removal_guard_currentness(view)?;
            binding.recheck(currentness, consumer)?;
            Ok(linearize_expected_old())
        }) {
            Ok(result) => Ok(result),
            Err(PreparedPublicationError::Store(_)) => {
                Err(LegacyRemovalGuardAdmissionErrorV2::InvalidAuthorityState)
            }
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    #[cfg(test)]
    pub(super) fn consume_for_test<T, E>(
        self,
        consumer: LegacyRemovalConsumerBindingV2,
        linearize_expected_old: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, LegacyRemovalGuardAdmissionErrorV2> {
        self.consume_bound_with_linearization(consumer, linearize_expected_old)
    }
}
