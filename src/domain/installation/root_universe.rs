#![allow(
    dead_code,
    reason = "V8 Installation universe leaf awaits the bounded MainIntegration export checkpoint"
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::InstallationCensusV1;
use crate::foundation::core::legacy_quarantine::LegacyQuarantineOwnerDomainV3;
use crate::foundation::core::root_universe::{
    DeclaredRootUniverseLeaseV1, FoundationDeclaredAbsenceFenceV1, FoundationDeclaredRootRoleV1,
    FoundationDeclaredRootRowV1, FoundationDeclaredRootUniverseFactsV1,
    FoundationOwnerUniverseCurrentnessV1, FoundationPresentRootAcquisitionV1,
    FoundationRootUniverseErrorV1, OwnerUniverseFinalRecheckPortV1, declared_root_universe_sealed,
};

const INSTALLATION_UNIVERSE_FORMAT_V1: u64 = 1;
const MAX_INSTALLATION_DECLARATIONS_V1: usize = 65_536;

pub(crate) mod installation_root_provider_sealed {
    pub trait Sealed {}
}

pub(crate) trait InstallationRootUniverseProviderV1:
    installation_root_provider_sealed::Sealed
{
    fn observe_complete_universe(
        &mut self,
        census_comparison_identity: [u8; 32],
        operation_attempt: [u8; 32],
    ) -> Result<InstallationRootUniverseObservationV1, InstallationRootUniverseErrorV1>;
}

pub(crate) struct InstallationDeclaredRootUniverseLeaseV1 {
    facts: FoundationDeclaredRootUniverseFactsV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationDeclaredRootUniverseLeaseV1 {
    pub(in crate::domain::installation) fn acquire(
        census: &InstallationCensusV1,
        mut provider: Box<dyn InstallationRootUniverseProviderV1>,
        operation_attempt: [u8; 32],
    ) -> Result<Self, InstallationRootUniverseErrorV1> {
        if operation_attempt == [0; 32] {
            return Err(InstallationRootUniverseErrorV1::InvalidUniverse);
        }
        let census_comparison_identity = census.legacy_root_universe_comparison_identity_v1()?;
        let observed =
            provider.observe_complete_universe(census_comparison_identity, operation_attempt)?;
        observed.validate(operation_attempt)?;
        let mut rows = Vec::with_capacity(observed.declarations.len());
        for declaration in &observed.declarations {
            rows.push(declaration.to_foundation_row(&observed, operation_attempt)?);
        }
        let final_recheck = Box::new(InstallationUniverseFinalRecheckV1 {
            provider,
            census_comparison_identity,
            operation_attempt,
            acquisition: observed.clone(),
        });
        let facts = FoundationDeclaredRootUniverseFactsV1::from_owner(
            LegacyQuarantineOwnerDomainV3::Installation,
            INSTALLATION_UNIVERSE_FORMAT_V1,
            observed.declaration_set_revision,
            observed.realm,
            operation_attempt,
            observed.provider_implementation,
            observed.provider_revision,
            observed.currentness,
            observed.revocation_revision,
            rows,
            final_recheck,
        )?;
        Ok(Self {
            facts,
            _not_send_or_sync: PhantomData,
        })
    }
}

impl declared_root_universe_sealed::Sealed for InstallationDeclaredRootUniverseLeaseV1 {}

impl DeclaredRootUniverseLeaseV1 for InstallationDeclaredRootUniverseLeaseV1 {
    fn into_foundation_universe(
        self,
    ) -> Result<FoundationDeclaredRootUniverseFactsV1, FoundationRootUniverseErrorV1> {
        Ok(self.facts)
    }
}

#[derive(Clone)]
pub(crate) struct InstallationRootUniverseObservationV1 {
    declaration_set_revision: u64,
    realm: [u8; 32],
    provider_implementation: [u8; 32],
    provider_revision: u64,
    currentness: [u8; 32],
    revocation_revision: u64,
    declarations: Vec<InstallationDeclaredRootV1>,
}

impl InstallationRootUniverseObservationV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the owner provider snapshot binds its complete currentness tuple"
    )]
    pub(crate) fn from_owner_provider(
        declaration_set_revision: u64,
        realm: [u8; 32],
        provider_implementation: [u8; 32],
        provider_revision: u64,
        currentness: [u8; 32],
        revocation_revision: u64,
        declarations: Vec<InstallationDeclaredRootV1>,
    ) -> Result<Self, InstallationRootUniverseErrorV1> {
        let observed = Self {
            declaration_set_revision,
            realm,
            provider_implementation,
            provider_revision,
            currentness,
            revocation_revision,
            declarations,
        };
        observed.validate_non_attempt_fields()?;
        Ok(observed)
    }

    fn validate(&self, operation_attempt: [u8; 32]) -> Result<(), InstallationRootUniverseErrorV1> {
        self.validate_non_attempt_fields()?;
        if operation_attempt == [0; 32] {
            return Err(InstallationRootUniverseErrorV1::InvalidUniverse);
        }
        for declaration in &self.declarations {
            declaration.validate(self, operation_attempt)?;
        }
        Ok(())
    }

    fn validate_non_attempt_fields(&self) -> Result<(), InstallationRootUniverseErrorV1> {
        if self.declaration_set_revision == 0
            || self.provider_revision == 0
            || self.revocation_revision == 0
            || [self.realm, self.provider_implementation, self.currentness].contains(&[0; 32])
            || self.declarations.is_empty()
            || self.declarations.len() > MAX_INSTALLATION_DECLARATIONS_V1
        {
            return Err(InstallationRootUniverseErrorV1::InvalidUniverse);
        }
        let mut declarations = self.declarations.iter().collect::<Vec<_>>();
        declarations.sort_by_key(|row| row.declaration_id);
        if declarations.windows(2).any(|pair| {
            pair[0].declaration_id == pair[1].declaration_id
                || pair[0].declared_locator_commitment == pair[1].declared_locator_commitment
        }) {
            return Err(InstallationRootUniverseErrorV1::DuplicateDeclaration);
        }
        Ok(())
    }

    fn identity(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.v8.installation.owner-root-universe-observation.v1\0");
        hasher.update(self.declaration_set_revision.to_be_bytes());
        hasher.update(self.realm);
        hasher.update(self.provider_implementation);
        hasher.update(self.provider_revision.to_be_bytes());
        hasher.update(self.currentness);
        hasher.update(self.revocation_revision.to_be_bytes());
        let mut rows = self.declarations.iter().collect::<Vec<_>>();
        rows.sort_by_key(|row| row.declaration_id);
        for row in rows {
            hasher.update(row.identity());
        }
        hasher.finalize().into()
    }
}

