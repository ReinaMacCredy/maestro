use std::env;
use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::domain::feature::{self, FeatureStatus};
use crate::domain::task::{self, TaskRecord, TaskState};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::StatusArgs;

pub fn run(args: StatusArgs) -> Result<()> {
    let repo_root = match discover_repo_root() {
        Ok(repo_root) => repo_root,
        Err(_) => {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let report = StatusReport::not_initialized(cwd, "repo root not found".to_string());
            return print_status(report, args.json);
        }
    };
    let paths = MaestroPaths::new(repo_root);
    if !paths.maestro_dir().is_dir() {
        let report = StatusReport::not_initialized(
            paths.repo_root().to_path_buf(),
            ".maestro is missing".to_string(),
        );
        return print_status(report, args.json);
    }

    let report = build_status_report(&paths)?;
    print_status(report, args.json)
}

pub fn run_task_next(paths: &MaestroPaths, json: bool) -> Result<()> {
    let report = build_status_report(paths)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&TaskNextJson::from(&report))?
        );
    } else {
        print_task_next(&report);
    }
    if report.next_action.is_none() {
        bail!("no actionable tasks");
    }
    Ok(())
}

fn print_status(report: StatusReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if report.status == "not_initialized" {
        println!("maestro status: not initialized");
        println!("repo: {}", report.repo);
        for warning in &report.warnings {
            println!("warning: {}", warning.message);
        }
        println!("next: maestro init --yes");
        return Ok(());
    }

    println!("maestro status");
    println!("repo: {}", report.repo);
    println!(
        "tasks: active={} ready={} needs_verification={} blocked={}",
        report.tasks.active,
        report.tasks.ready,
        report.tasks.needs_verification,
        report.tasks.blocked
    );
    println!(
        "features: active={} ready_to_ship={}",
        report.features.active,
        report.ready_to_ship_features.len()
    );
    for warning in &report.warnings {
        println!("warning: {}", warning.message);
    }
    if let Some(action) = &report.next_action {
        print_next_action(action);
    } else {
        println!("no actionable tasks");
    }
    if !report.task_rows.is_empty() {
        println!("ACTIONS");
        println!("NEXT\tTASK\tSTATE\tINSPECT\tTITLE");
        for row in &report.task_rows {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                row.next, row.id, row.state, row.inspect, row.title
            );
        }
    }
    if !report.ready_to_ship_features.is_empty() {
        println!("FEATURES READY TO SHIP");
        for feature in &report.ready_to_ship_features {
            println!(
                "{}\tverified={}/{}\trun: maestro feature ship {} --outcome \"<outcome>\"",
                feature.id, feature.verified, feature.total, feature.id
            );
        }
    }
    Ok(())
}

fn print_task_next(report: &StatusReport) {
    if let Some(action) = &report.next_action {
        print_next_action(action);
        return;
    }
    println!("no actionable tasks");
    if !report.ready_to_ship_features.is_empty() {
        println!("broader repo action available: ready_to_ship_features");
        println!("check broader repo status: maestro status");
    }
}

fn print_next_action(action: &NextAction) {
    println!("next: {}", action.command);
    if let Some(task_id) = action.task_id.as_deref() {
        println!("task: {task_id}");
    }
    if let Some(feature_id) = action.feature_id.as_deref() {
        println!("feature: {feature_id}");
    }
    if let Some(title) = action.title.as_deref() {
        println!("title: {title}");
    }
    println!("reason: {}", action.reason);
    if let Some(inspect) = action.inspect.as_deref() {
        println!("inspect: {inspect}");
    }
}

fn build_status_report(paths: &MaestroPaths) -> Result<StatusReport> {
    let tasks = task::load_task_records(&paths.tasks_dir())?;
    let features = feature::list(paths)?;
    let mut warnings = Vec::new();
    let mut current_task = None;
    let mut current_feature = None;

    let mut live_tasks: Vec<TaskRecord> = tasks
        .iter()
        .filter(|task| task.state.is_live())
        .cloned()
        .collect();
    live_tasks.sort_by(|left, right| left.id.cmp(&right.id));

    let current_task_action = match env::var("MAESTRO_CURRENT_TASK") {
        Ok(id) if !id.trim().is_empty() => match task::load_task_record(&paths.tasks_dir(), &id) {
            Ok(task) if task.state.is_live() => {
                current_task = Some(task.id.clone());
                task_action(paths, &task)?
            }
            Ok(task) => {
                warnings.push(WarningJson {
                    code: "current_task_terminal".to_string(),
                    message: format!(
                        "MAESTRO_CURRENT_TASK={} is {}; falling back to repo queue",
                        task.id,
                        task.state.as_str()
                    ),
                });
                None
            }
            Err(_) => {
                warnings.push(WarningJson {
                    code: "current_task_missing".to_string(),
                    message: format!("MAESTRO_CURRENT_TASK={id} was not found; falling back"),
                });
                None
            }
        },
        _ => None,
    };

    if let Ok(feature_id) = env::var("MAESTRO_CURRENT_FEATURE")
        && !feature_id.trim().is_empty()
    {
        current_feature = Some(feature_id);
    }

    let mut rows = Vec::new();
    for task in &live_tasks {
        rows.push(TaskRowJson {
            id: task.id.clone(),
            state: task_state_label(task),
            title: task.title.clone(),
            next: compact_next(paths, task)?,
            inspect: format!("maestro task show {}", task.id),
        });
    }

    let next_action = match current_task_action {
        Some(action) => Some(action),
        None => choose_next_task_action(paths, &live_tasks)?,
    };
    let ready_to_ship_features = ready_to_ship_features(&features);

    Ok(StatusReport {
        schema: "maestro.status.v1".to_string(),
        status: if next_action.is_some() {
            "actionable".to_string()
        } else {
            "no_action".to_string()
        },
        repo: paths.repo_root().display().to_string(),
        current_task,
        current_feature,
        warnings,
        next_action,
        tasks: TaskSummaryJson::from_tasks(&tasks),
        features: FeatureSummaryJson::from_features(&features),
        task_rows: rows,
        ready_to_ship_features,
    })
}

