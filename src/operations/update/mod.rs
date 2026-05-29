mod github_release;
mod replace;

use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::domain::skills::extract::{
    extract_bundled_skills, rollback_bundled_skill_writes, ExtractMode, SkillBackup,
};
use crate::foundation::core::hash::hex_digest;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{
    classify, Compat, BACKLOG_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION,
    INSTALL_LOCK_SCHEMA_VERSION,
};

pub use github_release::GitHubCurlDownloader;
pub use replace::AtomicBinaryReplacer;

use replace::{
    cleanup_prepared_binary, prepare_binary_update, prepared_release, replace_prepared_binary,
};

/// Options for one `maestro update` operation.
#[derive(Debug)]
pub struct UpdateOptions<'a> {
    /// Repository-local Maestro paths.
    pub paths: &'a MaestroPaths,
    /// Binary path that would be atomically replaced when a binary is available.
    pub executable_path: &'a Path,
    /// Backup timestamp shared by skill extraction for this update operation.
    pub backup_timestamp: &'a str,
    /// Current binary version string.
    pub current_version: &'a str,
    /// Only check release status; do not download, replace, or refresh artifacts.
    pub check_only: bool,
    /// Reinstall even when the remote reports this version as current.
    pub force: bool,
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
    /// A newer release is available, but this run did not install it.
    UpdateAvailable {
        release: ReleaseInfo,
        current_version: String,
    },
    /// The currently installed binary already matches the newest release.
    UpToDate { release: ReleaseInfo },
    /// No binary was available from the downloader seam.
    Skipped { reason: UpdateUnavailable },
    /// The executable path was atomically replaced.
    Replaced {
        path: PathBuf,
        release: Option<ReleaseInfo>,
    },
}

/// Reason a binary update is unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateUnavailable {
    /// This binary is not managed by Maestro's self-updater.
    LocalDevelopment,
    /// This binary should be upgraded through Homebrew.
    Homebrew,
    /// This binary should be upgraded through Cargo.
    Cargo,
    /// The latest release has no asset matching this platform.
    NoPlatformAsset { hint: String },
}

impl fmt::Display for UpdateUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalDevelopment => {
                formatter.write_str("for this build: running from a local development binary")
            }
            Self::Homebrew => formatter
                .write_str("for this install: installed with Homebrew. Run `brew upgrade maestro`"),
            Self::Cargo => formatter.write_str(
                "for this install: installed with Cargo. Run `cargo install --locked --force maestro`",
            ),
            Self::NoPlatformAsset { hint } => {
                write!(formatter, "because no GitHub release asset matches {hint}")
            }
        }
    }
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
    Unavailable(UpdateUnavailable),
}

/// Read-only update check result.
#[derive(Debug, Eq, PartialEq)]
pub enum UpdateCheck {
    /// A newer release is available.
    Available {
        release: ReleaseInfo,
        current_version: String,
    },
    /// The current binary already matches the newest release.
    UpToDate(ReleaseInfo),
    /// No release lookup is available for this build.
    Unavailable(UpdateUnavailable),
}

/// Request passed to release lookup/download implementations.
#[derive(Debug)]
pub struct UpdateRequest {
    /// Stage directory for downloaded update assets.
    pub work_dir: PathBuf,
    /// Current binary version string.
    pub current_version: String,
    /// Reinstall even when versions match.
    pub force: bool,
    /// Whether this operation is only checking update availability.
    pub check_only: bool,
}

/// How the running Maestro binary appears to be installed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMethod {
    /// A self-managed GitHub release binary, usually installed by curl.
    Curl,
    /// A Homebrew-managed binary.
    Homebrew,
    /// A Cargo-managed binary under a Cargo bin directory.
    Cargo,
    /// A local development binary from a Cargo target directory.
    LocalDevelopment,
}

/// Update failure phase for user-facing rollback messages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateFailurePhase {
    /// Release asset download failed.
    Download,
    /// Binary install/swap failed.
    Install,
}

