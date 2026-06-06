use std::env;
use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::domain::skills;
use crate::foundation::core::backup::backup_operation_timestamp;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::paths::{MaestroPaths, announce_repo_root, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::interfaces::cli::UpdateArgs;
use crate::operations::update;

const AUTO_CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60;

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
    let paths = optional_repo_paths()?;
    let executable_path = env::current_exe()?;
    let backup_timestamp = backup_operation_timestamp()?;
    let colors = Colors::detect();
    println!("{}", colors.info("Checking for updates..."));

    let outcome = match update::run_update(&update::UpdateOptions {
        paths: paths.as_ref(),
        executable_path: &executable_path,
        backup_timestamp: &backup_timestamp,
        current_version: env!("MAESTRO_VERSION"),
        check_only: args.check,
        force: args.force,
        global_skills_home: None,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            if let Some(failure) = error.downcast_ref::<update::UpdateFailure>() {
                print!("{}", render_failure(failure, colors));
            } else {
                println!(
                    "{}",
                    colors.error(&format!("Update failed: {}", sentence(error.to_string())))
                );
                println!();
                println!("Your current Maestro install was not changed.");
            }
            return Err(ReportedError.into());
        }
    };

    print!("{}", render_outcome(&outcome, args.verbose, colors));

    Ok(())
}

