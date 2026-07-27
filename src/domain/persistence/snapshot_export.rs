use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::domain::identity::{
    ContractRootIdV1, DescriptorIdV1, ManifestIdV1, ManifestIdentityV1, SchemaIdV1,
    StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};

use super::snapshot_blocks::StoreSnapshotBlockClosureV2;
use super::snapshot_rows::{ReachabilityStatusV1, StoreSnapshotRowV1};
use super::{
    ExportError, LogicalTombstoneV1, ReachabilitySnapshotV1, RetentionPinV1, RetentionRootKindV1,
    RetentionRootV1, STORE_OBJECT_STORAGE_CODEC_V1, SealedExportEntryV1, SealedExportLineageV1,
    SealedExportV1, StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreHeadV1,
    StoreObjectV1, StoreSnapshotRootV1, TombstonedObjectV1,
};

pub(crate) fn reconstruct_prior_export(
    snapshot: &StoreSnapshotRootV1,
    snapshot_blocks: StoreSnapshotBlockClosureV2,
) -> Result<SealedExportV1, ExportError> {
    let lineage = reconstruct_lineage(snapshot)?;
    let reachability = reconstruct_reachability(snapshot, &lineage)?;
    let pins = reconstruct_active_pins(snapshot)?;
    let entries = reconstruct_entries(snapshot)?;
    SealedExportV1::new_full_history(lineage, reachability, pins, entries, snapshot_blocks)
}

#[derive(Clone, Copy)]
struct GenerationRow {
    id: [u8; 32],
    ordinal: u64,
    previous: Option<[u8; 32]>,
    contract_root: [u8; 32],
    writer_manifest: [u8; 32],
    association_schema: [u8; 32],
    finality_manifest: [u8; 32],
    read_write_descriptor: [u8; 32],
    writer_epoch: [u8; 32],
    migration_epoch: [u8; 32],
}

#[derive(Clone, Copy)]
struct HeadRow {
    id: [u8; 32],
    generation_id: [u8; 32],
    generation_ordinal: u64,
    revision: u64,
    previous: Option<[u8; 32]>,
}

