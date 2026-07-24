use super::secure_fs::{
    DescriptorAnchoredCensusV1, DescriptorCensusLimitsV1, SecureFsError, SecureFsResult, SecureRoot,
};

mod backend_sealed {
    pub(in crate::foundation::core) trait Sealed {}
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct NamespaceWideSnapshotFactsV1 {
    namespace_identity: [u8; 32],
    provider_incarnation: [u8; 32],
    monotonic_epoch: u64,
}

impl NamespaceWideSnapshotFactsV1 {
    // TODO(Foundation Stage 11): Remove this expectation when the platform seed
    // mints the first production namespace snapshot after owner integration.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Stage 11 owner seed is predeclared but intentionally fail-closed"
        )
    )]
    pub(super) fn from_stage11_owner(
        namespace_identity: [u8; 32],
        provider_incarnation: [u8; 32],
        monotonic_epoch: u64,
    ) -> SecureFsResult<Self> {
        if namespace_identity == [0; 32] || provider_incarnation == [0; 32] || monotonic_epoch == 0
        {
            return Err(SecureFsError::CensusRefused);
        }
        Ok(Self {
            namespace_identity,
            provider_incarnation,
            monotonic_epoch,
        })
    }
}

pub(super) trait NamespaceWideSnapshotBackendV1: backend_sealed::Sealed {
    fn current_facts(&mut self) -> SecureFsResult<NamespaceWideSnapshotFactsV1>;
}

pub(super) use backend_sealed::Sealed as NamespaceWideSnapshotBackendSealedV1;

struct NamespaceWideSnapshotLeaseV1<'backend, B> {
    initial: NamespaceWideSnapshotFactsV1,
    backend: &'backend mut B,
}

impl<'backend, B: NamespaceWideSnapshotBackendV1> NamespaceWideSnapshotLeaseV1<'backend, B> {
    fn acquire(backend: &'backend mut B) -> SecureFsResult<Self> {
        let initial = backend.current_facts()?;
        Ok(Self { initial, backend })
    }

    fn recheck(self) -> SecureFsResult<()> {
        if self.backend.current_facts()? != self.initial {
            return Err(SecureFsError::CensusRefused);
        }
        Ok(())
    }
}

pub(crate) fn census(
    root: &SecureRoot,
    limits: DescriptorCensusLimitsV1,
) -> SecureFsResult<DescriptorAnchoredCensusV1> {
    let mut backend =
        super::descriptor_census_platform_stage11_seed::acquire_namespace_wide_snapshot(root)?;
    let lease = NamespaceWideSnapshotLeaseV1::acquire(&mut backend)?;
    let admitted = root
        .admit_descriptor_census_root()
        .map_err(|_| SecureFsError::CensusRefused)?;
    let census = SecureRoot::census_admitted_descriptor_root(admitted, limits)
        .map_err(|_| SecureFsError::CensusRefused)?;
    lease.recheck()?;
    Ok(census)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TurnoverBackend {
        calls: u64,
    }

    impl backend_sealed::Sealed for TurnoverBackend {}

    impl NamespaceWideSnapshotBackendV1 for TurnoverBackend {
        fn current_facts(&mut self) -> SecureFsResult<NamespaceWideSnapshotFactsV1> {
            self.calls += 1;
            NamespaceWideSnapshotFactsV1::from_stage11_owner([1; 32], [2; 32], self.calls)
        }
    }

    #[test]
    fn owner_seed_is_replaceable_but_turnover_fails_closed() {
        let mut backend = TurnoverBackend { calls: 0 };
        let lease = NamespaceWideSnapshotLeaseV1::acquire(&mut backend).unwrap();
        assert!(matches!(lease.recheck(), Err(SecureFsError::CensusRefused)));
    }

    #[test]
    fn seed_and_capability_surface_remain_foundation_private() {
        let module = include_str!("mod.rs");
        let platform = include_str!("descriptor_census_platform.rs");
        let seed = include_str!("descriptor_census_platform_stage11_seed.rs");
        let interface = platform.split("#[cfg(test)]").next().unwrap();
        let seed_interface = seed.split("#[cfg(test)]").next().unwrap();
        assert!(module.contains("mod descriptor_census_platform_stage11_seed;"));
        assert!(!module.contains("pub mod descriptor_census_platform_stage11_seed;"));
        assert!(interface.contains("pub(super) trait NamespaceWideSnapshotBackendV1"));
        assert!(!interface.contains("pub(crate) trait NamespaceWideSnapshotBackendV1"));
        assert!(!seed_interface.contains("pub(crate)"));
        assert!(!seed_interface.contains("pub fn acquire_namespace_wide_snapshot"));
    }
}
