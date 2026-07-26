#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes aggregate census before its Stage 11 production consumer"
    )
)]
pub(crate) mod aggregate_census;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the aggregate census owner seed before Stage 11 integrates it"
    )
)]
mod aggregate_census_stage11_seed;
pub mod backup;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the retired singular census port remains only as migration-proof history"
    )
)]
pub(crate) mod descriptor_census_platform;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the retired singular census seed remains only as migration-proof history"
    )
)]
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
