#![allow(
    dead_code,
    reason = "V8 owner-resolved root universe awaits MainIntegration owner wiring"
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::legacy_quarantine::LegacyQuarantineOwnerDomainV3;

const ROOT_ROW_DOMAIN_V1: &[u8] = b"maestro.foundation.declared-root-row.v1\0";
const ROOT_UNIVERSE_DOMAIN_V1: &[u8] = b"maestro.foundation.declared-root-universe.v1\0";
const OWNER_CURRENTNESS_DOMAIN_V1: &[u8] =
    b"maestro.foundation.owner-root-universe-currentness.v1\0";
const ABSENCE_FENCE_DOMAIN_V1: &[u8] = b"maestro.foundation.declared-root-absence-fence.v1\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum FoundationDeclaredRootRoleV1 {
    RepositoryStore,
    Active,
    Inactive,
    Snapshot,
    Cache,
    Archive,
    Host,
    Legacy,
}

impl FoundationDeclaredRootRoleV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::RepositoryStore => 1,
            Self::Active => 2,
            Self::Inactive => 3,
            Self::Snapshot => 4,
            Self::Cache => 5,
            Self::Archive => 6,
            Self::Host => 7,
            Self::Legacy => 8,
        }
    }
}

pub(crate) struct FoundationPresentRootAcquisitionV1 {
    path: PathBuf,
    declared_locator_commitment: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl FoundationPresentRootAcquisitionV1 {
    pub(crate) fn from_owner(
        path: PathBuf,
        declared_locator_commitment: [u8; 32],
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if !path.is_absolute() || declared_locator_commitment == [0; 32] {
            return Err(FoundationRootUniverseErrorV1::InvalidPresentCapability);
        }
        Ok(Self {
            path,
            declared_locator_commitment,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::foundation::core) fn into_path(self) -> (PathBuf, [u8; 32]) {
        (self.path, self.declared_locator_commitment)
    }
}

pub(crate) struct FoundationDeclaredAbsenceFenceV1 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    declaration_id: [u8; 32],
    declaration_revision: u64,
    owner_realm: [u8; 32],
    operation_attempt: [u8; 32],
    provider_id: [u8; 32],
    currentness: [u8; 32],
    fence: [u8; 32],
    revocation_revision: u64,
}

impl FoundationDeclaredAbsenceFenceV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "absence is owner-fenced against the complete declaration/currentness tuple"
    )]
    pub(crate) fn from_owner(
        owner: LegacyQuarantineOwnerDomainV3,
        declaration_id: [u8; 32],
        declaration_revision: u64,
        owner_realm: [u8; 32],
        operation_attempt: [u8; 32],
        provider_id: [u8; 32],
        currentness: [u8; 32],
        fence: [u8; 32],
        revocation_revision: u64,
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if [
            declaration_id,
            owner_realm,
            operation_attempt,
            provider_id,
            currentness,
            fence,
        ]
        .contains(&[0; 32])
            || declaration_revision == 0
            || revocation_revision == 0
        {
            return Err(FoundationRootUniverseErrorV1::InvalidAbsenceFence);
        }
        let identity = commitment(
            ABSENCE_FENCE_DOMAIN_V1,
            &[
                &[owner.tag()],
                &declaration_id,
                &declaration_revision.to_be_bytes(),
                &owner_realm,
                &operation_attempt,
                &provider_id,
                &currentness,
                &fence,
                &revocation_revision.to_be_bytes(),
            ],
        );
        Ok(Self {
            identity,
            owner,
            declaration_id,
            declaration_revision,
            owner_realm,
            operation_attempt,
            provider_id,
            currentness,
            fence,
            revocation_revision,
        })
    }
}

pub(in crate::foundation::core) enum FoundationDeclaredRootDispositionV1 {
    Present(FoundationPresentRootAcquisitionV1),
    DeclaredAbsent(FoundationDeclaredAbsenceFenceV1),
    Unsupported {
        reason_id: [u8; 32],
        provider_id: [u8; 32],
    },
}

pub(crate) struct FoundationDeclaredRootRowV1 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    declaration_id: [u8; 32],
    declaration_revision: u64,
    role: FoundationDeclaredRootRoleV1,
    required: bool,
    declared_locator_commitment: [u8; 32],
    provider_revision: u64,
    owner_realm: [u8; 32],
    operation_attempt: [u8; 32],
    currentness: [u8; 32],
    fence: [u8; 32],
    revocation_revision: u64,
    disposition: FoundationDeclaredRootDispositionV1,
}

