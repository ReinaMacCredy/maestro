#![allow(
    dead_code,
    reason = "Stage 11 Persistence custody leaf awaits MainIntegration wiring"
)]

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{StoreRoleV1, StoreStateV1, StoreV1};
use crate::foundation::core::legacy_quarantine::{
    FoundationCustodyCopyReceiptV1, FoundationLegacyQuarantineErrorV1,
    LegacyQuarantineExpectedSourceSetV3, LegacyQuarantineOwnerDomainV3,
    LegacyQuarantinePhysicalFactsV1, ProtectedPrimaryBoundaryPortV1, QuarantineCustodyPortV1,
    observe_physical_facts_v1, persistence_lease_sealed,
};
use crate::foundation::core::secure_fs::{
    CreateIfAbsent, DescriptorCensusLimitsV1, DescriptorCensusObjectKindV1, InventoryRowV1,
    RetainedDescriptorCensusLeaseV3, SecureRoot,
};

pub(crate) struct ProtectedPrimaryBoundaryLeaseV1 {
    root: std::path::PathBuf,
    facts: LegacyQuarantinePhysicalFactsV1,
    identity: [u8; 32],
    realm_identity: [u8; 32],
    currentness: [u8; 32],
    fence: [u8; 32],
    revocation_revision: u64,
    expected_sources: LegacyQuarantineExpectedSourceSetV3,
    retained_sources: Option<RetainedDescriptorCensusLeaseV3>,
    retained_limits: Option<DescriptorCensusLimitsV1>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ProtectedPrimaryBoundaryLeaseV1 {
    pub(crate) fn acquire_from_live_backend(
        root: impl AsRef<Path>,
        realm_identity: [u8; 32],
        currentness: [u8; 32],
        revocation_revision: u64,
        expected_sources: LegacyQuarantineExpectedSourceSetV3,
    ) -> Result<Self, PersistenceLegacyQuarantineErrorV1> {
        if realm_identity == [0; 32] || currentness == [0; 32] || revocation_revision == 0 {
            return Err(PersistenceLegacyQuarantineErrorV1::InvalidCurrentness);
        }
        let root = root.as_ref().to_path_buf();
        let facts = observe_physical_facts_v1(&root)?;
        if !expected_sources.binds_owner_roots(
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
            &[facts.resolved_locator_commitment()],
        ) {
            return Err(PersistenceLegacyQuarantineErrorV1::InvalidExpectedSources);
        }
        let fence = commitment(
            b"maestro.persistence.protected-primary.fence.v1\0",
            &[
                &facts.fence_identity(),
                &currentness,
                &revocation_revision.to_be_bytes(),
            ],
        );
        let identity = commitment(
            b"maestro.persistence.protected-primary-boundary-lease.v1\0",
            &[
                facts.display_locator(),
                &facts.resolved_locator_commitment(),
                &facts.object_identity(),
                &facts.mount_identity(),
                &facts.provider_identity(),
                &facts.anchor_identity(),
                &realm_identity,
                &currentness,
                &fence,
                &revocation_revision.to_be_bytes(),
                &expected_sources.identity(),
            ],
        );
        Ok(Self {
            root,
            facts,
            identity,
            realm_identity,
            currentness,
            fence,
            revocation_revision,
            expected_sources,
            retained_sources: None,
            retained_limits: None,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(super) fn history_boundary_binding_v1(
        &self,
    ) -> super::legacy_source_history::ProtectedPrimaryHistoryBoundaryBindingV1 {
        super::legacy_source_history::ProtectedPrimaryHistoryBoundaryBindingV1::from_boundary(
            self.root.clone(),
            self.identity,
            self.facts.resolved_locator_commitment(),
            self.facts.object_identity(),
            self.facts.provider_identity(),
            self.facts.mount_identity(),
            self.facts.anchor_identity(),
            self.realm_identity,
            self.currentness,
            self.fence,
            self.revocation_revision,
        )
        .expect("invariant: the live protected-primary lease has a complete history binding")
    }
}

impl persistence_lease_sealed::Sealed for ProtectedPrimaryBoundaryLeaseV1 {}

impl ProtectedPrimaryBoundaryPortV1 for ProtectedPrimaryBoundaryLeaseV1 {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }

    fn display_locator(&self) -> &[u8] {
        self.facts.display_locator()
    }

    fn resolved_locator_commitment(&self) -> [u8; 32] {
        self.facts.resolved_locator_commitment()
    }

    fn object_identity(&self) -> [u8; 32] {
        self.facts.object_identity()
    }

    fn mount_identity(&self) -> [u8; 32] {
        self.facts.mount_identity()
    }

    fn provider_identity(&self) -> [u8; 32] {
        self.facts.provider_identity()
    }

    fn anchor_identity(&self) -> [u8; 32] {
        self.facts.anchor_identity()
    }

    fn realm_identity(&self) -> [u8; 32] {
        self.realm_identity
    }

    fn currentness(&self) -> [u8; 32] {
        self.currentness
    }

    fn fence(&self) -> [u8; 32] {
        self.fence
    }

    fn revocation_revision(&self) -> u64 {
        self.revocation_revision
    }

    fn expected_sources(&self) -> &LegacyQuarantineExpectedSourceSetV3 {
        &self.expected_sources
    }

    fn retain_source_census(
        &mut self,
        limits: DescriptorCensusLimitsV1,
    ) -> Result<(), FoundationLegacyQuarantineErrorV1> {
        if self.retained_sources.is_some() {
            return Err(FoundationLegacyQuarantineErrorV1::SourceChanged);
        }
        self.retained_sources =
            Some(SecureRoot::open(&self.root)?.retain_descriptor_census_root_v3(limits)?);
        self.retained_limits = Some(limits);
        Ok(())
    }

    fn source_rows(&self) -> Result<&[InventoryRowV1], FoundationLegacyQuarantineErrorV1> {
        self.retained_sources
            .as_ref()
            .map(|lease| lease.census().rows())
            .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)
    }

    fn read_source(
        &self,
        relative_locator: &[u8],
        kind: DescriptorCensusObjectKindV1,
    ) -> Result<Vec<u8>, FoundationLegacyQuarantineErrorV1> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use std::os::unix::ffi::OsStrExt;

            let path = Path::new(std::ffi::OsStr::from_bytes(relative_locator));
            Ok(self
                .retained_sources
                .as_ref()
                .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?
                .read_immutable(path, kind)?)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (relative_locator, kind);
            Err(FoundationLegacyQuarantineErrorV1::UnsupportedPlatform)
        }
    }

