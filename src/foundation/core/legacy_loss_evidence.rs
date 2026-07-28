#![allow(
    dead_code,
    reason = "V8 owner-issued loss capability awaits MainIntegration owner wiring"
)]

use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::legacy_quarantine::LegacyQuarantineOwnerDomainV3;
use super::secure_fs::DescriptorCensusObjectKindV1;

const HISTORICAL_BINDING_DOMAIN_V1: &[u8] =
    b"maestro.foundation.legacy-source-historical-binding.v1\0";
const CURRENT_BINDING_DOMAIN_V1: &[u8] = b"maestro.foundation.legacy-source-current-binding.v1\0";
const OWNER_WITNESS_DOMAIN_V1: &[u8] =
    b"maestro.foundation.owner-unavailable-preexisting-loss-witness.v1\0";
const OWNER_EVIDENCE_SET_DOMAIN_V1: &[u8] =
    b"maestro.foundation.owner-issued-unavailable-preexisting-loss-evidence-set.v1\0";
const FOUNDATION_LOSS_RECEIPT_DOMAIN_V1: &[u8] =
    b"maestro.foundation.validated-unavailable-preexisting-loss-receipt.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LegacySourceHistoryKindV1 {
    RepositoryStore,
    InstallationStore,
    ProtectedPrimaryJournal,
}

impl LegacySourceHistoryKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::RepositoryStore => 1,
            Self::InstallationStore => 2,
            Self::ProtectedPrimaryJournal => 3,
        }
    }

    const fn owner(self) -> LegacyQuarantineOwnerDomainV3 {
        match self {
            Self::RepositoryStore => LegacyQuarantineOwnerDomainV3::Repository,
            Self::InstallationStore => LegacyQuarantineOwnerDomainV3::Installation,
            Self::ProtectedPrimaryJournal => LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
        }
    }
}

pub(crate) struct LegacySourceHistoricalBindingV1 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    snapshot_id: [u8; 32],
    issuer_id: [u8; 32],
    source_provenance_id: [u8; 32],
    root_binding: [u8; 32],
    relative_locator_commitment: [u8; 32],
    object_kind: DescriptorCensusObjectKindV1,
    object_identity: [u8; 32],
    expected_length: u64,
    content_sha256: [u8; 32],
    metadata_commitment: [u8; 32],
    namespace_epoch: u64,
    trust_root_id: [u8; 32],
    release_id: [u8; 32],
}

impl LegacySourceHistoricalBindingV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "owner history binds the complete immutable pre-loss tuple"
    )]
    fn from_owner(
        owner: LegacyQuarantineOwnerDomainV3,
        history_kind: LegacySourceHistoryKindV1,
        snapshot_id: [u8; 32],
        issuer_id: [u8; 32],
        history_instance_id: [u8; 32],
        historical_head_id: [u8; 32],
        historical_generation_id: [u8; 32],
        historical_state_revision: u64,
        namespace_epoch: u64,
        trust_root_id: [u8; 32],
        release_id: [u8; 32],
        provider_revision: u64,
        source_provenance_id: [u8; 32],
        root_binding: [u8; 32],
        relative_locator_commitment: [u8; 32],
        object_kind: DescriptorCensusObjectKindV1,
        object_identity: [u8; 32],
        expected_length: u64,
        content_sha256: [u8; 32],
        metadata_commitment: [u8; 32],
    ) -> Result<Self, FoundationLegacyLossEvidenceErrorV1> {
        if owner != history_kind.owner()
            || [
                snapshot_id,
                issuer_id,
                history_instance_id,
                historical_head_id,
                historical_generation_id,
                trust_root_id,
                release_id,
                source_provenance_id,
                root_binding,
                relative_locator_commitment,
                object_identity,
                content_sha256,
                metadata_commitment,
            ]
            .contains(&[0; 32])
            || historical_state_revision == 0
            || namespace_epoch == 0
            || provider_revision == 0
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::InvalidHistoricalBinding);
        }
        let kind_tag = [match object_kind {
            DescriptorCensusObjectKindV1::RegularFile => 1,
            DescriptorCensusObjectKindV1::SymbolicLink => 2,
        }];
        let identity = commitment(
            HISTORICAL_BINDING_DOMAIN_V1,
            &[
                &[owner.tag()],
                &[history_kind.tag()],
                &snapshot_id,
                &issuer_id,
                &history_instance_id,
                &historical_head_id,
                &historical_generation_id,
                &historical_state_revision.to_be_bytes(),
                &namespace_epoch.to_be_bytes(),
                &trust_root_id,
                &release_id,
                &provider_revision.to_be_bytes(),
                &source_provenance_id,
                &root_binding,
                &relative_locator_commitment,
                &kind_tag,
                &object_identity,
                &expected_length.to_be_bytes(),
                &content_sha256,
                &metadata_commitment,
            ],
        );
        Ok(Self {
            identity,
            owner,
            snapshot_id,
            issuer_id,
            source_provenance_id,
            root_binding,
            relative_locator_commitment,
            object_kind,
            object_identity,
            expected_length,
            content_sha256,
            metadata_commitment,
            namespace_epoch,
            trust_root_id,
            release_id,
        })
    }
}

