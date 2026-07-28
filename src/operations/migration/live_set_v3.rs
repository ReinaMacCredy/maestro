use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::installation::InstallationCensusV1;
use crate::domain::migration::runtime::{
    DeclaredOverlapManifestV2, FoundationMaterializedSourceCaseV3, LegacyNodeKindV3,
    LegacyOwnerDomainV3, LegacyPayloadStateV3, LegacyRollbackAssessmentV3,
    LegacyRollbackAssessmentV4, LegacySourceCaseManifestV3, LiveSetV3Error, MembershipKeyV3,
    MigrationClassificationManifestV3, MigrationDigestV1, MigrationIdentityErrorV1,
    ProtectedPrimaryOverlapPairV1, SealedQuarantineEntryV3, SealedQuarantineManifestV3,
    SourceCaseV3, UnavailablePreexistingLossManifestV3, UnavailablePreexistingLossManifestV4,
    UnavailablePreexistingLossV3, UnavailablePreexistingLossV4,
};
use crate::domain::persistence::StoreV1;
use crate::domain::repository::RepositoryRootAdmissionV3;
use crate::foundation::core::deterministic_cbor::CborValue;
use crate::foundation::core::legacy_quarantine::{
    FoundationLegacyPayloadStateV3, FoundationLegacyQuarantineErrorV1,
    FoundationLegacyQuarantineFinalityV2, FoundationLegacyQuarantineLeaseV1,
    FoundationLegacyQuarantineLeaseV2, FoundationSourceCopyContinuationV1,
    FoundationSourceCopyContinuationV2, LegacyQuarantineExpectedSourceSetV3,
    LegacyQuarantineExpectedSourceSetV4, LegacyQuarantineOwnerDomainV3,
    ProtectedPrimaryBoundaryPortV1, QuarantineCustodyPortV1,
};
use crate::foundation::core::secure_fs::DescriptorCensusLimitsV1;
use crate::foundation::core::secure_fs::DescriptorCensusObjectKindV1;
use crate::foundation::core::{
    DeclaredRootUniverseLeaseV1, OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the offline workflow keeps every owner, custody, and expected-old input explicit"
)]
pub(crate) fn execute_offline_live_set_v3<P, Q, F>(
    repository_store: &StoreV1,
    repository_expected_sources: LegacyQuarantineExpectedSourceSetV3,
    installation_census: &InstallationCensusV1,
    installation_expected_sources: LegacyQuarantineExpectedSourceSetV3,
    protected_primary: P,
    custody: Q,
    invocation: [u8; 32],
    limits: DescriptorCensusLimitsV1,
    custody_lease_id: MigrationDigestV1,
    expected_old_id: MigrationDigestV1,
    classify: F,
) -> Result<Stage11PhysicalClosureV3, Stage11LiveSetOperationErrorV3>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
    F: FnOnce(
        &LegacySourceCaseManifestV3,
    ) -> Result<MigrationClassificationManifestV3, Stage11LiveSetOperationErrorV3>,
{
    let live = Stage11LiveSetContinuationV3::from_live_owners(
        repository_store,
        repository_expected_sources,
        installation_census,
        installation_expected_sources,
        protected_primary,
        custody,
        invocation,
        limits,
    )?;
    let classifications = classify(live.source_cases())?;
    live.copy_present_sources(classifications, custody_lease_id, expected_old_id)?
        .finish()
}

pub(crate) struct Stage11LiveSetContinuationV3<P, Q> {
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    loss_rows: Vec<UnavailablePreexistingLossV3>,
    source_tokens: BTreeMap<MigrationDigestV1, [u8; 32]>,
    physical: FoundationSourceCopyContinuationV1<P, Q>,
}