impl FoundationDeclaredRootRowV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "a present declaration binds its complete owner and provider tuple"
    )]
    pub(crate) fn present(
        owner: LegacyQuarantineOwnerDomainV3,
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        provider_revision: u64,
        owner_realm: [u8; 32],
        operation_attempt: [u8; 32],
        currentness: [u8; 32],
        fence: [u8; 32],
        revocation_revision: u64,
        acquisition: FoundationPresentRootAcquisitionV1,
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if acquisition.declared_locator_commitment != declared_locator_commitment {
            return Err(FoundationRootUniverseErrorV1::InvalidPresentCapability);
        }
        Self::new(
            owner,
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            provider_revision,
            owner_realm,
            operation_attempt,
            currentness,
            fence,
            revocation_revision,
            FoundationDeclaredRootDispositionV1::Present(acquisition),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a declared absence binds its complete owner and provider tuple"
    )]
    pub(crate) fn declared_absent(
        owner: LegacyQuarantineOwnerDomainV3,
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        provider_revision: u64,
        owner_realm: [u8; 32],
        operation_attempt: [u8; 32],
        currentness: [u8; 32],
        fence: [u8; 32],
        revocation_revision: u64,
        absence: FoundationDeclaredAbsenceFenceV1,
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if absence.owner != owner
            || absence.declaration_id != declaration_id
            || absence.declaration_revision != declaration_revision
            || absence.owner_realm != owner_realm
            || absence.operation_attempt != operation_attempt
            || absence.currentness != currentness
            || absence.fence != fence
            || absence.revocation_revision != revocation_revision
        {
            return Err(FoundationRootUniverseErrorV1::InvalidAbsenceFence);
        }
        Self::new(
            owner,
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            provider_revision,
            owner_realm,
            operation_attempt,
            currentness,
            fence,
            revocation_revision,
            FoundationDeclaredRootDispositionV1::DeclaredAbsent(absence),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "an unsupported declaration remains a complete, explicit refusal row"
    )]
    pub(crate) fn unsupported(
        owner: LegacyQuarantineOwnerDomainV3,
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        provider_revision: u64,
        owner_realm: [u8; 32],
        operation_attempt: [u8; 32],
        currentness: [u8; 32],
        fence: [u8; 32],
        revocation_revision: u64,
        reason_id: [u8; 32],
        provider_id: [u8; 32],
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if reason_id == [0; 32] || provider_id == [0; 32] {
            return Err(FoundationRootUniverseErrorV1::InvalidUnsupportedRow);
        }
        Self::new(
            owner,
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            provider_revision,
            owner_realm,
            operation_attempt,
            currentness,
            fence,
            revocation_revision,
            FoundationDeclaredRootDispositionV1::Unsupported {
                reason_id,
                provider_id,
            },
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the closed row identity binds every declaration and currentness field"
    )]
    fn new(
        owner: LegacyQuarantineOwnerDomainV3,
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        provider_revision: u64,
        owner_realm: [u8; 32],
        operation_attempt: [u8; 32],
        currentness: [u8; 32],
        fence: [u8; 32],
        revocation_revision: u64,
        disposition: FoundationDeclaredRootDispositionV1,
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if [
            declaration_id,
            declared_locator_commitment,
            owner_realm,
            operation_attempt,
            currentness,
            fence,
        ]
        .contains(&[0; 32])
            || declaration_revision == 0
            || provider_revision == 0
            || revocation_revision == 0
            || (owner == LegacyQuarantineOwnerDomainV3::Repository
                && role != FoundationDeclaredRootRoleV1::RepositoryStore)
            || (owner == LegacyQuarantineOwnerDomainV3::Installation
                && role == FoundationDeclaredRootRoleV1::RepositoryStore)
            || owner == LegacyQuarantineOwnerDomainV3::ProtectedPrimary
        {
            return Err(FoundationRootUniverseErrorV1::InvalidDeclarationRow);
        }
        let (disposition_tag, disposition_id) = match &disposition {
            FoundationDeclaredRootDispositionV1::Present(_) => (1, [0; 32]),
            FoundationDeclaredRootDispositionV1::DeclaredAbsent(absence) => (2, absence.identity),
            FoundationDeclaredRootDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } => (
                3,
                commitment(
                    b"maestro.foundation.unsupported-root.v1\0",
                    &[reason_id, provider_id],
                ),
            ),
        };
        let identity = commitment(
            ROOT_ROW_DOMAIN_V1,
            &[
                &[owner.tag()],
                &declaration_id,
                &declaration_revision.to_be_bytes(),
                &[role.tag()],
                &[u8::from(required)],
                &declared_locator_commitment,
                &provider_revision.to_be_bytes(),
                &owner_realm,
                &operation_attempt,
                &currentness,
                &fence,
                &revocation_revision.to_be_bytes(),
                &[disposition_tag],
                &disposition_id,
            ],
        );
        Ok(Self {
            identity,
            owner,
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            provider_revision,
            owner_realm,
            operation_attempt,
            currentness,
            fence,
            revocation_revision,
            disposition,
        })
    }

    pub(in crate::foundation::core) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(in crate::foundation::core) const fn declaration_id(&self) -> [u8; 32] {
        self.declaration_id
    }

    pub(in crate::foundation::core) const fn owner(&self) -> LegacyQuarantineOwnerDomainV3 {
        self.owner
    }

    pub(in crate::foundation::core) const fn declaration_revision(&self) -> u64 {
        self.declaration_revision
    }

    pub(in crate::foundation::core) const fn role(&self) -> FoundationDeclaredRootRoleV1 {
        self.role
    }

    pub(in crate::foundation::core) const fn required(&self) -> bool {
        self.required
    }

    pub(in crate::foundation::core) const fn declared_locator_commitment(&self) -> [u8; 32] {
        self.declared_locator_commitment
    }

    pub(in crate::foundation::core) const fn operation_attempt(&self) -> [u8; 32] {
        self.operation_attempt
    }

    pub(in crate::foundation::core) const fn currentness(&self) -> [u8; 32] {
        self.currentness
    }

    pub(in crate::foundation::core) const fn provider_revision(&self) -> u64 {
        self.provider_revision
    }

    pub(in crate::foundation::core) const fn fence(&self) -> [u8; 32] {
        self.fence
    }

    pub(in crate::foundation::core) const fn revocation_revision(&self) -> u64 {
        self.revocation_revision
    }

    pub(in crate::foundation::core) fn into_disposition(
        self,
    ) -> FoundationDeclaredRootDispositionV1 {
        self.disposition
    }
}

