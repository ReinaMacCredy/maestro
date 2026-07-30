#![allow(
    dead_code,
    reason = "V8 freezes the Integration-owned host-slot snapshot before Installation composition"
)]

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::installation::root_universe::{
    InstallationDeclaredRootV1, InstallationRootUniverseErrorV1,
};
use crate::foundation::core::root_universe::FoundationDeclaredRootRoleV1;

const MAX_HOST_SLOT_DECLARATIONS_V1: usize = 65_536;

pub(in crate::domain) enum AuthenticatedLegacyHostSlotDispositionV1<'slot> {
    Present(&'slot Path),
    DeclaredAbsent {
        provider_id: [u8; 32],
        absence_semantics_revision: u64,
    },
    Unsupported {
        reason_id: [u8; 32],
        provider_id: [u8; 32],
    },
}

pub(in crate::domain) trait AuthenticatedLegacyHostSlotRowV1 {
    fn declaration_id(&self) -> [u8; 32];
    fn declaration_revision(&self) -> u64;
    fn required(&self) -> bool;
    fn declared_locator_commitment(&self) -> [u8; 32];
    fn fence(&self) -> [u8; 32];
    fn disposition(&self) -> AuthenticatedLegacyHostSlotDispositionV1<'_>;
}

pub(in crate::domain) trait AuthenticatedLegacyHostSlotSnapshotV1 {
    fn declaration_set_revision(&self) -> u64;
    fn realm(&self) -> [u8; 32];
    fn provider_implementation(&self) -> [u8; 32];
    fn provider_revision(&self) -> u64;
    fn currentness(&self) -> [u8; 32];
    fn revocation_revision(&self) -> u64;
    fn visit_complete_slots(
        &self,
        inspect: &mut dyn FnMut(&dyn AuthenticatedLegacyHostSlotRowV1) -> bool,
    ) -> bool;
}

pub(in crate::domain) trait LiveAuthenticatedLegacyHostSlotRegistryV1 {
    fn capture_complete_snapshot_no_io(
        &mut self,
        census_comparison_identity: [u8; 32],
        operation_attempt: [u8; 32],
        inspect: &mut dyn FnMut(&dyn AuthenticatedLegacyHostSlotSnapshotV1) -> bool,
    ) -> bool;

    fn recheck_complete_snapshot_no_io(
        &mut self,
        census_comparison_identity: [u8; 32],
        operation_attempt: [u8; 32],
        inspect: &mut dyn FnMut(&dyn AuthenticatedLegacyHostSlotSnapshotV1) -> bool,
    ) -> bool;
}

