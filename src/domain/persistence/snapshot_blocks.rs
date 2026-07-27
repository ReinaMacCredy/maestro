use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::identity::{
    StoreExportChunkIdV1, StoreExportFamilyManifestIdV1, StoreObjectIdV1, StoreSchemaManifestIdV1,
    StoreSnapshotRootIdV1, derive_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::object::StoreObjectV1;
use super::snapshot::StoreSnapshotRootV1;
use super::snapshot_export::reconstruct_prior_export;
use super::snapshot_rows::{
    MAX_REFERENCED_PRIOR_ROOTS_V1, StoreSnapshotFamilyV1, StoreSnapshotRowV1,
};
use super::{BackupReceiptV1, SEALED_BACKUP_FORMAT_V1};

const SNAPSHOT_BLOCK_CLOSURE_VERSION_V2: u64 = 2;
const FLAT_SNAPSHOT_ROOT_VERSION_V2: u64 = 2;
const FLAT_FAMILY_MANIFEST_VERSION_V2: u64 = 2;
const SEMANTIC_SNAPSHOT_VERSION_V1: u64 = 1;
const SEMANTIC_FAMILY_MANIFEST_VERSION_V1: u64 = 1;
pub(crate) const MAX_SNAPSHOT_BLOCKS_V2: usize = 65_536;
pub(crate) const MAX_SNAPSHOT_BLOCK_BYTES_V2: usize = 15 * 1024 * 1024;
pub(crate) const MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2: usize = 15 * 1024 * 1024;
const _: () =
    assert!(MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 < deterministic_cbor::MAX_BYTE_STRING_BYTES);
const MAX_PRIOR_EXPORT_RECONSTRUCTION_BYTES_V2: usize = 512 * 1024 * 1024;

type BlockKey = (StoreSnapshotBlockKindV2, [u8; 32]);
type ReachableSnapshotClosure = (
    BTreeMap<BlockKey, StoreSnapshotBlockV2>,
    BTreeMap<[u8; 32], ReconstructedSnapshot>,
);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StoreSnapshotBlockKindV2 {
    SchemaManifest,
    FamilyManifest,
    RowChunk,
    SnapshotRoot,
    StoreObjectPayload,
}

impl StoreSnapshotBlockKindV2 {
    const ALL: [Self; 5] = [
        Self::SchemaManifest,
        Self::FamilyManifest,
        Self::RowChunk,
        Self::SnapshotRoot,
        Self::StoreObjectPayload,
    ];

    pub(crate) const fn tag(self) -> u64 {
        match self {
            Self::SchemaManifest => 1,
            Self::FamilyManifest => 2,
            Self::RowChunk => 3,
            Self::SnapshotRoot => 4,
            Self::StoreObjectPayload => 5,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, StoreSnapshotBlockError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.tag() == tag)
            .ok_or(StoreSnapshotBlockError::UnknownBlockKind(tag))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreSnapshotBlockV2 {
    kind: StoreSnapshotBlockKindV2,
    declared_identity: [u8; 32],
    canonical_bytes: Vec<u8>,
    content_digest: [u8; 32],
}

impl StoreSnapshotBlockV2 {
    fn build(
        kind: StoreSnapshotBlockKindV2,
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, StoreSnapshotBlockError> {
        validate_block_byte_length(canonical_bytes.len())?;
        let declared_identity = derive_block_identity(kind, &canonical_bytes)?;
        let content_digest = Sha256::digest(&canonical_bytes).into();
        Ok(Self {
            kind,
            declared_identity,
            canonical_bytes,
            content_digest,
        })
    }

    fn decode(value: &CborValue) -> Result<Self, StoreSnapshotBlockError> {
        let CborValue::Array(fields) = value else {
            return Err(StoreSnapshotBlockError::InvalidBlockShape);
        };
        let [
            CborValue::Unsigned(kind),
            CborValue::Bytes(declared_identity),
            CborValue::Bytes(content_digest),
            CborValue::Bytes(canonical_bytes),
        ] = fields.as_slice()
        else {
            return Err(StoreSnapshotBlockError::InvalidBlockShape);
        };
        let kind = StoreSnapshotBlockKindV2::from_tag(*kind)?;
        let declared_identity = digest32(declared_identity, "declared block identity")?;
        let content_digest = digest32(content_digest, "block content digest")?;
        let rebuilt = Self::build(kind, canonical_bytes.clone())?;
        if rebuilt.declared_identity != declared_identity {
            return Err(StoreSnapshotBlockError::DeclaredIdentityMismatch { kind });
        }
        if rebuilt.content_digest != content_digest {
            return Err(StoreSnapshotBlockError::ContentDigestMismatch { kind });
        }
        Ok(rebuilt)
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.tag()),
            CborValue::Bytes(self.declared_identity.to_vec()),
            CborValue::Bytes(self.content_digest.to_vec()),
            CborValue::Bytes(self.canonical_bytes.clone()),
        ])
    }

    fn key(&self) -> BlockKey {
        (self.kind, self.declared_identity)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreSnapshotBlockClosureV2 {
    current_root_id: [u8; 32],
    referenced_prior_root_ids: Vec<[u8; 32]>,
    blocks: Vec<StoreSnapshotBlockV2>,
    canonical_bytes: Vec<u8>,
    current_snapshot: StoreSnapshotRootV1,
}

impl StoreSnapshotBlockClosureV2 {
    pub(crate) fn build(
        current: &StoreSnapshotRootV1,
        prior_closures: &[Self],
    ) -> Result<Self, StoreSnapshotBlockError> {
        let current_root_id = *current.id().as_bytes();
        let referenced_prior_root_ids = current.prior_root_ids().to_vec();
        if referenced_prior_root_ids.len() > MAX_REFERENCED_PRIOR_ROOTS_V1 {
            return Err(StoreSnapshotBlockError::PriorRootCountLimitExceeded);
        }
        validate_prior_closure_input_bounds(prior_closures)?;
        let prior_by_root = index_prior_closures(prior_closures)?;
        if prior_by_root.keys().copied().collect::<BTreeSet<_>>()
            != referenced_prior_root_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        {
            return Err(StoreSnapshotBlockError::PriorClosureSetMismatch);
        }
        let mut blocks = BTreeMap::new();
        let mut total_block_bytes = 0;
        extract_flat_snapshot(current, &mut blocks, &mut total_block_bytes)?;
        for prior_root in &referenced_prior_root_ids {
            let prior = prior_by_root
                .get(prior_root)
                .ok_or(StoreSnapshotBlockError::MissingPriorClosure(*prior_root))?;
            for block in &prior.blocks {
                insert_block(&mut blocks, &mut total_block_bytes, block.clone())?;
            }
        }
        Self::from_block_map(current_root_id, blocks)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, StoreSnapshotBlockError> {
        if bytes.len() > MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 {
            return Err(StoreSnapshotBlockError::ClosureBytesLimitExceeded);
        }
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = &value else {
            return Err(StoreSnapshotBlockError::InvalidClosureShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Bytes(current_root_id),
            CborValue::Array(block_values),
        ] = fields.as_slice()
        else {
            return Err(StoreSnapshotBlockError::InvalidClosureShape);
        };
        if *version != SNAPSHOT_BLOCK_CLOSURE_VERSION_V2 {
            return Err(StoreSnapshotBlockError::UnknownClosureVersion(*version));
        }
        if block_values.is_empty() || block_values.len() > MAX_SNAPSHOT_BLOCKS_V2 {
            return Err(StoreSnapshotBlockError::BlockCountLimitExceeded);
        }
        let current_root_id = digest32(current_root_id, "current flat snapshot root identity")?;
        let blocks = block_values
            .iter()
            .map(StoreSnapshotBlockV2::decode)
            .collect::<Result<Vec<_>, _>>()?;
        if blocks.windows(2).any(|pair| pair[0].key() >= pair[1].key()) {
            return Err(StoreSnapshotBlockError::BlockOrderMismatch);
        }
        validate_total_block_bytes(&blocks)?;
        let block_map = blocks
            .iter()
            .cloned()
            .map(|block| (block.key(), block))
            .collect::<BTreeMap<_, _>>();
        if block_map.len() != blocks.len() {
            return Err(StoreSnapshotBlockError::DuplicateBlock);
        }
        let (expected, snapshots) = expected_reachable_closure(current_root_id, &block_map)?;
        if expected != block_map {
            return Err(StoreSnapshotBlockError::ClosureSetMismatch);
        }
        let canonical_bytes = deterministic_cbor::encode(&value)?;
        if canonical_bytes != bytes {
            return Err(StoreSnapshotBlockError::NonCanonicalClosure);
        }
        let current_snapshot = snapshots
            .get(&current_root_id)
            .ok_or(StoreSnapshotBlockError::MissingCurrentRoot(current_root_id))?
            .snapshot
            .clone();
        let closure = Self {
            current_root_id,
            referenced_prior_root_ids: current_snapshot.prior_root_ids().to_vec(),
            blocks,
            canonical_bytes,
            current_snapshot,
        };
        closure.validate_prior_export_artifacts(&block_map, &snapshots)?;
        Ok(closure)
    }

    fn from_block_map(
        current_root_id: [u8; 32],
        blocks: BTreeMap<BlockKey, StoreSnapshotBlockV2>,
    ) -> Result<Self, StoreSnapshotBlockError> {
        if blocks.is_empty() || blocks.len() > MAX_SNAPSHOT_BLOCKS_V2 {
            return Err(StoreSnapshotBlockError::BlockCountLimitExceeded);
        }
        let (expected, snapshots) = expected_reachable_closure(current_root_id, &blocks)?;
        if expected != blocks {
            return Err(StoreSnapshotBlockError::ClosureSetMismatch);
        }
        let current = snapshots
            .get(&current_root_id)
            .ok_or(StoreSnapshotBlockError::MissingCurrentRoot(current_root_id))?;
        let closure =
            Self::from_reconstructed_block_map(current_root_id, blocks, current.snapshot.clone())?;
        let available = closure.block_map();
        closure.validate_prior_export_artifacts(&available, &snapshots)?;
        Ok(closure)
    }

    fn from_reconstructed_block_map(
        current_root_id: [u8; 32],
        blocks: BTreeMap<BlockKey, StoreSnapshotBlockV2>,
        current_snapshot: StoreSnapshotRootV1,
    ) -> Result<Self, StoreSnapshotBlockError> {
        if blocks.is_empty() || blocks.len() > MAX_SNAPSHOT_BLOCKS_V2 {
            return Err(StoreSnapshotBlockError::BlockCountLimitExceeded);
        }
        if *current_snapshot.id().as_bytes() != current_root_id {
            return Err(StoreSnapshotBlockError::FlatRootReconstructionMismatch);
        }
        let referenced_prior_root_ids = current_snapshot.prior_root_ids().to_vec();
        if referenced_prior_root_ids.len() > MAX_REFERENCED_PRIOR_ROOTS_V1 {
            return Err(StoreSnapshotBlockError::PriorRootCountLimitExceeded);
        }
        let blocks = blocks.into_values().collect::<Vec<_>>();
        validate_total_block_bytes(&blocks)?;
        let value = CborValue::Array(vec![
            CborValue::Unsigned(SNAPSHOT_BLOCK_CLOSURE_VERSION_V2),
            CborValue::Bytes(current_root_id.to_vec()),
            CborValue::Array(
                blocks
                    .iter()
                    .map(StoreSnapshotBlockV2::canonical_value)
                    .collect(),
            ),
        ]);
        let canonical_bytes = deterministic_cbor::encode(&value)?;
        if canonical_bytes.len() > MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 {
            return Err(StoreSnapshotBlockError::ClosureBytesLimitExceeded);
        }
        Ok(Self {
            current_root_id,
            referenced_prior_root_ids,
            blocks,
            canonical_bytes,
            current_snapshot,
        })
    }

    pub(crate) fn subclosure(&self, root_id: &[u8; 32]) -> Result<Self, StoreSnapshotBlockError> {
        let available = self.block_map();
        let (reachable, snapshots) = expected_reachable_closure(*root_id, &available)?;
        let current = snapshots
            .get(root_id)
            .ok_or(StoreSnapshotBlockError::MissingCurrentRoot(*root_id))?;
        let closure =
            Self::from_reconstructed_block_map(*root_id, reachable, current.snapshot.clone())?;
        let available = closure.block_map();
        closure.validate_prior_export_artifacts(&available, &snapshots)?;
        Ok(closure)
    }

    pub(crate) fn current_snapshot(&self) -> Result<StoreSnapshotRootV1, StoreSnapshotBlockError> {
        Ok(self.current_snapshot.clone())
    }

    fn block_map(&self) -> BTreeMap<BlockKey, StoreSnapshotBlockV2> {
        self.blocks
            .iter()
            .cloned()
            .map(|block| (block.key(), block))
            .collect()
    }

    pub(crate) const fn current_root_id(&self) -> &[u8; 32] {
        &self.current_root_id
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    fn validate_prior_export_artifacts(
        &self,
        available: &BTreeMap<BlockKey, StoreSnapshotBlockV2>,
        snapshots: &BTreeMap<[u8; 32], ReconstructedSnapshot>,
    ) -> Result<(), StoreSnapshotBlockError> {
        let current = &snapshots
            .get(&self.current_root_id)
            .ok_or(StoreSnapshotBlockError::MissingCurrentRoot(
                self.current_root_id,
            ))?
            .snapshot;
        let mut receipts = BTreeMap::new();
        for row in current.rows() {
            let StoreSnapshotRowV1::SealedExport {
                export_id,
                snapshot_root_id,
                committed_publication_clock,
                export_byte_length,
                export_bytes_digest,
                backup_receipt_id,
                carrier_format,
                ..
            } = row
            else {
                continue;
            };
            let prior_root_id = *snapshot_root_id.as_bytes();
            if receipts
                .insert(
                    prior_root_id,
                    PriorExportArtifactReceipt {
                        export_id: *export_id.as_bytes(),
                        backup_receipt_id: *backup_receipt_id.as_bytes(),
                        committed_publication_clock: committed_publication_clock.get(),
                        declared_byte_length: export_byte_length.get(),
                        bytes_digest: *export_bytes_digest.as_bytes(),
                        carrier_format: carrier_format.clone(),
                    },
                )
                .is_some()
            {
                return Err(StoreSnapshotBlockError::DuplicatePriorExportArtifactRoot(
                    prior_root_id,
                ));
            }
        }
        validate_declared_artifact_lengths(
            receipts
                .values()
                .map(|receipt| receipt.declared_byte_length),
            MAX_PRIOR_EXPORT_RECONSTRUCTION_BYTES_V2 as u64,
        )?;

        let mut reconstructed_bytes = 0_u64;
        for (prior_root_id, receipt) in receipts {
            let prior = &snapshots
                .get(&prior_root_id)
                .ok_or(StoreSnapshotBlockError::MissingPriorClosure(prior_root_id))?
                .snapshot;
            let prior_blocks =
                reachable_blocks_from_reconstruction_cache(prior_root_id, available, snapshots)?;
            let prior_closure =
                Self::from_reconstructed_block_map(prior_root_id, prior_blocks, prior.clone())?;
            let reconstructed =
                reconstruct_prior_export(prior, prior_closure).map_err(|error| {
                    StoreSnapshotBlockError::PriorExportReconstruction(error.to_string())
                })?;
            let backup_receipt = BackupReceiptV1::for_committed_export(
                &reconstructed,
                receipt.committed_publication_clock,
            )
            .map_err(|error| {
                StoreSnapshotBlockError::PriorExportReconstruction(error.to_string())
            })?;
            reconstructed_bytes = reconstructed_bytes
                .checked_add(reconstructed.canonical_bytes().len() as u64)
                .ok_or(StoreSnapshotBlockError::PriorExportReconstructionLimitExceeded)?;
            if reconstructed_bytes > MAX_PRIOR_EXPORT_RECONSTRUCTION_BYTES_V2 as u64 {
                return Err(StoreSnapshotBlockError::PriorExportReconstructionLimitExceeded);
            }
            if reconstructed.id().as_bytes() != &receipt.export_id
                || reconstructed.canonical_bytes().len() as u64 != receipt.declared_byte_length
                || Sha256::digest(reconstructed.canonical_bytes()).as_slice()
                    != receipt.bytes_digest
                || backup_receipt.id().as_bytes() != &receipt.backup_receipt_id
                || receipt.carrier_format != SEALED_BACKUP_FORMAT_V1
            {
                return Err(StoreSnapshotBlockError::PriorExportArtifactMismatch(
                    prior_root_id,
                ));
            }
        }
        Ok(())
    }
}

struct ReconstructedSnapshot {
    snapshot: StoreSnapshotRootV1,
    direct_keys: BTreeSet<BlockKey>,
}

#[derive(Clone)]
struct PriorExportArtifactReceipt {
    export_id: [u8; 32],
    backup_receipt_id: [u8; 32],
    committed_publication_clock: u64,
    declared_byte_length: u64,
    bytes_digest: [u8; 32],
    carrier_format: String,
}

fn extract_flat_snapshot(
    snapshot: &StoreSnapshotRootV1,
    blocks: &mut BTreeMap<BlockKey, StoreSnapshotBlockV2>,
    total_block_bytes: &mut usize,
) -> Result<(), StoreSnapshotBlockError> {
    let schema = StoreSnapshotBlockV2::build(
        StoreSnapshotBlockKindV2::SchemaManifest,
        deterministic_cbor::encode(snapshot.schema_manifest_value())?,
    )?;
    if schema.declared_identity != *snapshot.schema_manifest_id().as_bytes() {
        return Err(StoreSnapshotBlockError::ManifestIdentityMismatch);
    }
    insert_block(blocks, total_block_bytes, schema)?;

    let mut chunk_ids = Vec::new();
    for chunk_value in snapshot.chunk_values() {
        let chunk = StoreSnapshotBlockV2::build(
            StoreSnapshotBlockKindV2::RowChunk,
            deterministic_cbor::encode(chunk_value)?,
        )?;
        chunk_ids.push(chunk.declared_identity);
        insert_block(blocks, total_block_bytes, chunk)?;
    }
    if chunk_ids
        != snapshot
            .chunk_ids()
            .iter()
            .map(|identity| *identity.as_bytes())
            .collect::<Vec<_>>()
    {
        return Err(StoreSnapshotBlockError::ManifestIdentityMismatch);
    }

    let mut family_ids = Vec::new();
    for family_value in snapshot.family_manifest_values() {
        let family = StoreSnapshotBlockV2::build(
            StoreSnapshotBlockKindV2::FamilyManifest,
            deterministic_cbor::encode(family_value)?,
        )?;
        family_ids.push(family.declared_identity);
        insert_block(blocks, total_block_bytes, family)?;
    }
    if family_ids
        != snapshot
            .family_manifest_ids()
            .iter()
            .map(|identity| *identity.as_bytes())
            .collect::<Vec<_>>()
    {
        return Err(StoreSnapshotBlockError::ManifestIdentityMismatch);
    }

    for (declared_id, bytes) in snapshot.object_blobs() {
        let object = StoreSnapshotBlockV2::build(
            StoreSnapshotBlockKindV2::StoreObjectPayload,
            bytes.clone(),
        )?;
        if declared_id.as_slice() != object.declared_identity.as_slice() {
            return Err(StoreSnapshotBlockError::DeclaredIdentityMismatch {
                kind: StoreSnapshotBlockKindV2::StoreObjectPayload,
            });
        }
        insert_block(blocks, total_block_bytes, object)?;
    }

    let root = StoreSnapshotBlockV2::build(
        StoreSnapshotBlockKindV2::SnapshotRoot,
        deterministic_cbor::encode(snapshot.flat_root_value())?,
    )?;
    if root.declared_identity != *snapshot.id().as_bytes() {
        return Err(StoreSnapshotBlockError::FlatRootIdentityMismatch);
    }
    insert_block(blocks, total_block_bytes, root)
}

fn expected_reachable_closure(
    current_root_id: [u8; 32],
    available: &BTreeMap<BlockKey, StoreSnapshotBlockV2>,
) -> Result<ReachableSnapshotClosure, StoreSnapshotBlockError> {
    let mut expected = BTreeMap::new();
    let mut total_block_bytes = 0;
    let mut visiting = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut snapshots = BTreeMap::new();
    let mut stack = vec![PriorTraversalFrame::Enter {
        root_id: current_root_id,
        parent_root_id: None,
    }];
    while let Some(frame) = stack.pop() {
        match frame {
            PriorTraversalFrame::Exit(root_id) => {
                visiting.remove(&root_id);
                completed.insert(root_id);
            }
            PriorTraversalFrame::Enter {
                root_id,
                parent_root_id,
            } => {
                if visiting.contains(&root_id) {
                    return Err(StoreSnapshotBlockError::PriorClosureCycle(root_id));
                }
                if completed.contains(&root_id) {
                    if let Some(parent_root_id) = parent_root_id {
                        validate_parent_prior_binding(parent_root_id, root_id, &snapshots)?;
                    }
                    continue;
                }
                if snapshots.len() == MAX_SNAPSHOT_BLOCKS_V2 {
                    return Err(StoreSnapshotBlockError::PriorTraversalLimitExceeded);
                }
                let reconstructed = reconstruct_snapshot(root_id, available)?;
                if let Some(parent_root_id) = parent_root_id {
                    let parent = snapshots
                        .get(&parent_root_id)
                        .ok_or(StoreSnapshotBlockError::PriorRootBindingMismatch(root_id))?;
                    parent
                        .snapshot
                        .validate_prior_root_binding(&reconstructed.snapshot)
                        .map_err(|_| StoreSnapshotBlockError::PriorRootBindingMismatch(root_id))?;
                }
                for key in &reconstructed.direct_keys {
                    let block =
                        available
                            .get(key)
                            .ok_or(StoreSnapshotBlockError::MissingBlock {
                                kind: key.0,
                                identity: key.1,
                            })?;
                    insert_block(&mut expected, &mut total_block_bytes, block.clone())?;
                }
                let prior_root_ids = reconstructed.snapshot.prior_root_ids().to_vec();
                visiting.insert(root_id);
                snapshots.insert(root_id, reconstructed);
                stack.push(PriorTraversalFrame::Exit(root_id));
                for prior_root_id in prior_root_ids.into_iter().rev() {
                    stack.push(PriorTraversalFrame::Enter {
                        root_id: prior_root_id,
                        parent_root_id: Some(root_id),
                    });
                }
                if stack.len() > MAX_SNAPSHOT_BLOCKS_V2.saturating_mul(2) {
                    return Err(StoreSnapshotBlockError::PriorTraversalLimitExceeded);
                }
            }
        }
    }
    if !snapshots.contains_key(&current_root_id) {
        return Err(StoreSnapshotBlockError::MissingCurrentRoot(current_root_id));
    }
    Ok((expected, snapshots))
}

enum PriorTraversalFrame {
    Enter {
        root_id: [u8; 32],
        parent_root_id: Option<[u8; 32]>,
    },
    Exit([u8; 32]),
}

fn validate_parent_prior_binding(
    parent_root_id: [u8; 32],
    prior_root_id: [u8; 32],
    snapshots: &BTreeMap<[u8; 32], ReconstructedSnapshot>,
) -> Result<(), StoreSnapshotBlockError> {
    let parent =
        snapshots
            .get(&parent_root_id)
            .ok_or(StoreSnapshotBlockError::PriorRootBindingMismatch(
                prior_root_id,
            ))?;
    let prior =
        snapshots
            .get(&prior_root_id)
            .ok_or(StoreSnapshotBlockError::PriorRootBindingMismatch(
                prior_root_id,
            ))?;
    parent
        .snapshot
        .validate_prior_root_binding(&prior.snapshot)
        .map_err(|_| StoreSnapshotBlockError::PriorRootBindingMismatch(prior_root_id))
}

fn reachable_blocks_from_reconstruction_cache(
    root_id: [u8; 32],
    available: &BTreeMap<BlockKey, StoreSnapshotBlockV2>,
    snapshots: &BTreeMap<[u8; 32], ReconstructedSnapshot>,
) -> Result<BTreeMap<BlockKey, StoreSnapshotBlockV2>, StoreSnapshotBlockError> {
    let mut reachable = BTreeMap::new();
    let mut total_block_bytes = 0;
    let mut completed = BTreeSet::new();
    let mut stack = vec![root_id];
    while let Some(root_id) = stack.pop() {
        if !completed.insert(root_id) {
            continue;
        }
        let reconstructed = snapshots
            .get(&root_id)
            .ok_or(StoreSnapshotBlockError::MissingPriorClosure(root_id))?;
        for key in &reconstructed.direct_keys {
            let block = available
                .get(key)
                .ok_or(StoreSnapshotBlockError::MissingBlock {
                    kind: key.0,
                    identity: key.1,
                })?;
            insert_block(&mut reachable, &mut total_block_bytes, block.clone())?;
        }
        stack.extend(reconstructed.snapshot.prior_root_ids().iter().copied());
        if stack.len() > MAX_SNAPSHOT_BLOCKS_V2 {
            return Err(StoreSnapshotBlockError::PriorTraversalLimitExceeded);
        }
    }
    Ok(reachable)
}

fn reconstruct_snapshot(
    root_id: [u8; 32],
    blocks: &BTreeMap<BlockKey, StoreSnapshotBlockV2>,
) -> Result<ReconstructedSnapshot, StoreSnapshotBlockError> {
    let root_key = (StoreSnapshotBlockKindV2::SnapshotRoot, root_id);
    let root = blocks
        .get(&root_key)
        .ok_or(StoreSnapshotBlockError::MissingCurrentRoot(root_id))?;
    let root_value = canonical_block_value(root.canonical_bytes())?;
    let reference = decode_flat_root(&root_value)?;
    let mut direct_keys = BTreeSet::from([root_key]);

    let schema_key = (
        StoreSnapshotBlockKindV2::SchemaManifest,
        reference.schema_manifest_id,
    );
    let schema = blocks
        .get(&schema_key)
        .ok_or(StoreSnapshotBlockError::MissingBlock {
            kind: schema_key.0,
            identity: schema_key.1,
        })?;
    direct_keys.insert(schema_key);
    let schema_value = canonical_block_value(schema.canonical_bytes())?;

    let mut families = Vec::new();
    for family_id in &reference.family_manifest_ids {
        let family_key = (StoreSnapshotBlockKindV2::FamilyManifest, *family_id);
        let family = blocks
            .get(&family_key)
            .ok_or(StoreSnapshotBlockError::MissingBlock {
                kind: family_key.0,
                identity: family_key.1,
            })?;
        direct_keys.insert(family_key);
        let (value, chunks) = reconstruct_family(family, &schema_value, blocks)?;
        direct_keys.extend(chunks);
        families.push(value);
    }

    let mut objects = Vec::new();
    for object_id in &reference.object_ids {
        let object_key = (StoreSnapshotBlockKindV2::StoreObjectPayload, *object_id);
        let object = blocks
            .get(&object_key)
            .ok_or(StoreSnapshotBlockError::MissingBlock {
                kind: object_key.0,
                identity: object_key.1,
            })?;
        direct_keys.insert(object_key);
        objects.push(CborValue::Array(vec![
            CborValue::Bytes(object_id.to_vec()),
            CborValue::Bytes(object.canonical_bytes().to_vec()),
        ]));
    }
    let semantic = CborValue::Array(vec![
        CborValue::Unsigned(SEMANTIC_SNAPSHOT_VERSION_V1),
        CborValue::Unsigned(reference.role),
        CborValue::Bytes(reference.domain_id.to_vec()),
        schema_value,
        CborValue::Array(families),
        CborValue::Array(objects),
    ]);
    let snapshot = StoreSnapshotRootV1::decode(&semantic)
        .map_err(|error| StoreSnapshotBlockError::Snapshot(error.to_string()))?;
    if source_pointer_commitment(snapshot.rows())? != reference.source_pointer_commitment {
        return Err(StoreSnapshotBlockError::SourcePointerCommitmentMismatch);
    }
    if snapshot.flat_root_value() != &root_value
        || *snapshot.id().as_bytes() != root_id
        || snapshot.publication_clock() != reference.publication_clock
        || snapshot.prior_root_ids() != reference.prior_root_ids
        || *snapshot.family_manifest_set_digest() != reference.family_manifest_set_digest
        || *snapshot.payload_set_digest() != reference.payload_set_digest
    {
        return Err(StoreSnapshotBlockError::FlatRootReconstructionMismatch);
    }
    Ok(ReconstructedSnapshot {
        snapshot,
        direct_keys,
    })
}

fn reconstruct_family(
    block: &StoreSnapshotBlockV2,
    schema: &CborValue,
    blocks: &BTreeMap<BlockKey, StoreSnapshotBlockV2>,
) -> Result<(CborValue, BTreeSet<BlockKey>), StoreSnapshotBlockError> {
    let reference = decode_flat_family(&canonical_block_value(block.canonical_bytes())?)?;
    if family_table_manifest(schema, reference.family_tag)? != reference.table_manifest {
        return Err(StoreSnapshotBlockError::FamilyTableManifestMismatch);
    }
    let mut chunks = Vec::new();
    let mut chunk_keys = BTreeSet::new();
    for chunk_id in reference.chunk_ids {
        let key = (StoreSnapshotBlockKindV2::RowChunk, chunk_id);
        let chunk = blocks
            .get(&key)
            .ok_or(StoreSnapshotBlockError::MissingBlock {
                kind: key.0,
                identity: key.1,
            })?;
        chunk_keys.insert(key);
        chunks.push(canonical_block_value(chunk.canonical_bytes())?);
    }
    Ok((
        CborValue::Array(vec![
            CborValue::Unsigned(SEMANTIC_FAMILY_MANIFEST_VERSION_V1),
            CborValue::Unsigned(reference.family_tag),
            CborValue::Text(reference.name),
            CborValue::Text(reference.classification),
            CborValue::Unsigned(reference.row_count),
            CborValue::Bytes(reference.set_root.to_vec()),
            CborValue::Array(chunks),
        ]),
        chunk_keys,
    ))
}

struct FlatRootReference {
    role: u64,
    domain_id: [u8; 32],
    schema_manifest_id: [u8; 32],
    family_manifest_ids: Vec<[u8; 32]>,
    object_ids: Vec<[u8; 32]>,
    prior_root_ids: Vec<[u8; 32]>,
    publication_clock: u64,
    source_pointer_commitment: [u8; 32],
    family_manifest_set_digest: [u8; 32],
    payload_set_digest: [u8; 32],
}

fn decode_flat_root(value: &CborValue) -> Result<FlatRootReference, StoreSnapshotBlockError> {
    let CborValue::Array(fields) = value else {
        return Err(StoreSnapshotBlockError::InvalidFlatRootShape);
    };
    let [
        CborValue::Unsigned(version),
        CborValue::Unsigned(role),
        CborValue::Bytes(domain_id),
        CborValue::Bytes(schema_manifest_id),
        CborValue::Array(family_manifest_ids),
        CborValue::Array(object_ids),
        CborValue::Array(prior_root_ids),
        CborValue::Unsigned(publication_clock),
        CborValue::Bytes(source_pointer_commitment),
        CborValue::Bytes(family_manifest_set_digest),
        CborValue::Bytes(payload_set_digest),
    ] = fields.as_slice()
    else {
        return Err(StoreSnapshotBlockError::InvalidFlatRootShape);
    };
    if *version != FLAT_SNAPSHOT_ROOT_VERSION_V2 {
        return Err(StoreSnapshotBlockError::InvalidFlatRootShape);
    }
    let family_manifest_ids = decode_unique_ids(family_manifest_ids, "family manifest ids")?;
    let object_ids = decode_unique_ids(object_ids, "Store Object payload ids")?;
    let prior_root_ids = decode_unique_ids(prior_root_ids, "prior snapshot root ids")?;
    if prior_root_ids.len() > MAX_REFERENCED_PRIOR_ROOTS_V1 {
        return Err(StoreSnapshotBlockError::PriorRootCountLimitExceeded);
    }
    if object_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || prior_root_ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(StoreSnapshotBlockError::IdentityArrayOrderMismatch);
    }
    Ok(FlatRootReference {
        role: *role,
        domain_id: digest32(domain_id, "snapshot domain identity")?,
        schema_manifest_id: digest32(schema_manifest_id, "schema manifest identity")?,
        family_manifest_ids,
        object_ids,
        prior_root_ids,
        publication_clock: *publication_clock,
        source_pointer_commitment: digest32(
            source_pointer_commitment,
            "source pointer commitment",
        )?,
        family_manifest_set_digest: digest32(
            family_manifest_set_digest,
            "family manifest set digest",
        )?,
        payload_set_digest: digest32(payload_set_digest, "payload set digest")?,
    })
}

struct FlatFamilyReference {
    family_tag: u64,
    name: String,
    classification: String,
    row_count: u64,
    set_root: [u8; 32],
    table_manifest: CborValue,
    chunk_ids: Vec<[u8; 32]>,
}

fn decode_flat_family(value: &CborValue) -> Result<FlatFamilyReference, StoreSnapshotBlockError> {
    let CborValue::Array(fields) = value else {
        return Err(StoreSnapshotBlockError::InvalidFlatFamilyShape);
    };
    let [
        CborValue::Unsigned(version),
        CborValue::Unsigned(family_tag),
        CborValue::Text(name),
        CborValue::Text(classification),
        CborValue::Unsigned(row_count),
        CborValue::Bytes(set_root),
        table_manifest,
        CborValue::Array(chunk_ids),
    ] = fields.as_slice()
    else {
        return Err(StoreSnapshotBlockError::InvalidFlatFamilyShape);
    };
    if *version != FLAT_FAMILY_MANIFEST_VERSION_V2 {
        return Err(StoreSnapshotBlockError::InvalidFlatFamilyShape);
    }
    Ok(FlatFamilyReference {
        family_tag: *family_tag,
        name: name.clone(),
        classification: classification.clone(),
        row_count: *row_count,
        set_root: digest32(set_root, "family row set root")?,
        table_manifest: table_manifest.clone(),
        chunk_ids: decode_unique_ids(chunk_ids, "row chunk ids")?,
    })
}

fn family_table_manifest(
    schema: &CborValue,
    family_tag: u64,
) -> Result<CborValue, StoreSnapshotBlockError> {
    let CborValue::Array(fields) = schema else {
        return Err(StoreSnapshotBlockError::InvalidSchemaManifestShape);
    };
    let [_, _, _, _, _, CborValue::Array(tables)] = fields.as_slice() else {
        return Err(StoreSnapshotBlockError::InvalidSchemaManifestShape);
    };
    let selected = tables
        .iter()
        .filter_map(|table| {
            let CborValue::Array(table_fields) = table else {
                return Some(Err(StoreSnapshotBlockError::InvalidSchemaManifestShape));
            };
            let [CborValue::Array(descriptor), _] = table_fields.as_slice() else {
                return Some(Err(StoreSnapshotBlockError::InvalidSchemaManifestShape));
            };
            match descriptor.get(1) {
                Some(CborValue::Unsigned(tag)) if *tag == family_tag => Some(Ok(table.clone())),
                Some(CborValue::Unsigned(_)) => None,
                _ => Some(Err(StoreSnapshotBlockError::InvalidSchemaManifestShape)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if selected.is_empty() {
        return Err(StoreSnapshotBlockError::FamilyTableManifestMismatch);
    }
    Ok(CborValue::Array(selected))
}

fn source_pointer_commitment(
    rows: &[StoreSnapshotRowV1],
) -> Result<[u8; 32], StoreSnapshotBlockError> {
    let value = CborValue::Array(
        rows.iter()
            .filter(|row| {
                matches!(
                    row.family(),
                    StoreSnapshotFamilyV1::StoreIdentity | StoreSnapshotFamilyV1::SourcePointers
                )
            })
            .map(StoreSnapshotRowV1::to_canonical_value)
            .collect(),
    );
    Ok(Sha256::digest(deterministic_cbor::encode(&value)?).into())
}

fn derive_block_identity(
    kind: StoreSnapshotBlockKindV2,
    bytes: &[u8],
) -> Result<[u8; 32], StoreSnapshotBlockError> {
    match kind {
        StoreSnapshotBlockKindV2::SchemaManifest => {
            let identity: StoreSchemaManifestIdV1 =
                derive_identity(&canonical_block_value(bytes)?)?;
            Ok(*identity.as_bytes())
        }
        StoreSnapshotBlockKindV2::FamilyManifest => {
            decode_flat_family(&canonical_block_value(bytes)?)?;
            let identity: StoreExportFamilyManifestIdV1 =
                derive_identity(&canonical_block_value(bytes)?)?;
            Ok(*identity.as_bytes())
        }
        StoreSnapshotBlockKindV2::RowChunk => {
            let identity: StoreExportChunkIdV1 = derive_identity(&canonical_block_value(bytes)?)?;
            Ok(*identity.as_bytes())
        }
        StoreSnapshotBlockKindV2::SnapshotRoot => {
            decode_flat_root(&canonical_block_value(bytes)?)?;
            let identity: StoreSnapshotRootIdV1 = derive_identity(&canonical_block_value(bytes)?)?;
            Ok(*identity.as_bytes())
        }
        StoreSnapshotBlockKindV2::StoreObjectPayload => {
            let object = StoreObjectV1::decode(bytes)
                .map_err(|error| StoreSnapshotBlockError::StoreObject(error.to_string()))?;
            let identity: StoreObjectIdV1 = object.id();
            Ok(*identity.as_bytes())
        }
    }
}

fn validate_prior_closure_input_bounds(
    prior_closures: &[StoreSnapshotBlockClosureV2],
) -> Result<(), StoreSnapshotBlockError> {
    validate_prior_closure_lengths(
        prior_closures
            .iter()
            .map(|closure| closure.canonical_bytes().len()),
        MAX_REFERENCED_PRIOR_ROOTS_V1,
        MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2,
    )
}

fn validate_prior_closure_lengths(
    lengths: impl IntoIterator<Item = usize>,
    count_limit: usize,
    aggregate_bytes_limit: usize,
) -> Result<(), StoreSnapshotBlockError> {
    let mut aggregate_bytes = 0_usize;
    for (count, length) in lengths.into_iter().enumerate() {
        if count >= count_limit {
            return Err(StoreSnapshotBlockError::PriorClosureCountLimitExceeded);
        }
        aggregate_bytes = aggregate_bytes
            .checked_add(length)
            .ok_or(StoreSnapshotBlockError::PriorClosureBytesLimitExceeded)?;
        if aggregate_bytes > aggregate_bytes_limit {
            return Err(StoreSnapshotBlockError::PriorClosureBytesLimitExceeded);
        }
    }
    Ok(())
}

fn validate_declared_artifact_lengths(
    lengths: impl IntoIterator<Item = u64>,
    aggregate_bytes_limit: u64,
) -> Result<(), StoreSnapshotBlockError> {
    let mut aggregate_bytes = 0_u64;
    for length in lengths {
        aggregate_bytes = aggregate_bytes
            .checked_add(length)
            .ok_or(StoreSnapshotBlockError::PriorExportReconstructionLimitExceeded)?;
        if aggregate_bytes > aggregate_bytes_limit {
            return Err(StoreSnapshotBlockError::PriorExportReconstructionLimitExceeded);
        }
    }
    Ok(())
}

fn index_prior_closures(
    prior_closures: &[StoreSnapshotBlockClosureV2],
) -> Result<BTreeMap<[u8; 32], &StoreSnapshotBlockClosureV2>, StoreSnapshotBlockError> {
    let mut indexed = BTreeMap::new();
    for closure in prior_closures {
        let verified = StoreSnapshotBlockClosureV2::decode(closure.canonical_bytes())?;
        if verified != *closure {
            return Err(StoreSnapshotBlockError::CorruptPriorClosure(
                closure.current_root_id,
            ));
        }
        if indexed.insert(closure.current_root_id, closure).is_some() {
            return Err(StoreSnapshotBlockError::DuplicatePriorClosure(
                closure.current_root_id,
            ));
        }
    }
    Ok(indexed)
}

fn insert_block(
    blocks: &mut BTreeMap<BlockKey, StoreSnapshotBlockV2>,
    total_block_bytes: &mut usize,
    block: StoreSnapshotBlockV2,
) -> Result<(), StoreSnapshotBlockError> {
    let key = block.key();
    if let Some(existing) = blocks.get(&key) {
        if existing != &block {
            return Err(StoreSnapshotBlockError::MismatchedBlock {
                kind: key.0,
                identity: key.1,
            });
        }
        return Ok(());
    }
    if blocks.len() == MAX_SNAPSHOT_BLOCKS_V2 {
        return Err(StoreSnapshotBlockError::BlockCountLimitExceeded);
    }
    let next_total = total_block_bytes
        .checked_add(block.canonical_bytes.len())
        .ok_or(StoreSnapshotBlockError::ClosureBytesLimitExceeded)?;
    if next_total > MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 {
        return Err(StoreSnapshotBlockError::ClosureBytesLimitExceeded);
    }
    blocks.insert(key, block);
    *total_block_bytes = next_total;
    Ok(())
}

fn decode_unique_ids(
    values: &[CborValue],
    field: &'static str,
) -> Result<Vec<[u8; 32]>, StoreSnapshotBlockError> {
    let ids = values
        .iter()
        .map(|value| match value {
            CborValue::Bytes(bytes) => digest32(bytes, field),
            _ => Err(StoreSnapshotBlockError::InvalidIdentityArray(field)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(StoreSnapshotBlockError::DuplicateIdentity(field));
    }
    Ok(ids)
}

fn canonical_block_value(bytes: &[u8]) -> Result<CborValue, StoreSnapshotBlockError> {
    let value = deterministic_cbor::decode(bytes)?;
    if deterministic_cbor::encode(&value)? != bytes {
        return Err(StoreSnapshotBlockError::NonCanonicalBlock);
    }
    Ok(value)
}

fn digest32(bytes: &[u8], field: &'static str) -> Result<[u8; 32], StoreSnapshotBlockError> {
    let actual = bytes.len();
    bytes
        .try_into()
        .map_err(|_| StoreSnapshotBlockError::InvalidDigestLength { field, actual })
}

fn validate_block_byte_length(length: usize) -> Result<(), StoreSnapshotBlockError> {
    if length == 0 || length > MAX_SNAPSHOT_BLOCK_BYTES_V2 {
        Err(StoreSnapshotBlockError::BlockBytesLimitExceeded)
    } else {
        Ok(())
    }
}

fn validate_total_block_bytes(
    blocks: &[StoreSnapshotBlockV2],
) -> Result<(), StoreSnapshotBlockError> {
    let mut total = 0_usize;
    for block in blocks {
        total = total
            .checked_add(block.canonical_bytes.len())
            .ok_or(StoreSnapshotBlockError::ClosureBytesLimitExceeded)?;
        if total > MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 {
            return Err(StoreSnapshotBlockError::ClosureBytesLimitExceeded);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum StoreSnapshotBlockError {
    #[error("unknown Store snapshot block kind tag {0}")]
    UnknownBlockKind(u64),
    #[error("unsupported Store snapshot block closure version {0}")]
    UnknownClosureVersion(u64),
    #[error("Store snapshot block has an invalid canonical shape")]
    InvalidBlockShape,
    #[error("Store snapshot block closure has an invalid canonical shape")]
    InvalidClosureShape,
    #[error("flat snapshot root has an invalid canonical shape")]
    InvalidFlatRootShape,
    #[error("flat family manifest has an invalid canonical shape")]
    InvalidFlatFamilyShape,
    #[error("Store snapshot schema manifest has an invalid canonical shape")]
    InvalidSchemaManifestShape,
    #[error("Store snapshot block closure contains too many or zero blocks")]
    BlockCountLimitExceeded,
    #[error("Store snapshot block exceeds its finite byte limit")]
    BlockBytesLimitExceeded,
    #[error("Store snapshot block closure exceeds its finite byte limit")]
    ClosureBytesLimitExceeded,
    #[error("Store snapshot block order is not strict canonical kind-and-identity order")]
    BlockOrderMismatch,
    #[error("Store snapshot block closure contains a duplicate block")]
    DuplicateBlock,
    #[error("Store snapshot block closure contains missing or extra blocks")]
    ClosureSetMismatch,
    #[error("Store snapshot block is not canonical")]
    NonCanonicalBlock,
    #[error("Store snapshot block closure is not canonical")]
    NonCanonicalClosure,
    #[error("Store snapshot block {kind:?} declared identity does not match its bytes")]
    DeclaredIdentityMismatch { kind: StoreSnapshotBlockKindV2 },
    #[error("Store snapshot block {kind:?} content digest does not match its bytes")]
    ContentDigestMismatch { kind: StoreSnapshotBlockKindV2 },
    #[error("flat snapshot root identity does not match StoreSnapshotRootV1")]
    FlatRootIdentityMismatch,
    #[error("flat snapshot root does not exactly reconstruct StoreSnapshotRootV1")]
    FlatRootReconstructionMismatch,
    #[error("flat snapshot manifest identities do not match StoreSnapshotRootV1")]
    ManifestIdentityMismatch,
    #[error("flat family table manifest does not match the schema manifest")]
    FamilyTableManifestMismatch,
    #[error("flat snapshot source pointer commitment does not match typed rows")]
    SourcePointerCommitmentMismatch,
    #[error("Store snapshot block {kind:?} {identity:?} is missing")]
    MissingBlock {
        kind: StoreSnapshotBlockKindV2,
        identity: [u8; 32],
    },
    #[error("Store snapshot block {kind:?} {identity:?} conflicts with its declaration")]
    MismatchedBlock {
        kind: StoreSnapshotBlockKindV2,
        identity: [u8; 32],
    },
    #[error("Store snapshot closure is missing current root {0:?}")]
    MissingCurrentRoot([u8; 32]),
    #[error("Store snapshot closure is missing referenced prior root {0:?}")]
    MissingPriorClosure([u8; 32]),
    #[error("Store snapshot prior closure set has missing or extra roots")]
    PriorClosureSetMismatch,
    #[error("Store snapshot references more than 1,024 prior roots")]
    PriorRootCountLimitExceeded,
    #[error("Store snapshot was supplied more than 1,024 prior closures")]
    PriorClosureCountLimitExceeded,
    #[error("supplied prior Store snapshot closures exceed the 15 MiB aggregate byte limit")]
    PriorClosureBytesLimitExceeded,
    #[error("Store snapshot prior closure root {0:?} is duplicated")]
    DuplicatePriorClosure([u8; 32]),
    #[error("Store snapshot prior closure root {0:?} is corrupt")]
    CorruptPriorClosure([u8; 32]),
    #[error("Store snapshot prior closure graph contains a cycle at {0:?}")]
    PriorClosureCycle([u8; 32]),
    #[error("Store snapshot prior closure graph exceeds its finite traversal bound")]
    PriorTraversalLimitExceeded,
    #[error("Store snapshot prior root {0:?} does not match its naming sealed-export row")]
    PriorRootBindingMismatch([u8; 32]),
    #[error("prior sealed export could not be reconstructed from its exact Store snapshot: {0}")]
    PriorExportReconstruction(String),
    #[error("prior sealed-export reconstruction exceeds the finite verification byte budget")]
    PriorExportReconstructionLimitExceeded,
    #[error(
        "Store snapshot contains duplicate sealed-export artifact receipts for prior root {0:?}"
    )]
    DuplicatePriorExportArtifactRoot([u8; 32]),
    #[error(
        "prior Store snapshot root {0:?} does not reproduce its sealed-export artifact receipt"
    )]
    PriorExportArtifactMismatch([u8; 32]),
    #[error("Store snapshot identity array {0} has an invalid element")]
    InvalidIdentityArray(&'static str),
    #[error("Store snapshot identity array {0} contains a duplicate")]
    DuplicateIdentity(&'static str),
    #[error("Store snapshot identity array is not in required canonical order")]
    IdentityArrayOrderMismatch,
    #[error("Store snapshot block digest field {field} has {actual} bytes; expected 32")]
    InvalidDigestLength { field: &'static str, actual: usize },
    #[error("Store snapshot root is invalid: {0}")]
    Snapshot(String),
    #[error("Store Object block is invalid: {0}")]
    StoreObject(String),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::domain::identity::{ContractRootIdV1, SchemaIdV1};

    use super::super::{
        StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreObjectV1, StoreRoleV1, StoreV1,
    };
    use super::*;

    static NEXT_SNAPSHOT_BLOCK_TEST: AtomicU64 = AtomicU64::new(0);

    struct TestStorePath(PathBuf);

    impl TestStorePath {
        fn new() -> Self {
            let sequence = NEXT_SNAPSHOT_BLOCK_TEST.fetch_add(1, Ordering::Relaxed);
            let root = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            Self(root.join(format!(
                "maestro-vnext-snapshot-block-unit-{}-{sequence}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestStorePath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rendered(byte: u8) -> String {
        format!("sha256:{}", format!("{byte:02x}").repeat(32))
    }

    fn export_pair() -> (
        TestStorePath,
        super::super::SealedExportV1,
        super::super::SealedExportV1,
    ) {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"artifact-provenance")
            .expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let object = StoreObjectV1::new(
            SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
            CborValue::Array(vec![CborValue::Unsigned(1)]),
            vec![],
        )
        .expect("Store Object");
        store.put_object(&object).expect("persist object");
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![object.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");
        let first = store.seal_export().expect("first canonical export");
        let second = store.seal_export().expect("second canonical export");
        (path, first.export().clone(), second.export().clone())
    }

    fn assert_rewritten_artifact_rejected(
        replacement_export_id: Option<[u8; 32]>,
        replacement_byte_length: Option<u64>,
        replacement_bytes_digest: Option<[u8; 32]>,
    ) {
        let (_path, first, second) = export_pair();
        let first_root_id = *first
            .snapshot_root()
            .expect("first snapshot root")
            .id()
            .as_bytes();
        let mutated = second
            .snapshot_root()
            .expect("second snapshot root")
            .with_rewritten_sealed_export_artifact(
                first_root_id,
                replacement_export_id,
                replacement_byte_length,
                replacement_bytes_digest,
            )
            .expect("canonically rehashed mutated snapshot");
        let prior = first
            .snapshot_blocks()
            .expect("first snapshot block closure")
            .clone();

        assert!(matches!(
            StoreSnapshotBlockClosureV2::build(&mutated, &[prior]),
            Err(StoreSnapshotBlockError::PriorExportArtifactMismatch(root_id))
                if root_id == first_root_id
        ));
    }

    #[test]
    fn prior_export_id_must_match_the_reconstructed_canonical_export() {
        assert_rewritten_artifact_rejected(Some([0x91; 32]), None, None);
    }

    #[test]
    fn prior_export_byte_length_must_match_the_reconstructed_canonical_export() {
        assert_rewritten_artifact_rejected(None, Some(1), None);
    }

    #[test]
    fn prior_export_bytes_digest_must_match_the_reconstructed_canonical_export() {
        assert_rewritten_artifact_rejected(None, None, Some([0x92; 32]));
    }

    #[test]
    fn prior_receipt_cannot_commit_after_its_parent_snapshot_cut() {
        let (_path, first, second) = export_pair();
        let first_root = first.snapshot_root().expect("first snapshot root");
        let mutated_parent = second
            .snapshot_root()
            .expect("second snapshot root")
            .with_rewritten_publication_clock(first_root.publication_clock())
            .expect("canonically rehashed equality-clock parent");
        let prior = first
            .snapshot_blocks()
            .expect("first snapshot block closure")
            .clone();

        assert!(matches!(
            StoreSnapshotBlockClosureV2::build(&mutated_parent, &[prior]),
            Err(StoreSnapshotBlockError::PriorRootBindingMismatch(root_id))
                if root_id == *first_root.id().as_bytes()
        ));
    }

    #[test]
    fn snapshot_block_limits_are_frozen_and_fit_the_cbor_byte_string_budget() {
        assert_eq!(MAX_SNAPSHOT_BLOCKS_V2, 65_536);
        assert_eq!(MAX_SNAPSHOT_BLOCK_BYTES_V2, 15 * 1024 * 1024);
        assert_eq!(MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2, 15 * 1024 * 1024);
        assert_eq!(MAX_PRIOR_EXPORT_RECONSTRUCTION_BYTES_V2, 512 * 1024 * 1024);
    }

    #[test]
    fn injected_prior_closure_limits_reject_count_and_aggregate_bytes() {
        assert_eq!(validate_prior_closure_lengths([2, 2], 2, 4), Ok(()));
        assert_eq!(
            validate_prior_closure_lengths([1, 1, 1], 2, 4),
            Err(StoreSnapshotBlockError::PriorClosureCountLimitExceeded)
        );
        assert_eq!(
            validate_prior_closure_lengths([2, 3], 2, 4),
            Err(StoreSnapshotBlockError::PriorClosureBytesLimitExceeded)
        );
    }

    #[test]
    fn injected_declared_artifact_budget_fails_before_reconstruction() {
        assert_eq!(validate_declared_artifact_lengths([2, 3], 5), Ok(()));
        assert_eq!(
            validate_declared_artifact_lengths([2, 4], 5),
            Err(StoreSnapshotBlockError::PriorExportReconstructionLimitExceeded)
        );
        assert_eq!(
            validate_declared_artifact_lengths([u64::MAX, 1], u64::MAX),
            Err(StoreSnapshotBlockError::PriorExportReconstructionLimitExceeded)
        );
    }
}
