use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::domain::{card, decisions, feature, proof, task};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{
    GitReadout, ResumeArgs, clean_worktree_note, git_readout, proof_concern_line, render_git_line,
};
use crate::operations::memory::{
    self, ApprovedMemory, MemoryReadScope, MemoryReadSurface, MemorySuggestionHint,
};

const RESUME_SCHEMA: &str = "maestro.resume.v1";

pub fn run(args: ResumeArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    if !paths.maestro_dir().is_dir() {
        bail!("maestro is not initialized in this repo; run `maestro init --yes`");
    }

    let mode = ResumeMode::from_args(&args);
    let report = build_resume_report(&paths, &args, mode)?;
    let written_path = if args.write {
        let write = resume_write(&report)?;
        write_string_atomic(&write.path, &write.contents)
            .with_context(|| format!("failed to write {}", write.path.display()))?;
        Some(display_repo_relative(&paths, &write.path))
    } else {
        None
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render_resume_report(&report));
    }

    if let Some(path) = written_path {
        if args.json {
            eprintln!("wrote: {path}");
        } else {
            println!("wrote: {path}");
        }
    }

    Ok(())
}

fn build_resume_report(
    paths: &MaestroPaths,
    args: &ResumeArgs,
    mode: ResumeMode,
) -> Result<ResumeReport> {
    let selected_task = select_task(paths, args)?;
    let selected_feature = select_feature(paths, args, selected_task.as_ref())?;
    let selected_task_yaml = selected_task
        .as_ref()
        .map(|task| task::task_yaml_path(&paths.tasks_dir(), &task.id))
        .transpose()?;
    let next = next_action_for(selected_task.as_ref(), selected_feature.as_ref());
    let proof_concern = selected_task
        .as_ref()
        .and_then(|task| proof_concern_line(paths, task));
    let git = git_readout(paths);
    let close_or_verify_pending = verb_is_close_or_verify_shaped(
        selected_task.as_ref().map(|task| &task.state),
        selected_task
            .as_ref()
            .is_some_and(task::has_unresolved_blockers),
        selected_feature.as_ref().map(|feature| {
            (
                &feature.status,
                feature.counts.total,
                feature.counts.verified,
            )
        }),
    );
    let required_reads = required_reads(paths, selected_task.as_ref(), selected_feature.as_ref());
    let approved_memory = memory::approved_memory(
        paths,
        MemoryReadSurface::Resume,
        MemoryReadScope {
            task_id: selected_task.as_ref().map(|task| task.id.clone()),
            feature_id: selected_feature.as_ref().map(|feature| feature.id.clone()),
            ..MemoryReadScope::default()
        },
    )?;
    let memory_suggestions = memory::suggestion_hints(
        paths,
        MemoryReadSurface::Resume,
        MemoryReadScope {
            task_id: selected_task.as_ref().map(|task| task.id.clone()),
            feature_id: selected_feature.as_ref().map(|feature| feature.id.clone()),
            ..MemoryReadScope::default()
        },
    )?;
    let guardrails = vec![
        "preserve unrelated dirty files".to_string(),
        "do not commit planning or notes artifacts unless asked".to_string(),
        "read required artifacts before acting".to_string(),
    ];
    let source_refs = source_refs(
        paths,
        selected_task_yaml.as_deref(),
        selected_feature.as_ref(),
    )?;
    let write_path = write_path(
        paths,
        selected_task_yaml.as_deref(),
        selected_feature.as_ref(),
    )?;
    let full = if mode != ResumeMode::Compact {
        let tasks = task::load_task_records(&paths.tasks_dir())?;
        Some(full_context(
            paths,
            &tasks,
            selected_task.as_ref(),
            selected_feature.as_ref(),
            mode,
        )?)
    } else {
        None
    };

    Ok(ResumeReport {
        schema: RESUME_SCHEMA.to_string(),
        mode,
        repo: paths.repo_root().display().to_string(),
        objective: objective(selected_task.as_ref(), selected_feature.as_ref()),
        state: state_label(selected_task.as_ref(), selected_feature.as_ref()),
        git,
        close_or_verify_pending,
        blockers: blocker_lines(selected_task.as_ref()),
        next,
        proof_concern,
        required_reads,
        guardrails,
        approved_memory: approved_memory.memories,
        approved_memory_omitted: approved_memory.omitted,
        memory_suggestions: memory_suggestions.suggestions,
        memory_suggestions_omitted: memory_suggestions.omitted,
        memory: memory_lines(paths),
        source_refs,
        write_path,
        full,
    })
}

