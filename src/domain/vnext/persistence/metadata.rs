use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::identity::StoreDomainIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborValue};
use crate::foundation::core::secure_fs::{
    CreateIfAbsent, RegularFileBinding, SecureFsError, SecureRoot,
};

use super::{StoreRoleV1, snapshot_rows::StoreSnapshotRowV1};

pub(crate) const METADATA_SCHEMA_VERSION: i64 = 2;
pub(crate) const METADATA_APPLICATION_ID: i64 = 0x4d53_5431;
const METADATA_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DATABASE_INITIALIZATION_ATTEMPTS: usize = 32;
const MAX_AVAILABLE_OBJECT_BYTES_V1: u64 = 7 * 1024 * 1024;
static DATABASE_INITIALIZATION_COUNTER: AtomicU64 = AtomicU64::new(0);

const REQUIRED_TABLES: &[&str] = &[
    "store_metadata",
    "store_publication_clock",
    "store_state",
    "store_retention_revision",
    "store_objects",
    "store_object_references",
    "store_generations",
    "store_generation_roots",
    "store_heads",
    "store_active_head",
    "store_reachability_snapshots",
    "store_reachability_roots",
    "store_reachability_objects",
    "store_retention_pins",
    "store_retention_pin_releases",
    "store_logical_tombstones",
    "store_gc_plans",
    "store_gc_plan_objects",
    "store_gc_collection_occurrences",
    "store_sealed_exports",
    "store_sealed_export_pins",
    "store_sealed_export_objects",
    "store_restore_candidates",
    "store_restore_candidate_roots",
    "store_idempotency",
];

const IMMUTABLE_TABLES: &[&str] = &[
    "store_metadata",
    "store_objects",
    "store_object_references",
    "store_generations",
    "store_generation_roots",
    "store_heads",
    "store_reachability_snapshots",
    "store_reachability_roots",
    "store_reachability_objects",
    "store_retention_pins",
    "store_retention_pin_releases",
    "store_logical_tombstones",
    "store_gc_plans",
    "store_gc_plan_objects",
    "store_gc_collection_occurrences",
    "store_sealed_exports",
    "store_sealed_export_pins",
    "store_sealed_export_objects",
    "store_restore_candidates",
    "store_restore_candidate_roots",
    "store_idempotency",
];

const REQUIRED_MUTABLE_TRIGGERS: &[&str] = &[
    "store_state_monotonic_update",
    "store_state_reject_delete",
    "store_publication_clock_monotonic_update",
    "store_publication_clock_reject_delete",
    "store_retention_revision_monotonic_update",
    "store_retention_revision_reject_delete",
    "store_generations_contiguous_insert",
    "store_heads_generation_match_insert",
    "store_active_head_match_insert",
    "store_active_head_monotonic_update",
    "store_active_head_reject_delete",
    "store_reachability_roots_must_be_reachable_insert",
    "store_reachability_tombstones_must_exist_insert",
    "store_gc_plans_match_snapshot_insert",
    "store_gc_plan_objects_must_be_tombstoned_insert",
    "store_sealed_exports_match_closure_insert",
];

pub(crate) struct MetadataStore {
    connection: Connection,
    write_authorized: Box<AtomicBool>,
    database_path: PathBuf,
    database_identity: DatabaseFileIdentity,
}

pub(crate) enum PublicationMutation<T> {
    Commit(T),
    NoChange(T),
}

pub(crate) enum ConditionalTransactionError<E> {
    Metadata(MetadataError),
    Operation(E),
}

impl<E> From<MetadataError> for ConditionalTransactionError<E> {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl<E> From<rusqlite::Error> for ConditionalTransactionError<E> {
    fn from(error: rusqlite::Error) -> Self {
        Self::Metadata(MetadataError::Sqlite(error))
    }
}

trait PublicationTransactionError: From<MetadataError> + From<rusqlite::Error> {
    fn publication_may_have_committed(&self) -> bool;
}

impl PublicationTransactionError for MetadataError {
    fn publication_may_have_committed(&self) -> bool {
        self.publication_may_have_committed()
    }
}

impl<E> PublicationTransactionError for ConditionalTransactionError<E> {
    fn publication_may_have_committed(&self) -> bool {
        matches!(self, Self::Metadata(error) if error.publication_may_have_committed())
    }
}

struct MetadataReadBindings {
    database: RegularFileBinding,
    wal: Option<RegularFileBinding>,
    shm: Option<RegularFileBinding>,
}

struct MetadataBindingContext<'a> {
    database_path: &'a Path,
    database_identity: DatabaseFileIdentity,
    database_relative_path: &'a Path,
    wal_relative_path: &'a Path,
    shm_relative_path: &'a Path,
    root: &'a SecureRoot,
    bindings: &'a MetadataReadBindings,
}

impl MetadataStore {
    pub(crate) fn open_or_create(
        path: &Path,
        role: StoreRoleV1,
        domain_id: StoreDomainIdV1,
    ) -> Result<Self, MetadataError> {
        reject_database_symlinks(path)?;
        publish_initialized_database_if_absent(path, role, domain_id)?;
        validate_database_leaf(path)?;
        let database_identity = database_file_identity(path)?;
        let connection = open_connection(path)?;
        verify_database_binding(path, database_identity)?;
        configure_local_connection(&connection)?;
        validate_schema(&connection)?;
        verify_store_identity(&connection, role, domain_id)?;
        enable_wal(&connection)?;
        verify_sqlite_handle_binding(&connection, path)?;
        validate_admission_bounds(&connection)?;
        let write_authorized = Box::new(AtomicBool::new(false));
        install_write_authorizer(&connection, &write_authorized)?;
        validate_database_leaf(path)?;
        reject_database_sidecar_symlinks(path)?;
        Ok(Self {
            connection,
            write_authorized,
            database_path: path.to_path_buf(),
            database_identity,
        })
    }