    fn final_recheck(self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
        let observed = observe_physical_facts_v1(&self.root)?;
        if observed != self.facts {
            return Err(FoundationLegacyQuarantineErrorV1::SourceChanged);
        }
        self.retained_sources
            .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?
            .consume_final_recheck(
                self.retained_limits
                    .ok_or(FoundationLegacyQuarantineErrorV1::SourceChanged)?,
            )?;
        Ok(commitment(
            b"maestro.persistence.protected-primary-final-recheck.v1\0",
            &[
                &self.identity,
                &self.currentness,
                &self.fence,
                &self.revocation_revision.to_be_bytes(),
            ],
        ))
    }
}

pub(crate) struct QuarantineCustodyLeaseV1<'store> {
    store: &'store StoreV1,
    retained_root: SecureRoot,
    facts: LegacyQuarantinePhysicalFactsV1,
    identity: [u8; 32],
    manager_realm_identity: [u8; 32],
    security_realm_identity: [u8; 32],
    expected_old: [u8; 32],
    currentness: [u8; 32],
    fence: [u8; 32],
    state_revision: u64,
    revocation_revision: u64,
    custody_files: Vec<(PathBuf, Vec<u8>)>,
    created_files: Vec<(PathBuf, Vec<u8>)>,
    custody_records: Vec<[u8; 32]>,
    sealed: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'store> QuarantineCustodyLeaseV1<'store> {
    pub(crate) fn acquire_from_inactive_store(
        store: &'store StoreV1,
        manager_realm_identity: [u8; 32],
        security_realm_identity: [u8; 32],
        revocation_revision: u64,
    ) -> Result<Self, PersistenceLegacyQuarantineErrorV1> {
        if store.role() != StoreRoleV1::Installation
            || manager_realm_identity == [0; 32]
            || security_realm_identity == [0; 32]
            || revocation_revision == 0
        {
            return Err(PersistenceLegacyQuarantineErrorV1::InvalidCustodyStore);
        }
        let (state, state_revision) = store.state()?;
        if state != StoreStateV1::Inactive || store.active_head()?.is_some() {
            return Err(PersistenceLegacyQuarantineErrorV1::CustodyStoreActive);
        }
        let retained_root = SecureRoot::open(store.legacy_quarantine_root_path_v3())
            .map_err(FoundationLegacyQuarantineErrorV1::from)?;
        let facts = observe_physical_facts_v1(store.legacy_quarantine_root_path_v3())?;
        let expected_old = commitment(
            b"maestro.persistence.quarantine-custody.expected-old.v1\0",
            &[
                store.domain().id().as_bytes(),
                &state_revision.to_be_bytes(),
                &facts.object_identity(),
                &facts.resolved_locator_commitment(),
            ],
        );
        let currentness = commitment(
            b"maestro.persistence.quarantine-custody.currentness.v1\0",
            &[
                &expected_old,
                &manager_realm_identity,
                &security_realm_identity,
                &revocation_revision.to_be_bytes(),
            ],
        );
        let fence = commitment(
            b"maestro.persistence.quarantine-custody.fence.v1\0",
            &[&facts.fence_identity(), &currentness],
        );
        let identity = commitment(
            b"maestro.persistence.quarantine-custody-lease.v1\0",
            &[
                facts.display_locator(),
                &facts.resolved_locator_commitment(),
                &facts.object_identity(),
                &facts.mount_identity(),
                &facts.provider_identity(),
                &facts.anchor_identity(),
                &manager_realm_identity,
                &security_realm_identity,
                &expected_old,
                &currentness,
                &fence,
                &revocation_revision.to_be_bytes(),
            ],
        );
        Ok(Self {
            store,
            retained_root,
            facts,
            identity,
            manager_realm_identity,
            security_realm_identity,
            expected_old,
            currentness,
            fence,
            state_revision,
            revocation_revision,
            custody_files: Vec::new(),
            created_files: Vec::new(),
            custody_records: Vec::new(),
            sealed: false,
            _not_send_or_sync: PhantomData,
        })
    }
}