#[derive(Clone)]
pub(crate) struct InstallationDeclaredRootV1 {
    declaration_id: [u8; 32],
    declaration_revision: u64,
    role: FoundationDeclaredRootRoleV1,
    required: bool,
    declared_locator_commitment: [u8; 32],
    fence: [u8; 32],
    disposition: InstallationDeclaredRootDispositionV1,
}

impl InstallationDeclaredRootV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the declaration row binds its complete owner-controlled tuple"
    )]
    pub(crate) fn present(
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        fence: [u8; 32],
        path: PathBuf,
    ) -> Result<Self, InstallationRootUniverseErrorV1> {
        let row = Self {
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            fence,
            disposition: InstallationDeclaredRootDispositionV1::Present(path),
        };
        row.validate_shape()?;
        Ok(row)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the declaration row binds its complete owner-controlled tuple"
    )]
    pub(crate) fn declared_absent(
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        fence: [u8; 32],
        provider_id: [u8; 32],
        absence_semantics_revision: u64,
    ) -> Result<Self, InstallationRootUniverseErrorV1> {
        let row = Self {
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            fence,
            disposition: InstallationDeclaredRootDispositionV1::DeclaredAbsent {
                provider_id,
                absence_semantics_revision,
            },
        };
        row.validate_shape()?;
        Ok(row)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the declaration row binds its complete owner-controlled tuple"
    )]
    pub(crate) fn unsupported(
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        declared_locator_commitment: [u8; 32],
        fence: [u8; 32],
        reason_id: [u8; 32],
        provider_id: [u8; 32],
    ) -> Result<Self, InstallationRootUniverseErrorV1> {
        let row = Self {
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            fence,
            disposition: InstallationDeclaredRootDispositionV1::Unsupported {
                reason_id,
                provider_id,
            },
        };
        row.validate_shape()?;
        Ok(row)
    }

    fn validate(
        &self,
        observed: &InstallationRootUniverseObservationV1,
        operation_attempt: [u8; 32],
    ) -> Result<(), InstallationRootUniverseErrorV1> {
        self.validate_shape()?;
        if operation_attempt == [0; 32]
            || matches!(
                self.disposition,
                InstallationDeclaredRootDispositionV1::DeclaredAbsent { .. }
                    if self.required
            )
            || matches!(
                self.disposition,
                InstallationDeclaredRootDispositionV1::Unsupported { .. }
            )
        {
            return Err(InstallationRootUniverseErrorV1::RefusedDisposition);
        }
        if let InstallationDeclaredRootDispositionV1::Present(path) = &self.disposition {
            let metadata = std::fs::symlink_metadata(path)?;
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err(InstallationRootUniverseErrorV1::PresentRootUnavailable);
            }
            if commitment(
                b"maestro.v8.installation.declared-root-locator.v1\0",
                &[&lossless_locator(path)?],
            ) != self.declared_locator_commitment
            {
                return Err(InstallationRootUniverseErrorV1::InvalidUniverse);
            }
        }
        if observed.realm == [0; 32] {
            return Err(InstallationRootUniverseErrorV1::InvalidUniverse);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), InstallationRootUniverseErrorV1> {
        if self.declaration_id == [0; 32]
            || self.declaration_revision == 0
            || self.declared_locator_commitment == [0; 32]
            || self.fence == [0; 32]
        {
            return Err(InstallationRootUniverseErrorV1::InvalidUniverse);
        }
        match self.disposition {
            InstallationDeclaredRootDispositionV1::Present(ref path) if !path.is_absolute() => {
                Err(InstallationRootUniverseErrorV1::InvalidUniverse)
            }
            InstallationDeclaredRootDispositionV1::DeclaredAbsent {
                provider_id,
                absence_semantics_revision,
            } if provider_id == [0; 32] || absence_semantics_revision == 0 => {
                Err(InstallationRootUniverseErrorV1::InvalidUniverse)
            }
            InstallationDeclaredRootDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } if reason_id == [0; 32] || provider_id == [0; 32] => {
                Err(InstallationRootUniverseErrorV1::InvalidUniverse)
            }
            _ => Ok(()),
        }
    }

    fn to_foundation_row(
        &self,
        observed: &InstallationRootUniverseObservationV1,
        operation_attempt: [u8; 32],
    ) -> Result<FoundationDeclaredRootRowV1, InstallationRootUniverseErrorV1> {
        self.validate(observed, operation_attempt)?;
        let owner = LegacyQuarantineOwnerDomainV3::Installation;
        let row = match &self.disposition {
            InstallationDeclaredRootDispositionV1::Present(path) => {
                FoundationDeclaredRootRowV1::present(
                    owner,
                    self.declaration_id,
                    self.declaration_revision,
                    self.role,
                    self.required,
                    self.declared_locator_commitment,
                    observed.provider_revision,
                    observed.realm,
                    operation_attempt,
                    observed.currentness,
                    self.fence,
                    observed.revocation_revision,
                    FoundationPresentRootAcquisitionV1::from_owner(
                        path.clone(),
                        self.declared_locator_commitment,
                    )?,
                )?
            }
            InstallationDeclaredRootDispositionV1::DeclaredAbsent { provider_id, .. } => {
                let absence = FoundationDeclaredAbsenceFenceV1::from_owner(
                    owner,
                    self.declaration_id,
                    self.declaration_revision,
                    observed.realm,
                    operation_attempt,
                    *provider_id,
                    observed.currentness,
                    self.fence,
                    observed.revocation_revision,
                )?;
                FoundationDeclaredRootRowV1::declared_absent(
                    owner,
                    self.declaration_id,
                    self.declaration_revision,
                    self.role,
                    self.required,
                    self.declared_locator_commitment,
                    observed.provider_revision,
                    observed.realm,
                    operation_attempt,
                    observed.currentness,
                    self.fence,
                    observed.revocation_revision,
                    absence,
                )?
            }
            InstallationDeclaredRootDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } => FoundationDeclaredRootRowV1::unsupported(
                owner,
                self.declaration_id,
                self.declaration_revision,
                self.role,
                self.required,
                self.declared_locator_commitment,
                observed.provider_revision,
                observed.realm,
                operation_attempt,
                observed.currentness,
                self.fence,
                observed.revocation_revision,
                *reason_id,
                *provider_id,
            )?,
        };
        Ok(row)
    }

    fn identity(&self) -> [u8; 32] {
        let (disposition_tag, extra_a, extra_b) = match &self.disposition {
            InstallationDeclaredRootDispositionV1::Present(_) => {
                (1_u8, self.declared_locator_commitment, [0; 32])
            }
            InstallationDeclaredRootDispositionV1::DeclaredAbsent {
                provider_id,
                absence_semantics_revision,
            } => (
                2,
                *provider_id,
                commitment(
                    b"maestro.v8.installation.absence-semantics.v1\0",
                    &[&absence_semantics_revision.to_be_bytes()],
                ),
            ),
            InstallationDeclaredRootDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } => (3, *reason_id, *provider_id),
        };
        commitment(
            b"maestro.v8.installation.declared-root-row.v1\0",
            &[
                &self.declaration_id,
                &self.declaration_revision.to_be_bytes(),
                &[role_tag(self.role)],
                &[u8::from(self.required)],
                &self.declared_locator_commitment,
                &self.fence,
                &[disposition_tag],
                &extra_a,
                &extra_b,
            ],
        )
    }
}

