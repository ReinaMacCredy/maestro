use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::domain::card::locator::{ArtifactLocator, SurfaceLocator};
use crate::domain::card::{live_db, store as card_store};
use crate::domain::decisions;
use crate::domain::feature::registry;
use crate::domain::feature::verification::acceptance_id;
use crate::domain::proof;
use crate::domain::task::{self, BlockerTarget, TaskRecord, TaskState, TransitionDetails};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

const FINGERPRINT_CATEGORIES: [&str; 6] = [
    "contract",
    "decisions",
    "questions",
    "tasks",
    "qa",
    "handoff",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileSurfaceError {
    pub feature_id: String,
    pub surface: SurfaceLocator,
}

impl fmt::Display for ReconcileSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "db_backed feature {} cannot be reconciled directly; run `maestro feature reopen {}` first",
            self.feature_id, self.feature_id
        )
    }
}

impl std::error::Error for ReconcileSurfaceError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileStatus {
    Clean,
    ChangesRequired,
    Applied,
    StaleReceipt,
    Error,
}

impl ReconcileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::ChangesRequired => "changes_required",
            Self::Applied => "applied",
            Self::StaleReceipt => "stale_receipt",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileReport {
    pub status: ReconcileStatus,
    pub feature: ReconcileFeature,
    pub surface: SurfaceLocator,
    pub receipt: ReconcileReceipt,
    pub issues: Vec<ReconcileIssue>,
    pub contract: ReconcileContract,
    pub questions: ReconcileQuestions,
    pub tasks: ReconcileTasks,
    pub qa: ReconcileArtifactSummary,
    pub handoff: ReconcileArtifactSummary,
    pub next: Vec<ReconcileAction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileFeature {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileReceipt {
    pub state: String,
    pub mode: Option<String>,
    pub fresh: bool,
    pub artifact: Option<ArtifactLocator>,
    pub fingerprints: BTreeMap<String, ReconcileFingerprint>,
    pub stale: Vec<String>,
    pub plan_digest: Option<String>,
    pub created_at: Option<String>,
    pub actor: Option<ReconcileActor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileFingerprint {
    pub receipt: Option<String>,
    pub current: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReconcileActor {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ReconcileActor {
    pub fn agent(id: &str, session: Option<String>) -> Self {
        Self {
            kind: "agent".to_string(),
            id: Some(id.to_string()),
            session,
            name: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileIssue {
    pub kind: String,
    pub category: Option<String>,
    pub severity: String,
    pub summary: String,
    pub refs: Vec<ReconcileIssueRef>,
    pub next: Vec<ReconcileAction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileIssueRef {
    #[serde(rename = "type")]
    pub ref_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileContract {
    pub vision: Option<String>,
    pub description: Option<String>,
    pub acceptance: Vec<ReconcileTextItem>,
    pub non_goals: Vec<ReconcileTextItem>,
    pub affected_areas: Vec<ReconcileTextItem>,
    pub source_refs: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileQuestions {
    pub open: Vec<ReconcileTextItem>,
    pub resolved: Vec<Value>,
    pub stale_candidates: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileTasks {
    pub order: Vec<String>,
    pub items: Vec<ReconcileTaskItem>,
    pub removed: Vec<Value>,
    pub added_candidates: Vec<Value>,
    pub dependencies: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileTaskItem {
    pub id: String,
    pub title: String,
    pub state: String,
    pub acceptance: Vec<String>,
    pub covers: Vec<String>,
    pub depends_on: Vec<String>,
    pub history: ReconcileTaskHistory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileTaskHistory {
    pub has_proof: bool,
    pub has_execution: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileArtifactSummary {
    pub present: bool,
    pub digest: Option<String>,
    pub refs: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileAction {
    pub kind: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_change: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconcileTextItem {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReconcilePlan {
    vision: String,
    description: String,
    acceptance: Vec<String>,
    non_goals: Vec<String>,
    affected_areas: Vec<String>,
    questions: PlanQuestions,
    tasks: PlanTasks,
    rationale: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanQuestions {
    remove: Vec<QuestionRemoval>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QuestionRemoval {
    reference: Option<String>,
    #[serde(rename = "ref")]
    ref_: Option<String>,
    reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanTasks {
    order: Vec<String>,
    add: Vec<PlanTaskAdd>,
    remove: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanTaskAdd {
    key: String,
    title: String,
    intent: String,
    acceptance: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[derive(Clone, Debug)]
struct ValidatedPlan {
    question_remove_indexes: BTreeSet<usize>,
    task_id_by_ref: BTreeMap<String, String>,
    changed_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredReconcileReceipt {
    mode: String,
    surface: SurfaceLocator,
    fingerprints: BTreeMap<String, String>,
    changed_fields: Vec<String>,
    plan_digest: Option<String>,
    created_at: String,
    actor: ReconcileActor,
}

pub fn reconcile_report(paths: &MaestroPaths, id: &str) -> Result<ReconcileReport> {
    build_reconcile_report(paths, id, None)
}

pub fn reconcile_clean_check(
    paths: &MaestroPaths,
    id: &str,
    actor: ReconcileActor,
) -> Result<ReconcileReport> {
    let mut report = build_reconcile_report(paths, id, None)?;
    if report.issues.is_empty() && report.receipt.state != "current" {
        let receipt = store_current_receipt(
            paths,
            id,
            &report.surface,
            "clean_check",
            None,
            actor,
            Vec::new(),
        )?;
        report.receipt = receipt;
        report.status = ReconcileStatus::Clean;
        report.next = reconcile_next(id, &report.status);
    }
    Ok(report)
}

pub fn apply_reconcile_plan(
    paths: &MaestroPaths,
    id: &str,
    plan_path: &Path,
    actor: ReconcileActor,
) -> Result<ReconcileReport> {
    let surface = reconcile_surface(paths, id)?;
    let contents = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read {}", plan_path.display()))?;
    let plan: ReconcilePlan = serde_yaml::from_str(&contents)
        .with_context(|| format!("invalid reconcile plan schema in {}", plan_path.display()))?;
    let plan_digest = sha256_prefixed(contents.as_bytes());
    let (mut record, write) = registry::load_record_for_update(paths, id)?;
    if record.status.is_terminal() {
        bail!(
            "cannot reconcile {id} — terminal (status: {})",
            record.status.as_str()
        );
    }
    let tasks = task::filter_tasks(
        task::load_task_records(&paths.tasks_dir())?,
        &task::TaskFilter {
            feature_id: Some(id.to_string()),
            include_terminal: true,
            ..Default::default()
        },
    );
    let validated = validate_plan(&record.open_questions, &tasks, &plan)?;

    let mut created = Vec::new();
    let result = (|| -> Result<ReconcileReport> {
        let mut task_id_by_ref = validated.task_id_by_ref.clone();
        for item in &plan.tasks.add {
            let now = utc_now_timestamp();
            let created_task = task::create_task(
                &paths.tasks_dir(),
                &item.title,
                task::CreateTaskOptions {
                    feature: Some(id.to_string()),
                    covers: Vec::new(),
                    lane: None,
                    risk: None,
                    checks: item.acceptance.clone(),
                    project: None,
                    created_at: now,
                },
            )?;
            let now = utc_now_timestamp();
            let explored = task::transition_task(
                &paths.tasks_dir(),
                &created_task.id,
                TaskState::Exploring,
                actor.id.as_deref().unwrap_or("maestro"),
                &now,
                TransitionDetails {
                    summary: Some(item.intent.clone()),
                    ..TransitionDetails::default()
                },
            )?;
            task_id_by_ref.insert(item.key.clone(), explored.id.clone());
            created.push(explored);
        }

        apply_task_dependencies(paths, &plan, &task_id_by_ref, actor.id.as_deref())?;
        for item in &plan.tasks.add {
            let task_id = task_id_by_ref
                .get(&item.key)
                .context("created task key missing after apply")?;
            let now = utc_now_timestamp();
            let accepted = task::accept_task(
                &paths.tasks_dir(),
                task_id,
                actor.id.as_deref().unwrap_or("maestro"),
                &now,
            )?;
            if let Some(stored) = created.iter_mut().find(|task| task.id == accepted.id) {
                *stored = accepted;
            }
        }
        apply_task_removals(paths, &plan.tasks.remove, actor.id.as_deref())?;

        record.raw_request = Some(plan.vision.clone());
        record.description = Some(plan.description.clone());
        record.acceptance = plan.acceptance.clone();
        record.non_goals = plan.non_goals.clone();
        record.affected_areas = plan.affected_areas.clone();
        record.open_questions = record
            .open_questions
            .iter()
            .enumerate()
            .filter(|(index, _)| !validated.question_remove_indexes.contains(index))
            .map(|(_, question)| question.clone())
            .collect();
        record.updated_at = utc_now_timestamp();
        registry::save_record(&record, &write)?;

        let receipt = store_current_receipt(
            paths,
            id,
            &surface,
            "apply_plan",
            Some(plan_digest.clone()),
            actor.clone(),
            validated.changed_fields.clone(),
        )?;
        let mut report = build_reconcile_report(paths, id, Some(receipt))?;
        report.status = ReconcileStatus::Applied;
        report.next = reconcile_next(id, &report.status);
        Ok(report)
    })();

    if result.is_err() {
        rollback_created_tasks(paths, &created)?;
    }
    result
}

pub(crate) fn ensure_current_receipt_for_finalize(paths: &MaestroPaths, id: &str) -> Result<()> {
    let receipt = load_reconcile_receipt(paths, id)?;
    if receipt.state == "current" {
        return Ok(());
    }
    bail!("{}", finalize_receipt_error(id, &receipt));
}

fn build_reconcile_report(
    paths: &MaestroPaths,
    id: &str,
    receipt_override: Option<ReconcileReceipt>,
) -> Result<ReconcileReport> {
    let surface = reconcile_surface(paths, id)?;
    let view = registry::show(paths, id)?;
    let issues = reconcile_issues(id, &view.open_questions);
    let receipt = match receipt_override {
        Some(receipt) => receipt,
        None => load_reconcile_receipt(paths, id)?,
    };
    let status = if !issues.is_empty() {
        ReconcileStatus::ChangesRequired
    } else if receipt.state == "stale" {
        ReconcileStatus::StaleReceipt
    } else {
        ReconcileStatus::Clean
    };
    let tasks = task_items(paths, id)?;
    let qa = artifact_summary("qa", registry::read_sidecar_text(paths, id, "qa.md")?);
    let handoff = artifact_summary(
        "handoff",
        registry::read_sidecar_text(paths, id, "handoff.md")?,
    );
    Ok(ReconcileReport {
        next: reconcile_next(id, &status),
        status,
        feature: ReconcileFeature {
            id: view.id.clone(),
            title: view.title,
            status: view.status.as_str().to_string(),
        },
        surface,
        receipt,
        issues,
        contract: ReconcileContract {
            vision: view.raw_request,
            description: view.description,
            acceptance: text_items(view.acceptance, "ac", Some(acceptance_id)),
            non_goals: text_items(view.non_goals, "non_goal", None),
            affected_areas: text_items(view.affected_areas, "area", None),
            source_refs: decisions::decisions_for_feature(paths, id)?
                .into_iter()
                .map(|decision| json!({"type": "decision", "id": decision.id}))
                .collect(),
        },
        questions: ReconcileQuestions {
            open: text_items(view.open_questions, "q", None),
            resolved: Vec::new(),
            stale_candidates: Vec::new(),
        },
        tasks,
        qa,
        handoff,
    })
}

fn reconcile_surface(paths: &MaestroPaths, id: &str) -> Result<SurfaceLocator> {
    let workbench = paths.workbench_dir().join(id);
    if workbench.is_dir() {
        return Ok(SurfaceLocator::workbench(paths, id));
    }
    if live_db::contains_card_id(paths, id)? {
        bail!(ReconcileSurfaceError {
            feature_id: id.to_string(),
            surface: SurfaceLocator::db(paths, id),
        });
    }
    Ok(SurfaceLocator::card_folder(paths, id))
}

fn reconcile_issues(feature_id: &str, open_questions: &[String]) -> Vec<ReconcileIssue> {
    if open_questions.is_empty() {
        return Vec::new();
    }
    vec![ReconcileIssue {
        kind: "open_questions".to_string(),
        category: Some("questions".to_string()),
        severity: "blocker".to_string(),
        summary: "open questions require human or authorized-agent judgment before finalize"
            .to_string(),
        refs: open_questions
            .iter()
            .enumerate()
            .map(|(index, _)| ReconcileIssueRef {
                ref_type: "question".to_string(),
                id: Some(format!("q-{}", index + 1)),
                path: None,
                field: Some("open_questions".to_string()),
                section: None,
            })
            .collect(),
        next: vec![
            ReconcileAction::read(
                "Read the full human context before authoring the plan.",
                format!("maestro feature reconcile {feature_id} --full"),
            ),
            ReconcileAction::read(
                "Read the full agent context before authoring the plan.",
                format!("maestro feature reconcile {feature_id} --json"),
            ),
            ReconcileAction::plan_change(
                "Remove or retain each open question explicitly in reconcile.yml.",
                json!({"section": "questions.remove"}),
            ),
        ],
    }]
}

fn validate_plan(
    open_questions: &[String],
    tasks: &[TaskRecord],
    plan: &ReconcilePlan,
) -> Result<ValidatedPlan> {
    ensure_not_blank("vision", &plan.vision)?;
    ensure_not_blank("description", &plan.description)?;
    ensure_not_blank("rationale", &plan.rationale)?;
    ensure_no_blank_values("acceptance", &plan.acceptance)?;
    ensure_no_blank_values("non_goals", &plan.non_goals)?;
    ensure_no_blank_values("affected_areas", &plan.affected_areas)?;

    let question_remove_indexes = validate_question_removals(open_questions, &plan.questions)?;
    let task_id_by_ref = validate_task_plan(tasks, &plan.tasks)?;
    let mut changed_fields = vec![
        "vision".to_string(),
        "description".to_string(),
        "acceptance".to_string(),
        "non_goals".to_string(),
        "affected_areas".to_string(),
        "questions".to_string(),
        "tasks".to_string(),
        "rationale".to_string(),
    ];
    changed_fields.sort();
    Ok(ValidatedPlan {
        question_remove_indexes,
        task_id_by_ref,
        changed_fields,
    })
}

fn validate_question_removals(
    open_questions: &[String],
    questions: &PlanQuestions,
) -> Result<BTreeSet<usize>> {
    let mut indexes = BTreeSet::new();
    for removal in &questions.remove {
        let reference = removal_ref(removal)?;
        ensure_not_blank("questions.remove.reason", &removal.reason)?;
        let matches = open_questions
            .iter()
            .enumerate()
            .filter(|(index, question)| {
                reference == format!("q-{}", index + 1) || reference == question.as_str()
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [index] => {
                indexes.insert(*index);
            }
            [] => bail!("questions.remove ref `{reference}` did not match an open question"),
            _ => bail!("questions.remove ref `{reference}` matched multiple open questions"),
        }
    }
    Ok(indexes)
}

fn removal_ref(removal: &QuestionRemoval) -> Result<&str> {
    match (removal.reference.as_deref(), removal.ref_.as_deref()) {
        (Some(_), Some(_)) => {
            bail!("questions.remove entries use either ref or reference, not both")
        }
        (Some(value), None) | (None, Some(value)) if !value.trim().is_empty() => Ok(value.trim()),
        _ => bail!("questions.remove entries require ref plus reason"),
    }
}

fn validate_task_plan(tasks: &[TaskRecord], plan: &PlanTasks) -> Result<BTreeMap<String, String>> {
    let existing_ids = tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let removed = plan.remove.iter().cloned().collect::<BTreeSet<_>>();
    if removed.len() != plan.remove.len() {
        bail!("tasks.remove contains duplicate entries");
    }
    for id in &removed {
        if !existing_ids.contains(id) {
            bail!("tasks.remove references unknown task `{id}`");
        }
    }

    let mut added_keys = BTreeSet::new();
    for item in &plan.add {
        ensure_not_blank("tasks.add.key", &item.key)?;
        ensure_not_blank("tasks.add.title", &item.title)?;
        ensure_not_blank("tasks.add.intent", &item.intent)?;
        ensure_no_blank_values("tasks.add.acceptance", &item.acceptance)?;
        if item.acceptance.is_empty() {
            bail!(
                "tasks.add `{}` requires at least one acceptance entry",
                item.key
            );
        }
        if !added_keys.insert(item.key.clone()) {
            bail!("tasks.add key `{}` is duplicated", item.key);
        }
        if existing_ids.contains(&item.key) {
            bail!(
                "tasks.add key `{}` conflicts with an existing task id",
                item.key
            );
        }
    }

    let orderable_existing_ids = tasks
        .iter()
        .filter(|task| task.state.is_live())
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let remaining_orderable = orderable_existing_ids
        .iter()
        .filter(|id| !removed.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut known_refs = remaining_orderable.clone();
    known_refs.extend(added_keys.iter().cloned());

    let order_set = plan.order.iter().cloned().collect::<BTreeSet<_>>();
    if order_set.len() != plan.order.len() {
        bail!("tasks.order contains duplicate entries");
    }
    for reference in &plan.order {
        if removed.contains(reference) {
            bail!("tasks.order references removed task `{reference}`");
        }
        if !known_refs.contains(reference) {
            bail!("tasks.order references unknown task or plan key `{reference}`");
        }
    }
    if order_set != known_refs {
        let missing = known_refs
            .difference(&order_set)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "tasks.order must list every remaining task and added key exactly once; missing: {missing}"
        );
    }
    for item in &plan.add {
        for dependency in &item.depends_on {
            if dependency == &item.key {
                bail!("tasks.add `{}` cannot depend on itself", item.key);
            }
            if removed.contains(dependency) {
                bail!(
                    "tasks.add `{}` depends on removed task `{dependency}`",
                    item.key
                );
            }
            if !known_refs.contains(dependency) {
                bail!(
                    "tasks.add `{}` depends on unknown task or key `{dependency}`",
                    item.key
                );
            }
        }
    }
    Ok(remaining_orderable
        .iter()
        .map(|id| (id.clone(), id.clone()))
        .collect())
}

fn apply_task_dependencies(
    paths: &MaestroPaths,
    plan: &ReconcilePlan,
    task_id_by_ref: &BTreeMap<String, String>,
    actor: Option<&str>,
) -> Result<()> {
    let actor = actor.unwrap_or("maestro");
    for item in &plan.tasks.add {
        let task_id = task_id_by_ref
            .get(&item.key)
            .with_context(|| format!("tasks.add key `{}` was not created", item.key))?;
        for dependency in &item.depends_on {
            let predecessor = task_id_by_ref
                .get(dependency)
                .with_context(|| format!("dependency `{dependency}` was not resolved"))?;
            let reason = format!("after dependency: {dependency} ({predecessor}) verified");
            let now = utc_now_timestamp();
            task::block_task(
                &paths.tasks_dir(),
                task_id,
                &reason,
                BlockerTarget::Task(predecessor.clone()),
                actor,
                &now,
            )?;
        }
    }
    for pair in plan.tasks.order.windows(2) {
        let predecessor_ref = &pair[0];
        let current_ref = &pair[1];
        let current = task_id_by_ref
            .get(current_ref)
            .with_context(|| format!("tasks.order ref `{current_ref}` was not resolved"))?;
        let predecessor = task_id_by_ref
            .get(predecessor_ref)
            .with_context(|| format!("tasks.order ref `{predecessor_ref}` was not resolved"))?;
        let reason = format!("after dependency: {predecessor_ref} ({predecessor}) verified");
        let now = utc_now_timestamp();
        task::block_task(
            &paths.tasks_dir(),
            current,
            &reason,
            BlockerTarget::Task(predecessor.clone()),
            actor,
            &now,
        )?;
    }
    Ok(())
}

fn apply_task_removals(paths: &MaestroPaths, remove: &[String], actor: Option<&str>) -> Result<()> {
    let actor = actor.unwrap_or("maestro");
    for id in remove {
        let task = task::load_task_record(&paths.tasks_dir(), id)?;
        if safely_deletable(&task) {
            if let Some(resolved) = card_store::resolve(paths, id)? {
                card_store::remove_resolved(&resolved)
                    .with_context(|| format!("failed to remove task card {id}"))?;
            }
            continue;
        }
        let now = utc_now_timestamp();
        task::transition_task(
            &paths.tasks_dir(),
            id,
            TaskState::Superseded,
            actor,
            &now,
            TransitionDetails {
                summary: Some("removed from reconciled task order".to_string()),
                ..TransitionDetails::default()
            },
        )?;
    }
    Ok(())
}

fn safely_deletable(task: &TaskRecord) -> bool {
    matches!(
        task.state,
        TaskState::Draft | TaskState::Exploring | TaskState::Ready
    ) && task.claimed_by.is_none()
        && task.blockers.is_empty()
        && task.state_history.is_empty()
        && task.verification.status.is_none()
        && task.verification.claim_checks.is_empty()
        && task.verification.proof_sources.is_empty()
}

fn task_items(paths: &MaestroPaths, feature_id: &str) -> Result<ReconcileTasks> {
    let filter = task::TaskFilter {
        feature_id: Some(feature_id.to_string()),
        include_terminal: true,
        ..Default::default()
    };
    let mut order = Vec::new();
    let items = task::filter_tasks(task::load_task_records(&paths.tasks_dir())?, &filter)
        .into_iter()
        .map(|task| {
            let is_live = task.state.is_live();
            let state = task.state.as_str().to_string();
            let depends_on = task
                .blockers
                .iter()
                .filter(|blocker| blocker.resolved_at.is_none())
                .filter_map(|blocker| blocker.blocked_ref.as_ref())
                .filter(|blocked_ref| blocked_ref.kind == task::BlockerKind::Task)
                .map(|blocked_ref| blocked_ref.id.clone())
                .collect();
            let id = task.id;
            if is_live {
                order.push(id.clone());
            }
            ReconcileTaskItem {
                id,
                title: task.title,
                state,
                acceptance: task.acceptance.checks,
                covers: task.covers,
                depends_on,
                history: ReconcileTaskHistory {
                    has_proof: task.verification.status.is_some()
                        || !task.verification.claim_checks.is_empty()
                        || !task.verification.proof_sources.is_empty(),
                    has_execution: task.claimed_at.is_some() || !task.state_history.is_empty(),
                },
            }
        })
        .collect::<Vec<_>>();
    let dependencies = items
        .iter()
        .flat_map(|task| {
            task.depends_on.iter().map(move |dependency| {
                json!({
                    "from": task.id,
                    "to": dependency,
                })
            })
        })
        .collect();
    Ok(ReconcileTasks {
        order,
        items,
        removed: Vec::new(),
        added_candidates: Vec::new(),
        dependencies,
    })
}

fn store_current_receipt(
    paths: &MaestroPaths,
    feature_id: &str,
    surface: &SurfaceLocator,
    mode: &str,
    plan_digest: Option<String>,
    actor: ReconcileActor,
    changed_fields: Vec<String>,
) -> Result<ReconcileReceipt> {
    let fingerprints = current_fingerprints(paths, feature_id)?;
    let payload = StoredReconcileReceipt {
        mode: mode.to_string(),
        surface: surface.clone(),
        fingerprints: fingerprints.clone(),
        changed_fields,
        plan_digest: plan_digest.clone(),
        created_at: utc_now_timestamp(),
        actor,
    };
    let payload_json =
        serde_json::to_string(&payload).context("failed to serialize reconcile receipt")?;
    let extension = proof::store_reconcile_receipt_extension(
        paths,
        &receipt_artifact_id(feature_id),
        Some(feature_id),
        &payload_json,
    )?;
    Ok(receipt_from_payload(
        extension.artifact,
        extension.created_at,
        payload,
        fingerprints,
    ))
}

fn load_reconcile_receipt(paths: &MaestroPaths, feature_id: &str) -> Result<ReconcileReceipt> {
    let Some(extension) = proof::load_receipt_extension(
        paths,
        proof::RECONCILE_RECEIPT_TYPE,
        &receipt_artifact_id(feature_id),
    )?
    else {
        return Ok(ReconcileReceipt::not_created());
    };
    let payload: StoredReconcileReceipt = serde_json::from_str(&extension.payload_json)
        .context("failed to parse reconcile receipt payload")?;
    let current = current_fingerprints(paths, feature_id)?;
    Ok(receipt_from_payload(
        extension.artifact,
        extension.created_at,
        payload,
        current,
    ))
}

fn receipt_from_payload(
    artifact: ArtifactLocator,
    created_at: String,
    payload: StoredReconcileReceipt,
    current: BTreeMap<String, String>,
) -> ReconcileReceipt {
    let mut stale = Vec::new();
    let mut fingerprints = BTreeMap::new();
    for category in FINGERPRINT_CATEGORIES {
        let receipt = payload.fingerprints.get(category).cloned();
        let current_value = current.get(category).cloned();
        if receipt != current_value {
            stale.push(category.to_string());
        }
        fingerprints.insert(
            category.to_string(),
            ReconcileFingerprint {
                receipt,
                current: current_value,
            },
        );
    }
    ReconcileReceipt {
        state: if stale.is_empty() {
            "current".to_string()
        } else {
            "stale".to_string()
        },
        mode: Some(payload.mode),
        fresh: stale.is_empty(),
        artifact: Some(artifact),
        fingerprints,
        stale,
        plan_digest: payload.plan_digest,
        created_at: Some(created_at),
        actor: Some(payload.actor),
    }
}

fn current_fingerprints(
    paths: &MaestroPaths,
    feature_id: &str,
) -> Result<BTreeMap<String, String>> {
    let view = registry::show(paths, feature_id)?;
    let tasks = task_items(paths, feature_id)?;
    let decisions = decisions::decisions_for_feature(paths, feature_id)?;
    let qa = registry::read_sidecar_text(paths, feature_id, "qa.md")?;
    let handoff = registry::read_sidecar_text(paths, feature_id, "handoff.md")?;
    let spec = registry::read_sidecar_text(paths, feature_id, "spec.md")?;
    let notes = registry::read_sidecar_text(paths, feature_id, "notes.md")?;
    let worktree = registry::read_sidecar_text(paths, feature_id, "worktree.yml")?;
    let mut fingerprints = BTreeMap::new();
    fingerprints.insert(
        "contract".to_string(),
        digest_json(&json!({
            "vision": view.raw_request,
            "description": view.description,
            "acceptance": view.acceptance,
            "non_goals": view.non_goals,
            "affected_areas": view.affected_areas,
        }))?,
    );
    fingerprints.insert(
        "decisions".to_string(),
        digest_json(&json!(
            decisions
                .into_iter()
                .map(|decision| json!({
                    "id": decision.id,
                    "status": decision.status.as_str(),
                    "decision": decision.decision,
                    "superseded_by": decision.superseded_by,
                }))
                .collect::<Vec<_>>()
        ))?,
    );
    fingerprints.insert(
        "questions".to_string(),
        digest_json(&json!({
            "open": view.open_questions,
        }))?,
    );
    fingerprints.insert("tasks".to_string(), digest_json(&json!(tasks))?);
    fingerprints.insert("qa".to_string(), digest_json(&json!(qa))?);
    fingerprints.insert(
        "handoff".to_string(),
        digest_json(&json!({
            "handoff": handoff,
            "spec": spec,
            "notes": notes,
            "worktree": worktree,
        }))?,
    );
    Ok(fingerprints)
}

fn digest_json(value: &Value) -> Result<String> {
    Ok(sha256_prefixed(&serde_json::to_vec(value)?))
}

fn receipt_artifact_id(feature_id: &str) -> String {
    format!("reconcile-receipt-{feature_id}")
}

fn finalize_receipt_error(id: &str, receipt: &ReconcileReceipt) -> String {
    let mut out = String::new();
    match receipt.state.as_str() {
        "not_created" => {
            out.push_str(&format!(
                "cannot finalize {id}: reconcile receipt is missing\n"
            ));
        }
        "stale" => {
            out.push_str(&format!(
                "cannot finalize {id}: reconcile receipt is stale\n"
            ));
            out.push_str("stale:\n");
            for category in &receipt.stale {
                let fingerprint = receipt.fingerprints.get(category);
                out.push_str(&format!(
                    "  - {category}: receipt={} current={}\n",
                    fingerprint
                        .and_then(|value| value.receipt.as_deref())
                        .map(digest_prefix)
                        .unwrap_or_else(|| "null".to_string()),
                    fingerprint
                        .and_then(|value| value.current.as_deref())
                        .map(digest_prefix)
                        .unwrap_or_else(|| "null".to_string())
                ));
            }
        }
        other => {
            out.push_str(&format!(
                "cannot finalize {id}: reconcile receipt is {other}\n"
            ));
        }
    }
    out.push_str("next:\n");
    out.push_str(&format!("  maestro feature reconcile {id}\n"));
    out.push_str("details:\n");
    out.push_str(&format!("  maestro feature reconcile {id} --full\n"));
    out.push_str(&format!("  maestro feature reconcile {id} --json"));
    out
}

fn digest_prefix(value: &str) -> String {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return value.chars().take(12).collect();
    };
    format!("sha256:{}", hex.chars().take(12).collect::<String>())
}

fn artifact_summary(kind: &str, contents: Option<String>) -> ReconcileArtifactSummary {
    let digest = contents
        .as_ref()
        .map(|contents| sha256_prefixed(contents.as_bytes()));
    ReconcileArtifactSummary {
        present: contents.is_some(),
        digest,
        refs: vec![json!({"type": kind})],
    }
}

fn text_items(
    values: Vec<String>,
    prefix: &str,
    stable_id: Option<fn(usize) -> String>,
) -> Vec<ReconcileTextItem> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, text)| ReconcileTextItem {
            id: stable_id
                .map(|id_for| id_for(index))
                .unwrap_or_else(|| format!("{prefix}-{}", index + 1)),
            text,
        })
        .collect()
}

fn reconcile_next(id: &str, status: &ReconcileStatus) -> Vec<ReconcileAction> {
    match status {
        ReconcileStatus::Clean => vec![ReconcileAction::command(
            "Finalize the reconciled feature.",
            format!("maestro feature finalize {id}"),
        )],
        ReconcileStatus::Applied => vec![ReconcileAction::command(
            "Finalize the reconciled feature.",
            format!("maestro feature finalize {id}"),
        )],
        ReconcileStatus::StaleReceipt => vec![ReconcileAction::command(
            "Refresh the stale reconcile receipt.",
            format!("maestro feature reconcile {id}"),
        )],
        ReconcileStatus::ChangesRequired => vec![
            ReconcileAction::read(
                "Read full human context before authoring the plan.",
                format!("maestro feature reconcile {id} --full"),
            ),
            ReconcileAction::read(
                "Read full agent context before authoring the plan.",
                format!("maestro feature reconcile {id} --json"),
            ),
            ReconcileAction::manual(
                "Human or authorized agent chooses the intended contract and task order.",
            ),
            ReconcileAction::plan_change(
                "Write the reviewed full-contract reconcile.yml.",
                json!({"target": "reconcile.yml"}),
            ),
            ReconcileAction::command(
                "Apply the reviewed reconcile plan.",
                format!("maestro feature reconcile {id} --apply-plan reconcile.yml"),
            ),
        ],
        ReconcileStatus::Error => Vec::new(),
    }
}

impl ReconcileAction {
    fn read(description: &str, command: String) -> Self {
        Self {
            kind: "read".to_string(),
            description: description.to_string(),
            command: Some(command),
            plan_change: None,
        }
    }

    fn command(description: &str, command: String) -> Self {
        Self {
            kind: "command".to_string(),
            description: description.to_string(),
            command: Some(command),
            plan_change: None,
        }
    }

    fn manual(description: &str) -> Self {
        Self {
            kind: "manual".to_string(),
            description: description.to_string(),
            command: None,
            plan_change: None,
        }
    }

    fn plan_change(description: &str, plan_change: Value) -> Self {
        Self {
            kind: "plan_change".to_string(),
            description: description.to_string(),
            command: None,
            plan_change: Some(plan_change),
        }
    }
}

impl ReconcileReceipt {
    fn not_created() -> Self {
        Self {
            state: "not_created".to_string(),
            mode: None,
            fresh: false,
            artifact: None,
            fingerprints: BTreeMap::new(),
            stale: Vec::new(),
            plan_digest: None,
            created_at: None,
            actor: None,
        }
    }
}

fn ensure_not_blank(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(())
}

fn ensure_no_blank_values(field: &str, values: &[String]) -> Result<()> {
    if values.iter().any(|value| value.trim().is_empty()) {
        bail!("{field} values must not be empty");
    }
    Ok(())
}

fn rollback_created_tasks(paths: &MaestroPaths, created: &[TaskRecord]) -> Result<()> {
    for task in created.iter().rev() {
        if let Some(resolved) = card_store::resolve(paths, &task.id)? {
            card_store::remove_resolved(&resolved)
                .with_context(|| format!("failed to remove task card {}", task.id))?;
        }
    }
    Ok(())
}
