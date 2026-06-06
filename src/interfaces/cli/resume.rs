use std::env;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::domain::{decisions, feature, proof, task};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::ResumeArgs;

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
    let next = next_action_for(selected_task.as_ref(), selected_feature.as_ref());
    let required_reads = required_reads(paths, selected_task.as_ref(), selected_feature.as_ref());
    let guardrails = vec![
        "preserve unrelated dirty files".to_string(),
        "do not commit planning or notes artifacts unless asked".to_string(),
        "read required artifacts before acting".to_string(),
    ];
    let source_refs = source_refs(paths, selected_task.as_ref(), selected_feature.as_ref())?;
    let write_path = write_path(paths, selected_task.as_ref(), selected_feature.as_ref())?;
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
        blockers: blocker_lines(selected_task.as_ref()),
        next,
        required_reads,
        guardrails,
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
                format!("recover proof with maestro query proof {}", task.id)
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
                    "ship with maestro feature ship {} --outcome \"<outcome>\"",
                    feature.id
                )
            }
            feature::FeatureStatus::InProgress => format!(
                "inspect feature tasks with maestro feature show {}",
                feature.id
            ),
            feature::FeatureStatus::Shipped | feature::FeatureStatus::Cancelled => {
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
            reads.push(format!("maestro query proof {}", task.id));
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
    task: Option<&task::TaskRecord>,
    feature: Option<&feature::FeatureView>,
) -> Result<Vec<String>> {
    let mut refs = Vec::new();
    if let Some(task) = task {
        let task_yaml = task::task_yaml_path(&paths.tasks_dir(), &task.id)?;
        refs.push(display_repo_relative(paths, &task_yaml));
    }
    if let Some(feature) = feature {
        refs.push(display_repo_relative(
            paths,
            &paths.features_dir().join(&feature.id).join("feature.yaml"),
        ));
        refs.push(display_repo_relative(
            paths,
            &paths.features_dir().join(&feature.id).join("notes.md"),
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

fn render_resume_report(report: &ResumeReport) -> String {
    let mut out = String::new();
    writeln!(&mut out, "objective: {}", report.objective).unwrap();
    writeln!(&mut out, "state: {}", report.state).unwrap();
    if report.blockers.is_empty() {
        writeln!(&mut out, "blockers: none").unwrap();
    } else {
        writeln!(&mut out, "blockers:").unwrap();
        for blocker in &report.blockers {
            writeln!(&mut out, "  - {blocker}").unwrap();
        }
    }
    writeln!(&mut out, "next:").unwrap();
    writeln!(&mut out, "  {}", report.next).unwrap();
    write_list(&mut out, "required reads", &report.required_reads);
    write_list(&mut out, "guardrails", &report.guardrails);
    if let Some(full) = &report.full {
        write_list(&mut out, "prior decisions", &full.prior_decisions);
        write_list(&mut out, "last verified tasks", &full.last_verified_tasks);
        if let Some(proof) = &full.proof {
            writeln!(&mut out, "proof: {proof}").unwrap();
        }
        write_list(&mut out, "file scope", &full.file_scope);
        if let Some(prompt) = &full.prompt {
            writeln!(&mut out, "handoff prompt:").unwrap();
            writeln!(&mut out, "  {prompt}").unwrap();
        }
        write_list(&mut out, "source references", &report.source_refs);
    }
    out
}

fn write_list(out: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    writeln!(out, "{label}:").unwrap();
    for value in values {
        writeln!(out, "  - {value}").unwrap();
    }
}

fn resume_write(report: &ResumeReport) -> Result<ResumeWrite> {
    let path = report.write_path.clone();
    let mut contents = String::new();
    writeln!(&mut contents, "# Maestro Resume").unwrap();
    writeln!(&mut contents).unwrap();
    writeln!(&mut contents, "generated_at: {}", utc_now_timestamp()).unwrap();
    writeln!(&mut contents, "mode: {}", report.mode.as_str()).unwrap();
    writeln!(&mut contents).unwrap();
    contents.push_str(&render_resume_report(report));
    if report.full.is_none() {
        write_list(&mut contents, "source references", &report.source_refs);
    }
    Ok(ResumeWrite { path, contents })
}

fn write_path(
    paths: &MaestroPaths,
    task: Option<&task::TaskRecord>,
    feature: Option<&feature::FeatureView>,
) -> Result<PathBuf> {
    if let Some(task) = task {
        let task_yaml = task::task_yaml_path(&paths.tasks_dir(), &task.id)?;
        return Ok(task_yaml
            .parent()
            .context("task.yaml path is missing parent directory")?
            .join("resume.md"));
    }
    if let Some(feature) = feature {
        return Ok(paths.features_dir().join(&feature.id).join("resume.md"));
    }
    Ok(paths.maestro_dir().join("resume.md"))
}

fn display_repo_relative(paths: &MaestroPaths, path: &Path) -> String {
    path.strip_prefix(paths.repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
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
    blockers: Vec<String>,
    next: String,
    required_reads: Vec<String>,
    guardrails: Vec<String>,
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