fn choose_next_task_action(
    paths: &MaestroPaths,
    tasks: &[TaskRecord],
) -> Result<Option<NextAction>> {
    for state in [
        TaskState::NeedsVerification,
        TaskState::Ready,
        TaskState::InProgress,
        TaskState::Draft,
        TaskState::Exploring,
    ] {
        if let Some(action) = tasks
            .iter()
            .filter(|task| task.state == state)
            .find_map(|task| task_action(paths, task).transpose())
            .transpose()?
        {
            return Ok(Some(action));
        }
    }
    if let Some(action) = tasks
        .iter()
        .find(|task| task::has_unresolved_blockers(task))
        .map(blocked_action)
    {
        return Ok(Some(action));
    }
    Ok(None)
}

fn task_action(paths: &MaestroPaths, task: &TaskRecord) -> Result<Option<NextAction>> {
    if task::has_unresolved_blockers(task) {
        return Ok(Some(blocked_action(task)));
    }
    let checks = task::load_task_checks(&paths.tasks_dir(), task).unwrap_or_default();
    let has_verify_contract = task.feature_id.is_some() || !checks.is_empty();
    let action = match task.state {
        TaskState::NeedsVerification => NextAction::task(
            "proof_recovery",
            task,
            format!("maestro query proof {}", task.id),
            true,
            false,
            "verification needs proof recovery",
        ),
        TaskState::Ready => NextAction::task(
            "claim_task",
            task,
            format!("maestro task claim {}", task.id),
            true,
            false,
            "ready task is unclaimed",
        ),
        TaskState::InProgress => NextAction::task(
            "complete_task",
            task,
            format!(
                "maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"",
                task.id
            ),
            false,
            true,
            "claimed task needs completion proof",
        ),
        TaskState::Draft if !has_verify_contract => NextAction::task(
            "add_task_check",
            task,
            format!(
                "maestro task set {} --check \"<observable result>\"",
                task.id
            ),
            false,
            true,
            "standalone task needs a verify+ check",
        ),
        TaskState::Draft => NextAction::task(
            "explore_task",
            task,
            format!("maestro task explore {}", task.id),
            true,
            false,
            "draft task has a verify+ path",
        ),
        TaskState::Exploring if !has_verify_contract => NextAction::task(
            "add_task_check",
            task,
            format!(
                "maestro task set {} --check \"<observable result>\"",
                task.id
            ),
            false,
            true,
            "standalone task needs a verify+ check before accept",
        ),
        TaskState::Exploring => NextAction::task(
            "accept_task",
            task,
            format!("maestro task accept {}", task.id),
            true,
            false,
            "explored task can lock acceptance",
        ),
        TaskState::Verified
        | TaskState::Rejected
        | TaskState::Abandoned
        | TaskState::Superseded => {
            return Ok(None);
        }
    };
    Ok(Some(action))
}

fn blocked_action(task: &TaskRecord) -> NextAction {
    NextAction::task(
        "inspect_blocker",
        task,
        format!("maestro task show {}", task.id),
        true,
        false,
        "task has unresolved blockers",
    )
}

fn compact_next(paths: &MaestroPaths, task: &TaskRecord) -> Result<String> {
    Ok(task_action(paths, task)?
        .map(|action| action.kind)
        .unwrap_or_else(|| "status".to_string()))
}

fn task_state_label(task: &TaskRecord) -> String {
    if task::has_unresolved_blockers(task) {
        format!("{} / blocked", task.state.as_str())
    } else {
        task.state.as_str().to_string()
    }
}