impl<P, Q> Stage11LiveSetContinuationV3<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "the live owner admission keeps both packet-bound source sets explicit"
    )]
    pub(crate) fn from_live_owners(
        repository_store: &StoreV1,
        repository_expected_sources: LegacyQuarantineExpectedSourceSetV3,
        installation_census: &InstallationCensusV1,
        installation_expected_sources: LegacyQuarantineExpectedSourceSetV3,
        protected_primary: P,
        custody: Q,
        invocation: [u8; 32],
        limits: DescriptorCensusLimitsV1,
    ) -> Result<Self, Stage11LiveSetOperationErrorV3> {
        let repository = RepositoryRootAdmissionV3::mint_from_store(
            repository_store,
            repository_expected_sources,
        )?;
        let installation =
            installation_census.admit_legacy_quarantine_roots_v3(installation_expected_sources)?;
        let foundation = FoundationLegacyQuarantineLeaseV1::acquire(
            repository,
            installation,
            protected_primary,
            custody,
            invocation,
            limits,
        )?;
        Self::from_foundation(foundation)
    }

    pub(crate) fn from_foundation(
        foundation: FoundationLegacyQuarantineLeaseV1<P, Q>,
    ) -> Result<Self, Stage11LiveSetOperationErrorV3> {
        let foundation_invocation = MigrationDigestV1::from_digest(foundation.invocation())?;
        let admitted_set_id = MigrationDigestV1::from_digest(foundation.admitted_set())?;
        let mut rows = Vec::with_capacity(foundation.source_cases().len());
        let mut source_tokens = BTreeMap::new();
        let mut source_facts = BTreeMap::new();
        let mut loss_rows = Vec::new();
        for source in foundation.source_cases() {
            let owner = match source.owner() {
                LegacyQuarantineOwnerDomainV3::Repository => LegacyOwnerDomainV3::Repository,
                LegacyQuarantineOwnerDomainV3::Installation => LegacyOwnerDomainV3::Installation,
                LegacyQuarantineOwnerDomainV3::ProtectedPrimary => {
                    LegacyOwnerDomainV3::ProtectedPrimary
                }
            };
            let mut display_locator = source.display_locator().to_vec();
            if !display_locator.ends_with(b"/") {
                display_locator.push(b'/');
            }
            display_locator.extend_from_slice(source.relative_locator());
            let node_kind = match source.kind() {
                DescriptorCensusObjectKindV1::RegularFile => LegacyNodeKindV3::RegularFile,
                DescriptorCensusObjectKindV1::SymbolicLink => LegacyNodeKindV3::SymbolicLink,
            };
            let root_binding = MigrationDigestV1::from_digest(source.root_binding())?;
            let resolved_locator_commitment = MigrationDigestV1::identify(
                b"maestro.migration.resolved-leaf-locator.v3\0",
                &CborValue::Array(vec![
                    root_binding.canonical_value(),
                    CborValue::Bytes(source.relative_locator().to_vec()),
                ]),
            )?;
            let object_identity = MigrationDigestV1::from_digest(source.object_identity())?;
            let content_sha256 = MigrationDigestV1::from_digest(source.content_identity())?;
            let metadata_commitment = MigrationDigestV1::identify(
                b"maestro.migration.source-metadata.v3\0",
                &CborValue::Array(vec![
                    CborValue::Unsigned(match node_kind {
                        LegacyNodeKindV3::RegularFile => 1,
                        LegacyNodeKindV3::SymbolicLink => 2,
                    }),
                    CborValue::Unsigned(source.logical_byte_length()),
                    object_identity.canonical_value(),
                    content_sha256.canonical_value(),
                ]),
            )?;
            let membership = MembershipKeyV3::from_foundation(
                owner,
                root_binding,
                display_locator,
                resolved_locator_commitment,
                object_identity,
                node_kind,
                metadata_commitment,
                MigrationDigestV1::from_digest(source.owner_currentness())?,
                MigrationDigestV1::from_digest(source.owner_attestation())?,
            )?;
            let payload_state = match source.payload_state() {
                FoundationLegacyPayloadStateV3::Present => LegacyPayloadStateV3::Present,
                FoundationLegacyPayloadStateV3::UnavailablePreexistingLoss => {
                    LegacyPayloadStateV3::UnavailablePreexistingLoss
                }
            };
            let source_case = SourceCaseV3::from_foundation(
                membership,
                foundation_invocation,
                payload_state,
                source.logical_byte_length(),
                content_sha256,
                metadata_commitment,
            )?;
            if payload_state == LegacyPayloadStateV3::Present {
                if source_tokens
                    .insert(source_case.identity(), source.source_token())
                    .is_some()
                {
                    return Err(Stage11LiveSetOperationErrorV3::DuplicateSource);
                }
            } else {
                let loss_evidence_id = source
                    .loss_evidence_id()
                    .ok_or(Stage11LiveSetOperationErrorV3::MissingLossEvidence)?;
                loss_rows.push(UnavailablePreexistingLossV3::new(
                    &source_case,
                    source.logical_byte_length(),
                    content_sha256,
                    metadata_commitment,
                    MigrationDigestV1::from_digest(source.source_provenance_id())?,
                    MigrationDigestV1::from_digest(loss_evidence_id)?,
                )?);
            }
            source_facts.insert(
                source.source_token(),
                (
                    source_case.clone(),
                    MigrationDigestV1::from_digest(source.mount_identity())?,
                    MigrationDigestV1::from_digest(source.provider_identity())?,
                ),
            );
            rows.push(source_case);
        }
        let source_cases =
            LegacySourceCaseManifestV3::new(foundation_invocation, admitted_set_id, rows)?;
        let overlaps = DeclaredOverlapManifestV2::new(
            &source_cases,
            foundation
                .overlap_pairs()
                .iter()
                .map(|pair| {
                    let (owner, owner_mount, owner_provider) = source_facts
                        .get(&pair.owner_source_token())
                        .ok_or(Stage11LiveSetOperationErrorV3::InvalidOverlap)?;
                    let (primary, primary_mount, primary_provider) = source_facts
                        .get(&pair.primary_source_token())
                        .ok_or(Stage11LiveSetOperationErrorV3::InvalidOverlap)?;
                    Ok(ProtectedPrimaryOverlapPairV1::from_foundation(
                        owner,
                        primary,
                        *owner_mount,
                        *primary_mount,
                        *owner_provider,
                        *primary_provider,
                    )?)
                })
                .collect::<Result<Vec<_>, Stage11LiveSetOperationErrorV3>>()?,
        )?;
        Ok(Self {
            source_cases,
            overlaps,
            loss_rows,
            source_tokens,
            physical: foundation.into_copy_continuation(),
        })
    }

    pub(crate) const fn source_cases(&self) -> &LegacySourceCaseManifestV3 {
        &self.source_cases
    }

    pub(crate) const fn overlaps(&self) -> &DeclaredOverlapManifestV2 {
        &self.overlaps
    }

    pub(crate) fn copy_present_sources(
        mut self,
        classifications: MigrationClassificationManifestV3,
        custody_lease_id: MigrationDigestV1,
        expected_old_id: MigrationDigestV1,
    ) -> Result<Stage11SealedCopyContinuationV3<P, Q>, Stage11LiveSetOperationErrorV3> {
        let mut entries = Vec::new();
        for source in self.source_cases.rows().to_vec() {
            if source.payload_state() != LegacyPayloadStateV3::Present {
                continue;
            }
            let Some(token) = self.source_tokens.remove(&source.identity()) else {
                self.physical.rollback()?;
                return Err(Stage11LiveSetOperationErrorV3::MissingSourceToken);
            };
            let receipt = match self.physical.copy_once(token) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.physical.rollback()?;
                    return Err(error.into());
                }
            };
            let custody_record_id = MigrationDigestV1::from_digest(receipt.identity())?;
            let entry = match SealedQuarantineEntryV3::from_custody(
                &source,
                receipt.copied_length(),
                MigrationDigestV1::from_digest(receipt.copied_sha256())?,
                source.metadata_commitment(),
                custody_record_id,
            ) {
                Ok(entry) => entry,
                Err(error) => {
                    self.physical.rollback()?;
                    return Err(error.into());
                }
            };
            entries.push(entry);
        }
        if !self.source_tokens.is_empty() {
            self.physical.rollback()?;
            return Err(Stage11LiveSetOperationErrorV3::MissingSourceToken);
        }
        let losses = match UnavailablePreexistingLossManifestV3::new(
            &self.source_cases,
            &classifications,
            self.loss_rows,
        ) {
            Ok(losses) => losses,
            Err(error) => {
                self.physical.rollback()?;
                return Err(error.into());
            }
        };
        let quarantine = match SealedQuarantineManifestV3::new(
            &self.source_cases,
            &classifications,
            custody_lease_id,
            expected_old_id,
            entries,
        ) {
            Ok(quarantine) => quarantine,
            Err(error) => {
                self.physical.rollback()?;
                return Err(error.into());
            }
        };
        Ok(Stage11SealedCopyContinuationV3 {
            source_cases: self.source_cases,
            overlaps: self.overlaps,
            classifications,
            losses,
            quarantine,
            physical: self.physical,
        })
    }
}

