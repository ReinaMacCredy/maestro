use std::env;
use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::domain::{card, feature, loop_recipes, run, task};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::{timestamp_nanos, utc_now_timestamp};
use crate::interfaces::cli::{GitReadout, LoopArgs, LoopCommand, LoopNextArgs, WorkLeaseArgs};
use crate::interfaces::hooks::record;
use crate::operations::harness;
use crate::operations::memory::{
    self, ApprovedMemory, MemoryReadScope, MemoryReadSurface, MemorySuggestionHint,
    MemorySuggestionSet,
};

const WORK_LEASE_JSON_SCHEMA: &str = "maestro.work_lease.v1";
const WORK_LEASE_JSON_VERSION: u8 = 1;
const DEFAULT_HARD_STOPS: &[&str] = &[
    "external ship action not listed in ship_authority.allowed_external_actions",
    "destructive git",
    "secret rotation",
    "platform/tool approval failure",
    "hand-editing card.yaml or guarded sidecars",
];
const FOLLOW_UP_VERBS: &[&str] = &[
    "maestro card show <id> --json",
    "maestro status --json",
    "maestro card note <id> <text>",
    "maestro task complete <id> --summary <summary> --claim <claim> --proof <proof>",
    "maestro task verify <id>",
    "maestro task block <id> --reason <reason>",
    "maestro query run --json",
];
const RECURRENCE_EVIDENCE: &[&str] = &[
    "regression test",
    "proof gate",
    "QA checklist entry",
    "harness friction rule",
    "skill guidance update",
    "locked decision",
];
const WORK_LEASE_RESTART_POLICY: &str = "Cold-start from the card store plus the run ledger: rerun the inspect/status/reconcile handles; no daemon, queue, scheduler, executor, or hidden store exists.";

/// Execute `maestro loop [list | show <name>]`: print the recipe index (the
/// default and `list`), or one recipe verbatim. Served from the binary, so it
/// needs no `.maestro` repo.
pub fn run(args: LoopArgs) -> Result<()> {
    let custom_dir = custom_recipe_dir();
    match args.command {
        None | Some(LoopCommand::List) => {
            print!(
                "{}",
                loop_recipes::index_with_custom_dir(custom_dir.as_deref())?
            )
        }
        Some(LoopCommand::Show { name }) => {
            print!(
                "{}",
                loop_recipes::show_with_custom_dir(&name, custom_dir.as_deref())?
            )
        }
        Some(LoopCommand::Validate { name }) => print!(
            "{}",
            loop_recipes::validate_with_custom_dir(&name, custom_dir.as_deref())?
        ),
        Some(LoopCommand::Next(args)) => run_next(args)?,
        Some(LoopCommand::WorkLease(args)) => run_work_lease(*args)?,
    }
    Ok(())
}

fn custom_recipe_dir() -> Option<PathBuf> {
    let repo_root = discover_repo_root().ok()?;
    let paths = MaestroPaths::new(repo_root);
    Some(paths.loop_recipes_dir())
}

fn run_next(args: LoopNextArgs) -> Result<()> {
    let report = build_loop_next_report()?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_loop_next(&report);
    }
    Ok(())
}

fn build_loop_next_report() -> Result<loop_recipes::LoopNextReport> {
    let repo_root = discover_repo_root().or_else(|_| env::current_dir())?;
    let paths = MaestroPaths::new(repo_root);
    build_loop_next_report_for_paths(&paths)
}

pub(crate) fn build_loop_next_report_for_paths(
    paths: &MaestroPaths,
) -> Result<loop_recipes::LoopNextReport> {
    if !paths.maestro_dir().is_dir() {
        return loop_recipes::route_next(loop_recipes::LoopRouterInput {
            repo: paths.repo_root().display().to_string(),
            initialized: false,
            ..loop_recipes::LoopRouterInput::default()
        });
    }

    let mut warnings = Vec::new();
    let task_entries = match task::load_task_entries(&paths.tasks_dir()) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("task scan failed: {error:#}"));
            Vec::new()
        }
    };
    let mut features = Vec::new();
    for entry in feature::list_tolerant_with_entries(paths, &task_entries) {
        match entry {
            feature::FeatureRosterEntry::Loaded(view) => features.push(*view),
            feature::FeatureRosterEntry::Unreadable {
                id, path, error, ..
            } => {
                warnings.push(format!(
                    "feature {id} at {} is unreadable: {error}",
                    path.display()
                ));
            }
        }
    }

    let git = super::git_readout(paths);
    build_loop_next_report_from_snapshot(paths, &task_entries, &features, git.as_ref(), warnings)
}

