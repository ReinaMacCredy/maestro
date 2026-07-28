#![allow(
    dead_code,
    reason = "V8 Persistence root fact leaf awaits the bounded MainIntegration export checkpoint"
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::installation::root_universe::InstallationDeclaredRootV1;
use crate::foundation::core::root_universe::FoundationDeclaredRootRoleV1;

pub(crate) struct PersistenceRetainedStoreRootLeaseV1 {
    root: PathBuf,
    declaration_id: [u8; 32],
    declaration_revision: u64,
    role: FoundationDeclaredRootRoleV1,
    required: bool,
    declared_locator_commitment: [u8; 32],
    fence: [u8; 32],
    currentness: [u8; 32],
    revocation_revision: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl PersistenceRetainedStoreRootLeaseV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the retained Store-root fact binds every owner-currentness dimension"
    )]
    pub(super) fn acquire_present(
        root: PathBuf,
        declaration_id: [u8; 32],
        declaration_revision: u64,
        role: FoundationDeclaredRootRoleV1,
        required: bool,
        fence: [u8; 32],
        currentness: [u8; 32],
        revocation_revision: u64,
    ) -> Result<Self, PersistenceRootUniverseErrorV1> {
        if !matches!(
            role,
            FoundationDeclaredRootRoleV1::Active
                | FoundationDeclaredRootRoleV1::Inactive
                | FoundationDeclaredRootRoleV1::Snapshot
                | FoundationDeclaredRootRoleV1::Cache
                | FoundationDeclaredRootRoleV1::Archive
        ) || !root.is_absolute()
            || declaration_revision == 0
            || revocation_revision == 0
            || [declaration_id, fence, currentness].contains(&[0; 32])
        {
            return Err(PersistenceRootUniverseErrorV1::InvalidAcquisition);
        }
        let metadata = std::fs::symlink_metadata(&root)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(PersistenceRootUniverseErrorV1::InvalidAcquisition);
        }
        let declared_locator_commitment = commitment(
            b"maestro.v8.installation.declared-root-locator.v1\0",
            &[&lossless_locator(&root)?],
        );
        Ok(Self {
            root,
            declaration_id,
            declaration_revision,
            role,
            required,
            declared_locator_commitment,
            fence,
            currentness,
            revocation_revision,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn into_installation_declaration(
        self,
        expected_currentness: [u8; 32],
        expected_revocation_revision: u64,
    ) -> Result<InstallationDeclaredRootV1, PersistenceRootUniverseErrorV1> {
        if self.currentness != expected_currentness
            || self.revocation_revision != expected_revocation_revision
        {
            return Err(PersistenceRootUniverseErrorV1::CurrentnessDrift);
        }
        Ok(InstallationDeclaredRootV1::present(
            self.declaration_id,
            self.declaration_revision,
            self.role,
            self.required,
            self.declared_locator_commitment,
            self.fence,
            self.root,
        )?)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lossless_locator(path: &std::path::Path) -> Result<Vec<u8>, PersistenceRootUniverseErrorV1> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes().to_vec();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(PersistenceRootUniverseErrorV1::InvalidLocator);
    }
    Ok(bytes)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn lossless_locator(_path: &std::path::Path) -> Result<Vec<u8>, PersistenceRootUniverseErrorV1> {
    Err(PersistenceRootUniverseErrorV1::UnsupportedPlatform)
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
pub(crate) enum PersistenceRootUniverseErrorV1 {
    #[error("Persistence retained Store-root acquisition is invalid")]
    InvalidAcquisition,
    #[error("Persistence retained Store-root currentness changed")]
    CurrentnessDrift,
    #[error("Persistence retained Store-root locator is invalid")]
    InvalidLocator,
    #[error("Persistence retained Store-root acquisition is unsupported on this platform")]
    UnsupportedPlatform,
    #[error(transparent)]
    Installation(
        #[from] crate::domain::installation::root_universe::InstallationRootUniverseErrorV1,
    ),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