fn ready_to_ship_features(features: &[feature::FeatureView]) -> Vec<ReadyFeatureJson> {
    features
        .iter()
        .filter(|view| view.status == FeatureStatus::InProgress)
        .filter(|view| view.counts.total > 0 && view.counts.total == view.counts.verified)
        .map(|view| ReadyFeatureJson {
            id: view.id.clone(),
            title: view.title.clone(),
            total: view.counts.total,
            verified: view.counts.verified,
            command: format!("maestro feature ship {} --outcome \"<outcome>\"", view.id),
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
struct StatusReport {
    schema: String,
    status: String,
    repo: String,
    current_task: Option<String>,
    current_feature: Option<String>,
    warnings: Vec<WarningJson>,
    next_action: Option<NextAction>,
    tasks: TaskSummaryJson,
    features: FeatureSummaryJson,
    task_rows: Vec<TaskRowJson>,
    ready_to_ship_features: Vec<ReadyFeatureJson>,
}

impl StatusReport {
    fn not_initialized(repo: PathBuf, reason: String) -> Self {
        Self {
            schema: "maestro.status.v1".to_string(),
            status: "not_initialized".to_string(),
            repo: repo.display().to_string(),
            current_task: None,
            current_feature: None,
            warnings: vec![WarningJson {
                code: "not_initialized".to_string(),
                message: reason,
            }],
            next_action: Some(NextAction {
                kind: "init_maestro".to_string(),
                scope: "repo".to_string(),
                task_id: None,
                feature_id: None,
                title: None,
                command: "maestro init --yes".to_string(),
                runnable: true,
                requires_input: false,
                reason: "maestro is not initialized in this repo".to_string(),
                inspect: None,
            }),
            tasks: TaskSummaryJson::default(),
            features: FeatureSummaryJson::default(),
            task_rows: Vec::new(),
            ready_to_ship_features: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TaskNextJson {
    schema: String,
    status: String,
    next_action: Option<NextAction>,
    warnings: Vec<WarningJson>,
    summary: String,
}

impl From<&StatusReport> for TaskNextJson {
    fn from(report: &StatusReport) -> Self {
        let mut warnings = report.warnings.clone();
        if report.next_action.is_none() && !report.ready_to_ship_features.is_empty() {
            warnings.push(WarningJson {
                code: "broader_actions_available".to_string(),
                message: "broader repo action available; run maestro status".to_string(),
            });
        }
        Self {
            schema: "maestro.task_next.v1".to_string(),
            status: if report.next_action.is_some() {
                "actionable".to_string()
            } else {
                "no_action".to_string()
            },
            next_action: report.next_action.clone(),
            warnings,
            summary: if report.next_action.is_some() {
                "task action available".to_string()
            } else {
                "no actionable tasks".to_string()
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct NextAction {
    kind: String,
    scope: String,
    task_id: Option<String>,
    feature_id: Option<String>,
    title: Option<String>,
    command: String,
    runnable: bool,
    requires_input: bool,
    reason: String,
    inspect: Option<String>,
}

impl NextAction {
    fn task(
        kind: &str,
        task: &TaskRecord,
        command: String,
        runnable: bool,
        requires_input: bool,
        reason: &str,
    ) -> Self {
        Self {
            kind: kind.to_string(),
            scope: "task".to_string(),
            task_id: Some(task.id.clone()),
            feature_id: task.feature_id.clone(),
            title: Some(task.title.clone()),
            command,
            runnable,
            requires_input,
            reason: reason.to_string(),
            inspect: Some(format!("maestro task show {}", task.id)),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct WarningJson {
    code: String,
    message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
struct TaskSummaryJson {
    total: usize,
    active: usize,
    ready: usize,
    needs_verification: usize,
    blocked: usize,
    verified: usize,
}

impl TaskSummaryJson {
    fn from_tasks(tasks: &[TaskRecord]) -> Self {
        Self {
            total: tasks.len(),
            active: tasks.iter().filter(|task| task.state.is_live()).count(),
            ready: tasks
                .iter()
                .filter(|task| task.state == TaskState::Ready)
                .count(),
            needs_verification: tasks
                .iter()
                .filter(|task| task.state == TaskState::NeedsVerification)
                .count(),
            blocked: tasks
                .iter()
                .filter(|task| task::has_unresolved_blockers(task))
                .count(),
            verified: tasks
                .iter()
                .filter(|task| task.state == TaskState::Verified)
                .count(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct FeatureSummaryJson {
    total: usize,
    active: usize,
    shipped: usize,
    cancelled: usize,
}

impl FeatureSummaryJson {
    fn from_features(features: &[feature::FeatureView]) -> Self {
        Self {
            total: features.len(),
            active: features
                .iter()
                .filter(|view| !view.status.is_terminal())
                .count(),
            shipped: features
                .iter()
                .filter(|view| view.status == FeatureStatus::Shipped)
                .count(),
            cancelled: features
                .iter()
                .filter(|view| view.status == FeatureStatus::Cancelled)
                .count(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TaskRowJson {
    id: String,
    state: String,
    title: String,
    next: String,
    inspect: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReadyFeatureJson {
    id: String,
    title: String,
    total: usize,
    verified: usize,
    command: String,
}
