//! Task verification execution helpers.

use std::collections::BTreeSet;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::events::managed_event_files;
use super::stale::{FreshnessInputs, StoredFreshness};
use crate::domain::harness::HarnessConfig;
use crate::domain::task::{self, AcceptanceFile, TaskRecord, TaskState, VerificationBinding};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::git;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{EVENT_SCHEMA_VERSION, VERIFICATION_SCHEMA_VERSION};

/// High-level result returned by the Proof facade after task verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskVerification {
    pub task_id: String,
    pub status: TaskVerificationStatus,
    pub claim_count: usize,
    pub proof_source_count: usize,
    pub failures: Vec<String>,
}

/// High-level pass/fail status returned by the Proof facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskVerificationStatus {
    Passed,
    Failed,
}

/// Result status written to `verification.json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
}

/// Per-claim evidence matching result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClaimCheck {
    pub claim: String,
    pub matched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Proof source considered during verification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProofSource {
    pub kind: String,
    pub path: String,
    pub hash: String,
}

/// One verification command result captured from `harness.yml.verify`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationCommand {
    pub cmd: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

/// Verification artifact persisted in a task directory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationReport {
    pub schema_version: String,
    pub task_id: String,
    pub status: VerificationStatus,
    pub verified_at: String,
    #[serde(flatten)]
    pub freshness: StoredFreshness,
    pub claims: Vec<ClaimCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<VerificationCommand>,
    pub proof_sources: Vec<ProofSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
}

/// Paths and records for a loaded task.
#[derive(Clone, Debug)]
pub struct LoadedTask {
    pub task: TaskRecord,
    pub task_dir: PathBuf,
    handle: task::TaskHandle,
}

/// Execute proof validation for a task and return a facade-level result.
pub fn verify_task(paths: &MaestroPaths, task_id: &str, actor: &str) -> Result<TaskVerification> {
    let report = verify_task_report(paths, task_id, actor)?;
    Ok(TaskVerification::from_report(&report))
}

/// Execute proof validation for a task and persist `verification.json`.
pub fn verify_task_report(
    paths: &MaestroPaths,
    task_id: &str,
    actor: &str,
) -> Result<VerificationReport> {
    let now = timestamp();
    let mut loaded = load_task_by_id(paths, task_id)?;
    let inputs = freshness_inputs(paths, &loaded)?;
    let commands = run_verify_commands(paths)?;
    let claims = completion_claims(&loaded.task);
    let evidence = collect_evidence(paths, &loaded.task_dir, &loaded.task.id)?;
    let claim_checks = check_claims(&claims, &evidence);
    let failures = failures_for(&loaded.task, &claims, &claim_checks, &evidence, &commands);
    let status = if failures.is_empty() {
        VerificationStatus::Passed
    } else {
        VerificationStatus::Failed
    };

    let report = VerificationReport {
        schema_version: VERIFICATION_SCHEMA_VERSION.to_string(),
        task_id: loaded.task.id.clone(),
        status,
        verified_at: now.clone(),
        freshness: StoredFreshness {
            verified_commit: inputs.commit.clone(),
            task_contract_hash: inputs.task_contract_hash,
            acceptance_hash: inputs.acceptance_hash,
            checks_hash: inputs.checks_hash,
        },
        claims: claim_checks,
        commands,
        proof_sources: evidence
            .iter()
            .map(|source| ProofSource {
                kind: source.kind.clone(),
                path: display_path(paths.repo_root(), &source.path),
                hash: hash_bytes(source.text.as_bytes()),
            })
            .collect(),
        failures,
    };

    write_report(&loaded.task_dir, &report)?;
    apply_report_to_task(&mut loaded, &report, actor, &now)?;
    Ok(report)
}

