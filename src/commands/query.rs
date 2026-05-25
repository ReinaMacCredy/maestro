use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::commands::{QueryArgs, QueryCommand};
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::schema::{BACKLOG_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION};
use crate::feature::schema::FeatureRegistry;
use crate::harness::schema::BacklogConfig;
use crate::task::blockers::has_unresolved_blockers;
use crate::task::doctor::load_task_records;
use crate::task::template::{TaskRecord, TaskState};
use crate::verification::proof_status::{proof_status, render_proof_status, ProofStatusKind};

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        QueryCommand::Proof { task_id } => {
            let status = proof_status(&paths, &task_id)?;
            print!("{}", render_proof_status(&status));
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
    let tasks = load_task_records(&paths.tasks_dir())?;
    let mut task_rows = tasks
        .iter()
        .map(|task| matrix_row(paths, task))
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
    let event_files = event_files_under(&paths.runs_dir())?;
    let mut events = 0_usize;
    let mut user_prompts = 0_usize;
    let mut corrections = 0_usize;
    let mut kinds = BTreeMap::<String, usize>::new();

    for path in &event_files {
        let file =
            fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line
                .with_context(|| format!("failed to read {} line {}", path.display(), index + 1))?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Value = serde_json::from_str(&line).with_context(|| {
                format!("failed to parse {} line {}", path.display(), index + 1)
            })?;
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
    for file_name in decisions {
        let path = paths.decisions_dir().join(&file_name);
        let title = decision_title(&path)?;
        let id = file_name.trim_end_matches(".md");
        println!("{id}\t{file_name}\t{title}");
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

fn matrix_row(paths: &MaestroPaths, task: &TaskRecord) -> Result<MatrixRow> {
    Ok(MatrixRow {
        feature_id: task
            .feature_id
            .clone()
            .unwrap_or_else(|| "<none>".to_string()),
        id: task.id.clone(),
        state: task_state_label(&task.state, has_unresolved_blockers(task)),
        proof: proof_label(paths, &task.id)?,
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

fn proof_label(paths: &MaestroPaths, task_id: &str) -> Result<&'static str> {
    match proof_status(paths, task_id)?.kind {
        ProofStatusKind::Accepted => Ok("accepted"),
        ProofStatusKind::Failed => Ok("failed"),
        ProofStatusKind::Missing => Ok("missing"),
        ProofStatusKind::Stale => Ok("stale"),
    }
}

fn task_state_label(state: &TaskState, blocked: bool) -> &'static str {
    if blocked {
        return "blocked";
    }
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

fn decision_entries(decisions_dir: &Path) -> Result<Vec<String>> {
    if !decisions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(decisions_dir)
        .with_context(|| format!("failed to read {}", decisions_dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", decisions_dir.display()))?;
        if !entry.path().is_file() {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if file_name.starts_with("decision-") && file_name.ends_with(".md") {
            entries.push(file_name);
        }
    }
    entries.sort();
    Ok(entries)
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

fn event_files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    files.retain(|path| path.file_name().and_then(|name| name.to_str()) == Some("events.jsonl"));
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
                let path = entry.path();
                if path.is_dir() {
                    collect_files(&path, files)?;
                } else if path.is_file() {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}

fn event_kind(event: &Value) -> String {
    string_field(event, "kind")
        .or_else(|| string_field(event, "event"))
        .or_else(|| string_field(event, "type"))
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn event_text(event: &Value) -> Option<String> {
    string_field(event, "message")
        .or_else(|| string_field(event, "prompt"))
        .or_else(|| string_field(event, "text"))
}

fn string_field(event: &Value, field: &str) -> Option<String> {
    event.get(field).and_then(Value::as_str).map(str::to_string)
}

fn looks_like_correction(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("actually")
        || lower.contains("wait")
        || lower.contains(" no ")
        || lower.starts_with("no ")
        || lower == "no"
}