pub(crate) struct Stage11SealedCopyContinuationV3<P, Q> {
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    classifications: MigrationClassificationManifestV3,
    losses: UnavailablePreexistingLossManifestV3,
    quarantine: SealedQuarantineManifestV3,
    physical: FoundationSourceCopyContinuationV1<P, Q>,
}

impl<P, Q> Stage11SealedCopyContinuationV3<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    pub(crate) const fn source_cases(&self) -> &LegacySourceCaseManifestV3 {
        &self.source_cases
    }

    pub(crate) const fn classifications(&self) -> &MigrationClassificationManifestV3 {
        &self.classifications
    }

    pub(crate) const fn overlaps(&self) -> &DeclaredOverlapManifestV2 {
        &self.overlaps
    }

    pub(crate) const fn losses(&self) -> &UnavailablePreexistingLossManifestV3 {
        &self.losses
    }

    pub(crate) const fn quarantine(&self) -> &SealedQuarantineManifestV3 {
        &self.quarantine
    }

    pub(crate) fn finish(self) -> Result<Stage11PhysicalClosureV3, Stage11LiveSetOperationErrorV3> {
        let rollback = LegacyRollbackAssessmentV3::assess(
            &self.source_cases,
            &self.classifications,
            &self.losses,
            &self.quarantine,
        )?;
        let (closure, persistence_receipt) = self
            .physical
            .finish(self.quarantine.identity().into_bytes())?;
        Ok(Stage11PhysicalClosureV3 {
            foundation_closure_id: MigrationDigestV1::from_digest(closure.identity())?,
            persistence_receipt_id: MigrationDigestV1::from_digest(persistence_receipt)?,
            source_cases: self.source_cases,
            overlaps: self.overlaps,
            classifications: self.classifications,
            losses: self.losses,
            quarantine: self.quarantine,
            rollback,
        })
    }
}

