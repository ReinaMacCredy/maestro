use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::installation::InstallationCensusV1;
use crate::domain::migration::runtime::{
    DeclaredOverlapManifestV2, LegacyNodeKindV3, LegacyOwnerDomainV3, LegacyPayloadStateV3,
    LegacySourceCaseManifestV3, LiveSetV3Error, MembershipKeyV3, MigrationClassificationManifestV3,
    MigrationDigestV1, MigrationIdentityErrorV1, ProtectedPrimaryOverlapPairV1,
    SealedQuarantineEntryV3, SealedQuarantineManifestV3, SourceCaseV3,
};
use crate::domain::persistence::StoreV1;
use crate::domain::repository::RepositoryRootAdmissionV3;
use crate::foundation::core::deterministic_cbor::CborValue;
use crate::foundation::core::legacy_quarantine::{
    FoundationLegacyQuarantineClosureV1, FoundationLegacyQuarantineErrorV1,
    FoundationLegacyQuarantineLeaseV1, FoundationSourceCopyContinuationV1,
    LegacyQuarantineOwnerDomainV3, ProtectedPrimaryBoundaryPortV1, QuarantineCustodyPortV1,
};
use crate::foundation::core::secure_fs::DescriptorCensusLimitsV1;
use crate::foundation::core::secure_fs::DescriptorCensusObjectKindV1;

pub(crate) struct Stage11LiveSetContinuationV3<P, Q> {
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    source_tokens: BTreeMap<MigrationDigestV1, [u8; 32]>,
    physical: FoundationSourceCopyContinuationV1<P, Q>,
}

impl<P, Q> Stage11LiveSetContinuationV3<P, Q>
where
    P: ProtectedPrimaryBoundaryPortV1,
    Q: QuarantineCustodyPortV1,
{
    pub(crate) fn from_live_owners(
        repository_store: &StoreV1,
        installation_census: &InstallationCensusV1,
        protected_primary: P,
        custody: Q,
        invocation: [u8; 32],
        limits: DescriptorCensusLimitsV1,
    ) -> Result<Self, Stage11LiveSetOperationErrorV3> {
        let repository = RepositoryRootAdmissionV3::mint_from_store(repository_store)?;
        let installation = installation_census.admit_legacy_quarantine_roots_v3()?;
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
            let row = source.row();
            let node_kind = match row.kind() {
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
            let object_identity = MigrationDigestV1::from_digest(row.object_identity())?;
            let content_sha256 = MigrationDigestV1::from_digest(row.content_identity())?;
            let metadata_commitment = MigrationDigestV1::identify(
                b"maestro.migration.source-metadata.v3\0",
                &CborValue::Array(vec![
                    CborValue::Unsigned(match node_kind {
                        LegacyNodeKindV3::RegularFile => 1,
                        LegacyNodeKindV3::SymbolicLink => 2,
                    }),
                    CborValue::Unsigned(row.logical_byte_length()),
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
            let source_case = SourceCaseV3::from_foundation(
                membership,
                foundation_invocation,
                LegacyPayloadStateV3::Present,
                row.logical_byte_length(),
                content_sha256,
                metadata_commitment,
            )?;
            if source_tokens
                .insert(source_case.identity(), source.source_token())
                .is_some()
            {
                return Err(Stage11LiveSetOperationErrorV3::DuplicateSource);
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
            quarantine,
            physical: self.physical,
        })
    }
}

pub(crate) struct Stage11SealedCopyContinuationV3<P, Q> {
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    classifications: MigrationClassificationManifestV3,
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

    pub(crate) const fn quarantine(&self) -> &SealedQuarantineManifestV3 {
        &self.quarantine
    }

    pub(crate) fn finish(self) -> Result<Stage11PhysicalClosureV3, Stage11LiveSetOperationErrorV3> {
        let (closure, persistence_receipt) = self
            .physical
            .finish(self.quarantine.identity().into_bytes())?;
        Ok(Stage11PhysicalClosureV3::new(
            closure,
            persistence_receipt,
            self.source_cases,
            self.overlaps,
            self.classifications,
            self.quarantine,
        )?)
    }
}

pub(crate) struct Stage11PhysicalClosureV3 {
    foundation_closure_id: MigrationDigestV1,
    persistence_receipt_id: MigrationDigestV1,
    source_cases: LegacySourceCaseManifestV3,
    overlaps: DeclaredOverlapManifestV2,
    classifications: MigrationClassificationManifestV3,
    quarantine: SealedQuarantineManifestV3,
}

impl Stage11PhysicalClosureV3 {
    fn new(
        closure: FoundationLegacyQuarantineClosureV1,
        persistence_receipt: [u8; 32],
        source_cases: LegacySourceCaseManifestV3,
        overlaps: DeclaredOverlapManifestV2,
        classifications: MigrationClassificationManifestV3,
        quarantine: SealedQuarantineManifestV3,
    ) -> Result<Self, MigrationIdentityErrorV1> {
        Ok(Self {
            foundation_closure_id: MigrationDigestV1::from_digest(closure.identity())?,
            persistence_receipt_id: MigrationDigestV1::from_digest(persistence_receipt)?,
            source_cases,
            overlaps,
            classifications,
            quarantine,
        })
    }

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

    pub(crate) const fn quarantine(&self) -> &SealedQuarantineManifestV3 {
        &self.quarantine
    }
}

#[derive(Debug, Error)]
pub(crate) enum Stage11LiveSetOperationErrorV3 {
    #[error("Stage-11 Foundation census contains a duplicate semantic source")]
    DuplicateSource,
    #[error("Stage-11 descriptor copy continuation lost a source token")]
    MissingSourceToken,
    #[error("Stage-11 Foundation overlap pair did not bind two retained source cases")]
    InvalidOverlap,
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
