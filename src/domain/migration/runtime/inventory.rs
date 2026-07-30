use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

use super::{MigrationDigestV1, MigrationIdentityErrorV1};

const INVENTORY_IDENTITY_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.inventory.v1\0";
const SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.source-identity.v1\0";
const H3_INVENTORY_ROW_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.h3-source-inventory-row.v1\0";
const H3_INVENTORY_ROWS_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.h3-source-inventory-rows.v1\0";
const H3_PROTECTED_ROOTS_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.h3-protected-roots.v1\0";
const MAX_LOCATOR_BYTES_V1: usize = 4_096;
const MAX_DECLARED_ROOTS_V1: usize = 4_096;
const MAX_INVENTORY_ROWS_V1: usize = 1_000_000;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedLocatorV1(Vec<u8>);

impl NormalizedLocatorV1 {
    pub fn new(bytes: Vec<u8>) -> Result<Self, InventoryErrorV1> {
        if bytes.is_empty()
            || bytes.len() > MAX_LOCATOR_BYTES_V1
            || bytes[0] != b'/'
            || bytes.contains(&0)
        {
            return Err(InventoryErrorV1::InvalidLocator);
        }
        if bytes.len() > 1 && (bytes.ends_with(b"/") || bytes.windows(2).any(|pair| pair == b"//"))
        {
            return Err(InventoryErrorV1::InvalidLocator);
        }
        if bytes.len() > 1
            && bytes
                .split(|byte| *byte == b'/')
                .skip(1)
                .any(|component| component == b"." || component == b".." || component.is_empty())
        {
            return Err(InventoryErrorV1::InvalidLocator);
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_within(&self, root: &Self) -> bool {
        if root.0 == b"/" {
            return true;
        }
        self.0 == root.0
            || (self.0.starts_with(&root.0) && self.0.get(root.0.len()).copied() == Some(b'/'))
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        CborValue::Bytes(self.0.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InventoryDomainV1 {
    Repository,
    Installation,
}

impl InventoryDomainV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Repository => 1,
            Self::Installation => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InventoryNodeKindV1 {
    RegularFile,
    Directory,
    SymbolicLink,
    UnavailablePreexistingLoss,
}

impl InventoryNodeKindV1 {
    pub const fn tag(self) -> u64 {
        match self {
            Self::RegularFile => 1,
            Self::Directory => 2,
            Self::SymbolicLink => 3,
            Self::UnavailablePreexistingLoss => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryPayloadV1 {
    Present {
        byte_length: u64,
        sha256: MigrationDigestV1,
    },
    Unavailable {
        expected_byte_length: u64,
        expected_sha256: MigrationDigestV1,
        loss_evidence_id: MigrationDigestV1,
    },
}

impl InventoryPayloadV1 {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InventoryErrorV1> {
        Ok(Self::Present {
            byte_length: u64::try_from(bytes.len())
                .map_err(|_| InventoryErrorV1::PayloadLengthOverflow)?,
            sha256: MigrationDigestV1::digest_bytes(bytes)?,
        })
    }

    pub(crate) fn from_descriptor_census(
        byte_length: u64,
        sha256: [u8; 32],
    ) -> Result<Self, InventoryErrorV1> {
        Ok(Self::Present {
            byte_length,
            sha256: MigrationDigestV1::from_digest(sha256)?,
        })
    }

    pub const fn byte_length(&self) -> u64 {
        match self {
            Self::Present { byte_length, .. } => *byte_length,
            Self::Unavailable {
                expected_byte_length,
                ..
            } => *expected_byte_length,
        }
    }

    pub const fn sha256(&self) -> MigrationDigestV1 {
        match self {
            Self::Present { sha256, .. } => *sha256,
            Self::Unavailable {
                expected_sha256, ..
            } => *expected_sha256,
        }
    }

    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Present { .. })
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Present {
                byte_length,
                sha256,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(*byte_length),
                sha256.canonical_value(),
            ]),
            Self::Unavailable {
                expected_byte_length,
                expected_sha256,
                loss_evidence_id,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Unsigned(*expected_byte_length),
                expected_sha256.canonical_value(),
                loss_evidence_id.canonical_value(),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredRootV1 {
    id: MigrationDigestV1,
    display_locator: NormalizedLocatorV1,
    resolved_locator: NormalizedLocatorV1,
    domain: InventoryDomainV1,
    optional: bool,
}

impl DeclaredRootV1 {
    pub fn new(
        display_locator: NormalizedLocatorV1,
        resolved_locator: NormalizedLocatorV1,
        domain: InventoryDomainV1,
        optional: bool,
    ) -> Result<Self, InventoryErrorV1> {
        let id = MigrationDigestV1::identify(
            b"maestro.vnext.migration.declared-root.v1\0",
            &CborValue::Array(vec![
                display_locator.canonical_value(),
                resolved_locator.canonical_value(),
                CborValue::Unsigned(domain.tag()),
                CborValue::Bool(optional),
            ]),
        )?;
        Ok(Self {
            id,
            display_locator,
            resolved_locator,
            domain,
            optional,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn domain(&self) -> InventoryDomainV1 {
        self.domain
    }

    pub const fn optional(&self) -> bool {
        self.optional
    }

    pub fn display_locator(&self) -> &NormalizedLocatorV1 {
        &self.display_locator
    }

    pub fn resolved_locator(&self) -> &NormalizedLocatorV1 {
        &self.resolved_locator
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.id.canonical_value(),
            self.display_locator.canonical_value(),
            self.resolved_locator.canonical_value(),
            CborValue::Unsigned(self.domain.tag()),
            CborValue::Bool(self.optional),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryRowV1 {
    source_id: MigrationDigestV1,
    declared_root_id: MigrationDigestV1,
    display_locator: NormalizedLocatorV1,
    resolved_locator: NormalizedLocatorV1,
    domain: InventoryDomainV1,
    kind: InventoryNodeKindV1,
    payload: InventoryPayloadV1,
    metadata_commitment: MigrationDigestV1,
}

impl InventoryRowV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the byte-total row binds every source fact explicitly"
    )]
    pub fn new(
        declared_root_id: MigrationDigestV1,
        display_locator: NormalizedLocatorV1,
        resolved_locator: NormalizedLocatorV1,
        domain: InventoryDomainV1,
        kind: InventoryNodeKindV1,
        payload: InventoryPayloadV1,
        metadata_commitment: MigrationDigestV1,
    ) -> Result<Self, InventoryErrorV1> {
        if matches!(kind, InventoryNodeKindV1::UnavailablePreexistingLoss)
            != matches!(&payload, InventoryPayloadV1::Unavailable { .. })
        {
            return Err(InventoryErrorV1::PayloadKindMismatch);
        }
        if matches!(kind, InventoryNodeKindV1::Directory)
            && (payload.byte_length() != 0
                || payload.sha256() != MigrationDigestV1::digest_bytes(&[])?)
        {
            return Err(InventoryErrorV1::DirectoryPayloadNotEmpty);
        }
        let source_id = MigrationDigestV1::identify(
            SOURCE_IDENTITY_DOMAIN_V1,
            &CborValue::Array(vec![
                CborValue::Unsigned(kind.tag()),
                CborValue::Unsigned(payload.byte_length()),
                payload.sha256().canonical_value(),
                display_locator.canonical_value(),
            ]),
        )?;
        Ok(Self {
            source_id,
            declared_root_id,
            display_locator,
            resolved_locator,
            domain,
            kind,
            payload,
            metadata_commitment,
        })
    }

    pub const fn source_id(&self) -> MigrationDigestV1 {
        self.source_id
    }

    pub const fn declared_root_id(&self) -> MigrationDigestV1 {
        self.declared_root_id
    }

    pub fn display_locator(&self) -> &NormalizedLocatorV1 {
        &self.display_locator
    }

    pub fn resolved_locator(&self) -> &NormalizedLocatorV1 {
        &self.resolved_locator
    }

    pub const fn domain(&self) -> InventoryDomainV1 {
        self.domain
    }

    pub const fn kind(&self) -> InventoryNodeKindV1 {
        self.kind
    }

    pub const fn payload(&self) -> &InventoryPayloadV1 {
        &self.payload
    }

    pub const fn metadata_commitment(&self) -> MigrationDigestV1 {
        self.metadata_commitment
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.source_id.canonical_value(),
            self.declared_root_id.canonical_value(),
            self.display_locator.canonical_value(),
            self.resolved_locator.canonical_value(),
            CborValue::Unsigned(self.domain.tag()),
            CborValue::Unsigned(self.kind.tag()),
            self.payload.canonical_value(),
            self.metadata_commitment.canonical_value(),
        ])
    }

    pub(super) fn h3_row_commitment(&self) -> Result<MigrationDigestV1, MigrationIdentityErrorV1> {
        MigrationDigestV1::identify(H3_INVENTORY_ROW_DOMAIN_V1, &self.canonical_value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteTotalInventoryV1 {
    roots: Vec<DeclaredRootV1>,
    rows: Vec<InventoryRowV1>,
    byte_count: u64,
    id: MigrationDigestV1,
}

impl ByteTotalInventoryV1 {
    pub fn new(
        mut roots: Vec<DeclaredRootV1>,
        mut rows: Vec<InventoryRowV1>,
    ) -> Result<Self, InventoryErrorV1> {
        if roots.is_empty() || roots.len() > MAX_DECLARED_ROOTS_V1 {
            return Err(InventoryErrorV1::InvalidRootCount);
        }
        if rows.is_empty() || rows.len() > MAX_INVENTORY_ROWS_V1 {
            return Err(InventoryErrorV1::InvalidRowCount);
        }
        roots.sort_by_key(|root| root.id);
        rows.sort_by_key(|row| row.source_id);
        if roots.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(InventoryErrorV1::DuplicateDeclaredRoot);
        }
        if rows
            .windows(2)
            .any(|pair| pair[0].source_id == pair[1].source_id)
        {
            return Err(InventoryErrorV1::DuplicateSourceIdentity);
        }

        let root_index = roots
            .iter()
            .map(|root| (root.id, root))
            .collect::<BTreeMap<_, _>>();
        let mut covered_roots = BTreeSet::new();
        let mut display_locators = BTreeSet::new();
        let mut resolved_locators = BTreeSet::new();
        let mut byte_count = 0_u64;
        for row in &rows {
            let root = root_index
                .get(&row.declared_root_id)
                .ok_or(InventoryErrorV1::UnknownDeclaredRoot)?;
            if row.domain != root.domain
                || !row.display_locator.is_within(&root.display_locator)
                || !row.resolved_locator.is_within(&root.resolved_locator)
            {
                return Err(InventoryErrorV1::RowOutsideDeclaredRoot);
            }
            if !display_locators.insert(row.display_locator.clone())
                || !resolved_locators.insert(row.resolved_locator.clone())
            {
                return Err(InventoryErrorV1::DuplicateLocator);
            }
            covered_roots.insert(root.id);
            byte_count = byte_count
                .checked_add(row.payload.byte_length())
                .ok_or(InventoryErrorV1::PayloadLengthOverflow)?;
        }
        for root in &roots {
            if !root.optional && !covered_roots.contains(&root.id) {
                return Err(InventoryErrorV1::RequiredRootMissing);
            }
        }

        let id = MigrationDigestV1::identify(
            INVENTORY_IDENTITY_DOMAIN_V1,
            &CborValue::Array(vec![
                CborValue::Array(roots.iter().map(DeclaredRootV1::canonical_value).collect()),
                CborValue::Array(rows.iter().map(InventoryRowV1::canonical_value).collect()),
                CborValue::Unsigned(byte_count),
            ]),
        )?;
        Ok(Self {
            roots,
            rows,
            byte_count,
            id,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub fn roots(&self) -> &[DeclaredRootV1] {
        &self.roots
    }

    pub fn rows(&self) -> &[InventoryRowV1] {
        &self.rows
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub fn row(&self, source_id: MigrationDigestV1) -> Option<&InventoryRowV1> {
        self.rows
            .binary_search_by_key(&source_id, InventoryRowV1::source_id)
            .ok()
            .map(|index| &self.rows[index])
    }

    pub(super) fn h3_rows_commitment(&self) -> Result<MigrationDigestV1, MigrationIdentityErrorV1> {
        MigrationDigestV1::identify(
            H3_INVENTORY_ROWS_DOMAIN_V1,
            &CborValue::Array(
                self.rows
                    .iter()
                    .map(InventoryRowV1::canonical_value)
                    .collect(),
            ),
        )
    }

    pub(super) fn h3_protected_roots_commitment(
        &self,
    ) -> Result<MigrationDigestV1, MigrationIdentityErrorV1> {
        MigrationDigestV1::identify(
            H3_PROTECTED_ROOTS_DOMAIN_V1,
            &CborValue::Array(
                self.roots
                    .iter()
                    .map(DeclaredRootV1::canonical_value)
                    .collect(),
            ),
        )
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InventoryErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("locator must be a normalized absolute byte path")]
    InvalidLocator,
    #[error("declared-root count is outside the finite v1 bounds")]
    InvalidRootCount,
    #[error("inventory row count is outside the finite v1 bounds")]
    InvalidRowCount,
    #[error("inventory contains a duplicate declared-root identity")]
    DuplicateDeclaredRoot,
    #[error("inventory contains a duplicate source identity")]
    DuplicateSourceIdentity,
    #[error("inventory contains a duplicate display or resolved locator")]
    DuplicateLocator,
    #[error("inventory row references an unknown declared root")]
    UnknownDeclaredRoot,
    #[error("inventory row escapes or mismatches its declared root")]
    RowOutsideDeclaredRoot,
    #[error("required declared root has no inventory row")]
    RequiredRootMissing,
    #[error("inventory payload kind does not match node kind")]
    PayloadKindMismatch,
    #[error("directory inventory rows must bind the exact empty payload")]
    DirectoryPayloadNotEmpty,
    #[error("inventory payload byte count overflowed")]
    PayloadLengthOverflow,
}