pub(crate) fn build_loop_next_report_from_snapshot(
    paths: &MaestroPaths,
    task_entries: &[task::TaskEntry],
    features: &[feature::FeatureView],
    git: Option<&GitReadout>,
    mut warnings: Vec<String>,
) -> Result<loop_recipes::LoopNextReport> {
    let tasks = task_entries
        .iter()
        .map(|entry| loop_recipes::LoopTaskInput {
            id: entry.task.id.clone(),
            title: entry.task.title.clone(),
            state: entry.task.state.as_str().to_string(),
            feature_id: entry.task.feature_id.clone(),
            blocked: task::has_unresolved_blockers(&entry.task),
        })
        .collect::<Vec<_>>();
    let current_task = current_loop_task(task_entries);
    let features = features
        .iter()
        .map(|view| loop_recipes::LoopFeatureInput {
            id: view.id.clone(),
            title: view.title.clone(),
            status: view.status.as_str().to_string(),
            total_tasks: view.counts.total,
            verified_tasks: view.counts.verified,
            open_questions: view.open_questions.len(),
        })
        .collect::<Vec<_>>();
    let now = utc_now_timestamp();
    let roots = super::worktree_roots(paths);
    let active_sessions = match run::active_sessions_union(&roots, &now) {
        Ok(sessions) => sessions
            .iter()
            .filter(|session| session.presence != run::Presence::Stale)
            .count(),
        Err(error) => {
            warnings.push(format!("active session scan failed: {error:#}"));
            0
        }
    };

    loop_recipes::route_next(loop_recipes::LoopRouterInput {
        repo: paths.repo_root().display().to_string(),
        initialized: true,
        current_task,
        tasks,
        features,
        active_sessions,
        git: git.map(|git| loop_recipes::LoopGitInput {
            branch: git.branch.clone(),
            code_other_dirty: git.code_other_dirty,
            maestro_dirty: git.maestro_dirty,
            ahead: git
                .divergence
                .as_ref()
                .map(|divergence| divergence.ahead)
                .unwrap_or(0),
            behind: git
                .divergence
                .as_ref()
                .map(|divergence| divergence.behind)
                .unwrap_or(0),
        }),
        warnings,
    })
}

fn current_loop_task(entries: &[task::TaskEntry]) -> Option<loop_recipes::LoopTaskInput> {
    let id = env::var("MAESTRO_CURRENT_TASK").ok()?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let task = entries
        .iter()
        .find(|entry| entry.task.id == id && entry.task.state.is_live())?;
    Some(loop_recipes::LoopTaskInput {
        id: task.task.id.clone(),
        title: task.task.title.clone(),
        state: task.task.state.as_str().to_string(),
        feature_id: task.task.feature_id.clone(),
        blocked: task::has_unresolved_blockers(&task.task),
    })
}

fn print_loop_next(report: &loop_recipes::LoopNextReport) {
    if let Some(recipe) = report.recommended_recipe.as_deref() {
        println!("recipe: {recipe}");
        println!("status: {}", report.recommended_status);
    } else {
        println!("recipe: <uncertain>");
        println!("status: uncertain");
    }
    println!("confidence: {}", report.confidence);
    println!("priority: {}", report.priority);
    println!("reason: {}", report.reason);
    print_loop_next_list("authority_scope", &report.authority_scope);
    print_loop_next_list("autonomy", &report.autonomy);
    if !report.edges.is_empty() {
        println!("edges:");
        for edge in &report.edges {
            println!("- {}: {} -> {}", edge.kind, edge.trigger, edge.to);
        }
    }
    print_loop_next_list("hard_stops", &report.hard_stops);
    print_loop_next_list("inspect", &report.inspect);
    print_loop_next_list("next_verbs", &report.next_verbs);
}

fn print_loop_next_list(label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    println!("{label}:");
    for value in values {
        println!("- {value}");
    }
}

