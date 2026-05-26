use std::env;

use anyhow::Result;

use crate::commands::UpdateArgs;
use crate::core::backup::backup_operation_timestamp;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::update::{
    run_update, BinaryStatus, ReleaseInfo, UpdateFailure, UpdateFailurePhase, UpdateOptions,
    UpdateOutcome,
};

/// Marker error for update failures that already rendered user-facing output.
#[derive(Debug)]
pub struct ReportedError;

impl std::fmt::Display for ReportedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("update failed")
    }
}

impl std::error::Error for ReportedError {}

/// Execute `maestro update`.
pub fn run(args: UpdateArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let executable_path = env::current_exe()?;
    let backup_timestamp = backup_operation_timestamp()?;
    println!("Checking for updates...");

    let outcome = match run_update(&UpdateOptions {
        paths: &paths,
        executable_path: &executable_path,
        backup_timestamp: &backup_timestamp,
        current_version: env!("MAESTRO_BUILD_VERSION"),
        check_only: args.check,
        force: args.force,
        verbose: args.verbose,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            if let Some(failure) = error.downcast_ref::<UpdateFailure>() {
                print!("{}", render_failure(failure));
            } else {
                println!("Update failed: {}", sentence(error.to_string()));
                println!();
                println!("Your current Maestro install was not changed.");
            }
            return Err(ReportedError.into());
        }
    };

    print!("{}", render_outcome(&outcome, args.verbose));

    Ok(())
}

fn render_outcome(outcome: &UpdateOutcome, verbose: bool) -> String {
    let mut out = String::new();
    match &outcome.binary_status {
        BinaryStatus::UpdateAvailable {
            release,
            current_version,
        } => {
            out.push_str(&format!(
                "Update available: {}\n",
                release_summary_short(release)
            ));
            out.push_str(&format!("Current version: {current_version}\n"));
        }
        BinaryStatus::UpToDate { release } => {
            out.push_str(&format!(
                "✓ Maestro is already up to date ({})\n",
                release.version
            ));
        }
        BinaryStatus::Skipped { reason } => {
            out.push_str(&format!("Update unavailable for this build: {reason}.\n"));
        }
        BinaryStatus::Replaced { release, .. } => {
            if let Some(release) = release {
                out.push_str(&format!("Updating to version {}...\n", release.version));
                out.push_str(&download_complete_line(release));
            }
            out.push_str("Installing update...\n");
            if let Some(release) = release {
                out.push_str(&format!(
                    "✓ Maestro updated to version {}\n",
                    release_summary(release)
                ));
            } else {
                out.push_str("✓ Maestro updated\n");
            }
        }
    }

    if !outcome.skill_backups.is_empty() {
        out.push_str("Bundled skills re-extracted; edited skills backed up:\n");
        for backup in &outcome.skill_backups {
            out.push_str(&format!(
                "{} -> {}\n",
                backup.skill_name,
                backup.path.display()
            ));
        }
    }

    if verbose {
        if let BinaryStatus::Replaced { path, .. } = &outcome.binary_status {
            out.push_str(&format!("Installed binary: {}\n", path.display()));
        }
    }

    if !outcome.schema_mismatches.is_empty() {
        out.push_str(
            "Core harness/install schema mismatch detected; run `maestro migrate` before writing artifacts:\n",
        );
        for mismatch in &outcome.schema_mismatches {
            out.push_str(&format!(
                "{} expected {} found {}",
                mismatch.path.display(),
                mismatch.expected,
                mismatch.found
            ));
            out.push('\n');
        }
    }
    out
}

fn render_failure(failure: &UpdateFailure) -> String {
    let mut out = String::new();
    if let Some(release) = &failure.release {
        out.push_str(&format!("Updating to version {}...\n", release.version));
        match failure.phase {
            UpdateFailurePhase::Download => {
                out.push_str(&download_progress_line(
                    release,
                    failure.downloaded_bytes,
                    failure.total_bytes,
                ));
            }
            UpdateFailurePhase::Install => {
                out.push_str(&download_complete_line(release));
                out.push_str("Installing update...\n");
            }
        }
    }
    out.push_str(&format!("Update failed: {}\n", sentence(&failure.message)));
    out.push('\n');
    if failure.restored {
        out.push_str("Your current Maestro install was restored.\n");
    } else {
        out.push_str("Your current Maestro install was not changed.\n");
    }
    out
}

fn download_complete_line(release: &ReleaseInfo) -> String {
    match release.size_bytes {
        Some(size_bytes) => {
            let size = format_size(size_bytes);
            format!("Downloading update ({size}/{size})\n")
        }
        None => "Downloading update...\n".to_string(),
    }
}

fn download_progress_line(
    release: &ReleaseInfo,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
) -> String {
    match (downloaded_bytes, total_bytes.or(release.size_bytes)) {
        (Some(downloaded), Some(total)) => {
            format!(
                "Downloading update ({}/{})\n",
                format_size(downloaded),
                format_size(total)
            )
        }
        _ => "Downloading update...\n".to_string(),
    }
}

