use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Transaction;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::identity::{
    ManifestIdentityV1, SchemaIdV1, StoreDomainIdV1, StoreExportChunkIdV1,
    StoreExportFamilyManifestIdV1, StoreObjectIdV1, StoreSchemaManifestIdV1, StoreSnapshotRootIdV1,
    derive_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::metadata::{
    METADATA_APPLICATION_ID, METADATA_SCHEMA_VERSION, expected_schema_digest, schema_digest,
};
use super::snapshot_restore::{SnapshotRestoreAuditBasisV1, restore_snapshot_history_v1};
use super::snapshot_rows::{
    STORE_SNAPSHOT_TABLE_MANIFEST_V1, SealedExportEntryKindV1, StoreSnapshotFamilyV1,
    StoreSnapshotRowBoundsV1, StoreSnapshotRowError, StoreSnapshotRowV1, StoreStateV1,
};
use super::{SEALED_BACKUP_FORMAT_V1, SEALED_EXPORT_FORMAT_V2, StoreDomainV1, StoreRoleV1};

const SNAPSHOT_VERSION: u64 = 1;
const SCHEMA_MANIFEST_VERSION: u64 = 1;
const FAMILY_MANIFEST_VERSION: u64 = 1;
const CHUNK_VERSION: u64 = 1;
const CBOR_PROFILE_VERSION: u64 = 1;
const MAX_ROWS_PER_CHUNK: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreSnapshotRootV1 {
    role: StoreRoleV1,
    domain_id: StoreDomainIdV1,
    schema_manifest_id: StoreSchemaManifestIdV1,
    family_manifest_ids: Vec<StoreExportFamilyManifestIdV1>,
    chunk_ids: Vec<StoreExportChunkIdV1>,
    family_manifest_set_digest: [u8; 32],
    payload_set_digest: [u8; 32],
    publication_clock: u64,
    rows: Vec<StoreSnapshotRowV1>,
    object_blobs: Vec<(Vec<u8>, Vec<u8>)>,
    id: StoreSnapshotRootIdV1,
    canonical_value: CborValue,
    flat_root_value: CborValue,
    schema_manifest_value: CborValue,
    family_manifest_values: Vec<CborValue>,
    chunk_values: Vec<CborValue>,
    prior_root_ids: Vec<[u8; 32]>,
}

struct PriorExportBasisV1 {
    head_id: [u8; 32],
    generation_id: [u8; 32],
    snapshot_id: [u8; 32],
}

impl StoreSnapshotRootV1 {
    pub(crate) fn capture(
        transaction: &Transaction<'_>,
        domain: &StoreDomainV1,
        mut load_object: impl FnMut(StoreObjectIdV1) -> Result<Vec<u8>, SnapshotError>,
    ) -> Result<Self, SnapshotError> {
        if schema_digest(transaction)? != expected_schema_digest()? {
            return Err(SnapshotError::SchemaManifestMismatch);
        }
        let rows = StoreSnapshotRowV1::load_all(transaction)?;
        let mut keyed_rows = rows
            .into_iter()
            .map(|row| Ok((row.canonical_sort_key()?, row)))
            .collect::<Result<Vec<_>, StoreSnapshotRowError>>()?;
        keyed_rows.sort_by(|left, right| left.0.cmp(&right.0));
        let rows: Vec<StoreSnapshotRowV1> = keyed_rows.into_iter().map(|(_, row)| row).collect();
        let publication_clock = rows
            .iter()
            .find_map(|row| match row {
                StoreSnapshotRowV1::PublicationClock {
                    publication_clock, ..
                } => Some(publication_clock.get()),
                _ => None,
            })
            .ok_or(SnapshotError::InvalidShape)?;
        let required = required_object_ids(&rows);
        let mut object_blobs = Vec::with_capacity(required.len());
        for object_id in required {
            object_blobs.push((
                object_id.clone(),
                load_object(identity_from_bytes(&object_id)?)?,
            ));
        }
        Self::from_parts(
            domain.role(),
            domain.id(),
            publication_clock,
            rows,
            object_blobs,
        )
    }

    pub(crate) fn decode(value: &CborValue) -> Result<Self, SnapshotError> {
        let CborValue::Array(fields) = value else {
            return Err(SnapshotError::InvalidShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Unsigned(role),
            CborValue::Bytes(domain),
            schema,
            CborValue::Array(families),
            CborValue::Array(blobs),
        ] = fields.as_slice()
        else {
            return Err(SnapshotError::InvalidShape);
        };
        if *version != SNAPSHOT_VERSION {
            return Err(SnapshotError::UnknownVersion(*version));
        }
        let role = StoreRoleV1::from_tag(*role).map_err(|_| SnapshotError::InvalidShape)?;
        let domain_id = identity_from_bytes(domain)?;
        if *schema != schema_manifest_value(expected_schema_digest()?)? {
            return Err(SnapshotError::SchemaManifestMismatch);
        }
        if families.len() != StoreSnapshotFamilyV1::ALL.len() {
            return Err(SnapshotError::FamilyManifestMismatch);
        }
        let mut rows = Vec::new();
        let mut row_bounds = StoreSnapshotRowBoundsV1::new();
        for (index, family) in families.iter().enumerate() {
            for row in decode_family(StoreSnapshotFamilyV1::ALL[index], family)? {
                row_bounds.observe(&row)?;
                rows.push(row);
            }
        }
        let publication_clock = rows
            .iter()
            .find_map(|row| match row {
                StoreSnapshotRowV1::PublicationClock {
                    publication_clock, ..
                } => Some(publication_clock.get()),
                _ => None,
            })
            .ok_or(SnapshotError::InvalidShape)?;
        let object_blobs = blobs
            .iter()
            .map(|blob| match blob {
                CborValue::Array(fields) => match fields.as_slice() {
                    [CborValue::Bytes(id), CborValue::Bytes(bytes)] => {
                        Ok((id.clone(), bytes.clone()))
                    }
                    _ => Err(SnapshotError::InvalidShape),
                },
                _ => Err(SnapshotError::InvalidShape),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rebuilt = Self::from_parts(role, domain_id, publication_clock, rows, object_blobs)?;
        if &rebuilt.canonical_value != value {
            return Err(SnapshotError::NonCanonicalSnapshot);
        }
        Ok(rebuilt)
    }

    fn from_parts(
        role: StoreRoleV1,
        domain_id: StoreDomainIdV1,
        publication_clock: u64,
        rows: Vec<StoreSnapshotRowV1>,
        mut object_blobs: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, SnapshotError> {
        validate_rows(&rows)?;
        object_blobs.sort_by(|left, right| left.0.cmp(&right.0));
        let required = required_object_ids(&rows);
        if object_blobs
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>()
            != required
            || object_blobs.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(SnapshotError::ObjectClosureMismatch);
        }
        verify_object_blobs(&rows, &object_blobs)?;
        let schema_value = schema_manifest_value(expected_schema_digest()?)?;
        let schema_manifest_id = derive_identity(&schema_value)?;
        let mut family_values = Vec::new();
        let mut family_manifest_values = Vec::new();
        let mut family_manifest_ids = Vec::new();
        let mut chunk_ids = Vec::new();
        let mut chunk_values = Vec::new();
        for family in StoreSnapshotFamilyV1::ALL {
            let family_rows = rows
                .iter()
                .filter(|row| row.family() == family)
                .cloned()
                .collect::<Vec<_>>();
            let value = family_value(family, &family_rows, &mut chunk_ids)?;
            let manifest = flat_family_manifest_value(&value, &schema_value)?;
            family_manifest_ids.push(derive_identity(&manifest)?);
            let CborValue::Array(fields) = &value else {
                return Err(SnapshotError::InvalidShape);
            };
            let Some(CborValue::Array(chunks)) = fields.last() else {
                return Err(SnapshotError::InvalidShape);
            };
            chunk_values.extend(chunks.iter().cloned());
            family_manifest_values.push(manifest);
            family_values.push(value);
        }
        let family_manifest_set_digest = digest_value(&CborValue::Array(
            family_manifest_ids
                .iter()
                .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                .collect(),
        ))?;
        let payload_set_digest = digest_value(&CborValue::Array(
            object_blobs
                .iter()
                .map(|(id, bytes)| {
                    CborValue::Array(vec![
                        CborValue::Bytes(id.clone()),
                        CborValue::Bytes(Sha256::digest(bytes).to_vec()),
                    ])
                })
                .collect(),
        ))?;
        let prior_root_ids = prior_root_ids(&rows);
        let source_pointer_commitment = digest_value(&CborValue::Array(
            rows.iter()
                .filter(|row| {
                    matches!(
                        row.family(),
                        StoreSnapshotFamilyV1::StoreIdentity
                            | StoreSnapshotFamilyV1::SourcePointers
                    )
                })
                .map(StoreSnapshotRowV1::to_canonical_value)
                .collect(),
        ))?;
        let flat_root_value = CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Unsigned(role.tag()),
            CborValue::Bytes(domain_id.as_bytes().to_vec()),
            CborValue::Bytes(schema_manifest_id.as_bytes().to_vec()),
            CborValue::Array(
                family_manifest_ids
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
            CborValue::Array(
                object_blobs
                    .iter()
                    .map(|(id, _)| CborValue::Bytes(id.clone()))
                    .collect(),
            ),
            CborValue::Array(
                prior_root_ids
                    .iter()
                    .map(|id| CborValue::Bytes(id.to_vec()))
                    .collect(),
            ),
            CborValue::Unsigned(publication_clock),
            CborValue::Bytes(source_pointer_commitment.to_vec()),
            CborValue::Bytes(family_manifest_set_digest.to_vec()),
            CborValue::Bytes(payload_set_digest.to_vec()),
        ]);
        let canonical_value = CborValue::Array(vec![
            CborValue::Unsigned(SNAPSHOT_VERSION),
            CborValue::Unsigned(role.tag()),
            CborValue::Bytes(domain_id.as_bytes().to_vec()),
            schema_value.clone(),
            CborValue::Array(family_values),
            CborValue::Array(
                object_blobs
                    .iter()
                    .map(|(id, bytes)| {
                        CborValue::Array(vec![
                            CborValue::Bytes(id.clone()),
                            CborValue::Bytes(bytes.clone()),
                        ])
                    })
                    .collect(),
            ),
        ]);
        let id = derive_identity(&flat_root_value)?;
        Ok(Self {
            role,
            domain_id,
            schema_manifest_id,
            family_manifest_ids,
            chunk_ids,
            family_manifest_set_digest,
            payload_set_digest,
            publication_clock,
            rows,
            object_blobs,
            id,
            canonical_value,
            flat_root_value,
            schema_manifest_value: schema_value,
            family_manifest_values,
            chunk_values,
            prior_root_ids,
        })
    }

    pub fn id(&self) -> StoreSnapshotRootIdV1 {
        self.id
    }
    pub fn schema_manifest_id(&self) -> StoreSchemaManifestIdV1 {
        self.schema_manifest_id
    }
    pub fn family_manifest_ids(&self) -> &[StoreExportFamilyManifestIdV1] {
        &self.family_manifest_ids
    }
    pub fn chunk_ids(&self) -> &[StoreExportChunkIdV1] {
        &self.chunk_ids
    }
    pub fn family_manifest_set_digest(&self) -> &[u8; 32] {
        &self.family_manifest_set_digest
    }
    pub fn payload_set_digest(&self) -> &[u8; 32] {
        &self.payload_set_digest
    }
    pub fn publication_clock(&self) -> u64 {
        self.publication_clock
    }
    pub(crate) fn flat_root_value(&self) -> &CborValue {
        &self.flat_root_value
    }
    pub(crate) fn schema_manifest_value(&self) -> &CborValue {
        &self.schema_manifest_value
    }
    pub(crate) fn family_manifest_values(&self) -> &[CborValue] {
        &self.family_manifest_values
    }
    pub(crate) fn chunk_values(&self) -> &[CborValue] {
        &self.chunk_values
    }
    pub(crate) fn prior_root_ids(&self) -> &[[u8; 32]] {
        &self.prior_root_ids
    }
    pub(crate) fn rows(&self) -> &[StoreSnapshotRowV1] {
        &self.rows
    }
    pub(crate) fn object_blobs(&self) -> &[(Vec<u8>, Vec<u8>)] {
        &self.object_blobs
    }
    #[cfg(test)]
    pub(crate) fn with_rewritten_sealed_export_artifact(
        &self,
        snapshot_root_id: [u8; 32],
        replacement_export_id: Option<[u8; 32]>,
        replacement_byte_length: Option<u64>,
        replacement_bytes_digest: Option<[u8; 32]>,
    ) -> Result<Self, SnapshotError> {
        let mut rows = self.rows.clone();
        if !StoreSnapshotRowV1::rewrite_sealed_export_artifact(
            &mut rows,
            snapshot_root_id,
            replacement_export_id,
            replacement_byte_length,
            replacement_bytes_digest,
        ) {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }
        let mut keyed_rows = rows
            .into_iter()
            .map(|row| Ok((row.canonical_sort_key()?, row)))
            .collect::<Result<Vec<_>, StoreSnapshotRowError>>()?;
        keyed_rows.sort_by(|left, right| left.0.cmp(&right.0));
        Self::from_parts(
            self.role,
            self.domain_id,
            self.publication_clock,
            keyed_rows.into_iter().map(|(_, row)| row).collect(),
            self.object_blobs.clone(),
        )
    }
    #[cfg(test)]
    pub(crate) fn with_rewritten_publication_clock(
        &self,
        replacement_publication_clock: u64,
    ) -> Result<Self, SnapshotError> {
        let mut rows = self.rows.clone();
        if !StoreSnapshotRowV1::rewrite_publication_clock(&mut rows, replacement_publication_clock)
        {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }
        let mut keyed_rows = rows
            .into_iter()
            .map(|row| Ok((row.canonical_sort_key()?, row)))
            .collect::<Result<Vec<_>, StoreSnapshotRowError>>()?;
        keyed_rows.sort_by(|left, right| left.0.cmp(&right.0));
        Self::from_parts(
            self.role,
            self.domain_id,
            replacement_publication_clock,
            keyed_rows.into_iter().map(|(_, row)| row).collect(),
            self.object_blobs.clone(),
        )
    }
    pub(crate) fn restore_history_inactive(
        &self,
        transaction: &Transaction<'_>,
        destination_publication_clock: u64,
    ) -> Result<(), SnapshotError> {
        let audit_basis = restore_snapshot_history_v1(
            transaction,
            &self.rows,
            self.role,
            self.domain_id,
            destination_publication_clock,
        )
        .map_err(|error| SnapshotError::Restore(error.to_string()))?;
        self.validate_restore_audit_basis(&audit_basis)
    }

    pub(crate) fn validate_prior_root_binding(&self, prior: &Self) -> Result<(), SnapshotError> {
        if prior.role != self.role
            || prior.domain_id != self.domain_id
            || prior.publication_clock > self.publication_clock
        {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }
        let bindings = self.rows.iter().filter(|row| {
            matches!(
                row,
                StoreSnapshotRowV1::SealedExport { snapshot_root_id, .. }
                    if snapshot_root_id.as_bytes() == prior.id.as_bytes()
            )
        });
        let mut binding_count = 0_usize;
        let prior_basis = prior.export_basis()?;
        for binding in bindings {
            binding_count += 1;
            let StoreSnapshotRowV1::SealedExport {
                export_id,
                head_id,
                generation_id,
                snapshot_id,
                schema_manifest_id,
                family_manifest_set_digest,
                source_publication_clock,
                committed_publication_clock,
                payload_set_digest,
                export_format,
                carrier_format,
                ..
            } = binding
            else {
                return Err(SnapshotError::PriorRootBindingMismatch);
            };
            if head_id.as_bytes() != &prior_basis.head_id
                || generation_id.as_bytes() != &prior_basis.generation_id
                || snapshot_id.as_bytes() != &prior_basis.snapshot_id
                || schema_manifest_id.as_bytes() != prior.schema_manifest_id.as_bytes()
                || family_manifest_set_digest.as_bytes() != &prior.family_manifest_set_digest
                || source_publication_clock.get() != prior.publication_clock
                || prior.publication_clock.checked_add(1) != Some(committed_publication_clock.get())
                || committed_publication_clock.get() > self.publication_clock
                || payload_set_digest.as_bytes() != &prior.payload_set_digest
                || export_format != SEALED_EXPORT_FORMAT_V2
                || carrier_format != SEALED_BACKUP_FORMAT_V1
            {
                return Err(SnapshotError::PriorRootBindingMismatch);
            }
            self.validate_prior_export_sidecars(*export_id.as_bytes(), prior)?;
        }
        if binding_count != 1 {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }
        Ok(())
    }

    fn export_basis(&self) -> Result<PriorExportBasisV1, SnapshotError> {
        let mut active_heads = self.rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::ActiveHead {
                head_id,
                head_revision,
                ..
            } => Some((*head_id.as_bytes(), head_revision.get())),
            _ => None,
        });
        let Some((head_id, head_revision)) = active_heads.next() else {
            return Err(SnapshotError::PriorRootBindingMismatch);
        };
        if active_heads.next().is_some() {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }

        let mut heads = self.rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::Head {
                head_id: candidate_head_id,
                generation_id,
                head_revision: candidate_revision,
                ..
            } if candidate_head_id.as_bytes() == &head_id
                && candidate_revision.get() == head_revision =>
            {
                Some(*generation_id.as_bytes())
            }
            _ => None,
        });
        let Some(generation_id) = heads.next() else {
            return Err(SnapshotError::PriorRootBindingMismatch);
        };
        if heads.next().is_some() {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }

        let mut retention_revisions = self.rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionRevision {
                retention_revision, ..
            } => Some(retention_revision.get()),
            _ => None,
        });
        let Some(retention_revision) = retention_revisions.next() else {
            return Err(SnapshotError::PriorRootBindingMismatch);
        };
        if retention_revisions.next().is_some() {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }

        let mut reachability = self.rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::ReachabilitySnapshot {
                snapshot_id,
                head_id: candidate_head_id,
                retention_revision: candidate_revision,
            } if candidate_head_id.as_bytes() == &head_id
                && candidate_revision.get() == retention_revision =>
            {
                Some(*snapshot_id.as_bytes())
            }
            _ => None,
        });
        let Some(snapshot_id) = reachability.next() else {
            return Err(SnapshotError::PriorRootBindingMismatch);
        };
        if reachability.next().is_some() {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }
        Ok(PriorExportBasisV1 {
            head_id,
            generation_id,
            snapshot_id,
        })
    }

    fn validate_prior_export_sidecars(
        &self,
        export_id: [u8; 32],
        prior: &Self,
    ) -> Result<(), SnapshotError> {
        let released_pins = prior
            .rows
            .iter()
            .filter_map(|row| match row {
                StoreSnapshotRowV1::RetentionPinRelease { pin_id, .. } => Some(*pin_id.as_bytes()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let expected_pins = prior
            .rows
            .iter()
            .filter_map(|row| match row {
                StoreSnapshotRowV1::RetentionPin { pin_id, .. }
                    if !released_pins.contains(pin_id.as_bytes()) =>
                {
                    Some(*pin_id.as_bytes())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let actual_pins = self
            .rows
            .iter()
            .filter_map(|row| match row {
                StoreSnapshotRowV1::SealedExportPin {
                    export_id: candidate_export_id,
                    pin_id,
                } if candidate_export_id.as_bytes() == &export_id => Some(*pin_id.as_bytes()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if actual_pins != expected_pins {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }

        let tombstoned = prior
            .rows
            .iter()
            .filter_map(|row| match row {
                StoreSnapshotRowV1::LogicalTombstone { object_id, .. } => {
                    Some(*object_id.as_bytes())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let expected_entries = prior
            .rows
            .iter()
            .filter_map(|row| match row {
                StoreSnapshotRowV1::Object { object_id, .. } => Some((
                    *object_id.as_bytes(),
                    if tombstoned.contains(object_id.as_bytes()) {
                        SealedExportEntryKindV1::Tombstoned
                    } else {
                        SealedExportEntryKindV1::Available
                    },
                )),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let actual_entries = self
            .rows
            .iter()
            .filter_map(|row| match row {
                StoreSnapshotRowV1::SealedExportObject {
                    export_id: candidate_export_id,
                    object_id,
                    entry_kind,
                } if candidate_export_id.as_bytes() == &export_id => {
                    Some((*object_id.as_bytes(), *entry_kind))
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if actual_entries != expected_entries {
            return Err(SnapshotError::PriorRootBindingMismatch);
        }
        Ok(())
    }

    fn validate_restore_audit_basis(
        &self,
        basis: &SnapshotRestoreAuditBasisV1,
    ) -> Result<(), SnapshotError> {
        if basis.source_publication_clock != self.publication_clock
            || basis.source_retention_revision == 0
        {
            return Err(SnapshotError::RestoreAuditBasisMismatch);
        }
        let source_pointer_matches_state =
            basis.source_active_head.is_some_and(|(head_id, revision)| {
                (basis.source_state != StoreStateV1::Active || basis.source_state_revision > 0)
                    && self.rows.iter().any(|row| {
                        matches!(
                            row,
                            StoreSnapshotRowV1::Head {
                                head_id: row_head_id,
                                head_revision,
                                ..
                            } if *row_head_id == head_id && head_revision.get() == revision
                        )
                    })
            });
        let retention_basis_is_named = self.rows.iter().any(|row| {
            let StoreSnapshotRowV1::ReachabilitySnapshot {
                head_id,
                retention_revision,
                ..
            } = row
            else {
                return false;
            };
            retention_revision.get() == basis.source_retention_revision
                && basis
                    .source_active_head
                    .is_none_or(|(active_head_id, _)| active_head_id == *head_id)
                && self.rows.iter().any(|candidate| {
                    matches!(candidate, StoreSnapshotRowV1::Head { head_id: candidate_id, .. } if candidate_id == head_id)
                })
        });
        if !source_pointer_matches_state || !retention_basis_is_named {
            return Err(SnapshotError::RestoreAuditBasisMismatch);
        }
        Ok(())
    }
    pub(crate) fn role(&self) -> StoreRoleV1 {
        self.role
    }
    pub(crate) fn domain_id(&self) -> StoreDomainIdV1 {
        self.domain_id
    }
}

fn family_value(
    family: StoreSnapshotFamilyV1,
    rows: &[StoreSnapshotRowV1],
    chunk_ids: &mut Vec<StoreExportChunkIdV1>,
) -> Result<CborValue, SnapshotError> {
    let row_digests = rows
        .iter()
        .map(|row| digest_value(&row.to_canonical_value()))
        .collect::<Result<Vec<_>, _>>()?;
    let set_root = digest_value(&CborValue::Array(
        row_digests
            .iter()
            .map(|digest| CborValue::Bytes(digest.to_vec()))
            .collect(),
    ))?;
    let mut chunks = Vec::new();
    for (chunk_index, chunk) in rows.chunks(MAX_ROWS_PER_CHUNK).enumerate() {
        let value = CborValue::Array(vec![
            CborValue::Unsigned(CHUNK_VERSION),
            CborValue::Unsigned(family.tag()),
            CborValue::Unsigned(chunk_index as u64),
            CborValue::Unsigned((chunk_index * MAX_ROWS_PER_CHUNK) as u64),
            CborValue::Array(
                chunk
                    .iter()
                    .map(StoreSnapshotRowV1::to_canonical_value)
                    .collect(),
            ),
        ]);
        deterministic_cbor::encode(&value)?;
        chunk_ids.push(derive_identity(&value)?);
        chunks.push(value);
    }
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(FAMILY_MANIFEST_VERSION),
        CborValue::Unsigned(family.tag()),
        CborValue::text(family.name())?,
        CborValue::text(family_classification(family))?,
        CborValue::Unsigned(rows.len() as u64),
        CborValue::Bytes(set_root.to_vec()),
        CborValue::Array(chunks),
    ]))
}

fn flat_family_manifest_value(
    value: &CborValue,
    schema_manifest: &CborValue,
) -> Result<CborValue, SnapshotError> {
    let CborValue::Array(fields) = value else {
        return Err(SnapshotError::InvalidShape);
    };
    let [
        _version,
        tag,
        name,
        classification,
        row_count,
        set_root,
        CborValue::Array(chunks),
    ] = fields.as_slice()
    else {
        return Err(SnapshotError::InvalidShape);
    };
    let chunk_ids = chunks
        .iter()
        .map(|chunk| {
            let id: StoreExportChunkIdV1 = derive_identity(chunk)?;
            Ok(CborValue::Bytes(id.as_bytes().to_vec()))
        })
        .collect::<Result<Vec<_>, SnapshotError>>()?;
    let CborValue::Array(schema_fields) = schema_manifest else {
        return Err(SnapshotError::InvalidShape);
    };
    let Some(CborValue::Array(schema_tables)) = schema_fields.last() else {
        return Err(SnapshotError::InvalidShape);
    };
    let table_manifest = schema_tables
        .iter()
        .filter(|entry| {
            matches!(
                entry,
                CborValue::Array(entry_fields)
                    if matches!(
                        entry_fields.first(),
                        Some(CborValue::Array(descriptor))
                            if descriptor.get(1) == Some(tag)
                    )
            )
        })
        .cloned()
        .collect();
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(2),
        tag.clone(),
        name.clone(),
        classification.clone(),
        row_count.clone(),
        set_root.clone(),
        CborValue::Array(table_manifest),
        CborValue::Array(chunk_ids),
    ]))
}

fn decode_family(
    family: StoreSnapshotFamilyV1,
    value: &CborValue,
) -> Result<Vec<StoreSnapshotRowV1>, SnapshotError> {
    let CborValue::Array(fields) = value else {
        return Err(SnapshotError::InvalidShape);
    };
    let [
        CborValue::Unsigned(version),
        CborValue::Unsigned(tag),
        CborValue::Text(name),
        CborValue::Text(classification),
        CborValue::Unsigned(row_count),
        CborValue::Bytes(_),
        CborValue::Array(chunks),
    ] = fields.as_slice()
    else {
        return Err(SnapshotError::InvalidShape);
    };
    if *version != FAMILY_MANIFEST_VERSION
        || *tag != family.tag()
        || name != family.name()
        || classification != family_classification(family)
    {
        return Err(SnapshotError::FamilyManifestMismatch);
    }
    let mut rows = Vec::new();
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        let CborValue::Array(fields) = chunk else {
            return Err(SnapshotError::InvalidShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Unsigned(family_tag),
            CborValue::Unsigned(declared_chunk),
            CborValue::Unsigned(row_start),
            CborValue::Array(chunk_rows),
        ] = fields.as_slice()
        else {
            return Err(SnapshotError::InvalidShape);
        };
        if *version != CHUNK_VERSION
            || *family_tag != family.tag()
            || *declared_chunk != chunk_index as u64
            || *row_start != rows.len() as u64
            || chunk_rows.is_empty()
            || chunk_rows.len() > MAX_ROWS_PER_CHUNK
        {
            return Err(SnapshotError::ChunkManifestMismatch);
        }
        for row in chunk_rows {
            let decoded = StoreSnapshotRowV1::from_canonical_value(row.clone())?;
            if decoded.family() != family {
                return Err(SnapshotError::FamilyManifestMismatch);
            }
            rows.push(decoded);
        }
    }
    if rows.len() as u64 != *row_count {
        return Err(SnapshotError::ChunkManifestMismatch);
    }
    let mut ignored = Vec::new();
    if family_value(family, &rows, &mut ignored)? != *value {
        return Err(SnapshotError::FamilyManifestMismatch);
    }
    Ok(rows)
}

fn validate_rows(rows: &[StoreSnapshotRowV1]) -> Result<(), SnapshotError> {
    StoreSnapshotRowV1::validate_bounds(rows)?;
    let keys = rows
        .iter()
        .map(StoreSnapshotRowV1::canonical_sort_key)
        .collect::<Result<Vec<_>, _>>()?;
    if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SnapshotError::RowOrderMismatch);
    }
    for row in rows {
        let encoded = row.encode_canonical()?;
        if StoreSnapshotRowV1::decode_canonical(&encoded)? != *row {
            return Err(SnapshotError::NonCanonicalSnapshot);
        }
    }
    Ok(())
}

fn schema_manifest_value(schema_digest: [u8; 32]) -> Result<CborValue, SnapshotError> {
    let tables = STORE_SNAPSHOT_TABLE_MANIFEST_V1
        .iter()
        .map(|manifest| {
            let descriptor = CborValue::Array(vec![
                CborValue::Unsigned(manifest.order as u64),
                CborValue::Unsigned(manifest.family.tag()),
                CborValue::text(manifest.table.name())?,
                CborValue::Array(
                    manifest
                        .columns
                        .iter()
                        .map(|column| CborValue::text(*column))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                CborValue::Array(
                    manifest
                        .primary_key
                        .iter()
                        .map(|column| CborValue::text(*column))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            ]);
            let row_schema: SchemaIdV1 =
                ManifestIdentityV1::from_digest(digest_value(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.store-row-schema.v1")?,
                    descriptor.clone(),
                ]))?);
            Ok(CborValue::Array(vec![
                descriptor,
                CborValue::Bytes(row_schema.as_bytes().to_vec()),
            ]))
        })
        .collect::<Result<Vec<_>, SnapshotError>>()?;
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(SCHEMA_MANIFEST_VERSION),
        CborValue::Unsigned(METADATA_APPLICATION_ID as u64),
        CborValue::Unsigned(METADATA_SCHEMA_VERSION as u64),
        CborValue::Unsigned(CBOR_PROFILE_VERSION),
        CborValue::Bytes(schema_digest.to_vec()),
        CborValue::Array(tables),
    ]))
}

fn required_object_ids(rows: &[StoreSnapshotRowV1]) -> BTreeSet<Vec<u8>> {
    let mut ids = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::Object { object_id, .. } => Some(object_id.as_bytes().to_vec()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for row in rows {
        match row {
            StoreSnapshotRowV1::LogicalTombstone { object_id, .. }
            | StoreSnapshotRowV1::GcCollectionOccurrence { object_id, .. } => {
                ids.remove(object_id.as_bytes().as_slice());
            }
            _ => {}
        }
    }
    ids
}

fn prior_root_ids(rows: &[StoreSnapshotRowV1]) -> Vec<[u8; 32]> {
    rows.iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::SealedExport {
                snapshot_root_id, ..
            } => Some(*snapshot_root_id.as_bytes()),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn verify_object_blobs(
    rows: &[StoreSnapshotRowV1],
    blobs: &[(Vec<u8>, Vec<u8>)],
) -> Result<(), SnapshotError> {
    let object_rows = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::Object {
                object_id,
                logical_byte_length,
                stored_byte_length,
                stored_bytes_digest,
                ..
            } => Some((
                *object_id.as_bytes(),
                (
                    logical_byte_length.get(),
                    stored_byte_length.get(),
                    *stored_bytes_digest.as_bytes(),
                ),
            )),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    for (id, bytes) in blobs {
        let object_id: [u8; 32] = id
            .as_slice()
            .try_into()
            .map_err(|_| SnapshotError::ObjectClosureMismatch)?;
        let Some((logical_byte_length, stored_byte_length, stored_bytes_digest)) =
            object_rows.get(&object_id)
        else {
            return Err(SnapshotError::ObjectClosureMismatch);
        };
        if *logical_byte_length != bytes.len() as u64
            || *stored_byte_length != bytes.len() as u64
            || stored_bytes_digest.as_slice() != Sha256::digest(bytes).as_slice()
        {
            return Err(SnapshotError::ObjectClosureMismatch);
        }
    }
    Ok(())
}

const fn family_classification(family: StoreSnapshotFamilyV1) -> &'static str {
    match family {
        StoreSnapshotFamilyV1::StoreIdentity => "artifact_commitment",
        StoreSnapshotFamilyV1::SourcePointers => "mutable_pointer",
        StoreSnapshotFamilyV1::ObjectHistory
        | StoreSnapshotFamilyV1::GenerationHistory
        | StoreSnapshotFamilyV1::ReachabilityRetentionHistory
        | StoreSnapshotFamilyV1::GarbageCollectionHistory
        | StoreSnapshotFamilyV1::IdempotencyHistory => "authoritative_history",
        StoreSnapshotFamilyV1::ExportRestoreHistory => "artifact_commitment",
    }
}

fn digest_value(value: &CborValue) -> Result<[u8; 32], SnapshotError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn identity_from_bytes<K: crate::domain::identity::IdentityKindV1>(
    bytes: &[u8],
) -> Result<ManifestIdentityV1<K>, SnapshotError> {
    let digest: [u8; 32] = bytes.try_into().map_err(|_| SnapshotError::InvalidShape)?;
    Ok(ManifestIdentityV1::from_digest(digest))
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("Store snapshot has an invalid canonical shape")]
    InvalidShape,
    #[error("Store snapshot uses unsupported version {0}")]
    UnknownVersion(u64),
    #[error("Store snapshot schema manifest is not the exact supported schema")]
    SchemaManifestMismatch,
    #[error("Store snapshot family manifest is missing, extra, reordered, or malformed")]
    FamilyManifestMismatch,
    #[error("Store snapshot chunk is missing, extra, reordered, or malformed")]
    ChunkManifestMismatch,
    #[error("Store snapshot rows are duplicated or not canonically ordered")]
    RowOrderMismatch,
    #[error("Store snapshot object-byte closure is missing, extra, or corrupt")]
    ObjectClosureMismatch,
    #[error("Store snapshot object-byte closure could not be read: {0}")]
    ObjectRead(String),
    #[error("Store snapshot is not its exact canonical reconstruction")]
    NonCanonicalSnapshot,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
    #[error("Store snapshot typed row validation failed: {0}")]
    Row(String),
    #[error("Store snapshot history restore failed: {0}")]
    Restore(String),
    #[error("Store snapshot restore audit basis does not match the committed source snapshot")]
    RestoreAuditBasisMismatch,
    #[error("prior Store snapshot root does not match its naming sealed-export commitments")]
    PriorRootBindingMismatch,
    #[error("Store metadata validation failed: {0}")]
    Metadata(String),
}

impl From<super::metadata::MetadataError> for SnapshotError {
    fn from(error: super::metadata::MetadataError) -> Self {
        Self::Metadata(error.to_string())
    }
}

impl From<StoreSnapshotRowError> for SnapshotError {
    fn from(error: StoreSnapshotRowError) -> Self {
        Self::Row(error.to_string())
    }
}