pub(crate) struct LegacySourceCurrentBindingV1 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    expected_source_set_id: [u8; 32],
    operation_attempt: [u8; 32],
    owner_admission_id: [u8; 32],
    owner_currentness_id: [u8; 32],
    namespace_epoch: u64,
    trust_root_id: [u8; 32],
    release_id: [u8; 32],
}

impl LegacySourceCurrentBindingV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "fresh owner issuance binds the complete Store or journal currentness tuple"
    )]
    fn from_owner(
        owner: LegacyQuarantineOwnerDomainV3,
        expected_source_set_id: [u8; 32],
        operation_attempt: [u8; 32],
        owner_admission_id: [u8; 32],
        owner_currentness_id: [u8; 32],
        current_head_id: [u8; 32],
        current_generation_id: [u8; 32],
        current_state_revision: u64,
        namespace_epoch: u64,
        trust_root_id: [u8; 32],
        release_id: [u8; 32],
        provider_id: [u8; 32],
        mount_id: [u8; 32],
        anchor_id: [u8; 32],
        fence_id: [u8; 32],
        revocation_revision: u64,
    ) -> Result<Self, FoundationLegacyLossEvidenceErrorV1> {
        if [
            expected_source_set_id,
            operation_attempt,
            owner_admission_id,
            owner_currentness_id,
            current_head_id,
            current_generation_id,
            trust_root_id,
            release_id,
            provider_id,
            mount_id,
            anchor_id,
            fence_id,
        ]
        .contains(&[0; 32])
            || current_state_revision == 0
            || namespace_epoch == 0
            || revocation_revision == 0
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::InvalidCurrentBinding);
        }
        let identity = commitment(
            CURRENT_BINDING_DOMAIN_V1,
            &[
                &[owner.tag()],
                &expected_source_set_id,
                &operation_attempt,
                &owner_admission_id,
                &owner_currentness_id,
                &current_head_id,
                &current_generation_id,
                &current_state_revision.to_be_bytes(),
                &namespace_epoch.to_be_bytes(),
                &trust_root_id,
                &release_id,
                &provider_id,
                &mount_id,
                &anchor_id,
                &fence_id,
                &revocation_revision.to_be_bytes(),
            ],
        );
        Ok(Self {
            identity,
            owner,
            expected_source_set_id,
            operation_attempt,
            owner_admission_id,
            owner_currentness_id,
            namespace_epoch,
            trust_root_id,
            release_id,
        })
    }
}

pub(crate) struct OwnerUnavailablePreexistingLossWitnessV1 {
    identity: [u8; 32],
    historical: LegacySourceHistoricalBindingV1,
    current: LegacySourceCurrentBindingV1,
}

