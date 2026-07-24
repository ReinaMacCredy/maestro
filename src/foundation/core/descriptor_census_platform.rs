use super::secure_fs::{
    DescriptorAnchoredCensusV1, DescriptorCensusLimitsV1, SecureFsError, SecureFsResult, SecureRoot,
};

pub(crate) fn census(
    _root: &SecureRoot,
    limits: DescriptorCensusLimitsV1,
) -> SecureFsResult<DescriptorAnchoredCensusV1> {
    // A root-local advisory lease cannot exclude writes through independently
    // opened descendant roots. Production therefore refuses until Migration
    // supplies the Foundation-owned namespace-wide snapshot capability.
    let capability = namespace_wide_snapshot_capability();
    let Some(()) = capability else {
        return Err(SecureFsError::CensusRefused);
    };
    let admitted = _root
        .admit_descriptor_census_root()
        .map_err(|_| SecureFsError::CensusRefused)?;
    SecureRoot::census_admitted_descriptor_root(admitted, limits)
        .map_err(|_| SecureFsError::CensusRefused)
}

fn namespace_wide_snapshot_capability() -> Option<()> {
    None
}
