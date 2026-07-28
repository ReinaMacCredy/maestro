#![allow(
    dead_code,
    reason = "Stage 11 Installation admission leaf awaits MainIntegration wiring"
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::InstallationCensusV1;
use crate::foundation::core::legacy_quarantine::{
    FoundationLegacyQuarantineErrorV1, LegacyQuarantineExpectedSourceSetV3,
    LegacyQuarantineOwnerDomainV3, LegacyQuarantineRootAdmissionFactsV3,
    LegacyQuarantineRootAdmissionV3, observe_root_binding_v3, owner_admission_sealed,
};

pub(crate) struct InstallationRootAdmissionV3 {
    roots: Vec<(Vec<u8>, PathBuf, [u8; 32])>,
    owner_currentness: [u8; 32],
    owner_attestation: [u8; 32],
    expected_sources: LegacyQuarantineExpectedSourceSetV3,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationRootAdmissionV3 {
    pub(crate) fn mint_from_live_registry(
        census: &InstallationCensusV1,
        expected_sources: LegacyQuarantineExpectedSourceSetV3,
    ) -> Result<Self, InstallationRootAdmissionErrorV3> {
        let snapshot = census.legacy_quarantine_root_snapshot_v3()?;
        let mut roots = Vec::with_capacity(snapshot.roots.len());
        for locator in snapshot.roots {
            let path = PathBuf::from(&locator);
            if !path.is_absolute() {
                return Err(InstallationRootAdmissionErrorV3::InvalidRegistryRoot);
            }
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err(InstallationRootAdmissionErrorV3::InvalidRegistryRoot);
            }
            roots.push((
                lossless_locator(&path)?,
                path.clone(),
                observe_root_binding_v3(&path)?,
            ));
        }
        roots.sort_by(|left, right| (&left.0, left.2).cmp(&(&right.0, right.2)));
        if roots
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 || pair[0].2 == pair[1].2)
        {
            return Err(InstallationRootAdmissionErrorV3::DuplicateRegistryRoot);
        }
        let root_bindings = roots.iter().map(|root| root.2).collect::<Vec<_>>();
        if !expected_sources
            .binds_owner_roots(LegacyQuarantineOwnerDomainV3::Installation, &root_bindings)
        {
            return Err(InstallationRootAdmissionErrorV3::InvalidExpectedSources);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.installation.legacy-quarantine.admission.v3\0");
        hasher.update(snapshot.owner_attestation);
        hasher.update(snapshot.owner_currentness);
        for (display, _, binding) in &roots {
            hasher.update((display.len() as u64).to_be_bytes());
            hasher.update(display);
            hasher.update(binding);
        }
        hasher.update(expected_sources.identity());
        let owner_attestation = hasher.finalize().into();
        Ok(Self {
            roots,
            owner_currentness: snapshot.owner_currentness,
            owner_attestation,
            expected_sources,
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

impl owner_admission_sealed::Sealed for InstallationRootAdmissionV3 {}

impl LegacyQuarantineRootAdmissionV3 for InstallationRootAdmissionV3 {
    fn into_foundation_facts(self) -> LegacyQuarantineRootAdmissionFactsV3 {
        LegacyQuarantineRootAdmissionFactsV3::from_owner(
            LegacyQuarantineOwnerDomainV3::Installation,
            self.roots,
            self.expected_sources,
            self.owner_currentness,
            self.owner_attestation,
        )
        .expect("invariant: Installation admission was validated when minted")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lossless_locator(path: &std::path::Path) -> Result<Vec<u8>, InstallationRootAdmissionErrorV3> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes().to_vec();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(InstallationRootAdmissionErrorV3::InvalidRegistryRoot);
    }
    Ok(bytes)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn lossless_locator(_path: &std::path::Path) -> Result<Vec<u8>, InstallationRootAdmissionErrorV3> {
    Err(InstallationRootAdmissionErrorV3::UnsupportedPlatform)
}

#[derive(Debug, Error)]
pub(crate) enum InstallationRootAdmissionErrorV3 {
    #[error("Installation legacy root registry is empty or contains a non-directory root")]
    InvalidRegistryRoot,
    #[error("Installation legacy root registry contains a duplicate locator or physical root")]
    DuplicateRegistryRoot,
    #[error("Installation root admission is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("Installation expected source universe is not packet-bound")]
    InvalidExpectedSources,
    #[error(transparent)]
    Census(#[from] super::InstallationCensusErrorV1),
    #[error(transparent)]
    Foundation(#[from] FoundationLegacyQuarantineErrorV1),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