#[derive(Clone)]
enum InstallationDeclaredRootDispositionV1 {
    Present(PathBuf),
    DeclaredAbsent {
        provider_id: [u8; 32],
        absence_semantics_revision: u64,
    },
    Unsupported {
        reason_id: [u8; 32],
        provider_id: [u8; 32],
    },
}

struct InstallationUniverseFinalRecheckV1 {
    provider: Box<dyn InstallationRootUniverseProviderV1>,
    census_comparison_identity: [u8; 32],
    operation_attempt: [u8; 32],
    acquisition: InstallationRootUniverseObservationV1,
}

impl OwnerUniverseFinalRecheckPortV1 for InstallationUniverseFinalRecheckV1 {
    fn final_recheck(
        mut self: Box<Self>,
        expected: &FoundationOwnerUniverseCurrentnessV1,
    ) -> Result<[u8; 32], FoundationRootUniverseErrorV1> {
        let observed = self
            .provider
            .observe_complete_universe(self.census_comparison_identity, self.operation_attempt)
            .and_then(|observed| {
                observed.validate(self.operation_attempt)?;
                Ok(observed)
            })
            .map_err(|_| FoundationRootUniverseErrorV1::OwnerCurrentnessDrift)?;
        if observed.identity() != self.acquisition.identity()
            || expected.owner() != LegacyQuarantineOwnerDomainV3::Installation
            || expected.declaration_set_revision() != observed.declaration_set_revision
            || expected.realm() != observed.realm
            || expected.operation_attempt() != self.operation_attempt
            || expected.provider_implementation() != observed.provider_implementation
            || expected.provider_revision() != observed.provider_revision
            || expected.currentness() != observed.currentness
            || expected.revocation_revision() != observed.revocation_revision
        {
            return Err(FoundationRootUniverseErrorV1::OwnerCurrentnessDrift);
        }
        Ok(commitment(
            b"maestro.v8.installation.root-universe-final-currentness.v1\0",
            &[
                &expected.identity(),
                &expected.universe_identity(),
                &observed.identity(),
                &self.census_comparison_identity,
                &observed.currentness,
                &observed.revocation_revision.to_be_bytes(),
            ],
        ))
    }
}

