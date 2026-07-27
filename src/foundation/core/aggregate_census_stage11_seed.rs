use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::aggregate_census::{
    AggregateCensusBackendV1, AggregateCensusBackendV2, AggregateCensusResultV1,
    AggregateComponentCensusV1, AggregateRootFactsV1, AggregateRootRoleV1, AggregateRootSetFactsV1,
    CensusInvocationV2, FoundationAdmittedRootSourceV2, InstallationRootAdmissionV2,
    RepositoryRootAdmissionV2, owner_sealed,
};
use super::secure_fs::{
    DescriptorCensusLimitsV1, InventoryRowV1, SecureFsError, SecureFsResult, SecureRoot,
};

pub(crate) struct Stage11AggregateCensusBackendSeedV1 {
    _private: (),
}

pub(super) struct Stage11AggregateCensusOutputV1<'scan> {
    result: AggregateCensusResultV1<'scan>,
}

pub(super) struct Stage11AggregateCensusComponentV1 {
    resolved_identity: [u8; 32],
    inventory: [u8; 32],
    root_binding: [u8; 32],
    rows: Vec<InventoryRowV1>,
}

impl Stage11AggregateCensusOutputV1<'_> {
    pub(super) fn into_parts(self) -> ([u8; 32], u64, u64, Vec<Stage11AggregateCensusComponentV1>) {
        let (admitted_set, entries, bytes, roots) = self.result.into_stage11_parts();
        let roots = roots
            .into_iter()
            .map(|root| Stage11AggregateCensusComponentV1 {
                resolved_identity: root.resolved_identity,
                inventory: root.inventory,
                root_binding: root.root_binding,
                rows: root.rows,
            })
            .collect();
        (admitted_set, entries, bytes, roots)
    }
}

impl Stage11AggregateCensusComponentV1 {
    pub(super) fn into_parts(self) -> ([u8; 32], [u8; 32], [u8; 32], Vec<InventoryRowV1>) {
        (
            self.resolved_identity,
            self.inventory,
            self.root_binding,
            self.rows,
        )
    }
}

impl owner_sealed::Sealed for Stage11AggregateCensusBackendSeedV1 {}

pub(crate) struct Stage11AggregateCensusBackendSeedV2 {
    source: Option<FoundationAdmittedRootSourceV2>,
    roots: BTreeMap<[u8; 32], SecureRoot>,
    initial_roots: AggregateRootSetFactsV1,
    limits: DescriptorCensusLimitsV1,
    fence_consumed: bool,
    pass_count: u8,
}

pub(crate) struct PersistenceAdmittedRootSourceV2 {
    roots: Vec<(AggregateRootFactsV1, SecureRoot)>,
    owner_currentness: [u8; 32],
}

pub(crate) struct InstallationAdmittedRootSourceV2 {
    roots: Vec<(AggregateRootFactsV1, SecureRoot)>,
    owner_currentness: [u8; 32],
}

