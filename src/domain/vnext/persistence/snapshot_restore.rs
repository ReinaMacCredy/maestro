use rusqlite::{Transaction, params};
use thiserror::Error;

use crate::domain::vnext::identity::StoreDomainIdV1;

use super::StoreRoleV1;
use super::metadata::METADATA_SCHEMA_VERSION;
use super::snapshot_rows::{
    StoreSnapshotDigestV1, StoreSnapshotRowError, StoreSnapshotRowV1, StoreStateV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotRestoreAuditBasisV1 {
    pub(crate) source_publication_clock: u64,
    pub(crate) source_state: StoreStateV1,
    pub(crate) source_state_revision: u64,
    pub(crate) source_retention_revision: u64,
    pub(crate) source_active_head: Option<(StoreSnapshotDigestV1, u64)>,
}

pub(crate) fn restore_snapshot_history_v1(
    transaction: &Transaction<'_>,
    source_rows: &[StoreSnapshotRowV1],
    expected_role: StoreRoleV1,
    expected_domain_id: StoreDomainIdV1,
    destination_publication_clock: u64,
) -> Result<SnapshotRestoreAuditBasisV1, SnapshotRestoreError> {
    let audit_basis = validate_source_rows(source_rows, expected_role, expected_domain_id)?;
    verify_pristine_destination(
        transaction,
        expected_role,
        expected_domain_id,
        destination_publication_clock,
    )?;

    let mut history = source_rows
        .iter()
        .filter(|row| !is_authority_row(row))
        .map(|row| {
            Ok((
                insertion_rank(row),
                insertion_sequence(row),
                row.canonical_sort_key()?,
                row,
            ))
        })
        .collect::<Result<Vec<_>, SnapshotRestoreError>>()?;
    history.sort_by(|left, right| (&left.0, &left.1, &left.2).cmp(&(&right.0, &right.1, &right.2)));

    for (_, _, _, row) in history {
        insert_history_row(transaction, row)?;
    }

    verify_restored_snapshot_history_v1(
        transaction,
        source_rows,
        &audit_basis,
        expected_role,
        expected_domain_id,
        destination_publication_clock,
    )?;
    Ok(audit_basis)
}

pub(crate) fn verify_restored_snapshot_history_v1(
    transaction: &Transaction<'_>,
    source_rows: &[StoreSnapshotRowV1],
    expected_audit_basis: &SnapshotRestoreAuditBasisV1,
    expected_role: StoreRoleV1,
    expected_domain_id: StoreDomainIdV1,
    destination_publication_clock: u64,
) -> Result<(), SnapshotRestoreError> {
    let actual_audit_basis = validate_source_rows(source_rows, expected_role, expected_domain_id)?;
    if actual_audit_basis != *expected_audit_basis {
        return Err(SnapshotRestoreError::SourceAuditBasisMismatch);
    }
    verify_destination_authority(
        transaction,
        expected_role,
        expected_domain_id,
        destination_publication_clock,
    )?;

    let destination_rows = StoreSnapshotRowV1::load_all(transaction)?;
    let mut expected_history = history_rows(source_rows)?;
    let mut actual_history = history_rows(&destination_rows)?;
    expected_history.sort_by(|left, right| left.0.cmp(&right.0));
    actual_history.sort_by(|left, right| left.0.cmp(&right.0));
    if expected_history != actual_history {
        return Err(SnapshotRestoreError::HistoryParityMismatch);
    }
    Ok(())
}

fn history_rows(
    rows: &[StoreSnapshotRowV1],
) -> Result<
    Vec<(
        super::snapshot_rows::StoreSnapshotSortKeyV1,
        StoreSnapshotRowV1,
    )>,
    SnapshotRestoreError,
> {
    rows.iter()
        .filter(|row| !is_authority_row(row))
        .map(|row| Ok((row.canonical_sort_key()?, row.clone())))
        .collect()
}

fn validate_source_rows(
    rows: &[StoreSnapshotRowV1],
    expected_role: StoreRoleV1,
    expected_domain_id: StoreDomainIdV1,
) -> Result<SnapshotRestoreAuditBasisV1, SnapshotRestoreError> {
    let keys = rows
        .iter()
        .map(StoreSnapshotRowV1::canonical_sort_key)
        .collect::<Result<Vec<_>, _>>()?;
    if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SnapshotRestoreError::SourceRowsNotStrictlyOrdered);
    }

    let metadata = exactly_one(
        rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::Metadata {
                schema_version,
                store_role,
                domain_id,
                ..
            } => Some((*schema_version, *store_role, *domain_id)),
            _ => None,
        }),
        "store_metadata",
    )?;
    if metadata.0 != METADATA_SCHEMA_VERSION as u8
        || metadata.1 != expected_role
        || metadata.2.as_bytes() != expected_domain_id.as_bytes()
    {
        return Err(SnapshotRestoreError::SourceIdentityMismatch);
    }

    let source_publication_clock = exactly_one(
        rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::PublicationClock {
                publication_clock, ..
            } => Some(publication_clock.get()),
            _ => None,
        }),
        "store_publication_clock",
    )?;
    let (source_state, source_state_revision) = exactly_one(
        rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::State {
                state,
                state_revision,
                ..
            } => Some((*state, state_revision.get())),
            _ => None,
        }),
        "store_state",
    )?;
    let source_retention_revision = exactly_one(
        rows.iter().filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionRevision {
                retention_revision, ..
            } => Some(retention_revision.get()),
            _ => None,
        }),
        "store_retention_revision",
    )?;

    let mut active_heads = rows.iter().filter_map(|row| match row {
        StoreSnapshotRowV1::ActiveHead {
            head_id,
            head_revision,
            ..
        } => Some((*head_id, head_revision.get())),
        _ => None,
    });
    let source_active_head = active_heads.next();
    if active_heads.next().is_some() {
        return Err(SnapshotRestoreError::DuplicateSourceSingleton(
            "store_active_head",
        ));
    }
    if matches!(source_state, StoreStateV1::Active) && source_active_head.is_none() {
        return Err(SnapshotRestoreError::SourceAuthorityInconsistent);
    }
    if let Some((active_head_id, active_head_revision)) = source_active_head {
        let matching_heads = rows
            .iter()
            .filter(|row| {
                matches!(
                    row,
                    StoreSnapshotRowV1::Head {
                        head_id,
                        head_revision,
                        ..
                    } if *head_id == active_head_id && head_revision.get() == active_head_revision
                )
            })
            .count();
        if matching_heads != 1 {
            return Err(SnapshotRestoreError::SourceAuthorityInconsistent);
        }
    }

    Ok(SnapshotRestoreAuditBasisV1 {
        source_publication_clock,
        source_state,
        source_state_revision,
        source_retention_revision,
        source_active_head,
    })
}