impl persistence_lease_sealed::Sealed for QuarantineCustodyLeaseV1<'_> {}

impl QuarantineCustodyPortV1 for QuarantineCustodyLeaseV1<'_> {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }

    fn display_locator(&self) -> &[u8] {
        self.facts.display_locator()
    }

    fn resolved_locator_commitment(&self) -> [u8; 32] {
        self.facts.resolved_locator_commitment()
    }

    fn object_identity(&self) -> [u8; 32] {
        self.facts.object_identity()
    }

    fn mount_identity(&self) -> [u8; 32] {
        self.facts.mount_identity()
    }

    fn provider_identity(&self) -> [u8; 32] {
        self.facts.provider_identity()
    }

    fn anchor_identity(&self) -> [u8; 32] {
        self.facts.anchor_identity()
    }

    fn manager_realm_identity(&self) -> [u8; 32] {
        self.manager_realm_identity
    }

    fn security_realm_identity(&self) -> [u8; 32] {
        self.security_realm_identity
    }

    fn expected_old(&self) -> [u8; 32] {
        self.expected_old
    }

    fn currentness(&self) -> [u8; 32] {
        self.currentness
    }

    fn fence(&self) -> [u8; 32] {
        self.fence
    }

    fn revocation_revision(&self) -> u64 {
        self.revocation_revision
    }

    fn persist_source(
        &mut self,
        source_token: [u8; 32],
        object_identity: [u8; 32],
        kind: DescriptorCensusObjectKindV1,
        bytes: &[u8],
    ) -> Result<FoundationCustodyCopyReceiptV1, FoundationLegacyQuarantineErrorV1> {
        if source_token == [0; 32] || object_identity == [0; 32] {
            return Err(FoundationLegacyQuarantineErrorV1::CustodyWriteFailed);
        }
        let copied_length = u64::try_from(bytes.len())
            .map_err(|_| FoundationLegacyQuarantineErrorV1::CustodyWriteFailed)?;
        let copied_sha256: [u8; 32] = Sha256::digest(bytes).into();
        let record = custody_record_bytes(
            source_token,
            object_identity,
            kind,
            copied_length,
            copied_sha256,
        );
        let identity: [u8; 32] = Sha256::digest(&record).into();
        if self.custody_records.contains(&identity) {
            return Err(FoundationLegacyQuarantineErrorV1::CustodyWriteFailed);
        }
        self.store
            .legacy_quarantine_secure_root_v3()
            .create_dir_all("legacy-quarantine-v3/custody")?;
        let stem = hex_digest(source_token);
        let payload_path = PathBuf::from(format!("legacy-quarantine-v3/custody/{stem}.payload"));
        let record_path = PathBuf::from(format!("legacy-quarantine-v3/custody/{stem}.record"));
        self.create_or_verify(&payload_path, bytes)?;
        self.create_or_verify(&record_path, &record)?;
        self.store
            .legacy_quarantine_secure_root_v3()
            .read_exact(&payload_path, bytes)?;
        self.store
            .legacy_quarantine_secure_root_v3()
            .read_exact(&record_path, &record)?;
        self.custody_records.push(identity);
        FoundationCustodyCopyReceiptV1::from_persistence(identity, copied_length, copied_sha256)
    }

    fn rollback_partial(mut self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
        let rollback = self.rollback_created_files()?;
        self.sealed = true;
        Ok(rollback)
    }

    fn seal_expected_old(
        mut self,
        candidate_manifest: [u8; 32],
        custody_records: &[[u8; 32]],
    ) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
        if candidate_manifest == [0; 32] || custody_records != self.custody_records {
            return Err(FoundationLegacyQuarantineErrorV1::PartialCopy);
        }
        let (state, revision) = self
            .store
            .state()
            .map_err(|_| FoundationLegacyQuarantineErrorV1::SourceChanged)?;
        self.retained_root.verify_path_binding()?;
        let observed = observe_physical_facts_v1(self.store.legacy_quarantine_root_path_v3())?;
        if state != StoreStateV1::Inactive
            || revision != self.state_revision
            || self
                .store
                .active_head()
                .map_err(|_| FoundationLegacyQuarantineErrorV1::SourceChanged)?
                .is_some()
            || observed.display_locator() != self.facts.display_locator()
            || observed.mount_identity() != self.facts.mount_identity()
            || observed.provider_identity() != self.facts.provider_identity()
        {
            return Err(FoundationLegacyQuarantineErrorV1::SourceChanged);
        }
        for (path, bytes) in &self.custody_files {
            self.store
                .legacy_quarantine_secure_root_v3()
                .read_exact(path, bytes)?;
        }
        let records_commitment = commitment(
            b"maestro.persistence.quarantine-custody-record-set.v1\0",
            &custody_records
                .iter()
                .map(<[u8; 32]>::as_slice)
                .collect::<Vec<_>>(),
        );
        let receipt = commitment(
            b"maestro.persistence.quarantine-custody-receipt.v1\0",
            &[
                &self.identity,
                &self.expected_old,
                &candidate_manifest,
                &records_commitment,
                &observed.object_identity(),
                &observed.anchor_identity(),
                &observed.fence_identity(),
                &self.currentness,
                &self.fence,
                &self.revocation_revision.to_be_bytes(),
            ],
        );
        self.sealed = true;
        Ok(receipt)
    }
}