pub(in crate::domain) struct IntegrationLegacyHostSlotSnapshotProviderV1<'registry> {
    registry: &'registry mut dyn LiveAuthenticatedLegacyHostSlotRegistryV1,
    acquisition: Option<BoundLegacyHostSlotSnapshotV1>,
    census_comparison_identity: Option<[u8; 32]>,
    operation_attempt: Option<[u8; 32]>,
    final_recheck_consumed: bool,
    _not_send_or_sync: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl<'registry> IntegrationLegacyHostSlotSnapshotProviderV1<'registry> {
    pub(in crate::domain) fn acquire_from_designated_registry(
        registry: &'registry mut dyn LiveAuthenticatedLegacyHostSlotRegistryV1,
    ) -> Self {
        Self {
            registry,
            acquisition: None,
            census_comparison_identity: None,
            operation_attempt: None,
            final_recheck_consumed: false,
            _not_send_or_sync: std::marker::PhantomData,
        }
    }

    pub(in crate::domain) fn observe_complete_host_slots(
        &mut self,
        census_comparison_identity: [u8; 32],
        operation_attempt: [u8; 32],
    ) -> Result<IntegrationLegacyHostSlotSnapshotV1, IntegrationLegacyHostSlotSnapshotErrorV1> {
        if census_comparison_identity == [0; 32]
            || operation_attempt == [0; 32]
            || self.final_recheck_consumed
        {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSnapshot);
        }
        if self
            .census_comparison_identity
            .is_some_and(|expected| expected != census_comparison_identity)
            || self
                .operation_attempt
                .is_some_and(|expected| expected != operation_attempt)
        {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::CurrentnessDrift);
        }

        let final_recheck = self.acquisition.is_some();
        if final_recheck {
            self.final_recheck_consumed = true;
        }
        let mut callback_count = 0_u8;
        let mut captured = None;
        let mut capture_failed = false;
        let mut inspect = |snapshot: &dyn AuthenticatedLegacyHostSlotSnapshotV1| {
            callback_count = callback_count.saturating_add(1);
            if callback_count != 1 {
                capture_failed = true;
                return false;
            }
            match BoundLegacyHostSlotSnapshotV1::capture(snapshot) {
                Ok(snapshot) => {
                    captured = Some(snapshot);
                    true
                }
                Err(_) => {
                    capture_failed = true;
                    false
                }
            }
        };
        let accepted = if final_recheck {
            self.registry.recheck_complete_snapshot_no_io(
                census_comparison_identity,
                operation_attempt,
                &mut inspect,
            )
        } else {
            self.registry.capture_complete_snapshot_no_io(
                census_comparison_identity,
                operation_attempt,
                &mut inspect,
            )
        };
        if !accepted || capture_failed || callback_count != 1 {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::RegistryRefused);
        }
        let observed = captured.ok_or(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSnapshot)?;

        if let Some(acquisition) = &self.acquisition {
            if acquisition != &observed {
                return Err(IntegrationLegacyHostSlotSnapshotErrorV1::CurrentnessDrift);
            }
        } else {
            self.census_comparison_identity = Some(census_comparison_identity);
            self.operation_attempt = Some(operation_attempt);
            self.acquisition = Some(observed.clone());
        }

        Ok(observed.into_snapshot())
    }
}

pub(in crate::domain) struct IntegrationLegacyHostSlotSnapshotV1 {
    declaration_set_revision: u64,
    realm: [u8; 32],
    provider_implementation: [u8; 32],
    provider_revision: u64,
    currentness: [u8; 32],
    revocation_revision: u64,
    declarations: Vec<BoundLegacyHostSlotRowV1>,
}

impl IntegrationLegacyHostSlotSnapshotV1 {
    pub(in crate::domain) const fn declaration_set_revision(&self) -> u64 {
        self.declaration_set_revision
    }

    pub(in crate::domain) const fn realm(&self) -> [u8; 32] {
        self.realm
    }

    pub(in crate::domain) const fn provider_implementation(&self) -> [u8; 32] {
        self.provider_implementation
    }

    pub(in crate::domain) const fn provider_revision(&self) -> u64 {
        self.provider_revision
    }

    pub(in crate::domain) const fn currentness(&self) -> [u8; 32] {
        self.currentness
    }

    pub(in crate::domain) const fn revocation_revision(&self) -> u64 {
        self.revocation_revision
    }

    pub(in crate::domain) fn identity(&self) -> [u8; 32] {
        bound_snapshot_identity(
            self.declaration_set_revision,
            self.realm,
            self.provider_implementation,
            self.provider_revision,
            self.currentness,
            self.revocation_revision,
            &self.declarations,
        )
    }

    pub(in crate::domain) fn into_installation_declarations(
        self,
    ) -> Result<Vec<InstallationDeclaredRootV1>, IntegrationLegacyHostSlotSnapshotErrorV1> {
        self.declarations
            .into_iter()
            .map(BoundLegacyHostSlotRowV1::into_installation_declaration)
            .collect()
    }
}

#[derive(Clone, Eq, PartialEq)]
struct BoundLegacyHostSlotSnapshotV1 {
    declaration_set_revision: u64,
    realm: [u8; 32],
    provider_implementation: [u8; 32],
    provider_revision: u64,
    currentness: [u8; 32],
    revocation_revision: u64,
    declarations: Vec<BoundLegacyHostSlotRowV1>,
}