fn run_work_lease(args: WorkLeaseArgs) -> Result<()> {
    let _json = args.json;
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let now = utc_now_timestamp();
    let scope = LeaseScopeJson {
        repo: paths.repo_root().display().to_string(),
        feature: args.feature.clone(),
        project: args.project.clone(),
    };
    let run_id = super::cli_run_id();
    let run_event_path = format!(".maestro/runs/{}/events.jsonl", run::run_dir_name(&run_id));
    let ship_authority = ShipAuthorityJson::from_args(&args, &now);
    let lease_memory_scope = MemoryReadScope {
        feature_id: args.feature.clone(),
        project: args.project.clone(),
        ..MemoryReadScope::default()
    };

    if !paths.cards_dir().is_dir() {
        let memory_suggestions =
            memory::suggestion_hints(&paths, MemoryReadSurface::WorkLease, lease_memory_scope)?;
        print_work_lease(
            &paths,
            WorkLeaseJson::dry(
                "no_card_store",
                "this repo has no card store yet (.maestro/cards/)",
                scope,
                ship_authority,
                run_event_path,
                memory_suggestions,
            ),
        )?;
        return Ok(());
    }

    let cards = card::query::scan(&paths)?;
    let mut ready = card::query::ready(&cards);
    if let Some(feature) = args.feature.as_deref() {
        ready.retain(|candidate| candidate.parent.as_deref() == Some(feature));
    }
    if let Some(project) = args.project.as_deref() {
        ready.retain(|candidate| candidate.project.as_deref() == Some(project));
    }

    if ready.is_empty() {
        let memory_suggestions = memory::suggestion_hints(
            &paths,
            MemoryReadSurface::WorkLease,
            lease_memory_scope.clone(),
        )?;
        print_work_lease(
            &paths,
            WorkLeaseJson::dry(
                "no_ready_work",
                "no ready cards matched this lease scope",
                scope,
                ship_authority,
                run_event_path,
                memory_suggestions,
            ),
        )?;
        return Ok(());
    }

    let identity = claim_identity();
    let mut blocked = Vec::new();
    for (index, candidate) in ready.iter().enumerate() {
        let rank = index + 1;
        let before = (*candidate).clone();
        let mut claim_probe = before.clone();
        match card::edit::apply_claim(&mut claim_probe, &identity, &now) {
            Ok(_) => {}
            Err(error) if live_claim_error(&error) => {
                blocked.push(BlockedCardJson::new(rank, &before, error.to_string()));
                continue;
            }
            Err(error) => return Err(error.context(format!("failed to lease {}", before.id))),
        }
        let approved_lessons = memory::approved_memory(
            &paths,
            MemoryReadSurface::WorkLease,
            MemoryReadScope {
                card_id: Some(before.id.clone()),
                feature_id: before.parent.clone(),
                project: before.project.clone(),
                ..MemoryReadScope::default()
            },
        )?;
        let memory_suggestions = memory::suggestion_hints(
            &paths,
            MemoryReadSurface::WorkLease,
            MemoryReadScope {
                card_id: Some(before.id.clone()),
                feature_id: before.parent.clone(),
                project: before.project.clone(),
                ..MemoryReadScope::default()
            },
        )?;
        match card::edit::claim(&paths, &before.id, &identity, &now) {
            Ok(outcome) => {
                super::emit_work_touch(&paths, &before.id);
                emit_work_lease_action(
                    &paths,
                    &run_id,
                    &before,
                    &ship_authority,
                    LeaseActionEvent {
                        action: "work_lease_acquire",
                        before_state: &before.status,
                        result: "leased",
                        after_state: "in_progress",
                    },
                );
                print_work_lease(
                    &paths,
                    WorkLeaseJson::leased(
                        LeasedSelection {
                            rank,
                            card: &before,
                            claimed_by: &identity,
                            now: &now,
                            outcome: &outcome,
                        },
                        scope,
                        ship_authority,
                        run_event_path,
                        approved_lessons.memories,
                        memory_suggestions,
                    ),
                )?;
                return Ok(());
            }
            Err(error) if live_claim_error(&error) => {
                blocked.push(BlockedCardJson::new(rank, &before, error.to_string()));
            }
            Err(error) => return Err(error.context(format!("failed to lease {}", before.id))),
        }
    }

    emit_blocked_work_lease_action(&paths, &run_id, &ship_authority);
    let memory_suggestions =
        memory::suggestion_hints(&paths, MemoryReadSurface::WorkLease, lease_memory_scope)?;
    print_work_lease(
        &paths,
        WorkLeaseJson::blocked(
            "all ready cards are held by live claims",
            blocked,
            scope,
            ship_authority,
            run_event_path,
            memory_suggestions,
        ),
    )?;
    Ok(())
}

