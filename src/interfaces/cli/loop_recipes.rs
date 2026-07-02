use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::domain::{card, feature, loop_recipes, run, task};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::session::agent_runtime_from_env;
use crate::foundation::core::time::{timestamp_nanos, utc_now_timestamp};
use crate::interfaces::cli::{
    GitReadout, LoopArgs, LoopCommand, LoopImproveArgs, LoopNextArgs, LoopOutcomeArgs,
    WorkLeaseArgs,
};
use crate::interfaces::hooks::record;
use crate::operations::harness;
use crate::operations::memory::{
    self, ApprovedMemory, MemoryReadScope, MemoryReadSurface, MemorySuggestionHint,
    MemorySuggestionSet, parse_source_ref,
};

const LOOP_OUTCOME_SCHEMA: &str = "maestro.loop_outcome.v1";
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
        Some(LoopCommand::Show {
            name,
            compact,
            phase,
            json,
        }) => {
            if compact {
                let packet = loop_recipes::compact_packet_with_custom_dir(
                    &name,
                    custom_dir.as_deref(),
                    phase.as_deref(),
                )?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&packet)?);
                } else {
                    print!("{}", loop_recipes::render_compact_packet(&packet));
                }
            } else {
                if phase.is_some() {
                    bail!("--phase requires --compact");
                }
                if json {
                    bail!("--json requires --compact for `maestro loop show`");
                }
                print!(
                    "{}",
                    loop_recipes::show_with_custom_dir(&name, custom_dir.as_deref())?
                );
            }
        }
        Some(LoopCommand::Validate { name }) => print!(
            "{}",
            loop_recipes::validate_with_custom_dir(&name, custom_dir.as_deref())?
        ),
        Some(LoopCommand::Template { kind }) => {
            if kind != "custom" {
                bail!("unknown loop template {kind:?}; available: custom");
            }
            print!("{}", loop_recipes::custom_recipe_template());
        }
        Some(LoopCommand::Next(args)) => run_next(args, custom_dir.as_deref())?,
        Some(LoopCommand::Improve(args)) => run_improve(args)?,
        Some(LoopCommand::Outcome(args)) => run_outcome(*args)?,
        Some(LoopCommand::WorkLease(args)) => run_work_lease(*args)?,
    }
    Ok(())
}

fn custom_recipe_dir() -> Option<PathBuf> {
    let repo_root = discover_repo_root().ok()?;
    let paths = MaestroPaths::new(repo_root);
    Some(paths.loop_recipes_dir())
}

fn run_next(args: LoopNextArgs, custom_dir: Option<&std::path::Path>) -> Result<()> {
    let report = build_loop_next_report()?;
    if args.compact {
        let packet = loop_recipes::compact_packet_for_next_report(
            &report,
            custom_dir,
            args.phase.as_deref(),
        )?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&packet)?);
        } else {
            print!("{}", loop_recipes::render_compact_packet(&packet));
        }
    } else if args.phase.is_some() {
        bail!("--phase requires --compact");
    } else if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_loop_next(&report);
    }
    Ok(())
}

fn run_improve(args: LoopImproveArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let report = build_loop_improve_report(&paths)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_loop_improve(&report);
    }
    Ok(())
}