fn reconstruct_lineage(
    snapshot: &StoreSnapshotRootV1,
) -> Result<Vec<SealedExportLineageV1>, ExportError> {
    let domain = StoreDomainV1::from_identity(snapshot.role(), snapshot.domain_id());
    let mut roots = BTreeMap::<[u8; 32], Vec<(u64, [u8; 32])>>::new();
    let mut generation_rows = Vec::new();
    let mut head_rows = Vec::new();
    for row in snapshot.rows() {
        match row {
            StoreSnapshotRowV1::Generation {
                generation_id,
                generation_ordinal,
                previous_generation_id,
                contract_root_id,
                writer_compatibility_manifest_id,
                association_schema_id,
                finality_edge_manifest_id,
                schema_read_write_set_descriptor_id,
                writer_protocol_epoch_id,
                migration_epoch_id,
            } => generation_rows.push(GenerationRow {
                id: *generation_id.as_bytes(),
                ordinal: generation_ordinal.get(),
                previous: previous_generation_id.map(|value| *value.as_bytes()),
                contract_root: *contract_root_id.as_bytes(),
                writer_manifest: *writer_compatibility_manifest_id.as_bytes(),
                association_schema: *association_schema_id.as_bytes(),
                finality_manifest: *finality_edge_manifest_id.as_bytes(),
                read_write_descriptor: *schema_read_write_set_descriptor_id.as_bytes(),
                writer_epoch: *writer_protocol_epoch_id.as_bytes(),
                migration_epoch: *migration_epoch_id.as_bytes(),
            }),
            StoreSnapshotRowV1::GenerationRoot {
                generation_id,
                root_position,
                object_id,
            } => roots
                .entry(*generation_id.as_bytes())
                .or_default()
                .push((root_position.get(), *object_id.as_bytes())),
            StoreSnapshotRowV1::Head {
                head_id,
                generation_id,
                generation_ordinal,
                head_revision,
                previous_head_id,
            } => head_rows.push(HeadRow {
                id: *head_id.as_bytes(),
                generation_id: *generation_id.as_bytes(),
                generation_ordinal: generation_ordinal.get(),
                revision: head_revision.get(),
                previous: previous_head_id.map(|value| *value.as_bytes()),
            }),
            _ => {}
        }
    }
    generation_rows.sort_by_key(|row| row.ordinal);
    head_rows.sort_by_key(|row| row.revision);
    if generation_rows.is_empty() || generation_rows.len() != head_rows.len() {
        return mismatch("prior lineage cardinality");
    }

    let mut lineage = Vec::with_capacity(generation_rows.len());
    for (generation_row, head_row) in generation_rows.into_iter().zip(head_rows) {
        if generation_row.id != head_row.generation_id
            || generation_row.ordinal != head_row.generation_ordinal
            || generation_row.ordinal != head_row.revision
        {
            return mismatch("prior lineage pairing");
        }
        let generation_roots = ordered_identities(
            roots.remove(&generation_row.id).unwrap_or_default(),
            "prior generation roots",
        )?;
        let previous_generation: Option<StoreGenerationIdV1> =
            generation_row.previous.map(ManifestIdentityV1::from_digest);
        let contract_root: ContractRootIdV1 =
            ManifestIdentityV1::from_digest(generation_row.contract_root);
        let writer_manifest: ManifestIdV1 =
            ManifestIdentityV1::from_digest(generation_row.writer_manifest);
        let association_schema: SchemaIdV1 =
            ManifestIdentityV1::from_digest(generation_row.association_schema);
        let finality_manifest: ManifestIdV1 =
            ManifestIdentityV1::from_digest(generation_row.finality_manifest);
        let read_write_descriptor: DescriptorIdV1 =
            ManifestIdentityV1::from_digest(generation_row.read_write_descriptor);
        let writer_epoch: DescriptorIdV1 =
            ManifestIdentityV1::from_digest(generation_row.writer_epoch);
        let migration_epoch: DescriptorIdV1 =
            ManifestIdentityV1::from_digest(generation_row.migration_epoch);
        let generation = StoreGenerationV1::new(
            domain.clone(),
            generation_row.ordinal,
            previous_generation,
            contract_root,
            StoreCompatibilityV1::new(
                writer_manifest,
                association_schema,
                finality_manifest,
                read_write_descriptor,
                writer_epoch,
                migration_epoch,
            ),
            generation_roots,
        )?;
        if generation.id().as_bytes() != &generation_row.id {
            return mismatch("prior Generation identity");
        }
        let previous_head: Option<StoreHeadIdV1> =
            head_row.previous.map(ManifestIdentityV1::from_digest);
        let head = StoreHeadV1::new(&generation, head_row.revision, previous_head)?;
        if head.id().as_bytes() != &head_row.id {
            return mismatch("prior Head identity");
        }
        lineage.push(SealedExportLineageV1::new(generation, head)?);
    }
    if !roots.is_empty() {
        return mismatch("orphan prior generation roots");
    }
    Ok(lineage)
}

