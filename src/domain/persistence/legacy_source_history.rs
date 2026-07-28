#![allow(
    dead_code,
    reason = "V8 owner history leaf awaits the bounded MainIntegration export checkpoint"
)]

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    AtomicGenerationPublicationV1, PreparedPublicationError, StoreGenerationV1,
    StoreIdempotencyProbeV1, StoreIdempotencyV1, StoreObjectV1, StoreRoleV1, StoreStateV1, StoreV1,
};
use crate::domain::identity::SchemaIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborValue};
use crate::foundation::core::legacy_loss_evidence::{
    FoundationLegacyLossEvidenceErrorV1, FoundationOwnerEvidenceIssuanceBindingV1,
    LegacySourceCurrentBindingV1, LegacySourceHistoricalBindingV1, LegacySourceHistoryKindV1,
    OwnerIssuedUnavailablePreexistingLossEvidenceSetV1,
    OwnerUnavailablePreexistingLossEvidenceIssuerPortV1, OwnerUnavailablePreexistingLossWitnessV1,
    owner_loss_evidence_issuer_sealed,
};
use crate::foundation::core::legacy_quarantine::LegacyQuarantineOwnerDomainV3;
use crate::foundation::core::secure_fs::DescriptorCensusObjectKindV1;

const HISTORY_SCHEMA_ID: &str =
    "sha256:18ec165629f125287355fa6b8405035ddc39f71661bb80799b781a73527ccfc1";
const HISTORY_VERSION_V1: u64 = 1;
const HISTORY_PUBLICATION_NAMESPACE: &str = "maestro.v8.legacy-source-history.v1";
const MAX_HISTORY_SNAPSHOTS_V1: usize = 1_048_576;

#[derive(Clone, Copy)]
pub(in crate::domain) struct StoreLegacySourceHistoryContextV1 {
    pub(in crate::domain) namespace_epoch: u64,
    pub(in crate::domain) trust_root_id: [u8; 32],
    pub(in crate::domain) release_id: [u8; 32],
    pub(in crate::domain) provider_revision: u64,
    pub(in crate::domain) source_provenance_id: [u8; 32],
}

#[derive(Clone, Copy)]
pub(in crate::domain) struct StoreLegacySourceCurrentnessV1 {
    expected_source_set_id: [u8; 32],
    operation_attempt: [u8; 32],
    owner_admission_id: [u8; 32],
    owner_currentness_id: [u8; 32],
    namespace_epoch: u64,
    trust_root_id: [u8; 32],
    release_id: [u8; 32],
    provider_id: [u8; 32],
    mount_id: [u8; 32],
    anchor_id: [u8; 32],
    fence_id: [u8; 32],
    revocation_revision: u64,
}

#[derive(Clone, Copy)]
pub(in crate::domain) struct StoreLegacySourceCurrentViewV1 {
    pub(in crate::domain) namespace_epoch: u64,
    pub(in crate::domain) trust_root_id: [u8; 32],
    pub(in crate::domain) release_id: [u8; 32],
    pub(in crate::domain) provider_id: [u8; 32],
    pub(in crate::domain) mount_id: [u8; 32],
    pub(in crate::domain) anchor_id: [u8; 32],
    pub(in crate::domain) fence_id: [u8; 32],
    pub(in crate::domain) revocation_revision: u64,
}