fn optional_repo_paths() -> Result<Option<MaestroPaths>> {
    match discover_repo_root() {
        Ok(repo_root) => {
            announce_repo_root(&repo_root);
            Ok(Some(MaestroPaths::new(repo_root)))
        }
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::RepoRootNotFound { .. })
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Run a passive once-per-day update check. It only prints when an update is available.
pub fn run_auto_check() -> Result<()> {
    if env::var("MAESTRO_AUTO_UPDATE")
        .map(|value| value == "0" || value.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return Ok(());
    }

    let executable_path = env::current_exe()?;
    if update::detect_install_method(&executable_path) != update::InstallMethod::Curl {
        return Ok(());
    }

    let repo_root = match discover_repo_root() {
        Ok(repo_root) => repo_root,
        Err(_) => return Ok(()),
    };
    let paths = MaestroPaths::new(repo_root);
    let now = current_unix_seconds()?;
    if !auto_check_due(&paths, now)? {
        return Ok(());
    }

    let outcome = update::run_update(&update::UpdateOptions {
        paths: Some(&paths),
        executable_path: &executable_path,
        backup_timestamp: "",
        current_version: env!("MAESTRO_VERSION"),
        check_only: true,
        force: false,
        global_skills_home: None,
    })?;
    record_auto_check(&paths, now)?;

    if let update::BinaryStatus::UpdateAvailable { release, .. } = outcome.binary_status {
        let colors = Colors::detect();
        eprintln!(
            "{}",
            colors.info(&format!(
                "Update available: {}. Run `maestro update` to install.",
                release_summary_short(&release)
            ))
        );
    }

    Ok(())
}

fn render_outcome(outcome: &update::UpdateOutcome, verbose: bool, colors: Colors) -> String {
    let mut out = String::new();
    match &outcome.binary_status {
        update::BinaryStatus::UpdateAvailable {
            release,
            current_version,
        } => {
            out.push_str(&format!(
                "Update available: {}\n",
                colors.info(&release_summary_short(release))
            ));
            out.push_str(&format!("Current version: {current_version}\n"));
        }
        update::BinaryStatus::UpToDate { release } => {
            out.push_str(&colors.success(&format!(
                "✓ Maestro is already up to date ({})",
                release.version
            )));
            out.push('\n');
        }
        update::BinaryStatus::LocalNewer {
            release,
            current_version,
        } => {
            out.push_str(&colors.success("✓ Maestro is newer than the latest GitHub release"));
            out.push('\n');
            out.push_str(&format!("Current version: {current_version}\n"));
            out.push_str(&format!(
                "Latest GitHub release: {}\n",
                release_summary_short(release)
            ));
        }
        update::BinaryStatus::Skipped { reason } => {
            out.push_str(&format!("Update unavailable {reason}.\n"));
        }
        update::BinaryStatus::Replaced { release, .. } => {
            if let Some(release) = release {
                out.push_str(&colors.info(&format!("Updating to version {}...", release.version)));
                out.push('\n');
                out.push_str(&download_complete_line(release));
            }
            if let Some(release) = release {
                out.push_str(&colors.success(&format!(
                    "✓ Maestro updated to version {}",
                    release_summary_short(release)
                )));
                out.push('\n');
            } else {
                out.push_str(&colors.success("✓ Maestro updated"));
                out.push('\n');
            }
        }
    }

    if outcome.repo_uninitialized {
        out.push_str(
            "No `.maestro` here; run `maestro init` to set up this repo (bundled resources were not extracted).\n",
        );
    }

    if let Some(global_skills) = &outcome.global_skills {
        out.push_str(&skills::render_global_skills_outcome(global_skills));
    }
    if let Some(warning) = &outcome.global_skills_warning {
        out.push_str(&format!("warning: {warning}\n"));
    }

    // A created file (no `previous`) has no backup, so it is otherwise invisible
    // here -- the very gap that made a Cargo/local `update` look like a no-op while
    // it silently restored a deleted bundled resource. Edited files already surface
    // via the backup block below.
    let restored: Vec<_> = outcome
        .resource_writes
        .iter()
        .filter(|write| write.previous.is_none())
        .collect();
    if !restored.is_empty() {
        out.push_str("Bundled resources re-extracted; missing files restored:\n");
        for write in restored {
            out.push_str(&format!("{} -> {}\n", write.name, write.path.display()));
        }
    }

    if !outcome.resource_backups.is_empty() {
        out.push_str("Bundled resources re-extracted; edited files backed up:\n");
        for backup in &outcome.resource_backups {
            out.push_str(&format!("{} -> {}\n", backup.name, backup.path.display()));
        }
    }

    if verbose && let update::BinaryStatus::Replaced { path, .. } = &outcome.binary_status {
        out.push_str(&format!("Installed binary: {}\n", path.display()));
    }

    if !outcome.schema_mismatches.is_empty() {
        out.push_str(
            "Core harness/install schema mismatch detected; these artifacts are incompatible with this maestro version and were left unchanged:\n",
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
        out.push_str(
            "There is no in-place migration: remove the listed file(s) and re-run `maestro init --merge` (or `maestro install` for the install lock) to recreate them at the current schema, or use a maestro version matching the existing schema.\n",
        );
    }
    out
}

fn render_failure(failure: &update::UpdateFailure, colors: Colors) -> String {
    let mut out = String::new();
    if let Some(release) = &failure.release {
        out.push_str(&colors.info(&format!("Updating to version {}...", release.version)));
        out.push('\n');
        match failure.phase {
            update::UpdateFailurePhase::Download => {
                out.push_str(&download_progress_line(
                    release,
                    failure.downloaded_bytes,
                    failure.total_bytes,
                ));
            }
            update::UpdateFailurePhase::Install => {
                out.push_str(&download_complete_line(release));
                out.push_str("Installing update...\n");
            }
        }
    }
    out.push_str(&colors.error(&format!("Update failed: {}", sentence(&failure.message))));
    out.push('\n');
    out.push('\n');
    if failure.restored {
        out.push_str("Your current Maestro install was restored.\n");
    } else {
        out.push_str("Your current Maestro install was not changed.\n");
    }
    out
}

#[derive(Clone, Copy)]
struct Colors {
    enabled: bool,
}

impl Colors {
    fn detect() -> Self {
        let enabled = match env::var("MAESTRO_COLOR") {
            Ok(value) if value.eq_ignore_ascii_case("always") => true,
            Ok(value) if value.eq_ignore_ascii_case("never") => false,
            _ => std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none(),
        };
        Self { enabled }
    }

    #[cfg(test)]
    fn plain() -> Self {
        Self { enabled: false }
    }

    #[cfg(test)]
    fn always() -> Self {
        Self { enabled: true }
    }

    fn info(&self, text: &str) -> String {
        self.paint("94", text)
    }

    fn success(&self, text: &str) -> String {
        self.paint("32", text)
    }

    fn error(&self, text: &str) -> String {
        self.paint("31", text)
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
}

fn auto_check_due(paths: &MaestroPaths, now: u64) -> Result<bool> {
    let path = auto_check_stamp_path(paths);
    let Some(contents) = read_to_string_if_exists(&path)? else {
        return Ok(true);
    };
    let Some(last_check) = contents.trim().parse::<u64>().ok() else {
        return Ok(true);
    };
    Ok(now.saturating_sub(last_check) >= AUTO_CHECK_INTERVAL_SECONDS)
}

fn record_auto_check(paths: &MaestroPaths, now: u64) -> Result<()> {
    let path = auto_check_stamp_path(paths);
    write_string_atomic(path, &now.to_string())?;
    Ok(())
}

fn auto_check_stamp_path(paths: &MaestroPaths) -> std::path::PathBuf {
    paths.maestro_dir().join("update-check")
}

fn current_unix_seconds() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn download_complete_line(release: &update::ReleaseInfo) -> String {
    match release.size_bytes {
        Some(size_bytes) => {
            let size = format_size(size_bytes);
            format!("Downloading update ({size}/{size})\n")
        }
        None => "Downloading update...\n".to_string(),
    }
}

fn download_progress_line(
    release: &update::ReleaseInfo,
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

fn release_summary(release: &update::ReleaseInfo) -> String {
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

fn release_summary_short(release: &update::ReleaseInfo) -> String {
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
    use super::{Colors, auto_check_due, record_auto_check, render_failure, render_outcome};
    use crate::foundation::core::paths::MaestroPaths;
    use crate::operations::update;

    #[test]
    fn renders_no_update_as_version_only() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::UpToDate {
                release: update::ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: None,
                },
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: None,
        };

        assert_eq!(
            render_outcome(&outcome, false, Colors::plain()),
            "✓ Maestro is already up to date (0.0.1779772576-g751b94)\n"
        );
    }

    #[test]
    fn renders_update_progress_with_download_size() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::Replaced {
                path: std::path::PathBuf::from("/tmp/maestro"),
                release: Some(update::ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: Some(25_350_000),
                }),
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: None,
        };

        assert_eq!(
            render_outcome(&outcome, false, Colors::plain()),
            concat!(
                "Updating to version 0.0.1779772576-g751b94...\n",
                "Downloading update (25.35 MB/25.35 MB)\n",
                "✓ Maestro updated to version 0.0.1779772576-g751b94 (released 1h ago)\n",
            )
        );
    }

    #[test]
    fn renders_global_skill_warning_after_successful_update() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::Replaced {
                path: std::path::PathBuf::from("/tmp/maestro"),
                release: Some(update::ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: Some(25_350_000),
                }),
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: Some(
                "global Maestro skill sync skipped: late collision".to_string(),
            ),
        };

        assert_eq!(
            render_outcome(&outcome, false, Colors::plain()),
            concat!(
                "Updating to version 0.0.1779772576-g751b94...\n",
                "Downloading update (25.35 MB/25.35 MB)\n",
                "✓ Maestro updated to version 0.0.1779772576-g751b94 (released 1h ago)\n",
                "warning: global Maestro skill sync skipped: late collision\n",
            )
        );
    }

    #[test]
    fn renders_no_update_without_release_metadata() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::UpToDate {
                release: update::ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: None,
                    relative_age: None,
                    size_bytes: None,
                },
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: None,
        };

        assert_eq!(
            render_outcome(&outcome, false, Colors::plain()),
            "✓ Maestro is already up to date (0.0.1779772576-g751b94)\n"
        );
    }

    #[test]
    fn renders_check_update_available() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::UpdateAvailable {
                release: update::ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: Some(25_350_000),
                },
                current_version: "0.0.1779700000-gabc123".to_string(),
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: None,
        };

        assert_eq!(
            render_outcome(&outcome, false, Colors::plain()),
            concat!(
                "Update available: 0.0.1779772576-g751b94 (released 1h ago)\n",
                "Current version: 0.0.1779700000-gabc123\n",
            )
        );
    }

    #[test]
    fn renders_local_newer_without_advertising_an_update() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::LocalNewer {
                release: update::ReleaseInfo {
                    version: "0.0.1779700000-gabc123".to_string(),
                    released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                    relative_age: Some("1h ago".to_string()),
                    size_bytes: Some(25_350_000),
                },
                current_version: "0.0.1779772576-g751b94".to_string(),
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: None,
        };

        let output = render_outcome(&outcome, false, Colors::plain());
        assert!(!output.contains("Update available"));
        assert_eq!(
            output,
            concat!(
                "✓ Maestro is newer than the latest GitHub release\n",
                "Current version: 0.0.1779772576-g751b94\n",
                "Latest GitHub release: 0.0.1779700000-gabc123 (released 1h ago)\n",
            )
        );
    }

    #[test]
    fn renders_download_failure_with_partial_progress() {
        let failure = update::UpdateFailure::download(
            Some(update::ReleaseInfo {
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
            render_failure(&failure, Colors::plain()),
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
        let failure = update::UpdateFailure::install(
            Some(update::ReleaseInfo {
                version: "0.0.1779772576-g751b94".to_string(),
                released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
                relative_age: Some("1h ago".to_string()),
                size_bytes: Some(25_350_000),
            }),
            "could not replace the current binary",
            true,
        );

        assert_eq!(
            render_failure(&failure, Colors::plain()),
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

    #[test]
    fn colors_success_and_progress_lines_when_enabled() {
        let outcome = update::UpdateOutcome {
            binary_status: update::BinaryStatus::Replaced {
                path: std::path::PathBuf::from("/tmp/maestro"),
                release: Some(update::ReleaseInfo {
                    version: "0.0.1779772576-g751b94".to_string(),
                    released_at: None,
                    relative_age: None,
                    size_bytes: Some(25_350_000),
                }),
            },
            resource_backups: Vec::new(),
            resource_writes: Vec::new(),
            schema_mismatches: Vec::new(),
            repo_uninitialized: false,
            global_skills: None,
            global_skills_warning: None,
        };

        assert_eq!(
            render_outcome(&outcome, false, Colors::always()),
            concat!(
                "\u{1b}[94mUpdating to version 0.0.1779772576-g751b94...\u{1b}[0m\n",
                "Downloading update (25.35 MB/25.35 MB)\n",
                "\u{1b}[32m✓ Maestro updated to version 0.0.1779772576-g751b94\u{1b}[0m\n",
            )
        );
    }

    #[test]
    fn auto_check_stamp_enforces_24_hour_interval() {
        let temp_dir =
            std::env::temp_dir().join(format!("maestro-auto-check-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let paths = MaestroPaths::new(&temp_dir);

        assert!(auto_check_due(&paths, 100).expect("invariant: fresh stamp should be due"));
        record_auto_check(&paths, 100).expect("invariant: stamp should write");
        assert!(!auto_check_due(&paths, 100 + 60).expect("invariant: recent stamp should skip"));
        assert!(
            auto_check_due(&paths, 100 + 24 * 60 * 60)
                .expect("invariant: day-old stamp should be due")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