impl OwnerUnavailablePreexistingLossWitnessV1 {
    fn from_owner(
        historical: LegacySourceHistoricalBindingV1,
        current: LegacySourceCurrentBindingV1,
    ) -> Result<Self, FoundationLegacyLossEvidenceErrorV1> {
        if historical.owner != current.owner
            || historical.namespace_epoch != current.namespace_epoch
            || historical.trust_root_id != current.trust_root_id
            || historical.release_id != current.release_id
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::OwnerBindingMismatch);
        }
        let identity = commitment(
            OWNER_WITNESS_DOMAIN_V1,
            &[&historical.identity, &current.identity],
        );
        Ok(Self {
            identity,
            historical,
            current,
        })
    }

    pub(in crate::foundation::core) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(in crate::foundation::core) const fn owner(&self) -> LegacyQuarantineOwnerDomainV3 {
        self.historical.owner
    }

    pub(in crate::foundation::core) const fn expected_source_set_id(&self) -> [u8; 32] {
        self.current.expected_source_set_id
    }

    pub(in crate::foundation::core) const fn operation_attempt(&self) -> [u8; 32] {
        self.current.operation_attempt
    }

    pub(in crate::foundation::core) const fn owner_admission_id(&self) -> [u8; 32] {
        self.current.owner_admission_id
    }

    pub(in crate::foundation::core) const fn owner_currentness_id(&self) -> [u8; 32] {
        self.current.owner_currentness_id
    }

    pub(in crate::foundation::core) const fn source_provenance_id(&self) -> [u8; 32] {
        self.historical.source_provenance_id
    }

    pub(in crate::foundation::core) const fn root_binding(&self) -> [u8; 32] {
        self.historical.root_binding
    }

    pub(in crate::foundation::core) const fn relative_locator_commitment(&self) -> [u8; 32] {
        self.historical.relative_locator_commitment
    }

    pub(in crate::foundation::core) const fn object_kind(&self) -> DescriptorCensusObjectKindV1 {
        self.historical.object_kind
    }

    pub(in crate::foundation::core) const fn object_identity(&self) -> [u8; 32] {
        self.historical.object_identity
    }

    pub(in crate::foundation::core) const fn expected_length(&self) -> u64 {
        self.historical.expected_length
    }

    pub(in crate::foundation::core) const fn content_sha256(&self) -> [u8; 32] {
        self.historical.content_sha256
    }

    pub(in crate::foundation::core) const fn metadata_commitment(&self) -> [u8; 32] {
        self.historical.metadata_commitment
    }

    pub(in crate::foundation::core) const fn snapshot_id(&self) -> [u8; 32] {
        self.historical.snapshot_id
    }

    pub(in crate::foundation::core) const fn issuer_id(&self) -> [u8; 32] {
        self.historical.issuer_id
    }

    pub(in crate::foundation::core) const fn historical_tuple_id(&self) -> [u8; 32] {
        self.historical.identity
    }

    pub(in crate::foundation::core) const fn current_tuple_id(&self) -> [u8; 32] {
        self.current.identity
    }
}

pub(in crate::foundation::core) struct OwnerIssuedUnavailablePreexistingLossEvidenceSetV1 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    expected_source_set_id: [u8; 32],
    operation_attempt: [u8; 32],
    owner_admission_id: [u8; 32],
    owner_currentness_id: [u8; 32],
    witnesses: Vec<OwnerUnavailablePreexistingLossWitnessV1>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(in crate::foundation::core) struct FoundationConsumedOwnerEvidenceSetV1 {
    pub(in crate::foundation::core) identity: [u8; 32],
    pub(in crate::foundation::core) owner: LegacyQuarantineOwnerDomainV3,
    pub(in crate::foundation::core) expected_source_set_id: [u8; 32],
    pub(in crate::foundation::core) operation_attempt: [u8; 32],
    pub(in crate::foundation::core) owner_admission_id: [u8; 32],
    pub(in crate::foundation::core) owner_currentness_id: [u8; 32],
    pub(in crate::foundation::core) witnesses: Vec<OwnerUnavailablePreexistingLossWitnessV1>,
}