fn select_task(paths: &MaestroPaths, args: &ResumeArgs) -> Result<Option<task::TaskRecord>> {
    if let Some(id) = args.task.as_deref() {
        return task::load_task_record(&paths.tasks_dir(), id).map(Some);
    }

    if let Some(feature_id) = args.feature.as_deref() {
        let tasks = task::load_task_records(&paths.tasks_dir())?;
        return Ok(choose_next_task(
            tasks
                .iter()
                .filter(|task| task.feature_id.as_deref() == Some(feature_id)),
        )
        .cloned());
    }

    if let Ok(id) = env::var("MAESTRO_CURRENT_TASK")
        && !id.trim().is_empty()
        && let Ok(task) = task::load_task_record(&paths.tasks_dir(), &id)
        && task.state.is_live()
    {
        return Ok(Some(task));
    }

    let tasks = task::load_task_records(&paths.tasks_dir())?;
    Ok(choose_next_task(tasks.iter()).cloned())
}

fn choose_next_task<'a>(
    tasks: impl Iterator<Item = &'a task::TaskRecord>,
) -> Option<&'a task::TaskRecord> {
    let live = tasks
        .filter(|task| task.state.is_live())
        .collect::<Vec<_>>();
    for state in [
        task::TaskState::NeedsVerification,
        task::TaskState::Ready,
        task::TaskState::InProgress,
        task::TaskState::Draft,
        task::TaskState::Exploring,
    ] {
        if let Some(task) = live.iter().copied().find(|task| task.state == state) {
            return Some(task);
        }
    }
    None
}

fn select_feature(
    paths: &MaestroPaths,
    args: &ResumeArgs,
    task: Option<&task::TaskRecord>,
) -> Result<Option<feature::FeatureView>> {
    if let Some(id) = args.feature.as_deref() {
        return feature::show(paths, id).map(Some);
    }
    let Some(feature_id) = task.and_then(|task| task.feature_id.as_deref()) else {
        return Ok(None);
    };
    feature::show(paths, feature_id).map(Some)
}

fn objective(task: Option<&task::TaskRecord>, feature: Option<&feature::FeatureView>) -> String {
    match (task, feature) {
        (Some(task), Some(feature)) => {
            format!("continue task {} for feature {}", task.id, feature.id)
        }
        (Some(task), None) => format!("continue task {}", task.id),
        (None, Some(feature)) => format!("continue feature {}", feature.id),
        (None, None) => "inspect repo status".to_string(),
    }
}

fn state_label(task: Option<&task::TaskRecord>, feature: Option<&feature::FeatureView>) -> String {
    match (task, feature) {
        (Some(task), _) => task.state.as_str().to_string(),
        (None, Some(feature)) => feature::status_label(&feature.status).to_string(),
        (None, None) => "no_action".to_string(),
    }
}

fn blocker_lines(task: Option<&task::TaskRecord>) -> Vec<String> {
    task.map(|task| {
        task.blockers
            .iter()
            .filter(|blocker| blocker.resolved_at.is_none())
            .map(blocker_line)
            .collect()
    })
    .unwrap_or_default()
}

fn blocker_line(blocker: &task::Blocker) -> String {
    format!("{}: {}", blocker.id, blocker.reason)
}

