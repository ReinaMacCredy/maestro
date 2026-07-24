use super::secure_fs::{
    AdmittedDescriptorCensusRootV1, DescriptorAnchoredCensusV1, DescriptorCensusLimitsV1,
    SecureFsError, SecureFsResult, SecureRoot,
};

pub fn admit_root(root: &SecureRoot) -> SecureFsResult<AdmittedDescriptorCensusRootV1<'_>> {
    root.admit_descriptor_census_root()
        .map_err(|_| SecureFsError::CensusRefused)
}

pub fn census(
    root: AdmittedDescriptorCensusRootV1<'_>,
    limits: DescriptorCensusLimitsV1,
) -> SecureFsResult<DescriptorAnchoredCensusV1> {
    SecureRoot::census_admitted_descriptor_root(root, limits)
        .map_err(|_| SecureFsError::CensusRefused)
}

#[cfg(test)]
pub(crate) fn admit_and_census(
    root: &SecureRoot,
    limits: DescriptorCensusLimitsV1,
) -> SecureFsResult<DescriptorAnchoredCensusV1> {
    census(admit_root(root)?, limits)
}
