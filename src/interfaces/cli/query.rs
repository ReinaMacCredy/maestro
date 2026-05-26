use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::decisions::query::{decision_entries, decision_id};
use crate::domain::proof;
use crate::domain::task;
use crate::feature::schema::FeatureRegistry;
use crate::foundation::core::git;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::foundation::core::schema::{BACKLOG_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION};
use crate::harness::schema::BacklogConfig;
use crate::interfaces::cli::{QueryArgs, QueryCommand};
use crate::metrics::friction::{event_kind, event_text, looks_like_correction};

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        QueryCommand::Proof {
            task_id,
            task_id_flag,
        } => {
            let task_id = task_id
                .or(task_id_flag)
                .context("task id is required for `maestro query proof`")?;
            let status = proof::proof_status(&paths, &task_id)?;
            print!("{}", proof::render_proof_status(&status));
            Ok(())
        }
        QueryCommand::Matrix => query_matrix(&paths),
        QueryCommand::Friction => query_friction(&paths),
        QueryCommand::Decisions => query_decisions(&paths),
        QueryCommand::Backlog => query_backlog(&paths),
    }
}

fn query_matrix(paths: &MaestroPaths) -> Result<()> {
    let registry = load_feature_registry(paths)?;
    let current_commit = git::head(paths.repo_root()).unwrap_or(None);
    let entries = task::load_task_entries(&paths.tasks_dir())?;
    let mut task_rows = entries
        .iter()
        .map(|entry| matrix_row(paths, &entry.task, &entry.task_dir, current_commit.clone()))
        .collect::<Result<Vec<_>>>()?;
    task_rows.sort_by(|left, right| {
        left.feature_id
            .cmp(&right.feature_id)
            .then(left.id.cmp(&right.id))
    });

    println!("FEATURE\tTASK\tSTATE\tPROOF\tTITLE");
    let mut features_with_tasks = std::collections::HashSet::new();
    for row in &task_rows {
        if row.feature_id != "<none>" {
            features_with_tasks.insert(row.feature_id.clone());
        }
        println!(
            "{}\t{}\t{}\t{}\t{}",
            row.feature_id, row.id, row.state, row.proof, row.title
        );
    }

    for feature in registry
        .features
        .iter()
        .filter(|feature| !features_with_tasks.contains(&feature.id))
    {
        println!("{}\t<none>\t<none>\t<none>\t{}", feature.id, feature.title);
    }
    Ok(())
}

fn query_friction(paths: &MaestroPaths) -> Result<()> {
    let event_files = proof::managed_event_files(paths)?;
    let mut events = 0_usize;
    let mut user_prompts = 0_usize;
    let mut corrections = 0_usize;
    let mut kinds = BTreeMap::<String, usize>::new();

    for path in &event_files {
        for line in event_lines(path)? {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            events += 1;
            let kind = event_kind(&event);
            *kinds.entry(kind.clone()).or_default() += 1;
            if kind == "UserPromptSubmit" {
                user_prompts += 1;
                if looks_like_correction(event_text(&event).as_deref().unwrap_or_default()) {
                    corrections += 1;
                }
            }
        }
    }

    if events == 0 {
        println!("friction: no events found");
        return Ok(());
    }

    println!("FRICTION");
    println!("sessions: {}", event_files.len());
    println!("events: {events}");
    println!("user_prompts: {user_prompts}");
    println!("corrections: {corrections}");
    println!("event_kinds:");
    for (kind, count) in kinds {
        println!("- {kind}: {count}");
    }
    Ok(())
}

fn query_decisions(paths: &MaestroPaths) -> Result<()> {
    let decisions = decision_entries(&paths.decisions_dir())?;
    if decisions.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    println!("ID\tFILE\tTITLE");
    for entry in decisions {
        let title = decision_title(&entry.path)?;
        println!(
            "{}\t{}\t{}",
            decision_id(&entry.file_name),
            entry.file_name,
            title
        );
    }
    Ok(())
}

fn query_backlog(paths: &MaestroPaths) -> Result<()> {
    let path = paths.harness_dir().join("backlog.yaml");
    let backlog = load_backlog(&path)?;
    if backlog.items.is_empty() {
        println!("no backlog items found");
        return Ok(());
    }

    println!("ID\tTITLE");
    for item in backlog.items {
        println!("{}\t{}", item.id, item.title);
    }
    Ok(())
}

#[derive(Debug)]
struct MatrixRow {
    feature_id: String,
    id: String,
    state: &'static str,
    proof: &'static str,
    title: String,
}

fn matrix_row(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<MatrixRow> {
    Ok(MatrixRow {
        feature_id: task
            .feature_id
            .clone()
            .unwrap_or_else(|| "<none>".to_string()),
        id: task.id.clone(),
        state: task_state_label(&task.state, task::has_unresolved_blockers(task)),
        proof: proof_label(paths, task, task_dir, current_commit)?,
        title: task.title.clone(),
    })
}

fn load_feature_registry(paths: &MaestroPaths) -> Result<FeatureRegistry> {
    let path = paths.features_dir().join("features.yaml");
    if !path.is_file() {
        return Ok(FeatureRegistry::empty());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let registry: FeatureRegistry = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if registry.schema_version != FEATURE_SCHEMA_VERSION {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            FEATURE_SCHEMA_VERSION,
            registry.schema_version
        );
    }
    Ok(registry)
}

fn load_backlog(path: &Path) -> Result<BacklogConfig> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let backlog: BacklogConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if backlog.schema_version != BACKLOG_SCHEMA_VERSION {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            BACKLOG_SCHEMA_VERSION,
            backlog.schema_version
        );
    }
    Ok(backlog)
}

fn proof_label(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    task_dir: &Path,
    current_commit: Option<String>,
) -> Result<&'static str> {
    Ok(proof::proof_status_kind_for_task(paths, task, task_dir, current_commit)?.label())
}

fn task_state_label(state: &task::TaskState, blocked: bool) -> &'static str {
    if blocked {
        return "blocked";
    }
    match state {
        task::TaskState::Draft => "draft",
        task::TaskState::Exploring => "exploring",
        task::TaskState::Ready => "ready",
        task::TaskState::InProgress => "in_progress",
        task::TaskState::NeedsVerification => "needs_verification",
        task::TaskState::Verified => "verified",
        task::TaskState::Rejected => "rejected",
        task::TaskState::Abandoned => "abandoned",
        task::TaskState::Superseded => "superseded",
    }
}

fn decision_title(path: &Path) -> Result<String> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let title = raw
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .and_then(|heading| heading.split_once(": ").map(|(_, title)| title.to_string()))
        .unwrap_or_else(|| "<untitled>".to_string());
    Ok(title)
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