impl TaskVerification {
    fn from_report(report: &VerificationReport) -> Self {
        Self {
            task_id: report.task_id.clone(),
            status: match report.status {
                VerificationStatus::Passed => TaskVerificationStatus::Passed,
                VerificationStatus::Failed => TaskVerificationStatus::Failed,
            },
            claim_count: report.claims.len(),
            proof_source_count: report.proof_sources.len(),
            failures: report.failures.clone(),
        }
    }
}

/// Load a task by id or id prefix directory name.
pub fn load_task_by_id(paths: &MaestroPaths, task_id: &str) -> Result<LoadedTask> {
    let handle = task::load_task_for_update(&paths.tasks_dir(), task_id)?;

    Ok(LoadedTask {
        task: handle.task().clone(),
        task_dir: handle.task_dir().to_path_buf(),
        handle,
    })
}

/// Return the path to the verification artifact for a loaded task.
pub fn verification_path(task_dir: &Path) -> PathBuf {
    task_dir.join("verification.json")
}

/// Read `verification.json` when it exists.
pub fn read_report(task_dir: &Path) -> Result<Option<VerificationReport>> {
    let path = verification_path(task_dir);
    let Some(raw) = read_to_string_if_exists(&path)? else {
        return Ok(None);
    };
    let report: VerificationReport = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if report.schema_version != VERIFICATION_SCHEMA_VERSION {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            VERIFICATION_SCHEMA_VERSION,
            report.schema_version
        );
    }
    Ok(Some(report))
}

/// Compute current proof freshness inputs for a loaded task.
pub fn freshness_inputs(paths: &MaestroPaths, loaded: &LoadedTask) -> Result<FreshnessInputs> {
    let commit = git::head(paths.repo_root()).unwrap_or(None);
    freshness_inputs_for_task(&loaded.task, &loaded.task_dir, commit)
}

/// Compute current proof freshness inputs for a task artifact directory.
pub fn freshness_inputs_for_task(
    task: &TaskRecord,
    task_dir: &Path,
    commit: Option<String>,
) -> Result<FreshnessInputs> {
    let acceptance = load_acceptance(task_dir)?;

    Ok(FreshnessInputs {
        commit,
        task_contract_hash: task_contract_hash(task),
        acceptance_hash: hash_file(&task_dir.join("acceptance.yaml"))?,
        checks_hash: checks_hash(&acceptance),
    })
}

fn apply_report_to_task(
    loaded: &mut LoadedTask,
    report: &VerificationReport,
    actor: &str,
    now: &str,
) -> Result<()> {
    let outcome = match report.status {
        VerificationStatus::Passed => task::VerificationOutcome::Passed(task::VerificationPassed {
            binding: VerificationBinding {
                verified_at: Some(report.verified_at.clone()),
                verified_commit: report.freshness.verified_commit.clone(),
                verified_by_run: report
                    .proof_sources
                    .iter()
                    .find(|source| source.kind == "event")
                    .map(|source| source.path.clone()),
                task_contract_hash: Some(report.freshness.task_contract_hash.clone()),
                acceptance_hash: Some(report.freshness.acceptance_hash.clone()),
                checks_hash: Some(report.freshness.checks_hash.clone()),
            },
            summary: report_summary(report),
        }),
        VerificationStatus::Failed => task::VerificationOutcome::Failed {
            summary: report_summary(report),
            failures: report.failures.clone(),
        },
    };

    task::apply_verification_outcome_to_handle(&mut loaded.handle, outcome, actor, now)?;
    loaded.task = loaded.handle.task().clone();
    Ok(())
}

fn report_summary(report: &VerificationReport) -> String {
    match report.status {
        VerificationStatus::Passed => format!(
            "verification passed: {} claim(s), {} proof source(s)",
            report.claims.len(),
            report.proof_sources.len()
        ),
        VerificationStatus::Failed => {
            let first = report
                .failures
                .first()
                .map(String::as_str)
                .unwrap_or("unknown verification failure");
            format!("verification failed: {first}")
        }
    }
}