impl QuarantineCustodyLeaseV1<'_> {
    fn create_or_verify(
        &mut self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<(), FoundationLegacyQuarantineErrorV1> {
        match self
            .store
            .legacy_quarantine_secure_root_v3()
            .create_file_if_absent(path, bytes)?
        {
            CreateIfAbsent::Created => self
                .created_files
                .push((path.to_path_buf(), bytes.to_vec())),
            CreateIfAbsent::AlreadyExists => {
                self.store
                    .legacy_quarantine_secure_root_v3()
                    .read_exact(path, bytes)?;
            }
        }
        if self
            .custody_files
            .iter()
            .any(|(existing, _)| existing == path)
        {
            return Err(FoundationLegacyQuarantineErrorV1::CustodyWriteFailed);
        }
        self.custody_files
            .push((path.to_path_buf(), bytes.to_vec()));
        Ok(())
    }

    fn rollback_created_files(&mut self) -> Result<[u8; 32], FoundationLegacyQuarantineErrorV1> {
        let mut removed = Vec::new();
        for (path, bytes) in self.created_files.iter().rev() {
            if self
                .store
                .legacy_quarantine_secure_root_v3()
                .remove_file_if_matches(path, bytes)?
            {
                removed.push(commitment(
                    b"maestro.persistence.quarantine-custody-rollback-row.v1\0",
                    &[path.as_os_str().as_encoded_bytes(), &Sha256::digest(bytes)],
                ));
            }
        }
        self.custody_files.clear();
        self.created_files.clear();
        self.custody_records.clear();
        Ok(commitment(
            b"maestro.persistence.quarantine-custody-rollback.v1\0",
            &removed.iter().map(<[u8; 32]>::as_slice).collect::<Vec<_>>(),
        ))
    }
}

