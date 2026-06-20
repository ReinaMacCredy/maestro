//! Verification command execution sourced from on-disk harness config.

use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};

use super::verify_task::VerificationCommand;
use crate::domain::harness::HarnessConfig;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;

pub(super) struct VerificationCommandRun {
    pub commands: Vec<VerificationCommand>,
    pub claims_only: bool,
}

/// Outcome of running the repo-global `stack.verify` suite at the feature close gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackVerifyOutcome {
    /// The commands that ran (the configured `stack.verify`), in order, with exit codes.
    pub commands: Vec<VerificationCommand>,
    /// Detected stack kind, for messaging.
    pub stack_kind: String,
}

impl StackVerifyOutcome {
    /// Commands that exited non-zero. Empty means the suite passed.
    pub fn failed(&self) -> Vec<&VerificationCommand> {
        self.commands
            .iter()
            .filter(|command| command.exit_code != 0)
            .collect()
    }
}

/// Run the repo-global `stack.verify` suite from the repo root, ignoring any
/// per-task falsifier and the `claims_only` policy. This is the full-suite
/// backstop run at the feature close gate (decision-002), the counterpart to the
/// per-task narrow falsifier at task-verify. An empty `stack.verify` returns an
/// empty (passing) outcome — the close gate's other conditions still apply.
pub(crate) fn run_stack_verify(paths: &MaestroPaths) -> Result<StackVerifyOutcome> {
    let config = harness_verify_config(paths)?;
    let stack_kind = format!("{:?}", config.stack.kind).to_ascii_lowercase();
    let verify = config.stack.verify;
    // Serialize the heavy full suite across concurrent sessions so two machines'
    // worth of test/build jobs never run at once (decision 6a17). The guard holds
    // for the duration of the run and releases on drop. The per-task narrow
    // falsifier in `run_verify_commands` is deliberately NOT serialized. Skipped
    // when the suite is empty -- there is nothing heavy to gate.
    let _gate = (!verify.is_empty()).then(|| {
        crate::domain::gate_lock::acquire(paths, &crate::foundation::core::session::session_token())
    });
    let commands = run_commands(paths, verify)?;
    Ok(StackVerifyOutcome {
        commands,
        stack_kind,
    })
}

/// Run the verify commands for a task slice.
///
/// A per-task narrow falsifier (`verify_command`), when set, is the ONLY command
/// that runs. With no falsifier the slice runs NO commands: the repo-global
/// `stack.verify` full suite is the feature-close backstop (decision-002), not a
/// per-task fallback — so concurrent slice verifications never each re-run the
/// whole suite. `claims_only` is carried through so the caller can accept a
/// standalone slice on claims/proof alone.
pub(super) fn run_verify_commands(
    paths: &MaestroPaths,
    verify_command: Option<&str>,
) -> Result<VerificationCommandRun> {
    let (commands, claims_only) = match verify_command {
        Some(command) => (run_commands(paths, vec![command.to_string()])?, false),
        None => (
            Vec::new(),
            harness_verify_config(paths)?.claims_only_verification,
        ),
    };
    Ok(VerificationCommandRun {
        commands,
        claims_only,
    })
}

/// Run a list of shell verify commands from the repo root, in order.
fn run_commands(paths: &MaestroPaths, commands: Vec<String>) -> Result<Vec<VerificationCommand>> {
    let mut results = Vec::new();
    for command in commands {
        let started = Instant::now();
        let status = shell_command(&command)
            .current_dir(paths.repo_root())
            .status()
            .with_context(|| format!("failed to run verify command `{command}`"))?;
        results.push(VerificationCommand {
            cmd: command,
            exit_code: status.code().unwrap_or(1),
            duration_ms: started.elapsed().as_millis(),
        });
    }
    Ok(results)
}

fn harness_verify_config(paths: &MaestroPaths) -> Result<HarnessConfig> {
    let path = match managed_path(
        paths,
        ".maestro/harness/harness.yml",
        SymlinkPolicy::RejectAllComponents,
    ) {
        Ok(path) => path,
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::ManagedPathContainsSymlink { .. })
            ) =>
        {
            return Ok(HarnessConfig {
                schema_version: crate::foundation::core::schema::HARNESS_SCHEMA_VERSION.to_string(),
                stack: crate::domain::harness::StackConfig {
                    kind: crate::domain::harness::StackKind::Generic,
                    detected_by: Vec::new(),
                    verify: Vec::new(),
                },
                projects: Vec::new(),
                escalation: None,
                audit: None,
                claims_only_verification: false,
            });
        }
        Err(error) => return Err(error),
    };
    let Some(raw) = read_to_string_if_exists(&path)? else {
        return Ok(HarnessConfig {
            schema_version: crate::foundation::core::schema::HARNESS_SCHEMA_VERSION.to_string(),
            stack: crate::domain::harness::StackConfig {
                kind: crate::domain::harness::StackKind::Generic,
                detected_by: Vec::new(),
                verify: Vec::new(),
            },
            projects: Vec::new(),
            escalation: None,
            audit: None,
            claims_only_verification: false,
        });
    };
    serde_yaml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

#[cfg(test)]
fn harness_verify_commands(paths: &MaestroPaths) -> Result<Vec<String>> {
    let config = harness_verify_config(paths)?;
    Ok(config.stack.verify)
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    shell
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::foundation::core::paths::MaestroPaths;

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn harness_verify_commands_round_trips_config_stack_verify_unchanged() {
        let temp = TestTempDir::new("maestro-proof-verify-commands-source");
        let harness_dir = temp.path().join(".maestro/harness");
        fs::create_dir_all(&harness_dir).expect("invariant: harness dir should be creatable");
        fs::write(
            harness_dir.join("harness.yml"),
            "schema_version: \"1\"\nstack:\n  kind: rust\n  detected_by:\n    - Cargo.toml\n  verify:\n    - cargo test\n    - cargo clippy --all-targets\n",
        )
        .expect("invariant: harness config should be writable");

        let paths = MaestroPaths::new(temp.path());
        let commands =
            super::harness_verify_commands(&paths).expect("invariant: harness config should read");

        assert_eq!(
            commands,
            vec![
                "cargo test".to_string(),
                "cargo clippy --all-targets".to_string(),
            ],
            "verify commands must be sourced verbatim from harness config stack.verify"
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_command_invokes_sh_dash_c_with_the_command_verbatim() {
        let command = "cargo test && echo done $(pwd)";
        let shell = super::shell_command(command);

        assert_eq!(
            shell.get_program().to_str(),
            Some("sh"),
            "verify commands must invoke the sh shell on unix"
        );
        let args: Vec<&str> = shell
            .get_args()
            .map(|arg| arg.to_str().expect("invariant: shell args should be UTF-8"))
            .collect();
        assert_eq!(
            args,
            vec!["-c", command],
            "verify commands must reach the shell as `-c <command>` with no interpolation"
        );
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "{prefix}-{}-{timestamp}-{counter}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
