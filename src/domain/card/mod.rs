//! The card model: one typed entity (SPEC-beads-model.md) that folds features,
//! tasks, harness-backlog items, and decisions into a single flat
//! `.maestro/cards/<id>/` store.
//!
//! The CAS-backed store is the persistence seam; type-specific behavior stays in
//! the owning domain facades.

pub mod archive_db;
pub mod edit;
pub mod fold;
pub mod index;
pub mod live_db;
pub mod locator;
pub mod query;
pub mod schema;
pub mod store;
pub mod suggest;

pub use archive_db::{
    ArchiveDoctorReport, MigrationPlan, MigrationReport, archive_db_file as archive_db_path,
    cleanup_legacy_quarantine as cleanup_legacy_archive_quarantine,
    contains_card_id as archive_contains_card_id, doctor as archive_doctor,
    migrate_legacy_folders as migrate_legacy_archive_folders,
    migration_plan as archive_migration_plan, read_file as read_archived_file,
    resolve as resolve_archived,
};
pub use store::{
    CardArchiveMove, ScannedArchiveItem, archive_card_move, archive_entry_stages,
    archived_card_exists, archived_dir_card_exists, card_dir_move, live_card_exists,
    live_card_hash, live_dir_card_exists, prepare_entry_archive_stages, prepare_live_archive_move,
    prepare_scanned_archive_item, prepare_scanned_archive_move, restore_archived_feature_snapshot,
};
