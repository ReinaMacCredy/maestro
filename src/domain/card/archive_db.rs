use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::domain::card::schema::Card;
use crate::domain::card::store::{
    CARD_FILE, DECISIONS_FILE, IDEAS_FILE, TASK_FILE, validate_card_id,
};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::CARD_SCHEMA_VERSION;
use crate::foundation::core::time::utc_now_timestamp;

const ARCHIVE_DB_SCHEMA_VERSION: i64 = 1;
const SNAPSHOT_FORMAT_VERSION: &str = "maestro.archive.snapshot.v1";
const SNAPSHOT_MAGIC: &[u8] = b"MAESTRO_ARCHIVE_SNAPSHOT_V1\n";

#[derive(Clone, Debug)]
pub struct ArchivedCard {
    pub card: Card,
    pub path: PathBuf,
    pub snapshot_id: String,
}

#[derive(Clone, Debug)]
pub struct ArchiveDoctorReport {
    pub schema_version: i64,
    pub snapshots: usize,
    pub cards: usize,
    pub quarantine_dirs: usize,
}

#[derive(Clone, Debug)]
pub struct MigrationPlan {
    pub folder_archives: usize,
    pub importable_snapshots: usize,
    pub quarantine_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct MigrationReport {
    pub imported_snapshots: usize,
    pub quarantined_folders: usize,
    pub quarantine_dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ArchiveManifest {
    format_version: String,
    source_relpath: String,
    created_at: String,
    card_schema_version: String,
    files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestFile {
    path: String,
    mode: u32,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug)]
struct SnapshotFile {
    path: String,
    mode: u32,
    bytes: Vec<u8>,
}

struct SnapshotRow {
    id: String,
    source_relpath: String,
    manifest: ArchiveManifest,
    blob: Vec<u8>,
}

pub fn archive_db_file(paths: &MaestroPaths) -> PathBuf {
    paths.archive_dir().join("cards.sqlite")
}

pub fn legacy_quarantine_dir(paths: &MaestroPaths, stamp: &str) -> PathBuf {
    paths.archive_dir().join(format!("legacy-cards-{stamp}"))
}

pub fn contains_card_id(paths: &MaestroPaths, id: &str) -> Result<bool> {
    Ok(resolve(paths, id)?.is_some())
}

pub fn resolve(paths: &MaestroPaths, id: &str) -> Result<Option<ArchivedCard>> {
    validate_card_id(id)?;
    Ok(scan(paths)?
        .into_iter()
        .find(|archived| archived.card.id == id))
}

pub fn read_file(
    paths: &MaestroPaths,
    snapshot_id: &str,
    relative: &str,
) -> Result<Option<Vec<u8>>> {
    validate_card_id(snapshot_id)?;
    let Some(conn) = open_existing(paths)? else {
        return Ok(None);
    };
    let mut rows = read_rows(&conn, Some(snapshot_id))?;
    let Some(row) = rows.pop() else {
        return Ok(None);
    };
    let wanted = normalize_relative(Path::new(relative))?;
    for file in unpack_and_verify(&row)? {
        if file.path == wanted {
            return Ok(Some(file.bytes));
        }
    }
    Ok(None)
}

pub fn scan(paths: &MaestroPaths) -> Result<Vec<ArchivedCard>> {
    let Some(conn) = open_existing(paths)? else {
        return Ok(Vec::new());
    };
    let mut rows = read_rows(&conn, None)?;
    let mut cards = Vec::new();
    for row in rows.drain(..) {
        cards.extend(cards_from_row(paths, &row)?);
    }
    cards.sort_by(|a, b| a.card.id.cmp(&b.card.id));
    Ok(cards)
}

pub fn archive_directory(
    paths: &MaestroPaths,
    snapshot_id: &str,
    source_dir: &Path,
    source_relpath: &Path,
) -> Result<()> {
    validate_card_id(snapshot_id)?;
    let source_relpath = normalize_relative(source_relpath)?;
    let files = collect_snapshot_files(source_dir)?;
    if files.is_empty() {
        bail!(
            "cannot archive empty card directory {}",
            source_dir.display()
        );
    }
    archive_snapshot(paths, snapshot_id, &source_relpath, files)
}

pub fn archive_virtual_card(
    paths: &MaestroPaths,
    snapshot_id: &str,
    card: &Card,
    source_relpath: &Path,
) -> Result<()> {
    validate_card_id(snapshot_id)?;
    let source_relpath = normalize_relative(source_relpath)?;
    let contents = serde_yaml::to_string(card).context("failed to serialize archived card")?;
    let files = vec![SnapshotFile {
        path: CARD_FILE.to_string(),
        mode: default_file_mode(),
        bytes: contents.into_bytes(),
    }];
    archive_snapshot(paths, snapshot_id, &source_relpath, files)
}

pub fn restore_snapshots(paths: &MaestroPaths, snapshot_ids: &[String]) -> Result<usize> {
    let mut unique = BTreeSet::new();
    for id in snapshot_ids {
        validate_card_id(id)?;
        unique.insert(id.clone());
    }
    if unique.is_empty() {
        return Ok(0);
    }

    let conn = open_for_write(paths)?;
    let rows = read_rows_for_ids(&conn, unique.iter().map(String::as_str))?;
    if rows.len() != unique.len() {
        let found: BTreeSet<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        let missing = unique
            .iter()
            .find(|id| !found.contains(id.as_str()))
            .expect("invariant: len mismatch names a missing id");
        bail!("archived card not found: {missing}");
    }

    let mut restore_files = Vec::new();
    for row in &rows {
        for archived in cards_from_row(paths, row)? {
            if crate::domain::card::store::resolve(paths, &archived.card.id)?.is_some() {
                bail!(
                    "cannot unarchive {} — a live copy of {} already exists",
                    row.id,
                    archived.card.id
                );
            }
        }
        let files = unpack_and_verify(row)?;
        let root = paths.cards_dir().join(&row.source_relpath);
        if root.exists() {
            bail!(
                "cannot unarchive {} — live artifact already exists at {}",
                row.id,
                root.display()
            );
        }
        for file in files {
            let target = root.join(&file.path);
            if target.exists() {
                bail!(
                    "cannot unarchive {} — live artifact already exists at {}",
                    row.id,
                    target.display()
                );
            }
            restore_files.push((target, file));
        }
    }

    for (target, file) in restore_files {
        if let Some(parent) = target.parent() {
            ensure_dir(parent)?;
        }
        fs::write(&target, &file.bytes)
            .with_context(|| format!("failed to write {}", target.display()))?;
        set_file_mode(&target, file.mode)?;
    }
    for id in unique {
        conn.execute("DELETE FROM archived_snapshots WHERE id = ?1", params![id])?;
    }
    Ok(rows.len())
}

pub fn migration_plan(paths: &MaestroPaths) -> Result<MigrationPlan> {
    let archive_cards = paths.archive_cards_dir();
    let folder_archives = legacy_archive_dirs(&archive_cards)?.len();
    Ok(MigrationPlan {
        folder_archives,
        importable_snapshots: folder_archives,
        quarantine_dir: legacy_quarantine_dir(paths, &utc_now_timestamp()[..10]),
    })
}

pub fn migrate_legacy_folders(paths: &MaestroPaths) -> Result<MigrationReport> {
    let archive_cards = paths.archive_cards_dir();
    let dirs = legacy_archive_dirs(&archive_cards)?;
    if dirs.is_empty() {
        return Ok(MigrationReport {
            imported_snapshots: 0,
            quarantined_folders: 0,
            quarantine_dir: legacy_quarantine_dir(paths, &utc_now_timestamp()[..10]),
        });
    }

    let quarantine = unique_quarantine_dir(paths)?;
    let mut imported = 0;
    for dir in &dirs {
        let id = dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("archive directory has non-utf8 name")?;
        let source_relpath = dir.strip_prefix(&archive_cards).with_context(|| {
            format!(
                "failed to make legacy archive {} relative to {}",
                dir.display(),
                archive_cards.display()
            )
        })?;
        archive_directory(paths, id, dir, source_relpath)?;
        imported += 1;
    }

    ensure_dir(&quarantine)?;
    for dir in &dirs {
        let relative = dir.strip_prefix(&archive_cards).with_context(|| {
            format!(
                "failed to make legacy archive {} relative to {}",
                dir.display(),
                archive_cards.display()
            )
        })?;
        let target = quarantine.join(relative);
        if let Some(parent) = target.parent() {
            ensure_dir(parent)?;
        }
        fs::rename(dir, &target).with_context(|| {
            format!(
                "failed to move legacy archive {} to {}",
                dir.display(),
                target.display()
            )
        })?;
    }

    Ok(MigrationReport {
        imported_snapshots: imported,
        quarantined_folders: dirs.len(),
        quarantine_dir: quarantine,
    })
}

pub fn doctor(paths: &MaestroPaths) -> Result<ArchiveDoctorReport> {
    let Some(conn) = open_existing(paths)? else {
        return Ok(ArchiveDoctorReport {
            schema_version: ARCHIVE_DB_SCHEMA_VERSION,
            snapshots: 0,
            cards: 0,
            quarantine_dirs: legacy_quarantine_dirs(paths)?,
        });
    };
    let schema_version = read_schema_version(&conn)?;
    let rows = read_rows(&conn, None)?;
    let mut cards = 0;
    for row in &rows {
        cards += cards_from_row(paths, row)?.len();
    }
    Ok(ArchiveDoctorReport {
        schema_version,
        snapshots: rows.len(),
        cards,
        quarantine_dirs: legacy_quarantine_dirs(paths)?,
    })
}

pub fn cleanup_legacy_quarantine(paths: &MaestroPaths) -> Result<usize> {
    let dirs = legacy_quarantine_paths(paths)?;
    let _ = doctor(paths)?;
    for dir in &dirs {
        fs::remove_dir_all(dir).with_context(|| format!("failed to remove {}", dir.display()))?;
    }
    Ok(dirs.len())
}

fn archive_snapshot(
    paths: &MaestroPaths,
    snapshot_id: &str,
    source_relpath: &str,
    files: Vec<SnapshotFile>,
) -> Result<()> {
    let conn = open_for_write(paths)?;
    if snapshot_exists(&conn, snapshot_id)? {
        bail!("archived card {snapshot_id} already exists in the archive DB");
    }
    let created_at = utc_now_timestamp();
    let manifest = manifest(source_relpath, &created_at, &files);
    let manifest_json =
        serde_json::to_string(&manifest).context("failed to serialize archive manifest")?;
    let packed = pack_snapshot(&files)?;
    let compressed = zstd::stream::encode_all(Cursor::new(packed), 0)
        .context("failed to compress archive snapshot")?;
    let snapshot_sha256 = sha256_hex(&compressed);
    let search_text = search_text(&files);
    conn.execute(
        "INSERT INTO archived_snapshots
            (id, archived_at, source_relpath, manifest_json, snapshot_zstd, snapshot_sha256, search_text, last_checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            snapshot_id,
            created_at,
            source_relpath,
            manifest_json,
            compressed,
            snapshot_sha256,
            search_text
        ],
    )
    .with_context(|| format!("failed to insert archived snapshot {snapshot_id}"))?;
    Ok(())
}

fn open_for_write(paths: &MaestroPaths) -> Result<Connection> {
    ensure_dir(paths.archive_dir())?;
    let conn = Connection::open(archive_db_file(paths))
        .with_context(|| format!("failed to open {}", archive_db_file(paths).display()))?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn open_existing(paths: &MaestroPaths) -> Result<Option<Connection>> {
    let file = archive_db_file(paths);
    if !file.exists() {
        return Ok(None);
    }
    let conn =
        Connection::open(&file).with_context(|| format!("failed to open {}", file.display()))?;
    ensure_supported_schema(&conn)?;
    Ok(Some(conn))
}

fn initialize_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS archived_snapshots (
            id TEXT PRIMARY KEY NOT NULL,
            archived_at TEXT NOT NULL,
            source_relpath TEXT NOT NULL,
            manifest_json TEXT NOT NULL,
            snapshot_zstd BLOB NOT NULL,
            snapshot_sha256 TEXT NOT NULL,
            search_text TEXT NOT NULL,
            last_checked_at TEXT
        );",
    )?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO schema_version(version) VALUES (?1)",
            params![ARCHIVE_DB_SCHEMA_VERSION],
        )?;
    }
    ensure_supported_schema(conn)
}

