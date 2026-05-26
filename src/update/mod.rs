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
    /// Current binary version string.
    pub current_version: &'a str,
    /// Only check release status; do not download, replace, or refresh artifacts.
    pub check_only: bool,
    /// Reinstall even when the remote reports this version as current.
    pub force: bool,
    /// Request extra diagnostic output from future downloader implementations.
    pub verbose: bool,
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
    Unavailable(String),
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
    /// Whether the caller requested verbose diagnostics.
    pub verbose: bool,
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
            "running from a local development binary".to_string(),
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
            "running from a local development binary".to_string(),
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
    let downloader = GitHubCurlDownloader::for_repo_root(options.paths.repo_root());
    run_update_with_seams(
        options,
        &downloader,
        &Sha256Verifier::disabled(),
        &AtomicBinaryReplacer,
    )
}

/// `curl`-backed GitHub Releases downloader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubCurlDownloader {
    release_repo: Option<String>,
    api_base_url: String,
    asset_name: Option<String>,
}

impl GitHubCurlDownloader {
    /// Build the default downloader from environment overrides or git origin.
    pub fn for_repo_root(repo_root: &Path) -> Self {
        Self {
            release_repo: release_repo(repo_root),
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

    fn latest_release(&self) -> Result<Option<GithubRelease>> {
        let Some(url) = self.release_url() else {
            return Ok(None);
        };
        let response = curl_bytes(&url, None)?;
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

impl UpdateDownloader for GitHubCurlDownloader {
    fn check(&self, request: &UpdateRequest) -> Result<UpdateCheck> {
        let Some(release) = self.latest_release()? else {
            return Ok(UpdateCheck::Unavailable(
                "running from a local development binary".to_string(),
            ));
        };
        let asset = self.selected_asset(&release);
        let release_info = self.release_info(&release, asset);
        if release_versions_match(&release_info.version, &request.current_version) && !request.force
        {
            return Ok(UpdateCheck::UpToDate(release_info));
        }
        if asset.is_none() {
            return Ok(UpdateCheck::Unavailable(format!(
                "no GitHub release asset matches {}",
                platform_asset_hint()
            )));
        }
        Ok(UpdateCheck::Available {
            release: release_info,
            current_version: request.current_version.clone(),
        })
    }

    fn download(&self, request: &UpdateRequest) -> Result<DownloadedBinary> {
        let Some(release) = self.latest_release()? else {
            return Ok(DownloadedBinary::Unavailable(
                "running from a local development binary".to_string(),
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
        if let Err(error) = curl_download(&asset.browser_download_url, &candidate) {
            let downloaded = fs::metadata(&candidate).ok().map(|metadata| metadata.len());
            return Err(UpdateFailure::download(
                Some(release_info),
                downloaded,
                Some(asset.size),
                download_error_message(error),
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
    let schema_mismatches = detect_schema_mismatches(options.paths)?;
    if options.check_only {
        return Ok(UpdateOutcome {
            binary_status: check_binary_update(options, downloader)?,
            skill_backups: Vec::new(),
            schema_mismatches,
        });
    }
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
        Err(error) => {
            rollback_bundled_skill_writes(&extract_report)?;
            return Err(UpdateFailure::install(prepared_release, error.to_string(), true).into());
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
        verbose: options.verbose,
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

fn release_repo(repo_root: &Path) -> Option<String> {
    if let Ok(repo) = std::env::var("MAESTRO_RELEASE_REPO") {
        if !repo.trim().is_empty() {
            return Some(repo);
        }
    }

    let repo = git2::Repository::discover(repo_root).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    parse_github_repo(remote.url()?)
}

fn parse_github_repo(remote_url: &str) -> Option<String> {
    let trimmed = remote_url.trim().trim_end_matches(".git");
    if let Some(path) = trimmed.strip_prefix("https://github.com/") {
        return normalize_repo_path(path);
    }
    if let Some(path) = trimmed.strip_prefix("git@github.com:") {
        return normalize_repo_path(path);
    }
    None
}

fn normalize_repo_path(path: &str) -> Option<String> {
    let mut parts = path.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn curl_bytes(url: &str, output: Option<&Path>) -> Result<Vec<u8>> {
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
    curl_bytes(url, Some(output)).map(|_| ())
}

fn download_error_message(error: anyhow::Error) -> String {
    let message = error.to_string();
    if message.trim().is_empty() {
        "download interrupted".to_string()
    } else {
        message
    }
}

fn select_platform_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    let os_aliases = os_aliases();
    let arch_aliases = arch_aliases();
    assets
        .iter()
        .filter(|asset| asset.name.to_ascii_lowercase().contains("maestro"))
        .filter(|asset| !is_checksum_asset(&asset.name))
        .find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            os_aliases.iter().any(|alias| name.contains(alias.as_str()))
                && arch_aliases
                    .iter()
                    .any(|alias| name.contains(alias.as_str()))
        })
        .or_else(|| {
            assets
                .iter()
                .filter(|asset| !is_checksum_asset(&asset.name))
                .find(|asset| asset.name == "maestro")
        })
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

fn os_aliases() -> Vec<String> {
    match std::env::consts::OS {
        "macos" => vec![
            "macos".to_string(),
            "darwin".to_string(),
            "apple".to_string(),
        ],
        "linux" => vec!["linux".to_string()],
        "windows" => vec!["windows".to_string(), "win".to_string()],
        other => vec![other.to_string()],
    }
}

fn arch_aliases() -> Vec<String> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["x86_64".to_string(), "amd64".to_string()],
        "aarch64" => vec!["aarch64".to_string(), "arm64".to_string()],
        "arm" => vec!["arm".to_string()],
        other => vec![other.to_string()],
    }
}

fn normalized_release_version(tag_name: &str) -> String {
    tag_name.strip_prefix('v').unwrap_or(tag_name).to_string()
}

fn release_versions_match(release_version: &str, current_version: &str) -> bool {
    normalized_release_version(release_version) == normalized_release_version(current_version)
}

fn relative_age_from_rfc3339(value: &str) -> Option<String> {
    let released = parse_rfc3339_utc(value)?;
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

fn parse_rfc3339_utc(value: &str) -> Option<i64> {
    let value = value.strip_suffix('Z')?;
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }
    let time = time.split('.').next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second = time_parts.next()?.parse::<u32>().ok()?;
    if time_parts.next().is_some() {
        return None;
    }
    Some(
        days_from_civil(year, month, day)? * 86_400
            + hour as i64 * 3_600
            + minute as i64 * 60
            + second as i64,
    )
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) as i64)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_github_repo, parse_rfc3339_utc, release_versions_match, select_platform_asset,
        GithubAsset,
    };

    #[test]
    fn parses_github_remote_urls() {
        assert_eq!(
            parse_github_repo("https://github.com/ReinaMacCredy/maestro.git"),
            Some("ReinaMacCredy/maestro".to_string())
        );
        assert_eq!(
            parse_github_repo("git@github.com:ReinaMacCredy/maestro.git"),
            Some("ReinaMacCredy/maestro".to_string())
        );
        assert_eq!(parse_github_repo("https://example.com/repo.git"), None);
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
        assert_eq!(parse_rfc3339_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_utc("1970-01-01T00:00:01.000Z"), Some(1));
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