pub(crate) struct Stage11PhysicalClosureV3 {
    foundation_closure_id: MigrationDigestV1,
    persistence_receipt_id: MigrationDigestV1,
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    classifications: MigrationClassificationManifestV3,
    losses: UnavailablePreexistingLossManifestV3,
    quarantine: SealedQuarantineManifestV3,
    rollback: LegacyRollbackAssessmentV3,
}

impl Stage11PhysicalClosureV3 {
    pub(crate) const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.foundation_closure_id
    }

    pub(crate) const fn persistence_receipt_id(&self) -> MigrationDigestV1 {
        self.persistence_receipt_id
    }

    pub(crate) const fn source_cases(&self) -> &LegacySourceCaseManifestV3 {
        &self.source_cases
    }

    pub(crate) const fn classifications(&self) -> &MigrationClassificationManifestV3 {
        &self.classifications
    }

    pub(crate) const fn overlaps(&self) -> &DeclaredOverlapManifestV2 {
        &self.overlaps
    }

    pub(crate) const fn losses(&self) -> &UnavailablePreexistingLossManifestV3 {
        &self.losses
    }

    pub(crate) const fn quarantine(&self) -> &SealedQuarantineManifestV3 {
        &self.quarantine
    }

    pub(crate) const fn rollback(&self) -> &LegacyRollbackAssessmentV3 {
        &self.rollback
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "V4 consumes both nominal owner universes and all three complete evidence sets"
)]
pub(crate) fn execute_offline_live_set_v4<R, I, RE, IE, PE, P, Q, F>(
    repository: R,
    installation: I,
    protected_primary: P,
    custody: Q,
    expected_sources: LegacyQuarantineExpectedSourceSetV4,
    repository_evidence_issuer: RE,
    installation_evidence_issuer: IE,
    protected_primary_evidence_issuer: PE,
    invocation: [u8; 32],
    custody_lease_id: MigrationDigestV1,
    expected_old_id: MigrationDigestV1,
    classify: F,
) -> Result<Stage11PhysicalClosureV4, Stage11LiveSetOperationErrorV4>
where
    R: DeclaredRootUniverseLeaseV1,
    I: DeclaredRootUniverseLeaseV1,
    RE: OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
    IE: OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
    PE: OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
    F: FnOnce(
        &LegacySourceCaseManifestV3,
    ) -> Result<MigrationClassificationManifestV3, Stage11LiveSetOperationErrorV4>,
{
    let live = Stage11LiveSetContinuationV4::from_live_owners(
        repository,
        installation,
        protected_primary,
        custody,
        expected_sources,
        repository_evidence_issuer,
        installation_evidence_issuer,
        protected_primary_evidence_issuer,
        invocation,
    )?;
    let classifications = classify(live.source_cases())?;
    live.copy_present_sources(classifications, custody_lease_id, expected_old_id)?
        .finish()
}