fn next_action_for(
    task: Option<&task::TaskRecord>,
    feature: Option<&feature::FeatureView>,
) -> String {
    if let Some(task) = task {
        if task::has_unresolved_blockers(task) {
            return format!("inspect blockers with maestro task show {}", task.id);
        }
        return match task.state {
            task::TaskState::Draft => format!(
                "author checks or explore with maestro task show {}",
                task.id
            ),
            task::TaskState::Exploring => {
                format!("lock acceptance with maestro task accept {}", task.id)
            }
            task::TaskState::Ready => format!("claim with maestro task claim {}", task.id),
            task::TaskState::InProgress => format!(
                "run focused gate, then maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"",
                task.id
            ),
            task::TaskState::NeedsVerification => {
                format!("recover proof with maestro task verify {}", task.id)
            }
            task::TaskState::Verified
            | task::TaskState::Rejected
            | task::TaskState::Abandoned
            | task::TaskState::Superseded => "inspect repo status with maestro status".to_string(),
        };
    }

    if let Some(feature) = feature {
        return match feature.status {
            feature::FeatureStatus::Proposed => {
                format!("author contract with maestro feature show {}", feature.id)
            }
            feature::FeatureStatus::Ready => format!(
                "prepare implementation queue with maestro feature prepare {} --draft",
                feature.id
            ),
            feature::FeatureStatus::InProgress
                if feature.counts.total > 0 && feature.counts.total == feature.counts.verified =>
            {
                format!(
                    "close with maestro feature close {} --outcome \"<outcome>\"",
                    feature.id
                )
            }
            feature::FeatureStatus::InProgress => format!(
                "inspect feature tasks with maestro feature show {}",
                feature.id
            ),
            feature::FeatureStatus::Closed | feature::FeatureStatus::Cancelled => {
                "inspect repo status with maestro status".to_string()
            }
        };
    }

    "inspect repo status with maestro status".to_string()
}

fn required_reads(
    paths: &MaestroPaths,
    task: Option<&task::TaskRecord>,
    feature: Option<&feature::FeatureView>,
) -> Vec<String> {
    let mut reads = Vec::new();
    if let Some(task) = task {
        reads.push(format!("maestro task show {}", task.id));
        if task.state == task::TaskState::NeedsVerification {
            reads.push(format!("maestro task proof {}", task.id));
        }
    }
    if let Some(feature) = feature {
        reads.push(format!("maestro feature show {}", feature.id));
    }
    if let Some(notes) =
        implementation_notes_read(paths, feature.map(|feature| feature.id.as_str()))
    {
        reads.push(notes);
    }
    if reads.is_empty() {
        reads.push("maestro status".to_string());
    }
    reads
}

fn implementation_notes_read(paths: &MaestroPaths, feature_id: Option<&str>) -> Option<String> {
    if let Some(feature_id) = feature_id {
        let task_notes = format!("IMPLEMENTATION_NOTES-{feature_id}.md");
        if paths.repo_root().join(&task_notes).is_file() {
            return Some(task_notes);
        }
    }
    paths
        .repo_root()
        .join("IMPLEMENTATION_NOTES.md")
        .is_file()
        .then(|| "IMPLEMENTATION_NOTES.md".to_string())
}

fn source_refs(
    paths: &MaestroPaths,
    task_yaml: Option<&Path>,
    feature: Option<&feature::FeatureView>,
) -> Result<Vec<String>> {
    let mut refs = Vec::new();
    if let Some(task_yaml) = task_yaml {
        refs.push(display_repo_relative(paths, task_yaml));
    }
    if let Some(feature) = feature {
        let card_yaml = card::store::card_path(paths, &feature.id);
        refs.push(display_repo_relative(paths, &card_yaml));
        refs.push(display_repo_relative(
            paths,
            &card_yaml.with_file_name("notes.md"),
        ));
    }
    Ok(refs)
}

fn full_context(
    paths: &MaestroPaths,
    tasks: &[task::TaskRecord],
    task: Option<&task::TaskRecord>,
    feature: Option<&feature::FeatureView>,
    mode: ResumeMode,
) -> Result<ResumeFullContext> {
    let proof = task
        .map(|task| proof_summary(paths, &task.id))
        .transpose()?;
    Ok(ResumeFullContext {
        prior_decisions: prior_decisions(paths)?,
        last_verified_tasks: last_verified_tasks(tasks, feature.map(|feature| feature.id.as_str())),
        proof,
        file_scope: feature
            .map(|feature| feature.affected_areas.clone())
            .unwrap_or_default(),
        prompt: (mode == ResumeMode::Handoff).then(|| handoff_prompt(task, feature)),
    })
}