    pub(crate) fn open_existing(
        path: &Path,
        role: StoreRoleV1,
        domain_id: StoreDomainIdV1,
    ) -> Result<Self, MetadataError> {
        reject_database_symlinks(path)?;
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(MetadataError::NotRegularFile(path.to_path_buf())),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(MetadataError::MissingDatabase(path.to_path_buf()));
            }
            Err(source) => {
                return Err(MetadataError::InspectPath {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        validate_database_leaf(path)?;
        let database_identity = database_file_identity(path)?;
        let connection = open_connection(path)?;
        verify_database_binding(path, database_identity)?;
        configure_local_connection(&connection)?;
        validate_schema(&connection)?;
        verify_store_identity(&connection, role, domain_id)?;
        enable_wal(&connection)?;
        verify_sqlite_handle_binding(&connection, path)?;
        validate_admission_bounds(&connection)?;
        let write_authorized = Box::new(AtomicBool::new(false));
        install_write_authorizer(&connection, &write_authorized)?;
        validate_database_leaf(path)?;
        reject_database_sidecar_symlinks(path)?;
        Ok(Self {
            connection,
            write_authorized,
            database_path: path.to_path_buf(),
            database_identity,
        })
    }

    pub(crate) fn with_verified_read<T, E>(
        &self,
        root: &SecureRoot,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<MetadataError>,
    {
        let bindings = self.capture_read_bindings(root).map_err(E::from)?;
        let result = match self.connection.unchecked_transaction() {
            Ok(transaction) => {
                let result = match validate_admission_bounds(&transaction) {
                    Ok(()) => operation(&transaction),
                    Err(error) => Err(E::from(error)),
                };
                drop(transaction);
                result
            }
            Err(source) => Err(E::from(MetadataError::Sqlite(source))),
        };
        if let Err(error) = self.verify_read_bindings(root, &bindings) {
            return Err(E::from(error));
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn connection(&self) -> Result<&Connection, MetadataError> {
        self.verify_database_binding()?;
        Ok(&self.connection)
    }

    #[cfg(test)]
    fn with_test_authorized_connection<T>(&self, operation: impl FnOnce(&Connection) -> T) -> T {
        let _authorization = WriteAuthorizationGuard::new(&self.write_authorized)
            .expect("test write authorization should be exclusive");
        operation(&self.connection)
    }

    pub(crate) fn with_immediate_transaction<T>(
        &mut self,
        root: &SecureRoot,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, MetadataError>,
    ) -> Result<T, MetadataError> {
        self.with_publication_transaction_inner(root, None, None, |transaction| {
            operation(transaction).map(PublicationMutation::Commit)
        })
    }

    pub(crate) fn with_publication_transaction<T>(
        &mut self,
        root: &SecureRoot,
        expected_publication_clock: Option<u64>,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, MetadataError>,
    ) -> Result<T, MetadataError> {
        self.with_publication_transaction_inner(
            root,
            expected_publication_clock,
            None,
            |transaction| operation(transaction).map(PublicationMutation::Commit),
        )
    }

    pub(crate) fn with_restore_transaction<T>(
        &mut self,
        root: &SecureRoot,
        resumed_publication_clock: i64,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, MetadataError>,
    ) -> Result<T, MetadataError> {
        self.with_publication_transaction_inner(
            root,
            Some(0),
            Some(resumed_publication_clock),
            |transaction| operation(transaction).map(PublicationMutation::Commit),
        )
    }

    pub(crate) fn with_prepared_transaction<T, E>(
        &mut self,
        root: &SecureRoot,
        operation: impl FnOnce(
            &Transaction<'_>,
        )
            -> Result<PublicationMutation<T>, ConditionalTransactionError<E>>,
    ) -> Result<T, ConditionalTransactionError<E>> {
        self.with_publication_transaction_inner(root, None, None, operation)
    }

    fn with_publication_transaction_inner<T, E>(
        &mut self,
        root: &SecureRoot,
        expected_publication_clock: Option<u64>,
        resumed_publication_clock: Option<i64>,
        operation: impl FnOnce(&Transaction<'_>) -> Result<PublicationMutation<T>, E>,
    ) -> Result<T, E>
    where
        E: PublicationTransactionError,
    {
        let bindings = self.capture_read_bindings(root)?;
        let database_path = self.database_path.clone();
        let database_identity = self.database_identity;
        let database_relative_path = self.database_relative_path()?.to_path_buf();
        let wal_relative_path = self.sidecar_relative_path("-wal")?;
        let shm_relative_path = self.sidecar_relative_path("-shm")?;
        let binding_context = MetadataBindingContext {
            database_path: &database_path,
            database_identity,
            database_relative_path: &database_relative_path,
            wal_relative_path: &wal_relative_path,
            shm_relative_path: &shm_relative_path,
            root,
            bindings: &bindings,
        };
        let result: Result<(T, bool), E> = (|| -> Result<(T, bool), E> {
            let _authorization = WriteAuthorizationGuard::new(&self.write_authorized)?;
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            verify_database_binding(&database_path, database_identity)?;
            validate_admission_bounds(&transaction)?;
            let observed_clock: i64 = transaction.query_row(
                "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                [],
                |row| row.get(0),
            )?;
            if observed_clock < 0
                || expected_publication_clock
                    .is_some_and(|expected| u64::try_from(observed_clock).ok() != Some(expected))
            {
                return Err(MetadataError::FacadeCasMismatch.into());
            }
            if let Some(resumed_clock) = resumed_publication_clock {
                if resumed_clock <= observed_clock {
                    return Err(MetadataError::FacadeCasMismatch.into());
                }
                let changed = transaction.execute(
                    "UPDATE store_publication_clock
                     SET publication_clock = ?1
                     WHERE singleton = 1 AND publication_clock = ?2",
                    params![resumed_clock, observed_clock],
                )?;
                if changed != 1 {
                    return Err(MetadataError::FacadeCasMismatch.into());
                }
            }
            match operation(&transaction)? {
                PublicationMutation::Commit(value) => {
                    if resumed_publication_clock.is_none() {
                        let changed = transaction.execute(
                            "UPDATE store_publication_clock
                             SET publication_clock = publication_clock + 1
                             WHERE singleton = 1 AND publication_clock = ?1",
                            params![observed_clock],
                        )?;
                        if changed != 1 {
                            return Err(MetadataError::FacadeCasMismatch.into());
                        }
                    }
                    validate_admission_bounds(&transaction)?;
                    run_before_publication_commit_test_hook();
                    verify_read_bindings(&transaction, &binding_context)?;
                    transaction
                        .commit()
                        .map_err(|source| MetadataError::PublicationCommitUncertain { source })?;
                    Ok((value, true))
                }
                PublicationMutation::NoChange(value) => {
                    transaction.rollback()?;
                    Ok((value, false))
                }
            }
        })();
        match result {
            Ok((value, committed)) => {
                if committed {
                    run_after_publication_commit_test_hook();
                }
                self.verify_read_bindings(root, &bindings)
                    .map_err(|source| {
                        if committed {
                            MetadataError::PublicationPostCommitBinding {
                                source: Box::new(source),
                            }
                        } else {
                            source
                        }
                    })?;
                Ok(value)
            }
            Err(error) if error.publication_may_have_committed() => {
                let _ = self.verify_read_bindings(root, &bindings);
                Err(error)
            }
            Err(error) => {
                self.verify_read_bindings(root, &bindings)?;
                Err(error)
            }
        }
    }

    fn verify_database_binding(&self) -> Result<(), MetadataError> {
        verify_database_binding(&self.database_path, self.database_identity)?;
        verify_sqlite_handle_binding(&self.connection, &self.database_path)
    }

    fn capture_read_bindings(
        &self,
        root: &SecureRoot,
    ) -> Result<MetadataReadBindings, MetadataError> {
        root.verify_path_binding()?;
        self.verify_database_binding()?;
        self.stabilize_wal_reader()?;
        self.verify_database_binding()?;
        let database = root.bind_regular_file(self.database_relative_path()?)?;
        let wal = root.bind_optional_regular_file(self.sidecar_relative_path("-wal")?)?;
        let shm = root.bind_optional_regular_file(self.sidecar_relative_path("-shm")?)?;
        Ok(MetadataReadBindings { database, wal, shm })
    }

    fn stabilize_wal_reader(&self) -> Result<(), MetadataError> {
        self.connection.query_row(
            "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
            [],
            |_| Ok(()),
        )?;
        reject_database_sidecar_symlinks(&self.database_path)
    }

    fn verify_read_bindings(
        &self,
        root: &SecureRoot,
        bindings: &MetadataReadBindings,
    ) -> Result<(), MetadataError> {
        let wal_relative_path = self.sidecar_relative_path("-wal")?;
        let shm_relative_path = self.sidecar_relative_path("-shm")?;
        let context = MetadataBindingContext {
            database_path: &self.database_path,
            database_identity: self.database_identity,
            database_relative_path: self.database_relative_path()?,
            wal_relative_path: &wal_relative_path,
            shm_relative_path: &shm_relative_path,
            root,
            bindings,
        };
        verify_read_bindings(&self.connection, &context)
    }

    fn database_relative_path(&self) -> Result<&Path, MetadataError> {
        self.database_path
            .file_name()
            .map(Path::new)
            .ok_or_else(|| MetadataError::InvalidDatabasePath(self.database_path.clone()))
    }

    fn sidecar_relative_path(&self, suffix: &str) -> Result<PathBuf, MetadataError> {
        let leaf = self
            .database_path
            .file_name()
            .ok_or_else(|| MetadataError::InvalidDatabasePath(self.database_path.clone()))?;
        let mut sidecar = leaf.to_os_string();
        sidecar.push(suffix);
        Ok(PathBuf::from(sidecar))
    }
}

fn verify_read_bindings(
    connection: &Connection,
    context: &MetadataBindingContext<'_>,
) -> Result<(), MetadataError> {
    context.root.verify_path_binding()?;
    verify_database_binding(context.database_path, context.database_identity)?;
    verify_sqlite_handle_binding(connection, context.database_path)?;
    context
        .root
        .verify_regular_file_binding(context.database_relative_path, &context.bindings.database)?;
    context.root.verify_optional_regular_file_binding(
        context.wal_relative_path,
        context.bindings.wal.as_ref(),
    )?;
    context.root.verify_optional_regular_file_binding(
        context.shm_relative_path,
        context.bindings.shm.as_ref(),
    )?;
    Ok(())
}

#[cfg(test)]
thread_local! {
    static BEFORE_PUBLICATION_COMMIT_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
    static AFTER_PUBLICATION_COMMIT_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
fn run_before_publication_commit_test_hook() {
    BEFORE_PUBLICATION_COMMIT_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_before_publication_commit_test_hook() {}

#[cfg(test)]
fn run_after_publication_commit_test_hook() {
    AFTER_PUBLICATION_COMMIT_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_after_publication_commit_test_hook() {}

struct WriteAuthorizationGuard<'a> {
    authorized: &'a AtomicBool,
}

impl<'a> WriteAuthorizationGuard<'a> {
    fn new(authorized: &'a AtomicBool) -> Result<Self, MetadataError> {
        authorized
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| MetadataError::NestedWriteAuthorization)?;
        Ok(Self { authorized })
    }
}

impl Drop for WriteAuthorizationGuard<'_> {
    fn drop(&mut self) {
        self.authorized.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DatabaseFileIdentity {
    device: u64,
    inode: u64,
}

fn database_file_identity(path: &Path) -> Result<DatabaseFileIdentity, MetadataError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| MetadataError::InspectPath {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(MetadataError::NotRegularFile(path.to_path_buf()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(DatabaseFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        let modified = metadata
            .modified()
            .map_err(|source| MetadataError::InspectPath {
                path: path.to_path_buf(),
                source,
            })?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| MetadataError::DatabaseBindingChanged(path.to_path_buf()))?;
        Ok(DatabaseFileIdentity {
            device: metadata.len(),
            inode: modified.as_nanos().try_into().unwrap_or(u64::MAX),
        })
    }
}

fn verify_database_binding(
    path: &Path,
    expected: DatabaseFileIdentity,
) -> Result<(), MetadataError> {
    reject_database_symlinks(path)?;
    validate_database_leaf(path)?;
    let observed = database_file_identity(path)?;
    if observed != expected {
        return Err(MetadataError::DatabaseBindingChanged(path.to_path_buf()));
    }
    Ok(())
}

fn verify_sqlite_handle_binding(connection: &Connection, path: &Path) -> Result<(), MetadataError> {
    let database_name = c"main";
    let mut moved = 0_i32;
    let result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            database_name.as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
            std::ptr::addr_of_mut!(moved).cast(),
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        return Err(MetadataError::UnsupportedFileControl {
            operation: "SQLITE_FCNTL_HAS_MOVED(main)",
            result,
        });
    }
    if moved != 0 {
        return Err(MetadataError::DatabaseBindingChanged(path.to_path_buf()));
    }

    let mut journal = std::ptr::null_mut::<rusqlite::ffi::sqlite3_file>();
    let result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            database_name.as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_JOURNAL_POINTER,
            std::ptr::addr_of_mut!(journal).cast(),
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        return Err(MetadataError::UnsupportedFileControl {
            operation: "SQLITE_FCNTL_JOURNAL_POINTER(main)",
            result,
        });
    }
    if !journal.is_null() {
        let methods = unsafe { (*journal).pMethods };
        if methods.is_null() {
            return Ok(());
        }
        let file_control =
            unsafe { (*methods).xFileControl }.ok_or(MetadataError::MissingSqliteFileControl)?;
        let mut journal_moved = 0_i32;
        let result = unsafe {
            file_control(
                journal,
                rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
                std::ptr::addr_of_mut!(journal_moved).cast(),
            )
        };
        if result != rusqlite::ffi::SQLITE_OK {
            return Err(MetadataError::UnsupportedFileControl {
                operation: "SQLITE_FCNTL_HAS_MOVED(journal)",
                result,
            });
        }
        if journal_moved != 0 {
            return Err(MetadataError::DatabaseSidecarBindingChanged(
                path.to_path_buf(),
            ));
        }
    }
    Ok(())
}

fn install_write_authorizer(
    connection: &Connection,
    write_authorized: &AtomicBool,
) -> Result<(), MetadataError> {
    let result = unsafe {
        rusqlite::ffi::sqlite3_set_authorizer(
            connection.handle(),
            Some(metadata_authorizer),
            std::ptr::from_ref(write_authorized).cast_mut().cast(),
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        return Err(MetadataError::AuthorizerInstall(result));
    }
    Ok(())
}

unsafe extern "C" fn metadata_authorizer(
    context: *mut std::ffi::c_void,
    action: i32,
    _argument_one: *const std::ffi::c_char,
    argument_two: *const std::ffi::c_char,
    _database: *const std::ffi::c_char,
    _trigger: *const std::ffi::c_char,
) -> i32 {
    let write_authorized = unsafe { &*context.cast::<AtomicBool>() };
    if write_authorized.load(Ordering::SeqCst) {
        return rusqlite::ffi::SQLITE_OK;
    }
    match action {
        rusqlite::ffi::SQLITE_SELECT
        | rusqlite::ffi::SQLITE_READ
        | rusqlite::ffi::SQLITE_FUNCTION
        | rusqlite::ffi::SQLITE_RECURSIVE
        | rusqlite::ffi::SQLITE_TRANSACTION
        | rusqlite::ffi::SQLITE_SAVEPOINT => rusqlite::ffi::SQLITE_OK,
        rusqlite::ffi::SQLITE_PRAGMA if argument_two.is_null() => rusqlite::ffi::SQLITE_OK,
        _ => rusqlite::ffi::SQLITE_DENY,
    }
}

fn publish_initialized_database_if_absent(
    path: &Path,
    role: StoreRoleV1,
    domain_id: StoreDomainIdV1,
) -> Result<CreateIfAbsent, MetadataError> {
    let file_name = path
        .file_name()
        .ok_or_else(|| MetadataError::InvalidDatabasePath(path.to_path_buf()))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let root = SecureRoot::open(parent)?;
    if root.validate_regular_file(Path::new(file_name)).is_ok() {
        return Ok(CreateIfAbsent::AlreadyExists);
    }
    if path.exists() {
        root.validate_regular_file(Path::new(file_name))?;
    }

    for _ in 0..DATABASE_INITIALIZATION_ATTEMPTS {
        let sequence = DATABASE_INITIALIZATION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_name = format!(".maestro-store-init-{}-{sequence}.sqlite", process::id());
        if root.create_file_if_absent(&temporary_name, &[])? == CreateIfAbsent::AlreadyExists {
            continue;
        }
        let temporary_path = parent.join(&temporary_name);
        if let Err(error) = initialize_database_file(&temporary_path, role, domain_id) {
            cleanup_initialized_temporary(&root, &temporary_name);
            return Err(error);
        }
        match root.rename_file_no_replace(&temporary_name, Path::new(file_name))? {
            CreateIfAbsent::Created => return Ok(CreateIfAbsent::Created),
            CreateIfAbsent::AlreadyExists => {
                cleanup_initialized_temporary(&root, &temporary_name);
                root.validate_regular_file(Path::new(file_name))?;
                return Ok(CreateIfAbsent::AlreadyExists);
            }
        }
    }
    Err(MetadataError::InitializationNameExhausted(
        path.to_path_buf(),
    ))
}

fn initialize_database_file(
    path: &Path,
    role: StoreRoleV1,
    domain_id: StoreDomainIdV1,
) -> Result<(), MetadataError> {
    let mut connection = open_connection(path)?;
    configure_local_connection(&connection)?;
    initialize_schema(&mut connection, role, domain_id)?;
    validate_schema(&connection)?;
    verify_store_identity(&connection, role, domain_id)?;
    validate_admission_bounds(&connection)?;
    drop(connection);
    validate_database_leaf(path)
}

fn cleanup_initialized_temporary(root: &SecureRoot, temporary_name: &str) {
    let Ok(bytes) = root.read_immutable(temporary_name) else {
        return;
    };
    let _ = root.remove_file_if_matches(temporary_name, &bytes);
}

fn validate_database_leaf(path: &Path) -> Result<(), MetadataError> {
    let file_name = path
        .file_name()
        .ok_or_else(|| MetadataError::InvalidDatabasePath(path.to_path_buf()))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    SecureRoot::open(parent)?.validate_regular_file(Path::new(file_name))?;
    Ok(())
}

fn open_connection(path: &Path) -> Result<Connection, MetadataError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
        | OpenFlags::SQLITE_OPEN_EXRESCODE;
    Connection::open_with_flags(path, flags).map_err(|source| MetadataError::Open {
        path: path.to_path_buf(),
        source,
    })
}

fn reject_database_symlinks(path: &Path) -> Result<(), MetadataError> {
    reject_symlink(path)?;
    reject_database_sidecar_symlinks(path)
}

fn reject_database_sidecar_symlinks(path: &Path) -> Result<(), MetadataError> {
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let sidecar = PathBuf::from(sidecar);
        reject_symlink(&sidecar)?;
        validate_optional_database_leaf(&sidecar)?;
    }
    Ok(())
}

fn validate_optional_database_leaf(path: &Path) -> Result<(), MetadataError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(MetadataError::NotRegularFile(path.to_path_buf()));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.nlink() != 1 {
                    return Err(MetadataError::HardLinkedDatabase(path.to_path_buf()));
                }
            }
            let file_name = path
                .file_name()
                .ok_or_else(|| MetadataError::InvalidDatabasePath(path.to_path_buf()))?;
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            SecureRoot::open(parent)?.validate_regular_file(Path::new(file_name))?;
            Ok(())
        }
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(()),
        Err(source) => Err(MetadataError::InspectPath {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn reject_symlink(path: &Path) -> Result<(), MetadataError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(MetadataError::SymbolicLink(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(source) => Err(MetadataError::InspectPath {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn configure_local_connection(connection: &Connection) -> Result<(), MetadataError> {
    connection.busy_timeout(METADATA_BUSY_TIMEOUT)?;
    verify_integer_pragma(
        connection,
        "busy_timeout",
        METADATA_BUSY_TIMEOUT.as_millis() as i64,
    )?;

    connection.pragma_update(None, "foreign_keys", "ON")?;
    verify_integer_pragma(connection, "foreign_keys", 1)?;

    connection.pragma_update(None, "recursive_triggers", "ON")?;
    verify_integer_pragma(connection, "recursive_triggers", 1)?;

    connection.pragma_update(None, "synchronous", "FULL")?;
    verify_integer_pragma(connection, "synchronous", 2)?;
    Ok(())
}

fn enable_wal(connection: &Connection) -> Result<(), MetadataError> {
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(MetadataError::UnsupportedCapability {
            capability: "journal_mode",
            expected: "wal".to_owned(),
            actual: journal_mode,
        });
    }
    verify_integer_pragma(connection, "synchronous", 2)?;
    Ok(())
}

fn verify_integer_pragma(
    connection: &Connection,
    pragma: &'static str,
    expected: i64,
) -> Result<(), MetadataError> {
    let sql = format!("PRAGMA {pragma}");
    let actual: i64 = connection.query_row(&sql, [], |row| row.get(0))?;
    if actual != expected {
        return Err(MetadataError::UnsupportedCapability {
            capability: pragma,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn initialize_schema(
    connection: &mut Connection,
    role: StoreRoleV1,
    domain_id: StoreDomainIdV1,
) -> Result<(), MetadataError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if !schema_object_exists(&transaction, "table", "store_metadata")? {
        transaction.execute_batch(SCHEMA_SQL)?;
        install_immutability_triggers(&transaction)?;
        transaction.pragma_update(None, "application_id", METADATA_APPLICATION_ID)?;
        transaction.pragma_update(None, "user_version", METADATA_SCHEMA_VERSION)?;
        transaction.execute(
            "INSERT INTO store_metadata
             (singleton, schema_version, store_role, domain_id)
             VALUES (1, ?1, ?2, ?3)",
            params![
                METADATA_SCHEMA_VERSION,
                role.tag() as i64,
                domain_id.as_bytes()
            ],
        )?;
    }
    transaction.commit()?;
    validate_schema(connection)
}

fn install_immutability_triggers(transaction: &Transaction<'_>) -> Result<(), MetadataError> {
    for table in IMMUTABLE_TABLES {
        let update_trigger = format!(
            "CREATE TRIGGER IF NOT EXISTS {table}_reject_update
             BEFORE UPDATE ON {table}
             BEGIN SELECT RAISE(ABORT, '{table} is insert-only'); END"
        );
        let delete_trigger = format!(
            "CREATE TRIGGER IF NOT EXISTS {table}_reject_delete
             BEFORE DELETE ON {table}
             BEGIN SELECT RAISE(ABORT, '{table} is insert-only'); END"
        );
        transaction.execute_batch(&update_trigger)?;
        transaction.execute_batch(&delete_trigger)?;
    }
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), MetadataError> {
    verify_integer_pragma(connection, "application_id", METADATA_APPLICATION_ID)?;
    let schema_version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if schema_version != METADATA_SCHEMA_VERSION {
        return Err(MetadataError::UnsupportedSchemaVersion {
            expected: METADATA_SCHEMA_VERSION,
            actual: schema_version,
        });
    }
    for table in REQUIRED_TABLES {
        if !schema_object_exists(connection, "table", table)? {
            return Err(MetadataError::MissingSchemaObject {
                object_type: "table",
                name: table,
            });
        }
    }
    for table in IMMUTABLE_TABLES {
        for suffix in ["reject_update", "reject_delete"] {
            let name = format!("{table}_{suffix}");
            if !schema_object_exists(connection, "trigger", &name)? {
                return Err(MetadataError::MissingOwnedSchemaObject {
                    object_type: "trigger",
                    name,
                });
            }
        }
    }
    for trigger in REQUIRED_MUTABLE_TRIGGERS {
        if !schema_object_exists(connection, "trigger", trigger)? {
            return Err(MetadataError::MissingSchemaObject {
                object_type: "trigger",
                name: trigger,
            });
        }
    }
    let expected = expected_schema_rows()?;
    if schema_rows_bounded(connection, expected.len() + 1)? != expected {
        return Err(MetadataError::SchemaDefinitionMismatch);
    }
    validate_database_integrity(connection)?;
    Ok(())
}

fn validate_database_integrity(connection: &Connection) -> Result<(), MetadataError> {
    let integrity: String =
        connection.query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(MetadataError::DatabaseIntegrityCheckFailed);
    }
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut violations = statement.query([])?;
    if violations.next()?.is_some() {
        return Err(MetadataError::ForeignKeyIntegrityCheckFailed);
    }
    Ok(())
}

fn validate_available_object_capacity(connection: &Connection) -> Result<(), MetadataError> {
    let mut statement = connection.prepare(
        "SELECT stored_byte_length
         FROM store_objects AS object
         WHERE NOT EXISTS (
             SELECT 1 FROM store_logical_tombstones AS tombstone
             WHERE tombstone.object_id = object.object_id
         )
         AND NOT EXISTS (
             SELECT 1 FROM store_gc_collection_occurrences AS occurrence
             WHERE occurrence.object_id = object.object_id
         )
         ORDER BY object.object_id",
    )?;
    let mut rows = statement.query([])?;
    let mut count = 0_usize;
    let mut aggregate = 0_u64;
    while let Some(row) = rows.next()? {
        count = count
            .checked_add(1)
            .ok_or(MetadataError::AvailableObjectCapacityExceeded)?;
        if count > 65_536 {
            return Err(MetadataError::AvailableObjectCapacityExceeded);
        }
        let signed: i64 = row.get(0)?;
        let length =
            u64::try_from(signed).map_err(|_| MetadataError::AvailableObjectCapacityExceeded)?;
        if length > MAX_AVAILABLE_OBJECT_BYTES_V1 {
            return Err(MetadataError::AvailableObjectCapacityExceeded);
        }
        aggregate = aggregate
            .checked_add(length)
            .ok_or(MetadataError::AvailableObjectCapacityExceeded)?;
        if aggregate > MAX_AVAILABLE_OBJECT_BYTES_V1 {
            return Err(MetadataError::AvailableObjectCapacityExceeded);
        }
    }
    Ok(())
}

fn validate_admission_bounds(connection: &Connection) -> Result<(), MetadataError> {
    StoreSnapshotRowV1::validate_connection_bounds(connection)
        .map_err(|error| MetadataError::SnapshotAdmission(error.to_string()))?;
    validate_available_object_capacity(connection)
}

fn expected_schema_rows() -> Result<Vec<SchemaRow>, MetadataError> {
    let mut connection = Connection::open_in_memory()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(SCHEMA_SQL)?;
    install_immutability_triggers(&transaction)?;
    transaction.commit()?;
    schema_rows(&connection)
}

pub(crate) fn expected_schema_digest() -> Result<[u8; 32], MetadataError> {
    schema_digest_for_rows(&expected_schema_rows()?)
}

pub(crate) fn schema_digest(connection: &Connection) -> Result<[u8; 32], MetadataError> {
    let expected_count = expected_schema_rows()?.len();
    schema_digest_for_rows(&schema_rows_bounded(connection, expected_count + 1)?)
}

fn schema_digest_for_rows(rows: &[SchemaRow]) -> Result<[u8; 32], MetadataError> {
    let value = CborValue::Array(
        rows.iter()
            .map(|(object_type, name, table, sql)| {
                Ok(CborValue::Array(vec![
                    CborValue::text(object_type)?,
                    CborValue::text(name)?,
                    CborValue::text(table)?,
                    CborValue::optional(match sql {
                        Some(sql) => Some(CborValue::text(sql)?),
                        None => None,
                    }),
                ]))
            })
            .collect::<Result<Vec<_>, crate::foundation::core::deterministic_cbor::CborError>>()?,
    );
    Ok(Sha256::digest(deterministic_cbor::encode(&value)?).into())
}

type SchemaRow = (String, String, String, Option<String>);

fn schema_rows(connection: &Connection) -> Result<Vec<SchemaRow>, MetadataError> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name, tbl_name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn schema_rows_bounded(
    connection: &Connection,
    maximum_rows: usize,
) -> Result<Vec<SchemaRow>, MetadataError> {
    let maximum_rows =
        i64::try_from(maximum_rows).map_err(|_| MetadataError::SchemaDefinitionMismatch)?;
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name, tbl_name
         LIMIT ?1",
    )?;
    let rows = statement.query_map(params![maximum_rows], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn schema_object_exists(
    connection: &Connection,
    object_type: &str,
    name: &str,
) -> Result<bool, MetadataError> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2",
            params![object_type, name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn verify_store_identity(
    connection: &Connection,
    role: StoreRoleV1,
    domain_id: StoreDomainIdV1,
) -> Result<(), MetadataError> {
    let stored = connection
        .query_row(
            "SELECT schema_version, store_role, domain_id
             FROM store_metadata WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((schema_version, stored_role, stored_domain)) = stored else {
        return Err(MetadataError::MissingStoreMetadata);
    };
    if schema_version != METADATA_SCHEMA_VERSION
        || stored_role != role.tag() as i64
        || stored_domain.as_slice() != domain_id.as_bytes()
    {
        return Err(MetadataError::StoreIdentityMismatch);
    }
    Ok(())
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE store_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version = 2),
    store_role INTEGER NOT NULL CHECK (store_role IN (1, 2)),
    domain_id BLOB NOT NULL CHECK (length(domain_id) = 32)
) STRICT;

CREATE TABLE store_publication_clock (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    publication_clock INTEGER NOT NULL CHECK (publication_clock >= 0)
) STRICT;
INSERT INTO store_publication_clock(singleton, publication_clock) VALUES (1, 0);

CREATE TRIGGER store_publication_clock_monotonic_update
BEFORE UPDATE ON store_publication_clock
WHEN NEW.singleton != OLD.singleton
  OR NEW.publication_clock <= OLD.publication_clock
  OR (
      NEW.publication_clock != OLD.publication_clock + 1
      AND NOT (
          OLD.publication_clock = 0
          AND EXISTS (
              SELECT 1 FROM store_state
              WHERE singleton = 1 AND state = 'inactive' AND state_revision = 0
          )
          AND NOT EXISTS (SELECT 1 FROM store_active_head)
          AND NOT EXISTS (SELECT 1 FROM store_heads)
      )
  )
BEGIN SELECT RAISE(ABORT, 'publication clock must advance by one or perform a pristine inactive restore rebase'); END;
CREATE TRIGGER store_publication_clock_reject_delete
BEFORE DELETE ON store_publication_clock
BEGIN SELECT RAISE(ABORT, 'publication clock singleton cannot be deleted'); END;

CREATE TABLE store_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    state TEXT NOT NULL CHECK (state IN ('active', 'inactive')),
    state_revision INTEGER NOT NULL CHECK (state_revision >= 0)
) STRICT;
INSERT INTO store_state(singleton, state, state_revision) VALUES (1, 'inactive', 0);

CREATE TRIGGER store_state_monotonic_update
BEFORE UPDATE ON store_state
WHEN NEW.singleton != OLD.singleton
  OR OLD.state != 'inactive'
  OR NEW.state != 'active'
  OR NEW.state_revision != OLD.state_revision + 1
  OR NOT EXISTS (SELECT 1 FROM store_active_head WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'Store activation must advance inactive to active after publishing an active head'); END;
CREATE TRIGGER store_state_reject_delete
BEFORE DELETE ON store_state
BEGIN SELECT RAISE(ABORT, 'store state singleton cannot be deleted'); END;

CREATE TABLE store_retention_revision (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    retention_revision INTEGER NOT NULL CHECK (retention_revision >= 0)
) STRICT;
INSERT INTO store_retention_revision(singleton, retention_revision) VALUES (1, 0);

CREATE TRIGGER store_retention_revision_monotonic_update
BEFORE UPDATE ON store_retention_revision
WHEN NEW.singleton != OLD.singleton
  OR NEW.retention_revision != OLD.retention_revision + 1
BEGIN SELECT RAISE(ABORT, 'retention revision must advance by exactly one'); END;
CREATE TRIGGER store_retention_revision_reject_delete
BEFORE DELETE ON store_retention_revision
BEGIN SELECT RAISE(ABORT, 'retention revision singleton cannot be deleted'); END;

CREATE TABLE store_objects (
    object_id BLOB PRIMARY KEY CHECK (length(object_id) = 32),
    schema_id BLOB NOT NULL CHECK (length(schema_id) = 32),
    logical_byte_length INTEGER NOT NULL CHECK (logical_byte_length >= 0),
    stored_byte_length INTEGER NOT NULL CHECK (stored_byte_length > 0),
    stored_bytes_digest BLOB NOT NULL CHECK (length(stored_bytes_digest) = 32),
    storage_codec TEXT NOT NULL CHECK (length(storage_codec) BETWEEN 1 AND 64),
    key_envelope_id BLOB CHECK (key_envelope_id IS NULL OR length(key_envelope_id) = 32),
    key_envelope_kind TEXT CHECK (
        (key_envelope_id IS NULL AND key_envelope_kind IS NULL)
        OR (
            key_envelope_id IS NOT NULL
            AND key_envelope_kind IS NOT NULL
            AND length(key_envelope_kind) BETWEEN 1 AND 64
        )
    )
) STRICT, WITHOUT ROWID;

CREATE TABLE store_object_references (
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    reference_position INTEGER NOT NULL CHECK (reference_position >= 0),
    referenced_object_id BLOB NOT NULL CHECK (length(referenced_object_id) = 32),
    PRIMARY KEY (object_id, reference_position),
    UNIQUE (object_id, referenced_object_id),
    CHECK (object_id != referenced_object_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id),
    FOREIGN KEY (referenced_object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_generations (
    generation_id BLOB PRIMARY KEY CHECK (length(generation_id) = 32),
    generation_ordinal INTEGER NOT NULL CHECK (generation_ordinal > 0),
    previous_generation_id BLOB CHECK (
        previous_generation_id IS NULL OR length(previous_generation_id) = 32
    ),
    contract_root_id BLOB NOT NULL CHECK (length(contract_root_id) = 32),
    writer_compatibility_manifest_id BLOB NOT NULL CHECK (length(writer_compatibility_manifest_id) = 32),
    association_schema_id BLOB NOT NULL CHECK (length(association_schema_id) = 32),
    finality_edge_manifest_id BLOB NOT NULL CHECK (length(finality_edge_manifest_id) = 32),
    schema_read_write_set_descriptor_id BLOB NOT NULL CHECK (length(schema_read_write_set_descriptor_id) = 32),
    writer_protocol_epoch_id BLOB NOT NULL CHECK (length(writer_protocol_epoch_id) = 32),
    migration_epoch_id BLOB NOT NULL CHECK (length(migration_epoch_id) = 32),
    CHECK (
        (generation_ordinal = 1 AND previous_generation_id IS NULL)
        OR (generation_ordinal > 1 AND previous_generation_id IS NOT NULL)
    ),
    FOREIGN KEY (previous_generation_id) REFERENCES store_generations(generation_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER store_generations_contiguous_insert
BEFORE INSERT ON store_generations
WHEN NEW.generation_ordinal > 1 AND NOT EXISTS (
    SELECT 1 FROM store_generations
    WHERE generation_id = NEW.previous_generation_id
      AND generation_ordinal = NEW.generation_ordinal - 1
)
BEGIN SELECT RAISE(ABORT, 'generation predecessor must be the immediately prior ordinal'); END;

CREATE TABLE store_generation_roots (
    generation_id BLOB NOT NULL CHECK (length(generation_id) = 32),
    root_position INTEGER NOT NULL CHECK (root_position >= 0),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    PRIMARY KEY (generation_id, root_position),
    UNIQUE (generation_id, object_id),
    FOREIGN KEY (generation_id) REFERENCES store_generations(generation_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_heads (
    head_id BLOB PRIMARY KEY CHECK (length(head_id) = 32),
    generation_id BLOB NOT NULL UNIQUE CHECK (length(generation_id) = 32),
    generation_ordinal INTEGER NOT NULL UNIQUE CHECK (generation_ordinal > 0),
    head_revision INTEGER NOT NULL UNIQUE CHECK (head_revision > 0),
    previous_head_id BLOB UNIQUE CHECK (previous_head_id IS NULL OR length(previous_head_id) = 32),
    CHECK (generation_ordinal = head_revision),
    CHECK (
        (head_revision = 1 AND previous_head_id IS NULL)
        OR (head_revision > 1 AND previous_head_id IS NOT NULL)
    ),
    FOREIGN KEY (generation_id) REFERENCES store_generations(generation_id),
    FOREIGN KEY (previous_head_id) REFERENCES store_heads(head_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER store_heads_generation_match_insert
BEFORE INSERT ON store_heads
WHEN NOT EXISTS (
    SELECT 1 FROM store_generations
    WHERE generation_id = NEW.generation_id
      AND generation_ordinal = NEW.generation_ordinal
)
OR (
    NEW.head_revision > 1 AND NOT EXISTS (
        SELECT 1 FROM store_heads
        WHERE head_id = NEW.previous_head_id
          AND head_revision = NEW.head_revision - 1
    )
)
BEGIN SELECT RAISE(ABORT, 'head must bind its generation and immediately prior head'); END;

CREATE TABLE store_active_head (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    head_id BLOB NOT NULL UNIQUE CHECK (length(head_id) = 32),
    head_revision INTEGER NOT NULL UNIQUE CHECK (head_revision > 0),
    FOREIGN KEY (head_id) REFERENCES store_heads(head_id)
) STRICT;

CREATE TRIGGER store_active_head_match_insert
BEFORE INSERT ON store_active_head
WHEN NOT EXISTS (
    SELECT 1 FROM store_heads
    WHERE head_id = NEW.head_id AND head_revision = NEW.head_revision
)
BEGIN SELECT RAISE(ABORT, 'active head must bind an existing head revision'); END;
CREATE TRIGGER store_active_head_monotonic_update
BEFORE UPDATE ON store_active_head
WHEN NEW.singleton != OLD.singleton
  OR NEW.head_revision != OLD.head_revision + 1
  OR NOT EXISTS (
      SELECT 1 FROM store_heads
      WHERE head_id = NEW.head_id
        AND head_revision = NEW.head_revision
        AND previous_head_id = OLD.head_id
  )
BEGIN SELECT RAISE(ABORT, 'active head must advance through the immutable head chain'); END;
CREATE TRIGGER store_active_head_reject_delete
BEFORE DELETE ON store_active_head
BEGIN SELECT RAISE(ABORT, 'active head singleton cannot be deleted'); END;

CREATE TABLE store_reachability_snapshots (
    snapshot_id BLOB PRIMARY KEY CHECK (length(snapshot_id) = 32),
    head_id BLOB NOT NULL CHECK (length(head_id) = 32),
    retention_revision INTEGER NOT NULL CHECK (retention_revision > 0),
    FOREIGN KEY (head_id) REFERENCES store_heads(head_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_reachability_roots (
    snapshot_id BLOB NOT NULL CHECK (length(snapshot_id) = 32),
    root_position INTEGER NOT NULL CHECK (root_position >= 0),
    root_kind INTEGER NOT NULL CHECK (root_kind BETWEEN 1 AND 14),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    PRIMARY KEY (snapshot_id, root_position),
    UNIQUE (snapshot_id, root_kind, object_id),
    FOREIGN KEY (snapshot_id) REFERENCES store_reachability_snapshots(snapshot_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_reachability_objects (
    snapshot_id BLOB NOT NULL CHECK (length(snapshot_id) = 32),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    reachability_status TEXT NOT NULL CHECK (reachability_status IN ('reachable', 'tombstoned')),
    PRIMARY KEY (snapshot_id, object_id),
    FOREIGN KEY (snapshot_id) REFERENCES store_reachability_snapshots(snapshot_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER store_reachability_tombstones_must_exist_insert
BEFORE INSERT ON store_reachability_objects
WHEN NEW.reachability_status = 'tombstoned' AND NOT EXISTS (
    SELECT 1 FROM store_logical_tombstones
    WHERE object_id = NEW.object_id
)
BEGIN SELECT RAISE(ABORT, 'tombstoned reachability rows require a logical tombstone'); END;

CREATE TRIGGER store_reachability_roots_must_be_reachable_insert
BEFORE INSERT ON store_reachability_roots
WHEN NOT EXISTS (
    SELECT 1 FROM store_reachability_objects
    WHERE snapshot_id = NEW.snapshot_id
      AND object_id = NEW.object_id
      AND reachability_status = 'reachable'
)
BEGIN SELECT RAISE(ABORT, 'retention roots must belong to the reachable closure'); END;

CREATE TABLE store_retention_pins (
    pin_id BLOB PRIMARY KEY CHECK (length(pin_id) = 32),
    basis_head_id BLOB NOT NULL CHECK (length(basis_head_id) = 32),
    root_kind INTEGER NOT NULL CHECK (root_kind BETWEEN 1 AND 14),
    root_object_id BLOB NOT NULL CHECK (length(root_object_id) = 32),
    reason_digest BLOB NOT NULL CHECK (length(reason_digest) = 32),
    FOREIGN KEY (basis_head_id) REFERENCES store_heads(head_id),
    FOREIGN KEY (root_object_id) REFERENCES store_objects(object_id),
    UNIQUE (basis_head_id, root_kind, root_object_id, reason_digest)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_retention_pin_releases (
    pin_id BLOB PRIMARY KEY CHECK (length(pin_id) = 32),
    released_at_head_id BLOB NOT NULL CHECK (length(released_at_head_id) = 32),
    reason_digest BLOB NOT NULL CHECK (length(reason_digest) = 32),
    FOREIGN KEY (pin_id) REFERENCES store_retention_pins(pin_id),
    FOREIGN KEY (released_at_head_id) REFERENCES store_heads(head_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_logical_tombstones (
    tombstone_id BLOB PRIMARY KEY CHECK (length(tombstone_id) = 32),
    basis_head_id BLOB NOT NULL CHECK (length(basis_head_id) = 32),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    reason_digest BLOB NOT NULL CHECK (length(reason_digest) = 32),
    invalidation_digest BLOB NOT NULL CHECK (length(invalidation_digest) = 32),
    FOREIGN KEY (basis_head_id) REFERENCES store_heads(head_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id),
    UNIQUE (basis_head_id, object_id, reason_digest, invalidation_digest)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_gc_plans (
    plan_id BLOB PRIMARY KEY CHECK (length(plan_id) = 32),
    snapshot_id BLOB NOT NULL CHECK (length(snapshot_id) = 32),
    head_id BLOB NOT NULL CHECK (length(head_id) = 32),
    retention_revision INTEGER NOT NULL CHECK (retention_revision > 0),
    FOREIGN KEY (snapshot_id) REFERENCES store_reachability_snapshots(snapshot_id),
    FOREIGN KEY (head_id) REFERENCES store_heads(head_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER store_gc_plans_match_snapshot_insert
BEFORE INSERT ON store_gc_plans
WHEN NOT EXISTS (
    SELECT 1 FROM store_reachability_snapshots
    WHERE snapshot_id = NEW.snapshot_id
      AND head_id = NEW.head_id
      AND retention_revision = NEW.retention_revision
)
BEGIN SELECT RAISE(ABORT, 'GC plan must bind one coherent reachability snapshot'); END;

CREATE TABLE store_gc_plan_objects (
    plan_id BLOB NOT NULL CHECK (length(plan_id) = 32),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    PRIMARY KEY (plan_id, object_id),
    FOREIGN KEY (plan_id) REFERENCES store_gc_plans(plan_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_gc_collection_occurrences (
    plan_id BLOB NOT NULL CHECK (length(plan_id) = 32),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    stored_bytes_digest BLOB NOT NULL CHECK (length(stored_bytes_digest) = 32),
    PRIMARY KEY (plan_id, object_id),
    FOREIGN KEY (plan_id, object_id) REFERENCES store_gc_plan_objects(plan_id, object_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER store_gc_plan_objects_must_be_tombstoned_insert
BEFORE INSERT ON store_gc_plan_objects
WHEN NOT EXISTS (
    SELECT 1
    FROM store_gc_plans AS plan
    JOIN store_reachability_objects AS object
      ON object.snapshot_id = plan.snapshot_id
    WHERE plan.plan_id = NEW.plan_id
      AND object.object_id = NEW.object_id
      AND object.reachability_status = 'tombstoned'
)
OR EXISTS (
    SELECT 1
    FROM store_sealed_export_objects AS export_object
    WHERE export_object.object_id = NEW.object_id
      AND export_object.entry_kind = 'available'
)
BEGIN SELECT RAISE(ABORT, 'GC candidates must be tombstoned, unreachable, and free of sealed-export retention holds'); END;

CREATE TABLE store_sealed_exports (
    export_id BLOB PRIMARY KEY CHECK (length(export_id) = 32),
    head_id BLOB NOT NULL CHECK (length(head_id) = 32),
    generation_id BLOB NOT NULL CHECK (length(generation_id) = 32),
    snapshot_id BLOB NOT NULL CHECK (length(snapshot_id) = 32),
    export_byte_length INTEGER NOT NULL CHECK (export_byte_length > 0),
    export_bytes_digest BLOB NOT NULL CHECK (length(export_bytes_digest) = 32),
    export_format TEXT NOT NULL CHECK (length(export_format) BETWEEN 1 AND 64),
    schema_manifest_id BLOB NOT NULL CHECK (length(schema_manifest_id) = 32),
    family_manifest_set_digest BLOB NOT NULL CHECK (length(family_manifest_set_digest) = 32),
    snapshot_root_id BLOB NOT NULL CHECK (length(snapshot_root_id) = 32),
    source_publication_clock INTEGER NOT NULL CHECK (source_publication_clock >= 0),
    committed_publication_clock INTEGER NOT NULL CHECK (
        committed_publication_clock = source_publication_clock + 1
    ),
    payload_set_digest BLOB NOT NULL CHECK (length(payload_set_digest) = 32),
    backup_receipt_id BLOB NOT NULL CHECK (length(backup_receipt_id) = 32),
    carrier_format TEXT NOT NULL CHECK (length(carrier_format) BETWEEN 1 AND 64),
    FOREIGN KEY (head_id) REFERENCES store_heads(head_id),
    FOREIGN KEY (generation_id) REFERENCES store_generations(generation_id),
    FOREIGN KEY (snapshot_id) REFERENCES store_reachability_snapshots(snapshot_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER store_sealed_exports_match_closure_insert
BEFORE INSERT ON store_sealed_exports
WHEN NOT EXISTS (
    SELECT 1
    FROM store_heads AS head
    JOIN store_reachability_snapshots AS snapshot ON snapshot.head_id = head.head_id
    WHERE head.head_id = NEW.head_id
      AND head.generation_id = NEW.generation_id
      AND snapshot.snapshot_id = NEW.snapshot_id
)
BEGIN SELECT RAISE(ABORT, 'sealed export must bind one Generation and its coherent snapshot'); END;

CREATE TABLE store_sealed_export_objects (
    export_id BLOB NOT NULL CHECK (length(export_id) = 32),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    entry_kind TEXT NOT NULL CHECK (entry_kind IN ('available', 'tombstoned')),
    PRIMARY KEY (export_id, object_id),
    FOREIGN KEY (export_id) REFERENCES store_sealed_exports(export_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_sealed_export_pins (
    export_id BLOB NOT NULL CHECK (length(export_id) = 32),
    pin_id BLOB NOT NULL CHECK (length(pin_id) = 32),
    PRIMARY KEY (export_id, pin_id),
    FOREIGN KEY (export_id) REFERENCES store_sealed_exports(export_id),
    FOREIGN KEY (pin_id) REFERENCES store_retention_pins(pin_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_restore_candidates (
    candidate_id BLOB PRIMARY KEY CHECK (length(candidate_id) = 32),
    source_export_id BLOB NOT NULL CHECK (length(source_export_id) = 32),
    source_domain_id BLOB NOT NULL CHECK (length(source_domain_id) = 32),
    source_export_bytes_digest BLOB NOT NULL CHECK (length(source_export_bytes_digest) = 32),
    destination_domain_id BLOB NOT NULL CHECK (length(destination_domain_id) = 32),
    candidate_generation_id BLOB NOT NULL CHECK (length(candidate_generation_id) = 32),
    candidate_head_id BLOB NOT NULL CHECK (length(candidate_head_id) = 32),
    candidate_snapshot_id BLOB NOT NULL CHECK (length(candidate_snapshot_id) = 32),
    verification_digest BLOB NOT NULL CHECK (length(verification_digest) = 32),
    source_schema_manifest_id BLOB NOT NULL CHECK (length(source_schema_manifest_id) = 32),
    source_snapshot_root_id BLOB NOT NULL CHECK (length(source_snapshot_root_id) = 32),
    FOREIGN KEY (source_export_id) REFERENCES store_sealed_exports(export_id),
    FOREIGN KEY (candidate_generation_id) REFERENCES store_generations(generation_id),
    FOREIGN KEY (candidate_head_id) REFERENCES store_heads(head_id),
    FOREIGN KEY (candidate_snapshot_id) REFERENCES store_reachability_snapshots(snapshot_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_restore_candidate_roots (
    candidate_id BLOB NOT NULL CHECK (length(candidate_id) = 32),
    root_position INTEGER NOT NULL CHECK (root_position >= 0),
    object_id BLOB NOT NULL CHECK (length(object_id) = 32),
    PRIMARY KEY (candidate_id, root_position),
    UNIQUE (candidate_id, object_id),
    FOREIGN KEY (candidate_id) REFERENCES store_restore_candidates(candidate_id),
    FOREIGN KEY (object_id) REFERENCES store_objects(object_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE store_idempotency (
    namespace TEXT NOT NULL CHECK (length(namespace) BETWEEN 1 AND 128),
    key_digest BLOB NOT NULL CHECK (length(key_digest) = 32),
    meaning_digest BLOB NOT NULL CHECK (length(meaning_digest) = 32),
    result_object_id BLOB NOT NULL CHECK (length(result_object_id) = 32),
    generation_id BLOB NOT NULL CHECK (length(generation_id) = 32),
    head_id BLOB NOT NULL CHECK (length(head_id) = 32),
    PRIMARY KEY (namespace, key_digest),
    FOREIGN KEY (result_object_id) REFERENCES store_objects(object_id),
    FOREIGN KEY (generation_id) REFERENCES store_generations(generation_id),
    FOREIGN KEY (head_id) REFERENCES store_heads(head_id)
) STRICT, WITHOUT ROWID;
"#;

#[derive(Debug, Error)]
pub(crate) enum MetadataError {
    #[error("failed to inspect Store metadata path {path}")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Store metadata database path is a symbolic link: {0}")]
    SymbolicLink(PathBuf),
    #[error("Store metadata database path is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    #[error("Store metadata database or sidecar has an unsafe hard-link alias: {0}")]
    HardLinkedDatabase(PathBuf),
    #[error("Store metadata database path has no bounded leaf name: {0}")]
    InvalidDatabasePath(PathBuf),
    #[error("unable to allocate a bounded Store metadata initialization leaf for {0}")]
    InitializationNameExhausted(PathBuf),
    #[error("Store metadata database does not exist: {0}")]
    MissingDatabase(PathBuf),
    #[error("Store metadata database changed after its SQLite handle was bound: {0}")]
    DatabaseBindingChanged(PathBuf),
    #[error("Store metadata journal/WAL changed after its SQLite file handle was bound: {0}")]
    DatabaseSidecarBindingChanged(PathBuf),
    #[error("required SQLite file-control {operation} failed with result {result}")]
    UnsupportedFileControl {
        operation: &'static str,
        result: i32,
    },
    #[error("SQLite did not expose the required journal/WAL file-control method")]
    MissingSqliteFileControl,
    #[error("failed to install the Store metadata write authorizer: SQLite result {0}")]
    AuthorizerInstall(i32),
    #[error("nested Store metadata write authorization is forbidden")]
    NestedWriteAuthorization,
    #[error("failed to open Store metadata database {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("SQLite capability {capability} returned {actual}; expected {expected}")]
    UnsupportedCapability {
        capability: &'static str,
        expected: String,
        actual: String,
    },
    #[error("unsupported Store metadata schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { expected: i64, actual: i64 },
    #[error("Store metadata schema is missing {object_type} {name}")]
    MissingSchemaObject {
        object_type: &'static str,
        name: &'static str,
    },
    #[error("Store metadata schema is missing {object_type} {name}")]
    MissingOwnedSchemaObject {
        object_type: &'static str,
        name: String,
    },
    #[error("Store metadata schema definitions do not exactly match the owned v1 schema")]
    SchemaDefinitionMismatch,
    #[error("Store metadata database failed SQLite integrity_check")]
    DatabaseIntegrityCheckFailed,
    #[error("Store metadata database failed SQLite foreign_key_check")]
    ForeignKeyIntegrityCheckFailed,
    #[error("Store metadata singleton is missing")]
    MissingStoreMetadata,
    #[error("Store metadata role or domain identity does not match the requested Store")]
    StoreIdentityMismatch,
    #[error("Store publication expected-old or revision compare-and-swap did not match")]
    FacadeCasMismatch,
    #[error("Store idempotency key already binds a different request meaning")]
    IdempotencyMeaningConflict,
    #[error("Store authoritative metadata does not match its exact typed carrier")]
    FacadeIntegrityMismatch,
    #[error("Store snapshot restore failed: {0}")]
    SnapshotRestore(String),
    #[error("Store snapshot admission failed: {0}")]
    SnapshotAdmission(String),
    #[error("available Store Object capacity exceeds the frozen Stage 1 bound")]
    AvailableObjectCapacityExceeded,
    #[error("Store publication COMMIT failed; publication outcome is uncertain")]
    PublicationCommitUncertain {
        #[source]
        source: rusqlite::Error,
    },
    #[error("Store publication committed, but post-commit binding verification failed")]
    PublicationPostCommitBinding {
        #[source]
        source: Box<MetadataError>,
    },
    #[error(transparent)]
    CanonicalCbor(#[from] crate::foundation::core::deterministic_cbor::CborError),
    #[error(transparent)]
    SecureFs(#[from] SecureFsError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

impl MetadataError {
    pub(crate) const fn publication_may_have_committed(&self) -> bool {
        matches!(
            self,
            Self::PublicationCommitUncertain { .. } | Self::PublicationPostCommitBinding { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::domain::vnext::persistence::{StoreDomainV1, snapshot_rows::MAX_SNAPSHOT_ROWS_V1};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "maestro-vnext-metadata-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test metadata directory should be created");
            Self(
                path.canonicalize()
                    .expect("test metadata directory should canonicalize"),
            )
        }

        fn database(&self) -> PathBuf {
            self.0.join("metadata.sqlite")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn domain() -> (StoreRoleV1, StoreDomainIdV1) {
        let role = StoreRoleV1::Repository;
        let domain = StoreDomainV1::derive(role, b"metadata-test-domain")
            .expect("test domain should derive");
        (role, domain.id())
    }

    fn install_before_publication_commit_hook(hook: impl FnOnce() + 'static) {
        BEFORE_PUBLICATION_COMMIT_TEST_HOOK.with(|slot| {
            assert!(
                slot.borrow_mut().replace(Box::new(hook)).is_none(),
                "before-commit test hook must be exclusive"
            );
        });
    }

    fn install_after_publication_commit_hook(hook: impl FnOnce() + 'static) {
        AFTER_PUBLICATION_COMMIT_TEST_HOOK.with(|slot| {
            assert!(
                slot.borrow_mut().replace(Box::new(hook)).is_none(),
                "after-commit test hook must be exclusive"
            );
        });
    }

    fn substitute_database(database: &Path, replacement: &Path, detached: &Path) {
        fs::rename(database, detached).expect("canonical database should detach");
        fs::copy(replacement, database).expect("replacement database should publish");
    }

    fn insert_available_object(connection: &Connection, object_id: u8, stored_byte_length: u64) {
        connection
            .execute(
                "INSERT INTO store_objects
                 (object_id, schema_id, logical_byte_length, stored_byte_length,
                  stored_bytes_digest, storage_codec, key_envelope_id, key_envelope_kind)
                 VALUES (?1, ?2, 0, ?3, ?1, 'raw', NULL, NULL)",
                params![
                    vec![object_id; 32],
                    vec![0_u8; 32],
                    i64::try_from(stored_byte_length).expect("test byte length should fit SQLite")
                ],
            )
            .expect("hostile available Object row should insert");
    }

    #[test]
    fn open_binds_store_identity_and_verifies_capabilities() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&directory.database(), role, domain_id)
            .expect("metadata store should open");

        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .expect("foreign_keys pragma should read"),
            1
        );
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .expect("journal_mode pragma should read"),
            "wal"
        );
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
                .expect("synchronous pragma should read"),
            2
        );
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, i64>(0))
                .expect("busy_timeout pragma should read"),
            5_000
        );
    }

    #[test]
    fn every_open_rejects_a_different_store_identity() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        drop(
            MetadataStore::open_or_create(&directory.database(), role, domain_id)
                .expect("metadata store should be created"),
        );
        let other = StoreDomainV1::derive(role, b"different-domain")
            .expect("other test domain should derive");

        assert!(matches!(
            MetadataStore::open_existing(&directory.database(), role, other.id()),
            Err(MetadataError::StoreIdentityMismatch)
        ));
    }

    #[test]
    fn immutable_rows_and_singletons_enforce_monotonic_updates() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&directory.database(), role, domain_id)
            .expect("metadata store should open");

        store.with_test_authorized_connection(|connection| {
            assert!(
                connection
                    .execute("UPDATE store_metadata SET store_role = 2", [])
                    .is_err()
            );
            assert!(
                connection
                    .execute(
                    "INSERT OR REPLACE INTO store_metadata
                     (singleton, schema_version, store_role, domain_id)
                     SELECT singleton, schema_version, 2, domain_id FROM store_metadata",
                    [],
                )
                    .is_err()
            );
            assert!(
                connection
                    .execute(
                    "INSERT OR REPLACE INTO store_state(singleton, state, state_revision)
                     VALUES (1, 'inactive', 0)",
                    [],
                )
                    .is_err()
            );
            assert!(
                connection
                    .execute(
                    "UPDATE store_retention_revision SET retention_revision = 2 WHERE singleton = 1",
                    [],
                )
                    .is_err()
            );
            connection
                .execute(
                "UPDATE store_retention_revision SET retention_revision = 1 WHERE singleton = 1",
                [],
            )
                .expect("retention revision should advance by one");
            assert!(
                connection
                    .execute(
                    "UPDATE store_state SET state = 'active', state_revision = 1 WHERE singleton = 1",
                    [],
                )
                    .is_err()
            );
        });
    }

    #[test]
    fn raw_connection_refuses_mutation_outside_the_publication_facade() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&directory.database(), role, domain_id)
            .expect("metadata store should open");

        assert!(
            store
                .connection()
                .expect("database binding should remain current")
                .execute(
                    "UPDATE store_retention_revision SET retention_revision = 1 WHERE singleton = 1",
                    [],
                )
                .is_err()
        );
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row(
                    "SELECT retention_revision FROM store_retention_revision WHERE singleton = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("retention revision should read"),
            0
        );
    }

    #[test]
    fn immediate_transaction_rolls_back_closure_errors() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        let mut store = MetadataStore::open_or_create(&directory.database(), role, domain_id)
            .expect("metadata store should open");
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");

        let result: Result<(), MetadataError> =
            store.with_immediate_transaction(&root, |transaction| {
                transaction.execute(
                    "UPDATE store_retention_revision SET retention_revision = 1 WHERE singleton = 1",
                    [],
                )?;
                Err(MetadataError::StoreIdentityMismatch)
            });
        assert!(result.is_err());
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row(
                    "SELECT retention_revision FROM store_retention_revision WHERE singleton = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("retention revision should read"),
            0
        );
    }

    #[test]
    fn publication_transactions_tick_once_and_reject_a_stale_cut() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        let mut store = MetadataStore::open_or_create(&directory.database(), role, domain_id)
            .expect("metadata store should open");
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");

        store
            .with_publication_transaction(&root, Some(0), |_| Ok(()))
            .expect("first publication cut should commit");
        assert!(
            store
                .with_publication_transaction(&root, Some(0), |_| Ok(()))
                .is_err(),
            "a stale cut must fail before its closure can publish"
        );
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row(
                    "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("publication clock should read"),
            1
        );
    }

    #[test]
    fn pristine_inactive_restore_rebases_publication_continuity_once() {
        let directory = TestDir::new();
        let (role, domain_id) = domain();
        let mut store = MetadataStore::open_or_create(&directory.database(), role, domain_id)
            .expect("metadata store should open");
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");

        store
            .with_restore_transaction(&root, 48, |_| Ok(()))
            .expect("verified restore should resume after the source publication clock");
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row(
                    "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("publication clock should read"),
            48
        );
        assert!(
            store
                .with_restore_transaction(&root, 96, |_| Ok(()))
                .is_err(),
            "restore rebasing is exclusive to a pristine inactive Store"
        );
        store
            .with_publication_transaction(&root, Some(48), |_| Ok(()))
            .expect("ordinary publication should continue one tick after restore");
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row(
                    "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("publication clock should read"),
            49
        );
    }

    #[test]
    fn preexisting_empty_database_is_never_adopted() {
        let directory = TestDir::new();
        let database = directory.database();
        fs::write(&database, []).expect("empty foreign database should be created");
        let (role, domain_id) = domain();

        assert!(MetadataStore::open_or_create(&database, role, domain_id).is_err());
        assert_eq!(
            fs::read(&database).expect("foreign database should remain"),
            Vec::<u8>::new()
        );
        assert!(!PathBuf::from(format!("{}-wal", database.display())).exists());
        assert!(!PathBuf::from(format!("{}-shm", database.display())).exists());
    }

    #[test]
    fn crash_before_database_publication_leaves_only_inert_cleanup_debt() {
        let directory = TestDir::new();
        let database = directory.database();
        let temporary_name = ".maestro-store-init-crash.sqlite";
        let temporary = directory.0.join(temporary_name);
        let root = SecureRoot::open(&directory.0).expect("secure test root");
        root.create_file_if_absent(temporary_name, &[])
            .expect("temporary database leaf");
        let (role, domain_id) = domain();
        initialize_database_file(&temporary, role, domain_id)
            .expect("temporary database should initialize");

        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("retry should publish an independent initialized database");
        assert!(temporary.is_file());
        verify_store_identity(
            store
                .connection()
                .expect("database binding should remain current"),
            role,
            domain_id,
        )
        .expect("published Store identity");
    }

    #[test]
    fn crash_after_database_publication_before_wal_is_recoverable() {
        let directory = TestDir::new();
        let database = directory.database();
        let temporary_name = ".maestro-store-init-before-wal.sqlite";
        let temporary = directory.0.join(temporary_name);
        let root = SecureRoot::open(&directory.0).expect("secure test root");
        root.create_file_if_absent(temporary_name, &[])
            .expect("temporary database leaf");
        let (role, domain_id) = domain();
        initialize_database_file(&temporary, role, domain_id)
            .expect("temporary database should initialize");
        assert_eq!(
            root.rename_file_no_replace(temporary_name, "metadata.sqlite")
                .expect("publish initialized database"),
            CreateIfAbsent::Created
        );

        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("open should finish WAL configuration");
        assert_eq!(
            store
                .connection()
                .expect("database binding should remain current")
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .expect("journal mode"),
            "wal"
        );
    }

    #[test]
    fn exact_schema_validation_rejects_extra_owned_surface() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        store.with_test_authorized_connection(|connection| {
            connection
                .execute("CREATE TABLE hidden_authority(value INTEGER) STRICT", [])
                .expect("schema mutant should be installed");
        });
        drop(store);

        assert!(matches!(
            MetadataStore::open_existing(&database, role, domain_id),
            Err(MetadataError::SchemaDefinitionMismatch)
        ));
    }

    #[test]
    fn every_open_rejects_exact_schema_snapshot_bound_violations() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        store.with_test_authorized_connection(|connection| {
            let object_rows = MAX_SNAPSHOT_ROWS_V1 - 3;
            connection
                .execute(
                    "WITH RECURSIVE sequence(value) AS (
                         SELECT 0
                         UNION ALL
                         SELECT value + 1 FROM sequence WHERE value < ?1
                     )
                     INSERT INTO store_objects
                     (object_id, schema_id, logical_byte_length, stored_byte_length,
                      stored_bytes_digest, storage_codec, key_envelope_id, key_envelope_kind)
                     SELECT CAST(printf('%032d', value) AS BLOB), zeroblob(32), 0, 1,
                            CAST(printf('%032d', value) AS BLOB), 'raw', NULL, NULL
                     FROM sequence",
                    params![i64::try_from(object_rows - 1).expect("fixture row count should fit")],
                )
                .expect("exact-schema row-count mutant should be installed");
        });
        drop(store);

        for result in [
            MetadataStore::open_existing(&database, role, domain_id),
            MetadataStore::open_or_create(&database, role, domain_id),
        ] {
            assert!(matches!(result, Err(MetadataError::SnapshotAdmission(_))));
        }
    }

    #[test]
    fn every_open_rejects_exact_schema_available_object_bound_violations() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        store.with_test_authorized_connection(|connection| {
            insert_available_object(connection, 1, MAX_AVAILABLE_OBJECT_BYTES_V1 + 1);
        });
        drop(store);

        assert!(matches!(
            MetadataStore::open_existing(&database, role, domain_id),
            Err(MetadataError::AvailableObjectCapacityExceeded)
        ));
        assert!(matches!(
            MetadataStore::open_or_create(&database, role, domain_id),
            Err(MetadataError::AvailableObjectCapacityExceeded)
        ));
    }

    #[test]
    fn verified_read_checks_admission_bounds_before_running_the_logical_read() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        store.with_test_authorized_connection(|connection| {
            insert_available_object(connection, 1, MAX_AVAILABLE_OBJECT_BYTES_V1 + 1);
        });
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");
        let operation_ran = Cell::new(false);

        let result: Result<(), MetadataError> = store.with_verified_read(&root, |_| {
            operation_ran.set(true);
            Ok(())
        });

        assert!(matches!(
            result,
            Err(MetadataError::AvailableObjectCapacityExceeded)
        ));
        assert!(!operation_ran.get());
    }

    #[test]
    fn verified_read_uses_one_snapshot_for_admission_and_the_logical_read() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");

        let visible_objects: i64 = store
            .with_verified_read(&root, |read_connection| {
                let writer = Connection::open(&database)?;
                writer.busy_timeout(METADATA_BUSY_TIMEOUT)?;
                insert_available_object(&writer, 1, MAX_AVAILABLE_OBJECT_BYTES_V1 + 1);
                read_connection
                    .query_row("SELECT count(*) FROM store_objects", [], |row| row.get(0))
                    .map_err(MetadataError::from)
            })
            .expect("the admitted read snapshot should stay stable");

        assert_eq!(visible_objects, 0);
        let later_operation_ran = Cell::new(false);
        let later_result: Result<(), MetadataError> = store.with_verified_read(&root, |_| {
            later_operation_ran.set(true);
            Ok(())
        });
        assert!(matches!(
            later_result,
            Err(MetadataError::AvailableObjectCapacityExceeded)
        ));
        assert!(!later_operation_ran.get());
    }

    #[test]
    fn immediate_transaction_checks_admission_before_running_the_write_operation() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let mut store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        store.with_test_authorized_connection(|connection| {
            insert_available_object(connection, 1, MAX_AVAILABLE_OBJECT_BYTES_V1 + 1);
        });
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");
        let operation_ran = Cell::new(false);

        let result = store.with_immediate_transaction(&root, |_| {
            operation_ran.set(true);
            Ok(())
        });

        assert!(matches!(
            result,
            Err(MetadataError::AvailableObjectCapacityExceeded)
        ));
        assert!(!operation_ran.get());
    }

    #[test]
    fn precommit_database_substitution_rolls_back_and_is_not_commit_uncertain() {
        let directory = TestDir::new();
        let database = directory.database();
        let replacement_directory = TestDir::new();
        let replacement_database = replacement_directory.database();
        let detached = directory.0.join("detached.sqlite");
        let (role, domain_id) = domain();
        let mut store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("canonical metadata store should open");
        drop(
            MetadataStore::open_or_create(&replacement_database, role, domain_id)
                .expect("replacement metadata store should open"),
        );
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");
        install_before_publication_commit_hook({
            let database = database.clone();
            let replacement_database = replacement_database.clone();
            let detached = detached.clone();
            move || substitute_database(&database, &replacement_database, &detached)
        });

        let error = store
            .with_immediate_transaction(&root, |transaction| {
                transaction.execute(
                    "UPDATE store_retention_revision SET retention_revision = 1 WHERE singleton = 1",
                    [],
                )?;
                Ok(())
            })
            .expect_err("precommit substitution must reject publication");

        assert!(!error.publication_may_have_committed());
        store.with_test_authorized_connection(|connection| {
            assert_eq!(
                connection
                    .query_row(
                        "SELECT retention_revision FROM store_retention_revision WHERE singleton = 1",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .expect("detached transaction state should remain readable"),
                0
            );
            assert_eq!(
                connection
                    .query_row(
                        "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .expect("detached publication clock should remain readable"),
                0
            );
        });
    }

    #[test]
    fn postcommit_database_substitution_is_classified_as_maybe_committed() {
        let directory = TestDir::new();
        let database = directory.database();
        let replacement_directory = TestDir::new();
        let replacement_database = replacement_directory.database();
        let detached = directory.0.join("detached.sqlite");
        let (role, domain_id) = domain();
        let mut store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("canonical metadata store should open");
        drop(
            MetadataStore::open_or_create(&replacement_database, role, domain_id)
                .expect("replacement metadata store should open"),
        );
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");
        install_after_publication_commit_hook({
            let database = database.clone();
            let replacement_database = replacement_database.clone();
            let detached = detached.clone();
            move || substitute_database(&database, &replacement_database, &detached)
        });

        let error = store
            .with_immediate_transaction(&root, |_| Ok(()))
            .expect_err("postcommit substitution must reject successful handoff");

        assert!(matches!(
            &error,
            MetadataError::PublicationPostCommitBinding { .. }
        ));
        assert!(error.publication_may_have_committed());
        store.with_test_authorized_connection(|connection| {
            assert_eq!(
                connection
                    .query_row(
                        "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .expect("committed publication clock should remain readable"),
                1
            );
        });
        let commit_uncertain = MetadataError::PublicationCommitUncertain {
            source: rusqlite::Error::InvalidQuery,
        };
        assert!(commit_uncertain.publication_may_have_committed());
    }

    #[test]
    fn open_rejects_foreign_key_invalid_history_even_with_an_exact_schema() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("metadata store should be created");
        store.with_test_authorized_connection(|connection| {
            connection
                .pragma_update(None, "foreign_keys", false)
                .expect("disable foreign keys for corruption fixture");
            connection
                .execute(
                    "INSERT INTO store_object_references
                     (object_id, reference_position, referenced_object_id) VALUES (?1, 0, ?2)",
                    params![vec![1_u8; 32], vec![2_u8; 32]],
                )
                .expect("install same-schema foreign-key violation");
        });
        drop(store);

        assert!(matches!(
            MetadataStore::open_existing(&database, role, domain_id),
            Err(MetadataError::ForeignKeyIntegrityCheckFailed)
        ));
    }

    #[test]
    fn open_handle_rejects_same_path_database_leaf_substitution() {
        let directory = TestDir::new();
        let database = directory.database();
        let replacement_directory = TestDir::new();
        let replacement_database = replacement_directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("open canonical metadata Store");
        drop(
            MetadataStore::open_or_create(&replacement_database, role, domain_id)
                .expect("create a valid same-domain replacement database"),
        );
        let detached = directory.0.join("detached.sqlite");
        fs::rename(&database, &detached).expect("detach canonical leaf");
        fs::copy(&replacement_database, &database).expect("substitute valid database clone");

        assert!(matches!(
            store.connection(),
            Err(MetadataError::DatabaseBindingChanged(path)) if path == database
        ));
    }

    #[test]
    fn verified_read_rejects_database_substitution_after_a_successful_query() {
        let directory = TestDir::new();
        let database = directory.database();
        let replacement_directory = TestDir::new();
        let replacement_database = replacement_directory.database();
        let (role, domain_id) = domain();
        let store = MetadataStore::open_or_create(&database, role, domain_id)
            .expect("open canonical metadata Store");
        drop(
            MetadataStore::open_or_create(&replacement_database, role, domain_id)
                .expect("create valid same-domain replacement"),
        );
        let root = SecureRoot::open(&directory.0).expect("secure metadata root");
        let detached = directory.0.join("detached.sqlite");

        let result: Result<i64, MetadataError> = store.with_verified_read(&root, |connection| {
            let clock = connection.query_row(
                "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                [],
                |row| row.get(0),
            )?;
            fs::rename(&database, &detached).expect("detach canonical leaf mid-read");
            fs::copy(&replacement_database, &database).expect("substitute metadata mid-read");
            Ok(clock)
        });

        assert!(
            result.is_err(),
            "the successful stale query must be suppressed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_a_symbolic_link_database() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new();
        let real = directory.database();
        let link = directory.0.join("linked.sqlite");
        fs::write(&real, []).expect("target file should be created");
        symlink(&real, &link).expect("database symlink should be created");
        let (role, domain_id) = domain();

        assert!(matches!(
            MetadataStore::open_or_create(&link, role, domain_id),
            Err(MetadataError::SymbolicLink(path)) if path == link
        ));
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_a_hard_link_database_alias() {
        let directory = TestDir::new();
        let database = directory.database();
        let (role, domain_id) = domain();
        drop(
            MetadataStore::open_or_create(&database, role, domain_id)
                .expect("metadata store should be created"),
        );
        let alias = directory.0.join("metadata-alias.sqlite");
        fs::hard_link(&database, &alias).expect("database hard link should be created");

        assert!(matches!(
            MetadataStore::open_existing(&database, role, domain_id),
            Err(MetadataError::SecureFs(SecureFsError::UnsafeObject { .. }))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_symbolic_link_wal_and_shared_memory_sidecars() {
        use std::os::unix::fs::symlink;

        for suffix in ["-wal", "-shm"] {
            let directory = TestDir::new();
            let database = directory.database();
            let target = directory.0.join("sidecar-target");
            fs::write(&target, []).expect("sidecar target should be created");
            let mut sidecar = database.as_os_str().to_os_string();
            sidecar.push(suffix);
            let sidecar = PathBuf::from(sidecar);
            symlink(&target, &sidecar).expect("sidecar symlink should be created");
            let (role, domain_id) = domain();

            assert!(matches!(
                MetadataStore::open_or_create(&database, role, domain_id),
                Err(MetadataError::SymbolicLink(path)) if path == sidecar
            ));
        }
    }
}