fn claim_identity() -> String {
    let agent = match super::detected_agent_hint() {
        "claude" => "claude",
        "codex" => "codex",
        _ => "maestro",
    };
    format!("{agent}#{}", super::claim_session())
}

fn live_claim_error(error: &anyhow::Error) -> bool {
    error.to_string().contains("not stale yet")
}

fn print_work_lease(paths: &MaestroPaths, report: WorkLeaseJson) -> Result<()> {
    let mut value = serde_json::to_value(&report)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "scheduler".to_string(),
            serde_json::to_value(harness::scheduler_readout(paths)?)?,
        );
    }
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn emit_work_lease_action(
    paths: &MaestroPaths,
    run_id: &str,
    card: &card::schema::Card,
    authority: &ShipAuthorityJson,
    event: LeaseActionEvent<'_>,
) {
    let payload = json!({
        "event": "autonomy_action",
        "session_id": run_id,
        "action": event.action,
        "target_kind": card.card_type.as_str(),
        "target_id": card.id,
        "authority_ref": authority.authority_ref.as_deref().unwrap_or("absent"),
        "before_state": event.before_state,
        "command": "maestro loop work-lease --json",
        "result": event.result,
        "after_state": event.after_state,
        "agent": super::actor(),
    });
    if let Err(error) = record::record_value(paths, &payload) {
        eprintln!("maestro: work-lease run-event note failed: {error:#}");
    }
}

fn emit_blocked_work_lease_action(
    paths: &MaestroPaths,
    run_id: &str,
    authority: &ShipAuthorityJson,
) {
    let payload = json!({
        "event": "autonomy_action",
        "session_id": run_id,
        "action": "work_lease_blocked",
        "target_kind": "card",
        "target_id": "<ready>",
        "authority_ref": authority.authority_ref.as_deref().unwrap_or("absent"),
        "before_state": "ready",
        "command": "maestro loop work-lease --json",
        "result": "blocked_live_claims",
        "after_state": "ready",
        "agent": super::actor(),
    });
    if let Err(error) = record::record_value(paths, &payload) {
        eprintln!("maestro: work-lease run-event note failed: {error:#}");
    }
}

struct LeaseActionEvent<'a> {
    action: &'static str,
    before_state: &'a str,
    result: &'static str,
    after_state: &'static str,
}