pub(crate) struct FoundationOwnerUniverseCurrentnessV1 {
    identity: [u8; 32],
    owner: LegacyQuarantineOwnerDomainV3,
    universe_identity: [u8; 32],
    declaration_set_revision: u64,
    realm: [u8; 32],
    operation_attempt: [u8; 32],
    provider_implementation: [u8; 32],
    provider_revision: u64,
    currentness: [u8; 32],
    revocation_revision: u64,
}

impl FoundationOwnerUniverseCurrentnessV1 {
    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn owner(&self) -> LegacyQuarantineOwnerDomainV3 {
        self.owner
    }

    pub(crate) const fn universe_identity(&self) -> [u8; 32] {
        self.universe_identity
    }

    pub(crate) const fn declaration_set_revision(&self) -> u64 {
        self.declaration_set_revision
    }

    pub(crate) const fn realm(&self) -> [u8; 32] {
        self.realm
    }

    pub(crate) const fn operation_attempt(&self) -> [u8; 32] {
        self.operation_attempt
    }

    pub(crate) const fn provider_implementation(&self) -> [u8; 32] {
        self.provider_implementation
    }

    pub(crate) const fn provider_revision(&self) -> u64 {
        self.provider_revision
    }

    pub(crate) const fn currentness(&self) -> [u8; 32] {
        self.currentness
    }

    pub(crate) const fn revocation_revision(&self) -> u64 {
        self.revocation_revision
    }
}

pub(crate) trait OwnerUniverseFinalRecheckPortV1 {
    fn final_recheck(
        self: Box<Self>,
        expected: &FoundationOwnerUniverseCurrentnessV1,
    ) -> Result<[u8; 32], FoundationRootUniverseErrorV1>;
}

pub(crate) mod declared_root_universe_sealed {
    pub trait Sealed {}
}

pub(crate) trait DeclaredRootUniverseLeaseV1: declared_root_universe_sealed::Sealed {
    fn into_foundation_universe(
        self,
    ) -> Result<FoundationDeclaredRootUniverseFactsV1, FoundationRootUniverseErrorV1>;
}