fn prior_decisions(paths: &MaestroPaths) -> Result<Vec<String>> {
    let decisions = decisions::list(paths)?
        .into_iter()
        .rev()
        .take(5)
        .map(|entry| format!("{}: {}", entry.id, entry.title))
        .collect::<Vec<_>>();
    Ok(decisions)
}

fn last_verified_tasks(tasks: &[task::TaskRecord], feature_id: Option<&str>) -> Vec<String> {
    tasks
        .iter()
        .filter(|task| task.state == task::TaskState::Verified)
        .filter(|task| {
            feature_id.is_none_or(|feature_id| task.feature_id.as_deref() == Some(feature_id))
        })
        .rev()
        .take(3)
        .map(|task| format!("{}: {}", task.id, task.title))
        .collect()
}

fn proof_summary(paths: &MaestroPaths, task_id: &str) -> Result<String> {
    let status = proof::proof_status(paths, task_id)?;
    Ok(format!("{} proof: {}", task_id, status.kind.label()))
}

fn handoff_prompt(
    task: Option<&task::TaskRecord>,
    feature: Option<&feature::FeatureView>,
) -> String {
    format!(
        "Continue Maestro work from repo-local state. Start by running the required reads for {} before editing.",
        objective(task, feature)
    )
}

/// The last few archive lid lines (SPEC-archive-memory-2 R3): one file read,
/// newest last, bullet prefix stripped for the renderer. No INDEX.md (nothing
/// archived yet) is an empty section, never an error.
fn memory_lines(paths: &MaestroPaths) -> Vec<String> {
    let Ok(contents) = fs::read_to_string(paths.archive_index_file()) else {
        return Vec::new();
    };
    let lid: Vec<&str> = contents
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect();
    lid.iter()
        .rev()
        .take(3)
        .rev()
        .map(|line| line.trim_start_matches("- ").to_string())
        .collect()
}

fn render_resume_report(report: &ResumeReport) -> String {
    let mut out = String::new();
    push_line(&mut out, format!("objective: {}", report.objective));
    push_line(&mut out, format!("state: {}", report.state));
    if let Some(git) = &report.git {
        push_line(&mut out, render_git_line(git));
        if report.close_or_verify_pending && git.code_other_dirty > 0 {
            push_line(&mut out, clean_worktree_note(git.code_other_dirty));
        }
    }
    if report.blockers.is_empty() {
        push_line(&mut out, "blockers: none");
    } else {
        push_line(&mut out, "blockers:");
        for blocker in &report.blockers {
            push_line(&mut out, format!("  - {blocker}"));
        }
    }
    push_line(&mut out, "next:");
    push_line(&mut out, format!("  {}", report.next));
    if let Some(concern) = &report.proof_concern {
        push_line(&mut out, concern.clone());
    }
    write_list(&mut out, "required reads", &report.required_reads);
    write_list(&mut out, "guardrails", &report.guardrails);
    if !report.memory.is_empty() {
        write_list(&mut out, "memory", &report.memory);
        push_line(&mut out, "  full lid: .maestro/archive/cards/INDEX.md");
    }
    if !report.approved_memory.is_empty() {
        write_approved_memory(
            &mut out,
            &report.approved_memory,
            report.approved_memory_omitted,
        );
    }
    if !report.memory_suggestions.is_empty() {
        write_memory_suggestions(
            &mut out,
            &report.memory_suggestions,
            report.memory_suggestions_omitted,
        );
    }
    if let Some(full) = &report.full {
        write_list(&mut out, "prior decisions", &full.prior_decisions);
        write_list(&mut out, "last verified tasks", &full.last_verified_tasks);
        if let Some(proof) = &full.proof {
            push_line(&mut out, format!("proof: {proof}"));
        }
        write_list(&mut out, "file scope", &full.file_scope);
        if let Some(prompt) = &full.prompt {
            push_line(&mut out, "handoff prompt:");
            push_line(&mut out, format!("  {prompt}"));
        }
        write_list(&mut out, "source references", &report.source_refs);
    }
    out
}