fn write_report(task_dir: &Path, report: &VerificationReport) -> Result<()> {
    let path = verification_path(task_dir);
    let raw = serde_json::to_string_pretty(report)?;
    write_string_atomic(&path, &format!("{raw}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn failures_for(
    task: &TaskRecord,
    claims: &[String],
    claim_checks: &[ClaimCheck],
    evidence: &[EvidenceText],
    commands: &[VerificationCommand],
) -> Vec<String> {
    let mut failures = Vec::new();

    if task.state != TaskState::NeedsVerification && task.state != TaskState::Verified {
        failures.push(format!(
            "task is {}, expected needs_verification",
            state_name(&task.state)
        ));
    }
    if claims.is_empty() {
        failures.push("no completion claims found in task history".to_string());
    }
    if evidence.is_empty() {
        failures.push("missing proof: no task events or proof artifacts found".to_string());
    }

    for check in claim_checks.iter().filter(|check| !check.matched) {
        failures.push(format!("claim not backed by events/proof: {}", check.claim));
    }
    for command in commands.iter().filter(|command| command.exit_code != 0) {
        failures.push(format!(
            "verify command failed: {} (exit {})",
            command.cmd, command.exit_code
        ));
    }

    failures
}

fn run_verify_commands(paths: &MaestroPaths) -> Result<Vec<VerificationCommand>> {
    let commands = harness_verify_commands(paths)?;
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

fn harness_verify_commands(paths: &MaestroPaths) -> Result<Vec<String>> {
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
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    let Some(raw) = read_to_string_if_exists(&path)? else {
        return Ok(Vec::new());
    };
    let config: HarnessConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
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

fn check_claims(claims: &[String], evidence: &[EvidenceText]) -> Vec<ClaimCheck> {
    claims
        .iter()
        .map(|claim| {
            let normalized_claim = normalize_claim(claim);
            let source = evidence
                .iter()
                .find(|source| {
                    source
                        .claims
                        .iter()
                        .any(|candidate| normalize_claim(candidate) == normalized_claim)
                })
                .map(|source| source.path.display().to_string());
            ClaimCheck {
                claim: claim.clone(),
                matched: source.is_some(),
                source,
            }
        })
        .collect()
}

fn completion_claims(task: &TaskRecord) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut claims = Vec::new();
    for claim in task
        .state_history
        .iter()
        .flat_map(|entry| entry.claims.iter())
        .map(|claim| claim.trim())
        .filter(|claim| !claim.is_empty())
    {
        if seen.insert(claim.to_string()) {
            claims.push(claim.to_string());
        }
    }
    claims
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvidenceText {
    kind: String,
    path: PathBuf,
    text: String,
    claims: Vec<String>,
}

fn collect_evidence(
    paths: &MaestroPaths,
    task_dir: &Path,
    task_id: &str,
) -> Result<Vec<EvidenceText>> {
    let mut evidence = Vec::new();
    collect_task_artifact_text(task_dir, "evidence", &mut evidence)?;
    collect_task_artifact_text(task_dir, "proof", &mut evidence)?;
    collect_event_text(paths, task_id, &mut evidence)?;
    Ok(evidence)
}

fn collect_task_artifact_text(
    task_dir: &Path,
    dirname: &str,
    evidence: &mut Vec<EvidenceText>,
) -> Result<()> {
    let dir = task_dir.join(dirname);
    if !dir.is_dir() {
        return Ok(());
    }

    for path in text_files_under(&dir)? {
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let claims = proof_text_claims(&text);
        evidence.push(EvidenceText {
            kind: dirname.to_string(),
            path,
            text,
            claims,
        });
    }
    Ok(())
}

fn collect_event_text(
    paths: &MaestroPaths,
    task_id: &str,
    evidence: &mut Vec<EvidenceText>,
) -> Result<()> {
    for path in managed_event_files(paths)? {
        let mut matched = Vec::new();
        let mut claims = Vec::new();
        for line in event_lines(&path)? {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if event.get("task_id").and_then(Value::as_str) == Some(task_id)
                && is_proof_event(&event)
            {
                claims.extend(event_claims(&event));
                matched.push(line);
            }
        }
        if !matched.is_empty() {
            evidence.push(EvidenceText {
                kind: "event".to_string(),
                path,
                text: matched.join("\n"),
                claims,
            });
        }
    }
    Ok(())
}

fn is_proof_event(event: &Value) -> bool {
    matches!(event_kind(event), Some("proof" | "Proof" | "task_proof"))
        || is_phase4_tool_proof_event(event)
}

fn event_kind(event: &Value) -> Option<&str> {
    event
        .get("kind")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("type"))
        .and_then(Value::as_str)
}

fn event_claims(event: &Value) -> Vec<String> {
    let mut claims = Vec::new();
    if let Some(claim) = event.get("claim").and_then(Value::as_str) {
        claims.push(claim.to_string());
    }
    if let Some(message) = event.get("message").and_then(Value::as_str) {
        claims.push(message.to_string());
    }
    if let Some(values) = event.get("claims").and_then(Value::as_array) {
        claims.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if is_phase4_tool_proof_event(event) {
        claims.extend(phase4_tool_claims(event));
    }
    claims
}

fn is_phase4_tool_proof_event(event: &Value) -> bool {
    event.get("schema_version").and_then(Value::as_str) == Some(EVENT_SCHEMA_VERSION)
        && event.get("event_type").and_then(Value::as_str) == Some("PostToolUse")
        && event.get("status").and_then(Value::as_str) == Some("ok")
}

fn phase4_tool_claims(event: &Value) -> Vec<String> {
    let mut claims = Vec::new();
    let tool_name = event.get("tool_name").and_then(Value::as_str);
    let tool_input_hash = event.get("tool_input_hash").and_then(Value::as_str);

    if let (Some(tool_name), Some(tool_input_hash)) = (tool_name, tool_input_hash) {
        claims.push(format!("{tool_name} {tool_input_hash}"));
    }
    claims
}

fn proof_text_claims(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("claim:")
                .or_else(|| trimmed.strip_prefix("Claim:"))
                .map(str::trim)
                .filter(|claim| !claim.is_empty())
                .map(str::to_string)
        })
        .collect()
}

fn normalize_claim(claim: &str) -> String {
    claim.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn event_lines(path: &Path) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::to_string)
        .collect())
}