#[derive(Serialize)]
struct WorkLeaseJson {
    version: u8,
    schema: &'static str,
    helper: WorkLeaseHelperJson,
    status: &'static str,
    scope: LeaseScopeJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    lease: Option<LeaseJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_card: Option<LeaseCardJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_action: Option<SelectedActionJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claim: Option<ClaimJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    blocked_cards: Vec<BlockedCardJson>,
    hard_stops: Vec<String>,
    allowed_follow_up_verbs: Vec<String>,
    ship_authority: ShipAuthorityJson,
    recurrence_guard: RecurrenceGuardJson,
    handles: LeaseHandlesJson,
    inspect: InspectJson,
    run_events: RunEventsJson,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    approved_lessons: Vec<ApprovedMemory>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    memory_suggestions: Vec<MemorySuggestionHint>,
    #[serde(skip_serializing_if = "is_zero")]
    memory_suggestions_omitted: usize,
    worker_prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl WorkLeaseJson {
    fn leased(
        selection: LeasedSelection<'_>,
        scope: LeaseScopeJson,
        ship_authority: ShipAuthorityJson,
        run_event_path: String,
        approved_lessons: Vec<ApprovedMemory>,
        memory_suggestions: MemorySuggestionSet,
    ) -> Self {
        let card = selection.card;
        let now = selection.now;
        let lease_id = lease_id(card, now);
        let worker_prompt = worker_prompt(
            card.id.as_str(),
            &ship_authority,
            &approved_lessons,
            &memory_suggestions.suggestions,
        );
        let memory_suggestions_omitted = memory_suggestions.omitted;
        Self {
            version: WORK_LEASE_JSON_VERSION,
            schema: WORK_LEASE_JSON_SCHEMA,
            helper: WorkLeaseHelperJson::default(),
            status: "leased",
            scope,
            lease: Some(LeaseJson {
                id: lease_id.clone(),
                acquired_at: now.to_string(),
                stale_after_seconds: card::edit::STALE_CLAIM_AGE_SECONDS,
                stale_policy: format!(
                    "a later lease may reclaim this card after {} seconds using the existing card claim policy",
                    card::edit::STALE_CLAIM_AGE_SECONDS
                ),
            }),
            selected_card: Some(LeaseCardJson::new(selection.rank, card)),
            selected_action: Some(SelectedActionJson {
                kind: "work_card",
                command: format!("maestro card show {} --json", card.id),
                scope: "one ready card",
            }),
            claim: Some(ClaimJson {
                claimed_by: selection.claimed_by.to_string(),
                claimed_at: now.to_string(),
                outcome: claim_outcome(selection.outcome),
            }),
            blocked_cards: Vec::new(),
            hard_stops: hard_stops(),
            allowed_follow_up_verbs: follow_up_verbs(),
            ship_authority,
            recurrence_guard: RecurrenceGuardJson::default(),
            handles: LeaseHandlesJson::new(Some(card.id.as_str()), run_event_path.clone()),
            inspect: InspectJson::new(Some(card.id.as_str())),
            run_events: RunEventsJson::new(run_event_path),
            approved_lessons,
            memory_suggestions: memory_suggestions.suggestions,
            memory_suggestions_omitted,
            worker_prompt,
            reason: None,
        }
    }

    fn dry(
        reason_kind: &'static str,
        reason: &str,
        scope: LeaseScopeJson,
        ship_authority: ShipAuthorityJson,
        run_event_path: String,
        memory_suggestions: MemorySuggestionSet,
    ) -> Self {
        let memory_suggestions_omitted = memory_suggestions.omitted;
        Self {
            version: WORK_LEASE_JSON_VERSION,
            schema: WORK_LEASE_JSON_SCHEMA,
            helper: WorkLeaseHelperJson::default(),
            status: "dry",
            scope,
            lease: None,
            selected_card: None,
            selected_action: None,
            claim: None,
            blocked_cards: Vec::new(),
            hard_stops: hard_stops(),
            allowed_follow_up_verbs: follow_up_verbs(),
            ship_authority,
            recurrence_guard: RecurrenceGuardJson::default(),
            handles: LeaseHandlesJson::new(None, run_event_path.clone()),
            inspect: InspectJson::new(None),
            run_events: RunEventsJson::new(run_event_path),
            approved_lessons: Vec::new(),
            memory_suggestions: memory_suggestions.suggestions,
            memory_suggestions_omitted,
            worker_prompt: format!(
                "No work lease was acquired ({reason_kind}). Reconcile with `maestro card ready`, `maestro feature list`, and `maestro query run --json`; do not launch a worker."
            ),
            reason: Some(reason.to_string()),
        }
    }

    fn blocked(
        reason: &str,
        blocked_cards: Vec<BlockedCardJson>,
        scope: LeaseScopeJson,
        ship_authority: ShipAuthorityJson,
        run_event_path: String,
        memory_suggestions: MemorySuggestionSet,
    ) -> Self {
        let memory_suggestions_omitted = memory_suggestions.omitted;
        Self {
            version: WORK_LEASE_JSON_VERSION,
            schema: WORK_LEASE_JSON_SCHEMA,
            helper: WorkLeaseHelperJson::default(),
            status: "blocked",
            scope,
            lease: None,
            selected_card: None,
            selected_action: None,
            claim: None,
            blocked_cards,
            hard_stops: hard_stops(),
            allowed_follow_up_verbs: follow_up_verbs(),
            ship_authority,
            recurrence_guard: RecurrenceGuardJson::default(),
            handles: LeaseHandlesJson::new(None, run_event_path.clone()),
            inspect: InspectJson::new(None),
            run_events: RunEventsJson::new(run_event_path),
            approved_lessons: Vec::new(),
            memory_suggestions: memory_suggestions.suggestions,
            memory_suggestions_omitted,
            worker_prompt: "No work lease was acquired because ready cards are actively claimed. Reconcile with `maestro active`, linked-card messages, and `maestro query run --json`; do not steal live work.".to_string(),
            reason: Some(reason.to_string()),
        }
    }
}

#[derive(Serialize)]
struct WorkLeaseHelperJson {
    role: &'static str,
    phase: &'static str,
    parent_recipe: &'static str,
    selection_limit: &'static str,
    persistence: &'static str,
    hard_boundary: &'static str,
}

impl Default for WorkLeaseHelperJson {
    fn default() -> Self {
        Self {
            role: "internal_choose_phase_helper",
            phase: "choose",
            parent_recipe: "unattended",
            selection_limit: "exactly_one_ready_unit",
            persistence: "current Maestro card store plus run ledger evidence",
            hard_boundary: "not a top-level lifecycle, daemon, scheduler, executor, queue, worker launcher, or hidden store",
        }
    }
}

struct LeasedSelection<'a> {
    rank: usize,
    card: &'a card::schema::Card,
    claimed_by: &'a str,
    now: &'a str,
    outcome: &'a card::edit::ClaimOutcome,
}

