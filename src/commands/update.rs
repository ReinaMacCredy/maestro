use std::env;

use anyhow::Result;

use crate::core::backup::backup_operation_timestamp;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::update::{run_update, BinaryStatus, ReleaseInfo, UpdateOptions, UpdateOutcome};

/// Execute `maestro update`.
pub fn run() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let executable_path = env::current_exe()?;
    let backup_timestamp = backup_operation_timestamp()?;
    println!("Checking for updates...");

    let outcome = match run_update(&UpdateOptions {
        paths: &paths,
        executable_path: &executable_path,
        backup_timestamp: &backup_timestamp,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            println!("Update failed: {error}");
            println!();
            println!("Your current Maestro install was not changed.");
            return Err(error);
        }
    };

    print!("{}", render_outcome(&outcome));

    Ok(())
}

fn render_outcome(outcome: &UpdateOutcome) -> String {
    let mut out = String::new();
    match &outcome.binary_status {
        BinaryStatus::UpToDate { release } => {
            out.push_str(&format!(
                "✓ Maestro is already up to date ({})\n",
                release_summary(release)
            ));
        }
        BinaryStatus::Skipped { reason } => {
            out.push_str(&format!("Update unavailable for this build: {reason}.\n"));
        }
        BinaryStatus::Replaced { release, .. } => {
            if let Some(release) = release {
                out.push_str(&format!("Updating to version {}...\n", release.version));
                out.push_str(&download_line(release));
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

fn download_line(release: &ReleaseInfo) -> String {
    match release.size_bytes {
        Some(size_bytes) => {
            let size = format_size(size_bytes);
            format!("Downloading update ({size}/{size})\n")
        }
        None => "Downloading update...\n".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{render_outcome, ReleaseInfo};
    use crate::update::{BinaryStatus, UpdateOutcome};

    #[test]
    fn renders_no_update_with_release_timestamp_and_age() {
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
            render_outcome(&outcome),
            "✓ Maestro is already up to date (0.0.1779772576-g751b94 (released 2026-05-26T05:16:16.000Z, 1h ago))\n"
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
            render_outcome(&outcome),
            concat!(
                "Updating to version 0.0.1779772576-g751b94...\n",
                "Downloading update (25.35 MB/25.35 MB)\n",
                "Installing update...\n",
                "✓ Maestro updated to version 0.0.1779772576-g751b94 (released 2026-05-26T05:16:16.000Z, 1h ago)\n",
            )
        );
    }
}