fn exactly_one<T>(
    mut values: impl Iterator<Item = T>,
    table: &'static str,
) -> Result<T, SnapshotRestoreError> {
    let Some(value) = values.next() else {
        return Err(SnapshotRestoreError::MissingSourceSingleton(table));
    };
    if values.next().is_some() {
        return Err(SnapshotRestoreError::DuplicateSourceSingleton(table));
    }
    Ok(value)
}

fn verify_pristine_destination(
    transaction: &Transaction<'_>,
    expected_role: StoreRoleV1,
    expected_domain_id: StoreDomainIdV1,
    destination_publication_clock: u64,
) -> Result<(), SnapshotRestoreError> {
    verify_destination_authority(
        transaction,
        expected_role,
        expected_domain_id,
        destination_publication_clock,
    )?;
    let history_count: i64 = transaction.query_row(
        "SELECT
            (SELECT COUNT(*) FROM store_objects)
          + (SELECT COUNT(*) FROM store_object_references)
          + (SELECT COUNT(*) FROM store_generations)
          + (SELECT COUNT(*) FROM store_generation_roots)
          + (SELECT COUNT(*) FROM store_heads)
          + (SELECT COUNT(*) FROM store_reachability_snapshots)
          + (SELECT COUNT(*) FROM store_reachability_roots)
          + (SELECT COUNT(*) FROM store_reachability_objects)
          + (SELECT COUNT(*) FROM store_retention_pins)
          + (SELECT COUNT(*) FROM store_retention_pin_releases)
          + (SELECT COUNT(*) FROM store_logical_tombstones)
          + (SELECT COUNT(*) FROM store_gc_plans)
          + (SELECT COUNT(*) FROM store_gc_plan_objects)
          + (SELECT COUNT(*) FROM store_gc_collection_occurrences)
          + (SELECT COUNT(*) FROM store_sealed_exports)
          + (SELECT COUNT(*) FROM store_sealed_export_pins)
          + (SELECT COUNT(*) FROM store_sealed_export_objects)
          + (SELECT COUNT(*) FROM store_restore_candidates)
          + (SELECT COUNT(*) FROM store_restore_candidate_roots)
          + (SELECT COUNT(*) FROM store_idempotency)",
        [],
        |row| row.get(0),
    )?;
    if history_count != 0 {
        return Err(SnapshotRestoreError::DestinationHistoryNotEmpty(
            history_count,
        ));
    }
    Ok(())
}

