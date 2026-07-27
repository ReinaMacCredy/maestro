//! Historical V1 census evidence retained only for test compilation.
//!
//! Production Stage 11 must consume `MigrationClassificationContinuationV2`.

#![expect(
    dead_code,
    reason = "V1 census remains test-only historical negative proof for V4 removal guards"
)]

use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::vnext::migration::runtime::{
    ByteTotalInventoryV1, DeclaredRootV1, InventoryErrorV1, InventoryNodeKindV1,
    InventoryPayloadV1, InventoryRowV1, MigrationDigestV1, MigrationIdentityErrorV1,
    NormalizedLocatorV1,
};
use crate::foundation::core::descriptor_census_platform;
use crate::foundation::core::secure_fs::{
    DescriptorCensusLimitsV1, DescriptorCensusObjectKindV1, SecureFsError, SecureRoot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeclaredRootScanV1 {
    declaration: DeclaredRootV1,
    resolved_path: PathBuf,
}

impl DeclaredRootScanV1 {
    pub(crate) fn new(
        declaration: DeclaredRootV1,
        resolved_path: PathBuf,
    ) -> Result<Self, MigrationCensusErrorV1> {
        if !resolved_path.is_absolute()
            || resolved_path.as_os_str().as_bytes() != declaration.resolved_locator().as_bytes()
        {
            return Err(MigrationCensusErrorV1::ResolvedPathMismatch);
        }
        Ok(Self {
            declaration,
            resolved_path,
        })
    }

    fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }
}

pub(crate) fn recensus_declared_roots(
    mut scans: Vec<DeclaredRootScanV1>,
) -> Result<ByteTotalInventoryV1, MigrationCensusErrorV1> {
    scans.sort_by_key(|scan| scan.declaration.id());
    if scans
        .windows(2)
        .any(|pair| pair[0].declaration.id() == pair[1].declaration.id())
    {
        return Err(MigrationCensusErrorV1::DuplicateDeclaredRoot);
    }
    let roots = scans
        .iter()
        .map(|scan| scan.declaration.clone())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for scan in &scans {
        let root = match SecureRoot::open(scan.resolved_path()) {
            Ok(root) => root,
            Err(SecureFsError::Io { source, .. })
                if scan.declaration.optional() && source.kind() == io::ErrorKind::NotFound =>
            {
                continue;
            }
            Err(_) => return Err(MigrationCensusErrorV1::FoundationCensusRefused),
        };
        let census =
            descriptor_census_platform::census(&root, DescriptorCensusLimitsV1::bounded_default())
                .map_err(|_| MigrationCensusErrorV1::FoundationCensusRefused)?;
        for source in census.rows() {
            let kind = match source.kind() {
                DescriptorCensusObjectKindV1::RegularFile => InventoryNodeKindV1::RegularFile,
                DescriptorCensusObjectKindV1::SymbolicLink => InventoryNodeKindV1::SymbolicLink,
            };
            rows.push(InventoryRowV1::new(
                scan.declaration.id(),
                joined_locator(scan.declaration.display_locator(), source.relative_name())?,
                joined_locator(scan.declaration.resolved_locator(), source.relative_name())?,
                scan.declaration.domain(),
                kind,
                InventoryPayloadV1::from_descriptor_census(
                    source.logical_byte_length(),
                    source.content_identity(),
                )?,
                MigrationDigestV1::from_digest(source.object_identity())?,
            )?);
        }
    }
    Ok(ByteTotalInventoryV1::new(roots, rows)?)
}

fn joined_locator(
    root: &NormalizedLocatorV1,
    relative: &[u8],
) -> Result<NormalizedLocatorV1, MigrationCensusErrorV1> {
    if relative.is_empty() || relative[0] == b'/' {
        return Err(MigrationCensusErrorV1::InvalidFoundationLocator);
    }
    let mut bytes = root.as_bytes().to_vec();
    if bytes != b"/" {
        bytes.push(b'/');
    }
    bytes.extend_from_slice(relative);
    Ok(NormalizedLocatorV1::new(bytes)?)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum MigrationCensusErrorV1 {
    #[error("declared resolved path is not the exact normalized absolute locator")]
    ResolvedPathMismatch,
    #[error("declared root is duplicated")]
    DuplicateDeclaredRoot,
    #[error("Foundation refused the namespace-wide descriptor census")]
    FoundationCensusRefused,
    #[error("Foundation returned an invalid relative census locator")]
    InvalidFoundationLocator,
    #[error(transparent)]
    Inventory(#[from] InventoryErrorV1),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
}