impl StoreLegacySourceCurrentViewV1 {
    pub(in crate::domain) fn bind_foundation_issuance(
        self,
        binding: &FoundationOwnerEvidenceIssuanceBindingV1,
        expected_owner: LegacyQuarantineOwnerDomainV3,
    ) -> Result<StoreLegacySourceCurrentnessV1, FoundationLegacyLossEvidenceErrorV1> {
        if binding.owner() != expected_owner
            || self.namespace_epoch == 0
            || self.revocation_revision == 0
            || [
                self.trust_root_id,
                self.release_id,
                self.provider_id,
                self.mount_id,
                self.anchor_id,
                self.fence_id,
            ]
            .contains(&[0; 32])
        {
            return Err(FoundationLegacyLossEvidenceErrorV1::InvalidIssuanceBinding);
        }
        Ok(StoreLegacySourceCurrentnessV1 {
            expected_source_set_id: binding.expected_source_set_id(),
            operation_attempt: binding.operation_attempt(),
            owner_admission_id: binding.owner_admission_id(),
            owner_currentness_id: binding.owner_currentness_id(),
            namespace_epoch: self.namespace_epoch,
            trust_root_id: self.trust_root_id,
            release_id: self.release_id,
            provider_id: self.provider_id,
            mount_id: self.mount_id,
            anchor_id: self.anchor_id,
            fence_id: self.fence_id,
            revocation_revision: self.revocation_revision,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::domain) struct LegacySourceHistorySelectorV1 {
    pub(in crate::domain) root_binding: [u8; 32],
    pub(in crate::domain) relative_locator_commitment: [u8; 32],
    pub(in crate::domain) source_provenance_id: [u8; 32],
    pub(in crate::domain) object_kind: DescriptorCensusObjectKindV1,
    pub(in crate::domain) object_identity: [u8; 32],
    pub(in crate::domain) expected_length: u64,
    pub(in crate::domain) content_sha256: [u8; 32],
    pub(in crate::domain) metadata_commitment: [u8; 32],
}

impl LegacySourceHistorySelectorV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the selector preserves the complete expected historical source tuple"
    )]
    pub(crate) fn from_expected_source(
        root_binding: [u8; 32],
        relative_locator_commitment: [u8; 32],
        source_provenance_id: [u8; 32],
        object_kind: DescriptorCensusObjectKindV1,
        object_identity: [u8; 32],
        expected_length: u64,
        content_sha256: [u8; 32],
        metadata_commitment: [u8; 32],
    ) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        if [
            root_binding,
            relative_locator_commitment,
            source_provenance_id,
            object_identity,
            content_sha256,
            metadata_commitment,
        ]
        .contains(&[0; 32])
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
        }
        Ok(Self {
            root_binding,
            relative_locator_commitment,
            source_provenance_id,
            object_kind,
            object_identity,
            expected_length,
            content_sha256,
            metadata_commitment,
        })
    }

    fn identity(&self) -> [u8; 32] {
        commitment(
            b"maestro.v8.legacy-source-history-selector.v1\0",
            &[
                &self.root_binding,
                &self.relative_locator_commitment,
                &self.source_provenance_id,
                &object_kind_tag(self.object_kind).to_be_bytes(),
                &self.object_identity,
                &self.expected_length.to_be_bytes(),
                &self.content_sha256,
                &self.metadata_commitment,
            ],
        )
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(in crate::domain) struct StoreLegacySourceHistorySnapshotV1 {
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
}

impl StoreLegacySourceHistorySnapshotV1 {
    pub(in crate::domain) fn capture_present_file(
        store: &StoreV1,
        owner: LegacyQuarantineOwnerDomainV3,
        history_kind: LegacySourceHistoryKindV1,
        relative_locator: &[u8],
        context: StoreLegacySourceHistoryContextV1,
    ) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        require_owner_store_role(owner, history_kind, store.role())?;
        validate_context(context)?;
        let (state, state_revision) = store.state()?;
        if state != StoreStateV1::Active || state_revision == 0 {
            return Err(StoreLegacySourceHistoryErrorV1::StoreNotCurrent);
        }
        let head = store
            .active_head()?
            .ok_or(StoreLegacySourceHistoryErrorV1::StoreNotCurrent)?;
        let generation = store.publication_generation(head.id())?;
        if generation.id() != head.generation_id()
            || generation.ordinal() != head.revision()
            || generation.domain() != store.domain()
        {
            return Err(StoreLegacySourceHistoryErrorV1::StoreNotCurrent);
        }
        let root = store.legacy_quarantine_root_path_v3();
        let relative_path = validated_relative_path(relative_locator)?;
        let source_path = root.join(&relative_path);
        let metadata = std::fs::symlink_metadata(&source_path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(StoreLegacySourceHistoryErrorV1::SourceNotPresent);
        }
        let bytes = std::fs::read(&source_path)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(StoreLegacySourceHistoryErrorV1::SourceChanged);
        }
        let content_sha256: [u8; 32] = Sha256::digest(&bytes).into();
        let relative_locator_commitment = commitment(
            b"maestro.v8.legacy-source.relative-locator.v1\0",
            &[relative_locator],
        );
        let root_binding =
            crate::foundation::core::legacy_quarantine::observe_root_binding_v3(root)?;
        let metadata_commitment = metadata_commitment(&metadata);
        let object_identity = commitment(
            b"maestro.v8.legacy-source.object.v1\0",
            &[
                &root_binding,
                &relative_locator_commitment,
                &metadata.len().to_be_bytes(),
                &content_sha256,
                &metadata_commitment,
            ],
        );
        let issuer_id = commitment(
            b"maestro.v8.legacy-source.issuer.v1\0",
            &[
                store.domain().id().as_bytes(),
                &context.provider_revision.to_be_bytes(),
                &context.trust_root_id,
                &context.release_id,
            ],
        );
        let mut snapshot = Self {
            owner,
            history_kind,
            snapshot_id: [0; 32],
            issuer_id,
            history_instance_id: *store.domain().id().as_bytes(),
            historical_head_id: *head.id().as_bytes(),
            historical_generation_id: *generation.id().as_bytes(),
            historical_state_revision: state_revision,
            namespace_epoch: context.namespace_epoch,
            trust_root_id: context.trust_root_id,
            release_id: context.release_id,
            provider_revision: context.provider_revision,
            source_provenance_id: context.source_provenance_id,
            root_binding,
            relative_locator_commitment,
            object_kind: DescriptorCensusObjectKindV1::RegularFile,
            object_identity,
            expected_length: metadata.len(),
            content_sha256,
            metadata_commitment,
        };
        snapshot.snapshot_id = snapshot.identity_without_snapshot_id();
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub(in crate::domain) fn persist(
        &self,
        store: &mut StoreV1,
    ) -> Result<(), StoreLegacySourceHistoryErrorV1> {
        require_owner_store_role(self.owner, self.history_kind, store.role())?;
        self.validate()?;
        let snapshot_object = self.to_store_object()?;
        let key_digest = self.snapshot_id;
        let meaning_digest: [u8; 32] = Sha256::digest(snapshot_object.canonical_bytes()).into();
        let probe = StoreIdempotencyProbeV1::new(
            HISTORY_PUBLICATION_NAMESPACE,
            key_digest,
            meaning_digest,
        )?;
        let snapshot_object_for_prepare = snapshot_object.clone();
        let owner = self.owner;
        let historical_head_id = self.historical_head_id;
        let historical_generation_id = self.historical_generation_id;
        store
            .publish_generation_atomically_with_prepare(&probe, move |view| {
                let head = view
                    .active_head()?
                    .ok_or(StoreLegacySourceHistoryErrorV1::StoreNotCurrent)?;
                let generation = view
                    .active_generation()?
                    .ok_or(StoreLegacySourceHistoryErrorV1::StoreNotCurrent)?;
                let mut objects = view.active_generation_objects()?;
                if view.role() != store_role_for_owner(owner) {
                    return Err(StoreLegacySourceHistoryErrorV1::WrongOwner);
                }
                if *head.id().as_bytes() != historical_head_id
                    || *generation.id().as_bytes() != historical_generation_id
                {
                    return Err(StoreLegacySourceHistoryErrorV1::SourceChanged);
                }
                let mut roots = generation.roots().to_vec();
                roots.push(snapshot_object_for_prepare.id());
                roots.sort();
                roots.dedup();
                objects.push(snapshot_object_for_prepare.clone());
                let next = StoreGenerationV1::new(
                    generation.domain().clone(),
                    generation
                        .ordinal()
                        .checked_add(1)
                        .ok_or(StoreLegacySourceHistoryErrorV1::StoreNotCurrent)?,
                    Some(generation.id()),
                    generation.contract_root_id(),
                    generation.compatibility().clone(),
                    roots,
                )?;
                let idempotency = StoreIdempotencyV1::new(
                    HISTORY_PUBLICATION_NAMESPACE,
                    key_digest,
                    meaning_digest,
                    snapshot_object_for_prepare.id(),
                )?;
                Ok(AtomicGenerationPublicationV1::new(
                    next,
                    Some(head.id()),
                    objects,
                    idempotency,
                )?)
            })
            .map_err(|error| match error {
                PreparedPublicationError::Store(error) => {
                    StoreLegacySourceHistoryErrorV1::Store(error)
                }
                PreparedPublicationError::Prepare(error) => error,
            })?;
        Ok(())
    }

    fn to_store_object(&self) -> Result<StoreObjectV1, StoreLegacySourceHistoryErrorV1> {
        self.validate()?;
        Ok(StoreObjectV1::new(
            history_schema_id()?,
            self.canonical_value(),
            Vec::new(),
        )?)
    }

    fn from_store_object(
        object: &StoreObjectV1,
    ) -> Result<Option<Self>, StoreLegacySourceHistoryErrorV1> {
        if object.schema_id() != history_schema_id()? {
            return Ok(None);
        }
        let CborValue::Array(fields) = object.value() else {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
        };
        if fields.len() != 21 {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
        }
        let snapshot = Self {
            owner: owner_from_tag(unsigned(&fields[1])?)?,
            history_kind: history_kind_from_tag(unsigned(&fields[2])?)?,
            snapshot_id: digest(&fields[3])?,
            issuer_id: digest(&fields[4])?,
            history_instance_id: digest(&fields[5])?,
            historical_head_id: digest(&fields[6])?,
            historical_generation_id: digest(&fields[7])?,
            historical_state_revision: unsigned(&fields[8])?,
            namespace_epoch: unsigned(&fields[9])?,
            trust_root_id: digest(&fields[10])?,
            release_id: digest(&fields[11])?,
            provider_revision: unsigned(&fields[12])?,
            source_provenance_id: digest(&fields[13])?,
            root_binding: digest(&fields[14])?,
            relative_locator_commitment: digest(&fields[15])?,
            object_kind: object_kind_from_tag(unsigned(&fields[16])?)?,
            object_identity: digest(&fields[17])?,
            expected_length: unsigned(&fields[18])?,
            content_sha256: digest(&fields[19])?,
            metadata_commitment: digest(&fields[20])?,
        };
        if unsigned(&fields[0])? != HISTORY_VERSION_V1
            || snapshot.to_store_object()?.canonical_bytes() != object.canonical_bytes()
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
        }
        snapshot.validate()?;
        Ok(Some(snapshot))
    }

    fn historical_binding(
        &self,
    ) -> Result<LegacySourceHistoricalBindingV1, StoreLegacySourceHistoryErrorV1> {
        Ok(LegacySourceHistoricalBindingV1::from_owner(
            self.owner,
            self.history_kind,
            self.snapshot_id,
            self.issuer_id,
            self.history_instance_id,
            self.historical_head_id,
            self.historical_generation_id,
            self.historical_state_revision,
            self.namespace_epoch,
            self.trust_root_id,
            self.release_id,
            self.provider_revision,
            self.source_provenance_id,
            self.root_binding,
            self.relative_locator_commitment,
            self.object_kind,
            self.object_identity,
            self.expected_length,
            self.content_sha256,
            self.metadata_commitment,
        )?)
    }

    fn validate(&self) -> Result<(), StoreLegacySourceHistoryErrorV1> {
        require_owner_store_role(
            self.owner,
            self.history_kind,
            store_role_for_owner(self.owner),
        )?;
        if self.snapshot_id == [0; 32]
            || self.snapshot_id != self.identity_without_snapshot_id()
            || [
                self.issuer_id,
                self.history_instance_id,
                self.historical_head_id,
                self.historical_generation_id,
                self.trust_root_id,
                self.release_id,
                self.source_provenance_id,
                self.root_binding,
                self.relative_locator_commitment,
                self.object_identity,
                self.content_sha256,
                self.metadata_commitment,
            ]
            .contains(&[0; 32])
            || self.historical_state_revision == 0
            || self.namespace_epoch == 0
            || self.provider_revision == 0
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
        }
        Ok(())
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(HISTORY_VERSION_V1),
            CborValue::Unsigned(owner_tag(self.owner)),
            CborValue::Unsigned(history_kind_tag(self.history_kind)),
            bytes(self.snapshot_id),
            bytes(self.issuer_id),
            bytes(self.history_instance_id),
            bytes(self.historical_head_id),
            bytes(self.historical_generation_id),
            CborValue::Unsigned(self.historical_state_revision),
            CborValue::Unsigned(self.namespace_epoch),
            bytes(self.trust_root_id),
            bytes(self.release_id),
            CborValue::Unsigned(self.provider_revision),
            bytes(self.source_provenance_id),
            bytes(self.root_binding),
            bytes(self.relative_locator_commitment),
            CborValue::Unsigned(object_kind_tag(self.object_kind)),
            bytes(self.object_identity),
            CborValue::Unsigned(self.expected_length),
            bytes(self.content_sha256),
            bytes(self.metadata_commitment),
        ])
    }

    fn identity_without_snapshot_id(&self) -> [u8; 32] {
        commitment(
            b"maestro.v8.legacy-source-history-snapshot.v1\0",
            &[
                &owner_tag(self.owner).to_be_bytes(),
                &history_kind_tag(self.history_kind).to_be_bytes(),
                &self.issuer_id,
                &self.history_instance_id,
                &self.historical_head_id,
                &self.historical_generation_id,
                &self.historical_state_revision.to_be_bytes(),
                &self.namespace_epoch.to_be_bytes(),
                &self.trust_root_id,
                &self.release_id,
                &self.provider_revision.to_be_bytes(),
                &self.source_provenance_id,
                &self.root_binding,
                &self.relative_locator_commitment,
                &object_kind_tag(self.object_kind).to_be_bytes(),
                &self.object_identity,
                &self.expected_length.to_be_bytes(),
                &self.content_sha256,
                &self.metadata_commitment,
            ],
        )
    }
}