pub(crate) struct Stage11LiveSetContinuationV4<P, Q> {
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    loss_rows: Vec<UnavailablePreexistingLossV4>,
    source_tokens: BTreeMap<MigrationDigestV1, [u8; 32]>,
    physical: FoundationSourceCopyContinuationV2<P, Q>,
}

impl<P, Q> Stage11LiveSetContinuationV4<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "V4 accepts only owner-issued universes and loss evidence, never caller roots"
    )]
    pub(crate) fn from_live_owners<R, I, RE, IE, PE>(
        repository: R,
        installation: I,
        protected_primary: P,
        custody: Q,
        expected_sources: LegacyQuarantineExpectedSourceSetV4,
        repository_evidence_issuer: RE,
        installation_evidence_issuer: IE,
        protected_primary_evidence_issuer: PE,
        invocation: [u8; 32],
    ) -> Result<Self, Stage11LiveSetOperationErrorV4>
    where
        R: DeclaredRootUniverseLeaseV1,
        I: DeclaredRootUniverseLeaseV1,
        RE: OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
        IE: OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
        PE: OwnerUnavailablePreexistingLossEvidenceIssuerPortV1,
    {
        let foundation = FoundationLegacyQuarantineLeaseV2::acquire(
            repository,
            installation,
            protected_primary,
            custody,
            expected_sources,
            repository_evidence_issuer,
            installation_evidence_issuer,
            protected_primary_evidence_issuer,
            invocation,
        )?;
        Self::from_foundation(foundation)
    }

    pub(crate) fn from_foundation(
        mut foundation: FoundationLegacyQuarantineLeaseV2<P, Q>,
    ) -> Result<Self, Stage11LiveSetOperationErrorV4> {
        let foundation_invocation = MigrationDigestV1::from_digest(foundation.invocation())?;
        let admitted_set_id = MigrationDigestV1::from_digest(foundation.admitted_set())?;
        let materialized = foundation
            .take_migration_sources()
            .into_iter()
            .map(|parts| {
                FoundationMaterializedSourceCaseV3::from_foundation_v2(parts, foundation_invocation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut rows = Vec::with_capacity(materialized.len());
        let mut source_tokens = BTreeMap::new();
        let mut source_facts = BTreeMap::new();
        let mut loss_rows = Vec::new();
        for mut source in materialized {
            let source_case = source.source_case().clone();
            if source_case.payload_state() == LegacyPayloadStateV3::Present {
                if source_tokens
                    .insert(source_case.identity(), source.source_token())
                    .is_some()
                {
                    return Err(Stage11LiveSetOperationErrorV4::DuplicateSource);
                }
                if source.take_loss_receipt().is_some() {
                    return Err(Stage11LiveSetOperationErrorV4::UnexpectedLossReceipt);
                }
            } else {
                let receipt = source
                    .take_loss_receipt()
                    .ok_or(Stage11LiveSetOperationErrorV4::MissingLossReceipt)?;
                loss_rows.push(UnavailablePreexistingLossV4::from_foundation(
                    &source_case,
                    source.source_token(),
                    receipt,
                )?);
            }
            source_facts.insert(
                source.source_token(),
                (
                    source_case,
                    source.mount_identity(),
                    source.provider_identity(),
                    source.anchor_identity(),
                    source.fence_identity(),
                ),
            );
            rows.push(source.into_source_case());
        }
        let source_cases =
            LegacySourceCaseManifestV3::new(foundation_invocation, admitted_set_id, rows)?;
        let overlaps = DeclaredOverlapManifestV2::new(
            &source_cases,
            foundation
                .overlap_pairs()
                .iter()
                .map(|pair| {
                    let (owner, owner_mount, owner_provider, _, _) = source_facts
                        .get(&pair.owner_source_token())
                        .ok_or(Stage11LiveSetOperationErrorV4::InvalidOverlap)?;
                    let (primary, primary_mount, primary_provider, _, _) = source_facts
                        .get(&pair.primary_source_token())
                        .ok_or(Stage11LiveSetOperationErrorV4::InvalidOverlap)?;
                    Ok(ProtectedPrimaryOverlapPairV1::from_foundation(
                        owner,
                        primary,
                        *owner_mount,
                        *primary_mount,
                        *owner_provider,
                        *primary_provider,
                    )?)
                })
                .collect::<Result<Vec<_>, Stage11LiveSetOperationErrorV4>>()?,
        )?;
        Ok(Self {
            source_cases,
            overlaps,
            loss_rows,
            source_tokens,
            physical: foundation.into_copy_continuation(),
        })
    }

    pub(crate) const fn source_cases(&self) -> &LegacySourceCaseManifestV3 {
        &self.source_cases
    }

    pub(crate) const fn overlaps(&self) -> &DeclaredOverlapManifestV2 {
        &self.overlaps
    }

    pub(crate) fn copy_present_sources(
        mut self,
        classifications: MigrationClassificationManifestV3,
        custody_lease_id: MigrationDigestV1,
        expected_old_id: MigrationDigestV1,
    ) -> Result<Stage11SealedCopyContinuationV4<P, Q>, Stage11LiveSetOperationErrorV4> {
        let mut entries = Vec::new();
        for source in self.source_cases.rows().to_vec() {
            if source.payload_state() != LegacyPayloadStateV3::Present {
                continue;
            }
            let Some(token) = self.source_tokens.remove(&source.identity()) else {
                self.physical.rollback()?;
                return Err(Stage11LiveSetOperationErrorV4::MissingSourceToken);
            };
            let receipt = match self.physical.copy_once(token) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.physical.rollback()?;
                    return Err(error.into());
                }
            };
            let entry = match SealedQuarantineEntryV3::from_custody(
                &source,
                receipt.copied_length(),
                MigrationDigestV1::from_digest(receipt.copied_sha256())?,
                source.metadata_commitment(),
                MigrationDigestV1::from_digest(receipt.identity())?,
            ) {
                Ok(entry) => entry,
                Err(error) => {
                    self.physical.rollback()?;
                    return Err(error.into());
                }
            };
            entries.push(entry);
        }
        if !self.source_tokens.is_empty() {
            self.physical.rollback()?;
            return Err(Stage11LiveSetOperationErrorV4::MissingSourceToken);
        }
        let losses = match UnavailablePreexistingLossManifestV4::new(
            &self.source_cases,
            &classifications,
            self.loss_rows,
        ) {
            Ok(losses) => losses,
            Err(error) => {
                self.physical.rollback()?;
                return Err(error.into());
            }
        };
        let quarantine = match SealedQuarantineManifestV3::new(
            &self.source_cases,
            &classifications,
            custody_lease_id,
            expected_old_id,
            entries,
        ) {
            Ok(quarantine) => quarantine,
            Err(error) => {
                self.physical.rollback()?;
                return Err(error.into());
            }
        };
        Ok(Stage11SealedCopyContinuationV4 {
            source_cases: self.source_cases,
            overlaps: self.overlaps,
            classifications,
            losses,
            quarantine,
            physical: self.physical,
        })
    }
}

