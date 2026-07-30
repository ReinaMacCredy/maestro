#![allow(
    dead_code,
    reason = "V8 Repository universe leaf awaits the bounded MainIntegration export checkpoint"
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::persistence::{StoreDomainV1, StoreRoleV1, StoreStateV1, StoreV1};
use crate::foundation::core::legacy_quarantine::LegacyQuarantineOwnerDomainV3;
use crate::foundation::core::root_universe::{
    DeclaredRootUniverseLeaseV1, FoundationDeclaredRootRoleV1, FoundationDeclaredRootRowV1,
    FoundationDeclaredRootUniverseFactsV1, FoundationOwnerUniverseCurrentnessV1,
    FoundationPresentRootAcquisitionV1, FoundationRootUniverseErrorV1,
    OwnerUniverseFinalRecheckPortV1, declared_root_universe_sealed,
};

const REPOSITORY_UNIVERSE_FORMAT_V1: u64 = 1;
const REPOSITORY_PROVIDER_REVISION_V1: u64 = 1;

pub(crate) struct RepositoryDeclaredRootUniverseLeaseV1 {
    facts: FoundationDeclaredRootUniverseFactsV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RepositoryDeclaredRootUniverseLeaseV1 {
    pub(in crate::domain::repository) fn acquire(
        store: &StoreV1,
        operation_attempt: [u8; 32],
    ) -> Result<Self, RepositoryRootUniverseErrorV1> {
        if store.role() != StoreRoleV1::Repository || operation_attempt == [0; 32] {
            return Err(RepositoryRootUniverseErrorV1::InvalidAcquisition);
        }
        let observed = RepositoryUniverseObservationV1::observe(store, operation_attempt)?;
        let acquisition = FoundationPresentRootAcquisitionV1::from_owner(
            observed.root.clone(),
            observed.declared_locator_commitment,
        )?;
        let row = FoundationDeclaredRootRowV1::present(
            LegacyQuarantineOwnerDomainV3::Repository,
            observed.declaration_id,
            observed.declaration_set_revision,
            FoundationDeclaredRootRoleV1::RepositoryStore,
            true,
            observed.declared_locator_commitment,
            REPOSITORY_PROVIDER_REVISION_V1,
            observed.realm,
            operation_attempt,
            observed.currentness,
            observed.fence,
            observed.revocation_revision,
            acquisition,
        )?;
        let final_recheck = Box::new(RepositoryUniverseFinalRecheckV1 {
            domain: store.domain().clone(),
            root: observed.root.clone(),
            acquisition: observed.clone(),
        });
        let facts = FoundationDeclaredRootUniverseFactsV1::from_owner(
            LegacyQuarantineOwnerDomainV3::Repository,
            REPOSITORY_UNIVERSE_FORMAT_V1,
            observed.declaration_set_revision,
            observed.realm,
            operation_attempt,
            observed.provider_implementation,
            REPOSITORY_PROVIDER_REVISION_V1,
            observed.currentness,
            observed.revocation_revision,
            vec![row],
            final_recheck,
        )?;
        Ok(Self {
            facts,
            _not_send_or_sync: PhantomData,
        })
    }
}

impl declared_root_universe_sealed::Sealed for RepositoryDeclaredRootUniverseLeaseV1 {}

impl DeclaredRootUniverseLeaseV1 for RepositoryDeclaredRootUniverseLeaseV1 {
    fn into_foundation_universe(
        self,
    ) -> Result<FoundationDeclaredRootUniverseFactsV1, FoundationRootUniverseErrorV1> {
        Ok(self.facts)
    }
}

#[derive(Clone)]
struct RepositoryUniverseObservationV1 {
    root: PathBuf,
    declaration_id: [u8; 32],
    declaration_set_revision: u64,
    realm: [u8; 32],
    operation_attempt: [u8; 32],
    provider_implementation: [u8; 32],
    declared_locator_commitment: [u8; 32],
    currentness: [u8; 32],
    fence: [u8; 32],
    revocation_revision: u64,
    head_id: [u8; 32],
    generation_id: [u8; 32],
    contract_root_id: [u8; 32],
}

impl RepositoryUniverseObservationV1 {
    fn observe(
        store: &StoreV1,
        operation_attempt: [u8; 32],
    ) -> Result<Self, RepositoryRootUniverseErrorV1> {
        let (state, state_revision) = store.state()?;
        let head = store
            .active_head()?
            .ok_or(RepositoryRootUniverseErrorV1::InvalidAcquisition)?;
        let generation = store.publication_generation(head.id())?;
        if state != StoreStateV1::Active
            || state_revision == 0
            || head.revision() != generation.ordinal()
            || head.generation_id() != generation.id()
            || generation.domain() != store.domain()
        {
            return Err(RepositoryRootUniverseErrorV1::InvalidAcquisition);
        }
        let root = store.legacy_quarantine_root_path_v3().to_path_buf();
        let declared_locator = lossless_locator(&root)?;
        let declared_locator_commitment = commitment(
            b"maestro.v8.repository.declared-root-locator.v1\0",
            &[&declared_locator],
        );
        let realm = *store.domain().id().as_bytes();
        let provider_implementation = commitment(
            b"maestro.v8.repository.root-universe-provider.v1\0",
            &[b"StoreV1"],
        );
        let declaration_id = commitment(
            b"maestro.v8.repository.root-declaration.v1\0",
            &[&realm, &declared_locator_commitment],
        );
        let currentness = commitment(
            b"maestro.v8.repository.root-universe-currentness.v1\0",
            &[
                &realm,
                head.id().as_bytes(),
                generation.id().as_bytes(),
                generation.contract_root_id().as_bytes(),
                &state_revision.to_be_bytes(),
                &operation_attempt,
                &provider_implementation,
                &REPOSITORY_PROVIDER_REVISION_V1.to_be_bytes(),
            ],
        );
        let fence = commitment(
            b"maestro.v8.repository.root-universe-fence.v1\0",
            &[
                &currentness,
                &declaration_id,
                &declared_locator_commitment,
                &state_revision.to_be_bytes(),
            ],
        );
        Ok(Self {
            root,
            declaration_id,
            declaration_set_revision: generation.ordinal(),
            realm,
            operation_attempt,
            provider_implementation,
            declared_locator_commitment,
            currentness,
            fence,
            revocation_revision: state_revision,
            head_id: *head.id().as_bytes(),
            generation_id: *generation.id().as_bytes(),
            contract_root_id: *generation.contract_root_id().as_bytes(),
        })
    }
}

struct RepositoryUniverseFinalRecheckV1 {
    domain: StoreDomainV1,
    root: PathBuf,
    acquisition: RepositoryUniverseObservationV1,
}

impl OwnerUniverseFinalRecheckPortV1 for RepositoryUniverseFinalRecheckV1 {
    fn final_recheck(
        self: Box<Self>,
        expected: &FoundationOwnerUniverseCurrentnessV1,
    ) -> Result<[u8; 32], FoundationRootUniverseErrorV1> {
        let store = StoreV1::open(&self.root, self.domain.clone())
            .map_err(|_| FoundationRootUniverseErrorV1::OwnerCurrentnessDrift)?;
        let observed =
            RepositoryUniverseObservationV1::observe(&store, self.acquisition.operation_attempt)
                .map_err(|_| FoundationRootUniverseErrorV1::OwnerCurrentnessDrift)?;
        if observed.declaration_id != self.acquisition.declaration_id
            || observed.declared_locator_commitment != self.acquisition.declared_locator_commitment
            || observed.head_id != self.acquisition.head_id
            || observed.generation_id != self.acquisition.generation_id
            || observed.contract_root_id != self.acquisition.contract_root_id
            || observed.currentness != self.acquisition.currentness
            || observed.fence != self.acquisition.fence
            || expected.owner() != LegacyQuarantineOwnerDomainV3::Repository
            || expected.declaration_set_revision() != observed.declaration_set_revision
            || expected.realm() != observed.realm
            || expected.operation_attempt() != observed.operation_attempt
            || expected.provider_implementation() != observed.provider_implementation
            || expected.provider_revision() != REPOSITORY_PROVIDER_REVISION_V1
            || expected.currentness() != observed.currentness
            || expected.revocation_revision() != observed.revocation_revision
        {
            return Err(FoundationRootUniverseErrorV1::OwnerCurrentnessDrift);
        }
        Ok(commitment(
            b"maestro.v8.repository.root-universe-final-currentness.v1\0",
            &[
                &expected.identity(),
                &expected.universe_identity(),
                &observed.head_id,
                &observed.generation_id,
                &observed.contract_root_id,
                &observed.currentness,
                &observed.fence,
                &observed.revocation_revision.to_be_bytes(),
            ],
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lossless_locator(path: &std::path::Path) -> Result<Vec<u8>, RepositoryRootUniverseErrorV1> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes().to_vec();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(RepositoryRootUniverseErrorV1::InvalidLocator);
    }
    Ok(bytes)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn lossless_locator(_path: &std::path::Path) -> Result<Vec<u8>, RepositoryRootUniverseErrorV1> {
    Err(RepositoryRootUniverseErrorV1::UnsupportedPlatform)
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
pub(crate) enum RepositoryRootUniverseErrorV1 {
    #[error("Repository root-universe acquisition requires one coherent active Repository Store")]
    InvalidAcquisition,
    #[error("Repository root locator is not losslessly representable")]
    InvalidLocator,
    #[error("Repository root-universe acquisition is unsupported on this platform")]
    UnsupportedPlatform,
    #[error(transparent)]
    Store(#[from] crate::domain::persistence::StoreError),
    #[error(transparent)]
    Foundation(#[from] FoundationRootUniverseErrorV1),
}
