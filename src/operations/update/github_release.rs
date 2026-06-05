use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{
    ChecksumVerifier, DownloadedBinary, ReleaseInfo, Sha256Verifier, UpdateCheck, UpdateDownloader,
    UpdateFailure, UpdateRequest, UpdateUnavailable,
};
use crate::foundation::core::time::{parse_utc_timestamp, relative_age_from_unix_seconds};

const DEFAULT_RELEASE_REPO: &str = "ReinaMacCredy/maestro";

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

impl UpdateDownloader for GitHubCurlDownloader {
    fn check(&self, request: &UpdateRequest) -> Result<UpdateCheck> {
        let Some(release) = self.latest_release(request.check_only)? else {
            return Ok(UpdateCheck::Unavailable(
                UpdateUnavailable::LocalDevelopment,
            ));
        };
        let asset = self.selected_asset(&release);
        let release_info = self.release_info(&release, asset);
        match release_version_decision(&release_info.version, &request.current_version) {
            ReleaseVersionDecision::Current if !request.force => {
                return Ok(UpdateCheck::UpToDate(release_info));
            }
            ReleaseVersionDecision::LocalNewer => {
                return Ok(UpdateCheck::LocalNewer {
                    release: release_info,
                    current_version: request.current_version.clone(),
                });
            }
            ReleaseVersionDecision::Current | ReleaseVersionDecision::Available => {}
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
        let asset = self.selected_asset(&release);
        let release_info = self.release_info(&release, asset);
        match release_version_decision(&release_info.version, &request.current_version) {
            ReleaseVersionDecision::Current if !request.force => {
                return Ok(DownloadedBinary::UpToDate(release_info));
            }
            ReleaseVersionDecision::LocalNewer => {
                return Ok(DownloadedBinary::LocalNewer {
                    release: release_info,
                    current_version: request.current_version.clone(),
                });
            }
            ReleaseVersionDecision::Current | ReleaseVersionDecision::Available => {}
        }
        let Some(asset) = asset else {
            return Err(UpdateFailure::download(
                Some(release_info),
                None,
                None,
                format!("no GitHub release asset matches {}", platform_asset_hint()),
            )
            .into());
        };

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
        let digest = require_sha256_digest(asset)?;
        Sha256Verifier::new(digest).verify(&candidate)?;

        Ok(DownloadedBinary::Available {
            path: candidate,
            release: Some(release_info),
        })
    }
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

/// Require a verifiable sha256 digest for a release asset.
///
/// GitHub populates a `sha256:` digest on release assets; the updater verifies
/// the download against it. When the digest is absent or uses another
/// algorithm there is nothing to check, so fail closed rather than install an
/// unverified binary.
fn require_sha256_digest(asset: &GithubAsset) -> Result<&str> {
    asset.sha256_digest().with_context(|| {
        format!(
            "release asset `{}` has no sha256 checksum; refusing to install an unverified binary",
            asset.name
        )
    })
}

fn release_repo() -> String {
    if let Ok(repo) = std::env::var("MAESTRO_RELEASE_REPO")
        && !repo.trim().is_empty()
    {
        return repo;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseVersionDecision {
    Current,
    Available,
    LocalNewer,
}

fn release_version_decision(
    release_version: &str,
    current_version: &str,
) -> ReleaseVersionDecision {
    let release = normalized_release_version(release_version);
    let current = normalized_release_version(current_version);
    if release == current {
        return ReleaseVersionDecision::Current;
    }

    match (version_order_key(&release), version_order_key(&current)) {
        (Some(release), Some(current)) => match release.cmp(&current) {
            Ordering::Less => ReleaseVersionDecision::LocalNewer,
            Ordering::Equal | Ordering::Greater => ReleaseVersionDecision::Available,
        },
        _ => ReleaseVersionDecision::Available,
    }
}

fn version_order_key(version: &str) -> Option<Vec<u64>> {
    let numeric = version.split('-').next().unwrap_or(version);
    let components: Option<Vec<u64>> = numeric
        .split('.')
        .map(|component| component.parse::<u64>().ok())
        .collect();
    components.filter(|components| !components.is_empty())
}

fn relative_age_from_rfc3339(value: &str) -> Option<String> {
    let released = (parse_utc_timestamp(value)?.nanos_since_epoch / 1_000_000_000) as i64;
    relative_age_from_unix_seconds(released)
}

#[cfg(test)]
mod tests {
    use super::{
        GithubAsset, ReleaseVersionDecision, release_version_decision, select_platform_asset,
    };

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
    fn require_sha256_digest_returns_the_hex_for_a_sha256_asset() {
        let mut asset = asset("maestro-linux-amd64");
        asset.digest = Some("sha256:abc123".to_string());
        assert_eq!(
            super::require_sha256_digest(&asset)
                .expect("invariant: a sha256 digest should resolve"),
            "abc123"
        );
    }

    #[test]
    fn require_sha256_digest_fails_closed_without_a_digest() {
        let asset = asset("maestro-linux-amd64");
        let error =
            super::require_sha256_digest(&asset).expect_err("a missing digest must fail closed");
        assert!(
            error
                .to_string()
                .contains("refusing to install an unverified binary"),
            "error should explain the fail-closed refusal: {error}"
        );
    }

    #[test]
    fn require_sha256_digest_fails_closed_for_non_sha256_digest() {
        let mut asset = asset("maestro-linux-amd64");
        asset.digest = Some("sha512:deadbeef".to_string());
        assert!(
            super::require_sha256_digest(&asset).is_err(),
            "a non-sha256 digest must fail closed rather than skip verification"
        );
    }

    #[test]
    fn parses_release_timestamp_and_version_match() {
        assert!(super::relative_age_from_rfc3339("1970-01-01T00:00:00Z").is_some());
        assert_eq!(
            release_version_decision("v0.1.0", "0.1.0"),
            ReleaseVersionDecision::Current
        );
    }

    #[test]
    fn orders_release_versions_to_prevent_downgrades() {
        assert_eq!(
            release_version_decision(
                "v0.107.0.1780674159-g057a731",
                "0.107.0.1780680000-gf00ba47"
            ),
            ReleaseVersionDecision::LocalNewer
        );
        assert_eq!(
            release_version_decision(
                "v0.107.0.1780680000-gf00ba47",
                "0.107.0.1780674159-g057a731"
            ),
            ReleaseVersionDecision::Available
        );
        assert_eq!(
            release_version_decision(
                "v0.107.0.1780674159-g057a731",
                "0.107.0.1780674159-g057a7310"
            ),
            ReleaseVersionDecision::Available
        );
        assert_eq!(
            release_version_decision("v9.9.9-gfuture", "0.107.0.1780674159-g057a731"),
            ReleaseVersionDecision::Available
        );
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