#[derive(Serialize)]
struct LeaseScopeJson {
    repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    feature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
}

#[derive(Serialize)]
struct LeaseJson {
    id: String,
    acquired_at: String,
    stale_after_seconds: u64,
    stale_policy: String,
}

#[derive(Serialize)]
struct LeaseCardJson {
    rank: usize,
    id: String,
    #[serde(rename = "type")]
    card_type: &'static str,
    title: String,
    status_before_claim: String,
    status_after_claim: &'static str,
    parent: Option<String>,
    project: Option<String>,
}

impl LeaseCardJson {
    fn new(rank: usize, card: &card::schema::Card) -> Self {
        Self {
            rank,
            id: card.id.clone(),
            card_type: card.card_type.as_str(),
            title: card.title.clone(),
            status_before_claim: card.status.clone(),
            status_after_claim: "in_progress",
            parent: card.parent.clone(),
            project: card.project.clone(),
        }
    }
}

#[derive(Serialize)]
struct SelectedActionJson {
    kind: &'static str,
    command: String,
    scope: &'static str,
}

#[derive(Serialize)]
struct ClaimJson {
    claimed_by: String,
    claimed_at: String,
    outcome: &'static str,
}

#[derive(Serialize)]
struct BlockedCardJson {
    rank: usize,
    id: String,
    #[serde(rename = "type")]
    card_type: &'static str,
    title: String,
    claimed_by: Option<String>,
    claimed_at: Option<String>,
    reason: String,
}

impl BlockedCardJson {
    fn new(rank: usize, card: &card::schema::Card, reason: String) -> Self {
        Self {
            rank,
            id: card.id.clone(),
            card_type: card.card_type.as_str(),
            title: card.title.clone(),
            claimed_by: card.claimed_by.clone(),
            claimed_at: card.claimed_at.clone(),
            reason,
        }
    }
}

#[derive(Serialize)]
struct ShipAuthorityJson {
    status: &'static str,
    external_ship_allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    allowed_external_actions: Vec<String>,
    required_evidence: Vec<String>,
    hard_stops: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    reason: String,
}

