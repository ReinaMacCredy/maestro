use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::domain::skills::extract::{
    extract_bundled_skills, rollback_bundled_skill_writes, ExtractMode, SkillBackup,
};
use crate::foundation::core::fs::ensure_parent_dir;
use crate::foundation::core::hash::hex_digest;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{
    classify, Compat, BACKLOG_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION,
    INSTALL_LOCK_SCHEMA_VERSION,
};
use crate::foundation::core::time::parse_utc_timestamp;

static REPLACE_COUNTER: AtomicU64 = AtomicU64::new(0);
const DEFAULT_RELEASE_REPO: &str = "ReinaMacCredy/maestro";

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

/// `curl`-backed GitHub Releases downloader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubCurlDownloader {
    release_repo: Option<String>,
    api_base_url: String,
    asset_name: Option<String>,
}

impl GitHubCurlDownloader {
    /// Build the default downloader from environment overrides.
    pub fn new() -> Self {
        Self {
            release_repo: Some(release_repo()),
            api_base_url: std::env::var("MAESTRO_RELEASE_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.github.com".to_string()),
            asset_name: std::env::var("MAESTRO_RELEASE_ASSET").ok(),
        }
    }

    fn release_url(&self) -> Option<String> {
        let repo = self.release_repo.as_ref()?;
        Some(format!(
            "{}/repos/{repo}/releases/latest",
            self.api_base_url.trim_end_matches('/')
        ))
    }

    fn latest_release(&self, check_only: bool) -> Result<Option<GithubRelease>> {
        let Some(url) = self.release_url() else {
            return Ok(None);
        };
        let response = curl_bytes(&url, None, check_only)?;
        let release: GithubRelease = serde_json::from_slice(&response)
            .with_context(|| format!("failed to parse GitHub release response from {url}"))?;
        Ok(Some(release))
    }

    fn selected_asset<'a>(&self, release: &'a GithubRelease) -> Option<&'a GithubAsset> {
        if let Some(asset_name) = self.asset_name.as_deref() {
            return release.assets.iter().find(|asset| asset.name == asset_name);
        }
        select_platform_asset(&release.assets)
    }

    fn release_info(&self, release: &GithubRelease, asset: Option<&GithubAsset>) -> ReleaseInfo {
        ReleaseInfo {
            version: normalized_release_version(&release.tag_name),
            released_at: release.published_at.clone(),
            relative_age: release
                .published_at
                .as_deref()
                .and_then(relative_age_from_rfc3339),
            size_bytes: asset.map(|asset| asset.size),
        }
    }
}