pub(in crate::domain) struct StoreLegacySourceHistoryProviderV1 {
    owner: LegacyQuarantineOwnerDomainV3,
    snapshots: Vec<StoreLegacySourceHistorySnapshotV1>,
    _not_send_or_sync: std::marker::PhantomData<std::rc::Rc<()>>,
}

pub(super) struct ProtectedPrimaryHistoryBoundaryBindingV1 {
    root: PathBuf,
    boundary_lease_id: [u8; 32],
    root_binding: [u8; 32],
    object_identity: [u8; 32],
    provider_id: [u8; 32],
    mount_id: [u8; 32],
    anchor_id: [u8; 32],
    realm_id: [u8; 32],
    currentness: [u8; 32],
    fence_id: [u8; 32],
    revocation_revision: u64,
}

impl ProtectedPrimaryHistoryBoundaryBindingV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "protected-primary history binds the complete live boundary tuple"
    )]
    pub(super) fn from_boundary(
        root: PathBuf,
        boundary_lease_id: [u8; 32],
        root_binding: [u8; 32],
        object_identity: [u8; 32],
        provider_id: [u8; 32],
        mount_id: [u8; 32],
        anchor_id: [u8; 32],
        realm_id: [u8; 32],
        currentness: [u8; 32],
        fence_id: [u8; 32],
        revocation_revision: u64,
    ) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        if !root.is_absolute()
            || revocation_revision == 0
            || [
                boundary_lease_id,
                root_binding,
                object_identity,
                provider_id,
                mount_id,
                anchor_id,
                realm_id,
                currentness,
                fence_id,
            ]
            .contains(&[0; 32])
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidCurrentness);
        }
        Ok(Self {
            root,
            boundary_lease_id,
            root_binding,
            object_identity,
            provider_id,
            mount_id,
            anchor_id,
            realm_id,
            currentness,
            fence_id,
            revocation_revision,
        })
    }

    fn validate_live(&self) -> Result<(), StoreLegacySourceHistoryErrorV1> {
        let observed =
            crate::foundation::core::legacy_quarantine::observe_physical_facts_v1(&self.root)?;
        if observed.resolved_locator_commitment() != self.root_binding
            || observed.object_identity() != self.object_identity
            || observed.provider_identity() != self.provider_id
            || observed.mount_identity() != self.mount_id
            || observed.anchor_identity() != self.anchor_id
        {
            return Err(StoreLegacySourceHistoryErrorV1::SourceChanged);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct ProtectedPrimaryHistoryContextV1 {
    pub(super) namespace_epoch: u64,
    pub(super) trust_root_id: [u8; 32],
    pub(super) release_id: [u8; 32],
    pub(super) backend_implementation_id: [u8; 32],
    pub(super) backend_revision: u64,
    pub(super) source_provenance_id: [u8; 32],
}

pub(crate) struct ProtectedPrimaryHistoryJournalV1 {
    journal_root: PathBuf,
    journal_instance_id: [u8; 32],
    context: ProtectedPrimaryHistoryContextV1,
    _not_send_or_sync: std::marker::PhantomData<std::rc::Rc<()>>,
}

pub(crate) struct ProtectedPrimaryUnavailablePreexistingLossEvidenceIssuerV1 {
    journal: ProtectedPrimaryHistoryJournalV1,
    boundary: ProtectedPrimaryHistoryBoundaryBindingV1,
    current_view: StoreLegacySourceCurrentViewV1,
    absent_sources: Vec<LegacySourceHistorySelectorV1>,
    _not_send_or_sync: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl ProtectedPrimaryUnavailablePreexistingLossEvidenceIssuerV1 {
    pub(super) fn prepare(
        journal: ProtectedPrimaryHistoryJournalV1,
        boundary: ProtectedPrimaryHistoryBoundaryBindingV1,
        current_view: StoreLegacySourceCurrentViewV1,
        absent_sources: Vec<LegacySourceHistorySelectorV1>,
    ) -> Self {
        Self {
            journal,
            boundary,
            current_view,
            absent_sources,
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl owner_loss_evidence_issuer_sealed::Sealed
    for ProtectedPrimaryUnavailablePreexistingLossEvidenceIssuerV1
{
}

impl OwnerUnavailablePreexistingLossEvidenceIssuerPortV1
    for ProtectedPrimaryUnavailablePreexistingLossEvidenceIssuerV1
{
    fn issue_for_foundation(
        self,
        binding: FoundationOwnerEvidenceIssuanceBindingV1,
    ) -> Result<
        OwnerIssuedUnavailablePreexistingLossEvidenceSetV1,
        FoundationLegacyLossEvidenceErrorV1,
    > {
        let currentness = self
            .current_view
            .bind_foundation_issuance(&binding, LegacyQuarantineOwnerDomainV3::ProtectedPrimary)?;
        self.journal
            .issue_bound_absent_sources(self.boundary, currentness, &self.absent_sources)
            .map_err(|_| FoundationLegacyLossEvidenceErrorV1::InvalidEvidenceSet)
    }
}

impl ProtectedPrimaryHistoryJournalV1 {
    pub(super) fn open_or_create(
        journal_root: impl AsRef<Path>,
        journal_instance_id: [u8; 32],
        context: ProtectedPrimaryHistoryContextV1,
    ) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        if journal_instance_id == [0; 32]
            || context.namespace_epoch == 0
            || context.backend_revision == 0
            || [
                context.trust_root_id,
                context.release_id,
                context.backend_implementation_id,
                context.source_provenance_id,
            ]
            .contains(&[0; 32])
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
        }
        let journal_root = journal_root.as_ref().to_path_buf();
        fs::create_dir_all(journal_root.join("records"))?;
        let metadata = fs::symlink_metadata(&journal_root)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
        }
        Ok(Self {
            journal_root,
            journal_instance_id,
            context,
            _not_send_or_sync: std::marker::PhantomData,
        })
    }

    pub(super) fn append_present_file(
        &mut self,
        boundary: &ProtectedPrimaryHistoryBoundaryBindingV1,
        relative_locator: &[u8],
    ) -> Result<[u8; 32], StoreLegacySourceHistoryErrorV1> {
        boundary.validate_live()?;
        let _lock = JournalLockV1::acquire(&self.journal_root)?;
        let (prior_revision, prior_head) = read_journal_head(&self.journal_root)?;
        let revision = prior_revision
            .checked_add(1)
            .ok_or(StoreLegacySourceHistoryErrorV1::InvalidJournal)?;
        let relative_path = validated_relative_path(relative_locator)?;
        let source_path = boundary.root.join(relative_path);
        let metadata = fs::symlink_metadata(&source_path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(StoreLegacySourceHistoryErrorV1::SourceNotPresent);
        }
        let payload = fs::read(&source_path)?;
        if payload.len() as u64 != metadata.len() {
            return Err(StoreLegacySourceHistoryErrorV1::SourceChanged);
        }
        let relative_locator_commitment = commitment(
            b"maestro.v8.protected-primary.relative-locator.v1\0",
            &[relative_locator],
        );
        let content_sha256: [u8; 32] = Sha256::digest(&payload).into();
        let metadata_commitment = metadata_commitment(&metadata);
        let source_object_identity = commitment(
            b"maestro.v8.protected-primary.source-object.v1\0",
            &[
                &boundary.root_binding,
                &relative_locator_commitment,
                &metadata.len().to_be_bytes(),
                &content_sha256,
                &metadata_commitment,
            ],
        );
        let historical_generation_id = prior_head.unwrap_or(self.journal_instance_id);
        let issuer_id = commitment(
            b"maestro.v8.protected-primary.history-issuer.v1\0",
            &[
                &self.context.backend_implementation_id,
                &self.context.backend_revision.to_be_bytes(),
                &boundary.provider_id,
                &boundary.realm_id,
            ],
        );
        let mut record = ProtectedPrimaryHistoryRecordV1 {
            snapshot_id: [0; 32],
            issuer_id,
            journal_instance_id: self.journal_instance_id,
            historical_head_id: [0; 32],
            historical_generation_id,
            journal_revision: revision,
            namespace_epoch: self.context.namespace_epoch,
            trust_root_id: self.context.trust_root_id,
            release_id: self.context.release_id,
            backend_revision: self.context.backend_revision,
            source_provenance_id: self.context.source_provenance_id,
            root_binding: boundary.root_binding,
            relative_locator_commitment,
            object_identity: source_object_identity,
            expected_length: metadata.len(),
            content_sha256,
            metadata_commitment,
            boundary_lease_id: boundary.boundary_lease_id,
            boundary_object_identity: boundary.object_identity,
            provider_id: boundary.provider_id,
            mount_id: boundary.mount_id,
            anchor_id: boundary.anchor_id,
            realm_id: boundary.realm_id,
            boundary_currentness: boundary.currentness,
            fence_id: boundary.fence_id,
            revocation_revision: boundary.revocation_revision,
        };
        record.snapshot_id = record.identity();
        record.historical_head_id = record.snapshot_id;
        let bytes = record.canonical_bytes()?;
        let record_path = self
            .journal_root
            .join("records")
            .join(hex_digest(record.snapshot_id));
        write_create_new_synced(&record_path, &bytes)?;
        write_journal_head(
            &self.journal_root,
            prior_revision,
            prior_head,
            revision,
            record.snapshot_id,
        )?;
        Ok(record.snapshot_id)
    }

    fn issue_bound_absent_sources(
        self,
        boundary: ProtectedPrimaryHistoryBoundaryBindingV1,
        currentness: StoreLegacySourceCurrentnessV1,
        absent_sources: &[LegacySourceHistorySelectorV1],
    ) -> Result<OwnerIssuedUnavailablePreexistingLossEvidenceSetV1, StoreLegacySourceHistoryErrorV1>
    {
        validate_currentness(currentness)?;
        boundary.validate_live()?;
        if currentness.provider_id != boundary.provider_id
            || currentness.mount_id != boundary.mount_id
            || currentness.anchor_id != boundary.anchor_id
            || currentness.fence_id != boundary.fence_id
            || currentness.owner_currentness_id != boundary.currentness
            || currentness.revocation_revision != boundary.revocation_revision
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidCurrentness);
        }
        let (journal_revision, journal_head) = read_journal_head(&self.journal_root)?;
        let journal_head = journal_head.ok_or(StoreLegacySourceHistoryErrorV1::MissingHistory)?;
        let mut expected = absent_sources.to_vec();
        expected.sort_by_key(LegacySourceHistorySelectorV1::identity);
        if expected
            .windows(2)
            .any(|pair| pair[0].identity() == pair[1].identity())
        {
            return Err(StoreLegacySourceHistoryErrorV1::AmbiguousHistory);
        }
        let records = self.load_current_records(journal_head)?;
        let mut witnesses = Vec::with_capacity(expected.len());
        for selector in expected {
            let matches = records
                .iter()
                .filter(|record| {
                    record.root_binding == selector.root_binding
                        && record.relative_locator_commitment
                            == selector.relative_locator_commitment
                        && record.source_provenance_id == selector.source_provenance_id
                        && selector.object_kind == DescriptorCensusObjectKindV1::RegularFile
                        && record.object_identity == selector.object_identity
                        && record.expected_length == selector.expected_length
                        && record.content_sha256 == selector.content_sha256
                        && record.metadata_commitment == selector.metadata_commitment
                        && record.namespace_epoch == currentness.namespace_epoch
                        && record.trust_root_id == currentness.trust_root_id
                        && record.release_id == currentness.release_id
                })
                .collect::<Vec<_>>();
            let [record] = matches.as_slice() else {
                return Err(if matches.is_empty() {
                    StoreLegacySourceHistoryErrorV1::MissingHistory
                } else {
                    StoreLegacySourceHistoryErrorV1::AmbiguousHistory
                });
            };
            if record.journal_revision > journal_revision
                || record.provider_id != boundary.provider_id
                || record.mount_id != boundary.mount_id
                || record.anchor_id != boundary.anchor_id
                || record.realm_id != boundary.realm_id
            {
                return Err(StoreLegacySourceHistoryErrorV1::InvalidCurrentness);
            }
            let historical = record.historical_binding()?;
            let current = LegacySourceCurrentBindingV1::from_owner(
                LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
                currentness.expected_source_set_id,
                currentness.operation_attempt,
                currentness.owner_admission_id,
                currentness.owner_currentness_id,
                journal_head,
                self.journal_instance_id,
                journal_revision,
                currentness.namespace_epoch,
                currentness.trust_root_id,
                currentness.release_id,
                currentness.provider_id,
                currentness.mount_id,
                currentness.anchor_id,
                currentness.fence_id,
                currentness.revocation_revision,
            )?;
            witnesses.push(OwnerUnavailablePreexistingLossWitnessV1::from_owner(
                historical, current,
            )?);
        }
        Ok(
            OwnerIssuedUnavailablePreexistingLossEvidenceSetV1::from_owner(
                LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
                currentness.expected_source_set_id,
                currentness.operation_attempt,
                currentness.owner_admission_id,
                currentness.owner_currentness_id,
                witnesses,
            )?,
        )
    }

    fn load_current_records(
        &self,
        journal_head: [u8; 32],
    ) -> Result<Vec<ProtectedPrimaryHistoryRecordV1>, StoreLegacySourceHistoryErrorV1> {
        let mut by_identity = BTreeMap::new();
        for entry in fs::read_dir(self.journal_root.join("records"))? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.file_type().is_file() {
                return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
            }
            let record = ProtectedPrimaryHistoryRecordV1::decode(&fs::read(entry.path())?)?;
            if by_identity.insert(record.snapshot_id, record).is_some() {
                return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
            }
        }
        let mut records = Vec::new();
        let mut cursor = journal_head;
        loop {
            let record = by_identity
                .remove(&cursor)
                .ok_or(StoreLegacySourceHistoryErrorV1::InvalidJournal)?;
            let previous = record.historical_generation_id;
            records.push(record);
            if previous == self.journal_instance_id {
                break;
            }
            cursor = previous;
        }
        records.reverse();
        for (index, record) in records.iter().enumerate() {
            if record.journal_revision != index as u64 + 1 {
                return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
            }
        }
        Ok(records)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct ProtectedPrimaryHistoryRecordV1 {
    snapshot_id: [u8; 32],
    issuer_id: [u8; 32],
    journal_instance_id: [u8; 32],
    historical_head_id: [u8; 32],
    historical_generation_id: [u8; 32],
    journal_revision: u64,
    namespace_epoch: u64,
    trust_root_id: [u8; 32],
    release_id: [u8; 32],
    backend_revision: u64,
    source_provenance_id: [u8; 32],
    root_binding: [u8; 32],
    relative_locator_commitment: [u8; 32],
    object_identity: [u8; 32],
    expected_length: u64,
    content_sha256: [u8; 32],
    metadata_commitment: [u8; 32],
    boundary_lease_id: [u8; 32],
    boundary_object_identity: [u8; 32],
    provider_id: [u8; 32],
    mount_id: [u8; 32],
    anchor_id: [u8; 32],
    realm_id: [u8; 32],
    boundary_currentness: [u8; 32],
    fence_id: [u8; 32],
    revocation_revision: u64,
}

impl ProtectedPrimaryHistoryRecordV1 {
    fn historical_binding(
        &self,
    ) -> Result<LegacySourceHistoricalBindingV1, StoreLegacySourceHistoryErrorV1> {
        Ok(LegacySourceHistoricalBindingV1::from_owner(
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
            LegacySourceHistoryKindV1::ProtectedPrimaryJournal,
            self.snapshot_id,
            self.issuer_id,
            self.journal_instance_id,
            self.historical_head_id,
            self.historical_generation_id,
            self.journal_revision,
            self.namespace_epoch,
            self.trust_root_id,
            self.release_id,
            self.backend_revision,
            self.source_provenance_id,
            self.root_binding,
            self.relative_locator_commitment,
            DescriptorCensusObjectKindV1::RegularFile,
            self.object_identity,
            self.expected_length,
            self.content_sha256,
            self.metadata_commitment,
        )?)
    }

    fn identity(&self) -> [u8; 32] {
        let bytes = self.canonical_value(false);
        Sha256::digest(
            deterministic_cbor::encode(&bytes)
                .expect("invariant: fixed protected-primary history values encode canonically"),
        )
        .into()
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, StoreLegacySourceHistoryErrorV1> {
        Ok(deterministic_cbor::encode(&self.canonical_value(true))?)
    }

    fn canonical_value(&self, include_snapshot_id: bool) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            bytes(if include_snapshot_id {
                self.snapshot_id
            } else {
                [0; 32]
            }),
            bytes(self.issuer_id),
            bytes(self.journal_instance_id),
            bytes(if include_snapshot_id {
                self.historical_head_id
            } else {
                [0; 32]
            }),
            bytes(self.historical_generation_id),
            CborValue::Unsigned(self.journal_revision),
            CborValue::Unsigned(self.namespace_epoch),
            bytes(self.trust_root_id),
            bytes(self.release_id),
            CborValue::Unsigned(self.backend_revision),
            bytes(self.source_provenance_id),
            bytes(self.root_binding),
            bytes(self.relative_locator_commitment),
            bytes(self.object_identity),
            CborValue::Unsigned(self.expected_length),
            bytes(self.content_sha256),
            bytes(self.metadata_commitment),
            bytes(self.boundary_lease_id),
            bytes(self.boundary_object_identity),
            bytes(self.provider_id),
            bytes(self.mount_id),
            bytes(self.anchor_id),
            bytes(self.realm_id),
            bytes(self.boundary_currentness),
            bytes(self.fence_id),
            CborValue::Unsigned(self.revocation_revision),
        ])
    }

    fn decode(bytes: &[u8]) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        let CborValue::Array(fields) = deterministic_cbor::decode(bytes)? else {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
        };
        if fields.len() != 27 || unsigned(&fields[0])? != 1 {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
        }
        let record = Self {
            snapshot_id: digest(&fields[1])?,
            issuer_id: digest(&fields[2])?,
            journal_instance_id: digest(&fields[3])?,
            historical_head_id: digest(&fields[4])?,
            historical_generation_id: digest(&fields[5])?,
            journal_revision: unsigned(&fields[6])?,
            namespace_epoch: unsigned(&fields[7])?,
            trust_root_id: digest(&fields[8])?,
            release_id: digest(&fields[9])?,
            backend_revision: unsigned(&fields[10])?,
            source_provenance_id: digest(&fields[11])?,
            root_binding: digest(&fields[12])?,
            relative_locator_commitment: digest(&fields[13])?,
            object_identity: digest(&fields[14])?,
            expected_length: unsigned(&fields[15])?,
            content_sha256: digest(&fields[16])?,
            metadata_commitment: digest(&fields[17])?,
            boundary_lease_id: digest(&fields[18])?,
            boundary_object_identity: digest(&fields[19])?,
            provider_id: digest(&fields[20])?,
            mount_id: digest(&fields[21])?,
            anchor_id: digest(&fields[22])?,
            realm_id: digest(&fields[23])?,
            boundary_currentness: digest(&fields[24])?,
            fence_id: digest(&fields[25])?,
            revocation_revision: unsigned(&fields[26])?,
        };
        if record.snapshot_id == [0; 32]
            || record.historical_head_id != record.snapshot_id
            || record.identity() != record.snapshot_id
            || record.canonical_bytes()? != bytes
        {
            return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
        }
        Ok(record)
    }
}