impl Stage11AggregateCensusBackendSeedV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Foundation invocation binds the complete finite aggregate-census limit tuple"
    )]
    pub(crate) fn from_owner_sources(
        repository: PersistenceAdmittedRootSourceV2,
        installation: InstallationAdmittedRootSourceV2,
        invocation: [u8; 32],
        namespace_epoch: u64,
        maximum_entries: u64,
        maximum_bytes: u64,
        maximum_roots: u64,
        maximum_descriptors: u64,
        maximum_depth: u64,
        maximum_name_bytes: u64,
        revocation_revision: u64,
    ) -> SecureFsResult<Self> {
        let mut retained = BTreeMap::new();
        let repository_facts = retain_unique_roots(repository.roots, &mut retained)?;
        let installation_facts = retain_unique_roots(installation.roots, &mut retained)?;
        let repository = RepositoryRootAdmissionV2::from_persistence_owner(
            repository_facts,
            repository.owner_currentness,
        )?;
        let installation = InstallationRootAdmissionV2::from_installation_owner(
            installation_facts,
            installation.owner_currentness,
        )?;
        let invocation = CensusInvocationV2::from_foundation_owner(
            invocation,
            namespace_epoch,
            maximum_entries,
            maximum_bytes,
            maximum_roots,
            maximum_descriptors,
            maximum_depth,
            maximum_name_bytes,
            revocation_revision,
        )?;
        let source = FoundationAdmittedRootSourceV2::join(repository, installation, invocation)?;
        let initial_roots = source_preview(&source)?;
        let maximum_depth =
            usize::try_from(maximum_depth).map_err(|_| SecureFsError::CensusRefused)?;
        let maximum_entries =
            usize::try_from(maximum_entries).map_err(|_| SecureFsError::CensusRefused)?;
        let maximum_bytes =
            usize::try_from(maximum_bytes).map_err(|_| SecureFsError::CensusRefused)?;
        let maximum_name_bytes =
            usize::try_from(maximum_name_bytes).map_err(|_| SecureFsError::CensusRefused)?;
        let limits = DescriptorCensusLimitsV1::bounded(
            maximum_depth,
            maximum_entries,
            maximum_bytes,
            maximum_bytes,
            maximum_name_bytes,
            maximum_name_bytes,
        )
        .ok_or(SecureFsError::CensusRefused)?;
        Ok(Self {
            source: Some(source),
            roots: retained,
            initial_roots,
            limits,
            fence_consumed: false,
            pass_count: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_unavailable() -> Self {
        Self {
            source: None,
            roots: BTreeMap::new(),
            initial_roots: unavailable_root_set(),
            limits: DescriptorCensusLimitsV1::bounded_default(),
            fence_consumed: true,
            pass_count: 0,
        }
    }
}

impl owner_sealed::Sealed for Stage11AggregateCensusBackendSeedV2 {}

impl AggregateCensusBackendV2 for Stage11AggregateCensusBackendSeedV2 {
    fn acquire_complete_admitted_root_source(
        &mut self,
    ) -> SecureFsResult<FoundationAdmittedRootSourceV2> {
        self.source.take().ok_or(SecureFsError::CensusRefused)
    }

    fn census_pass(
        &mut self,
        roots: &AggregateRootSetFactsV1,
        pass: u8,
    ) -> SecureFsResult<Vec<AggregateComponentCensusV1>> {
        if self.fence_consumed
            || roots != &self.initial_roots
            || pass != self.pass_count.saturating_add(1)
            || pass > 2
        {
            return Err(SecureFsError::CensusRefused);
        }
        self.pass_count = pass;
        census_retained_roots(roots, &self.roots, self.limits)
    }

    fn final_root_set_recheck(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
        if self.fence_consumed || self.pass_count != 2 {
            return Err(SecureFsError::CensusRefused);
        }
        reobserve_root_set(&self.initial_roots, &self.roots)
    }

    fn aggregate_fence_is_live(&self) -> bool {
        !self.fence_consumed
            && self.pass_count <= 2
            && reobserve_root_set(&self.initial_roots, &self.roots)
                .is_ok_and(|observed| observed == self.initial_roots)
    }

    fn consume_final_aggregate_fence(&mut self, scan_invocation: [u8; 32]) -> SecureFsResult<()> {
        if self.fence_consumed
            || self.pass_count != 2
            || scan_invocation != self.initial_roots.scan_invocation
            || reobserve_root_set(&self.initial_roots, &self.roots)? != self.initial_roots
        {
            return Err(SecureFsError::CensusRefused);
        }
        self.fence_consumed = true;
        Ok(())
    }
}

pub(crate) fn admit_persistence_roots_v2(
    roots: &[impl AsRef<Path>],
    owner_currentness: [u8; 32],
) -> SecureFsResult<PersistenceAdmittedRootSourceV2> {
    Ok(PersistenceAdmittedRootSourceV2 {
        roots: admit_required_roots(roots)?,
        owner_currentness,
    })
}

pub(crate) fn admit_installation_roots_v2(
    roots: &[impl AsRef<Path>],
    owner_currentness: [u8; 32],
) -> SecureFsResult<InstallationAdmittedRootSourceV2> {
    Ok(InstallationAdmittedRootSourceV2 {
        roots: admit_required_roots(roots)?,
        owner_currentness,
    })
}

fn admit_required_roots(
    paths: &[impl AsRef<Path>],
) -> SecureFsResult<Vec<(AggregateRootFactsV1, SecureRoot)>> {
    if paths.is_empty() {
        return Err(SecureFsError::CensusRefused);
    }
    paths
        .iter()
        .map(|path| {
            let root = SecureRoot::open(path)?;
            let facts = root.descriptor_census_admission_facts_v2()?;
            let mut declared = Sha256::new();
            declared.update(b"maestro.foundation.declared-root.v2\0");
            declared.update((root.path().as_os_str().as_bytes().len() as u64).to_be_bytes());
            declared.update(root.path().as_os_str().as_bytes());
            Ok((
                AggregateRootFactsV1 {
                    role: AggregateRootRoleV1::Required,
                    declared_locator: declared.finalize().into(),
                    resolved_identity: facts.resolved_identity,
                    mount_identity: facts.mount_identity,
                    provider_identity: facts.provider_identity,
                    anchor_identity: facts.anchor_identity,
                    fence_identity: facts.fence_identity,
                    journal_position: facts.journal_position,
                    locator_components: facts.locator_components,
                    absence_fence: None,
                },
                root,
            ))
        })
        .collect()
}

fn retain_unique_roots(
    roots: Vec<(AggregateRootFactsV1, SecureRoot)>,
    retained: &mut BTreeMap<[u8; 32], SecureRoot>,
) -> SecureFsResult<Vec<AggregateRootFactsV1>> {
    let mut facts = Vec::with_capacity(roots.len());
    for (root_facts, root) in roots {
        if retained
            .insert(root_facts.resolved_identity, root)
            .is_some()
        {
            return Err(SecureFsError::CensusRefused);
        }
        facts.push(root_facts);
    }
    Ok(facts)
}

fn census_retained_roots(
    roots: &AggregateRootSetFactsV1,
    retained: &BTreeMap<[u8; 32], SecureRoot>,
    limits: DescriptorCensusLimitsV1,
) -> SecureFsResult<Vec<AggregateComponentCensusV1>> {
    roots
        .roots
        .iter()
        .filter(|root| root.role != AggregateRootRoleV1::OptionalAbsent)
        .map(|facts| {
            let root = retained
                .get(&facts.resolved_identity)
                .ok_or(SecureFsError::CensusRefused)?;
            let census = SecureRoot::census_admitted_descriptor_root(
                root.admit_descriptor_census_root()?,
                limits,
            )?;
            let byte_count = census.rows().iter().try_fold(0_u64, |total, row| {
                total
                    .checked_add(row.logical_byte_length())
                    .ok_or(SecureFsError::CensusRefused)
            })?;
            let mut binding = Sha256::new();
            binding.update(b"maestro.foundation.aggregate-component-root-binding.v2\0");
            binding.update(facts.resolved_identity);
            binding.update(facts.mount_identity);
            binding.update(facts.anchor_identity);
            Ok(AggregateComponentCensusV1 {
                resolved_identity: facts.resolved_identity,
                inventory: stable_inventory_identity(facts, census.rows()),
                root_binding: binding.finalize().into(),
                rows: census.rows().to_vec(),
                entry_count: census.rows().len() as u64,
                byte_count,
            })
        })
        .collect()
}

fn stable_inventory_identity(facts: &AggregateRootFactsV1, rows: &[InventoryRowV1]) -> [u8; 32] {
    let mut inventory = Sha256::new();
    inventory.update(b"maestro.foundation.aggregate-component-inventory.v2\0");
    inventory.update(facts.resolved_identity);
    inventory.update(facts.mount_identity);
    inventory.update((rows.len() as u64).to_be_bytes());
    for row in rows {
        inventory.update((row.relative_name().len() as u64).to_be_bytes());
        inventory.update(row.relative_name());
        inventory.update([match row.kind() {
            super::secure_fs::DescriptorCensusObjectKindV1::RegularFile => 1,
            super::secure_fs::DescriptorCensusObjectKindV1::SymbolicLink => 2,
        }]);
        inventory.update(row.logical_byte_length().to_be_bytes());
        inventory.update(row.object_identity());
        inventory.update(row.content_identity());
    }
    inventory.finalize().into()
}

fn reobserve_root_set(
    initial: &AggregateRootSetFactsV1,
    retained: &BTreeMap<[u8; 32], SecureRoot>,
) -> SecureFsResult<AggregateRootSetFactsV1> {
    let mut observed = initial.clone();
    for facts in &mut observed.roots {
        if facts.role == AggregateRootRoleV1::OptionalAbsent {
            continue;
        }
        let root = retained
            .get(&facts.resolved_identity)
            .ok_or(SecureFsError::CensusRefused)?;
        let current = root.descriptor_census_admission_facts_v2()?;
        facts.resolved_identity = current.resolved_identity;
        facts.mount_identity = current.mount_identity;
        facts.provider_identity = current.provider_identity;
        facts.anchor_identity = current.anchor_identity;
        facts.fence_identity = current.fence_identity;
        facts.journal_position = current.journal_position;
        facts.locator_components = current.locator_components;
    }
    Ok(observed)
}

fn source_preview(
    source: &FoundationAdmittedRootSourceV2,
) -> SecureFsResult<AggregateRootSetFactsV1> {
    let repository = &source.repository;
    let installation = &source.installation;
    let invocation = &source.invocation;
    let mut roots = repository.roots.clone();
    roots.extend(installation.roots.clone());
    let mut digest = Sha256::new();
    digest.update(b"maestro.foundation.admitted-root-set.v2\0");
    digest.update(repository.owner_currentness);
    digest.update(installation.owner_currentness);
    digest.update(invocation.invocation);
    let admitted_set = digest.finalize().into();
    Ok(AggregateRootSetFactsV1 {
        admitted_set,
        namespace_epoch: invocation.namespace_epoch,
        roots,
        maximum_entries: invocation.maximum_entries,
        maximum_bytes: invocation.maximum_bytes,
        maximum_roots: invocation.maximum_roots,
        maximum_descriptors: invocation.maximum_descriptors,
        maximum_depth: invocation.maximum_depth,
        maximum_name_bytes: invocation.maximum_name_bytes,
        scan_invocation: invocation.invocation,
        root_set_currentness: admitted_set,
        revocation_revision: invocation.revocation_revision,
    })
}

fn unavailable_root_set() -> AggregateRootSetFactsV1 {
    AggregateRootSetFactsV1 {
        admitted_set: [0; 32],
        namespace_epoch: 0,
        roots: Vec::new(),
        maximum_entries: 0,
        maximum_bytes: 0,
        maximum_roots: 0,
        maximum_descriptors: 0,
        maximum_depth: 0,
        maximum_name_bytes: 0,
        scan_invocation: [0; 32],
        root_set_currentness: [0; 32],
        revocation_revision: 0,
    }
}

impl AggregateCensusBackendV1 for Stage11AggregateCensusBackendSeedV1 {
    fn acquire_complete_root_set(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
        Err(SecureFsError::CensusRefused)
    }

    fn census_pass(
        &mut self,
        _roots: &AggregateRootSetFactsV1,
        _pass: u8,
    ) -> SecureFsResult<Vec<AggregateComponentCensusV1>> {
        Err(SecureFsError::CensusRefused)
    }

    fn final_root_set_recheck(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
        Err(SecureFsError::CensusRefused)
    }

    fn aggregate_fence_is_live(&self) -> bool {
        false
    }

    fn consume_final_aggregate_fence(&mut self, _scan_invocation: [u8; 32]) -> SecureFsResult<()> {
        Err(SecureFsError::CensusRefused)
    }
}

impl Stage11AggregateCensusBackendSeedV1 {
    #[cfg(test)]
    pub(crate) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

#[cfg(test)]
mod v2_tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::foundation::core::aggregate_census::census_from_stage11_owner_v2;

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Roots {
        base: std::path::PathBuf,
        repository: std::path::PathBuf,
        installation: std::path::PathBuf,
    }

    impl Roots {
        fn new() -> Self {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let temp = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            let base = temp.join(format!(
                "maestro-stage11-descriptor-census-{}-{serial}",
                std::process::id()
            ));
            let repository = base.join("repository");
            let installation = base.join("installation");
            fs::create_dir_all(&repository).expect("create repository root");
            fs::create_dir_all(&installation).expect("create installation root");
            Self {
                base,
                repository,
                installation,
            }
        }

        fn provider(&self) -> Stage11AggregateCensusBackendSeedV2 {
            Stage11AggregateCensusBackendSeedV2::from_owner_sources(
                admit_persistence_roots_v2(&[&self.repository], [1; 32])
                    .expect("repository admission"),
                admit_installation_roots_v2(&[&self.installation], [2; 32])
                    .expect("installation admission"),
                [3; 32],
                4,
                100,
                1_000_000,
                2,
                100,
                64,
                255,
                5,
            )
            .expect("descriptor provider")
        }
    }

    impl Drop for Roots {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn descriptor_backend_censuses_all_owner_admitted_roots_twice() {
        let roots = Roots::new();
        fs::write(roots.repository.join("repository.bin"), b"repo").expect("repository file");
        fs::write(roots.installation.join("installation.bin"), b"installation")
            .expect("installation file");
        let mut provider = roots.provider();
        let continuation = census_from_stage11_owner_v2(&mut provider).expect("census");
        let (_, entries, bytes, components) = continuation.into_stage11_parts();
        assert_eq!(entries, 2);
        assert_eq!(bytes, 16);
        assert_eq!(components.len(), 2);
        assert!(provider.fence_consumed);
        assert_eq!(provider.pass_count, 2);
        assert!(provider.acquire_complete_admitted_root_source().is_err());
    }

    #[test]
    fn mutation_between_descriptor_passes_is_refused() {
        let roots = Roots::new();
        fs::write(roots.repository.join("before"), b"before").expect("seed file");
        SecureRoot::install_after_first_census_pass_test_hook(|root| {
            fs::write(root.join("after"), b"after").expect("mutate root");
        });
        let mut provider = roots.provider();
        assert!(census_from_stage11_owner_v2(&mut provider).is_err());
        assert!(!provider.fence_consumed);
    }

    #[test]
    fn root_turnover_after_admission_is_refused() {
        let roots = Roots::new();
        fs::write(roots.repository.join("before"), b"before").expect("seed file");
        let mut provider = roots.provider();
        let moved = roots.base.join("repository-moved");
        fs::rename(&roots.repository, &moved).expect("move admitted root");
        fs::create_dir(&roots.repository).expect("replace admitted path");
        assert!(census_from_stage11_owner_v2(&mut provider).is_err());
        assert!(!provider.fence_consumed);
    }

    #[test]
    fn cross_root_hard_link_alias_is_refused() {
        let roots = Roots::new();
        let source = roots.repository.join("shared");
        fs::write(&source, b"shared").expect("seed hard link");
        fs::hard_link(&source, roots.installation.join("shared")).expect("cross-root hard link");
        let mut provider = roots.provider();
        assert!(census_from_stage11_owner_v2(&mut provider).is_err());
        assert!(!provider.fence_consumed);
    }
}

pub(super) fn census_from_stage11_owner<'scan>(
    backend: &'scan mut dyn AggregateCensusBackendV1,
) -> SecureFsResult<Stage11AggregateCensusOutputV1<'scan>> {
    super::aggregate_census::census_from_stage11_owner(backend)
        .map(|result| Stage11AggregateCensusOutputV1 { result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_is_owner_local_and_fail_closed() {
        let mut backend = Stage11AggregateCensusBackendSeedV1::test_unavailable();
        assert!(matches!(
            census_from_stage11_owner(&mut backend),
            Err(SecureFsError::CensusRefused)
        ));
    }
}