impl ShipAuthorityJson {
    fn from_args(args: &WorkLeaseArgs, now: &str) -> Self {
        let any_authority = args.authority_ref.is_some()
            || args.authority_summary.is_some()
            || args.authority_scope.is_some()
            || args.authority_target.is_some()
            || !args.allow_external_actions.is_empty()
            || !args.required_evidence.is_empty()
            || args.authority_expires_at.is_some()
            || !args.authority_hard_stops.is_empty();
        let allowed_external_actions = clean_list(&args.allow_external_actions);
        let required_evidence = clean_list(&args.required_evidence);
        let hard_stops = if args.authority_hard_stops.is_empty() {
            hard_stops()
        } else {
            clean_list(&args.authority_hard_stops)
        };
        let mut authority = Self {
            status: "absent",
            external_ship_allowed: false,
            authority_ref: clean_opt(&args.authority_ref),
            authority_summary: clean_opt(&args.authority_summary),
            scope: clean_opt(&args.authority_scope),
            target: clean_opt(&args.authority_target),
            allowed_external_actions,
            required_evidence,
            hard_stops,
            expires_at: clean_opt(&args.authority_expires_at),
            reason: "no explicit run-scoped ship authority was provided; push, release, publish, tag, archive, and external ship actions are hard stops".to_string(),
        };
        if !any_authority {
            return authority;
        }
        if authority.has_missing_required_fields() {
            authority.status = "ambiguous";
            authority.reason = "partial ship authority is not enough; provide ref, summary, scope, target, allowed external actions, and required evidence".to_string();
            return authority;
        }
        if authority.expires_at.as_deref().is_some_and(|expires_at| {
            timestamp_nanos(expires_at)
                .zip(timestamp_nanos(now))
                .is_none_or(|(expires_at, now)| expires_at <= now)
        }) {
            authority.status = "stale";
            authority.reason =
                "ship authority is expired or has an unparsable expiry timestamp".to_string();
            return authority;
        }
        if authority
            .allowed_external_actions
            .iter()
            .any(|action| overbroad_action(action))
        {
            authority.status = "overbroad";
            authority.reason =
                "ship authority must name concrete external actions, not all/everything/*"
                    .to_string();
            return authority;
        }
        authority.status = "explicit";
        authority.external_ship_allowed = true;
        authority.reason =
            "explicit bounded run-scoped authority is present; only listed external actions may be used after required evidence is satisfied".to_string();
        authority
    }

    fn has_missing_required_fields(&self) -> bool {
        self.authority_ref.is_none()
            || self.authority_summary.is_none()
            || self.scope.is_none()
            || self.target.is_none()
            || self.allowed_external_actions.is_empty()
            || self.required_evidence.is_empty()
    }
}

#[derive(Serialize)]
struct RecurrenceGuardJson {
    required: bool,
    completion_gate: &'static str,
    acceptable_evidence: Vec<String>,
}

