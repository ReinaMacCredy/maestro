use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

use crate::domain::installation::Stage12ConsumerReaderHoldClosureV2;

#[derive(Debug, Error, Eq, PartialEq)]
pub(in crate::domain) enum LegacyRemovalGuardAdmissionErrorV2 {
    #[error("the complete owner-issued legacy removal facts do not describe one exact cut")]
    OwnerFactsMismatch,
    #[error("the live Authority state is absent, inactive, or not an Installation authority")]
    InvalidAuthorityState,
    #[error("the live Authority currentness or revocation state changed before guard minting")]
    AuthorityCurrentnessDrift,
    #[error("Authority could not issue a unique process-local pruning invocation")]
    InvocationUnavailable,
}

pub(super) struct LegacyRemovalGuardBindingV2 {
    _realm: [u8; 32],
    _release: [u8; 32],
    _store_generation: [u8; 32],
    _store_head: [u8; 32],
    _invocation: [u8; 32],
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
        invocation: [u8; 32],
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
}

struct AuthorityRemovalCutLifetimeV2;

pub(in crate::domain) struct LegacyRemovalGuardV2<'cut> {
    _binding: LegacyRemovalGuardBindingV2,
    _cut: PhantomData<&'cut mut AuthorityRemovalCutLifetimeV2>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'cut> LegacyRemovalGuardV2<'cut> {
    pub(super) const fn mint(binding: LegacyRemovalGuardBindingV2) -> Self {
        Self {
            _binding: binding,
            _cut: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain) fn consume_for(
        self,
        closure: &Stage12ConsumerReaderHoldClosureV2,
    ) -> Result<(), LegacyRemovalGuardAdmissionErrorV2> {
        if self._binding._consumer_closure != *closure.identity().as_bytes()
            || self._binding._release != *closure.release_id().as_bytes()
            || self._binding._quarantine_epoch != *closure.legacy_quarantine_epoch_id().as_bytes()
            || self._binding._sighting_manifest != *closure.sighting_manifest_id().as_bytes()
            || self._binding._replacement_activation
                != *closure.replacement_activation_id().as_bytes()
            || self._binding._reader_closure
                != *closure.physical_pruning_reader_zero_id().as_bytes()
            || self._binding._hold_closure != *closure.physical_pruning_hold_zero_id().as_bytes()
            || self._binding._rollback_rehearsal != *closure.rollback_rehearsal_id().as_bytes()
            || self._binding._deletion_plan != *closure.deletion_plan_id().as_bytes()
        {
            return Err(LegacyRemovalGuardAdmissionErrorV2::OwnerFactsMismatch);
        }
        Ok(())
    }
}