fn verify_destination_authority(
    transaction: &Transaction<'_>,
    expected_role: StoreRoleV1,
    expected_domain_id: StoreDomainIdV1,
    destination_publication_clock: u64,
) -> Result<(), SnapshotRestoreError> {
    let metadata_count: i64 =
        transaction.query_row("SELECT COUNT(*) FROM store_metadata", [], |row| row.get(0))?;
    let metadata_matches: bool = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM store_metadata
             WHERE singleton = 1 AND schema_version = ?1 AND store_role = ?2 AND domain_id = ?3
         )",
        params![
            METADATA_SCHEMA_VERSION,
            sql_integer(expected_role.tag(), "store_role")?,
            expected_domain_id.as_bytes()
        ],
        |row| row.get(0),
    )?;
    let publication_clock_matches: bool = transaction.query_row(
        "SELECT (SELECT COUNT(*) FROM store_publication_clock) = 1
             AND EXISTS(
                 SELECT 1 FROM store_publication_clock
                 WHERE singleton = 1 AND publication_clock = ?1
             )",
        params![sql_integer(
            destination_publication_clock,
            "destination_publication_clock"
        )?],
        |row| row.get(0),
    )?;
    let state_matches: bool = transaction.query_row(
        "SELECT (SELECT COUNT(*) FROM store_state) = 1
             AND EXISTS(
                 SELECT 1 FROM store_state
                 WHERE singleton = 1 AND state = 'inactive' AND state_revision = 0
             )",
        [],
        |row| row.get(0),
    )?;
    let retention_matches: bool = transaction.query_row(
        "SELECT (SELECT COUNT(*) FROM store_retention_revision) = 1
             AND EXISTS(
                 SELECT 1 FROM store_retention_revision
                 WHERE singleton = 1 AND retention_revision = 0
             )",
        [],
        |row| row.get(0),
    )?;
    let active_head_count: i64 =
        transaction.query_row("SELECT COUNT(*) FROM store_active_head", [], |row| {
            row.get(0)
        })?;
    if metadata_count != 1
        || !metadata_matches
        || !publication_clock_matches
        || !state_matches
        || !retention_matches
        || active_head_count != 0
    {
        return Err(SnapshotRestoreError::DestinationAuthorityMismatch);
    }
    Ok(())
}

const fn is_authority_row(row: &StoreSnapshotRowV1) -> bool {
    matches!(
        row,
        StoreSnapshotRowV1::Metadata { .. }
            | StoreSnapshotRowV1::PublicationClock { .. }
            | StoreSnapshotRowV1::State { .. }
            | StoreSnapshotRowV1::RetentionRevision { .. }
            | StoreSnapshotRowV1::ActiveHead { .. }
    )
}