impl BoundLegacyHostSlotSnapshotV1 {
    fn capture(
        snapshot: &dyn AuthenticatedLegacyHostSlotSnapshotV1,
    ) -> Result<Self, IntegrationLegacyHostSlotSnapshotErrorV1> {
        let mut declarations = Vec::new();
        let mut capture_failed = false;
        let complete = snapshot.visit_complete_slots(&mut |row| {
            if declarations.len() >= MAX_HOST_SLOT_DECLARATIONS_V1 {
                capture_failed = true;
                return false;
            }
            match BoundLegacyHostSlotRowV1::capture(row) {
                Ok(row) => {
                    declarations.push(row);
                    true
                }
                Err(_) => {
                    capture_failed = true;
                    false
                }
            }
        });
        if !complete || capture_failed {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSnapshot);
        }
        declarations.sort_by_key(|row| row.declaration_id);
        if declarations.windows(2).any(|pair| {
            pair[0].declaration_id == pair[1].declaration_id
                || pair[0].declared_locator_commitment == pair[1].declared_locator_commitment
        }) {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::DuplicateSlot);
        }
        let captured = Self {
            declaration_set_revision: snapshot.declaration_set_revision(),
            realm: snapshot.realm(),
            provider_implementation: snapshot.provider_implementation(),
            provider_revision: snapshot.provider_revision(),
            currentness: snapshot.currentness(),
            revocation_revision: snapshot.revocation_revision(),
            declarations,
        };
        if captured.declaration_set_revision == 0
            || captured.provider_revision == 0
            || captured.revocation_revision == 0
            || [
                captured.realm,
                captured.provider_implementation,
                captured.currentness,
            ]
            .contains(&[0; 32])
        {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSnapshot);
        }
        Ok(captured)
    }

    fn into_snapshot(self) -> IntegrationLegacyHostSlotSnapshotV1 {
        IntegrationLegacyHostSlotSnapshotV1 {
            declaration_set_revision: self.declaration_set_revision,
            realm: self.realm,
            provider_implementation: self.provider_implementation,
            provider_revision: self.provider_revision,
            currentness: self.currentness,
            revocation_revision: self.revocation_revision,
            declarations: self.declarations,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct BoundLegacyHostSlotRowV1 {
    declaration_id: [u8; 32],
    declaration_revision: u64,
    required: bool,
    declared_locator_commitment: [u8; 32],
    fence: [u8; 32],
    disposition: BoundLegacyHostSlotDispositionV1,
}

impl BoundLegacyHostSlotRowV1 {
    fn capture(
        row: &dyn AuthenticatedLegacyHostSlotRowV1,
    ) -> Result<Self, IntegrationLegacyHostSlotSnapshotErrorV1> {
        let disposition = match row.disposition() {
            AuthenticatedLegacyHostSlotDispositionV1::Present(path) => {
                if !path.is_absolute()
                    || declared_locator_commitment(path)? != row.declared_locator_commitment()
                {
                    return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSlot);
                }
                BoundLegacyHostSlotDispositionV1::Present(path.to_path_buf())
            }
            AuthenticatedLegacyHostSlotDispositionV1::DeclaredAbsent {
                provider_id,
                absence_semantics_revision,
            } => {
                if row.required() || provider_id == [0; 32] || absence_semantics_revision == 0 {
                    return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSlot);
                }
                BoundLegacyHostSlotDispositionV1::DeclaredAbsent {
                    provider_id,
                    absence_semantics_revision,
                }
            }
            AuthenticatedLegacyHostSlotDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } => {
                if reason_id == [0; 32] || provider_id == [0; 32] {
                    return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSlot);
                }
                BoundLegacyHostSlotDispositionV1::Unsupported {
                    reason_id,
                    provider_id,
                }
            }
        };
        let captured = Self {
            declaration_id: row.declaration_id(),
            declaration_revision: row.declaration_revision(),
            required: row.required(),
            declared_locator_commitment: row.declared_locator_commitment(),
            fence: row.fence(),
            disposition,
        };
        if captured.declaration_id == [0; 32]
            || captured.declaration_revision == 0
            || captured.declared_locator_commitment == [0; 32]
            || captured.fence == [0; 32]
        {
            return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSlot);
        }
        Ok(captured)
    }

    fn into_installation_declaration(
        self,
    ) -> Result<InstallationDeclaredRootV1, IntegrationLegacyHostSlotSnapshotErrorV1> {
        let role = FoundationDeclaredRootRoleV1::Host;
        Ok(match self.disposition {
            BoundLegacyHostSlotDispositionV1::Present(path) => InstallationDeclaredRootV1::present(
                self.declaration_id,
                self.declaration_revision,
                role,
                self.required,
                self.declared_locator_commitment,
                self.fence,
                path,
            )?,
            BoundLegacyHostSlotDispositionV1::DeclaredAbsent {
                provider_id,
                absence_semantics_revision,
            } => InstallationDeclaredRootV1::declared_absent(
                self.declaration_id,
                self.declaration_revision,
                role,
                self.required,
                self.declared_locator_commitment,
                self.fence,
                provider_id,
                absence_semantics_revision,
            )?,
            BoundLegacyHostSlotDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } => InstallationDeclaredRootV1::unsupported(
                self.declaration_id,
                self.declaration_revision,
                role,
                self.required,
                self.declared_locator_commitment,
                self.fence,
                reason_id,
                provider_id,
            )?,
        })
    }

    fn identity(&self) -> [u8; 32] {
        let (disposition_tag, extra_a, extra_b) = match self.disposition {
            BoundLegacyHostSlotDispositionV1::Present(_) => (1_u8, [0; 32], [0; 32]),
            BoundLegacyHostSlotDispositionV1::DeclaredAbsent {
                provider_id,
                absence_semantics_revision,
            } => (
                2,
                provider_id,
                commitment(
                    b"maestro.v8.integration.host-slot-absence-semantics.v1\0",
                    &[&absence_semantics_revision.to_be_bytes()],
                ),
            ),
            BoundLegacyHostSlotDispositionV1::Unsupported {
                reason_id,
                provider_id,
            } => (3, reason_id, provider_id),
        };
        commitment(
            b"maestro.v8.integration.host-slot-row.v1\0",
            &[
                &self.declaration_id,
                &self.declaration_revision.to_be_bytes(),
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

#[derive(Clone, Eq, PartialEq)]
enum BoundLegacyHostSlotDispositionV1 {
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

fn bound_snapshot_identity(
    declaration_set_revision: u64,
    realm: [u8; 32],
    provider_implementation: [u8; 32],
    provider_revision: u64,
    currentness: [u8; 32],
    revocation_revision: u64,
    declarations: &[BoundLegacyHostSlotRowV1],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"maestro.v8.integration.complete-host-slot-snapshot.v1\0");
    hasher.update(declaration_set_revision.to_be_bytes());
    hasher.update(realm);
    hasher.update(provider_implementation);
    hasher.update(provider_revision.to_be_bytes());
    hasher.update(currentness);
    hasher.update(revocation_revision.to_be_bytes());
    for declaration in declarations {
        hasher.update(declaration.identity());
    }
    hasher.finalize().into()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn declared_locator_commitment(
    path: &Path,
) -> Result<[u8; 32], IntegrationLegacyHostSlotSnapshotErrorV1> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(IntegrationLegacyHostSlotSnapshotErrorV1::InvalidSlot);
    }
    Ok(commitment(
        b"maestro.v8.installation.declared-root-locator.v1\0",
        &[bytes],
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn declared_locator_commitment(
    _path: &Path,
) -> Result<[u8; 32], IntegrationLegacyHostSlotSnapshotErrorV1> {
    Err(IntegrationLegacyHostSlotSnapshotErrorV1::UnsupportedPlatform)
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
pub(in crate::domain) enum IntegrationLegacyHostSlotSnapshotErrorV1 {
    #[error("Integration host-slot registry refused the snapshot")]
    RegistryRefused,
    #[error("Integration host-slot snapshot is incomplete or invalid")]
    InvalidSnapshot,
    #[error("Integration host-slot declaration is invalid")]
    InvalidSlot,
    #[error("Integration host-slot declaration set contains a duplicate")]
    DuplicateSlot,
    #[error("Integration host-slot registry currentness changed")]
    CurrentnessDrift,
    #[error("Integration host-slot locators are unsupported on this platform")]
    UnsupportedPlatform,
    #[error(transparent)]
    Installation(#[from] InstallationRootUniverseErrorV1),
}