pub(crate) struct Stage11SealedCopyContinuationV4<P, Q> {
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    classifications: MigrationClassificationManifestV3,
    losses: UnavailablePreexistingLossManifestV4,
    quarantine: SealedQuarantineManifestV3,
    physical: FoundationSourceCopyContinuationV2<P, Q>,
}

impl<P, Q> Stage11SealedCopyContinuationV4<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    pub(crate) const fn losses(&self) -> &UnavailablePreexistingLossManifestV4 {
        &self.losses
    }

    pub(crate) fn finish(self) -> Result<Stage11PhysicalClosureV4, Stage11LiveSetOperationErrorV4> {
        let rollback = LegacyRollbackAssessmentV4::assess(
            &self.source_cases,
            &self.classifications,
            &self.losses,
            &self.quarantine,
        )?;
        match self
            .physical
            .finish(self.quarantine.identity().into_bytes())?
        {
            FoundationLegacyQuarantineFinalityV2::Closed {
                closure,
                persistence_receipt,
            } => Ok(Stage11PhysicalClosureV4::Closed(
                Stage11ClosedPhysicalClosureV4 {
                    foundation_closure_id: MigrationDigestV1::from_digest(closure.identity())?,
                    persistence_receipt_id: MigrationDigestV1::from_digest(persistence_receipt)?,
                    source_cases: self.source_cases,
                    overlaps: self.overlaps,
                    classifications: self.classifications,
                    losses: self.losses,
                    quarantine: self.quarantine,
                    rollback,
                },
            )),
            FoundationLegacyQuarantineFinalityV2::RecoveryRequired {
                admitted_set,
                persistence_receipt,
            } => Ok(Stage11PhysicalClosureV4::RecoveryRequired {
                admitted_set_id: MigrationDigestV1::from_digest(admitted_set)?,
                persistence_receipt_id: MigrationDigestV1::from_digest(persistence_receipt)?,
            }),
            FoundationLegacyQuarantineFinalityV2::InDoubt {
                admitted_set,
                candidate_manifest,
            } => Ok(Stage11PhysicalClosureV4::InDoubt {
                admitted_set_id: MigrationDigestV1::from_digest(admitted_set)?,
                candidate_manifest_id: MigrationDigestV1::from_digest(candidate_manifest)?,
            }),
        }
    }
}