const fn insertion_rank(row: &StoreSnapshotRowV1) -> u8 {
    match row {
        StoreSnapshotRowV1::Object { .. } => 1,
        StoreSnapshotRowV1::ObjectReference { .. } => 2,
        StoreSnapshotRowV1::Generation { .. } => 3,
        StoreSnapshotRowV1::GenerationRoot { .. } => 4,
        StoreSnapshotRowV1::Head { .. } => 5,
        StoreSnapshotRowV1::RetentionPin { .. } => 6,
        StoreSnapshotRowV1::RetentionPinRelease { .. } => 7,
        StoreSnapshotRowV1::LogicalTombstone { .. } => 8,
        StoreSnapshotRowV1::ReachabilitySnapshot { .. } => 9,
        StoreSnapshotRowV1::ReachabilityObject { .. } => 10,
        StoreSnapshotRowV1::ReachabilityRoot { .. } => 11,
        StoreSnapshotRowV1::GcPlan { .. } => 12,
        StoreSnapshotRowV1::GcPlanObject { .. } => 13,
        StoreSnapshotRowV1::GcCollectionOccurrence { .. } => 14,
        StoreSnapshotRowV1::SealedExport { .. } => 15,
        StoreSnapshotRowV1::SealedExportPin { .. } => 16,
        StoreSnapshotRowV1::SealedExportObject { .. } => 17,
        StoreSnapshotRowV1::RestoreCandidate { .. } => 18,
        StoreSnapshotRowV1::RestoreCandidateRoot { .. } => 19,
        StoreSnapshotRowV1::Idempotency { .. } => 20,
        StoreSnapshotRowV1::Metadata { .. }
        | StoreSnapshotRowV1::PublicationClock { .. }
        | StoreSnapshotRowV1::State { .. }
        | StoreSnapshotRowV1::RetentionRevision { .. }
        | StoreSnapshotRowV1::ActiveHead { .. } => 0,
    }
}

const fn insertion_sequence(row: &StoreSnapshotRowV1) -> u64 {
    match row {
        StoreSnapshotRowV1::Generation {
            generation_ordinal, ..
        } => generation_ordinal.get(),
        StoreSnapshotRowV1::Head { head_revision, .. } => head_revision.get(),
        _ => 0,
    }
}

