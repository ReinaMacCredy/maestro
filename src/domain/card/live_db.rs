use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::card::archive_db;
use crate::domain::card::schema::Card;
use crate::domain::card::store::{CARD_FILE, DECISIONS_FILE, TASK_FILE, validate_card_id};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{CARD_SCHEMA_VERSION, Compat, classify};
use crate::foundation::core::time::utc_now_timestamp;

const STORE_DB_SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct DbCard {
    pub card: Card,
    pub path: PathBuf,
    pub raw: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredReceiptArtifact {
    pub artifact_type: String,
    pub id: String,
    pub card_id: Option<String>,
    pub created_at: String,
    pub payload_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredFile {
    path: String,
    mode: i64,
    bytes: Vec<u8>,
}

struct ImportedCard {
    card: Card,
    record_file: String,
    files: Vec<StoredFile>,
}

pub fn db_file(paths: &MaestroPaths) -> PathBuf {
    paths.store_db_file()
}

pub fn synthetic_card_path(paths: &MaestroPaths, id: &str, record_file: &str) -> PathBuf {
    db_file(paths).join("cards").join(id).join(record_file)
}

pub fn contains_card_id(paths: &MaestroPaths, id: &str) -> Result<bool> {
    validate_card_id(id)?;
    let Some(conn) = open_existing(paths)? else {
        return Ok(false);
    };
    card_exists(&conn, id)
}

pub fn resolve(paths: &MaestroPaths, id: &str) -> Result<Option<DbCard>> {
    validate_card_id(id)?;
    let Some(conn) = open_existing(paths)? else {
        return Ok(None);
    };
    let Some((card_yaml, record_file)) = conn
        .query_row(
            "SELECT card_yaml, record_file FROM cards WHERE id = ?1",
            params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
    else {
        return Ok(None);
    };
    let card = parse_card(&card_yaml, &synthetic_card_path(paths, id, &record_file))?;
    Ok(Some(DbCard {
        card,
        path: synthetic_card_path(paths, id, &record_file),
        raw: card_yaml,
    }))
}

pub fn scan(paths: &MaestroPaths) -> Result<Vec<(Card, PathBuf)>> {
    let Some(conn) = open_existing(paths)? else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare("SELECT id, card_yaml, record_file FROM cards ORDER BY id")?;
    let mut rows = stmt.query([])?;
    let mut cards = Vec::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let card_yaml: String = row.get(1)?;
        let record_file: String = row.get(2)?;
        let path = synthetic_card_path(paths, &id, &record_file);
        cards.push((parse_card(&card_yaml, &path)?, path));
    }
    Ok(cards)
}

pub fn insert_card(paths: &MaestroPaths, card: &Card, record_file: &str) -> Result<()> {
    validate_card_id(&card.id)?;
    validate_record_file(record_file)?;
    let conn = open_for_write(paths)?;
    if card_exists(&conn, &card.id)? {
        bail!("card {} already exists in the DB store", card.id);
    }
    let card_yaml = serde_yaml::to_string(card).context("failed to serialize DB card")?;
    insert_card_row(&conn, card, record_file, &card_yaml)?;
    upsert_file(&conn, &card.id, record_file, 0o644, card_yaml.as_bytes())?;
    Ok(())
}

pub fn save_card_if_unchanged(paths: &MaestroPaths, card: &Card, expected_raw: &str) -> Result<()> {
    validate_card_id(&card.id)?;
    let mut conn = open_for_write(paths)?;
    let record_file: String = conn
        .query_row(
            "SELECT record_file FROM cards WHERE id = ?1",
            params![card.id],
            |row| row.get(0),
        )
        .optional()?
        .with_context(|| format!("card {} not found in the DB store", card.id))?;
    let card_yaml = serde_yaml::to_string(card).context("failed to serialize DB card")?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE cards
            SET card_type = ?2,
                parent = ?3,
                status = ?4,
                title = ?5,
                card_yaml = ?6,
                updated_at = ?7
          WHERE id = ?1 AND card_yaml = ?8",
        params![
            card.id,
            card.card_type.as_str(),
            card.parent.as_deref(),
            card.status,
            card.title,
            card_yaml,
            card.updated_at,
            expected_raw
        ],
    )?;
    if changed == 0 {
        bail!(
            "card {} changed since it was read; re-run the command",
            card.id
        );
    }
    upsert_file(&tx, &card.id, &record_file, 0o644, card_yaml.as_bytes())?;
    tx.commit()?;
    Ok(())
}

pub fn remove_card_if_unchanged(
    paths: &MaestroPaths,
    card: &Card,
    expected_raw: &str,
) -> Result<()> {
    validate_card_id(&card.id)?;
    let mut conn = open_for_write(paths)?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "DELETE FROM cards WHERE id = ?1 AND card_yaml = ?2",
        params![card.id, expected_raw],
    )?;
    if changed == 0 {
        bail!(
            "card {} changed since it was read; re-run the command",
            card.id
        );
    }
    tx.execute(
        "DELETE FROM card_files WHERE card_id = ?1",
        params![card.id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn parent_is_db_container(paths: &MaestroPaths, parent: Option<&str>) -> Result<bool> {
    let Some(parent) = parent else {
        return Ok(false);
    };
    let Some(db_card) = resolve(paths, parent)? else {
        return Ok(false);
    };
    Ok(db_card.card.card_type.owns_task_container())
}

pub fn read_file(paths: &MaestroPaths, card_id: &str, relative: &str) -> Result<Option<Vec<u8>>> {
    validate_card_id(card_id)?;
    let Some(conn) = open_existing(paths)? else {
        return Ok(None);
    };
    let relative = normalize_relative(Path::new(relative))?;
    conn.query_row(
        "SELECT contents FROM card_files WHERE card_id = ?1 AND path = ?2",
        params![card_id, relative],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn read_text_file(
    paths: &MaestroPaths,
    card_id: &str,
    relative: &str,
) -> Result<Option<String>> {
    read_file(paths, card_id, relative)?
        .map(String::from_utf8)
        .transpose()
        .with_context(|| format!("DB sidecar {card_id}/{relative} is not UTF-8"))
}

pub fn write_text_file(
    paths: &MaestroPaths,
    card_id: &str,
    relative: &str,
    contents: &str,
) -> Result<()> {
    validate_card_id(card_id)?;
    let conn = open_for_write(paths)?;
    if !card_exists(&conn, card_id)? {
        bail!("card {card_id} not found in the DB store");
    }
    let relative = normalize_relative(Path::new(relative))?;
    upsert_file(&conn, card_id, &relative, 0o644, contents.as_bytes())
}

pub fn import_card_dir(
    paths: &MaestroPaths,
    card_id: &str,
    source_dir: &Path,
    remove_source: bool,
) -> Result<()> {
    validate_card_id(card_id)?;
    let files = collect_files(source_dir)?;
    let record_file = if files.iter().any(|file| file.path == CARD_FILE) {
        CARD_FILE
    } else if files.iter().any(|file| file.path == TASK_FILE) {
        TASK_FILE
    } else {
        bail!(
            "cannot import {} into DB store: no card.yaml or task.yaml found",
            source_dir.display()
        );
    };
    let card_bytes = files
        .iter()
        .find(|file| file.path == record_file)
        .expect("invariant: record file existence checked")
        .bytes
        .clone();
    let card_yaml = String::from_utf8(card_bytes)
        .with_context(|| format!("{} is not UTF-8", source_dir.join(record_file).display()))?;
    let card = parse_card(&card_yaml, &source_dir.join(record_file))?;
    if card.id != card_id {
        bail!(
            "cannot import {} as {card_id}: card id is {}",
            source_dir.display(),
            card.id
        );
    }
    let child_cards = imported_child_cards(source_dir, card_id, &files)?;

    let mut conn = open_for_write(paths)?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM card_files WHERE card_id = ?1",
        params![card_id],
    )?;
    tx.execute("DELETE FROM cards WHERE id = ?1", params![card_id])?;
    insert_card_row(&tx, &card, record_file, &card_yaml)?;
    for file in &files {
        upsert_file(&tx, card_id, &file.path, file.mode, &file.bytes)?;
    }
    for imported in child_cards {
        tx.execute(
            "DELETE FROM card_files WHERE card_id = ?1",
            params![imported.card.id],
        )?;
        tx.execute("DELETE FROM cards WHERE id = ?1", params![imported.card.id])?;
        let card_yaml =
            serde_yaml::to_string(&imported.card).context("failed to serialize DB child card")?;
        insert_card_row(&tx, &imported.card, &imported.record_file, &card_yaml)?;
        for file in &imported.files {
            upsert_file(&tx, &imported.card.id, &file.path, file.mode, &file.bytes)?;
        }
    }
    tx.commit()?;

    if remove_source {
        fs::remove_dir_all(source_dir).with_context(|| {
            format!(
                "failed to remove imported card dir {}",
                source_dir.display()
            )
        })?;
    }
    Ok(())
}

fn imported_child_cards(
    source_dir: &Path,
    root_card_id: &str,
    files: &[StoredFile],
) -> Result<Vec<ImportedCard>> {
    let mut imported = Vec::new();
    if let Some(file) = files.iter().find(|file| file.path == DECISIONS_FILE) {
        let text = String::from_utf8(file.bytes.clone())
            .with_context(|| format!("{} is not UTF-8", source_dir.join(&file.path).display()))?;
        let cards: Vec<Card> = serde_yaml::from_str(&text).with_context(|| {
            format!("failed to parse {}", source_dir.join(&file.path).display())
        })?;
        for card in cards {
            validate_imported_child_card(source_dir, root_card_id, &file.path, &card)?;
            let card_yaml =
                serde_yaml::to_string(&card).context("failed to serialize imported decision")?;
            imported.push(ImportedCard {
                card,
                record_file: CARD_FILE.to_string(),
                files: vec![StoredFile {
                    path: CARD_FILE.to_string(),
                    mode: default_file_mode(),
                    bytes: card_yaml.into_bytes(),
                }],
            });
        }
    }

    for file in files.iter().filter(|file| {
        Path::new(&file.path)
            .file_name()
            .is_some_and(|name| name == TASK_FILE)
            && Path::new(&file.path)
                .parent()
                .and_then(Path::parent)
                .is_some_and(|parent| parent == Path::new("tasks"))
    }) {
        let text = String::from_utf8(file.bytes.clone())
            .with_context(|| format!("{} is not UTF-8", source_dir.join(&file.path).display()))?;
        let card = parse_card(&text, &source_dir.join(&file.path))?;
        validate_imported_child_card(source_dir, root_card_id, &file.path, &card)?;
        let prefix = Path::new("tasks").join(&card.id);
        let child_files = files
            .iter()
            .filter_map(|stored| {
                let relative = Path::new(&stored.path).strip_prefix(&prefix).ok()?;
                Some(StoredFile {
                    path: relative.to_string_lossy().into_owned(),
                    mode: stored.mode,
                    bytes: stored.bytes.clone(),
                })
            })
            .collect::<Vec<_>>();
        imported.push(ImportedCard {
            card,
            record_file: TASK_FILE.to_string(),
            files: child_files,
        });
    }
    Ok(imported)
}

fn validate_imported_child_card(
    source_dir: &Path,
    root_card_id: &str,
    relative: &str,
    card: &Card,
) -> Result<()> {
    if card.id == root_card_id {
        bail!(
            "cannot import {}: child card in {relative} reuses root card id {root_card_id}",
            source_dir.display()
        );
    }
    validate_card_id(&card.id)?;
    if classify(&card.schema_version, CARD_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            source_dir.join(relative).display(),
            CARD_SCHEMA_VERSION,
            card.schema_version
        );
    }
    Ok(())
}

pub fn export_card_to_dir(paths: &MaestroPaths, card_id: &str, target_dir: &Path) -> Result<usize> {
    validate_card_id(card_id)?;
    let Some(conn) = open_existing(paths)? else {
        bail!("card {card_id} not found in the DB store");
    };
    if !card_exists(&conn, card_id)? {
        bail!("card {card_id} not found in the DB store");
    }
    if target_dir.exists() {
        bail!(
            "cannot export DB-backed card {card_id}: target already exists at {}",
            target_dir.display()
        );
    }
    ensure_dir(target_dir)?;
    let files = read_files(&conn, card_id)?;
    for file in &files {
        let target = target_dir.join(&file.path);
        if let Some(parent) = target.parent() {
            ensure_dir(parent)?;
        }
        fs::write(&target, &file.bytes)
            .with_context(|| format!("failed to write {}", target.display()))?;
    }
    Ok(files.len())
}

pub(crate) fn archive_card_snapshot(
    paths: &MaestroPaths,
    card_id: &str,
    source_relpath: &Path,
) -> Result<DbCard> {
    validate_card_id(card_id)?;
    let Some(conn) = open_existing(paths)? else {
        bail!("card {card_id} not found in the DB store");
    };
    let Some((card_yaml, record_file)) = conn
        .query_row(
            "SELECT card_yaml, record_file FROM cards WHERE id = ?1",
            params![card_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
    else {
        bail!("card {card_id} not found in the DB store");
    };
    let card = parse_card(
        &card_yaml,
        &synthetic_card_path(paths, card_id, &record_file),
    )?;
    let files = read_files(&conn, card_id)?
        .into_iter()
        .map(|file| {
            let mode = u32::try_from(file.mode)
                .with_context(|| format!("invalid DB file mode {} for {card_id}", file.mode))?;
            Ok(archive_db::SnapshotFile {
                path: file.path,
                mode,
                bytes: file.bytes,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    archive_db::archive_files(paths, card_id, source_relpath, files)?;
    Ok(DbCard {
        card,
        path: synthetic_card_path(paths, card_id, &record_file),
        raw: card_yaml,
    })
}

pub fn record_receipt_artifact(
    paths: &MaestroPaths,
    artifact_type: &str,
    artifact_id: &str,
    card_id: Option<&str>,
    payload_json: &str,
) -> Result<StoredReceiptArtifact> {
    validate_artifact_id(artifact_id)?;
    if let Some(card_id) = card_id {
        validate_card_id(card_id)?;
    }
    let conn = open_for_write(paths)?;
    let created_at = utc_now_timestamp();
    conn.execute(
        "INSERT OR REPLACE INTO receipt_artifacts
            (artifact_type, id, card_id, created_at, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            artifact_type,
            artifact_id,
            card_id,
            created_at,
            payload_json
        ],
    )?;
    Ok(StoredReceiptArtifact {
        artifact_type: artifact_type.to_string(),
        id: artifact_id.to_string(),
        card_id: card_id.map(str::to_string),
        created_at,
        payload_json: payload_json.to_string(),
    })
}

pub fn load_receipt_artifact(
    paths: &MaestroPaths,
    artifact_type: &str,
    artifact_id: &str,
) -> Result<Option<StoredReceiptArtifact>> {
    validate_artifact_id(artifact_id)?;
    let Some(conn) = open_existing(paths)? else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT artifact_type, id, card_id, created_at, payload_json
           FROM receipt_artifacts
          WHERE artifact_type = ?1 AND id = ?2",
        params![artifact_type, artifact_id],
        |row| {
            Ok(StoredReceiptArtifact {
                artifact_type: row.get(0)?,
                id: row.get(1)?,
                card_id: row.get(2)?,
                created_at: row.get(3)?,
                payload_json: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn open_for_write(paths: &MaestroPaths) -> Result<Connection> {
    ensure_dir(paths.maestro_dir())?;
    let conn = Connection::open(db_file(paths))
        .with_context(|| format!("failed to open {}", db_file(paths).display()))?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn open_existing(paths: &MaestroPaths) -> Result<Option<Connection>> {
    let file = db_file(paths);
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
        CREATE TABLE IF NOT EXISTS cards (
            id TEXT PRIMARY KEY NOT NULL,
            card_type TEXT NOT NULL,
            parent TEXT,
            status TEXT NOT NULL,
            title TEXT NOT NULL,
            record_file TEXT NOT NULL,
            card_yaml TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            imported_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS card_files (
            card_id TEXT NOT NULL,
            path TEXT NOT NULL,
            mode INTEGER NOT NULL,
            contents BLOB NOT NULL,
            sha256 TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (card_id, path),
            FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS receipt_artifacts (
            artifact_type TEXT NOT NULL,
            id TEXT NOT NULL,
            card_id TEXT,
            created_at TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            PRIMARY KEY (artifact_type, id)
        );",
    )?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO schema_version(version) VALUES (?1)",
            params![STORE_DB_SCHEMA_VERSION],
        )?;
    }
    ensure_supported_schema(conn)
}

fn ensure_supported_schema(conn: &Connection) -> Result<()> {
    let version = read_schema_version(conn)?;
    if version != STORE_DB_SCHEMA_VERSION {
        bail!("unsupported store DB schema version {version}; expected {STORE_DB_SCHEMA_VERSION}");
    }
    Ok(())
}

fn read_schema_version(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
        row.get(0)
    })
    .context("failed to read store DB schema version")
}

fn insert_card_row(
    conn: &Connection,
    card: &Card,
    record_file: &str,
    card_yaml: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO cards
            (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            card.id,
            card.card_type.as_str(),
            card.parent.as_deref(),
            card.status,
            card.title,
            record_file,
            card_yaml,
            card.created_at,
            card.updated_at,
            utc_now_timestamp()
        ],
    )
    .with_context(|| format!("failed to insert DB card {}", card.id))?;
    Ok(())
}

fn card_exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM cards WHERE id = ?1", params![id], |_| Ok(()))
        .optional()?
        .is_some())
}

fn parse_card(contents: &str, path: &Path) -> Result<Card> {
    let card: Card = serde_yaml::from_str(contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&card.schema_version, CARD_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            CARD_SCHEMA_VERSION,
            card.schema_version
        );
    }
    Ok(card)
}

fn collect_files(root: &Path) -> Result<Vec<StoredFile>> {
    if !root.is_dir() {
        bail!("card import root is not a directory: {}", root.display());
    }
    let mut files = BTreeMap::new();
    collect_files_inner(root, root, &mut files)?;
    Ok(files.into_values().collect())
}

fn collect_files_inner(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, StoredFile>,
) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read directory {}", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read directory entry under {}", dir.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!(
                "DB import refuses symlinked card artifact {}",
                path.display()
            );
        }
        if metadata.is_dir() {
            collect_files_inner(root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let relative = normalize_relative(
            path.strip_prefix(root)
                .with_context(|| format!("failed to relativize {}", path.display()))?,
        )?;
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        files.insert(
            relative.clone(),
            StoredFile {
                path: relative,
                mode: default_file_mode(),
                bytes,
            },
        );
    }
    Ok(())
}

fn read_files(conn: &Connection, card_id: &str) -> Result<Vec<StoredFile>> {
    let mut stmt = conn
        .prepare("SELECT path, mode, contents FROM card_files WHERE card_id = ?1 ORDER BY path")?;
    let rows = stmt.query_map(params![card_id], |row| {
        Ok(StoredFile {
            path: row.get(0)?,
            mode: row.get(1)?,
            bytes: row.get(2)?,
        })
    })?;
    let mut files = Vec::new();
    for row in rows {
        files.push(row?);
    }
    Ok(files)
}

fn upsert_file(
    conn: &Connection,
    card_id: &str,
    relative: &str,
    mode: i64,
    bytes: &[u8],
) -> Result<()> {
    let now = utc_now_timestamp();
    let sha = sha256_hex(bytes);
    conn.execute(
        "INSERT INTO card_files (card_id, path, mode, contents, sha256, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(card_id, path) DO UPDATE SET
            mode = excluded.mode,
            contents = excluded.contents,
            sha256 = excluded.sha256,
            updated_at = excluded.updated_at",
        params![card_id, relative, mode, bytes, sha, now],
    )?;
    Ok(())
}

fn normalize_relative(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .with_context(|| format!("path component is not UTF-8: {}", path.display()))?;
                parts.push(part.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe relative path: {}", path.display())
            }
        }
    }
    if parts.is_empty() {
        bail!("relative path must not be empty");
    }
    Ok(parts.join("/"))
}

fn validate_record_file(record_file: &str) -> Result<()> {
    match record_file {
        CARD_FILE | TASK_FILE => Ok(()),
        _ => bail!("unsupported DB card record file: {record_file}"),
    }
}

fn validate_artifact_id(id: &str) -> Result<()> {
    validate_card_id(id)
}

#[cfg(unix)]
fn default_file_mode() -> i64 {
    0o644
}

#[cfg(not(unix))]
fn default_file_mode() -> i64 {
    0
}