fn build_loop_improve_report(paths: &MaestroPaths) -> Result<loop_recipes::LoopImproveReport> {
    let mut outcomes = Vec::new();
    run::visit_managed_events(paths, |record| {
        let event = record.event();
        if event.event_type() != Some("loop_outcome") {
            return Ok(());
        }
        let value: Value = serde_json::from_str(record.raw_line())?;
        let failure_class = json_string(&value, "failure_class");
        if failure_class.is_empty() {
            return Ok(());
        }
        outcomes.push(loop_recipes::LoopOutcomeInput {
            session_id: record.session_id().to_string(),
            recipe: json_string(&value, "recipe"),
            phase: json_string(&value, "phase"),
            selected_unit: json_string(&value, "selected_unit"),
            failure_class,
            route_action: json_string_at(&value, &["route", "action"]),
            route_recipe: json_string_at(&value, &["route", "recipe"]),
            proof_result: json_string(&value, "proof_result"),
            blocker_class: json_string(&value, "blocker_class"),
            retry_count: value
                .get("retry_count")
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32,
            duration_ms: value
                .get("duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            learning_candidate: optional_json_string(&value, "learning_candidate"),
            source_refs: loop_outcome_source_refs(record.session_id(), &value),
        });
        Ok(())
    })?;
    Ok(loop_recipes::improve_from_outcomes(
        loop_recipes::LoopImproveInput { outcomes },
    ))
}

fn print_loop_improve(report: &loop_recipes::LoopImproveReport) {
    if report.proposals.is_empty() {
        println!("no loop improvement proposals");
        return;
    }
    println!("LoopImprove proposals: {}", report.proposal_count);
    for proposal in &report.proposals {
        println!(
            "{} [{}] {} ({})",
            proposal.id, proposal.kind, proposal.title, proposal.severity
        );
        println!("  apply: {}", proposal.apply_command);
    }
}

fn loop_outcome_source_refs(session_id: &str, value: &Value) -> Vec<loop_recipes::LoopContextRef> {
    let mut refs = vec![loop_recipes::LoopContextRef {
        kind: "run_event".to_string(),
        id: Some(session_id.to_string()),
        path: None,
        command: Some(format!("maestro session show {session_id} --json")),
    }];
    if let Some(items) = value.get("source_refs").and_then(Value::as_array) {
        for item in items {
            let kind = json_string(item, "kind");
            if kind.is_empty() {
                continue;
            }
            refs.push(loop_recipes::LoopContextRef {
                kind,
                id: optional_json_string(item, "id"),
                path: optional_json_string(item, "path"),
                command: optional_json_string(item, "command"),
            });
        }
    }
    refs
}

fn json_string(value: &Value, field: &str) -> String {
    optional_json_string(value, field).unwrap_or_default()
}

fn optional_json_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn json_string_at(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for field in path {
        let Some(next) = current.get(*field) else {
            return String::new();
        };
        current = next;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

#[derive(Clone, Copy, Debug, Serialize)]
struct LoopOutcomeRoute {
    action: &'static str,
    recipe: &'static str,
    reason: &'static str,
}

fn run_outcome(args: LoopOutcomeArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let recipe = required_arg(args.recipe, "--recipe")?;
    let phase = required_arg(args.phase, "--phase")?;
    let selected_unit = required_arg(args.selected_unit, "--selected-unit")?;
    let failure_class = required_arg(args.failure_class, "--failure-class")?.to_ascii_lowercase();
    let proof_result =
        optional_arg(args.proof_result, "--proof-result")?.unwrap_or_else(|| "unknown".to_string());
    let blocker_class =
        optional_arg(args.blocker_class, "--blocker-class")?.unwrap_or_else(|| "none".to_string());
    let constraints = args
        .constraints
        .into_iter()
        .map(|constraint| required_arg(constraint, "--constraint"))
        .collect::<Result<Vec<_>>>()?;
    let source_refs = args
        .source_ref
        .iter()
        .map(|raw| parse_source_ref(raw))
        .collect::<Vec<_>>();
    let learning_candidate = optional_arg(args.learning_candidate, "--learning-candidate")?;
    let route = loop_outcome_route(&failure_class)?;
    let run_id = args.run.unwrap_or_else(super::cli_run_id);
    let mut event = json!({
        "schema_version": LOOP_OUTCOME_SCHEMA,
        "ts": utc_now_timestamp(),
        "event_type": "loop_outcome",
        "session_id": run_id,
        "recipe": recipe,
        "phase": phase,
        "selected_unit": selected_unit,
        "constraints": constraints,
        "proof_result": proof_result,
        "failure_class": failure_class,
        "blocker_class": blocker_class,
        "retry_count": args.retry_count,
        "duration_ms": args.duration_ms,
        "learning_candidate": learning_candidate,
        "source_refs": source_refs,
        "route": route,
    });
    run::insert_agent_runtime(&mut event, agent_runtime_from_env());
    run::append_manual_event(&paths, &run_id, &event)?;
    if args.json {
        println!("{}", serde_json::to_string(&event)?);
    } else {
        println!("recorded loop_outcome event for run {run_id}");
        println!("route: {} -> {}", route.action, route.recipe);
    }
    Ok(())
}

fn required_arg(value: String, flag: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{flag} must not be empty");
    }
    Ok(value.to_string())
}

fn optional_arg(value: Option<String>, flag: &str) -> Result<Option<String>> {
    value.map(|value| required_arg(value, flag)).transpose()
}

fn loop_outcome_route(failure_class: &str) -> Result<LoopOutcomeRoute> {
    match failure_class {
        "proof_gap" => Ok(LoopOutcomeRoute {
            action: "repair",
            recipe: "work",
            reason: "proof failed or evidence is missing; repair the current unit before retrying",
        }),
        "test_failure" => Ok(LoopOutcomeRoute {
            action: "repair",
            recipe: "work",
            reason: "verification failed; repair implementation or tests before retrying",
        }),
        "scope_ambiguity" => Ok(LoopOutcomeRoute {
            action: "design",
            recipe: "design",
            reason: "route is ambiguous; clarify contract, scope, or acceptance before coding",
        }),
        "authority_gap" => Ok(LoopOutcomeRoute {
            action: "hard_stop",
            recipe: "ship",
            reason: "required external authority is absent; stop before irreversible action",
        }),
        "dirty_scope" => Ok(LoopOutcomeRoute {
            action: "repair",
            recipe: "work",
            reason: "dirty tree or ownership risk must be isolated before continuing",
        }),
        "conflict" => Ok(LoopOutcomeRoute {
            action: "hard_stop",
            recipe: "conflict-handoff",
            reason: "active ownership or scope conflict requires coordination before continuing",
        }),
        "memory_collision" => Ok(LoopOutcomeRoute {
            action: "learning",
            recipe: "learning",
            reason: "memory evidence conflicts; reconcile or propose a memory update before reuse",
        }),
        "external_approval" => Ok(LoopOutcomeRoute {
            action: "hard_stop",
            recipe: "ship",
            reason: "external approval gate is unmet; stop before shipping or publishing",
        }),
        "repeated_failure" => Ok(LoopOutcomeRoute {
            action: "audit",
            recipe: "audit",
            reason: "same class failed repeatedly; audit before another implementation attempt",
        }),
        _ => bail!(
            "unsupported --failure-class {failure_class:?}; supported: proof_gap, test_failure, scope_ambiguity, authority_gap, dirty_scope, conflict, memory_collision, external_approval, repeated_failure"
        ),
    }
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
    let readiness = loop_readiness_index(paths, &mut warnings);
    let tasks = task_entries
        .iter()
        .map(|entry| loop_task_input(entry, &readiness))
        .collect::<Vec<_>>();
    let current_task = current_loop_task(task_entries, &readiness);
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
    let pending_synthesis = pending_synthesis_count(paths, features.as_slice(), &mut warnings);
    let now = utc_now_timestamp();
    let roots = super::worktree_roots(paths);
    let (active_sessions, active_conflicts) = match run::active_sessions_union(&roots, &now) {
        Ok(sessions) => {
            let active_sessions = sessions
                .iter()
                .filter(|session| session.presence != run::Presence::Stale)
                .count();
            let active_conflicts = match actionable_active_conflict_count(paths, &roots, &sessions)
            {
                Ok(count) => count,
                Err(error) => {
                    warnings.push(format!("active overlap scan failed: {error:#}"));
                    0
                }
            };
            (active_sessions, active_conflicts)
        }
        Err(error) => {
            warnings.push(format!("active session scan failed: {error:#}"));
            (0, 0)
        }
    };

    let mut input = loop_recipes::LoopRouterInput {
        repo: paths.repo_root().display().to_string(),
        initialized: true,
        current_task,
        tasks,
        features,
        memory_hits: Vec::new(),
        active_conflicts,
        active_sessions,
        pending_synthesis,
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
    };
    let report = loop_recipes::route_next(input.clone())?;
    input.memory_hits = loop_memory_preflight_hits(paths, &input, &report);
    loop_recipes::route_next(input)
}

fn loop_memory_preflight_hits(
    paths: &MaestroPaths,
    input: &loop_recipes::LoopRouterInput,
    report: &loop_recipes::LoopNextReport,
) -> Vec<loop_recipes::LoopMemoryHit> {
    let base_scope = loop_memory_scope(input, None);
    let mut hits = Vec::new();
    if let Ok(set) = memory::approved_memory(paths, MemoryReadSurface::Status, base_scope.clone()) {
        hits.extend(set.memories.into_iter().map(approved_memory_hit));
    }
    if let Some(recipe) = report.recommended_recipe.as_deref() {
        let route_scope = loop_memory_scope(input, Some(recipe));
        if let Ok(set) = memory::approved_memory(paths, MemoryReadSurface::Status, route_scope) {
            hits.extend(set.memories.into_iter().map(approved_memory_hit));
        }
    }
    if let Ok(set) = memory::suggestion_hints(paths, MemoryReadSurface::Status, base_scope) {
        hits.extend(set.suggestions.into_iter().map(memory_suggestion_hit));
    }
    hits.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    hits.dedup_by(|left, right| left.id == right.id && left.kind == right.kind);
    hits
}

fn loop_memory_scope(
    input: &loop_recipes::LoopRouterInput,
    query: Option<&str>,
) -> MemoryReadScope {
    let selected_task = input
        .current_task
        .as_ref()
        .or_else(|| input.tasks.iter().find(|task| task.state == "in_progress"))
        .or_else(|| input.tasks.iter().find(|task| task.state == "ready"))
        .or_else(|| {
            input
                .tasks
                .iter()
                .find(|task| task.state == "needs_verification")
        });
    MemoryReadScope {
        task_id: selected_task.map(|task| task.id.clone()),
        feature_id: selected_task
            .and_then(|task| task.feature_id.clone())
            .or_else(|| {
                input
                    .features
                    .iter()
                    .find(|feature| feature.status == "in_progress" || feature.status == "proposed")
                    .map(|feature| feature.id.clone())
            }),
        query: query.map(str::to_string),
        ..MemoryReadScope::default()
    }
}

fn approved_memory_hit(memory: ApprovedMemory) -> loop_recipes::LoopMemoryHit {
    let kind = classify_memory_signals(memory.signal_types.iter().map(|signal| signal.as_str()));
    loop_recipes::LoopMemoryHit {
        id: memory.id.clone(),
        kind,
        reason: format!(
            "approved memory matched {} scope; {}; {}",
            memory.scope_kind.as_str(),
            memory.reason,
            memory.summary
        ),
        source_refs: vec![
            loop_context_command_ref(
                "memory",
                Some(memory.id.clone()),
                format!("maestro memory show {}", memory.id),
            ),
            loop_context_path_ref("memory_lesson", Some(memory.id), memory.lesson_path),
        ],
    }
}

fn memory_suggestion_hit(suggestion: MemorySuggestionHint) -> loop_recipes::LoopMemoryHit {
    let kind = classify_memory_signals(std::iter::once(suggestion.signal_type.as_str()));
    loop_recipes::LoopMemoryHit {
        id: suggestion.id.clone(),
        kind,
        reason: format!(
            "open memory suggestion matched {} scope; signal={}; target={}; {}",
            suggestion.scope_kind,
            suggestion.signal_type,
            suggestion.target_surface,
            suggestion.summary
        ),
        source_refs: vec![loop_context_command_ref(
            "memory_suggestion",
            Some(suggestion.id),
            "maestro memory suggest list",
        )],
    }
}

fn classify_memory_signals<'a>(signals: impl Iterator<Item = &'a str>) -> String {
    let mut user_correction = false;
    let mut success_pattern = false;
    let mut decision = false;
    let mut guardrail = false;
    for signal in signals {
        match signal {
            "failure" | "repeated_block" => return "prior_failure".to_string(),
            "user_correction" => user_correction = true,
            "verified_success" | "good_run" | "approval" => success_pattern = true,
            "manual_final_decision" | "rejection" => decision = true,
            "loop_hard_stop" | "health_signal" => guardrail = true,
            _ => {}
        }
    }
    if user_correction {
        "user_correction".to_string()
    } else if success_pattern {
        "success_pattern".to_string()
    } else if decision {
        "decision".to_string()
    } else if guardrail {
        "guardrail".to_string()
    } else {
        "recipe_hint".to_string()
    }
}

fn loop_context_command_ref(
    kind: &str,
    id: Option<String>,
    command: impl Into<String>,
) -> loop_recipes::LoopContextRef {
    loop_recipes::LoopContextRef {
        kind: kind.to_string(),
        id,
        path: None,
        command: Some(command.into()),
    }
}

fn loop_context_path_ref(
    kind: &str,
    id: Option<String>,
    path: impl Into<String>,
) -> loop_recipes::LoopContextRef {
    loop_recipes::LoopContextRef {
        kind: kind.to_string(),
        id,
        path: Some(path.into()),
        command: None,
    }
}

fn pending_synthesis_count(
    paths: &MaestroPaths,
    features: &[loop_recipes::LoopFeatureInput],
    warnings: &mut Vec<String>,
) -> usize {
    let mut pending = 0;
    for feature in features {
        if feature.status == "closed" || feature.status == "cancelled" {
            continue;
        }
        match feature::lane_statuses(paths, &feature.id) {
            Ok(lanes) => {
                pending += lanes
                    .iter()
                    .filter(|lane| lane.state == feature::WorktreeComputedState::NeedsSynthesis)
                    .count();
            }
            Err(error) => warnings.push(format!(
                "worktree synthesis scan failed for {}: {error:#}",
                feature.id
            )),
        }
    }
    pending
}

fn actionable_active_conflict_count(
    paths: &MaestroPaths,
    roots: &[MaestroPaths],
    sessions: &[run::SessionActivity],
) -> Result<usize> {
    let me = run::union_session_id(paths, roots, &super::cli_run_id());
    let mut current_cards = sessions
        .iter()
        .filter(|session| session.session_id == me)
        .filter_map(|session| session.bound_card.clone())
        .collect::<BTreeSet<_>>();
    if let Some(card) = super::current_card(paths) {
        current_cards.insert(card);
    }

    let presence_by_session = sessions
        .iter()
        .map(|session| (session.session_id.as_str(), session.presence))
        .collect::<BTreeMap<_, _>>();
    let mut conflicts = BTreeSet::new();

    for session in sessions {
        if !session_can_conflict(session, &me) {
            continue;
        }
        if session
            .bound_card
            .as_ref()
            .is_some_and(|card| current_cards.contains(card))
        {
            conflicts.insert(session.session_id.clone());
        }
    }

    for overlap in run::declared_scope_overlaps_for_active_union(roots, sessions)? {
        if !overlap.owners.iter().any(|owner| owner.session_id == me) {
            continue;
        }
        for owner in overlap.owners {
            if owner.session_id == me {
                continue;
            }
            let Some(presence) = presence_by_session.get(owner.session_id.as_str()) else {
                continue;
            };
            if presence_can_conflict(*presence) {
                conflicts.insert(owner.session_id);
            }
        }
    }

    Ok(conflicts.len())
}

fn session_can_conflict(session: &run::SessionActivity, me: &str) -> bool {
    session.session_id != me && presence_can_conflict(session.presence)
}

fn presence_can_conflict(presence: run::Presence) -> bool {
    matches!(
        presence,
        run::Presence::Working | run::Presence::QuietWorking | run::Presence::Waiting
    )
}

#[derive(Default)]
struct LoopReadinessIndex {
    startable: BTreeSet<String>,
    remaining_blockers: BTreeMap<String, Vec<String>>,
}

fn loop_readiness_index(paths: &MaestroPaths, warnings: &mut Vec<String>) -> LoopReadinessIndex {
    let projection = match task::readiness::projection(
        paths,
        task::readiness::ReadinessFilter {
            blocked_next_limit: usize::MAX,
            ..Default::default()
        },
    ) {
        Ok(projection) => projection,
        Err(error) => {
            warnings.push(format!("ready projection unavailable: {error:#}"));
            return LoopReadinessIndex::default();
        }
    };
    let mut index = LoopReadinessIndex::default();
    for row in projection
        .parallel_wave
        .iter()
        .chain(projection.serial_gates.iter())
    {
        index.startable.insert(row.id.clone());
    }
    for row in projection.blocked_next {
        index
            .remaining_blockers
            .insert(row.id, row.remaining_blockers);
    }
    index
}

fn loop_task_input(
    entry: &task::TaskEntry,
    readiness: &LoopReadinessIndex,
) -> loop_recipes::LoopTaskInput {
    let remaining_blockers = readiness
        .remaining_blockers
        .get(&entry.task.id)
        .cloned()
        .unwrap_or_default();
    loop_recipes::LoopTaskInput {
        id: entry.task.id.clone(),
        title: entry.task.title.clone(),
        state: entry.task.state.as_str().to_string(),
        feature_id: entry.task.feature_id.clone(),
        blocked: task::has_unresolved_blockers(&entry.task) || !remaining_blockers.is_empty(),
        ready_startable: readiness.startable.contains(&entry.task.id),
        gate: entry.task.gate,
        gate_kind: entry.task.gate_kind.clone(),
        lane: entry.task.lane.clone(),
        remaining_blockers,
    }
}

fn current_loop_task(
    entries: &[task::TaskEntry],
    readiness: &LoopReadinessIndex,
) -> Option<loop_recipes::LoopTaskInput> {
    let id = env::var("MAESTRO_CURRENT_TASK").ok()?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let task = entries
        .iter()
        .find(|entry| entry.task.id == id && entry.task.state.is_live())?;
    Some(loop_task_input(task, readiness))
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
    if let Some(score) = report.score {
        println!("score: {score}");
    }
    if let Some(phase) = report.recommended_phase.as_deref() {
        println!("recommended_phase: {phase}");
    }
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
    if !report.why_not.is_empty() {
        println!("why_not:");
        for alternative in &report.why_not {
            println!(
                "- {} blocked_by: {}",
                alternative.recipe,
                alternative.blocked_by.join(", ")
            );
        }
    }
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