fn write_list(out: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    push_line(out, format!("{label}:"));
    for value in values {
        push_line(out, format!("  - {value}"));
    }
}

fn write_approved_memory(out: &mut String, memories: &[ApprovedMemory], omitted: usize) {
    push_line(out, "approved memory:");
    for memory in memories {
        push_line(
            out,
            format!(
                "  - {} scope={} risk={} {}",
                memory.id,
                memory.scope_kind.as_str(),
                memory.risk.as_str(),
                memory.summary
            ),
        );
        push_line(out, format!("    show: {}", memory.show_command));
    }
    if omitted > 0 {
        push_line(
            out,
            format!("  - {omitted} omitted; search with `maestro memory search <query>`"),
        );
    }
}

fn write_memory_suggestions(
    out: &mut String,
    suggestions: &[MemorySuggestionHint],
    omitted: usize,
) {
    push_line(out, "memory suggestions:");
    for suggestion in suggestions {
        push_line(
            out,
            format!(
                "  - {} sources={} {}",
                suggestion.id, suggestion.source_count, suggestion.summary
            ),
        );
        push_line(out, format!("    create: {}", suggestion.create_command));
        push_line(out, format!("    dismiss: {}", suggestion.dismiss_command));
    }
    if omitted > 0 {
        push_line(
            out,
            format!("  - {omitted} omitted; inspect with `maestro memory suggest list --all`"),
        );
    }
}

fn resume_write(report: &ResumeReport) -> Result<ResumeWrite> {
    let path = report.write_path.clone();
    let mut contents = String::new();
    push_line(&mut contents, "# Maestro Resume");
    push_blank_line(&mut contents);
    push_line(
        &mut contents,
        format!("generated_at: {}", utc_now_timestamp()),
    );
    push_line(&mut contents, format!("mode: {}", report.mode.as_str()));
    push_blank_line(&mut contents);
    contents.push_str(&render_resume_report(report));
    if report.full.is_none() {
        write_list(&mut contents, "source references", &report.source_refs);
    }
    Ok(ResumeWrite { path, contents })
}

fn push_line(out: &mut String, line: impl AsRef<str>) {
    out.push_str(line.as_ref());
    out.push('\n');
}

fn push_blank_line(out: &mut String) {
    out.push('\n');
}

fn write_path(
    paths: &MaestroPaths,
    task_yaml: Option<&Path>,
    feature: Option<&feature::FeatureView>,
) -> Result<PathBuf> {
    if let Some(task_yaml) = task_yaml {
        return Ok(task_yaml
            .parent()
            .context("task.yaml path is missing parent directory")?
            .join("resume.md"));
    }
    if let Some(feature) = feature {
        return Ok(feature::feature_sidecar_dir(paths, &feature.id).join("resume.md"));
    }
    Ok(paths.maestro_dir().join("resume.md"))
}