/// Error with enough context to render the update transcript consistently.
#[derive(Debug, Eq, PartialEq)]
pub struct UpdateFailure {
    /// Failed phase.
    pub phase: UpdateFailurePhase,
    /// Release being installed, when known.
    pub release: Option<ReleaseInfo>,
    /// Downloaded byte count when a partial download failed.
    pub downloaded_bytes: Option<u64>,
    /// Total expected bytes when a partial download failed.
    pub total_bytes: Option<u64>,
    /// Short user-facing failure cause.
    pub message: String,
    /// Whether rollback restored previously changed files.
    pub restored: bool,
}

impl UpdateFailure {
    /// Build a download failure for future GitHub/curl downloaders.
    pub fn download(
        release: Option<ReleaseInfo>,
        downloaded_bytes: Option<u64>,
        total_bytes: Option<u64>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: UpdateFailurePhase::Download,
            release,
            downloaded_bytes,
            total_bytes,
            message: message.into(),
            restored: false,
        }
    }

    /// Build an install/swap failure after rollback has been attempted.
    pub fn install(
        release: Option<ReleaseInfo>,
        message: impl Into<String>,
        restored: bool,
    ) -> Self {
        Self {
            phase: UpdateFailurePhase::Install,
            release,
            downloaded_bytes: None,
            total_bytes: None,
            message: message.into(),
            restored,
        }
    }
}

impl fmt::Display for UpdateFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for UpdateFailure {}

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
    /// Check whether an update is available without downloading it.
    fn check(&self, request: &UpdateRequest) -> Result<UpdateCheck> {
        let _ = request;
        Ok(UpdateCheck::Unavailable(
            UpdateUnavailable::LocalDevelopment,
        ))
    }

    /// Fetch a binary candidate into `work_dir`.
    fn download(&self, request: &UpdateRequest) -> Result<DownloadedBinary>;
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
    fn download(&self, _request: &UpdateRequest) -> Result<DownloadedBinary> {
        Ok(DownloadedBinary::Unavailable(
            UpdateUnavailable::LocalDevelopment,
        ))
    }
}

/// Downloader used for install methods Maestro should not mutate directly.
#[derive(Debug)]
pub struct UnavailableDownloader {
    reason: UpdateUnavailable,
}

impl UnavailableDownloader {
    fn new(reason: UpdateUnavailable) -> Self {
        Self { reason }
    }
}

impl UpdateDownloader for UnavailableDownloader {
    fn check(&self, _request: &UpdateRequest) -> Result<UpdateCheck> {
        Ok(UpdateCheck::Unavailable(self.reason.clone()))
    }

