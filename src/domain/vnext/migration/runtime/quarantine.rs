use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{
    ByteTotalInventoryV1, ClassificationSetV1, InventoryPayloadV1, InventoryRowV1,
    MigrationDigestV1, MigrationDispositionV1, MigrationIdentityErrorV1, NormalizedLocatorV1,
};

const QUARANTINE_ENTRY_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.quarantine-entry.v1\0";
const QUARANTINE_MANIFEST_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.quarantine.v1\0";
const QUARANTINE_CHUNK_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_QUARANTINE_ENTRIES_V1: usize = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineEntryV1 {
    id: MigrationDigestV1,
    source_id: MigrationDigestV1,
    source_byte_length: u64,
    source_sha256: MigrationDigestV1,
    metadata_commitment: MigrationDigestV1,
    reason_id: MigrationDigestV1,
    recovery_disposition_id: MigrationDigestV1,
    chunks: Vec<Vec<u8>>,
    chunk_digests: Vec<MigrationDigestV1>,
}

impl QuarantineEntryV1 {
    pub fn new(
        source: &InventoryRowV1,
        bytes: Vec<u8>,
        reason_id: MigrationDigestV1,
        recovery_disposition_id: MigrationDigestV1,
    ) -> Result<Self, QuarantineErrorV1> {
        let InventoryPayloadV1::Present {
            byte_length,
            sha256,
        } = source.payload()
        else {
            return Err(QuarantineErrorV1::UnavailableSource);
        };
        if u64::try_from(bytes.len()).map_err(|_| QuarantineErrorV1::PayloadLengthOverflow)?
            != *byte_length
            || MigrationDigestV1::digest_bytes(&bytes)? != *sha256
        {
            return Err(QuarantineErrorV1::SourceBytesMismatch);
        }
        let chunks = if bytes.is_empty() {
            vec![Vec::new()]
        } else {
            bytes
                .chunks(QUARANTINE_CHUNK_BYTES_V1)
                .map(<[u8]>::to_vec)
                .collect()
        };
        let chunk_digests = chunks
            .iter()
            .map(|chunk| MigrationDigestV1::digest_bytes(chunk))
            .collect::<Result<Vec<_>, _>>()?;
        let id = MigrationDigestV1::identify(
            QUARANTINE_ENTRY_DOMAIN_V1,
            &CborValue::Array(vec![
                source.source_id().canonical_value(),
                CborValue::Unsigned(*byte_length),
                sha256.canonical_value(),
                source.metadata_commitment().canonical_value(),
                reason_id.canonical_value(),
                recovery_disposition_id.canonical_value(),
                CborValue::Array(
                    chunks
                        .iter()
                        .zip(&chunk_digests)
                        .map(|(chunk, digest)| {
                            CborValue::Array(vec![
                                CborValue::Unsigned(chunk.len() as u64),
                                digest.canonical_value(),
                            ])
                        })
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            id,
            source_id: source.source_id(),
            source_byte_length: *byte_length,
            source_sha256: *sha256,
            metadata_commitment: source.metadata_commitment(),
            reason_id,
            recovery_disposition_id,
            chunks,
            chunk_digests,
        })
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub const fn source_id(&self) -> MigrationDigestV1 {
        self.source_id
    }

    pub const fn source_byte_length(&self) -> u64 {
        self.source_byte_length
    }

    pub const fn source_sha256(&self) -> MigrationDigestV1 {
        self.source_sha256
    }

    pub const fn metadata_commitment(&self) -> MigrationDigestV1 {
        self.metadata_commitment
    }

    pub const fn reason_id(&self) -> MigrationDigestV1 {
        self.reason_id
    }

    pub const fn recovery_disposition_id(&self) -> MigrationDigestV1 {
        self.recovery_disposition_id
    }

    pub fn chunks(&self) -> &[Vec<u8>] {
        &self.chunks
    }

    pub fn chunk_digests(&self) -> &[MigrationDigestV1] {
        &self.chunk_digests
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.id.canonical_value(),
            self.source_id.canonical_value(),
            CborValue::Unsigned(self.source_byte_length),
            self.source_sha256.canonical_value(),
            self.metadata_commitment.canonical_value(),
            self.reason_id.canonical_value(),
            self.recovery_disposition_id.canonical_value(),
            CborValue::Array(
                self.chunks
                    .iter()
                    .zip(&self.chunk_digests)
                    .map(|(chunk, digest)| {
                        CborValue::Array(vec![
                            CborValue::Unsigned(chunk.len() as u64),
                            digest.canonical_value(),
                        ])
                    })
                    .collect(),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedQuarantineManifestV1 {
    inventory_id: MigrationDigestV1,
    classification_set_id: MigrationDigestV1,
    quarantine_root: NormalizedLocatorV1,
    active_discovery_roots: Vec<NormalizedLocatorV1>,
    entries: Vec<QuarantineEntryV1>,
    id: MigrationDigestV1,
    canonical_bytes: Vec<u8>,
}

impl SealedQuarantineManifestV1 {
    pub fn new(
        inventory: &ByteTotalInventoryV1,
        classifications: &ClassificationSetV1,
        quarantine_root: NormalizedLocatorV1,
        mut entries: Vec<QuarantineEntryV1>,
    ) -> Result<Self, QuarantineErrorV1> {
        if classifications.inventory_id() != inventory.id() {
            return Err(QuarantineErrorV1::InventoryClassificationMismatch);
        }
        if entries.len() > MAX_QUARANTINE_ENTRIES_V1 {
            return Err(QuarantineErrorV1::EntryCountExceeded);
        }
        // The fence is derived from the inventory being sealed, never from
        // caller input: every declared root (display and resolved) is an
        // active discovery locator the quarantine must stay outside of.
        let mut active_discovery_roots = inventory
            .roots()
            .iter()
            .flat_map(|root| {
                [
                    root.display_locator().clone(),
                    root.resolved_locator().clone(),
                ]
            })
            .collect::<Vec<_>>();
        active_discovery_roots.sort();
        active_discovery_roots.dedup();
        if active_discovery_roots.is_empty() {
            return Err(QuarantineErrorV1::InvalidDiscoveryRootSet);
        }
        if active_discovery_roots
            .iter()
            .any(|root| quarantine_root.is_within(root) || root.is_within(&quarantine_root))
        {
            return Err(QuarantineErrorV1::QuarantineInsideDiscovery);
        }
        entries.sort_by_key(QuarantineEntryV1::source_id);
        if entries
            .windows(2)
            .any(|pair| pair[0].source_id == pair[1].source_id)
        {
            return Err(QuarantineErrorV1::DuplicateSource);
        }
        let expected = classifications
            .rows()
            .iter()
            .filter(|row| row.disposition() == MigrationDispositionV1::Quarantined)
            .map(|row| {
                (
                    row.source_id(),
                    row.quarantine_entry_id()
                        .expect("invariant: quarantined classification has an entry id"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let observed = entries
            .iter()
            .map(|entry| (entry.source_id, entry.id))
            .collect::<BTreeMap<_, _>>();
        if expected != observed {
            return Err(QuarantineErrorV1::ClassificationCoverageMismatch);
        }
        for entry in &entries {
            let source = inventory
                .row(entry.source_id)
                .ok_or(QuarantineErrorV1::ClassificationCoverageMismatch)?;
            if source.payload().byte_length() != entry.source_byte_length
                || source.payload().sha256() != entry.source_sha256
                || source.metadata_commitment() != entry.metadata_commitment
            {
                return Err(QuarantineErrorV1::SourceBytesMismatch);
            }
            let classification = classifications
                .row(entry.source_id)
                .ok_or(QuarantineErrorV1::ClassificationCoverageMismatch)?;
            if classification.reason_id() != entry.reason_id {
                return Err(QuarantineErrorV1::ReasonMismatch);
            }
            if entry
                .chunks
                .iter()
                .zip(&entry.chunk_digests)
                .map(|(chunk, digest)| {
                    Ok::<_, MigrationIdentityErrorV1>(
                        MigrationDigestV1::digest_bytes(chunk)? != *digest,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .any(|mismatch| mismatch)
            {
                return Err(QuarantineErrorV1::ChunkDigestMismatch);
            }
            let reconstructed = entry.chunks.concat();
            if u64::try_from(reconstructed.len())
                .map_err(|_| QuarantineErrorV1::PayloadLengthOverflow)?
                != entry.source_byte_length
                || MigrationDigestV1::digest_bytes(&reconstructed)? != entry.source_sha256
            {
                return Err(QuarantineErrorV1::SourceBytesMismatch);
            }
        }
        let manifest_value = CborValue::Array(vec![
            inventory.id().canonical_value(),
            classifications.id().canonical_value(),
            quarantine_root.canonical_value(),
            CborValue::Array(
                active_discovery_roots
                    .iter()
                    .map(NormalizedLocatorV1::canonical_value)
                    .collect(),
            ),
            CborValue::Array(
                entries
                    .iter()
                    .map(QuarantineEntryV1::canonical_value)
                    .collect(),
            ),
        ]);
        let id = MigrationDigestV1::identify(QUARANTINE_MANIFEST_DOMAIN_V1, &manifest_value)?;
        let canonical_bytes = deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Unsigned(1),
            id.canonical_value(),
            manifest_value,
        ]))?;
        Ok(Self {
            inventory_id: inventory.id(),
            classification_set_id: classifications.id(),
            quarantine_root,
            active_discovery_roots,
            entries,
            id,
            canonical_bytes,
        })
    }

    pub const fn inventory_id(&self) -> MigrationDigestV1 {
        self.inventory_id
    }

    pub const fn classification_set_id(&self) -> MigrationDigestV1 {
        self.classification_set_id
    }

    pub fn quarantine_root(&self) -> &NormalizedLocatorV1 {
        &self.quarantine_root
    }

    pub fn active_discovery_roots(&self) -> &[NormalizedLocatorV1] {
        &self.active_discovery_roots
    }

    pub fn entries(&self) -> &[QuarantineEntryV1] {
        &self.entries
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn chunk_identity_set(&self) -> BTreeSet<MigrationDigestV1> {
        self.entries
            .iter()
            .flat_map(|entry| entry.chunk_digests.iter().copied())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum QuarantineErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("quarantine cannot preserve an unavailable source")]
    UnavailableSource,
    #[error("quarantine bytes do not match the byte-total source inventory")]
    SourceBytesMismatch,
    #[error("quarantine chunk digest does not match its immutable bytes")]
    ChunkDigestMismatch,
    #[error("quarantine entry reason does not match its source classification")]
    ReasonMismatch,
    #[error("quarantine and classification refer to different inventories")]
    InventoryClassificationMismatch,
    #[error("quarantine entries do not exactly cover quarantined classifications")]
    ClassificationCoverageMismatch,
    #[error("quarantine contains a source more than once")]
    DuplicateSource,
    #[error("quarantine entry count exceeds the finite v1 bound")]
    EntryCountExceeded,
    #[error("sealed quarantine must be outside every active discovery root")]
    QuarantineInsideDiscovery,
    #[error("sealed quarantine requires a nonempty active discovery-root set")]
    InvalidDiscoveryRootSet,
    #[error("quarantine payload byte count overflowed")]
    PayloadLengthOverflow,
}