fn display_repo_relative(paths: &MaestroPaths, path: &Path) -> String {
    path.strip_prefix(paths.repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// Whether the next verb resume would recommend is close/verify-shaped (the
/// states where uncommitted code matters for the proof or close). Mirrors the
/// close/verify arms of `next_action_for`; kept over primitive inputs so it is
/// value-level unit-testable without constructing a full `TaskRecord`.
fn verb_is_close_or_verify_shaped(
    task_state: Option<&task::TaskState>,
    task_has_blockers: bool,
    feature: Option<(&feature::FeatureStatus, usize, usize)>,
) -> bool {
    if let Some(state) = task_state {
        if task_has_blockers {
            return false;
        }
        return matches!(
            state,
            task::TaskState::InProgress | task::TaskState::NeedsVerification
        );
    }
    if let Some((status, total, verified)) = feature {
        return *status == feature::FeatureStatus::InProgress && total > 0 && total == verified;
    }
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResumeMode {
    Compact,
    Full,
    Handoff,
}

impl ResumeMode {
    fn from_args(args: &ResumeArgs) -> Self {
        if args.handoff {
            Self::Handoff
        } else if args.full {
            Self::Full
        } else {
            Self::Compact
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Full => "full",
            Self::Handoff => "handoff",
        }
    }
}

#[derive(Debug, Serialize)]
struct ResumeReport {
    schema: String,
    mode: ResumeMode,
    repo: String,
    objective: String,
    state: String,
    /// Working-tree git readout; `None` when the repo is not a git repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitReadout>,
    /// Whether the next verb is close/verify-shaped; drives the clean-worktree
    /// note. Render-only, not part of the serialized contract.
    #[serde(skip)]
    close_or_verify_pending: bool,
    blockers: Vec<String>,
    next: String,
    /// Concern-only proof repair line for the focal task; render-only, not part
    /// of the serialized contract. `None` when the proof needs no action.
    #[serde(skip)]
    proof_concern: Option<String>,
    required_reads: Vec<String>,
    guardrails: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    approved_memory: Vec<ApprovedMemory>,
    #[serde(skip_serializing_if = "is_zero")]
    approved_memory_omitted: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    memory_suggestions: Vec<MemorySuggestionHint>,
    #[serde(skip_serializing_if = "is_zero")]
    memory_suggestions_omitted: usize,
    /// Recent archive lid lines (SPEC-archive-memory-2 R3); empty when
    /// nothing has been archived.
    memory: Vec<String>,
    source_refs: Vec<String>,
    #[serde(skip_serializing)]
    write_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    full: Option<ResumeFullContext>,
}

#[derive(Debug, Serialize)]
struct ResumeFullContext {
    prior_decisions: Vec<String>,
    last_verified_tasks: Vec<String>,
    proof: Option<String>,
    file_scope: Vec<String>,
    prompt: Option<String>,
}

struct ResumeWrite {
    path: PathBuf,
    contents: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_or_verify_shaped_covers_only_proof_and_close_states() {
        // Task-selected arms: only InProgress (next: task complete) and
        // NeedsVerification (next: proof recovery) are close/verify-shaped.
        assert!(verb_is_close_or_verify_shaped(
            Some(&task::TaskState::InProgress),
            false,
            None
        ));
        assert!(verb_is_close_or_verify_shaped(
            Some(&task::TaskState::NeedsVerification),
            false,
            None
        ));
        for state in [
            task::TaskState::Draft,
            task::TaskState::Exploring,
            task::TaskState::Ready,
            task::TaskState::Verified,
        ] {
            assert!(
                !verb_is_close_or_verify_shaped(Some(&state), false, None),
                "state {state:?} should not be close/verify-shaped"
            );
        }
        // Unresolved blockers redirect the next verb to "inspect blockers".
        assert!(!verb_is_close_or_verify_shaped(
            Some(&task::TaskState::InProgress),
            true,
            None
        ));
        // Feature-only arm (reachable via `maestro resume --feature`): close is
        // shaped only when every counted task is verified.
        assert!(verb_is_close_or_verify_shaped(
            None,
            false,
            Some((&feature::FeatureStatus::InProgress, 3, 3))
        ));
        assert!(!verb_is_close_or_verify_shaped(
            None,
            false,
            Some((&feature::FeatureStatus::InProgress, 3, 2))
        ));
        assert!(!verb_is_close_or_verify_shaped(
            None,
            false,
            Some((&feature::FeatureStatus::InProgress, 0, 0))
        ));
        assert!(!verb_is_close_or_verify_shaped(
            None,
            false,
            Some((&feature::FeatureStatus::Ready, 0, 0))
        ));
        // Nothing selected: next verb is "inspect repo status".
        assert!(!verb_is_close_or_verify_shaped(None, false, None));
    }
}
