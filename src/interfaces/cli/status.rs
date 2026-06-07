use std::env;
use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::domain::feature::{self, FeatureRosterEntry, FeatureStatus};
use crate::domain::task::{self, TaskRecord, TaskState};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{StatusArgs, recovery_label};
use crate::operations::harness;

const STATUS_TASK_ROW_LIMIT: usize = 5;

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
    if report.next_action.is_none() && report.harness_friction.is_empty() {
        bail!("no actionable task");
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
        println!("next:");
        println!("- preview setup: maestro init --dry-run");
        println!("- initialize: maestro init --yes");
        return Ok(());
    }

    if !report.warnings.is_empty() {
        for warning in &report.warnings {
            println!("warning: {}", warning.message);
        }
        println!();
    }
    println!("maestro status");
    println!("repo: {}", report.repo);
    println!("resume: maestro resume");
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
    print_harness_friction(&report.harness_friction);
    print_audit_hint(report.audit_hint.as_ref());
    if let Some(action) = &report.next_action {
        print_next_action(action);
    } else {
        println!("no actionable tasks");
    }
    if !report.task_rows.is_empty() {
        println!("ACTIONS");
        println!("NEXT\tTASK\tSTATE\tINSPECT\tTITLE");
        for row in report.task_rows.iter().take(STATUS_TASK_ROW_LIMIT) {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                row.next, row.id, row.state, row.inspect, row.title
            );
        }
        if report.task_rows.len() > STATUS_TASK_ROW_LIMIT {
            println!(
                "... {} more active task(s); run maestro task list",
                report.task_rows.len() - STATUS_TASK_ROW_LIMIT
            );
        }
    }
    if !report.active_features.is_empty() {
        println!("ACTIVE FEATURES");
        println!("FEATURE\tSTATE\tNEXT\tINSPECT\tTITLE");
        for row in &report.active_features {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                row.id, row.state, row.next, row.inspect, row.title
            );
        }
    }
    if !report.ready_to_ship_features.is_empty() {
        println!("FEATURES READY TO SHIP");
        for feature in &report.ready_to_ship_features {
            println!(
                "{}\tverified={}/{}\ttemplate: {}",
                feature.id, feature.verified, feature.total, feature.next_action.command.display
            );
        }
    }
    Ok(())
}

fn print_task_next(report: &StatusReport) {
    if !report.warnings.is_empty() {
        for warning in &report.warnings {
            println!("warning: {}", warning.message);
        }
        println!();
    }
    print_harness_friction(&report.harness_friction);
    print_audit_hint(report.audit_hint.as_ref());
    if let Some(action) = &report.next_action {
        print_next_action(action);
        return;
    }
    println!("no actionable task");
    if !report.ready_to_ship_features.is_empty() {
        println!("broader repo action available: ready_to_ship_features");
        println!("check broader repo status: maestro status");
    }
}

pub(crate) fn print_harness_friction_epilogue(paths: &MaestroPaths) -> Result<()> {
    let items = harness::over_threshold_items(paths)?
        .into_iter()
        .map(HarnessFrictionJson::from)
        .collect::<Vec<_>>();
    print_harness_friction(&items);
    Ok(())
}

fn print_harness_friction(items: &[HarnessFrictionJson]) {
    if items.is_empty() {
        return;
    }
    println!("HARNESS FRICTION");
    for item in items {
        println!("! friction {} over threshold", item.id);
        println!("  seen: {}x/{}s", item.occurrences, item.sessions);
        println!("  title: {}", item.title);
        println!("  apply: maestro harness apply {}", item.id);
        println!(
            "  dismiss: maestro harness dismiss {} --reason \"<why>\"",
            item.id
        );
    }
}

fn print_audit_hint(hint: Option<&AuditHintJson>) {
    let Some(hint) = hint else {
        return;
    };
    println!("AUDIT");
    println!(
        "! repo audit overdue: {} session(s) since last audit (threshold {})",
        hint.sessions_since_audit, hint.every_sessions
    );
    println!("  skill: maestro-audit");
    println!(
        "  propose: maestro harness propose --title \"<finding>\" --evidence \"<evidence>\" --topic <slug>"
    );
}