fn ensure_supported_schema(conn: &Connection) -> Result<()> {
    let version = read_schema_version(conn)?;
    if version != ARCHIVE_DB_SCHEMA_VERSION {
        bail!(
            "unsupported archive DB schema version {version}; expected {ARCHIVE_DB_SCHEMA_VERSION}"
        );
    }
    Ok(())
}

fn read_schema_version(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
        row.get(0)
    })
    .context("archive DB missing schema_version")
}

fn snapshot_exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM archived_snapshots WHERE id = ?1",
            params![id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn read_rows(conn: &Connection, where_id: Option<&str>) -> Result<Vec<SnapshotRow>> {
    match where_id {
        Some(id) => {
            let mut stmt = conn.prepare(
                "SELECT id, source_relpath, manifest_json, snapshot_zstd
                 FROM archived_snapshots WHERE id = ?1 ORDER BY id",
            )?;
            rows_from_stmt(stmt.query(params![id])?)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, source_relpath, manifest_json, snapshot_zstd
                 FROM archived_snapshots ORDER BY id",
            )?;
            rows_from_stmt(stmt.query([])?)
        }
    }
}

fn read_rows_for_ids<'a>(
    conn: &Connection,
    ids: impl Iterator<Item = &'a str>,
) -> Result<Vec<SnapshotRow>> {
    let mut rows = Vec::new();
    for id in ids {
        rows.extend(read_rows(conn, Some(id))?);
    }
    Ok(rows)
}