pub(crate) enum Stage11PhysicalClosureV4 {
    Closed(Stage11ClosedPhysicalClosureV4),
    RecoveryRequired {
        admitted_set_id: MigrationDigestV1,
        persistence_receipt_id: MigrationDigestV1,
    },
    InDoubt {
        admitted_set_id: MigrationDigestV1,
        candidate_manifest_id: MigrationDigestV1,
    },
}

pub(crate) struct Stage11ClosedPhysicalClosureV4 {
    foundation_closure_id: MigrationDigestV1,
    persistence_receipt_id: MigrationDigestV1,
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    classifications: MigrationClassificationManifestV3,
    losses: UnavailablePreexistingLossManifestV4,
    quarantine: SealedQuarantineManifestV3,
    rollback: LegacyRollbackAssessmentV4,
}

impl Stage11ClosedPhysicalClosureV4 {
    pub(crate) const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.foundation_closure_id
    }

    pub(crate) const fn persistence_receipt_id(&self) -> MigrationDigestV1 {
        self.persistence_receipt_id
    }

    pub(crate) const fn source_cases(&self) -> &LegacySourceCaseManifestV3 {
        &self.source_cases
    }

    pub(crate) const fn classifications(&self) -> &MigrationClassificationManifestV3 {
        &self.classifications
    }

    pub(crate) const fn overlaps(&self) -> &DeclaredOverlapManifestV2 {
        &self.overlaps
    }

    pub(crate) const fn losses(&self) -> &UnavailablePreexistingLossManifestV4 {
        &self.losses
    }

    pub(crate) const fn quarantine(&self) -> &SealedQuarantineManifestV3 {
        &self.quarantine
    }

    pub(crate) const fn rollback(&self) -> &LegacyRollbackAssessmentV4 {
        &self.rollback
    }
}

#[derive(Debug, Error)]
pub(crate) enum Stage11LiveSetOperationErrorV4 {
    #[error("Stage-11 V4 Foundation census contains a duplicate semantic source")]
    DuplicateSource,
    #[error("Stage-11 V4 descriptor copy continuation lost a source token")]
    MissingSourceToken,
    #[error("Stage-11 V4 Foundation overlap pair did not bind two retained source cases")]
    InvalidOverlap,
    #[error("Stage-11 V4 unavailable source lacks its Foundation loss receipt")]
    MissingLossReceipt,
    #[error("Stage-11 V4 present source carried a loss receipt")]
    UnexpectedLossReceipt,
    #[error(transparent)]
    Foundation(#[from] FoundationLegacyQuarantineErrorV1),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error(transparent)]
    LiveSet(#[from] LiveSetV3Error),
}

#[derive(Debug, Error)]
pub(crate) enum Stage11LiveSetOperationErrorV3 {
    #[error("Stage-11 Foundation census contains a duplicate semantic source")]
    DuplicateSource,
    #[error("Stage-11 descriptor copy continuation lost a source token")]
    MissingSourceToken,
    #[error("Stage-11 Foundation overlap pair did not bind two retained source cases")]
    InvalidOverlap,
    #[error("Stage-11 unavailable source lacks independent loss evidence")]
    MissingLossEvidence,
    #[error(transparent)]
    Foundation(#[from] FoundationLegacyQuarantineErrorV1),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error(transparent)]
    LiveSet(#[from] LiveSetV3Error),
    #[error(transparent)]
    RepositoryAdmission(#[from] crate::domain::repository::RepositoryRootAdmissionErrorV3),
    #[error(transparent)]
    InstallationAdmission(#[from] crate::domain::installation::InstallationCensusErrorV1),
}