struct JournalLockV1 {
    path: PathBuf,
}

impl JournalLockV1 {
    fn acquire(root: &Path) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        let path = root.join("writer.lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| StoreLegacySourceHistoryErrorV1::JournalBusy)?;
        Ok(Self { path })
    }
}

impl Drop for JournalLockV1 {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn read_journal_head(
    root: &Path,
) -> Result<(u64, Option<[u8; 32]>), StoreLegacySourceHistoryErrorV1> {
    let path = root.join("head.v1");
    match fs::read(path) {
        Ok(bytes) => {
            if bytes.len() != 40 {
                return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
            }
            let revision = u64::from_be_bytes(
                bytes[..8]
                    .try_into()
                    .map_err(|_| StoreLegacySourceHistoryErrorV1::InvalidJournal)?,
            );
            let head = bytes[8..]
                .try_into()
                .map_err(|_| StoreLegacySourceHistoryErrorV1::InvalidJournal)?;
            if revision == 0 || head == [0; 32] {
                return Err(StoreLegacySourceHistoryErrorV1::InvalidJournal);
            }
            Ok((revision, Some(head)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((0, None)),
        Err(error) => Err(error.into()),
    }
}

fn write_journal_head(
    root: &Path,
    expected_revision: u64,
    expected_head: Option<[u8; 32]>,
    revision: u64,
    head: [u8; 32],
) -> Result<(), StoreLegacySourceHistoryErrorV1> {
    if read_journal_head(root)? != (expected_revision, expected_head) {
        return Err(StoreLegacySourceHistoryErrorV1::SourceChanged);
    }
    let mut bytes = Vec::with_capacity(40);
    bytes.extend_from_slice(&revision.to_be_bytes());
    bytes.extend_from_slice(&head);
    let temporary = root.join(format!(".head.{}.tmp", hex_digest(head)));
    write_create_new_synced(&temporary, &bytes)?;
    fs::rename(temporary, root.join("head.v1"))?;
    Ok(())
}

fn write_create_new_synced(
    path: &Path,
    bytes: &[u8],
) -> Result<(), StoreLegacySourceHistoryErrorV1> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl StoreLegacySourceHistoryProviderV1 {
    pub(in crate::domain) fn acquire(
        store: &StoreV1,
        owner: LegacyQuarantineOwnerDomainV3,
    ) -> Result<Self, StoreLegacySourceHistoryErrorV1> {
        if store.role() != store_role_for_owner(owner) {
            return Err(StoreLegacySourceHistoryErrorV1::WrongOwner);
        }
        let (state, _) = store.state()?;
        if state != StoreStateV1::Active {
            return Err(StoreLegacySourceHistoryErrorV1::StoreNotCurrent);
        }
        let mut generation = store
            .active_head()?
            .ok_or(StoreLegacySourceHistoryErrorV1::StoreNotCurrent)?
            .generation_id();
        let mut snapshots = Vec::new();
        loop {
            let (_, historical_generation, objects) =
                store.coherent_generation_snapshot(generation)?;
            for object in objects {
                if let Some(snapshot) =
                    StoreLegacySourceHistorySnapshotV1::from_store_object(&object)?
                    && snapshot.owner == owner
                {
                    snapshots.push(snapshot);
                }
            }
            let Some(previous) = historical_generation.previous() else {
                break;
            };
            generation = previous;
        }
        snapshots.sort_by_key(|snapshot| snapshot.snapshot_id);
        snapshots.dedup_by_key(|snapshot| snapshot.snapshot_id);
        if snapshots.len() > MAX_HISTORY_SNAPSHOTS_V1 {
            return Err(StoreLegacySourceHistoryErrorV1::TooManySnapshots);
        }
        Ok(Self {
            owner,
            snapshots,
            _not_send_or_sync: std::marker::PhantomData,
        })
    }

    pub(in crate::domain) fn issue_bound_absent_sources(
        self,
        store: &StoreV1,
        currentness: StoreLegacySourceCurrentnessV1,
        absent_sources: &[LegacySourceHistorySelectorV1],
    ) -> Result<OwnerIssuedUnavailablePreexistingLossEvidenceSetV1, StoreLegacySourceHistoryErrorV1>
    {
        validate_currentness(currentness)?;
        let (state, state_revision) = store.state()?;
        let head = store
            .active_head()?
            .ok_or(StoreLegacySourceHistoryErrorV1::StoreNotCurrent)?;
        let generation = store.publication_generation(head.id())?;
        if state != StoreStateV1::Active
            || state_revision == 0
            || store.role() != store_role_for_owner(self.owner)
        {
            return Err(StoreLegacySourceHistoryErrorV1::StoreNotCurrent);
        }
        let mut expected = absent_sources.to_vec();
        expected.sort_by_key(LegacySourceHistorySelectorV1::identity);
        if expected
            .windows(2)
            .any(|pair| pair[0].identity() == pair[1].identity())
        {
            return Err(StoreLegacySourceHistoryErrorV1::AmbiguousHistory);
        }
        let mut witnesses = Vec::with_capacity(expected.len());
        for selector in expected {
            let mut matches = self.snapshots.iter().filter(|snapshot| {
                snapshot.root_binding == selector.root_binding
                    && snapshot.relative_locator_commitment == selector.relative_locator_commitment
                    && snapshot.source_provenance_id == selector.source_provenance_id
                    && snapshot.object_kind == selector.object_kind
                    && snapshot.object_identity == selector.object_identity
                    && snapshot.expected_length == selector.expected_length
                    && snapshot.content_sha256 == selector.content_sha256
                    && snapshot.metadata_commitment == selector.metadata_commitment
                    && snapshot.namespace_epoch == currentness.namespace_epoch
                    && snapshot.trust_root_id == currentness.trust_root_id
                    && snapshot.release_id == currentness.release_id
            });
            let snapshot = matches
                .next()
                .ok_or(StoreLegacySourceHistoryErrorV1::MissingHistory)?;
            if matches.next().is_some() {
                return Err(StoreLegacySourceHistoryErrorV1::AmbiguousHistory);
            }
            let historical = snapshot.historical_binding()?;
            let current = LegacySourceCurrentBindingV1::from_owner(
                self.owner,
                currentness.expected_source_set_id,
                currentness.operation_attempt,
                currentness.owner_admission_id,
                currentness.owner_currentness_id,
                *head.id().as_bytes(),
                *generation.id().as_bytes(),
                state_revision,
                currentness.namespace_epoch,
                currentness.trust_root_id,
                currentness.release_id,
                currentness.provider_id,
                currentness.mount_id,
                currentness.anchor_id,
                currentness.fence_id,
                currentness.revocation_revision,
            )?;
            witnesses.push(OwnerUnavailablePreexistingLossWitnessV1::from_owner(
                historical, current,
            )?);
        }
        Ok(
            OwnerIssuedUnavailablePreexistingLossEvidenceSetV1::from_owner(
                self.owner,
                currentness.expected_source_set_id,
                currentness.operation_attempt,
                currentness.owner_admission_id,
                currentness.owner_currentness_id,
                witnesses,
            )?,
        )
    }
}

fn validate_context(
    context: StoreLegacySourceHistoryContextV1,
) -> Result<(), StoreLegacySourceHistoryErrorV1> {
    if context.namespace_epoch == 0
        || context.provider_revision == 0
        || [
            context.trust_root_id,
            context.release_id,
            context.source_provenance_id,
        ]
        .contains(&[0; 32])
    {
        return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
    }
    Ok(())
}

fn validate_currentness(
    currentness: StoreLegacySourceCurrentnessV1,
) -> Result<(), StoreLegacySourceHistoryErrorV1> {
    if currentness.namespace_epoch == 0
        || currentness.revocation_revision == 0
        || [
            currentness.expected_source_set_id,
            currentness.operation_attempt,
            currentness.owner_admission_id,
            currentness.owner_currentness_id,
            currentness.trust_root_id,
            currentness.release_id,
            currentness.provider_id,
            currentness.mount_id,
            currentness.anchor_id,
            currentness.fence_id,
        ]
        .contains(&[0; 32])
    {
        return Err(StoreLegacySourceHistoryErrorV1::InvalidCurrentness);
    }
    Ok(())
}

fn require_owner_store_role(
    owner: LegacyQuarantineOwnerDomainV3,
    history_kind: LegacySourceHistoryKindV1,
    role: StoreRoleV1,
) -> Result<(), StoreLegacySourceHistoryErrorV1> {
    let valid = matches!(
        (owner, history_kind, role),
        (
            LegacyQuarantineOwnerDomainV3::Repository,
            LegacySourceHistoryKindV1::RepositoryStore,
            StoreRoleV1::Repository
        ) | (
            LegacyQuarantineOwnerDomainV3::Installation,
            LegacySourceHistoryKindV1::InstallationStore,
            StoreRoleV1::Installation
        )
    );
    if !valid {
        return Err(StoreLegacySourceHistoryErrorV1::WrongOwner);
    }
    Ok(())
}

fn store_role_for_owner(owner: LegacyQuarantineOwnerDomainV3) -> StoreRoleV1 {
    match owner {
        LegacyQuarantineOwnerDomainV3::Repository => StoreRoleV1::Repository,
        LegacyQuarantineOwnerDomainV3::Installation => StoreRoleV1::Installation,
        LegacyQuarantineOwnerDomainV3::ProtectedPrimary => StoreRoleV1::Repository,
    }
}

fn owner_tag(owner: LegacyQuarantineOwnerDomainV3) -> u64 {
    match owner {
        LegacyQuarantineOwnerDomainV3::Repository => 1,
        LegacyQuarantineOwnerDomainV3::Installation => 2,
        LegacyQuarantineOwnerDomainV3::ProtectedPrimary => 3,
    }
}

fn owner_from_tag(
    tag: u64,
) -> Result<LegacyQuarantineOwnerDomainV3, StoreLegacySourceHistoryErrorV1> {
    match tag {
        1 => Ok(LegacyQuarantineOwnerDomainV3::Repository),
        2 => Ok(LegacyQuarantineOwnerDomainV3::Installation),
        3 => Ok(LegacyQuarantineOwnerDomainV3::ProtectedPrimary),
        _ => Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot),
    }
}

fn history_kind_tag(kind: LegacySourceHistoryKindV1) -> u64 {
    match kind {
        LegacySourceHistoryKindV1::RepositoryStore => 1,
        LegacySourceHistoryKindV1::InstallationStore => 2,
        LegacySourceHistoryKindV1::ProtectedPrimaryJournal => 3,
    }
}

fn history_kind_from_tag(
    tag: u64,
) -> Result<LegacySourceHistoryKindV1, StoreLegacySourceHistoryErrorV1> {
    match tag {
        1 => Ok(LegacySourceHistoryKindV1::RepositoryStore),
        2 => Ok(LegacySourceHistoryKindV1::InstallationStore),
        3 => Ok(LegacySourceHistoryKindV1::ProtectedPrimaryJournal),
        _ => Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot),
    }
}

fn object_kind_tag(kind: DescriptorCensusObjectKindV1) -> u64 {
    match kind {
        DescriptorCensusObjectKindV1::RegularFile => 1,
        DescriptorCensusObjectKindV1::SymbolicLink => 2,
    }
}

fn object_kind_from_tag(
    tag: u64,
) -> Result<DescriptorCensusObjectKindV1, StoreLegacySourceHistoryErrorV1> {
    match tag {
        1 => Ok(DescriptorCensusObjectKindV1::RegularFile),
        2 => Ok(DescriptorCensusObjectKindV1::SymbolicLink),
        _ => Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot),
    }
}

fn history_schema_id() -> Result<SchemaIdV1, StoreLegacySourceHistoryErrorV1> {
    Ok(SchemaIdV1::parse(HISTORY_SCHEMA_ID)?)
}

fn bytes(value: [u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn digest(value: &CborValue) -> Result<[u8; 32], StoreLegacySourceHistoryErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| StoreLegacySourceHistoryErrorV1::InvalidSnapshot)
}

fn unsigned(value: &CborValue) -> Result<u64, StoreLegacySourceHistoryErrorV1> {
    let CborValue::Unsigned(value) = value else {
        return Err(StoreLegacySourceHistoryErrorV1::InvalidSnapshot);
    };
    Ok(*value)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validated_relative_path(
    relative_locator: &[u8],
) -> Result<PathBuf, StoreLegacySourceHistoryErrorV1> {
    use std::os::unix::ffi::OsStrExt;

    if relative_locator.is_empty() || relative_locator.contains(&0) {
        return Err(StoreLegacySourceHistoryErrorV1::InvalidRelativeLocator);
    }
    let path = Path::new(std::ffi::OsStr::from_bytes(relative_locator));
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StoreLegacySourceHistoryErrorV1::InvalidRelativeLocator);
    }
    Ok(path.to_path_buf())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn validated_relative_path(
    _relative_locator: &[u8],
) -> Result<PathBuf, StoreLegacySourceHistoryErrorV1> {
    Err(StoreLegacySourceHistoryErrorV1::UnsupportedPlatform)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn metadata_commitment(metadata: &std::fs::Metadata) -> [u8; 32] {
    use std::os::unix::fs::MetadataExt;

    commitment(
        b"maestro.v8.legacy-source.metadata.v1\0",
        &[
            &metadata.dev().to_be_bytes(),
            &metadata.ino().to_be_bytes(),
            &metadata.mode().to_be_bytes(),
            &metadata.uid().to_be_bytes(),
            &metadata.gid().to_be_bytes(),
            &metadata.len().to_be_bytes(),
            &metadata.mtime().to_be_bytes(),
            &metadata.mtime_nsec().to_be_bytes(),
        ],
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn metadata_commitment(_metadata: &std::fs::Metadata) -> [u8; 32] {
    [0; 32]
}

fn commitment(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[derive(Debug, Error)]
pub(in crate::domain) enum StoreLegacySourceHistoryErrorV1 {
    #[error("legacy source history owner and Store role do not match")]
    WrongOwner,
    #[error("legacy source history requires a coherent active Store")]
    StoreNotCurrent,
    #[error("legacy source history source is not a present regular file")]
    SourceNotPresent,
    #[error("legacy source changed while its history snapshot was captured")]
    SourceChanged,
    #[error("legacy source relative locator is invalid")]
    InvalidRelativeLocator,
    #[error("legacy source history is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("legacy source history snapshot is invalid or non-canonical")]
    InvalidSnapshot,
    #[error("legacy source currentness tuple is incomplete")]
    InvalidCurrentness,
    #[error("legacy source history graph exceeds its finite bound")]
    TooManySnapshots,
    #[error("protected-primary history journal is invalid or not head-reachable")]
    InvalidJournal,
    #[error("protected-primary history journal already has a live writer")]
    JournalBusy,
    #[error("no retained pre-loss history matches the absent owner source")]
    MissingHistory,
    #[error("multiple retained history snapshots ambiguously match one absent source")]
    AmbiguousHistory,
    #[error(transparent)]
    Store(#[from] super::StoreError),
    #[error(transparent)]
    AtomicPublication(#[from] super::AtomicPublicationError),
    #[error(transparent)]
    Generation(#[from] super::GenerationError),
    #[error(transparent)]
    StoreObject(#[from] super::StoreObjectError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
    #[error(transparent)]
    Foundation(#[from] FoundationLegacyLossEvidenceErrorV1),
    #[error(transparent)]
    LegacyQuarantine(
        #[from] crate::foundation::core::legacy_quarantine::FoundationLegacyQuarantineErrorV1,
    ),
    #[error(transparent)]
    CanonicalCbor(#[from] crate::foundation::core::deterministic_cbor::CborError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