    fn download(&self, _request: &UpdateRequest) -> Result<DownloadedBinary> {
        Ok(DownloadedBinary::Unavailable(self.reason.clone()))
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

/// Run update with the default offline-safe seams.
pub fn run_update(options: &UpdateOptions<'_>) -> Result<UpdateOutcome> {
    match detect_install_method(options.executable_path) {
        InstallMethod::Curl => {
            let downloader = GitHubCurlDownloader::new();
            run_update_with_seams(
                options,
                &downloader,
                &Sha256Verifier::disabled(),
                &AtomicBinaryReplacer,
            )
        }
        InstallMethod::Homebrew => {
            let downloader = UnavailableDownloader::new(UpdateUnavailable::Homebrew);
            run_update_with_seams(
                options,
                &downloader,
                &Sha256Verifier::disabled(),
                &AtomicBinaryReplacer,
            )
        }
        InstallMethod::Cargo => {
            let downloader = UnavailableDownloader::new(UpdateUnavailable::Cargo);
            run_update_with_seams(
                options,
                &downloader,
                &Sha256Verifier::disabled(),
                &AtomicBinaryReplacer,
            )
        }
        InstallMethod::LocalDevelopment => {
            let downloader = UnavailableDownloader::new(UpdateUnavailable::LocalDevelopment);
            run_update_with_seams(
                options,
                &downloader,
                &Sha256Verifier::disabled(),
                &AtomicBinaryReplacer,
            )
        }
    }
}

/// Detect how the current binary is managed.
pub fn detect_install_method(executable_path: &Path) -> InstallMethod {
    if let Ok(value) = std::env::var("MAESTRO_INSTALL_METHOD") {
        match value.trim().to_ascii_lowercase().as_str() {
            "curl" | "github" | "self" => return InstallMethod::Curl,
            "brew" | "homebrew" => return InstallMethod::Homebrew,
            "cargo" => return InstallMethod::Cargo,
            "local" | "dev" | "development" => return InstallMethod::LocalDevelopment,
            _ => {}
        }
    }

    let path = executable_path.to_string_lossy();
    if path.contains("/target/debug/") || path.contains("/target/release/") {
        return InstallMethod::LocalDevelopment;
    }
    if path.contains("/.cargo/bin/") {
        return InstallMethod::Cargo;
    }
    if path.contains("/Cellar/maestro/") || path.contains("/Homebrew/") {
        return InstallMethod::Homebrew;
    }
    InstallMethod::Curl
}

/// Run update with explicit seams for tests and future release wiring.
pub fn run_update_with_seams(
    options: &UpdateOptions<'_>,
    downloader: &dyn UpdateDownloader,
    verifier: &dyn ChecksumVerifier,
    replacer: &dyn BinaryReplacer,
) -> Result<UpdateOutcome> {
    if options.check_only {
        return Ok(UpdateOutcome {
            binary_status: check_binary_update(options, downloader)?,
            skill_backups: Vec::new(),
            schema_mismatches: Vec::new(),
        });
    }
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
    let prepared_release = prepared_release(&binary_candidate);
    let binary_status = match replace_prepared_binary(options, replacer, binary_candidate) {
        Ok(status) => status,
        Err(_error) => {
            rollback_bundled_skill_writes(&extract_report)?;
            return Err(UpdateFailure::install(
                prepared_release,
                "could not replace the current binary",
                true,
            )
            .into());
        }
    };

    Ok(UpdateOutcome {
        binary_status,
        skill_backups: extract_report.backups,
        schema_mismatches,
    })
}

pub(super) fn update_request(options: &UpdateOptions<'_>) -> UpdateRequest {
    UpdateRequest {
        work_dir: options.paths.maestro_dir().join("update"),
        current_version: options.current_version.to_string(),
        force: options.force,
        check_only: options.check_only,
    }
}

fn check_binary_update(
    options: &UpdateOptions<'_>,
    downloader: &dyn UpdateDownloader,
) -> Result<BinaryStatus> {
    match downloader.check(&update_request(options))? {
        UpdateCheck::Available {
            release,
            current_version,
        } => Ok(BinaryStatus::UpdateAvailable {
            release,
            current_version,
        }),
        UpdateCheck::UpToDate(release) => Ok(BinaryStatus::UpToDate { release }),
        UpdateCheck::Unavailable(reason) => Ok(BinaryStatus::Skipped { reason }),
    }
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
        if classify(&found, expected) != Compat::Exact {
            mismatches.push(SchemaMismatch {
                path,
                expected,
                found,
            });
        }
    }

    Ok(mismatches)
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

#[cfg(test)]
mod tests {
    use super::{detect_install_method, InstallMethod};

    #[test]
    fn detects_common_install_methods_from_path() {
        assert_eq!(
            detect_install_method(std::path::Path::new("/repo/target/debug/maestro")),
            InstallMethod::LocalDevelopment
        );
        assert_eq!(
            detect_install_method(std::path::Path::new("/Users/me/.cargo/bin/maestro")),
            InstallMethod::Cargo
        );
        assert_eq!(
            detect_install_method(std::path::Path::new(
                "/opt/homebrew/Cellar/maestro/1779772576/bin/maestro"
            )),
            InstallMethod::Homebrew
        );
        assert_eq!(
            detect_install_method(std::path::Path::new("/usr/local/bin/maestro")),
            InstallMethod::Curl
        );
    }
}