impl Drop for QuarantineCustodyLeaseV1<'_> {
    fn drop(&mut self) {
        if !self.sealed {
            let _ = self.rollback_created_files();
        }
    }
}

fn custody_record_bytes(
    source_token: [u8; 32],
    object_identity: [u8; 32],
    kind: DescriptorCensusObjectKindV1,
    copied_length: u64,
    copied_sha256: [u8; 32],
) -> Vec<u8> {
    let mut record = Vec::with_capacity(137);
    record.extend_from_slice(b"maestro.persistence.quarantine-custody-record.v1\0");
    record.extend_from_slice(&source_token);
    record.extend_from_slice(&object_identity);
    record.push(match kind {
        DescriptorCensusObjectKindV1::RegularFile => 1,
        DescriptorCensusObjectKindV1::SymbolicLink => 2,
    });
    record.extend_from_slice(&copied_length.to_be_bytes());
    record.extend_from_slice(&copied_sha256);
    record
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PersistenceLegacyQuarantineReceiptV1 {
    identity: [u8; 32],
}

impl PersistenceLegacyQuarantineReceiptV1 {
    pub(crate) fn from_foundation_receipt(
        identity: [u8; 32],
    ) -> Result<Self, PersistenceLegacyQuarantineErrorV1> {
        if identity == [0; 32] {
            return Err(PersistenceLegacyQuarantineErrorV1::InvalidReceipt);
        }
        Ok(Self { identity })
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }
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
pub(crate) enum PersistenceLegacyQuarantineErrorV1 {
    #[error("protected-primary currentness is incomplete")]
    InvalidCurrentness,
    #[error("protected-primary expected source universe is invalid")]
    InvalidExpectedSources,
    #[error("quarantine custody requires a live inactive Installation Store")]
    InvalidCustodyStore,
    #[error("quarantine custody Store is active or has a live head")]
    CustodyStoreActive,
    #[error("legacy quarantine Persistence receipt is invalid")]
    InvalidReceipt,
    #[error(transparent)]
    Store(#[from] super::StoreError),
    #[error(transparent)]
    Foundation(#[from] FoundationLegacyQuarantineErrorV1),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::persistence::{StoreDomainV1, StoreRoleV1, StoreV1};
    use crate::foundation::core::legacy_quarantine::LegacyQuarantineExpectedSourceV3;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let temp_parent = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            let path = temp_parent.join(format!(
                "maestro-stage11-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test root");
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn inactive_installation_store(root: &TempRoot, label: &[u8]) -> StoreV1 {
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Installation, label).expect("Installation domain");
        StoreV1::create(&root.0, domain).expect("create inactive Installation Store")
    }

    #[test]
    fn custody_persists_exact_payload_and_record_bytes_until_expected_old_seals() {
        let root = TempRoot::new("custody-commit");
        let store = inactive_installation_store(&root, b"custody-commit");
        let mut custody =
            QuarantineCustodyLeaseV1::acquire_from_inactive_store(&store, [1; 32], [2; 32], 7)
                .expect("acquire custody");
        let source_token = [3; 32];
        let bytes = b"owner-held exact legacy bytes";
        let receipt = custody
            .persist_source(
                source_token,
                [4; 32],
                DescriptorCensusObjectKindV1::RegularFile,
                bytes,
            )
            .expect("persist source");
        assert_eq!(
            store.state().expect("state"),
            (StoreStateV1::Inactive, custody.state_revision)
        );
        assert!(store.active_head().expect("head").is_none());
        custody
            .retained_root
            .verify_path_binding()
            .expect("retained custody binding");
        let observed = observe_physical_facts_v1(store.legacy_quarantine_root_path_v3())
            .expect("physical facts");
        assert_eq!(observed.mount_identity(), custody.facts.mount_identity());
        assert_eq!(
            observed.provider_identity(),
            custody.facts.provider_identity()
        );
        let records = [receipt.identity()];
        let sealed = custody
            .seal_expected_old([5; 32], &records)
            .expect("seal expected-old custody");

        assert_ne!(sealed, [0; 32]);
        let stem = hex_digest(source_token);
        assert_eq!(
            fs::read(
                root.0
                    .join(format!("legacy-quarantine-v3/custody/{stem}.payload"))
            )
            .expect("read retained payload"),
            bytes
        );
        assert!(
            root.0
                .join(format!("legacy-quarantine-v3/custody/{stem}.record"))
                .is_file()
        );
    }

    #[test]
    fn explicit_rollback_removes_only_files_created_by_the_custody_lease() {
        let root = TempRoot::new("custody-rollback");
        let store = inactive_installation_store(&root, b"custody-rollback");
        let mut custody =
            QuarantineCustodyLeaseV1::acquire_from_inactive_store(&store, [6; 32], [7; 32], 9)
                .expect("acquire custody");
        let source_token = [8; 32];
        custody
            .persist_source(
                source_token,
                [9; 32],
                DescriptorCensusObjectKindV1::RegularFile,
                b"rollback payload",
            )
            .expect("persist source");
        let stem = hex_digest(source_token);
        let payload = root
            .0
            .join(format!("legacy-quarantine-v3/custody/{stem}.payload"));
        let record = root
            .0
            .join(format!("legacy-quarantine-v3/custody/{stem}.record"));
        assert!(payload.is_file() && record.is_file());

        let rollback = custody.rollback_partial().expect("rollback custody");

        assert_ne!(rollback, [0; 32]);
        assert!(!payload.exists());
        assert!(!record.exists());
    }

    #[test]
    fn dropped_unsealed_custody_rolls_back_created_files() {
        let root = TempRoot::new("custody-drop-rollback");
        let store = inactive_installation_store(&root, b"custody-drop-rollback");
        let source_token = [12; 32];
        {
            let mut custody = QuarantineCustodyLeaseV1::acquire_from_inactive_store(
                &store, [13; 32], [14; 32], 3,
            )
            .expect("acquire custody");
            custody
                .persist_source(
                    source_token,
                    [15; 32],
                    DescriptorCensusObjectKindV1::RegularFile,
                    b"drop rollback payload",
                )
                .expect("persist source");
        }
        let stem = hex_digest(source_token);
        assert!(
            !root
                .0
                .join(format!("legacy-quarantine-v3/custody/{stem}.payload"))
                .exists()
        );
        assert!(
            !root
                .0
                .join(format!("legacy-quarantine-v3/custody/{stem}.record"))
                .exists()
        );
    }

    #[test]
    fn final_seal_rechecks_preexisting_custody_bytes() {
        let root = TempRoot::new("custody-preexisting-recheck");
        let store = inactive_installation_store(&root, b"custody-preexisting-recheck");
        let source_token = [16; 32];
        let object_identity = [17; 32];
        let bytes = b"preexisting exact payload";
        let mut first =
            QuarantineCustodyLeaseV1::acquire_from_inactive_store(&store, [18; 32], [19; 32], 4)
                .expect("acquire first custody");
        let receipt = first
            .persist_source(
                source_token,
                object_identity,
                DescriptorCensusObjectKindV1::RegularFile,
                bytes,
            )
            .expect("persist first source");
        first
            .seal_expected_old([20; 32], &[receipt.identity()])
            .expect("seal first custody");

        let mut second =
            QuarantineCustodyLeaseV1::acquire_from_inactive_store(&store, [18; 32], [19; 32], 4)
                .expect("acquire second custody");
        let receipt = second
            .persist_source(
                source_token,
                object_identity,
                DescriptorCensusObjectKindV1::RegularFile,
                bytes,
            )
            .expect("reuse exact custody bytes");
        let stem = hex_digest(source_token);
        fs::write(
            root.0
                .join(format!("legacy-quarantine-v3/custody/{stem}.payload")),
            b"changed after reuse",
        )
        .expect("change reused payload");

        assert!(
            second
                .seal_expected_old([20; 32], &[receipt.identity()])
                .is_err()
        );
    }

    #[test]
    fn protected_primary_retains_descriptor_serviced_bytes_through_final_recheck() {
        let root = TempRoot::new("primary-retained");
        fs::write(root.0.join("legacy.txt"), b"protected primary bytes").expect("write source");
        let facts = observe_physical_facts_v1(&root.0).expect("primary facts");
        let census = SecureRoot::open(&root.0)
            .expect("open primary")
            .retain_descriptor_census_root_v3(DescriptorCensusLimitsV1::bounded_default())
            .expect("expected census");
        let expected = census.census().rows()[0].clone();
        let expected = LegacyQuarantineExpectedSourceV3::from_packet(
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
            facts.resolved_locator_commitment(),
            expected.relative_name().to_vec(),
            expected.kind(),
            expected.logical_byte_length(),
            expected.object_identity(),
            expected.content_identity(),
            [12; 32],
            None,
        )
        .expect("expected source");
        let expected = LegacyQuarantineExpectedSourceSetV3::from_packet(
            [13; 32],
            LegacyQuarantineOwnerDomainV3::ProtectedPrimary,
            vec![expected],
        )
        .expect("expected source set");
        drop(census);
        let mut primary = ProtectedPrimaryBoundaryLeaseV1::acquire_from_live_backend(
            &root.0, [10; 32], [11; 32], 5, expected,
        )
        .expect("acquire protected primary");
        primary
            .retain_source_census(DescriptorCensusLimitsV1::bounded_default())
            .expect("retain primary census");
        let row = primary
            .source_rows()
            .expect("primary rows")
            .first()
            .expect("one primary row")
            .clone();
        let bytes = primary
            .read_source(row.relative_name(), row.kind())
            .expect("descriptor-serviced read");

        assert_eq!(bytes, b"protected primary bytes");
        assert_ne!(primary.final_recheck().expect("final recheck"), [0; 32]);
    }
}