impl Default for RecurrenceGuardJson {
    fn default() -> Self {
        Self {
            required: true,
            completion_gate: "if the worker fixes any issue discovered during the loop, completion or ship must include durable recurrence-guard evidence",
            acceptable_evidence: RECURRENCE_EVIDENCE
                .iter()
                .map(|item| item.to_string())
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct LeaseHandlesJson {
    inspect: InspectHandlesJson,
    status: StatusHandlesJson,
    reconcile: ReconcileHandlesJson,
    restart_policy: &'static str,
}

impl LeaseHandlesJson {
    fn new(card_id: Option<&str>, run_event_path: String) -> Self {
        Self {
            inspect: InspectHandlesJson::new(card_id),
            status: StatusHandlesJson::new(card_id),
            reconcile: ReconcileHandlesJson::new(run_event_path),
            restart_policy: WORK_LEASE_RESTART_POLICY,
        }
    }
}

#[derive(Serialize)]
struct InspectHandlesJson {
    repo: &'static str,
    ready_queue: &'static str,
    active_sessions: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_card: Option<String>,
}

impl InspectHandlesJson {
    fn new(card_id: Option<&str>) -> Self {
        Self {
            repo: "maestro status --json",
            ready_queue: "maestro card ready --json",
            active_sessions: "maestro active",
            selected_card: card_id.map(|id| format!("maestro card show {id} --json")),
        }
    }
}

#[derive(Serialize)]
struct StatusHandlesJson {
    repo: &'static str,
    ready_queue: &'static str,
    active_sessions: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    claim: Option<String>,
}

impl StatusHandlesJson {
    fn new(card_id: Option<&str>) -> Self {
        Self {
            repo: "maestro status --json",
            ready_queue: "maestro card ready --json",
            active_sessions: "maestro active",
            claim: card_id.map(|id| format!("maestro card show {id} --json")),
        }
    }
}

#[derive(Serialize)]
struct ReconcileHandlesJson {
    run_report: &'static str,
    run_events_jsonl: String,
    active_sessions: &'static str,
    ready_queue: &'static str,
}

impl ReconcileHandlesJson {
    fn new(run_events_jsonl: String) -> Self {
        Self {
            run_report: "maestro query run --json",
            run_events_jsonl,
            active_sessions: "maestro active",
            ready_queue: "maestro card ready --json",
        }
    }
}

#[derive(Serialize)]
struct InspectJson {
    status: &'static str,
    ready: &'static str,
    active: &'static str,
    query_run: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    card: Option<String>,
    reconcile: Vec<String>,
}

impl InspectJson {
    fn new(card_id: Option<&str>) -> Self {
        Self {
            status: "maestro status --json",
            ready: "maestro card ready --json",
            active: "maestro active",
            query_run: "maestro query run --json",
            card: card_id.map(|id| format!("maestro card show {id} --json")),
            reconcile: vec![
                "maestro active".to_string(),
                "maestro card ready --json".to_string(),
                "maestro query run --json".to_string(),
            ],
        }
    }
}

#[derive(Serialize)]
struct RunEventsJson {
    events_jsonl: String,
    record_autonomy_start: &'static str,
    record_autonomy_action: &'static str,
    report: &'static str,
}

impl RunEventsJson {
    fn new(events_jsonl: String) -> Self {
        Self {
            events_jsonl,
            record_autonomy_start: "maestro hook record --event autonomy_start --session <run>",
            record_autonomy_action: "maestro hook record --event autonomy_action --session <run>",
            report: "maestro query run --json",
        }
    }
}

fn lease_id(card: &card::schema::Card, now: &str) -> String {
    let stamp = now.replace([':', '.'], "-");
    format!("wl-{}-{stamp}", card.id)
}

fn claim_outcome(outcome: &card::edit::ClaimOutcome) -> &'static str {
    match outcome {
        card::edit::ClaimOutcome::Claimed => "claimed",
        card::edit::ClaimOutcome::AlreadyMine => "already_mine",
        card::edit::ClaimOutcome::Reclaimed { .. } => "reclaimed_stale",
    }
}

fn hard_stops() -> Vec<String> {
    DEFAULT_HARD_STOPS
        .iter()
        .map(|stop| (*stop).to_string())
        .collect()
}

fn follow_up_verbs() -> Vec<String> {
    FOLLOW_UP_VERBS
        .iter()
        .map(|verb| (*verb).to_string())
        .collect()
}

fn clean_opt(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn clean_list(values: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !cleaned.iter().any(|existing| existing == value) {
            cleaned.push(value.to_string());
        }
    }
    cleaned
}

fn overbroad_action(action: &str) -> bool {
    let normalized = action.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "*" | "all" | "everything" | "ship" | "external"
    )
}

fn worker_prompt(
    card_id: &str,
    authority: &ShipAuthorityJson,
    lessons: &[ApprovedMemory],
    suggestions: &[MemorySuggestionHint],
) -> String {
    let ship_line = if authority.external_ship_allowed {
        "External ship actions are allowed only for ship_authority.allowed_external_actions after required_evidence is satisfied."
    } else {
        "Do not push, release, publish, tag, archive, or perform any external ship action."
    };
    let memory_line = if lessons.is_empty() {
        String::new()
    } else {
        let summaries = lessons
            .iter()
            .take(MemoryReadSurface::WorkerPrompt.cap())
            .map(|memory| {
                format!(
                    "{}: {:?} ({})",
                    memory.id, memory.summary, memory.show_command
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!(
            " Approved Memory is advisory only and lower priority than live user instruction, acceptance, Proof/QA, and run authority: {summaries}."
        )
    };
    let suggestion_line = if suggestions.is_empty() {
        String::new()
    } else {
        let summaries = suggestions
            .iter()
            .take(MemoryReadSurface::WorkerPrompt.cap())
            .map(|suggestion| {
                format!(
                    "{}: {:?} (sources={}; create: {}; dismiss: {})",
                    suggestion.id,
                    suggestion.summary,
                    suggestion.source_count,
                    suggestion.create_command,
                    suggestion.dismiss_command
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!(" Memory suggestions are review-only: {summaries}.")
    };
    format!(
        "Work exactly one leased card: {card_id}. Read `maestro card show {card_id} --json`, make the smallest correct change, record proof, and verify through the normal Maestro verbs.{memory_line}{suggestion_line} {ship_line} If you fix a loop-discovered issue, record durable recurrence-guard evidence before completion or ship. Stop on any hard stop and report with `maestro query run --json`."
    )
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}