fn rows_from_stmt(mut rows: rusqlite::Rows<'_>) -> Result<Vec<SnapshotRow>> {
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let manifest_json: String = row.get(2)?;
        let manifest: ArchiveManifest =
            serde_json::from_str(&manifest_json).context("failed to parse archive manifest")?;
        if manifest.format_version != SNAPSHOT_FORMAT_VERSION {
            bail!(
                "unsupported archive snapshot format {}; expected {SNAPSHOT_FORMAT_VERSION}",
                manifest.format_version
            );
        }
        out.push(SnapshotRow {
            id: row.get(0)?,
            source_relpath: row.get(1)?,
            manifest,
            blob: row.get(3)?,
        });
    }
    Ok(out)
}

fn collect_snapshot_files(source_dir: &Path) -> Result<Vec<SnapshotFile>> {
    let mut files = Vec::new();
    collect_snapshot_files_inner(source_dir, source_dir, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_snapshot_files_inner(
    root: &Path,
    current: &Path,
    files: &mut Vec<SnapshotFile>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read directory {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("cannot archive symlinked artifact {}", path.display());
        }
        if metadata.is_dir() {
            collect_snapshot_files_inner(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .with_context(|| format!("failed to relativize {}", path.display()))?;
            let relative = normalize_relative(relative)?;
            let bytes =
                fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
            files.push(SnapshotFile {
                path: relative,
                mode: file_mode(&metadata),
                bytes,
            });
        }
    }
    Ok(())
}

fn manifest(source_relpath: &str, created_at: &str, files: &[SnapshotFile]) -> ArchiveManifest {
    ArchiveManifest {
        format_version: SNAPSHOT_FORMAT_VERSION.to_string(),
        source_relpath: source_relpath.to_string(),
        created_at: created_at.to_string(),
        card_schema_version: CARD_SCHEMA_VERSION.to_string(),
        files: files
            .iter()
            .map(|file| ManifestFile {
                path: file.path.clone(),
                mode: file.mode,
                size: file.bytes.len() as u64,
                sha256: sha256_hex(&file.bytes),
            })
            .collect(),
    }
}

fn pack_snapshot(files: &[SnapshotFile]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(SNAPSHOT_MAGIC);
    write_u32(&mut out, files.len() as u32);
    for file in files {
        let path = file.path.as_bytes();
        write_u32(&mut out, path.len() as u32);
        write_u64(&mut out, file.bytes.len() as u64);
        out.extend_from_slice(path);
        out.extend_from_slice(&file.bytes);
    }
    Ok(out)
}

fn unpack_and_verify(row: &SnapshotRow) -> Result<Vec<SnapshotFile>> {
    let decompressed = zstd::stream::decode_all(Cursor::new(&row.blob))
        .with_context(|| format!("failed to decompress archived snapshot {}", row.id))?;
    let mut cursor = Cursor::new(decompressed);
    let mut magic = vec![0_u8; SNAPSHOT_MAGIC.len()];
    cursor.read_exact(&mut magic)?;
    if magic != SNAPSHOT_MAGIC {
        bail!("archive snapshot {} has invalid magic", row.id);
    }
    let count = read_u32(&mut cursor)? as usize;
    if count != row.manifest.files.len() {
        bail!(
            "archive snapshot {} manifest file count mismatch: manifest {}, blob {count}",
            row.id,
            row.manifest.files.len()
        );
    }
    let mut files = Vec::with_capacity(count);
    let manifest_by_path: BTreeMap<&str, &ManifestFile> = row
        .manifest
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    for _ in 0..count {
        let path_len = read_u32(&mut cursor)? as usize;
        let content_len = read_u64(&mut cursor)? as usize;
        let mut path_bytes = vec![0_u8; path_len];
        cursor.read_exact(&mut path_bytes)?;
        let path = String::from_utf8(path_bytes).context("archive snapshot path is not utf8")?;
        let mut bytes = vec![0_u8; content_len];
        cursor.read_exact(&mut bytes)?;
        let Some(manifest) = manifest_by_path.get(path.as_str()) else {
            bail!(
                "archive snapshot {} contains unmanifested file {path}",
                row.id
            );
        };
        let actual_hash = sha256_hex(&bytes);
        if manifest.sha256 != actual_hash {
            bail!(
                "archive snapshot {} file {path} hash mismatch: expected {}, found {actual_hash}",
                row.id,
                manifest.sha256
            );
        }
        if manifest.size != bytes.len() as u64 {
            bail!(
                "archive snapshot {} file {path} size mismatch: expected {}, found {}",
                row.id,
                manifest.size,
                bytes.len()
            );
        }
        files.push(SnapshotFile {
            path,
            mode: manifest.mode,
            bytes,
        });
    }
    Ok(files)
}

fn cards_from_row(paths: &MaestroPaths, row: &SnapshotRow) -> Result<Vec<ArchivedCard>> {
    let files = unpack_and_verify(row)?;
    let mut cards = Vec::new();
    for file in &files {
        let path = Path::new(&file.path);
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        match name {
            CARD_FILE | TASK_FILE => {
                let card: Card = serde_yaml::from_slice(&file.bytes)
                    .with_context(|| format!("failed to parse archived {}", file.path))?;
                cards.push(ArchivedCard {
                    card,
                    path: synthetic_path(paths, &row.id, &file.path),
                    snapshot_id: row.id.clone(),
                });
            }
            DECISIONS_FILE | IDEAS_FILE => {
                let entries: Vec<Card> = serde_yaml::from_slice(&file.bytes)
                    .with_context(|| format!("failed to parse archived {}", file.path))?;
                for card in entries {
                    cards.push(ArchivedCard {
                        card,
                        path: synthetic_path(paths, &row.id, &file.path),
                        snapshot_id: row.id.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    cards.sort_by(|a, b| a.card.id.cmp(&b.card.id));
    Ok(cards)
}

fn synthetic_path(paths: &MaestroPaths, snapshot_id: &str, relative: &str) -> PathBuf {
    archive_db_file(paths).join(snapshot_id).join(relative)
}

fn search_text(files: &[SnapshotFile]) -> String {
    let mut text = String::new();
    for file in files {
        if let Ok(value) = std::str::from_utf8(&file.bytes) {
            text.push_str(value);
            text.push('\n');
        }
    }
    text
}

fn normalize_relative(path: &Path) -> Result<String> {
    if path.is_absolute() {
        bail!("archive snapshot path must be relative: {}", path.display());
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe archive snapshot path: {}", path.display());
            }
        }
    }
    if parts.is_empty() {
        bail!("archive snapshot path cannot be empty");
    }
    Ok(parts.join("/"))
}

fn legacy_archive_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    legacy_archive_dirs_inner(root, &mut dirs)?;
    dirs.sort();
    Ok(dirs)
}

fn legacy_archive_dirs_inner(current: &Path, dirs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        if path.join(CARD_FILE).is_file() || path.join(TASK_FILE).is_file() {
            dirs.push(path);
        } else {
            legacy_archive_dirs_inner(&path, dirs)?;
        }
    }
    Ok(())
}

fn legacy_quarantine_dirs(paths: &MaestroPaths) -> Result<usize> {
    Ok(legacy_quarantine_paths(paths)?.len())
}

fn legacy_quarantine_paths(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    let archive_dir = paths.archive_dir();
    if !archive_dir.exists() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    for entry in fs::read_dir(&archive_dir)
        .with_context(|| format!("failed to read {}", archive_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("legacy-cards-"))
            && path.is_dir()
        {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn unique_quarantine_dir(paths: &MaestroPaths) -> Result<PathBuf> {
    let base = utc_now_timestamp()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    for suffix in 0..1000 {
        let stamp = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        let candidate = legacy_quarantine_dir(paths, &stamp);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("failed to choose a unique legacy archive quarantine directory")
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u32(cursor: &mut Cursor<Vec<u8>>) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(cursor: &mut Cursor<Vec<u8>>) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    cursor.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    metadata.mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    default_file_mode()
}

fn default_file_mode() -> u32 {
    0o644
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
