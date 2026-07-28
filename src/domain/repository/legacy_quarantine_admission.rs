#![allow(
    dead_code,
    reason = "Stage 11 Repository admission leaf awaits MainIntegration wiring"
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::persistence::{StoreRoleV1, StoreStateV1, StoreV1};
use crate::foundation::core::legacy_quarantine::{
    FoundationLegacyQuarantineErrorV1, LegacyQuarantineOwnerDomainV3,
    LegacyQuarantineRootAdmissionFactsV3, LegacyQuarantineRootAdmissionV3, observe_root_binding_v3,
    owner_admission_sealed,
};

pub(crate) struct RepositoryRootAdmissionV3 {
    root: PathBuf,
    display_locator: Vec<u8>,
    root_binding: [u8; 32],
    owner_currentness: [u8; 32],
    owner_attestation: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RepositoryRootAdmissionV3 {
    pub(crate) fn mint_from_store(store: &StoreV1) -> Result<Self, RepositoryRootAdmissionErrorV3> {
        if store.role() != StoreRoleV1::Repository {
            return Err(RepositoryRootAdmissionErrorV3::WrongStoreRole);
        }
        let (state, state_revision) = store.state()?;
        if state != StoreStateV1::Active || state_revision == 0 {
            return Err(RepositoryRootAdmissionErrorV3::StoreNotActive);
        }
        let head = store
            .active_head()?
            .ok_or(RepositoryRootAdmissionErrorV3::MissingHead)?;
        let root = store.legacy_quarantine_root_path_v3().to_path_buf();
        let display_locator = lossless_locator(&root)?;
        let root_binding = observe_root_binding_v3(&root)?;
        let owner_currentness = commitment(
            b"maestro.repository.legacy-quarantine.currentness.v3\0",
            &[
                store.domain().id().as_bytes(),
                head.id().as_bytes(),
                &state_revision.to_be_bytes(),
                &root_binding,
            ],
        );
        let owner_attestation = commitment(
            b"maestro.repository.legacy-quarantine.admission.v3\0",
            &[
                &display_locator,
                &root_binding,
                &owner_currentness,
                head.id().as_bytes(),
            ],
        );
        Ok(Self {
            root,
            display_locator,
            root_binding,
            owner_currentness,
            owner_attestation,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.owner_attestation
    }

    pub(crate) const fn owner_currentness(&self) -> [u8; 32] {
        self.owner_currentness
    }
}

impl owner_admission_sealed::Sealed for RepositoryRootAdmissionV3 {}

impl LegacyQuarantineRootAdmissionV3 for RepositoryRootAdmissionV3 {
    fn into_foundation_facts(self) -> LegacyQuarantineRootAdmissionFactsV3 {
        LegacyQuarantineRootAdmissionFactsV3::from_owner(
            LegacyQuarantineOwnerDomainV3::Repository,
            vec![(self.display_locator, self.root, self.root_binding)],
            self.owner_currentness,
            self.owner_attestation,
        )
        .expect("invariant: Repository admission was validated when minted")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lossless_locator(path: &std::path::Path) -> Result<Vec<u8>, RepositoryRootAdmissionErrorV3> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes().to_vec();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(RepositoryRootAdmissionErrorV3::InvalidLocator);
    }
    Ok(bytes)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn lossless_locator(_path: &std::path::Path) -> Result<Vec<u8>, RepositoryRootAdmissionErrorV3> {
    Err(RepositoryRootAdmissionErrorV3::UnsupportedPlatform)
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
pub(crate) enum RepositoryRootAdmissionErrorV3 {
    #[error("RepositoryRootAdmissionV3 requires a Repository Store")]
    WrongStoreRole,
    #[error("RepositoryRootAdmissionV3 requires a live active Repository Store")]
    StoreNotActive,
    #[error("RepositoryRootAdmissionV3 requires the live Repository head")]
    MissingHead,
    #[error("Repository root locator is not losslessly representable")]
    InvalidLocator,
    #[error("Repository root admission is unsupported on this platform")]
    UnsupportedPlatform,
    #[error(transparent)]
    Store(#[from] crate::domain::persistence::StoreError),
    #[error(transparent)]
    Foundation(#[from] FoundationLegacyQuarantineErrorV1),
}
