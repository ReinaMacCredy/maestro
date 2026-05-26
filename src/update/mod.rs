use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::core::fs::ensure_parent_dir;
use crate::core::hash::hex_digest;
use crate::core::paths::MaestroPaths;
use crate::core::schema::{
    BACKLOG_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION,
    INSTALL_LOCK_SCHEMA_VERSION,
};
use crate::skills::extract::{
    extract_bundled_skills, rollback_bundled_skill_writes, ExtractMode, SkillBackup,
};

static REPLACE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Options for one `maestro update` operation.
#[derive(Debug)]
pub struct UpdateOptions<'a> {
    /// Repository-local Maestro paths.
    pub paths: &'a MaestroPaths,
    /// Binary path that would be atomically replaced when a binary is available.
    pub executable_path: &'a Path,
    /// Backup timestamp shared by skill extraction for this update operation.
    pub backup_timestamp: &'a str,
}

/// Result of a complete update operation.
#[derive(Debug, Eq, PartialEq)]
pub struct UpdateOutcome {
    /// Binary replacement result.
    pub binary_status: BinaryStatus,
    /// Edited bundled skills backed up before overwrite.
    pub skill_backups: Vec<SkillBackup>,
    /// On-disk schema versions that differ from this binary.
    pub schema_mismatches: Vec<SchemaMismatch>,
}

/// Human-facing release metadata for update status output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseInfo {
    /// Release version string.
    pub version: String,
    /// Release timestamp, when known.
    pub released_at: Option<String>,
    /// Human-readable age, when known.
    pub relative_age: Option<String>,
    /// Download size in bytes, when known.
    pub size_bytes: Option<u64>,
}

/// Binary update result.
#[derive(Debug, Eq, PartialEq)]
pub enum BinaryStatus {
    /// The currently installed binary already matches the newest release.
    UpToDate { release: ReleaseInfo },
    /// No binary was available from the downloader seam.
    Skipped { reason: String },
    /// The executable path was atomically replaced.
    Replaced {
        path: PathBuf,
        release: Option<ReleaseInfo>,
    },
}

/// Download result from the binary update seam.
#[derive(Debug, Eq, PartialEq)]
pub enum DownloadedBinary {
    /// Downloaded binary candidate path.
    Available {
        path: PathBuf,
        release: Option<ReleaseInfo>,
    },
    /// The latest release already matches the current binary.
    UpToDate(ReleaseInfo),
    /// No binary work should be performed in this run.
    Unavailable(String),
}

/// One schema mismatch found on disk.
#[derive(Debug, Eq, PartialEq)]
pub struct SchemaMismatch {
    /// Artifact path.
    pub path: PathBuf,
    /// Schema version supported by this binary.
    pub expected: &'static str,
    /// Schema version read from disk.
    pub found: String,
}

/// Seam for fetching a binary candidate.
pub trait UpdateDownloader {
    /// Fetch a binary candidate into `work_dir`.
    fn download(&self, work_dir: &Path) -> Result<DownloadedBinary>;
}

/// Seam for verifying a downloaded binary candidate.
pub trait ChecksumVerifier {
    /// Verify a binary candidate before replacement.
    fn verify(&self, candidate: &Path) -> Result<()>;
}

/// Seam for replacing the active executable.
pub trait BinaryReplacer {
    /// Replace `current` with `candidate`.
    fn replace(&self, current: &Path, candidate: &Path) -> Result<()>;
}

/// Offline downloader used by V1 until real release downloads are wired in.
#[derive(Debug, Default)]
pub struct OfflineDownloader;

impl UpdateDownloader for OfflineDownloader {
    fn download(&self, _work_dir: &Path) -> Result<DownloadedBinary> {
        Ok(DownloadedBinary::Unavailable(
            "release download seam is not configured in this build".to_string(),
        ))
    }
}

/// Optional SHA-256 verifier for future release assets.
#[derive(Debug, Default)]
pub struct Sha256Verifier {
    expected: Option<String>,
}

impl Sha256Verifier {
    /// Create a verifier that skips checksum validation.
    pub fn disabled() -> Self {
        Self { expected: None }
    }