fn reconstruct_reachability(
    snapshot: &StoreSnapshotRootV1,
    lineage: &[SealedExportLineageV1],
) -> Result<ReachabilitySnapshotV1, ExportError> {
    let head_id = lineage
        .last()
        .ok_or(ExportError::InvalidLineageLength)?
        .head()
        .id();
    let revisions = snapshot
        .rows()
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionRevision {
                retention_revision, ..
            } => Some(retention_revision.get()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [retention_revision] = revisions.as_slice() else {
        return mismatch("prior retention revision");
    };
    let bases = snapshot
        .rows()
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::ReachabilitySnapshot {
                snapshot_id,
                head_id: candidate_head,
                retention_revision: candidate_revision,
            } if candidate_head.as_bytes() == head_id.as_bytes()
                && candidate_revision.get() == *retention_revision =>
            {
                Some(*snapshot_id.as_bytes())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [snapshot_id] = bases.as_slice() else {
        return mismatch("prior reachability basis");
    };

    let mut roots = Vec::new();
    let mut reachable = Vec::new();
    let mut tombstoned = Vec::new();
    for row in snapshot.rows() {
        match row {
            StoreSnapshotRowV1::ReachabilityRoot {
                snapshot_id: candidate_snapshot,
                root_position,
                root_kind,
                object_id,
            } if candidate_snapshot.as_bytes() == snapshot_id => roots.push((
                root_position.get(),
                RetentionRootV1::new(
                    RetentionRootKindV1::from_tag(root_kind.tag())?,
                    ManifestIdentityV1::from_digest(*object_id.as_bytes()),
                ),
            )),
            StoreSnapshotRowV1::ReachabilityObject {
                snapshot_id: candidate_snapshot,
                object_id,
                reachability_status,
            } if candidate_snapshot.as_bytes() == snapshot_id => match reachability_status {
                ReachabilityStatusV1::Reachable => {
                    reachable.push(ManifestIdentityV1::from_digest(*object_id.as_bytes()))
                }
                ReachabilityStatusV1::Tombstoned => {
                    tombstoned.push(ManifestIdentityV1::from_digest(*object_id.as_bytes()))
                }
            },
            _ => {}
        }
    }
    roots.sort_by_key(|(position, _)| *position);
    if roots
        .iter()
        .enumerate()
        .any(|(expected, (actual, _))| *actual != expected as u64)
    {
        return mismatch("prior reachability root positions");
    }
    let roots = roots.into_iter().map(|(_, root)| root).collect();
    reachable.sort();
    tombstoned.sort();
    let reachability =
        ReachabilitySnapshotV1::new(head_id, *retention_revision, roots, reachable, tombstoned)?;
    if reachability.id().as_bytes() != snapshot_id {
        return mismatch("prior reachability identity");
    }
    Ok(reachability)
}

fn reconstruct_active_pins(
    snapshot: &StoreSnapshotRootV1,
) -> Result<Vec<RetentionPinV1>, ExportError> {
    let released = snapshot
        .rows()
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionPinRelease { pin_id, .. } => Some(*pin_id.as_bytes()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut pins = Vec::new();
    for row in snapshot.rows() {
        let StoreSnapshotRowV1::RetentionPin {
            pin_id,
            basis_head_id,
            root_kind,
            root_object_id,
            reason_digest,
        } = row
        else {
            continue;
        };
        if released.contains(pin_id.as_bytes()) {
            continue;
        }
        let pin = RetentionPinV1::new(
            ManifestIdentityV1::from_digest(*basis_head_id.as_bytes()),
            RetentionRootV1::new(
                RetentionRootKindV1::from_tag(root_kind.tag())?,
                ManifestIdentityV1::from_digest(*root_object_id.as_bytes()),
            ),
            *reason_digest.as_bytes(),
        )?;
        if pin.id().as_bytes() != pin_id.as_bytes() {
            return mismatch("prior retention pin identity");
        }
        pins.push(pin);
    }
    pins.sort_by_key(RetentionPinV1::id);
    Ok(pins)
}

fn reconstruct_entries(
    snapshot: &StoreSnapshotRootV1,
) -> Result<Vec<SealedExportEntryV1>, ExportError> {
    let mut references = BTreeMap::<[u8; 32], Vec<(u64, [u8; 32])>>::new();
    let mut tombstones = BTreeMap::new();
    for row in snapshot.rows() {
        match row {
            StoreSnapshotRowV1::ObjectReference {
                object_id,
                reference_position,
                referenced_object_id,
            } => references
                .entry(*object_id.as_bytes())
                .or_default()
                .push((reference_position.get(), *referenced_object_id.as_bytes())),
            StoreSnapshotRowV1::LogicalTombstone {
                tombstone_id,
                basis_head_id,
                object_id,
                reason_digest,
                invalidation_digest,
            } => {
                let tombstone = LogicalTombstoneV1::new(
                    ManifestIdentityV1::from_digest(*basis_head_id.as_bytes()),
                    ManifestIdentityV1::from_digest(*object_id.as_bytes()),
                    *reason_digest.as_bytes(),
                    *invalidation_digest.as_bytes(),
                )?;
                if tombstone.id().as_bytes() != tombstone_id.as_bytes()
                    || tombstones
                        .insert(*object_id.as_bytes(), tombstone)
                        .is_some()
                {
                    return mismatch("prior tombstone identity");
                }
            }
            _ => {}
        }
    }
    let blobs = snapshot
        .object_blobs()
        .iter()
        .map(|(id, bytes)| {
            let id: [u8; 32] = id
                .as_slice()
                .try_into()
                .map_err(|_| ExportError::InvalidIdentityLength)?;
            Ok((id, bytes.as_slice()))
        })
        .collect::<Result<BTreeMap<_, _>, ExportError>>()?;

    let mut entries = Vec::new();
    for row in snapshot.rows() {
        let StoreSnapshotRowV1::Object {
            object_id,
            schema_id,
            logical_byte_length,
            stored_byte_length,
            stored_bytes_digest,
            storage_codec,
            key_envelope_id,
            key_envelope_kind,
        } = row
        else {
            continue;
        };
        let object_id_bytes = *object_id.as_bytes();
        let object_references = ordered_identities(
            references.remove(&object_id_bytes).unwrap_or_default(),
            "prior object references",
        )?;
        if let Some(tombstone) = tombstones.remove(&object_id_bytes) {
            let key_envelope = match (key_envelope_id, key_envelope_kind) {
                (Some(id), Some(kind)) => Some((*id.as_bytes(), kind.clone())),
                (None, None) => None,
                _ => return mismatch("prior tombstoned object envelope"),
            };
            entries.push(SealedExportEntryV1::Tombstoned(TombstonedObjectV1::new(
                tombstone,
                ManifestIdentityV1::from_digest(*schema_id.as_bytes()),
                logical_byte_length.get(),
                stored_byte_length.get(),
                *stored_bytes_digest.as_bytes(),
                storage_codec.clone(),
                key_envelope,
                object_references,
            )?));
        } else {
            let bytes =
                blobs
                    .get(&object_id_bytes)
                    .ok_or(ExportError::SnapshotClosureBasisMismatch(
                        "prior available object bytes",
                    ))?;
            let object = StoreObjectV1::decode(bytes)?;
            if object.id().as_bytes() != &object_id_bytes
                || object.schema_id().as_bytes() != schema_id.as_bytes()
                || object.references() != object_references
                || logical_byte_length.get() != bytes.len() as u64
                || stored_byte_length.get() != bytes.len() as u64
                || stored_bytes_digest.as_bytes().as_slice() != Sha256::digest(bytes).as_slice()
                || storage_codec != STORE_OBJECT_STORAGE_CODEC_V1
                || key_envelope_id.is_some()
                || key_envelope_kind.is_some()
            {
                return mismatch("prior available object metadata");
            }
            entries.push(SealedExportEntryV1::Available(object));
        }
    }
    if !references.is_empty() || !tombstones.is_empty() {
        return mismatch("orphan prior object history");
    }
    entries.sort_by_key(SealedExportEntryV1::object_id);
    Ok(entries)
}

fn ordered_identities(
    mut values: Vec<(u64, [u8; 32])>,
    label: &'static str,
) -> Result<Vec<StoreObjectIdV1>, ExportError> {
    values.sort_by_key(|(position, _)| *position);
    if values
        .iter()
        .enumerate()
        .any(|(expected, (actual, _))| *actual != expected as u64)
    {
        return mismatch(label);
    }
    Ok(values
        .into_iter()
        .map(|(_, id)| ManifestIdentityV1::from_digest(id))
        .collect())
}

fn mismatch<T>(label: &'static str) -> Result<T, ExportError> {
    Err(ExportError::SnapshotClosureBasisMismatch(label))
}