fn print_next_action(action: &NextAction) {
    if action.requires_input {
        println!("template: {}", action.command.display);
    } else {
        println!("run: {}", action.command.display);
    }
    if !action.command.requires_input.is_empty() {
        println!("required input:");
        for input in &action.command.requires_input {
            println!("- {}: {}", input.name, input.description);
        }
    }
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
    let mut features = Vec::new();
    let mut unreadable_features = Vec::new();
    for entry in feature::list_tolerant(paths) {
        match entry {
            FeatureRosterEntry::Loaded(view) => features.push(*view),
            FeatureRosterEntry::Unreadable {
                id,
                path,
                error,
                hint,
                typed_error,
            } => unreadable_features.push((id, path, error, hint, typed_error)),
        }
    }
    if features.is_empty()
        && let Some((_, _, error, _, typed_error)) = unreadable_features.first()
    {
        if let Some(typed_error) = typed_error.clone() {
            return Err(typed_error.into());
        }
        bail!("{error}");
    }
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
                current_feature = task.feature_id.clone();
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
    let mut active_features = active_feature_rows(&features);
    for (id, path, error, hint, _) in unreadable_features {
        warnings.push(WarningJson {
            code: "feature_unreadable".to_string(),
            message: format!("{} is unreadable: {error}", path.display()),
        });
        active_features.push(FeatureRowJson {
            id: id.clone(),
            state: "unreadable".to_string(),
            title: error,
            next: recovery_label(hint.as_deref()),
            inspect: format!("maestro feature spec {id}"),
        });
    }
    let harness_friction = harness::over_threshold_items(paths)?
        .into_iter()
        .map(HarnessFrictionJson::from)
        .collect::<Vec<_>>();
    let audit_hint = harness::audit_overdue_hint(paths)?.map(AuditHintJson::from);
    let sections = StatusSectionsJson {
        ready_to_ship: ready_to_ship_features.clone(),
    };

    Ok(StatusReport {
        schema: "maestro.status.v1".to_string(),
        status: if next_action.is_some() || !harness_friction.is_empty() || audit_hint.is_some() {
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
        active_features,
        harness_friction,
        audit_hint,
        sections,
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
            runnable_command(["maestro", "query", "proof", task.id.as_str()]),
            "verification needs proof recovery",
        ),
        TaskState::Ready => NextAction::task(
            "claim_task",
            task,
            runnable_command(["maestro", "task", "claim", task.id.as_str()]),
            "ready task is unclaimed",
        ),
        TaskState::InProgress => NextAction::task(
            "complete_task",
            task,
            task_complete_template(&task.id),
            "claimed task needs completion proof",
        ),
        TaskState::Draft if !has_verify_contract => NextAction::task(
            "add_task_check",
            task,
            task_check_template(&task.id),
            "standalone task needs a verify+ check",
        ),
        TaskState::Draft => NextAction::task(
            "explore_task",
            task,
            runnable_command(["maestro", "task", "explore", task.id.as_str()]),
            "draft task has a verify+ path",
        ),
        TaskState::Exploring if !has_verify_contract => NextAction::task(
            "add_task_check",
            task,
            task_check_template(&task.id),
            "standalone task needs a verify+ check before accept",
        ),
        TaskState::Exploring => NextAction::task(
            "accept_task",
            task,
            runnable_command(["maestro", "task", "accept", task.id.as_str()]),
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
        runnable_command(["maestro", "task", "show", task.id.as_str()]),
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
            feature_id: view.id.clone(),
            title: view.title.clone(),
            total: view.counts.total,
            verified: view.counts.verified,
            next_action: NextAction::feature_ship(view),
        })
        .collect()
}

fn active_feature_rows(features: &[feature::FeatureView]) -> Vec<FeatureRowJson> {
    features
        .iter()
        .filter(|view| !view.status.is_terminal())
        .map(|view| FeatureRowJson {
            id: view.id.clone(),
            state: feature::status_label(&view.status).to_string(),
            title: view.title.clone(),
            next: status_feature_next_label(view).to_string(),
            inspect: format!("maestro feature show {}", view.id),
        })
        .collect()
}

fn status_feature_next_label(view: &feature::FeatureView) -> &'static str {
    match view.status {
        FeatureStatus::Proposed => "template: set_contract",
        FeatureStatus::Ready => "run: prepare_feature",
        FeatureStatus::InProgress
            if view.counts.total > 0 && view.counts.total == view.counts.verified =>
        {
            "template: ship_feature"
        }
        FeatureStatus::InProgress => "run: resolve_tasks",
        FeatureStatus::Shipped | FeatureStatus::Cancelled => "run: archive_feature",
    }
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
    active_features: Vec<FeatureRowJson>,
    harness_friction: Vec<HarnessFrictionJson>,
    audit_hint: Option<AuditHintJson>,
    sections: StatusSectionsJson,
    ready_to_ship_features: Vec<ReadyFeatureJson>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct StatusSectionsJson {
    ready_to_ship: Vec<ReadyFeatureJson>,
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
            next_action: Some(NextAction::repo(
                "init_maestro",
                runnable_command(["maestro", "init", "--yes"]),
                "maestro is not initialized in this repo",
            )),
            tasks: TaskSummaryJson::default(),
            features: FeatureSummaryJson::default(),
            task_rows: Vec::new(),
            active_features: Vec::new(),
            harness_friction: Vec::new(),
            audit_hint: None,
            sections: StatusSectionsJson::default(),
            ready_to_ship_features: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TaskNextJson {
    schema: String,
    status: String,
    next_action: Option<NextAction>,
    harness_friction: Vec<HarnessFrictionJson>,
    audit_hint: Option<AuditHintJson>,
    broader_actions: Vec<BroaderActionJson>,
    warnings: Vec<WarningJson>,
    summary: String,
}

#[derive(Clone, Debug, Serialize)]
struct BroaderActionJson {
    kind: String,
    feature_id: String,
    summary: String,
    inspect: String,
}

impl From<&ReadyFeatureJson> for BroaderActionJson {
    fn from(feature: &ReadyFeatureJson) -> Self {
        Self {
            kind: "feature_ready_to_ship".to_string(),
            feature_id: feature.id.clone(),
            summary: "ready-to-ship feature available in status".to_string(),
            inspect: format!("maestro feature show {}", feature.id),
        }
    }
}

impl From<&StatusReport> for TaskNextJson {
    fn from(report: &StatusReport) -> Self {
        let warnings = report.warnings.clone();
        let broader_actions = if report.next_action.is_none()
            && report.harness_friction.is_empty()
            && report.audit_hint.is_none()
        {
            report
                .ready_to_ship_features
                .iter()
                .map(BroaderActionJson::from)
                .collect()
        } else {
            Vec::new()
        };
        Self {
            schema: "maestro.task_next.v1".to_string(),
            status: if report.next_action.is_some()
                || !report.harness_friction.is_empty()
                || report.audit_hint.is_some()
            {
                "actionable".to_string()
            } else {
                "no_action".to_string()
            },
            next_action: report.next_action.clone(),
            harness_friction: report.harness_friction.clone(),
            audit_hint: report.audit_hint.clone(),
            broader_actions,
            warnings,
            summary: if report.next_action.is_some()
                || !report.harness_friction.is_empty()
                || report.audit_hint.is_some()
            {
                "task action available".to_string()
            } else {
                "no actionable tasks".to_string()
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct AuditHintJson {
    sessions_since_audit: usize,
    every_sessions: usize,
    skill: String,
}

impl From<harness::AuditHint> for AuditHintJson {
    fn from(hint: harness::AuditHint) -> Self {
        Self {
            sessions_since_audit: hint.sessions_since_audit,
            every_sessions: hint.every_sessions,
            skill: "maestro-audit".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct HarnessFrictionJson {
    id: String,
    item_type: String,
    title: String,
    priority: String,
    occurrences: usize,
    sessions: usize,
}

impl From<harness::OverThresholdItem> for HarnessFrictionJson {
    fn from(item: harness::OverThresholdItem) -> Self {
        Self {
            id: item.id,
            item_type: item.item_type,
            title: item.title,
            priority: item.priority,
            occurrences: item.occurrences,
            sessions: item.sessions,
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
    command: CommandJson,
    runnable: bool,
    requires_input: bool,
    reason: String,
    inspect: Option<String>,
}

impl NextAction {
    fn task(kind: &str, task: &TaskRecord, command: CommandJson, reason: &str) -> Self {
        let runnable = command.argv.is_some();
        let requires_input = !command.requires_input.is_empty();
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

    fn repo(kind: &str, command: CommandJson, reason: &str) -> Self {
        let runnable = command.argv.is_some();
        let requires_input = !command.requires_input.is_empty();
        Self {
            kind: kind.to_string(),
            scope: "repo".to_string(),
            task_id: None,
            feature_id: None,
            title: None,
            command,
            runnable,
            requires_input,
            reason: reason.to_string(),
            inspect: None,
        }
    }

    fn feature_ship(view: &feature::FeatureView) -> Self {
        let command = feature_ship_template(&view.id);
        let runnable = command.argv.is_some();
        let requires_input = !command.requires_input.is_empty();
        Self {
            kind: "feature_ship".to_string(),
            scope: "feature".to_string(),
            task_id: None,
            feature_id: Some(view.id.clone()),
            title: Some(view.title.clone()),
            command,
            runnable,
            requires_input,
            reason: "feature has no open child task work".to_string(),
            inspect: Some(format!("maestro feature show {}", view.id)),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct CommandJson {
    display: String,
    argv: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    argv_template: Option<Vec<String>>,
    requires_input: Vec<RequiredInputJson>,
}

#[derive(Clone, Debug, Serialize)]
struct RequiredInputJson {
    name: String,
    flag: String,
    placeholder: String,
    description: String,
}

fn runnable_command<const N: usize>(parts: [&str; N]) -> CommandJson {
    CommandJson {
        display: parts.join(" "),
        argv: Some(parts.iter().map(|part| (*part).to_string()).collect()),
        argv_template: None,
        requires_input: Vec::new(),
    }
}

fn task_check_template(task_id: &str) -> CommandJson {
    template_command(
        format!("maestro task set {task_id} --check \"<observable result>\""),
        vec![
            "maestro",
            "task",
            "set",
            task_id,
            "--check",
            "<observable result>",
        ],
        vec![required_input(
            "observable_result",
            "--check",
            "<observable result>",
            "observable acceptance check text",
        )],
    )
}

fn task_complete_template(task_id: &str) -> CommandJson {
    template_command(
        format!(
            "maestro task complete {task_id} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\""
        ),
        vec![
            "maestro",
            "task",
            "complete",
            task_id,
            "--summary",
            "<summary>",
            "--claim",
            "<claim>",
            "--proof",
            "<observed evidence>",
        ],
        vec![
            required_input("summary", "--summary", "<summary>", "what changed"),
            required_input("claim", "--claim", "<claim>", "observable completion claim"),
            required_input(
                "proof",
                "--proof",
                "<observed evidence>",
                "observed proof text",
            ),
        ],
    )
}

fn feature_ship_template(feature_id: &str) -> CommandJson {
    template_command(
        format!("maestro feature ship {feature_id} --outcome \"<outcome>\""),
        vec![
            "maestro",
            "feature",
            "ship",
            feature_id,
            "--outcome",
            "<outcome>",
        ],
        vec![required_input(
            "outcome",
            "--outcome",
            "<outcome>",
            "shipping outcome text",
        )],
    )
}

fn template_command(
    display: String,
    argv_template: Vec<&str>,
    inputs: Vec<RequiredInputJson>,
) -> CommandJson {
    CommandJson {
        display,
        argv: None,
        argv_template: Some(
            argv_template
                .into_iter()
                .map(|part| part.to_string())
                .collect(),
        ),
        requires_input: inputs,
    }
}

fn required_input(
    name: &str,
    flag: &str,
    placeholder: &str,
    description: &str,
) -> RequiredInputJson {
    RequiredInputJson {
        name: name.to_string(),
        flag: flag.to_string(),
        placeholder: placeholder.to_string(),
        description: description.to_string(),
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
struct FeatureRowJson {
    id: String,
    state: String,
    title: String,
    next: String,
    inspect: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReadyFeatureJson {
    id: String,
    feature_id: String,
    title: String,
    total: usize,
    verified: usize,
    next_action: NextAction,
}