fn text_files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("failed to inspect {}", path.display()))?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    collect_files(&path, files)?;
                } else if file_type.is_file() {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}

fn load_acceptance(task_dir: &Path) -> Result<AcceptanceFile> {
    let path = task_dir.join("acceptance.yaml");
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

fn task_contract_hash(task: &TaskRecord) -> String {
    let claims = completion_claims(task);
    let contract = json!({
        "schema_version": task.schema_version,
        "id": task.id,
        "slug": task.slug,
        "feature_id": task.feature_id,
        "title": task.title,
        "task_type": task.task_type,
        "lane": task.lane,
        "risk": task.risk,
        "raw_request": task.raw_request,
        "input_type": task.input_type,
        "affected_areas": task.affected_areas,
        "open_questions": task.open_questions,
        "acceptance_locked": task.acceptance_locked,
        "claims": claims,
    });
    hash_bytes(contract.to_string().as_bytes())
}

fn checks_hash(acceptance: &AcceptanceFile) -> String {
    hash_bytes(
        serde_json::to_string(&acceptance.checks)
            .expect("invariant: acceptance checks should serialize")
            .as_bytes(),
    )
}

fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn state_name(state: &TaskState) -> &'static str {
    match state {
        TaskState::Draft => "draft",
        TaskState::Exploring => "exploring",
        TaskState::Ready => "ready",
        TaskState::InProgress => "in_progress",
        TaskState::NeedsVerification => "needs_verification",
        TaskState::Verified => "verified",
        TaskState::Rejected => "rejected",
        TaskState::Abandoned => "abandoned",
        TaskState::Superseded => "superseded",
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