fn role_tag(role: FoundationDeclaredRootRoleV1) -> u8 {
    match role {
        FoundationDeclaredRootRoleV1::RepositoryStore => 1,
        FoundationDeclaredRootRoleV1::Active => 2,
        FoundationDeclaredRootRoleV1::Inactive => 3,
        FoundationDeclaredRootRoleV1::Snapshot => 4,
        FoundationDeclaredRootRoleV1::Cache => 5,
        FoundationDeclaredRootRoleV1::Archive => 6,
        FoundationDeclaredRootRoleV1::Host => 7,
        FoundationDeclaredRootRoleV1::Legacy => 8,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lossless_locator(path: &std::path::Path) -> Result<Vec<u8>, InstallationRootUniverseErrorV1> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes().to_vec();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(InstallationRootUniverseErrorV1::InvalidLocator);
    }
    Ok(bytes)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn lossless_locator(_path: &std::path::Path) -> Result<Vec<u8>, InstallationRootUniverseErrorV1> {
    Err(InstallationRootUniverseErrorV1::UnsupportedPlatform)
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
pub(crate) enum InstallationRootUniverseErrorV1 {
    #[error("Installation root-universe provider returned an incomplete or invalid universe")]
    InvalidUniverse,
    #[error("Installation root-universe contains duplicate declarations or locators")]
    DuplicateDeclaration,
    #[error("Installation root disposition is required-absent or unsupported")]
    RefusedDisposition,
    #[error("Installation present root is unavailable, unreadable, or not a directory")]
    PresentRootUnavailable,
    #[error("Installation root locator is not losslessly representable")]
    InvalidLocator,
    #[error("Installation root-universe acquisition is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("Installation census comparison provenance is invalid")]
    InvalidCensus,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Census(#[from] super::InstallationCensusErrorV1),
    #[error(transparent)]
    Foundation(#[from] FoundationRootUniverseErrorV1),
}