fn release_summary(release: &ReleaseInfo) -> String {
    match (&release.released_at, &release.relative_age) {
        (Some(released_at), Some(relative_age)) => {
            format!(
                "{} (released {}, {})",
                release.version, released_at, relative_age
            )
        }
        (Some(released_at), None) => format!("{} (released {})", release.version, released_at),
        (None, Some(relative_age)) => format!("{} (released {})", release.version, relative_age),
        (None, None) => release.version.clone(),
    }
}

fn release_summary_short(release: &ReleaseInfo) -> String {
    match &release.relative_age {
        Some(relative_age) => format!("{} (released {})", release.version, relative_age),
        None => release_summary(release),
    }
}

fn format_size(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / MB)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

fn sentence(message: impl AsRef<str>) -> String {
    let message = message.as_ref().trim();
    if message.ends_with('.') {
        message.to_string()
    } else {
        format!("{message}.")
    }
}

#[cfg(test)]
mod tests {
    use super::{render_failure, render_outcome, ReleaseInfo};
    use crate::update::{BinaryStatus, UpdateFailure, UpdateOutcome};

    #[test]
    fn renders_no_update_as_version_only() {
        let outcome = UpdateOutcome {
            binary_status: BinaryStatus::UpToDate {
                release: ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: None,
                },
            },
            skill_backups: Vec::new(),
            schema_mismatches: Vec::new(),
        };

        assert_eq!(
            render_outcome(&outcome, false),
            "✓ Maestro is already up to date (0.0.1779772576-g751b94)\n"
        );
    }

    #[test]
    fn renders_update_progress_with_download_size() {
        let outcome = UpdateOutcome {
            binary_status: BinaryStatus::Replaced {
                path: std::path::PathBuf::from("/tmp/maestro"),
                release: Some(ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: Some(25_350_000),
                }),
            },
            skill_backups: Vec::new(),
            schema_mismatches: Vec::new(),
        };

        assert_eq!(
            render_outcome(&outcome, false),
            concat!(
                "Updating to version 0.0.1779772576-g751b94...\n",
                "Downloading update (25.35 MB/25.35 MB)\n",
                "Installing update...\n",
                "✓ Maestro updated to version 0.0.1779772576-g751b94 (released 2026-05-26T05:16:16.000Z, 1h ago)\n",
            )
        );
    }

    #[test]
    fn renders_no_update_without_release_metadata() {
        let outcome = UpdateOutcome {
            binary_status: BinaryStatus::UpToDate {
                release: ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: None,
                    relative_age: None,
                    size_bytes: None,
                },
            },
            skill_backups: Vec::new(),
            schema_mismatches: Vec::new(),
        };

        assert_eq!(
            render_outcome(&outcome, false),
            "✓ Maestro is already up to date (0.0.1779772576-g751b94)\n"
        );
    }

    #[test]
    fn renders_check_update_available() {
        let outcome = UpdateOutcome {
            binary_status: BinaryStatus::UpdateAvailable {
                release: ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: Some(25_350_000),
                },
                current_version: "0.0.1779700000-gabc123".to_string(),
            },
            skill_backups: Vec::new(),
            schema_mismatches: Vec::new(),
        };

        assert_eq!(
            render_outcome(&outcome, false),
            concat!(
                "Update available: 0.0.1779772576-g751b94 (released 1h ago)\n",
                "Current version: 0.0.1779700000-gabc123\n",
            )
        );
    }

    #[test]
    fn renders_download_failure_with_partial_progress() {
        let failure = UpdateFailure::download(
            Some(ReleaseInfo {
                version: "0.0.1779772576-g751b94".to_string(),
                released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                relative_age: Some("1h ago".to_string()),
                size_bytes: Some(25_350_000),
            }),
            Some(8_140_000),
            Some(25_350_000),
            "download interrupted",
        );

        assert_eq!(
            render_failure(&failure),
            concat!(
                "Updating to version 0.0.1779772576-g751b94...\n",
                "Downloading update (8.14 MB/25.35 MB)\n",
                "Update failed: download interrupted.\n",
                "\n",
                "Your current Maestro install was not changed.\n",
            )
        );
    }

    #[test]
    fn renders_install_failure_with_restore_message() {
        let failure = UpdateFailure::install(
            Some(ReleaseInfo {
                version: "0.0.1779772576-g751b94".to_string(),
                released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                relative_age: Some("1h ago".to_string()),
                size_bytes: Some(25_350_000),
            }),
            "could not replace the current binary",
            true,
        );

        assert_eq!(
            render_failure(&failure),
            concat!(
                "Updating to version 0.0.1779772576-g751b94...\n",
                "Downloading update (25.35 MB/25.35 MB)\n",
                "Installing update...\n",
                "Update failed: could not replace the current binary.\n",
                "\n",
                "Your current Maestro install was restored.\n",
            )
        );
    }
}