pub(crate) struct FoundationOwnerEvidenceIssuanceBindingV1 {
    owner: LegacyQuarantineOwnerDomainV3,
    expected_source_set_id: [u8; 32],
    operation_attempt: [u8; 32],
    owner_admission_id: [u8; 32],
    owner_currentness_id: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl FoundationOwnerEvidenceIssuanceBindingV1 {
    pub(in crate::foundation::core) fn from_foundation(
        owner: LegacyQuarantineOwnerDomainV3,
        expected_source_set_id: [u8; 32],
        operation_attempt: [u8; 32],
        owner_admission_id: [u8; 32],
        owner_currentness_id: [u8; 32],
    ) -> Result<Self, FoundationLegacyLossEvidenceErrorV1> {
        if [
            expected_source_set_id,
            operation_attempt,
            owner_admission_id,
            owner_currentness_id,
        ]
        .contains(&[0; 32])
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::InvalidIssuanceBinding);
        }
        Ok(Self {
            owner,
            expected_source_set_id,
            operation_attempt,
            owner_admission_id,
            owner_currentness_id,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(crate) struct FoundationOwnerEvidenceMintV1 {
    binding: FoundationOwnerEvidenceIssuanceBindingV1,
    witnesses: Vec<OwnerUnavailablePreexistingLossWitnessV1>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl FoundationOwnerEvidenceMintV1 {
    pub(in crate::foundation::core) fn from_foundation(
        binding: FoundationOwnerEvidenceIssuanceBindingV1,
    ) -> Self {
        Self {
            binding,
            witnesses: Vec::new(),
            _not_send_or_sync: PhantomData,
        }
    }

    pub(crate) const fn owner(&self) -> LegacyQuarantineOwnerDomainV3 {
        self.binding.owner
    }

    pub(crate) const fn expected_source_set_id(&self) -> [u8; 32] {
        self.binding.expected_source_set_id
    }

    pub(crate) const fn operation_attempt(&self) -> [u8; 32] {
        self.binding.operation_attempt
    }

    pub(crate) const fn owner_admission_id(&self) -> [u8; 32] {
        self.binding.owner_admission_id
    }

    pub(crate) const fn owner_currentness_id(&self) -> [u8; 32] {
        self.binding.owner_currentness_id
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the owner records one complete historical/current observation into a Foundation-bound one-use mint"
    )]
    pub(crate) fn record_unavailable_preexisting_loss(
        &mut self,
        history_kind: LegacySourceHistoryKindV1,
        snapshot_id: [u8; 32],
        issuer_id: [u8; 32],
        history_instance_id: [u8; 32],
        historical_head_id: [u8; 32],
        historical_generation_id: [u8; 32],
        historical_state_revision: u64,
        namespace_epoch: u64,
        trust_root_id: [u8; 32],
        release_id: [u8; 32],
        provider_revision: u64,
        source_provenance_id: [u8; 32],
        root_binding: [u8; 32],
        relative_locator_commitment: [u8; 32],
        object_kind: DescriptorCensusObjectKindV1,
        object_identity: [u8; 32],
        expected_length: u64,
        content_sha256: [u8; 32],
        metadata_commitment: [u8; 32],
        current_head_id: [u8; 32],
        current_generation_id: [u8; 32],
        current_state_revision: u64,
        provider_id: [u8; 32],
        mount_id: [u8; 32],
        anchor_id: [u8; 32],
        fence_id: [u8; 32],
        revocation_revision: u64,
    ) -> Result<(), FoundationLegacyLossEvidenceErrorV1> {
        let historical = LegacySourceHistoricalBindingV1::from_owner(
            self.binding.owner,
            history_kind,
            snapshot_id,
            issuer_id,
            history_instance_id,
            historical_head_id,
            historical_generation_id,
            historical_state_revision,
            namespace_epoch,
            trust_root_id,
            release_id,
            provider_revision,
            source_provenance_id,
            root_binding,
            relative_locator_commitment,
            object_kind,
            object_identity,
            expected_length,
            content_sha256,
            metadata_commitment,
        )?;
        let current = LegacySourceCurrentBindingV1::from_owner(
            self.binding.owner,
            self.binding.expected_source_set_id,
            self.binding.operation_attempt,
            self.binding.owner_admission_id,
            self.binding.owner_currentness_id,
            current_head_id,
            current_generation_id,
            current_state_revision,
            namespace_epoch,
            trust_root_id,
            release_id,
            provider_id,
            mount_id,
            anchor_id,
            fence_id,
            revocation_revision,
        )?;
        self.witnesses
            .push(OwnerUnavailablePreexistingLossWitnessV1::from_owner(
                historical, current,
            )?);
        Ok(())
    }

    pub(in crate::foundation::core) fn finish(
        self,
    ) -> Result<
        OwnerIssuedUnavailablePreexistingLossEvidenceSetV1,
        FoundationLegacyLossEvidenceErrorV1,
    > {
        OwnerIssuedUnavailablePreexistingLossEvidenceSetV1::from_owner(
            self.binding.owner,
            self.binding.expected_source_set_id,
            self.binding.operation_attempt,
            self.binding.owner_admission_id,
            self.binding.owner_currentness_id,
            self.witnesses,
        )
    }
}

pub(crate) mod owner_loss_evidence_issuer_sealed {
    pub trait Sealed {}
}

pub(crate) trait OwnerUnavailablePreexistingLossEvidenceIssuerPortV1:
    owner_loss_evidence_issuer_sealed::Sealed
{
    fn issue_for_foundation(
        self,
        mint: &mut FoundationOwnerEvidenceMintV1,
    ) -> Result<(), FoundationLegacyLossEvidenceErrorV1>;
}

impl OwnerIssuedUnavailablePreexistingLossEvidenceSetV1 {
    fn from_owner(
        owner: LegacyQuarantineOwnerDomainV3,
        expected_source_set_id: [u8; 32],
        operation_attempt: [u8; 32],
        owner_admission_id: [u8; 32],
        owner_currentness_id: [u8; 32],
        mut witnesses: Vec<OwnerUnavailablePreexistingLossWitnessV1>,
    ) -> Result<Self, FoundationLegacyLossEvidenceErrorV1> {
        if [
            expected_source_set_id,
            operation_attempt,
            owner_admission_id,
            owner_currentness_id,
        ]
        .contains(&[0; 32])
            || witnesses.iter().any(|witness| {
                witness.owner() != owner
                    || witness.expected_source_set_id() != expected_source_set_id
                    || witness.operation_attempt() != operation_attempt
                    || witness.owner_admission_id() != owner_admission_id
                    || witness.owner_currentness_id() != owner_currentness_id
            })
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::InvalidEvidenceSet);
        }
        witnesses.sort_by_key(OwnerUnavailablePreexistingLossWitnessV1::identity);
        if witnesses.windows(2).any(|pair| {
            pair[0].identity() == pair[1].identity()
                || (
                    pair[0].root_binding(),
                    pair[0].relative_locator_commitment(),
                ) == (
                    pair[1].root_binding(),
                    pair[1].relative_locator_commitment(),
                )
        }) {
            return Err(FoundationLegacyLossEvidenceErrorV1::DuplicateWitness);
        }
        let owner_tag = [owner.tag()];
        let mut parts = vec![
            owner_tag.as_slice(),
            expected_source_set_id.as_slice(),
            operation_attempt.as_slice(),
            owner_admission_id.as_slice(),
            owner_currentness_id.as_slice(),
        ];
        parts.extend(witnesses.iter().map(|witness| witness.identity.as_slice()));
        let identity = commitment(OWNER_EVIDENCE_SET_DOMAIN_V1, &parts);
        Ok(Self {
            identity,
            owner,
            expected_source_set_id,
            operation_attempt,
            owner_admission_id,
            owner_currentness_id,
            witnesses,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::foundation::core) fn into_foundation_witnesses(
        self,
    ) -> FoundationConsumedOwnerEvidenceSetV1 {
        FoundationConsumedOwnerEvidenceSetV1 {
            identity: self.identity,
            owner: self.owner,
            expected_source_set_id: self.expected_source_set_id,
            operation_attempt: self.operation_attempt,
            owner_admission_id: self.owner_admission_id,
            owner_currentness_id: self.owner_currentness_id,
            witnesses: self.witnesses,
        }
    }
}

pub(crate) struct FoundationValidatedUnavailablePreexistingLossReceiptV1 {
    identity: [u8; 32],
    source_token: [u8; 32],
    snapshot_id: [u8; 32],
    issuer_id: [u8; 32],
    historical_tuple_id: [u8; 32],
    current_tuple_id: [u8; 32],
    source_provenance_id: [u8; 32],
    owner_admission_id: [u8; 32],
    owner_currentness_id: [u8; 32],
    validation_invocation: [u8; 32],
    pass_a_absence_id: [u8; 32],
    pass_b_absence_id: [u8; 32],
}

impl FoundationValidatedUnavailablePreexistingLossReceiptV1 {
    pub(in crate::foundation::core) fn from_foundation(
        source_token: [u8; 32],
        witness: &OwnerUnavailablePreexistingLossWitnessV1,
        validation_invocation: [u8; 32],
        admitted_set_id: [u8; 32],
        pass_a_absence_id: [u8; 32],
        pass_b_absence_id: [u8; 32],
    ) -> Result<Self, FoundationLegacyLossEvidenceErrorV1> {
        if [
            source_token,
            validation_invocation,
            admitted_set_id,
            pass_a_absence_id,
            pass_b_absence_id,
        ]
        .contains(&[0; 32])
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::InvalidFoundationReceipt);
        }
        let identity = commitment(
            FOUNDATION_LOSS_RECEIPT_DOMAIN_V1,
            &[
                &source_token,
                &witness.identity,
                &validation_invocation,
                &admitted_set_id,
                &pass_a_absence_id,
                &pass_b_absence_id,
            ],
        );
        Ok(Self {
            identity,
            source_token,
            snapshot_id: witness.snapshot_id(),
            issuer_id: witness.issuer_id(),
            historical_tuple_id: witness.historical_tuple_id(),
            current_tuple_id: witness.current_tuple_id(),
            source_provenance_id: witness.source_provenance_id(),
            owner_admission_id: witness.owner_admission_id(),
            owner_currentness_id: witness.owner_currentness_id(),
            validation_invocation,
            pass_a_absence_id,
            pass_b_absence_id,
        })
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn source_token(&self) -> [u8; 32] {
        self.source_token
    }

    pub(crate) const fn snapshot_id(&self) -> [u8; 32] {
        self.snapshot_id
    }

    pub(crate) const fn issuer_id(&self) -> [u8; 32] {
        self.issuer_id
    }

    pub(crate) const fn historical_tuple_id(&self) -> [u8; 32] {
        self.historical_tuple_id
    }

    pub(crate) const fn current_tuple_id(&self) -> [u8; 32] {
        self.current_tuple_id
    }

    pub(crate) const fn source_provenance_id(&self) -> [u8; 32] {
        self.source_provenance_id
    }

    pub(crate) const fn owner_admission_id(&self) -> [u8; 32] {
        self.owner_admission_id
    }

    pub(crate) const fn owner_currentness_id(&self) -> [u8; 32] {
        self.owner_currentness_id
    }

    pub(crate) const fn validation_invocation(&self) -> [u8; 32] {
        self.validation_invocation
    }

    pub(crate) const fn pass_a_absence_id(&self) -> [u8; 32] {
        self.pass_a_absence_id
    }

    pub(crate) const fn pass_b_absence_id(&self) -> [u8; 32] {
        self.pass_b_absence_id
    }
}

fn commitment(namespace: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[derive(Debug, Error)]
pub(crate) enum FoundationLegacyLossEvidenceErrorV1 {
    #[error("retained owner history binding is incomplete, zero, or foreign")]
    InvalidHistoricalBinding,
    #[error("fresh owner currentness binding is incomplete or zero")]
    InvalidCurrentBinding,
    #[error("historical and current owner bindings disagree")]
    OwnerBindingMismatch,
    #[error("owner-issued loss evidence set does not bind one coherent invocation")]
    InvalidEvidenceSet,
    #[error("owner-issued loss evidence contains a duplicate source witness")]
    DuplicateWitness,
    #[error("Foundation loss receipt is incomplete")]
    InvalidFoundationReceipt,
    #[error("Foundation owner-evidence issuance binding is incomplete")]
    InvalidIssuanceBinding,
}