    /// Create a verifier for an expected lowercase hex SHA-256 digest.
    pub fn new(expected: impl Into<String>) -> Self {
        Self {
            expected: Some(expected.into()),
        }
    }
}

impl ChecksumVerifier for Sha256Verifier {
    fn verify(&self, candidate: &Path) -> Result<()> {
        let Some(expected) = &self.expected else {
            return Ok(());
        };

        let mut file = File::open(candidate)
            .with_context(|| format!("failed to open update candidate {}", candidate.display()))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let read = file.read(&mut buffer).with_context(|| {
                format!("failed to read update candidate {}", candidate.display())
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        let digest = hasher.finalize();
        let actual = hex_digest(&digest);
        if actual != *expected {
            anyhow::bail!(
                "checksum mismatch for {}: expected {}, found {}",
                candidate.display(),
                expected,
                actual
            );
        }

        Ok(())
    }
}

/// Replacer that writes a sibling temp file and renames it over the executable.
#[derive(Debug, Default)]
pub struct AtomicBinaryReplacer;

impl BinaryReplacer for AtomicBinaryReplacer {
    fn replace(&self, current: &Path, candidate: &Path) -> Result<()> {
        replace_binary_atomic(current, candidate)
    }
}

/// Run update with the default offline-safe seams.
pub fn run_update(options: &UpdateOptions<'_>) -> Result<UpdateOutcome> {
    run_update_with_seams(
        options,
        &OfflineDownloader,
        &Sha256Verifier::disabled(),
        &AtomicBinaryReplacer,
    )
}

/// Run update with explicit seams for tests and future release wiring.
pub fn run_update_with_seams(
    options: &UpdateOptions<'_>,
    downloader: &dyn UpdateDownloader,
    verifier: &dyn ChecksumVerifier,
    replacer: &dyn BinaryReplacer,
) -> Result<UpdateOutcome> {
    let schema_mismatches = detect_schema_mismatches(options.paths)?;
    let binary_candidate = prepare_binary_update(options, downloader, verifier)?;
    let extract_report = match extract_bundled_skills(
        options.paths,
        ExtractMode::Update {
            backup_timestamp: options.backup_timestamp,
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            cleanup_prepared_binary(&binary_candidate);
            return Err(error);
        }
    };
    let binary_status = match replace_prepared_binary(options, replacer, binary_candidate) {
        Ok(status) => status,
        Err(error) => {
            rollback_bundled_skill_writes(&extract_report)?;
            return Err(error);
        }
    };

    Ok(UpdateOutcome {
        binary_status,
        skill_backups: extract_report.backups,
        schema_mismatches,
    })
}

/// Detect known repo-local schema mismatches without mutating artifacts.
pub fn detect_schema_mismatches(paths: &MaestroPaths) -> Result<Vec<SchemaMismatch>> {
    let artifacts = [
        (
            paths.harness_dir().join("harness.yml"),
            HARNESS_SCHEMA_VERSION,
        ),
        (
            paths.features_dir().join("features.yaml"),
            FEATURE_SCHEMA_VERSION,
        ),
        (
            paths.harness_dir().join("backlog.yaml"),
            BACKLOG_SCHEMA_VERSION,
        ),
        (paths.install_lock_file(), INSTALL_LOCK_SCHEMA_VERSION),
    ];
    let mut mismatches = Vec::new();

    for (path, expected) in artifacts {
        if !path.exists() {
            continue;
        }

        let found = read_schema_version(&path)?;
        if found != expected {
            mismatches.push(SchemaMismatch {
                path,
                expected,
                found,
            });
        }
    }

    Ok(mismatches)
}

enum PreparedBinary {
    UpToDate {
        release: ReleaseInfo,
    },
    Skipped {
        reason: String,
    },
    Candidate {
        path: PathBuf,
        work_dir: PathBuf,
        release: Option<ReleaseInfo>,
    },
}

fn prepare_binary_update(
    options: &UpdateOptions<'_>,
    downloader: &dyn UpdateDownloader,
    verifier: &dyn ChecksumVerifier,
) -> Result<PreparedBinary> {
    let work_dir = options.paths.maestro_dir().join("update");
    let candidate = match downloader.download(&work_dir) {
        Ok(DownloadedBinary::Available { path, release }) => (path, release),
        Ok(DownloadedBinary::UpToDate(release)) => {
            cleanup_work_dir(&work_dir);
            return Ok(PreparedBinary::UpToDate { release });
        }
        Ok(DownloadedBinary::Unavailable(reason)) => {
            cleanup_work_dir(&work_dir);
            return Ok(PreparedBinary::Skipped { reason });
        }
        Err(error) => {
            cleanup_work_dir(&work_dir);
            return Err(error);
        }
    };

    if let Err(error) = verifier.verify(&candidate.0) {
        cleanup_work_dir(&work_dir);
        return Err(error);
    }

    Ok(PreparedBinary::Candidate {
        path: candidate.0,
        work_dir,
        release: candidate.1,
    })
}

fn replace_prepared_binary(
    options: &UpdateOptions<'_>,
    replacer: &dyn BinaryReplacer,
    binary: PreparedBinary,
) -> Result<BinaryStatus> {
    match binary {
        PreparedBinary::UpToDate { release } => Ok(BinaryStatus::UpToDate { release }),
        PreparedBinary::Skipped { reason } => Ok(BinaryStatus::Skipped { reason }),
        PreparedBinary::Candidate {
            path,
            work_dir,
            release,
        } => {
            let result =
                replacer
                    .replace(options.executable_path, &path)
                    .map(|()| BinaryStatus::Replaced {
                        path: options.executable_path.to_path_buf(),
                        release,
                    });
            cleanup_work_dir(&work_dir);
            result
        }
    }
}

fn cleanup_work_dir(work_dir: &Path) {
    let _ = fs::remove_dir_all(work_dir);
}

fn cleanup_prepared_binary(binary: &PreparedBinary) {
    if let PreparedBinary::Candidate { work_dir, .. } = binary {
        cleanup_work_dir(work_dir);
    }
}

fn read_schema_version(path: &Path) -> Result<String> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read schema artifact {}", path.display()))?;
    let value: serde_yaml::Value = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse schema artifact {}", path.display()))?;

    let found = value
        .get("schema_version")
        .and_then(serde_yaml::Value::as_str)
        .unwrap_or("<missing>");

    Ok(found.to_string())
}

fn replace_binary_atomic(current: &Path, candidate: &Path) -> Result<()> {
    ensure_parent_dir(current)?;

    let metadata = fs::metadata(candidate)
        .with_context(|| format!("failed to inspect update candidate {}", candidate.display()))?;
    let temp_path = temp_sibling_path(current)?;

    if let Err(error) = copy_candidate_to_temp(candidate, &temp_path, metadata.permissions()) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temp_path, current) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| {
            format!(
                "failed to replace {} with update candidate {}",
                current.display(),
                temp_path.display()
            )
        });
    }
    let _ = sync_parent_dir(current);

    Ok(())
}

fn copy_candidate_to_temp(
    candidate: &Path,
    temp_path: &Path,
    permissions: fs::Permissions,
) -> Result<()> {
    let mut source = File::open(candidate)
        .with_context(|| format!("failed to open update candidate {}", candidate.display()))?;
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .with_context(|| format!("failed to create temp update file {}", temp_path.display()))?;

    std::io::copy(&mut source, &mut temp)
        .and_then(|_| temp.flush())
        .and_then(|_| temp.sync_all())
        .with_context(|| {
            format!(
                "failed to copy update candidate {} to {}",
                candidate.display(),
                temp_path.display()
            )
        })?;
    fs::set_permissions(temp_path, permissions)
        .with_context(|| format!("failed to set permissions on {}", temp_path.display()))
}

fn temp_sibling_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("path has no valid file name: {}", path.display()))?;
    let parent = match path.parent() {
        Some(parent) => parent,
        None => Path::new(""),
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let counter = REPLACE_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(parent.join(format!(
        ".{file_name}.update.{}.{}.{}",
        process::id(),
        timestamp,
        counter
    )))
}

fn sync_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };

    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("failed to sync parent directory {}", parent.display()))
}
