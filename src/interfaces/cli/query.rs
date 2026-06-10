use std::collections::BTreeMap;

use anyhow::{Result, bail};

use crate::decisions;
use crate::domain::feature;
use crate::domain::proof;
use crate::domain::run;
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::interfaces::cli::{QueryArgs, QueryCommand};
use crate::operations::harness;

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        QueryCommand::Proof {
            task_id,
            task_id_flag,
        } => {
            let explicit = match (task_id, task_id_flag) {
                (Some(positional), Some(flag)) if positional != flag => bail!(
                    "conflicting task ids: positional `{positional}` and --task-id `{flag}`; pass just one"
                ),
                (Some(id), _) | (None, Some(id)) => Some(id),
                (None, None) => None,
            };
            // Honor MAESTRO_CURRENT_TASK like the sibling read view `task show`
            // (strict: no single-task auto-detect), and name it in the remedy.
            let task_id = match explicit {
                Some(id) => id,
                None => match std::env::var("MAESTRO_CURRENT_TASK") {
                    Ok(id) if !id.trim().is_empty() => id,
                    _ => bail!(
                        "task id is required or set MAESTRO_CURRENT_TASK for `maestro query proof`"
                    ),
                },
            };
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
    let features = feature::list(paths)?;
    let current_commit = git::head(paths.repo_root()).unwrap_or(None);
    let entries = task::load_task_entries(&paths.tasks_dir())?;
    let mut task_rows = entries
        .iter()
        .map(|entry| matrix_row(&entry.task, current_commit.clone()))
        .collect::<Result<Vec<_>>>()?;
    task_rows.sort_by(|left, right| {
        left.feature_id
            .cmp(&right.feature_id)
            .then(left.id.cmp(&right.id))
    });

    if task_rows.is_empty() && features.is_empty() {
        println!("no features or tasks found");
        return Ok(());
    }

    let mut features_with_tasks = std::collections::HashSet::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in &task_rows {
        if row.feature_id != "<none>" {
            features_with_tasks.insert(row.feature_id.clone());
        }
        rows.push(vec![
            row.feature_id.clone(),
            row.id.clone(),
            row.state.to_string(),
            row.proof.to_string(),
            row.title.clone(),
        ]);
    }

    for view in features
        .iter()
        .filter(|view| !features_with_tasks.contains(&view.id))
    {
        rows.push(vec![
            view.id.clone(),
            "<none>".to_string(),
            "<none>".to_string(),
            "<none>".to_string(),
            view.title.clone(),
        ]);
    }
    print!(
        "{}",
        table::render_table(&["FEATURE", "TASK", "STATE", "PROOF", "TITLE"], &rows)
    );
    Ok(())
}

fn query_friction(paths: &MaestroPaths) -> Result<()> {
    let sessions = proof::managed_event_files(paths)?.len();
    let mut events = 0_usize;
    let mut user_prompts = 0_usize;
    let mut corrections = 0_usize;
    let mut kinds = BTreeMap::<String, usize>::new();

    run::visit_managed_events(paths, |record| {
        let event = record.event();
        events += 1;
        let kind = event
            .event_type()
            .or_else(|| event.alias_kind())
            .unwrap_or("<unknown>")
            .to_string();
        *kinds.entry(kind.clone()).or_default() += 1;
        if kind == "UserPromptSubmit" {
            user_prompts += 1;
            if harness::looks_like_correction(event.prompt_text().unwrap_or_default()) {
                corrections += 1;
            }
        }
        Ok(())
    })?;

    if events == 0 {
        println!("friction: no events found");
        return Ok(());
    }

    println!("FRICTION");
    println!("sessions: {sessions}");
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
    let entries = decisions::list(paths)?;
    if entries.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            vec![
                entry.id.clone(),
                entry.status.clone(),
                decision_home(&entry.source),
                entry.title.clone(),
            ]
        })
        .collect();
    print!(
        "{}",
        table::render_table(&["ID", "STATUS", "HOME", "TITLE"], &rows)
    );
    Ok(())
}

fn decision_home(source: &decisions::DecisionSource) -> String {
    match source {
        decisions::DecisionSource::Global => "global".to_string(),
        decisions::DecisionSource::Feature { feature_id } => format!("feature:{feature_id}"),
        decisions::DecisionSource::Legacy => "legacy-md".to_string(),
    }
}

fn query_backlog(paths: &MaestroPaths) -> Result<()> {
    let backlog = harness::load_backlog(paths)?;
    if backlog.items.is_empty() {
        println!("no backlog items found");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = backlog
        .items
        .iter()
        .map(|item| vec![item.id.clone(), item.title.clone()])
        .collect();
    print!("{}", table::render_table(&["ID", "TITLE"], &rows));
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

fn matrix_row(task: &task::TaskRecord, current_commit: Option<String>) -> Result<MatrixRow> {
    Ok(MatrixRow {
        feature_id: task
            .feature_id
            .clone()
            .unwrap_or_else(|| "<none>".to_string()),
        id: task.id.clone(),
        state: task_state_label(&task.state, task::has_unresolved_blockers(task)),
        proof: proof_label(task, current_commit)?,
        title: task.title.clone(),
    })
}

fn proof_label(task: &task::TaskRecord, current_commit: Option<String>) -> Result<&'static str> {
    Ok(proof::proof_status_kind_for_task(task, current_commit)?.label())
}

fn task_state_label(state: &task::TaskState, blocked: bool) -> &'static str {
    if blocked {
        return "blocked";
    }
    state.as_str()
}
