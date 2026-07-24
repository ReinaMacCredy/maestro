pub mod backup;
pub(crate) mod descriptor_census_platform;
mod descriptor_census_platform_stage11_seed;
pub mod deterministic_cbor;
pub mod diff;
pub mod error;
pub mod fs;
pub mod git;
pub mod hash;
pub mod managed_blocks;
pub mod managed_path;
pub mod paths;
pub mod retention;

const _: fn(
    &secure_fs::SecureRoot,
    secure_fs::DescriptorCensusLimitsV1,
) -> secure_fs::SecureFsResult<secure_fs::DescriptorAnchoredCensusV1> =
    descriptor_census_platform::census;
pub mod safe_write;
pub mod schema;
pub mod secure_fs;
pub mod session;
pub mod slug;
pub mod table;
pub mod time;