fn insert_history_row(
    transaction: &Transaction<'_>,
    row: &StoreSnapshotRowV1,
) -> Result<(), SnapshotRestoreError> {
    match row {
        StoreSnapshotRowV1::Object {
            object_id,
            schema_id,
            logical_byte_length,
            stored_byte_length,
            stored_bytes_digest,
            storage_codec,
            key_envelope_id,
            key_envelope_kind,
        } => transaction.execute(
            "INSERT INTO store_objects
             (object_id, schema_id, logical_byte_length, stored_byte_length,
              stored_bytes_digest, storage_codec, key_envelope_id, key_envelope_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                object_id.as_bytes().as_slice(),
                schema_id.as_bytes().as_slice(),
                sql_integer(logical_byte_length.get(), "logical_byte_length")?,
                sql_integer(stored_byte_length.get(), "stored_byte_length")?,
                stored_bytes_digest.as_bytes().as_slice(),
                storage_codec,
                key_envelope_id
                    .as_ref()
                    .map(|value| value.as_bytes().as_slice()),
                key_envelope_kind.as_deref(),
            ],
        )?,
        StoreSnapshotRowV1::ObjectReference {
            object_id,
            reference_position,
            referenced_object_id,
        } => transaction.execute(
            "INSERT INTO store_object_references
             (object_id, reference_position, referenced_object_id) VALUES (?1, ?2, ?3)",
            params![
                object_id.as_bytes().as_slice(),
                sql_integer(reference_position.get(), "reference_position")?,
                referenced_object_id.as_bytes().as_slice(),
            ],
        )?,
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
        } => transaction.execute(
            "INSERT INTO store_generations
             (generation_id, generation_ordinal, previous_generation_id, contract_root_id,
              writer_compatibility_manifest_id, association_schema_id, finality_edge_manifest_id,
              schema_read_write_set_descriptor_id, writer_protocol_epoch_id, migration_epoch_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                generation_id.as_bytes().as_slice(),
                sql_integer(generation_ordinal.get(), "generation_ordinal")?,
                previous_generation_id
                    .as_ref()
                    .map(|value| value.as_bytes().as_slice()),
                contract_root_id.as_bytes().as_slice(),
                writer_compatibility_manifest_id.as_bytes().as_slice(),
                association_schema_id.as_bytes().as_slice(),
                finality_edge_manifest_id.as_bytes().as_slice(),
                schema_read_write_set_descriptor_id.as_bytes().as_slice(),
                writer_protocol_epoch_id.as_bytes().as_slice(),
                migration_epoch_id.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::GenerationRoot {
            generation_id,
            root_position,
            object_id,
        } => transaction.execute(
            "INSERT INTO store_generation_roots
             (generation_id, root_position, object_id) VALUES (?1, ?2, ?3)",
            params![
                generation_id.as_bytes().as_slice(),
                sql_integer(root_position.get(), "root_position")?,
                object_id.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::Head {
            head_id,
            generation_id,
            generation_ordinal,
            head_revision,
            previous_head_id,
        } => transaction.execute(
            "INSERT INTO store_heads
             (head_id, generation_id, generation_ordinal, head_revision, previous_head_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                head_id.as_bytes().as_slice(),
                generation_id.as_bytes().as_slice(),
                sql_integer(generation_ordinal.get(), "generation_ordinal")?,
                sql_integer(head_revision.get(), "head_revision")?,
                previous_head_id
                    .as_ref()
                    .map(|value| value.as_bytes().as_slice()),
            ],
        )?,
        StoreSnapshotRowV1::RetentionPin {
            pin_id,
            basis_head_id,
            root_kind,
            root_object_id,
            reason_digest,
        } => transaction.execute(
            "INSERT INTO store_retention_pins
             (pin_id, basis_head_id, root_kind, root_object_id, reason_digest)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                pin_id.as_bytes().as_slice(),
                basis_head_id.as_bytes().as_slice(),
                sql_integer(root_kind.tag(), "root_kind")?,
                root_object_id.as_bytes().as_slice(),
                reason_digest.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::RetentionPinRelease {
            pin_id,
            released_at_head_id,
            reason_digest,
        } => transaction.execute(
            "INSERT INTO store_retention_pin_releases
             (pin_id, released_at_head_id, reason_digest) VALUES (?1, ?2, ?3)",
            params![
                pin_id.as_bytes().as_slice(),
                released_at_head_id.as_bytes().as_slice(),
                reason_digest.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::LogicalTombstone {
            tombstone_id,
            basis_head_id,
            object_id,
            reason_digest,
            invalidation_digest,
        } => transaction.execute(
            "INSERT INTO store_logical_tombstones
             (tombstone_id, basis_head_id, object_id, reason_digest, invalidation_digest)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                tombstone_id.as_bytes().as_slice(),
                basis_head_id.as_bytes().as_slice(),
                object_id.as_bytes().as_slice(),
                reason_digest.as_bytes().as_slice(),
                invalidation_digest.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::ReachabilitySnapshot {
            snapshot_id,
            head_id,
            retention_revision,
        } => transaction.execute(
            "INSERT INTO store_reachability_snapshots
             (snapshot_id, head_id, retention_revision) VALUES (?1, ?2, ?3)",
            params![
                snapshot_id.as_bytes().as_slice(),
                head_id.as_bytes().as_slice(),
                sql_integer(retention_revision.get(), "retention_revision")?,
            ],
        )?,
        StoreSnapshotRowV1::ReachabilityObject {
            snapshot_id,
            object_id,
            reachability_status,
        } => transaction.execute(
            "INSERT INTO store_reachability_objects
             (snapshot_id, object_id, reachability_status) VALUES (?1, ?2, ?3)",
            params![
                snapshot_id.as_bytes().as_slice(),
                object_id.as_bytes().as_slice(),
                reachability_status.as_str(),
            ],
        )?,
        StoreSnapshotRowV1::ReachabilityRoot {
            snapshot_id,
            root_position,
            root_kind,
            object_id,
        } => transaction.execute(
            "INSERT INTO store_reachability_roots
             (snapshot_id, root_position, root_kind, object_id) VALUES (?1, ?2, ?3, ?4)",
            params![
                snapshot_id.as_bytes().as_slice(),
                sql_integer(root_position.get(), "root_position")?,
                sql_integer(root_kind.tag(), "root_kind")?,
                object_id.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::GcPlan {
            plan_id,
            snapshot_id,
            head_id,
            retention_revision,
        } => transaction.execute(
            "INSERT INTO store_gc_plans
             (plan_id, snapshot_id, head_id, retention_revision) VALUES (?1, ?2, ?3, ?4)",
            params![
                plan_id.as_bytes().as_slice(),
                snapshot_id.as_bytes().as_slice(),
                head_id.as_bytes().as_slice(),
                sql_integer(retention_revision.get(), "retention_revision")?,
            ],
        )?,
        StoreSnapshotRowV1::GcPlanObject { plan_id, object_id } => transaction.execute(
            "INSERT INTO store_gc_plan_objects (plan_id, object_id) VALUES (?1, ?2)",
            params![
                plan_id.as_bytes().as_slice(),
                object_id.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::GcCollectionOccurrence {
            plan_id,
            object_id,
            stored_bytes_digest,
        } => transaction.execute(
            "INSERT INTO store_gc_collection_occurrences
             (plan_id, object_id, stored_bytes_digest) VALUES (?1, ?2, ?3)",
            params![
                plan_id.as_bytes().as_slice(),
                object_id.as_bytes().as_slice(),
                stored_bytes_digest.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::SealedExport {
            export_id,
            head_id,
            generation_id,
            snapshot_id,
            schema_manifest_id,
            family_manifest_set_digest,
            snapshot_root_id,
            source_publication_clock,
            committed_publication_clock,
            payload_set_digest,
            export_byte_length,
            export_bytes_digest,
            export_format,
            backup_receipt_id,
            carrier_format,
        } => transaction.execute(
            "INSERT INTO store_sealed_exports
             (export_id, head_id, generation_id, snapshot_id, schema_manifest_id,
              family_manifest_set_digest, snapshot_root_id, source_publication_clock,
              committed_publication_clock, payload_set_digest, export_byte_length,
              export_bytes_digest, export_format, backup_receipt_id, carrier_format)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                export_id.as_bytes().as_slice(),
                head_id.as_bytes().as_slice(),
                generation_id.as_bytes().as_slice(),
                snapshot_id.as_bytes().as_slice(),
                schema_manifest_id.as_bytes().as_slice(),
                family_manifest_set_digest.as_bytes().as_slice(),
                snapshot_root_id.as_bytes().as_slice(),
                sql_integer(source_publication_clock.get(), "source_publication_clock")?,
                sql_integer(
                    committed_publication_clock.get(),
                    "committed_publication_clock",
                )?,
                payload_set_digest.as_bytes().as_slice(),
                sql_integer(export_byte_length.get(), "export_byte_length")?,
                export_bytes_digest.as_bytes().as_slice(),
                export_format,
                backup_receipt_id.as_bytes().as_slice(),
                carrier_format,
            ],
        )?,
        StoreSnapshotRowV1::SealedExportPin { export_id, pin_id } => transaction.execute(
            "INSERT INTO store_sealed_export_pins (export_id, pin_id) VALUES (?1, ?2)",
            params![
                export_id.as_bytes().as_slice(),
                pin_id.as_bytes().as_slice()
            ],
        )?,
        StoreSnapshotRowV1::SealedExportObject {
            export_id,
            object_id,
            entry_kind,
        } => transaction.execute(
            "INSERT INTO store_sealed_export_objects
             (export_id, object_id, entry_kind) VALUES (?1, ?2, ?3)",
            params![
                export_id.as_bytes().as_slice(),
                object_id.as_bytes().as_slice(),
                entry_kind.as_str(),
            ],
        )?,
        StoreSnapshotRowV1::RestoreCandidate {
            candidate_id,
            source_export_id,
            source_domain_id,
            source_export_bytes_digest,
            source_schema_manifest_id,
            source_snapshot_root_id,
            destination_domain_id,
            candidate_generation_id,
            candidate_head_id,
            candidate_snapshot_id,
            verification_digest,
        } => transaction.execute(
            "INSERT INTO store_restore_candidates
             (candidate_id, source_export_id, source_domain_id, source_export_bytes_digest,
              source_schema_manifest_id, source_snapshot_root_id, destination_domain_id,
              candidate_generation_id, candidate_head_id, candidate_snapshot_id,
              verification_digest)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                candidate_id.as_bytes().as_slice(),
                source_export_id.as_bytes().as_slice(),
                source_domain_id.as_bytes().as_slice(),
                source_export_bytes_digest.as_bytes().as_slice(),
                source_schema_manifest_id.as_bytes().as_slice(),
                source_snapshot_root_id.as_bytes().as_slice(),
                destination_domain_id.as_bytes().as_slice(),
                candidate_generation_id.as_bytes().as_slice(),
                candidate_head_id.as_bytes().as_slice(),
                candidate_snapshot_id.as_bytes().as_slice(),
                verification_digest.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::RestoreCandidateRoot {
            candidate_id,
            root_position,
            object_id,
        } => transaction.execute(
            "INSERT INTO store_restore_candidate_roots
             (candidate_id, root_position, object_id) VALUES (?1, ?2, ?3)",
            params![
                candidate_id.as_bytes().as_slice(),
                sql_integer(root_position.get(), "root_position")?,
                object_id.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::Idempotency {
            namespace,
            key_digest,
            meaning_digest,
            result_object_id,
            generation_id,
            head_id,
        } => transaction.execute(
            "INSERT INTO store_idempotency
             (namespace, key_digest, meaning_digest, result_object_id, generation_id, head_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                namespace,
                key_digest.as_bytes().as_slice(),
                meaning_digest.as_bytes().as_slice(),
                result_object_id.as_bytes().as_slice(),
                generation_id.as_bytes().as_slice(),
                head_id.as_bytes().as_slice(),
            ],
        )?,
        StoreSnapshotRowV1::Metadata { .. }
        | StoreSnapshotRowV1::PublicationClock { .. }
        | StoreSnapshotRowV1::State { .. }
        | StoreSnapshotRowV1::RetentionRevision { .. }
        | StoreSnapshotRowV1::ActiveHead { .. } => {
            return Err(SnapshotRestoreError::AuthorityRowInsertionAttempt);
        }
    };
    Ok(())
}

fn sql_integer(value: u64, column: &'static str) -> Result<i64, SnapshotRestoreError> {
    i64::try_from(value).map_err(|_| SnapshotRestoreError::SqlIntegerOutOfRange { column, value })
}

#[derive(Debug, Error)]
pub(crate) enum SnapshotRestoreError {
    #[error("source snapshot rows are not in strict canonical order or contain duplicate keys")]
    SourceRowsNotStrictlyOrdered,
    #[error("source snapshot is missing required singleton table {0}")]
    MissingSourceSingleton(&'static str),
    #[error("source snapshot contains duplicate singleton table {0}")]
    DuplicateSourceSingleton(&'static str),
    #[error("source snapshot metadata does not match the expected schema, role, and domain")]
    SourceIdentityMismatch,
    #[error("source snapshot state and active-head audit basis are inconsistent")]
    SourceAuthorityInconsistent,
    #[error("source snapshot audit basis changed before parity verification")]
    SourceAuditBasisMismatch,
    #[error("destination Store authority is not the exact inactive restore basis")]
    DestinationAuthorityMismatch,
    #[error("destination Store contains {0} history rows; exact restore requires an empty Store")]
    DestinationHistoryNotEmpty(i64),
    #[error("restored destination history differs from the exact source typed rows")]
    HistoryParityMismatch,
    #[error("restore attempted to insert a source authority row into destination live state")]
    AuthorityRowInsertionAttempt,
    #[error("Store snapshot integer column {column} exceeds SQLite INTEGER range: {value}")]
    SqlIntegerOutOfRange { column: &'static str, value: u64 },
    #[error(transparent)]
    SnapshotRows(#[from] StoreSnapshotRowError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}