pub(crate) struct FoundationDeclaredRootUniverseFactsV1 {
    identity: [u8; 32],
    currentness: FoundationOwnerUniverseCurrentnessV1,
    rows: Vec<FoundationDeclaredRootRowV1>,
    final_recheck: Box<dyn OwnerUniverseFinalRecheckPortV1>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl FoundationDeclaredRootUniverseFactsV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the complete owner universe binds every provider/currentness dimension"
    )]
    pub(crate) fn from_owner(
        owner: LegacyQuarantineOwnerDomainV3,
        universe_format: u64,
        declaration_set_revision: u64,
        realm: [u8; 32],
        operation_attempt: [u8; 32],
        provider_implementation: [u8; 32],
        provider_revision: u64,
        currentness: [u8; 32],
        revocation_revision: u64,
        mut rows: Vec<FoundationDeclaredRootRowV1>,
        final_recheck: Box<dyn OwnerUniverseFinalRecheckPortV1>,
    ) -> Result<Self, FoundationRootUniverseErrorV1> {
        if owner == LegacyQuarantineOwnerDomainV3::ProtectedPrimary
            || universe_format == 0
            || declaration_set_revision == 0
            || provider_revision == 0
            || revocation_revision == 0
            || [
                realm,
                operation_attempt,
                provider_implementation,
                currentness,
            ]
            .contains(&[0; 32])
            || rows.is_empty()
            || rows.iter().any(|row| {
                row.owner != owner
                    || row.operation_attempt != operation_attempt
                    || row.owner_realm != realm
                    || row.provider_revision != provider_revision
                    || row.currentness != currentness
                    || row.revocation_revision != revocation_revision
            })
        {
            return Err(FoundationRootUniverseErrorV1::InvalidUniverse);
        }
        rows.sort_by_key(|row| (row.declaration_id, row.identity));
        if rows.windows(2).any(|pair| {
            pair[0].declaration_id == pair[1].declaration_id
                || pair[0].declared_locator_commitment == pair[1].declared_locator_commitment
        }) {
            return Err(FoundationRootUniverseErrorV1::DuplicateDeclaration);
        }
        if owner == LegacyQuarantineOwnerDomainV3::Repository
            && (rows.len() != 1
                || rows[0].role != FoundationDeclaredRootRoleV1::RepositoryStore
                || !rows[0].required)
        {
            return Err(FoundationRootUniverseErrorV1::InvalidUniverse);
        }
        let universe_format_bytes = universe_format.to_be_bytes();
        let declaration_set_revision_bytes = declaration_set_revision.to_be_bytes();
        let provider_revision_bytes = provider_revision.to_be_bytes();
        let revocation_revision_bytes = revocation_revision.to_be_bytes();
        let owner_tag = [owner.tag()];
        let mut parts = vec![
            owner_tag.as_slice(),
            universe_format_bytes.as_slice(),
            declaration_set_revision_bytes.as_slice(),
            realm.as_slice(),
            operation_attempt.as_slice(),
            provider_implementation.as_slice(),
            provider_revision_bytes.as_slice(),
            currentness.as_slice(),
            revocation_revision_bytes.as_slice(),
        ];
        parts.extend(rows.iter().map(|row| row.identity.as_slice()));
        let identity = commitment(ROOT_UNIVERSE_DOMAIN_V1, &parts);
        let owner_currentness_identity = commitment(
            OWNER_CURRENTNESS_DOMAIN_V1,
            &[
                &[owner.tag()],
                &identity,
                &declaration_set_revision.to_be_bytes(),
                &realm,
                &operation_attempt,
                &provider_implementation,
                &provider_revision.to_be_bytes(),
                &currentness,
                &revocation_revision.to_be_bytes(),
            ],
        );
        Ok(Self {
            identity,
            currentness: FoundationOwnerUniverseCurrentnessV1 {
                identity: owner_currentness_identity,
                owner,
                universe_identity: identity,
                declaration_set_revision,
                realm,
                operation_attempt,
                provider_implementation,
                provider_revision,
                currentness,
                revocation_revision,
            },
            rows,
            final_recheck,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::foundation::core) fn into_parts(
        self,
    ) -> (
        [u8; 32],
        FoundationOwnerUniverseCurrentnessV1,
        Vec<FoundationDeclaredRootRowV1>,
        Box<dyn OwnerUniverseFinalRecheckPortV1>,
    ) {
        (
            self.identity,
            self.currentness,
            self.rows,
            self.final_recheck,
        )
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
pub(crate) enum FoundationRootUniverseErrorV1 {
    #[error("present root acquisition capability is invalid")]
    InvalidPresentCapability,
    #[error("declared-root absence fence is incomplete or foreign")]
    InvalidAbsenceFence,
    #[error("declared-root row is incomplete, invalid, or belongs to the wrong owner")]
    InvalidDeclarationRow,
    #[error("unsupported root row lacks stable reason or provider identity")]
    InvalidUnsupportedRow,
    #[error("declared-root universe is incomplete or currentness-incoherent")]
    InvalidUniverse,
    #[error("declared-root universe contains duplicate declaration or locator membership")]
    DuplicateDeclaration,
    #[error("required declared root is absent")]
    RequiredRootAbsent,
    #[error("unsupported production root refuses the Foundation attempt")]
    UnsupportedRoot,
    #[error("owner root-universe final currentness changed")]
    OwnerCurrentnessDrift,
}