impl Default for GitHubCurlDownloader {
    fn default() -> Self {
        Self::new()
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

impl UpdateDownloader for GitHubCurlDownloader {
    fn check(&self, request: &UpdateRequest) -> Result<UpdateCheck> {
        let Some(release) = self.latest_release(request.check_only)? else {
            return Ok(UpdateCheck::Unavailable(
                UpdateUnavailable::LocalDevelopment,
            ));
        };
        let asset = self.selected_asset(&release);
        let release_info = self.release_info(&release, asset);
        if release_versions_match(&release_info.version, &request.current_version) && !request.force
        {
            return Ok(UpdateCheck::UpToDate(release_info));
        }
        if asset.is_none() {
            return Ok(UpdateCheck::Unavailable(
                UpdateUnavailable::NoPlatformAsset {
                    hint: platform_asset_hint(),
                },
            ));
        }
        Ok(UpdateCheck::Available {
            release: release_info,
            current_version: request.current_version.clone(),
        })
    }

    fn download(&self, request: &UpdateRequest) -> Result<DownloadedBinary> {
        let Some(release) = self.latest_release(request.check_only)? else {
            return Ok(DownloadedBinary::Unavailable(
                UpdateUnavailable::LocalDevelopment,
            ));
        };
        let asset = self.selected_asset(&release).ok_or_else(|| {
            UpdateFailure::download(
                Some(self.release_info(&release, None)),
                None,
                None,
                format!("no GitHub release asset matches {}", platform_asset_hint()),
            )
        })?;
        let release_info = self.release_info(&release, Some(asset));
        if release_versions_match(&release_info.version, &request.current_version) && !request.force
        {
            return Ok(DownloadedBinary::UpToDate(release_info));
        }

        fs::create_dir_all(&request.work_dir)
            .with_context(|| format!("failed to create {}", request.work_dir.display()))?;
        let candidate = request.work_dir.join("maestro-update-candidate");
        if curl_download(&asset.browser_download_url, &candidate).is_err() {
            let downloaded = fs::metadata(&candidate).ok().map(|metadata| metadata.len());
            return Err(UpdateFailure::download(
                Some(release_info),
                downloaded,
                Some(asset.size),
                "download interrupted",
            )
            .into());
        }
        if let Some(digest) = asset.sha256_digest() {
            Sha256Verifier::new(digest).verify(&candidate)?;
        }

        Ok(DownloadedBinary::Available {
            path: candidate,
            release: Some(release_info),
        })
    }
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

fn update_request(options: &UpdateOptions<'_>) -> UpdateRequest {
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

enum PreparedBinary {
    UpToDate {
        release: ReleaseInfo,
    },
    Skipped {
        reason: UpdateUnavailable,
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
    let request = update_request(options);
    let work_dir = request.work_dir.clone();
    let candidate = match downloader.download(&request) {
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

fn prepared_release(binary: &PreparedBinary) -> Option<ReleaseInfo> {
    match binary {
        PreparedBinary::UpToDate { release } => Some(release.clone()),
        PreparedBinary::Skipped { .. } => None,
        PreparedBinary::Candidate { release, .. } => release.clone(),
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

    let permissions = fs::metadata(current)
        .or_else(|_| fs::metadata(candidate))
        .with_context(|| {
            format!(
                "failed to inspect update replacement permissions for {}",
                candidate.display()
            )
        })?
        .permissions();
    let temp_path = temp_sibling_path(current)?;

    if let Err(error) = copy_candidate_to_temp(candidate, &temp_path, permissions) {
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

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

impl GithubAsset {
    fn sha256_digest(&self) -> Option<&str> {
        self.digest.as_deref()?.strip_prefix("sha256:")
    }
}

fn release_repo() -> String {
    if let Ok(repo) = std::env::var("MAESTRO_RELEASE_REPO") {
        if !repo.trim().is_empty() {
            return repo;
        }
    }
    DEFAULT_RELEASE_REPO.to_string()
}

fn curl_bytes(url: &str, output: Option<&Path>, fast_timeout: bool) -> Result<Vec<u8>> {
    let mut command = Command::new("curl");
    command.args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--header",
        "Accept: application/vnd.github+json",
        "--header",
        "User-Agent: maestro",
    ]);
    if fast_timeout {
        command.args(["--connect-timeout", "3", "--max-time", "8"]);
    } else {
        command.args(["--connect-timeout", "15", "--max-time", "600"]);
    }
    if let Some(output) = output {
        command.arg("--output");
        command.arg(output);
    }
    command.arg(url);

    let output = command
        .output()
        .with_context(|| "failed to run curl for GitHub release update")?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(output.stdout)
}

fn curl_download(url: &str, output: &Path) -> Result<()> {
    curl_bytes(url, Some(output), false).map(|_| ())
}

fn select_platform_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    let os_aliases = os_aliases();
    let arch_aliases = arch_aliases();
    let mut fallback = None;

    for asset in assets {
        if is_checksum_asset(&asset.name) {
            continue;
        }
        if asset.name == "maestro" && fallback.is_none() {
            fallback = Some(asset);
        }

        let name = asset.name.to_ascii_lowercase();
        if !name.contains("maestro") {
            continue;
        }
        if os_aliases.iter().any(|alias| name.contains(*alias))
            && arch_aliases.iter().any(|alias| name.contains(*alias))
        {
            return Some(asset);
        }
    }

    fallback
}

fn is_checksum_asset(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".sha256")
        || name.ends_with(".sha256sum")
        || name.ends_with(".sig")
        || name.ends_with(".asc")
}

fn platform_asset_hint() -> String {
    format!(
        "maestro {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn os_aliases() -> &'static [&'static str] {
    match std::env::consts::OS {
        "macos" => &["macos", "darwin", "apple"],
        "linux" => &["linux"],
        "windows" => &["windows", "win"],
        _ => &[],
    }
}

fn arch_aliases() -> &'static [&'static str] {
    match std::env::consts::ARCH {
        "x86_64" => &["x86_64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        "arm" => &["arm"],
        _ => &[],
    }
}

fn normalized_release_version(tag_name: &str) -> String {
    tag_name.strip_prefix('v').unwrap_or(tag_name).to_string()
}

fn release_versions_match(release_version: &str, current_version: &str) -> bool {
    normalized_release_version(release_version) == normalized_release_version(current_version)
}

fn relative_age_from_rfc3339(value: &str) -> Option<String> {
    let released = (parse_utc_timestamp(value)?.nanos_since_epoch / 1_000_000_000) as i64;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let elapsed = now.saturating_sub(released);
    if elapsed < 60 {
        return Some("less than 1m ago".to_string());
    }
    if elapsed < 3_600 {
        return Some(format!("{}m ago", elapsed / 60));
    }
    if elapsed < 86_400 {
        return Some(format!("{}h ago", elapsed / 3_600));
    }
    Some(format!("{}d ago", elapsed / 86_400))
}

#[cfg(test)]
mod tests {
    use super::{
        detect_install_method, release_versions_match, select_platform_asset, GithubAsset,
        InstallMethod,
    };

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

    #[test]
    fn selects_current_platform_maestro_asset() {
        let platform_asset = format!(
            "maestro-{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let assets = vec![
            asset("maestro-linux-amd64.sha256"),
            asset("maestro-windows-amd64.exe"),
            asset(&platform_asset),
        ];

        assert_eq!(
            select_platform_asset(&assets).map(|asset| asset.name.as_str()),
            Some(platform_asset.as_str())
        );
    }

    #[test]
    fn parses_release_timestamp_and_version_match() {
        assert!(super::relative_age_from_rfc3339("1970-01-01T00:00:00Z").is_some());
        assert!(release_versions_match("v0.1.0", "0.1.0"));
    }

    fn asset(name: &str) -> GithubAsset {
        GithubAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.test/{name}"),
            size: 123,
            digest: None,
        }
    }
}
