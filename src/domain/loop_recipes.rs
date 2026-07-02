//! The bundled loop recipes, served on demand from the binary.
//!
//! Each recipe is structured control grammar for current Maestro artifacts:
//! when it applies, what authority it has, how it maps current bricks into the
//! six loop phases, and where it may transition or invoke helpers. The
//! authoritative shipped catalog lives in `embedded/loop-recipes/` as
//! `maestro.recipe.v2` YAML. `maestro loop show <name>` renders readable docs
//! from that structure, so human output cannot drift from the contract.
//!
//! The module is named `loop_recipes` rather than `loop` because `loop` is a
//! reserved Rust keyword; the CLI subcommand is still `maestro loop`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};

/// The structured recipe contract tree, embedded at build time.
static LOOP_RECIPE_CONTRACTS_DIR: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/embedded/loop-recipes");

const CONTRACT_SCHEMA_VERSION: &str = "maestro.recipe.v2";
const LOOP_COMPACT_PACKET_SCHEMA: &str = "maestro.loop_compact_packet.v1";
const LOOP_IMPROVE_SCHEMA: &str = "maestro.loop_improve.v1";
const REQUIRED_PHASES: [&str; 6] = ["perceive", "choose", "act", "observe", "learn", "continue"];
const CANONICAL_RECIPE_IDS: [&str; 14] = [
    "adversarial-review",
    "audit",
    "conflict-handoff",
    "design",
    "feature-fanout",
    "generate-filter",
    "intake-triage",
    "learning",
    "loop-until-done",
    "progress",
    "ship",
    "synthesize",
    "unattended",
    "work",
];
const LEGACY_RECIPE_IDS: [&str; 4] = [
    "adversarial-fan-out",
    "feature-fan-out",
    "generate-and-filter",
    "unattended-loop",
];
const CUSTOM_RECIPE_POLICY: [&str; 4] = [
    "Evaluate shipped applies_when rules first.",
    "Use a run-scoped or card-scoped custom recipe only when no shipped recipe fits.",
    "Custom recipes must use maestro.recipe.v2, six phases, current Maestro verbs, hard stops, and continue output.",
    "Custom recipes cannot add non-Maestro write surfaces or skip proof, QA, authority, or human approval gates.",
];
const FORBIDDEN_BYPASS_PHRASES: [&str; 10] = [
    "bypass acceptance",
    "bypass proof",
    "bypass qa",
    "ignore hard stops",
    "launch workers",
    "start a daemon",
    "run scheduler",
    "create hidden store",
    "separate lifecycle",
    "second lifecycle",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeContract {
    pub schema_version: String,
    pub id: String,
    pub kind: RecipeKind,
    pub title: String,
    pub summary: String,
    pub progress_tasks: Vec<ProgressTaskContract>,
    pub applies_when: Vec<String>,
    pub authority_scope: Vec<String>,
    pub autonomy: Vec<String>,
    pub hard_stops: Vec<String>,
    pub transitions: Vec<RecipeEdge>,
    pub invocations: Vec<RecipeEdge>,
    pub outputs: Vec<String>,
    pub router: RouterMetadata,
    pub phases: BTreeMap<String, PhaseContract>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeKind {
    pub category: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressTaskContract {
    pub id: String,
    pub title: String,
    pub phase: String,
    pub required: bool,
    pub done_check: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterMetadata {
    pub status: String,
    pub priority: u16,
    pub confidence: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeEdge {
    pub trigger: String,
    pub to: String,
    pub authority_scope: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub forbidden_verbs: Vec<String>,
    pub hard_stops: Vec<String>,
    pub return_condition: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseContract {
    pub goal: String,
    pub bricks: Vec<String>,
    pub reads: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub forbidden_verbs: Vec<String>,
    pub checks: Vec<String>,
    pub durable_learning: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub optional_helpers: Vec<String>,
    #[serde(default)]
    pub helper_contract: Option<HelperContract>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelperContract {
    pub work_lease: Option<WorkLeaseHelperContract>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkLeaseHelperContract {
    pub selected_unit: Vec<String>,
    pub authority_scope: Vec<String>,
    pub claim_or_reservation: Vec<String>,
    pub expires_or_stale_policy: Vec<String>,
    pub allowed_follow_up_verbs: Vec<String>,
    pub hard_stops: Vec<String>,
    pub observe_requirement: Vec<String>,
    pub reconcile_handles: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct LoopRouterInput {
    pub repo: String,
    pub initialized: bool,
    pub current_task: Option<LoopTaskInput>,
    pub tasks: Vec<LoopTaskInput>,
    pub features: Vec<LoopFeatureInput>,
    pub memory_hits: Vec<LoopMemoryHit>,
    pub active_conflicts: usize,
    pub active_sessions: usize,
    pub pending_synthesis: usize,
    pub git: Option<LoopGitInput>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopTaskInput {
    pub id: String,
    pub title: String,
    pub state: String,
    pub feature_id: Option<String>,
    pub blocked: bool,
    pub ready_startable: bool,
    pub gate: bool,
    pub gate_kind: Option<String>,
    pub lane: Option<String>,
    pub remaining_blockers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopFeatureInput {
    pub id: String,
    pub title: String,
    pub status: String,
    pub total_tasks: usize,
    pub verified_tasks: usize,
    pub open_questions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopGitInput {
    pub branch: Option<String>,
    pub code_other_dirty: usize,
    pub maestro_dirty: usize,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopContext {
    pub schema: &'static str,
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task: Option<LoopContextTask>,
    pub candidate_tasks: Vec<LoopContextTask>,
    pub features: Vec<LoopContextFeature>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<LoopContextGit>,
    pub proof: LoopContextPlaceholder,
    pub qa: LoopContextPlaceholder,
    pub active_sessions: usize,
    pub active_conflicts: usize,
    pub pending_synthesis: usize,
    pub blockers: Vec<LoopContextBlocker>,
    pub memory: Vec<LoopMemoryHit>,
    pub recent_outcomes: Vec<LoopRecentOutcome>,
    pub context_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopContextTask {
    pub id: String,
    pub title: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,
    pub blocked: bool,
    pub ready_startable: bool,
    pub gate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remaining_blockers: Vec<String>,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopContextFeature {
    pub id: String,
    pub title: String,
    pub status: String,
    pub total_tasks: usize,
    pub verified_tasks: usize,
    pub open_questions: usize,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopContextGit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub code_other_dirty: usize,
    pub maestro_dirty: usize,
    pub ahead: usize,
    pub behind: usize,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopContextPlaceholder {
    pub status: LoopConstraintStatus,
    pub reason: String,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopContextBlocker {
    pub target_kind: String,
    pub target_id: String,
    pub reason: String,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopMemoryHit {
    pub id: String,
    pub kind: String,
    pub reason: String,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopRecentOutcome {
    pub id: String,
    pub recipe: String,
    pub phase: String,
    pub result: String,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Default)]
pub struct LoopImproveInput {
    pub outcomes: Vec<LoopOutcomeInput>,
}

#[derive(Clone, Debug)]
pub struct LoopOutcomeInput {
    pub session_id: String,
    pub recipe: String,
    pub phase: String,
    pub selected_unit: String,
    pub failure_class: String,
    pub route_action: String,
    pub route_recipe: String,
    pub proof_result: String,
    pub blocker_class: String,
    pub retry_count: u32,
    pub duration_ms: u64,
    pub learning_candidate: Option<String>,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopImproveReport {
    pub schema: &'static str,
    pub read_only: bool,
    pub proposal_count: usize,
    pub proposals: Vec<LoopImproveProposal>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopImproveProposal {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub severity: String,
    pub reason: String,
    pub failure_class: String,
    pub outcome_count: usize,
    pub source_refs: Vec<LoopContextRef>,
    pub dry_plan: Vec<String>,
    pub apply_command: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LoopContextRef {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LoopConstraint {
    pub id: String,
    pub status: LoopConstraintStatus,
    pub severity: LoopConstraintSeverity,
    pub recipe: String,
    pub reason: String,
    pub blocks: Vec<String>,
    pub unblocks_with: Vec<String>,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopConstraintStatus {
    Pass,
    Warn,
    Fail,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopConstraintSeverity {
    Info,
    Warning,
    Blocker,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopNextReport {
    pub schema: &'static str,
    pub status: String,
    pub repo: String,
    pub recommended_recipe: Option<String>,
    pub recommended_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_phase: Option<String>,
    pub reason: String,
    pub confidence: String,
    pub priority: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
    pub authority_scope: Vec<String>,
    pub autonomy: Vec<String>,
    pub edges: Vec<LoopNextEdge>,
    pub hard_stops: Vec<String>,
    pub inspect: Vec<String>,
    pub next_verbs: Vec<String>,
    pub candidates: Vec<LoopNextCandidate>,
    pub constraints: Vec<LoopConstraint>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub memory_hits: Vec<LoopMemoryHit>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub why_not: Vec<LoopWhyNot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_policy: Option<LoopAttemptPolicy>,
    pub context_refs: Vec<LoopContextRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<LoopNextGit>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopNextEdge {
    pub kind: &'static str,
    pub trigger: String,
    pub to: String,
    pub authority_scope: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub forbidden_verbs: Vec<String>,
    pub hard_stops: Vec<String>,
    pub return_condition: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopNextCandidate {
    pub recipe: String,
    pub status: String,
    pub priority: u16,
    pub confidence: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopWhyNot {
    pub recipe: String,
    pub blocked_by: Vec<String>,
    pub reason: String,
    pub source_refs: Vec<LoopContextRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopAttemptPolicy {
    pub policy: String,
    pub max_attempts: u8,
    pub retry_after: Vec<String>,
    pub stop_on: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopNextGit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub code_other_dirty: usize,
    pub maestro_dirty: usize,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopCompactPacket {
    pub schema: &'static str,
    pub recipe: String,
    pub phase: String,
    pub progress_task: String,
    pub reads: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub forbidden_verbs: Vec<String>,
    pub checks: Vec<String>,
    pub hard_stops: Vec<String>,
    pub next: Vec<String>,
}

#[derive(Clone, Debug)]
struct RouterCandidate {
    recipe: &'static str,
    reason: String,
    inspect: Vec<String>,
    next_verbs: Vec<String>,
}

/// Recommend the next loop recipe from already-read local state.
///
/// This is intentionally a read-only scorer over caller-supplied facts. It does
/// not inspect the filesystem, run git, execute tests, mutate Maestro artifacts,
/// dispatch workers, or call back into the CLI.
pub fn route_next(input: LoopRouterInput) -> Result<LoopNextReport> {
    let mut candidates = Vec::new();
    if !input.initialized {
        return report_for_candidate(
            &input,
            RouterCandidate {
                recipe: "intake-triage",
                reason: ".maestro is missing; route through setup/intake before choosing an executable recipe".to_string(),
                inspect: vec![
                    "maestro status --json".to_string(),
                    "maestro init --dry-run".to_string(),
                ],
                next_verbs: vec![
                    "maestro init --dry-run".to_string(),
                    "maestro init --yes".to_string(),
                ],
            },
            "uncertain",
            Vec::new(),
        );
    }

    if !input.warnings.is_empty() {
        return uncertain_report(
            &input,
            "local state had unreadable or incomplete evidence; inspect before choosing a recipe",
        );
    }

    if input.active_conflicts > 0 {
        candidates.push(RouterCandidate {
            recipe: "conflict-handoff",
            reason: format!(
                "{} active session overlap{} visible; check overlap before implementation or merge-back",
                input.active_conflicts,
                if input.active_conflicts == 1 { " is" } else { "s are" }
            ),
            inspect: vec![
                "maestro active".to_string(),
                "maestro status --json".to_string(),
                "git status --short --branch".to_string(),
            ],
            next_verbs: vec![
                "maestro loop show conflict-handoff".to_string(),
                "maestro active".to_string(),
            ],
        });
    }

    if input.git.as_ref().is_some_and(|git| git.behind > 0) {
        candidates.push(RouterCandidate {
            recipe: "conflict-handoff",
            reason: "shared branch moved since this worktree forked; rebase or merge-back safety must be resolved".to_string(),
            inspect: vec![
                "maestro status --json".to_string(),
                "git status --short --branch".to_string(),
            ],
            next_verbs: vec![
                "maestro loop show conflict-handoff".to_string(),
                "git rebase <shared-branch>".to_string(),
            ],
        });
    }

    if input.pending_synthesis > 0 {
        candidates.push(RouterCandidate {
            recipe: "synthesize",
            reason: format!(
                "{} pending worktree synthesis handoff{} need root/main merge ownership",
                input.pending_synthesis,
                if input.pending_synthesis == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            inspect: vec![
                "maestro status --json".to_string(),
                "maestro feature list --all".to_string(),
            ],
            next_verbs: vec![
                "maestro loop show synthesize".to_string(),
                "maestro status".to_string(),
            ],
        });
    }

    if let Some(task) = input.current_task.as_ref() {
        if task.blocked {
            if candidates.is_empty() {
                return uncertain_report_with_actions(
                    &input,
                    &format!(
                        "current task {} is blocked; inspect blockers before choosing a recipe",
                        task.id
                    ),
                    vec![
                        format!("maestro task show {}", task.id),
                        "maestro status --json".to_string(),
                    ],
                    vec![
                        format!("maestro task show {}", task.id),
                        "maestro task unblock <blocker-id> --reason \"<why>\"".to_string(),
                    ],
                );
            }
        } else if is_ship_gate(task) && matches!(task.state.as_str(), "ready" | "in_progress") {
            candidates.push(ship_gate_candidate(task));
        } else {
            candidates.push(work_candidate(task, "current task is live"));
        }
    }

    let live_tasks: Vec<&LoopTaskInput> = input
        .tasks
        .iter()
        .filter(|task| is_live_task_state(&task.state) && !task.blocked)
        .collect();
    if let Some(task) = live_tasks
        .iter()
        .find(|task| task.state == "needs_verification")
    {
        candidates.push(work_candidate(
            task,
            "task needs proof recovery or verification",
        ));
    }
    if let Some(task) = live_tasks
        .iter()
        .find(|task| task.state == "ready" && task.ready_startable && is_ship_gate(task))
    {
        candidates.push(ship_gate_candidate(task));
    } else {
        let parallel_wave = live_tasks
            .iter()
            .copied()
            .filter(|task| task.state == "ready" && task.ready_startable && !task.gate)
            .collect::<Vec<_>>();
        if !parallel_wave.is_empty() {
            candidates.push(parallel_wave_candidate(&parallel_wave));
        } else if let Some(task) = live_tasks
            .iter()
            .find(|task| task.state == "ready" && task.ready_startable && task.gate)
        {
            candidates.push(serial_gate_candidate(task));
        }
    }
    if let Some(task) = live_tasks.iter().find(|task| task.state == "in_progress") {
        candidates.push(work_candidate(
            task,
            "in-progress task should continue through work",
        ));
    }

    if candidates.is_empty()
        && let Some(task) = input
            .tasks
            .iter()
            .find(|task| task.state == "ready" && task.blocked)
    {
        candidates.push(blocked_ready_candidate(task));
    }

    if let Some(feature) = input.features.iter().find(|feature| {
        feature.status == "in_progress"
            && feature.total_tasks > 0
            && feature.total_tasks == feature.verified_tasks
    }) {
        candidates.push(RouterCandidate {
            recipe: "ship",
            reason: format!(
                "feature {} has all child tasks verified ({}/{})",
                feature.id, feature.verified_tasks, feature.total_tasks
            ),
            inspect: vec![
                format!("maestro feature show {}", feature.id),
                "git status --short --branch".to_string(),
            ],
            next_verbs: vec![
                "maestro loop show ship".to_string(),
                format!(
                    "maestro feature close {} --outcome \"<outcome>\"",
                    feature.id
                ),
            ],
        });
    }

    if candidates.is_empty()
        && let Some(feature) = input.features.iter().find(|feature| {
            feature.status == "proposed" || feature.open_questions > 0 || feature.total_tasks == 0
        })
    {
        candidates.push(RouterCandidate {
            recipe: "design",
            reason: format!(
                "feature {} still needs design or contract clarification",
                feature.id
            ),
            inspect: vec![
                format!("maestro feature show {}", feature.id),
                format!("maestro decision list --feature {}", feature.id),
            ],
            next_verbs: vec![
                "maestro loop show design".to_string(),
                format!("maestro feature show {}", feature.id),
            ],
        });
    }

    let Some(candidate) = best_candidate(&candidates)? else {
        return uncertain_report(
            &input,
            "no confident recipe matched the current local Maestro state",
        );
    };
    report_for_candidate(&input, candidate, "recommended", candidates)
}

fn report_for_candidate(
    input: &LoopRouterInput,
    candidate: RouterCandidate,
    status: &str,
    candidates: Vec<RouterCandidate>,
) -> Result<LoopNextReport> {
    let contract = contract(candidate.recipe)?;
    let context = LoopContext::from_input(input);
    let constraints = evaluate_base_constraints(&context, Some(&contract), Some(&candidate));
    let score = score_candidate(&contract, &constraints);
    let recommended_phase = default_phase_for_next(&contract.id, &candidate.reason).to_string();
    let why_not = why_not_candidates(&context, &candidate, &candidates)?;
    let attempt_policy = attempt_policy_for(&contract.id, &constraints);
    let candidates = if candidates.is_empty() {
        vec![candidate_report(&contract, &candidate)]
    } else {
        candidate_reports(candidates)?
    };
    Ok(LoopNextReport {
        schema: "maestro.loop_next.v1",
        status: status.to_string(),
        repo: input.repo.clone(),
        recommended_recipe: Some(contract.id.clone()),
        recommended_status: contract.router.status.clone(),
        recommended_phase: Some(recommended_phase),
        reason: candidate.reason,
        confidence: contract.router.confidence.clone(),
        priority: contract.router.priority,
        score: Some(score),
        authority_scope: contract.authority_scope.clone(),
        autonomy: contract.autonomy.clone(),
        edges: edge_reports(&contract),
        hard_stops: contract.hard_stops.clone(),
        inspect: candidate.inspect,
        next_verbs: candidate.next_verbs,
        candidates,
        constraints,
        memory_hits: context.memory.clone(),
        why_not,
        attempt_policy: Some(attempt_policy),
        context_refs: context.context_refs,
        warnings: input.warnings.clone(),
        git: input.git.clone().map(LoopNextGit::from),
    })
}

fn uncertain_report(input: &LoopRouterInput, reason: &str) -> Result<LoopNextReport> {
    uncertain_report_with_actions(
        input,
        reason,
        vec![
            "maestro status --json".to_string(),
            "maestro task list --json".to_string(),
            "maestro feature list --all".to_string(),
            "maestro active".to_string(),
        ],
        vec!["maestro status".to_string(), "maestro loop".to_string()],
    )
}

fn uncertain_report_with_actions(
    input: &LoopRouterInput,
    reason: &str,
    inspect: Vec<String>,
    next_verbs: Vec<String>,
) -> Result<LoopNextReport> {
    let context = LoopContext::from_input(input);
    let constraints = evaluate_base_constraints(&context, None, None);
    Ok(LoopNextReport {
        schema: "maestro.loop_next.v1",
        status: "uncertain".to_string(),
        repo: input.repo.clone(),
        recommended_recipe: None,
        recommended_status: "uncertain".to_string(),
        recommended_phase: None,
        reason: reason.to_string(),
        confidence: "low".to_string(),
        priority: 0,
        score: None,
        authority_scope: Vec::new(),
        autonomy: Vec::new(),
        edges: Vec::new(),
        hard_stops: vec![
            "do not mutate cards, tasks, features, git, releases, archives, or files from loop next"
                .to_string(),
            "inspect the current state before choosing a write verb".to_string(),
        ],
        inspect,
        next_verbs,
        candidates: Vec::new(),
        constraints,
        memory_hits: context.memory.clone(),
        why_not: Vec::new(),
        attempt_policy: None,
        context_refs: context.context_refs,
        warnings: input.warnings.clone(),
        git: input.git.clone().map(LoopNextGit::from),
    })
}

impl LoopContext {
    fn from_input(input: &LoopRouterInput) -> Self {
        let mut refs = vec![LoopContextRef::command(
            "status",
            None,
            "maestro status --json",
        )];

        let current_task = input.current_task.as_ref().map(context_task);
        let mut candidate_tasks = Vec::new();
        let mut blockers = Vec::new();
        for task in &input.tasks {
            let item = context_task(task);
            refs.extend(item.source_refs.iter().cloned());
            if task.blocked {
                blockers.push(LoopContextBlocker {
                    target_kind: "task".to_string(),
                    target_id: task.id.clone(),
                    reason: "task has unresolved blockers".to_string(),
                    source_refs: item.source_refs.clone(),
                });
            }
            candidate_tasks.push(item);
        }
        if let Some(task) = current_task.as_ref() {
            refs.extend(task.source_refs.iter().cloned());
        }

        let features = input
            .features
            .iter()
            .map(|feature| {
                let source_refs = vec![LoopContextRef::command(
                    "feature",
                    Some(feature.id.clone()),
                    format!("maestro feature show {}", feature.id),
                )];
                refs.extend(source_refs.iter().cloned());
                LoopContextFeature {
                    id: feature.id.clone(),
                    title: feature.title.clone(),
                    status: feature.status.clone(),
                    total_tasks: feature.total_tasks,
                    verified_tasks: feature.verified_tasks,
                    open_questions: feature.open_questions,
                    source_refs,
                }
            })
            .collect::<Vec<_>>();

        let git = input.git.as_ref().map(|git| {
            let source_refs = vec![LoopContextRef::command(
                "git",
                None,
                "git status --short --branch",
            )];
            refs.extend(source_refs.iter().cloned());
            LoopContextGit {
                branch: git.branch.clone(),
                code_other_dirty: git.code_other_dirty,
                maestro_dirty: git.maestro_dirty,
                ahead: git.ahead,
                behind: git.behind,
                source_refs,
            }
        });

        let active_ref = LoopContextRef::command("active_sessions", None, "maestro active");
        refs.push(active_ref.clone());
        let memory = input.memory_hits.clone();
        for hit in &memory {
            refs.extend(hit.source_refs.iter().cloned());
        }
        refs.sort_by_key(context_ref_sort_key);
        refs.dedup();

        Self {
            schema: "maestro.loop_context.v1",
            repo: input.repo.clone(),
            current_task,
            candidate_tasks,
            features,
            git,
            proof: LoopContextPlaceholder::unknown(
                "proof state is not yet included in loop context",
            ),
            qa: LoopContextPlaceholder::unknown("QA state is not yet included in loop context"),
            active_sessions: input.active_sessions,
            active_conflicts: input.active_conflicts,
            pending_synthesis: input.pending_synthesis,
            blockers,
            memory,
            recent_outcomes: Vec::new(),
            context_refs: refs,
        }
    }
}

impl LoopContextPlaceholder {
    fn unknown(reason: &str) -> Self {
        Self {
            status: LoopConstraintStatus::Unknown,
            reason: reason.to_string(),
            source_refs: Vec::new(),
        }
    }
}

impl LoopContextRef {
    fn command(kind: &str, id: Option<String>, command: impl Into<String>) -> Self {
        Self {
            kind: kind.to_string(),
            id,
            path: None,
            command: Some(command.into()),
        }
    }
}

pub fn improve_from_outcomes(input: LoopImproveInput) -> LoopImproveReport {
    let mut by_failure_class = BTreeMap::<String, Vec<LoopOutcomeInput>>::new();
    for outcome in input.outcomes {
        let failure_class = outcome.failure_class.trim().to_ascii_lowercase();
        if failure_class.is_empty() {
            continue;
        }
        by_failure_class
            .entry(failure_class)
            .or_default()
            .push(outcome);
    }

    let mut proposals = Vec::new();
    for (failure_class, mut outcomes) in by_failure_class {
        outcomes.sort_by(|left, right| {
            left.session_id
                .cmp(&right.session_id)
                .then_with(|| left.selected_unit.cmp(&right.selected_unit))
                .then_with(|| left.recipe.cmp(&right.recipe))
        });
        let high_severity = high_severity_failure_class(&failure_class);
        if outcomes.len() < 2 && !high_severity {
            continue;
        }
        for kind in loop_improve_kinds(&failure_class) {
            if matches!(kind, "recipe_edit_proposal" | "skill_update_proposal")
                && outcomes.len() < 2
                && !high_severity
            {
                continue;
            }
            proposals.push(loop_improve_proposal(
                kind,
                &failure_class,
                &outcomes,
                high_severity,
            ));
        }
    }

    proposals.sort_by(|left, right| left.id.cmp(&right.id));
    proposals.dedup_by(|left, right| left.id == right.id);
    LoopImproveReport {
        schema: LOOP_IMPROVE_SCHEMA,
        read_only: true,
        proposal_count: proposals.len(),
        proposals,
    }
}

fn high_severity_failure_class(failure_class: &str) -> bool {
    matches!(
        failure_class,
        "authority_gap" | "conflict" | "external_approval" | "repeated_failure"
    )
}

fn loop_improve_kinds(failure_class: &str) -> Vec<&'static str> {
    match failure_class {
        "proof_gap" => vec!["memory_suggestion", "proof_guard", "recipe_edit_proposal"],
        "test_failure" => vec!["memory_suggestion", "qa_guard"],
        "scope_ambiguity" => vec!["harness_friction", "recipe_edit_proposal"],
        "repeated_failure" => vec![
            "memory_suggestion",
            "harness_friction",
            "skill_update_proposal",
        ],
        "memory_collision" => vec!["memory_suggestion"],
        "dirty_scope" => vec!["harness_friction"],
        "authority_gap" | "conflict" | "external_approval" => {
            vec!["harness_friction", "skill_update_proposal"]
        }
        _ => vec!["harness_friction"],
    }
}

fn loop_improve_proposal(
    kind: &str,
    failure_class: &str,
    outcomes: &[LoopOutcomeInput],
    high_severity: bool,
) -> LoopImproveProposal {
    let source_refs = loop_improve_source_refs(outcomes);
    let outcome_count = outcomes.len();
    let severity = if high_severity || outcome_count >= 4 {
        "high"
    } else {
        "medium"
    };
    let id = format!("limp-{}-{failure_class}", slug(kind));
    let title = loop_improve_title(kind, failure_class);
    let reason = format!(
        "{outcome_count} sourced loop_outcome event(s) reported {failure_class}; latest route {} -> {}",
        outcomes
            .last()
            .map(|outcome| outcome.route_action.as_str())
            .unwrap_or("unknown"),
        outcomes
            .last()
            .map(|outcome| outcome.route_recipe.as_str())
            .unwrap_or("unknown")
    );
    let dry_plan = loop_improve_dry_plan(kind);
    let apply_command = loop_improve_apply_command(kind, failure_class, &title, outcomes);
    LoopImproveProposal {
        id,
        kind: kind.to_string(),
        title,
        severity: severity.to_string(),
        reason,
        failure_class: failure_class.to_string(),
        outcome_count,
        source_refs,
        dry_plan,
        apply_command,
    }
}

fn loop_improve_title(kind: &str, failure_class: &str) -> String {
    match kind {
        "memory_suggestion" => format!("Capture loop lesson for {failure_class} outcomes"),
        "harness_friction" => format!("Reduce recurring {failure_class} loop friction"),
        "recipe_edit_proposal" => {
            format!("Review loop recipe routing for {failure_class} outcomes")
        }
        "skill_update_proposal" => {
            format!("Update loop guidance for {failure_class} outcomes")
        }
        "qa_guard" => format!("Add QA guard for recurring {failure_class} outcomes"),
        "proof_guard" => format!("Add proof guard for recurring {failure_class} outcomes"),
        _ => format!("Review loop improvement for {failure_class} outcomes"),
    }
}

fn loop_improve_dry_plan(kind: &str) -> Vec<String> {
    let target = match kind {
        "memory_suggestion" => "draft a visible memory suggestion",
        "harness_friction" => "file a visible harness friction proposal",
        "recipe_edit_proposal" => "file a recipe edit proposal for review",
        "skill_update_proposal" => "file a skill guidance proposal for review",
        "qa_guard" => "file a QA guard proposal for review",
        "proof_guard" => "file a proof guard proposal for review",
        _ => "file an explicit improvement proposal for review",
    };
    vec![
        "Inspect the sourced loop_outcome events.".to_string(),
        format!("Confirm the pattern is still valid, then {target}."),
        "Run the apply command only after review; planning does not mutate recipes, skills, harness files, schemas, git, releases, or external systems.".to_string(),
    ]
}

fn loop_improve_apply_command(
    kind: &str,
    failure_class: &str,
    title: &str,
    outcomes: &[LoopOutcomeInput],
) -> String {
    let first_session = outcomes
        .first()
        .map(|outcome| outcome.session_id.as_str())
        .unwrap_or("unknown");
    let evidence = format!(
        "{} sourced loop_outcome event(s) for {}; inspect maestro session show {} --json",
        outcomes.len(),
        failure_class,
        first_session
    );
    if kind == "memory_suggestion" {
        return format!(
            "maestro memory suggest create --source-ref run_event:{} --signal-type failure --summary {} --scope-kind repo --target-surface memory_note --dedupe-key loop-improve-{}",
            shell_single_quote(first_session),
            shell_single_quote(title),
            slug(failure_class)
        );
    }
    format!(
        "maestro harness propose --topic loop-improve-{}-{} --title {} --evidence {}",
        slug(kind),
        slug(failure_class),
        shell_single_quote(title),
        shell_single_quote(&evidence)
    )
}

fn loop_improve_source_refs(outcomes: &[LoopOutcomeInput]) -> Vec<LoopContextRef> {
    let mut refs = Vec::new();
    for outcome in outcomes {
        if outcome.source_refs.is_empty() {
            refs.push(LoopContextRef::command(
                "run_event",
                Some(outcome.session_id.clone()),
                format!("maestro session show {} --json", outcome.session_id),
            ));
        } else {
            refs.extend(outcome.source_refs.iter().cloned());
        }
    }
    refs.sort_by_key(context_ref_sort_key);
    refs.dedup_by(|left, right| {
        left.kind == right.kind
            && left.id == right.id
            && left.path == right.path
            && left.command == right.command
    });
    refs
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn context_task(task: &LoopTaskInput) -> LoopContextTask {
    let source_refs = vec![LoopContextRef::command(
        "task",
        Some(task.id.clone()),
        format!("maestro task show {}", task.id),
    )];
    LoopContextTask {
        id: task.id.clone(),
        title: task.title.clone(),
        state: task.state.clone(),
        feature_id: task.feature_id.clone(),
        blocked: task.blocked,
        ready_startable: task.ready_startable,
        gate: task.gate,
        gate_kind: task.gate_kind.clone(),
        lane: task.lane.clone(),
        remaining_blockers: task.remaining_blockers.clone(),
        source_refs,
    }
}

fn context_ref_sort_key(reference: &LoopContextRef) -> (String, String, String, String) {
    (
        reference.kind.clone(),
        reference.id.clone().unwrap_or_default(),
        reference.path.clone().unwrap_or_default(),
        reference.command.clone().unwrap_or_default(),
    )
}

fn evaluate_base_constraints(
    context: &LoopContext,
    contract: Option<&RecipeContract>,
    candidate: Option<&RouterCandidate>,
) -> Vec<LoopConstraint> {
    let recipe = contract
        .map(|contract| contract.id.clone())
        .unwrap_or_else(|| "uncertain".to_string());
    let selected_unit_refs = selected_unit_refs(context, candidate);
    let git_refs = context
        .git
        .as_ref()
        .map(|git| git.source_refs.clone())
        .unwrap_or_default();
    let active_refs = context
        .context_refs
        .iter()
        .filter(|reference| reference.kind == "active_sessions")
        .cloned()
        .collect::<Vec<_>>();

    vec![
        constraint(
            "authority_ok",
            authority_status(contract),
            authority_severity(contract),
            &recipe,
            authority_reason(contract),
            ConstraintExtras::default(),
        ),
        constraint(
            "scope_clear",
            if context.blockers.is_empty() {
                LoopConstraintStatus::Pass
            } else {
                LoopConstraintStatus::Warn
            },
            if context.blockers.is_empty() {
                LoopConstraintSeverity::Info
            } else {
                LoopConstraintSeverity::Warning
            },
            &recipe,
            if context.blockers.is_empty() {
                "no unresolved task blockers are visible in the router snapshot"
            } else {
                "one or more visible tasks have unresolved blockers"
            },
            ConstraintExtras::with_source_refs(selected_unit_refs.clone()),
        ),
        constraint(
            "selected_unit_ok",
            if selected_unit_refs.is_empty() {
                LoopConstraintStatus::Unknown
            } else {
                LoopConstraintStatus::Pass
            },
            if selected_unit_refs.is_empty() {
                LoopConstraintSeverity::Warning
            } else {
                LoopConstraintSeverity::Info
            },
            &recipe,
            if selected_unit_refs.is_empty() {
                "no selected task or feature was identified"
            } else {
                "selected task or feature is backed by source refs"
            },
            ConstraintExtras::with_source_refs(selected_unit_refs.clone()),
        ),
        constraint(
            "proof_ready",
            proof_status(&recipe),
            proof_severity(&recipe),
            &recipe,
            proof_reason(&recipe),
            ConstraintExtras::with_source_refs(context.proof.source_refs.clone()),
        ),
        constraint(
            "qa_ready",
            qa_status(&recipe),
            qa_severity(&recipe),
            &recipe,
            qa_reason(&recipe),
            ConstraintExtras::with_source_refs(context.qa.source_refs.clone()),
        ),
        constraint(
            "dirty_tree_risk",
            dirty_tree_status(context),
            dirty_tree_severity(context),
            &recipe,
            dirty_tree_reason(context),
            ConstraintExtras {
                blocks: dirty_tree_blocks(&recipe, context),
                unblocks_with: vec!["git status --short --branch".to_string()],
                source_refs: git_refs,
            },
        ),
        constraint(
            "conflict_risk",
            if context.active_conflicts > 0 {
                LoopConstraintStatus::Warn
            } else {
                LoopConstraintStatus::Pass
            },
            if context.active_conflicts > 0 {
                LoopConstraintSeverity::Warning
            } else {
                LoopConstraintSeverity::Info
            },
            &recipe,
            if context.active_conflicts > 0 {
                "active overlapping sessions are visible"
            } else {
                "no active overlap count is visible in the router snapshot"
            },
            ConstraintExtras {
                blocks: if context.active_conflicts > 0 {
                    vec!["work".to_string(), "ship".to_string()]
                } else {
                    Vec::new()
                },
                unblocks_with: vec!["maestro active".to_string()],
                source_refs: active_refs,
            },
        ),
        constraint(
            "memory_relevance",
            memory_relevance_status(context),
            memory_relevance_severity(context),
            &recipe,
            memory_relevance_reason(context),
            ConstraintExtras::with_source_refs(memory_source_refs(context)),
        ),
        constraint(
            "prior_failure_risk",
            prior_failure_status(context),
            prior_failure_severity(context),
            &recipe,
            prior_failure_reason(context),
            ConstraintExtras::with_source_refs(prior_failure_source_refs(context)),
        ),
        constraint(
            "route_confidence",
            route_confidence_status(contract),
            route_confidence_severity(contract),
            &recipe,
            route_confidence_reason(contract),
            ConstraintExtras::default(),
        ),
        constraint(
            "ship_gate_ok",
            ship_gate_status(&recipe),
            ship_gate_severity(&recipe),
            &recipe,
            ship_gate_reason(&recipe),
            ConstraintExtras::with_source_refs(selected_unit_refs.clone()),
        ),
        constraint(
            "human_approval_ok",
            LoopConstraintStatus::Pass,
            LoopConstraintSeverity::Info,
            &recipe,
            "router recommendation does not change acceptance, non-goals, ship authority, dependencies, schemas, git, secrets, or platform approval",
            ConstraintExtras::default(),
        ),
    ]
}

#[derive(Default)]
struct ConstraintExtras {
    blocks: Vec<String>,
    unblocks_with: Vec<String>,
    source_refs: Vec<LoopContextRef>,
}

impl ConstraintExtras {
    fn with_source_refs(source_refs: Vec<LoopContextRef>) -> Self {
        Self {
            source_refs,
            ..Self::default()
        }
    }
}

fn constraint(
    id: &str,
    status: LoopConstraintStatus,
    severity: LoopConstraintSeverity,
    recipe: &str,
    reason: impl Into<String>,
    extras: ConstraintExtras,
) -> LoopConstraint {
    LoopConstraint {
        id: id.to_string(),
        status,
        severity,
        recipe: recipe.to_string(),
        reason: reason.into(),
        blocks: extras.blocks,
        unblocks_with: extras.unblocks_with,
        source_refs: extras.source_refs,
    }
}

fn memory_source_refs(context: &LoopContext) -> Vec<LoopContextRef> {
    let mut refs = context
        .memory
        .iter()
        .flat_map(|hit| hit.source_refs.iter().cloned())
        .collect::<Vec<_>>();
    refs.sort_by_key(context_ref_sort_key);
    refs.dedup();
    refs
}

fn memory_relevance_status(context: &LoopContext) -> LoopConstraintStatus {
    if context.memory.is_empty() {
        LoopConstraintStatus::Unknown
    } else {
        LoopConstraintStatus::Pass
    }
}

fn memory_relevance_severity(_context: &LoopContext) -> LoopConstraintSeverity {
    LoopConstraintSeverity::Info
}

fn memory_relevance_reason(context: &LoopContext) -> String {
    if context.memory.is_empty() {
        "no scoped approved memories or memory suggestions matched the router snapshot".to_string()
    } else {
        format!(
            "{} scoped memory hit{} matched the router snapshot",
            context.memory.len(),
            if context.memory.len() == 1 { "" } else { "s" }
        )
    }
}

fn prior_failure_status(context: &LoopContext) -> LoopConstraintStatus {
    if context.memory.iter().any(memory_kind_is_failure_risk) {
        LoopConstraintStatus::Warn
    } else if context.memory.is_empty() {
        LoopConstraintStatus::Unknown
    } else {
        LoopConstraintStatus::Pass
    }
}

fn prior_failure_severity(context: &LoopContext) -> LoopConstraintSeverity {
    if context.memory.iter().any(memory_kind_is_failure_risk) {
        LoopConstraintSeverity::Warning
    } else {
        LoopConstraintSeverity::Info
    }
}

fn prior_failure_reason(context: &LoopContext) -> String {
    let count = context
        .memory
        .iter()
        .filter(|hit| memory_kind_is_failure_risk(hit))
        .count();
    if count > 0 {
        format!(
            "{count} memory hit{} indicate prior failure, guardrail, or user correction risk",
            if count == 1 { "" } else { "s" }
        )
    } else if context.memory.is_empty() {
        "recent loop outcomes and failure memories are not yet populated".to_string()
    } else {
        "matched memory hits do not indicate prior failure risk".to_string()
    }
}

fn prior_failure_source_refs(context: &LoopContext) -> Vec<LoopContextRef> {
    let mut refs = context
        .memory
        .iter()
        .filter(|hit| memory_kind_is_failure_risk(hit))
        .flat_map(|hit| hit.source_refs.iter().cloned())
        .collect::<Vec<_>>();
    refs.sort_by_key(context_ref_sort_key);
    refs.dedup();
    refs
}

fn memory_kind_is_failure_risk(hit: &LoopMemoryHit) -> bool {
    matches!(
        hit.kind.as_str(),
        "prior_failure" | "guardrail" | "user_correction"
    )
}

fn selected_unit_refs(
    context: &LoopContext,
    candidate: Option<&RouterCandidate>,
) -> Vec<LoopContextRef> {
    let Some(candidate) = candidate else {
        return Vec::new();
    };
    context
        .context_refs
        .iter()
        .filter(|reference| {
            reference
                .id
                .as_ref()
                .is_some_and(|id| candidate.reason.contains(id))
        })
        .cloned()
        .collect()
}

fn authority_status(contract: Option<&RecipeContract>) -> LoopConstraintStatus {
    if contract.is_some() {
        LoopConstraintStatus::Pass
    } else {
        LoopConstraintStatus::Unknown
    }
}

fn authority_severity(contract: Option<&RecipeContract>) -> LoopConstraintSeverity {
    if contract.is_some() {
        LoopConstraintSeverity::Info
    } else {
        LoopConstraintSeverity::Warning
    }
}

fn authority_reason(contract: Option<&RecipeContract>) -> &'static str {
    if contract.is_some() {
        "recommended recipe carries an explicit authority scope"
    } else {
        "no recipe authority scope was selected"
    }
}

fn proof_status(recipe: &str) -> LoopConstraintStatus {
    if recipe == "ship" {
        LoopConstraintStatus::Unknown
    } else {
        LoopConstraintStatus::Pass
    }
}

fn proof_severity(recipe: &str) -> LoopConstraintSeverity {
    if recipe == "ship" {
        LoopConstraintSeverity::Warning
    } else {
        LoopConstraintSeverity::Info
    }
}

fn proof_reason(recipe: &str) -> &'static str {
    if recipe == "ship" {
        "ship proof requires feature-level verification evidence not yet modeled in LoopContext"
    } else {
        "route can proceed before final proof, which remains enforced by task and feature gates"
    }
}

fn qa_status(recipe: &str) -> LoopConstraintStatus {
    if recipe == "ship" {
        LoopConstraintStatus::Unknown
    } else {
        LoopConstraintStatus::Pass
    }
}

fn qa_severity(recipe: &str) -> LoopConstraintSeverity {
    if recipe == "ship" {
        LoopConstraintSeverity::Warning
    } else {
        LoopConstraintSeverity::Info
    }
}

fn qa_reason(recipe: &str) -> &'static str {
    if recipe == "ship" {
        "ship QA requires feature QA evidence not yet modeled in LoopContext"
    } else {
        "QA is not a blocker for this route before final feature gates"
    }
}

fn dirty_tree_status(context: &LoopContext) -> LoopConstraintStatus {
    match context.git.as_ref() {
        Some(git) if git.code_other_dirty > 0 || git.maestro_dirty > 0 => {
            LoopConstraintStatus::Warn
        }
        Some(_) => LoopConstraintStatus::Pass,
        None => LoopConstraintStatus::Unknown,
    }
}

fn dirty_tree_severity(context: &LoopContext) -> LoopConstraintSeverity {
    match dirty_tree_status(context) {
        LoopConstraintStatus::Warn => LoopConstraintSeverity::Warning,
        LoopConstraintStatus::Unknown => LoopConstraintSeverity::Warning,
        _ => LoopConstraintSeverity::Info,
    }
}

fn dirty_tree_reason(context: &LoopContext) -> &'static str {
    match context.git.as_ref() {
        Some(git) if git.code_other_dirty > 0 || git.maestro_dirty > 0 => {
            "working tree has dirty code/other or Maestro files"
        }
        Some(_) => "working tree is clean in the router snapshot",
        None => "git state is unavailable in the router snapshot",
    }
}

fn dirty_tree_blocks(recipe: &str, context: &LoopContext) -> Vec<String> {
    if recipe == "ship" && dirty_tree_status(context) == LoopConstraintStatus::Warn {
        vec!["ship".to_string()]
    } else {
        Vec::new()
    }
}

fn route_confidence_status(contract: Option<&RecipeContract>) -> LoopConstraintStatus {
    match contract.map(|contract| contract.router.confidence.as_str()) {
        Some("high") => LoopConstraintStatus::Pass,
        Some("medium") => LoopConstraintStatus::Warn,
        Some("low") => LoopConstraintStatus::Unknown,
        Some(_) | None => LoopConstraintStatus::Unknown,
    }
}

fn route_confidence_severity(contract: Option<&RecipeContract>) -> LoopConstraintSeverity {
    match route_confidence_status(contract) {
        LoopConstraintStatus::Pass => LoopConstraintSeverity::Info,
        LoopConstraintStatus::Warn => LoopConstraintSeverity::Warning,
        _ => LoopConstraintSeverity::Warning,
    }
}

fn route_confidence_reason(contract: Option<&RecipeContract>) -> &'static str {
    match contract.map(|contract| contract.router.confidence.as_str()) {
        Some("high") => "recipe router metadata confidence is high",
        Some("medium") => "recipe router metadata confidence is medium",
        Some("low") => "recipe router metadata confidence is low",
        Some(_) => "recipe router metadata confidence is unrecognized",
        None => "no recipe was selected",
    }
}

fn ship_gate_status(recipe: &str) -> LoopConstraintStatus {
    if recipe == "ship" {
        LoopConstraintStatus::Unknown
    } else {
        LoopConstraintStatus::Pass
    }
}

fn ship_gate_severity(recipe: &str) -> LoopConstraintSeverity {
    if recipe == "ship" {
        LoopConstraintSeverity::Warning
    } else {
        LoopConstraintSeverity::Info
    }
}

fn ship_gate_reason(recipe: &str) -> &'static str {
    if recipe == "ship" {
        "feature ship gate requires proof and QA evidence before close"
    } else {
        "route is not attempting to close or ship a feature"
    }
}

fn score_candidate(contract: &RecipeContract, constraints: &[LoopConstraint]) -> u8 {
    let confidence_bonus = match contract.router.confidence.as_str() {
        "high" => 20,
        "medium" => 10,
        "low" => 0,
        _ => 0,
    };
    let mut score = 50_i16 + contract.router.priority.min(30) as i16 + confidence_bonus;
    for constraint in constraints {
        score -= match constraint.status {
            LoopConstraintStatus::Fail => 40,
            LoopConstraintStatus::Warn => 10,
            LoopConstraintStatus::Unknown
                if constraint.severity == LoopConstraintSeverity::Warning =>
            {
                5
            }
            LoopConstraintStatus::Unknown | LoopConstraintStatus::Pass => 0,
        };
    }
    score.clamp(0, 100) as u8
}

fn why_not_candidates(
    context: &LoopContext,
    selected: &RouterCandidate,
    candidates: &[RouterCandidate],
) -> Result<Vec<LoopWhyNot>> {
    let selected_contract = contract(selected.recipe)?;
    let mut reports = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.recipe != selected.recipe)
    {
        let candidate_contract = contract(candidate.recipe)?;
        let candidate_constraints =
            evaluate_base_constraints(context, Some(&candidate_contract), Some(candidate));
        let mut blocked_by = blocked_by_for_candidate(candidate.recipe, &candidate_constraints);
        let reason = if blocked_by.is_empty() {
            blocked_by.push("lower_priority_than_selected_recipe".to_string());
            format!(
                "{} priority {} did not outrank selected {} priority {}",
                candidate_contract.id,
                candidate_contract.router.priority,
                selected_contract.id,
                selected_contract.router.priority
            )
        } else {
            format!(
                "{} was not selected because constraint(s) blocked or warned on that route",
                candidate_contract.id
            )
        };
        reports.push(LoopWhyNot {
            recipe: candidate_contract.id,
            blocked_by,
            reason,
            source_refs: selected_unit_refs(context, Some(candidate)),
        });
    }
    reports.sort_by(|left, right| left.recipe.cmp(&right.recipe));
    reports.dedup_by(|left, right| left.recipe == right.recipe);
    Ok(reports)
}

fn blocked_by_for_candidate(recipe: &str, constraints: &[LoopConstraint]) -> Vec<String> {
    let mut blocked_by = constraints
        .iter()
        .filter(|constraint| {
            constraint.status == LoopConstraintStatus::Fail
                || constraint.blocks.iter().any(|blocked| blocked == recipe)
        })
        .map(|constraint| constraint.id.clone())
        .collect::<Vec<_>>();
    blocked_by.sort();
    blocked_by.dedup();
    blocked_by
}

fn attempt_policy_for(recipe: &str, constraints: &[LoopConstraint]) -> LoopAttemptPolicy {
    let mut stop_on = blocked_by_for_candidate(recipe, constraints);
    for constraint in constraints.iter().filter(|constraint| {
        constraint.id == "human_approval_ok" && constraint.status != LoopConstraintStatus::Pass
    }) {
        stop_on.push(constraint.id.clone());
    }
    stop_on.sort();
    stop_on.dedup();
    if stop_on.is_empty() {
        stop_on.push("failed proof, QA, conflict, dirty-tree, or approval constraint".to_string());
    }
    LoopAttemptPolicy {
        policy: "single_attempt_then_observe".to_string(),
        max_attempts: 1,
        retry_after: vec![
            "record LoopOutcome before retrying the same recipe".to_string(),
            "rerun loop next with refreshed local state".to_string(),
        ],
        stop_on,
    }
}

fn best_candidate(candidates: &[RouterCandidate]) -> Result<Option<RouterCandidate>> {
    let mut ranked = candidates
        .iter()
        .map(|candidate| Ok((contract(candidate.recipe)?, candidate)))
        .collect::<Result<Vec<_>>>()?;
    ranked.sort_by(
        |(left_contract, left_candidate), (right_contract, right_candidate)| {
            right_contract
                .router
                .priority
                .cmp(&left_contract.router.priority)
                .then_with(|| left_candidate.recipe.cmp(right_candidate.recipe))
                .then_with(|| left_candidate.reason.cmp(&right_candidate.reason))
        },
    );
    Ok(ranked.first().map(|(_, candidate)| (*candidate).clone()))
}

fn candidate_reports(candidates: Vec<RouterCandidate>) -> Result<Vec<LoopNextCandidate>> {
    let mut reports = candidates
        .into_iter()
        .map(|candidate| {
            let contract = contract(candidate.recipe)?;
            Ok(candidate_report(&contract, &candidate))
        })
        .collect::<Result<Vec<_>>>()?;
    reports.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.recipe.cmp(&right.recipe))
            .then_with(|| left.reason.cmp(&right.reason))
    });
    reports.dedup_by(|left, right| left.recipe == right.recipe && left.reason == right.reason);
    Ok(reports)
}

fn candidate_report(contract: &RecipeContract, candidate: &RouterCandidate) -> LoopNextCandidate {
    LoopNextCandidate {
        recipe: contract.id.clone(),
        status: contract.router.status.clone(),
        priority: contract.router.priority,
        confidence: contract.router.confidence.clone(),
        reason: candidate.reason.clone(),
    }
}

fn edge_reports(contract: &RecipeContract) -> Vec<LoopNextEdge> {
    contract
        .transitions
        .iter()
        .map(|edge| edge_report("transition", edge))
        .chain(
            contract
                .invocations
                .iter()
                .map(|edge| edge_report("invocation", edge)),
        )
        .collect()
}

fn edge_report(kind: &'static str, edge: &RecipeEdge) -> LoopNextEdge {
    LoopNextEdge {
        kind,
        trigger: edge.trigger.clone(),
        to: edge.to.clone(),
        authority_scope: edge.authority_scope.clone(),
        allowed_verbs: edge.allowed_verbs.clone(),
        forbidden_verbs: edge.forbidden_verbs.clone(),
        hard_stops: edge.hard_stops.clone(),
        return_condition: edge.return_condition.clone(),
    }
}

fn work_candidate(task: &LoopTaskInput, reason: &str) -> RouterCandidate {
    RouterCandidate {
        recipe: "work",
        reason: format!("{reason}: {} ({})", task.id, task.state),
        inspect: vec![
            format!("maestro task show {}", task.id),
            "maestro status --json".to_string(),
        ],
        next_verbs: vec![
            "maestro loop show work".to_string(),
            format!("maestro task show {}", task.id),
        ],
    }
}

fn parallel_wave_candidate(tasks: &[&LoopTaskInput]) -> RouterCandidate {
    let first = tasks[0];
    let lanes = ready_lane_count(tasks);
    RouterCandidate {
        recipe: "work",
        reason: format!(
            "{} executable task{} ready now across {} lane{}",
            tasks.len(),
            if tasks.len() == 1 { " is" } else { "s are" },
            lanes,
            if lanes == 1 { "" } else { "s" }
        ),
        inspect: vec![
            "maestro ready".to_string(),
            format!("maestro task show {}", first.id),
        ],
        next_verbs: vec![
            "maestro loop show work".to_string(),
            "maestro ready".to_string(),
            format!("maestro task start {}", first.id),
        ],
    }
}

fn serial_gate_candidate(task: &LoopTaskInput) -> RouterCandidate {
    let kind = task.gate_kind.as_deref().unwrap_or("integration");
    RouterCandidate {
        recipe: "work",
        reason: format!("{kind} gate {} is ready and must run serially", task.id),
        inspect: vec![
            "maestro ready".to_string(),
            format!("maestro task show {}", task.id),
        ],
        next_verbs: vec![
            "maestro loop show work".to_string(),
            format!("maestro task start {}", task.id),
        ],
    }
}

fn ship_gate_candidate(task: &LoopTaskInput) -> RouterCandidate {
    RouterCandidate {
        recipe: "ship",
        reason: format!("ship gate {} is ready", task.id),
        inspect: vec![
            "maestro ready".to_string(),
            format!("maestro task show {}", task.id),
            "git status --short --branch".to_string(),
        ],
        next_verbs: vec![
            "maestro loop show ship".to_string(),
            format!("maestro task start {}", task.id),
        ],
    }
}

fn blocked_ready_candidate(task: &LoopTaskInput) -> RouterCandidate {
    let blockers = if task.remaining_blockers.is_empty() {
        "unresolved blockers".to_string()
    } else {
        task.remaining_blockers.join(", ")
    };
    RouterCandidate {
        recipe: "work",
        reason: format!("ready graph is blocked; {} waits on {blockers}", task.id),
        inspect: vec![
            "maestro ready".to_string(),
            format!("maestro task show {}", task.id),
        ],
        next_verbs: vec![
            "maestro loop show work".to_string(),
            "maestro ready".to_string(),
        ],
    }
}

fn ready_lane_count(tasks: &[&LoopTaskInput]) -> usize {
    let mut lanes = BTreeSet::new();
    for task in tasks {
        lanes.insert(task.lane.as_deref().unwrap_or("general"));
    }
    lanes.len()
}

fn is_ship_gate(task: &LoopTaskInput) -> bool {
    task.gate && (task.gate_kind.as_deref() == Some("ship") || task.lane.as_deref() == Some("ship"))
}

fn is_live_task_state(state: &str) -> bool {
    matches!(
        state,
        "draft" | "exploring" | "ready" | "in_progress" | "needs_verification"
    )
}

impl From<LoopGitInput> for LoopNextGit {
    fn from(git: LoopGitInput) -> Self {
        Self {
            branch: git.branch,
            code_other_dirty: git.code_other_dirty,
            maestro_dirty: git.maestro_dirty,
            ahead: git.ahead,
            behind: git.behind,
        }
    }
}

/// Render one shipped recipe by its canonical id. An unknown name fails loud
/// with the available list, never a dead end.
pub fn serve(name: &str) -> Result<String> {
    show(name)
}

/// The index enumerates the embedded structured catalog so the list never
/// drifts from what ships.
pub fn index() -> String {
    let mut out = "# Loop Recipes\n\n".to_string();
    out.push_str(
        "Maestro is the loop: recipes are structured control grammar over current cards, tasks, features, decisions, proof, QA, run events, notes, memory, and skills. `maestro loop list/show/next/validate` are read-only; `maestro loop work-lease` is a mutating helper that emits run evidence and may claim a card. Existing Maestro verbs perform writes.\n\n",
    );
    out.push_str("## Shipped Recipe Catalog\n\n");
    for contract in contracts().expect("invariant: shipped loop recipe contracts validate") {
        out.push_str(&format!(
            "    {}  [{}]  --  {}\n",
            contract.id, contract.kind.category, contract.summary
        ));
    }
    out.push_str("\n\n## Custom Recipe Policy\n\n");
    push_bullets(&mut out, "", &CUSTOM_RECIPE_POLICY);
    out
}

pub fn index_with_custom_dir(custom_dir: Option<&Path>) -> Result<String> {
    let mut out = index();
    if let Some(custom_dir) = custom_dir {
        let contracts = custom_contracts(custom_dir)?;
        if !contracts.is_empty() {
            out.push_str("\n\n## Project Custom Recipes\n\n");
            for contract in contracts {
                out.push_str(&format!("    {}  --  {}\n", contract.id, contract.summary));
            }
        }
    }
    Ok(out)
}

/// Render one structured shipped recipe contract.
pub fn show(name: &str) -> Result<String> {
    if contract_names().contains(&name) {
        return Ok(render_contract(&contract(name)?));
    }
    bail!(
        "unknown loop recipe \"{name}\"; run `maestro loop` for the index (available: {})",
        available_names().join(", ")
    );
}

pub fn show_with_custom_dir(name: &str, custom_dir: Option<&Path>) -> Result<String> {
    let custom_names = match custom_dir {
        Some(custom_dir) => custom_contract_names(custom_dir)?,
        None => Vec::new(),
    };
    if contract_names().contains(&name) {
        return show(name);
    }
    if let Some(custom_dir) = custom_dir
        && custom_names.iter().any(|custom| custom == name)
    {
        return Ok(render_contract(&custom_contract_known(custom_dir, name)?));
    }
    bail!(
        "unknown loop recipe \"{name}\"; run `maestro loop` for the index (available: {})",
        available_names_with_custom(custom_dir)?.join(", ")
    );
}

pub fn compact_packet_with_custom_dir(
    name: &str,
    custom_dir: Option<&Path>,
    phase: Option<&str>,
) -> Result<LoopCompactPacket> {
    let custom_names = match custom_dir {
        Some(custom_dir) => custom_contract_names(custom_dir)?,
        None => Vec::new(),
    };
    if contract_names().contains(&name) {
        return compact_packet_for_contract(&contract(name)?, phase);
    }
    if let Some(custom_dir) = custom_dir
        && custom_names.iter().any(|custom| custom == name)
    {
        return compact_packet_for_contract(&custom_contract_known(custom_dir, name)?, phase);
    }
    bail!(
        "unknown loop recipe \"{name}\"; run `maestro loop` for the index (available: {})",
        available_names_with_custom(custom_dir)?.join(", ")
    );
}

pub fn compact_packet_for_next_report(
    report: &LoopNextReport,
    custom_dir: Option<&Path>,
    phase: Option<&str>,
) -> Result<LoopCompactPacket> {
    let recipe = report
        .recommended_recipe
        .as_deref()
        .with_context(|| format!("loop next has no recommended recipe: {}", report.reason))?;
    let selected_phase = phase.unwrap_or_else(|| default_phase_for_next(recipe, &report.reason));
    compact_packet_with_custom_dir(recipe, custom_dir, Some(selected_phase))
}

pub fn render_compact_packet(packet: &LoopCompactPacket) -> String {
    let mut out = format!(
        "schema: {}\nrecipe: {}\nphase: {}\nprogress_task: {}\n",
        packet.schema, packet.recipe, packet.phase, packet.progress_task
    );
    push_compact_list(&mut out, "reads", &packet.reads);
    push_compact_list(&mut out, "allowed_verbs", &packet.allowed_verbs);
    push_compact_list(&mut out, "forbidden_verbs", &packet.forbidden_verbs);
    push_compact_list(&mut out, "checks", &packet.checks);
    push_compact_list(&mut out, "hard_stops", &packet.hard_stops);
    push_compact_list(&mut out, "next", &packet.next);
    out
}

pub fn validate_with_custom_dir(name: &str, custom_dir: Option<&Path>) -> Result<String> {
    let custom_names = match custom_dir {
        Some(custom_dir) => custom_contract_names(custom_dir)?,
        None => Vec::new(),
    };
    if contract_names().contains(&name) {
        contract(name)?;
        return Ok(format!("valid shipped loop recipe: {name}\n"));
    }
    if let Some(custom_dir) = custom_dir
        && custom_names.iter().any(|custom| custom == name)
    {
        custom_contract_known(custom_dir, name)?;
        return Ok(format!("valid project custom loop recipe: {name}\n"));
    }
    bail!(
        "unknown structured loop recipe \"{name}\"; run `maestro loop` for the index (available: {})",
        available_names_with_custom(custom_dir)?.join(", ")
    );
}

pub fn custom_recipe_template() -> &'static str {
    r#"schema_version: maestro.recipe.v2
id: custom
kind:
  category: custom
  tags: ["custom", "template"]
title: Custom loop
summary: Handle one bounded custom workflow through current Maestro artifacts.
progress_tasks:
  - id: anchor-scope
    title: Anchor custom scope
    phase: perceive
    required: true
    done_check: scope, authority, and hard stops are known
  - id: choose-next
    title: Choose next custom move
    phase: choose
    required: true
    done_check: next move is selected from recipe rules
  - id: execute-move
    title: Execute bounded custom move
    phase: act
    required: true
    done_check: bounded recipe action is complete or blocked
  - id: observe-evidence
    title: Observe custom evidence
    phase: observe
    required: true
    done_check: evidence, result, or blocker is recorded
  - id: record-learning
    title: Record reusable custom learning when needed
    phase: learn
    required: true
    done_check: durable learning is recorded or explicitly unnecessary
  - id: return-next-gate
    title: Return next custom gate
    phase: continue
    required: true
    done_check: next gate, next task, or hard stop is visible
authority_scope:
  - current custom request and selected Maestro artifacts
autonomy:
  - local autonomous work only inside the selected custom scope
router:
  status: custom
  priority: 3
  confidence: medium
transitions: []
invocations: []
outputs:
  - selected work
  - recorded evidence
  - next gate or hard stop
applies_when:
  - no shipped recipe fits the current card or run
hard_stops:
  - authority, proof, QA, or approval gate would be skipped
phases:
  perceive:
    goal: Read current Maestro state and the selected custom scope.
    bricks: ["status", "card", "task"]
    reads: ["maestro status", "maestro card show <id>", "maestro task list"]
    allowed_verbs: ["maestro status", "maestro card show <id>", "maestro task list"]
    forbidden_verbs: ["external ship action"]
    checks: ["scope, authority, and hard stops are visible"]
    durable_learning: []
    outputs: ["current scope", "hard stop"]
  choose:
    goal: Choose the smallest legal next move.
    bricks: ["task", "decision", "recipe"]
    reads: ["maestro loop next", "maestro task next"]
    allowed_verbs: ["maestro loop next", "maestro task next"]
    forbidden_verbs: ["unbounded worker launch"]
    checks: ["one move is selected"]
    durable_learning: []
    outputs: ["selected move"]
  act:
    goal: Execute the bounded move through existing Maestro verbs.
    bricks: ["task", "proof", "note"]
    reads: ["maestro task show <id>"]
    allowed_verbs: ["maestro task complete <id>", "maestro task verify <id>", "maestro card note <id> <text>"]
    forbidden_verbs: ["unapproved dependency", "force push"]
    checks: ["action stays inside selected scope"]
    durable_learning: []
    outputs: ["completed move", "blocker"]
  observe:
    goal: Verify the result and capture inspectable evidence.
    bricks: ["proof", "test", "status"]
    reads: ["maestro status", "test output"]
    allowed_verbs: ["maestro task verify <id>", "cargo test <target>", "maestro status"]
    forbidden_verbs: ["claim success without proof"]
    checks: ["evidence backs every claim"]
    durable_learning: []
    outputs: ["verified result", "failed proof", "hard stop"]
  learn:
    goal: Preserve reusable corrections only when they will guide future work.
    bricks: ["event", "memory", "note"]
    reads: ["failed proof", "user correction"]
    allowed_verbs: ["maestro event intervention --note <text>", "maestro card note <id> <text>"]
    forbidden_verbs: ["chat-only learning"]
    checks: ["lesson has a durable source when needed"]
    durable_learning: ["card note", "intervention event", "memory candidate"]
    outputs: ["recorded lesson", "no learning needed"]
  continue:
    goal: Return the next gate, next task, or hard stop.
    bricks: ["status", "task next", "report"]
    reads: ["maestro status", "maestro task next"]
    allowed_verbs: ["maestro status", "maestro task next"]
    forbidden_verbs: ["external ship action"]
    checks: ["next step is explicit"]
    durable_learning: []
    outputs: ["next gate", "next task", "hard stop"]
"#
}

/// Every shipped structured recipe contract name, sorted.
pub fn contract_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = LOOP_RECIPE_CONTRACTS_DIR
        .files()
        .filter_map(|file| {
            let name = file
                .path()
                .strip_prefix(LOOP_RECIPE_CONTRACTS_DIR.path())
                .ok()
                .and_then(|path| path.to_str())?;
            name.strip_suffix(".yml")
        })
        .collect();
    names.sort_unstable();
    names
}

/// Parse and validate every shipped structured lifecycle recipe contract.
pub fn contracts() -> Result<Vec<RecipeContract>> {
    let contracts = contract_names()
        .into_iter()
        .map(contract)
        .collect::<Result<Vec<_>>>()?;
    ensure_contract_set(&contracts)?;
    Ok(contracts)
}

/// Parse and validate one shipped structured lifecycle recipe contract.
pub fn contract(name: &str) -> Result<RecipeContract> {
    let file_name = format!("{name}.yml");
    let body = LOOP_RECIPE_CONTRACTS_DIR
        .get_file(LOOP_RECIPE_CONTRACTS_DIR.path().join(&file_name))
        .and_then(|file| file.contents_utf8())
        .with_context(|| {
            format!(
                "unknown loop recipe contract \"{name}\"; available: {}",
                contract_names().join(", ")
            )
        })?;
    let contract = parse_contract_body(name, body)?;
    validate_edge_targets(&contract, &allowed_edge_targets(&[]))?;
    ensure!(
        contract.id == name,
        "recipe contract {name} id mismatch: {}",
        contract.id
    );
    Ok(contract)
}

pub fn custom_contracts(custom_dir: &Path) -> Result<Vec<RecipeContract>> {
    let names = custom_contract_names(custom_dir)?;
    names
        .iter()
        .map(|name| custom_contract_known_with_names(custom_dir, name, &names))
        .collect()
}

pub fn custom_contract_names(custom_dir: &Path) -> Result<Vec<String>> {
    let Some(metadata) = custom_recipe_dir_metadata(custom_dir)? else {
        return Ok(Vec::new());
    };
    if !metadata.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(custom_dir).with_context(|| {
        format!(
            "failed to read custom loop recipe dir {}",
            custom_dir.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "failed to read custom loop recipe entry in {}",
                custom_dir.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect custom loop recipe {}", path.display()))?;
        ensure!(
            !file_type.is_symlink(),
            "custom loop recipe {} is a symlink; refusing to read it",
            path.display()
        );
        if !file_type.is_file() {
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("yml") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        ensure!(
            !CANONICAL_RECIPE_IDS.contains(&name) && !LEGACY_RECIPE_IDS.contains(&name),
            "custom loop recipe {name}.yml collides with a shipped or legacy recipe id"
        );
        names.push(name.to_string());
    }
    names.sort();
    Ok(names)
}

pub fn custom_contract(custom_dir: &Path, name: &str) -> Result<RecipeContract> {
    let path = custom_contract_path(custom_dir, name)?;
    let names = custom_contract_names(custom_dir)?;
    read_custom_contract(&path, name, &names)
}

fn custom_contract_known(custom_dir: &Path, name: &str) -> Result<RecipeContract> {
    let names = custom_contract_names(custom_dir)?;
    custom_contract_known_with_names(custom_dir, name, &names)
}

fn custom_contract_known_with_names(
    custom_dir: &Path,
    name: &str,
    custom_names: &[String],
) -> Result<RecipeContract> {
    let path = custom_contract_file_path(custom_dir, name)?;
    read_custom_contract(&path, name, custom_names)
}

fn read_custom_contract(
    path: &Path,
    name: &str,
    custom_names: &[String],
) -> Result<RecipeContract> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect custom loop recipe {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "custom loop recipe {} is a symlink; refusing to read it",
        path.display()
    );
    ensure!(
        metadata.is_file(),
        "custom loop recipe {} is not a regular file",
        path.display()
    );
    let body = fs::read_to_string(path)
        .with_context(|| format!("failed to read custom loop recipe {}", path.display()))?;
    let contract = parse_contract_body(name, &body)
        .with_context(|| format!("invalid custom loop recipe {name}.yml"))?;
    validate_edge_targets(&contract, &allowed_edge_targets(custom_names))
        .with_context(|| format!("invalid custom loop recipe {name}.yml"))?;
    ensure!(
        contract.id == name,
        "custom loop recipe {name} id mismatch: {}",
        contract.id
    );
    Ok(contract)
}

fn custom_recipe_dir_metadata(custom_dir: &Path) -> Result<Option<fs::Metadata>> {
    let metadata = match fs::symlink_metadata(custom_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", custom_dir.display()));
        }
    };
    ensure!(
        !metadata.file_type().is_symlink(),
        "custom loop recipe dir {} is a symlink; refusing to read it",
        custom_dir.display()
    );
    Ok(Some(metadata))
}

pub fn validate_contract(contract: &RecipeContract) -> Result<()> {
    ensure!(
        contract.schema_version == CONTRACT_SCHEMA_VERSION,
        "recipe {} uses schema_version {}, expected {CONTRACT_SCHEMA_VERSION}",
        contract.id,
        contract.schema_version
    );
    ensure!(
        !LEGACY_RECIPE_IDS.contains(&contract.id.as_str()),
        "recipe {} uses legacy id; use a canonical recipe id",
        contract.id
    );
    require_non_empty("id", &contract.id)?;
    require_non_empty("kind.category", &contract.kind.category)?;
    require_non_empty_list("kind.tags", &contract.kind.tags)?;
    require_non_empty("title", &contract.title)?;
    require_non_empty("summary", &contract.summary)?;
    require_non_empty_list("applies_when", &contract.applies_when)?;
    require_non_empty_list("authority_scope", &contract.authority_scope)?;
    require_non_empty_list("autonomy", &contract.autonomy)?;
    require_non_empty_list("hard_stops", &contract.hard_stops)?;
    require_non_empty_list("outputs", &contract.outputs)?;
    validate_progress_tasks(&contract.id, &contract.progress_tasks)?;
    require_non_empty("router.status", &contract.router.status)?;
    require_non_empty("router.confidence", &contract.router.confidence)?;
    ensure!(
        contract.router.priority > 0,
        "router.priority must be non-zero"
    );
    validate_edges(&contract.id, "transitions", &contract.transitions)?;
    validate_edges(&contract.id, "invocations", &contract.invocations)?;
    reject_forbidden_text(contract)?;

    let actual: BTreeSet<&str> = contract.phases.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = REQUIRED_PHASES.into_iter().collect();
    ensure!(
        actual == expected,
        "recipe {} phases must be exactly {:?}; found {:?}",
        contract.id,
        expected,
        actual
    );

    for phase_name in REQUIRED_PHASES {
        let phase = contract
            .phases
            .get(phase_name)
            .expect("invariant: required phase set was checked");
        validate_phase(&contract.id, phase_name, phase)?;
    }
    Ok(())
}

fn render_contract(contract: &RecipeContract) -> String {
    let mut out = format!(
        "# {}\n\nschema_version: {}\nid: {}\nkind: {}\ntags: {}\n\n{}\n\n",
        contract.title,
        contract.schema_version,
        contract.id,
        contract.kind.category,
        contract.kind.tags.join(", "),
        contract.summary
    );
    out.push_str("## Router Metadata\n\n");
    out.push_str(&format!(
        "- status: {}\n- priority: {}\n- confidence: {}\n",
        contract.router.status, contract.router.priority, contract.router.confidence
    ));
    out.push_str("\n## Authority Scope\n\n");
    push_bullets(&mut out, "", &contract.authority_scope);
    out.push_str("\n## Autonomy\n\n");
    push_bullets(&mut out, "", &contract.autonomy);
    out.push_str("## Applies When\n\n");
    push_bullets(&mut out, "", &contract.applies_when);
    out.push_str("\n## Hard Stops\n\n");
    push_bullets(&mut out, "", &contract.hard_stops);
    out.push_str("\n## Outputs\n\n");
    push_bullets(&mut out, "", &contract.outputs);
    render_progress_tasks(&mut out, &contract.progress_tasks);
    render_edges(&mut out, "Transitions", &contract.transitions);
    render_edges(&mut out, "Invocations", &contract.invocations);
    out.push_str("\n## Custom Recipe Policy\n\n");
    push_bullets(&mut out, "", &CUSTOM_RECIPE_POLICY);
    out.push_str(
        "\n## Loop Grammar\n\nperceive -> choose -> act -> observe -> learn -> continue\n\n",
    );
    out.push_str("## Phases\n\n");
    for name in REQUIRED_PHASES {
        let phase = contract
            .phases
            .get(name)
            .expect("invariant: contract validates before rendering");
        render_phase(&mut out, name, phase);
    }
    out
}

fn compact_packet_for_contract(
    contract: &RecipeContract,
    phase: Option<&str>,
) -> Result<LoopCompactPacket> {
    let phase_name = phase.unwrap_or("perceive");
    ensure!(
        REQUIRED_PHASES.contains(&phase_name),
        "unknown loop recipe phase {phase_name:?}; expected one of {}",
        REQUIRED_PHASES.join(", ")
    );
    let phase = contract
        .phases
        .get(phase_name)
        .with_context(|| format!("recipe {} does not define phase {phase_name}", contract.id))?;
    let progress_task = contract
        .progress_tasks
        .iter()
        .find(|task| task.phase == phase_name)
        .with_context(|| {
            format!(
                "recipe {} has no progress task for phase {phase_name}",
                contract.id
            )
        })?;
    Ok(LoopCompactPacket {
        schema: LOOP_COMPACT_PACKET_SCHEMA,
        recipe: contract.id.clone(),
        phase: phase_name.to_string(),
        progress_task: progress_task.id.clone(),
        reads: phase.reads.clone(),
        allowed_verbs: phase.allowed_verbs.clone(),
        forbidden_verbs: phase.forbidden_verbs.clone(),
        checks: phase.checks.clone(),
        hard_stops: contract.hard_stops.clone(),
        next: phase.outputs.clone(),
    })
}

fn default_phase_for_next(recipe: &str, reason: &str) -> &'static str {
    if recipe == "work" {
        if reason.contains("needs_verification") {
            return "observe";
        }
        if reason.contains("in-progress")
            || reason.contains("in_progress")
            || reason.contains("current task is live")
        {
            return "act";
        }
    }
    "perceive"
}

fn render_edges(out: &mut String, title: &str, edges: &[RecipeEdge]) {
    if edges.is_empty() {
        return;
    }
    out.push_str(&format!("\n## {title}\n\n"));
    for edge in edges {
        out.push_str(&format!("- {} -> {}\n", edge.trigger, edge.to));
        push_nested_named_list(out, "authority_scope", &edge.authority_scope);
        push_nested_named_list(out, "allowed_verbs", &edge.allowed_verbs);
        push_nested_named_list(out, "forbidden_verbs", &edge.forbidden_verbs);
        push_nested_named_list(out, "hard_stops", &edge.hard_stops);
        out.push_str(&format!(
            "  - return_condition: {}\n",
            edge.return_condition
        ));
    }
}

fn render_progress_tasks(out: &mut String, tasks: &[ProgressTaskContract]) {
    out.push_str("\n## Progress Tasks\n\n");
    for task in tasks {
        out.push_str(&format!(
            "- {} [{} required={}]: {}\n",
            task.id, task.phase, task.required, task.title
        ));
        out.push_str(&format!("  - done_check: {}\n", task.done_check));
    }
}

fn render_phase(out: &mut String, name: &str, phase: &PhaseContract) {
    out.push_str(&format!("### {name}\n\n"));
    out.push_str(&format!("- Goal: {}\n", phase.goal));
    push_named_list(out, "Bricks", &phase.bricks);
    push_named_list(out, "Reads", &phase.reads);
    push_named_list(out, "Allowed verbs", &phase.allowed_verbs);
    push_named_list(out, "Forbidden verbs", &phase.forbidden_verbs);
    push_named_list(out, "Checks", &phase.checks);
    push_named_list(out, "Durable learning", &phase.durable_learning);
    push_named_list(out, "Outputs", &phase.outputs);
    if !phase.optional_helpers.is_empty() {
        push_named_list(out, "Optional helpers", &phase.optional_helpers);
    }
    if let Some(helper) = phase
        .helper_contract
        .as_ref()
        .and_then(|contract| contract.work_lease.as_ref())
    {
        out.push_str("- Work Lease helper contract:\n");
        push_nested_named_list(out, "selected_unit", &helper.selected_unit);
        push_nested_named_list(out, "authority_scope", &helper.authority_scope);
        push_nested_named_list(out, "claim_or_reservation", &helper.claim_or_reservation);
        push_nested_named_list(
            out,
            "expires_or_stale_policy",
            &helper.expires_or_stale_policy,
        );
        push_nested_named_list(
            out,
            "allowed_follow_up_verbs",
            &helper.allowed_follow_up_verbs,
        );
        push_nested_named_list(out, "hard_stops", &helper.hard_stops);
        push_nested_named_list(out, "observe_requirement", &helper.observe_requirement);
        push_nested_named_list(out, "reconcile_handles", &helper.reconcile_handles);
    }
    out.push('\n');
}

fn push_named_list(out: &mut String, name: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(&format!("- {name}:\n"));
    push_bullets(out, "  ", values);
}

fn push_nested_named_list(out: &mut String, name: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(&format!("  - {name}:\n"));
    push_bullets(out, "    ", values);
}

fn push_bullets<S: AsRef<str>>(out: &mut String, indent: &str, values: &[S]) {
    for value in values {
        out.push_str(&format!("{indent}- {}\n", value.as_ref()));
    }
}

fn push_compact_list(out: &mut String, name: &str, values: &[String]) {
    out.push_str(&format!("{name}:\n"));
    push_bullets(out, "  ", values);
}

fn parse_contract_body(name: &str, body: &str) -> Result<RecipeContract> {
    let contract: RecipeContract = serde_yaml::from_str(body)
        .with_context(|| format!("failed to parse loop recipe contract {name}.yml"))?;
    validate_contract(&contract)
        .with_context(|| format!("invalid loop recipe contract {name}.yml"))?;
    Ok(contract)
}

fn ensure_contract_set(contracts: &[RecipeContract]) -> Result<()> {
    let names: BTreeSet<&str> = contracts
        .iter()
        .map(|contract| contract.id.as_str())
        .collect();
    for expected in CANONICAL_RECIPE_IDS {
        ensure!(
            names.contains(expected),
            "loop recipe contracts are missing {expected}.yml"
        );
    }
    ensure!(
        names.len() == CANONICAL_RECIPE_IDS.len(),
        "loop recipe contract set drifted: expected {:?}, found {:?}",
        CANONICAL_RECIPE_IDS,
        names
    );
    for legacy in LEGACY_RECIPE_IDS {
        ensure!(
            !names.contains(legacy),
            "legacy recipe id {legacy} must not be shipped as an alias"
        );
    }
    ensure!(
        contracts.iter().any(contract_supports_work_lease),
        "at least one loop recipe contract must declare the work lease choose helper"
    );
    Ok(())
}

fn available_names() -> Vec<&'static str> {
    let mut names = contract_names();
    names.sort_unstable();
    names
}

fn available_names_with_custom(custom_dir: Option<&Path>) -> Result<Vec<String>> {
    let mut names: Vec<String> = contract_names().into_iter().map(str::to_string).collect();
    if let Some(custom_dir) = custom_dir {
        names.extend(custom_contract_names(custom_dir)?);
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn custom_contract_path(custom_dir: &Path, name: &str) -> Result<PathBuf> {
    let path = custom_contract_file_path(custom_dir, name)?;
    let names = custom_contract_names(custom_dir)?;
    ensure!(
        names.iter().any(|custom| custom == name),
        "unknown custom loop recipe \"{name}\" in {}",
        custom_dir.display()
    );
    Ok(path)
}

fn custom_contract_file_path(custom_dir: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        !name.contains('/') && !name.contains('\\') && name != "." && name != "..",
        "custom loop recipe name must be a file stem, got {name:?}"
    );
    Ok(custom_dir.join(format!("{name}.yml")))
}

fn contract_supports_work_lease(contract: &RecipeContract) -> bool {
    contract
        .phases
        .get("choose")
        .is_some_and(has_work_lease_helper)
}

fn validate_phase(recipe_id: &str, name: &str, phase: &PhaseContract) -> Result<()> {
    let prefix = format!("recipe {recipe_id} phase {name}");
    require_non_empty(&format!("{prefix}.goal"), &phase.goal)?;
    require_non_empty_list(&format!("{prefix}.bricks"), &phase.bricks)?;
    require_non_empty_list(&format!("{prefix}.reads"), &phase.reads)?;
    require_non_empty_list(&format!("{prefix}.checks"), &phase.checks)?;
    ensure!(
        !phase.allowed_verbs.is_empty() || !phase.forbidden_verbs.is_empty(),
        "{prefix} must declare allowed_verbs or forbidden_verbs"
    );
    if name == "learn" {
        require_non_empty_list(
            &format!("{prefix}.durable_learning"),
            &phase.durable_learning,
        )?;
    }
    if name == "continue" {
        require_non_empty_list(&format!("{prefix}.outputs"), &phase.outputs)?;
    }
    if has_work_lease_helper(phase) {
        let helper = phase
            .helper_contract
            .as_ref()
            .and_then(|contract| contract.work_lease.as_ref())
            .with_context(|| {
                format!("{prefix} declares optional helper work lease but omits helper_contract.work_lease")
            })?;
        validate_work_lease_helper(&prefix, helper)?;
    }
    Ok(())
}

fn has_work_lease_helper(phase: &PhaseContract) -> bool {
    phase
        .optional_helpers
        .iter()
        .any(|helper| helper == "work lease")
}

fn validate_work_lease_helper(prefix: &str, helper: &WorkLeaseHelperContract) -> Result<()> {
    require_non_empty_list(
        &format!("{prefix}.work_lease.selected_unit"),
        &helper.selected_unit,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.authority_scope"),
        &helper.authority_scope,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.claim_or_reservation"),
        &helper.claim_or_reservation,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.expires_or_stale_policy"),
        &helper.expires_or_stale_policy,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.allowed_follow_up_verbs"),
        &helper.allowed_follow_up_verbs,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.hard_stops"),
        &helper.hard_stops,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.observe_requirement"),
        &helper.observe_requirement,
    )?;
    require_non_empty_list(
        &format!("{prefix}.work_lease.reconcile_handles"),
        &helper.reconcile_handles,
    )?;
    Ok(())
}

fn validate_progress_tasks(recipe_id: &str, tasks: &[ProgressTaskContract]) -> Result<()> {
    ensure!(
        !tasks.is_empty(),
        "recipe {recipe_id}.progress_tasks must not be empty"
    );
    let valid_phases: BTreeSet<&str> = REQUIRED_PHASES.into_iter().collect();
    let mut ids = BTreeSet::new();
    for (index, task) in tasks.iter().enumerate() {
        let prefix = format!("recipe {recipe_id}.progress_tasks[{index}]");
        require_non_empty(&format!("{prefix}.id"), &task.id)?;
        ensure!(
            ids.insert(task.id.as_str()),
            "{prefix}.id duplicates progress task id {}",
            task.id
        );
        require_non_empty(&format!("{prefix}.title"), &task.title)?;
        require_non_empty(&format!("{prefix}.phase"), &task.phase)?;
        ensure!(
            valid_phases.contains(task.phase.as_str()),
            "{prefix}.phase references unknown phase {}",
            task.phase
        );
        require_non_empty(&format!("{prefix}.done_check"), &task.done_check)?;
    }
    Ok(())
}

fn validate_edges(recipe_id: &str, field: &str, edges: &[RecipeEdge]) -> Result<()> {
    for (index, edge) in edges.iter().enumerate() {
        let prefix = format!("recipe {recipe_id}.{field}[{index}]");
        require_non_empty(&format!("{prefix}.trigger"), &edge.trigger)?;
        require_non_empty(&format!("{prefix}.to"), &edge.to)?;
        require_non_empty_list(&format!("{prefix}.authority_scope"), &edge.authority_scope)?;
        require_non_empty_list(&format!("{prefix}.allowed_verbs"), &edge.allowed_verbs)?;
        require_non_empty_list(&format!("{prefix}.forbidden_verbs"), &edge.forbidden_verbs)?;
        require_non_empty_list(&format!("{prefix}.hard_stops"), &edge.hard_stops)?;
        require_non_empty(
            &format!("{prefix}.return_condition"),
            &edge.return_condition,
        )?;
    }
    Ok(())
}

fn allowed_edge_targets(custom_names: &[String]) -> BTreeSet<String> {
    let mut names = CANONICAL_RECIPE_IDS
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    names.extend(custom_names.iter().cloned());
    names
}

fn validate_edge_targets(contract: &RecipeContract, allowed: &BTreeSet<String>) -> Result<()> {
    for (field, edges) in [
        ("transitions", contract.transitions.as_slice()),
        ("invocations", contract.invocations.as_slice()),
    ] {
        for (index, edge) in edges.iter().enumerate() {
            ensure!(
                allowed.contains(&edge.to),
                "recipe {}.{}[{}].to references unknown recipe {}",
                contract.id,
                field,
                index,
                edge.to
            );
        }
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{field} must not be empty");
    Ok(())
}

fn require_non_empty_list(field: &str, values: &[String]) -> Result<()> {
    ensure!(!values.is_empty(), "{field} must not be empty");
    for value in values {
        require_non_empty(field, value)?;
    }
    Ok(())
}

fn reject_forbidden_text(contract: &RecipeContract) -> Result<()> {
    let mut values = Vec::new();
    values.extend([
        contract.id.as_str(),
        contract.kind.category.as_str(),
        contract.title.as_str(),
        contract.summary.as_str(),
        contract.router.status.as_str(),
        contract.router.confidence.as_str(),
    ]);
    values.extend(contract.kind.tags.iter().map(String::as_str));
    values.extend(contract.applies_when.iter().map(String::as_str));
    values.extend(contract.authority_scope.iter().map(String::as_str));
    values.extend(contract.autonomy.iter().map(String::as_str));
    values.extend(contract.hard_stops.iter().map(String::as_str));
    values.extend(contract.outputs.iter().map(String::as_str));
    for task in &contract.progress_tasks {
        values.extend([
            task.id.as_str(),
            task.title.as_str(),
            task.phase.as_str(),
            task.done_check.as_str(),
        ]);
    }
    for edge in contract
        .transitions
        .iter()
        .chain(contract.invocations.iter())
    {
        values.extend([
            edge.trigger.as_str(),
            edge.to.as_str(),
            edge.return_condition.as_str(),
        ]);
        values.extend(edge.authority_scope.iter().map(String::as_str));
        values.extend(edge.allowed_verbs.iter().map(String::as_str));
        values.extend(edge.forbidden_verbs.iter().map(String::as_str));
        values.extend(edge.hard_stops.iter().map(String::as_str));
    }
    for phase in contract.phases.values() {
        values.extend([phase.goal.as_str()]);
        values.extend(phase.bricks.iter().map(String::as_str));
        values.extend(phase.reads.iter().map(String::as_str));
        values.extend(phase.allowed_verbs.iter().map(String::as_str));
        values.extend(phase.forbidden_verbs.iter().map(String::as_str));
        values.extend(phase.checks.iter().map(String::as_str));
        values.extend(phase.durable_learning.iter().map(String::as_str));
        values.extend(phase.outputs.iter().map(String::as_str));
        values.extend(phase.optional_helpers.iter().map(String::as_str));
        if let Some(helper) = phase
            .helper_contract
            .as_ref()
            .and_then(|contract| contract.work_lease.as_ref())
        {
            values.extend(helper.selected_unit.iter().map(String::as_str));
            values.extend(helper.authority_scope.iter().map(String::as_str));
            values.extend(helper.claim_or_reservation.iter().map(String::as_str));
            values.extend(helper.expires_or_stale_policy.iter().map(String::as_str));
            values.extend(helper.allowed_follow_up_verbs.iter().map(String::as_str));
            values.extend(helper.hard_stops.iter().map(String::as_str));
            values.extend(helper.observe_requirement.iter().map(String::as_str));
            values.extend(helper.reconcile_handles.iter().map(String::as_str));
        }
    }
    for value in values {
        let lower = value.to_ascii_lowercase();
        for phrase in FORBIDDEN_BYPASS_PHRASES {
            ensure!(
                !lower.contains(phrase),
                "recipe {} contains forbidden lifecycle-bypass wording: {phrase}",
                contract.id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_every_recipe_from_the_structured_catalog() {
        for name in contract_names() {
            let rendered = serve(name).expect("recipe should render");
            assert!(
                rendered.contains(CONTRACT_SCHEMA_VERSION),
                "{name}: {rendered}"
            );
            assert!(
                rendered.contains(&format!("id: {name}")),
                "{name}: {rendered}"
            );
        }
    }

    #[test]
    fn ships_expected_canonical_structured_recipe_contracts() {
        let names = contract_names();
        assert_eq!(
            names, CANONICAL_RECIPE_IDS,
            "structured recipe contract set drifted"
        );
        for legacy in LEGACY_RECIPE_IDS {
            assert!(
                !names.contains(&legacy),
                "legacy recipe id {legacy} must not be accepted as a shipped alias"
            );
        }
    }

    #[test]
    fn validates_every_shipped_structured_recipe_contract() {
        let contracts = contracts().expect("shipped contracts should validate");
        assert_eq!(contracts.len(), CANONICAL_RECIPE_IDS.len());
        assert!(contracts.iter().any(contract_supports_work_lease));
    }

    #[test]
    fn rejects_contract_with_missing_required_field() {
        let body = "schema_version: maestro.recipe.v2\nid: broken\n";
        let error = parse_contract_body("broken", body).unwrap_err().to_string();
        assert!(error.contains("failed to parse"), "{error}");
    }

    #[test]
    fn rejects_legacy_recipe_id_as_alias() {
        let mut contract =
            contract("feature-fanout").expect("feature-fanout contract should validate");
        contract.id = "feature-fan-out".to_string();
        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(error.contains("legacy id"), "{error}");
    }

    #[test]
    fn rejects_contract_with_missing_phase() {
        let mut contract = contract("work").expect("work contract should validate");
        contract.phases.remove("learn");
        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(error.contains("phases must be exactly"), "{error}");
    }

    #[test]
    fn rejects_progress_task_duplicate_ids() {
        let mut contract = contract("work").expect("work contract should validate");
        contract.progress_tasks[1].id = contract.progress_tasks[0].id.clone();

        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(error.contains("duplicates progress task id"), "{error}");
    }

    #[test]
    fn rejects_progress_task_unknown_phase() {
        let mut contract = contract("work").expect("work contract should validate");
        contract.progress_tasks[0].phase = "invalid-phase".to_string();

        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(error.contains("invalid-phase"), "{error}");
        assert!(error.contains("progress_tasks"), "{error}");
    }

    #[test]
    fn rejects_progress_task_blank_done_check() {
        let mut contract = contract("work").expect("work contract should validate");
        contract.progress_tasks[0].done_check.clear();

        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(error.contains("done_check must not be empty"), "{error}");
    }

    #[test]
    fn rejects_work_lease_helper_missing_required_fields() {
        let mut contract = contract("unattended").expect("unattended contract should validate");
        let helper = contract
            .phases
            .get_mut("choose")
            .and_then(|phase| phase.helper_contract.as_mut())
            .and_then(|helper| helper.work_lease.as_mut())
            .expect("unattended choose phase should declare work lease helper");
        helper.reconcile_handles.clear();

        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(error.contains("reconcile_handles"), "{error}");
    }

    #[test]
    fn rejects_forbidden_lifecycle_bypass_wording() {
        let mut contract = contract("design").expect("design contract should validate");
        contract.summary = "agent may bypass acceptance when convenient".to_string();
        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(
            error.contains("forbidden lifecycle-bypass wording"),
            "{error}"
        );
    }

    #[test]
    fn rejects_forbidden_work_lease_helper_wording() {
        let mut contract = contract("unattended").expect("unattended contract should validate");
        let helper = contract
            .phases
            .get_mut("choose")
            .and_then(|phase| phase.helper_contract.as_mut())
            .and_then(|helper| helper.work_lease.as_mut())
            .expect("unattended choose phase should declare work lease helper");
        helper
            .allowed_follow_up_verbs
            .push("launch workers from this contract".to_string());

        let error = validate_contract(&contract).unwrap_err().to_string();
        assert!(
            error.contains("forbidden lifecycle-bypass wording"),
            "{error}"
        );
    }

    #[test]
    fn index_lists_every_canonical_recipe() {
        let idx = index();
        for name in contract_names() {
            assert!(idx.contains(name), "index lists recipe {name}");
        }
        assert!(idx.contains("## Custom Recipe Policy"), "{idx}");
        assert!(idx.contains("Maestro is the loop"), "{idx}");
    }

    #[test]
    fn show_renders_structured_contract_from_yaml() {
        let body = show("design").expect("design contract should render");
        assert!(body.contains("# Design loop"), "{body}");
        assert!(body.contains("schema_version: maestro.recipe.v2"), "{body}");
        assert!(body.contains("## Router Metadata"), "{body}");
        assert!(body.contains("## Authority Scope"), "{body}");
        assert!(body.contains("## Autonomy"), "{body}");
        assert!(body.contains("## Applies When"), "{body}");
        assert!(body.contains("## Custom Recipe Policy"), "{body}");
        assert!(
            body.contains("perceive -> choose -> act -> observe -> learn -> continue"),
            "{body}"
        );
        assert!(body.contains("### perceive"), "{body}");
        assert!(body.contains("### continue"), "{body}");
    }

    #[test]
    fn show_renders_work_lease_helper_details() {
        let body = show("unattended").expect("unattended contract should render");
        assert!(body.contains("Optional helpers"), "{body}");
        assert!(body.contains("work lease"), "{body}");
        assert!(body.contains("Work Lease helper contract"), "{body}");
        assert!(body.contains("selected_unit"), "{body}");
        assert!(body.contains("reconcile_handles"), "{body}");
    }

    #[test]
    fn show_renders_migrated_orchestration_recipes_from_yaml() {
        let body = show("conflict-handoff").expect("migrated recipe should render");
        assert!(body.contains("# Conflict handoff"), "{body}");
        assert!(body.contains("git worktree add"), "{body}");
        assert!(body.contains("schema_version: maestro.recipe.v2"), "{body}");
    }

    #[test]
    fn route_next_recommends_work_for_current_task_with_edges() {
        let task = task_input("task-router", "in_progress", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            current_task: Some(task.clone()),
            tasks: vec![task],
            ..LoopRouterInput::default()
        })
        .expect("router should recommend work");

        assert_eq!(report.status, "recommended");
        assert_eq!(report.recommended_recipe.as_deref(), Some("work"));
        assert_eq!(report.recommended_status, "work");
        assert!(report.reason.contains("task-router"), "{report:?}");
        assert!(
            report
                .inspect
                .contains(&"maestro task show task-router".to_string())
        );
        assert!(report.edges.iter().any(|edge| {
            edge.kind == "transition" && edge.to == "design" && edge.trigger.contains("too unclear")
        }));
        assert!(
            report
                .edges
                .iter()
                .any(|edge| { edge.kind == "invocation" && edge.to == "audit" })
        );
    }

    #[test]
    fn route_next_includes_context_refs_and_base_constraints() {
        let task = task_input("task-router", "in_progress", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            current_task: Some(task.clone()),
            tasks: vec![task],
            active_sessions: 3,
            git: Some(LoopGitInput {
                branch: Some("main".to_string()),
                code_other_dirty: 1,
                maestro_dirty: 2,
                ahead: 0,
                behind: 0,
            }),
            ..LoopRouterInput::default()
        })
        .expect("router should recommend work");

        assert!(
            report
                .context_refs
                .iter()
                .any(|reference| reference.kind == "task"
                    && reference.id.as_deref() == Some("task-router")),
            "{report:#?}"
        );
        assert!(
            report
                .context_refs
                .iter()
                .any(|reference| reference.kind == "git"
                    && reference.command.as_deref() == Some("git status --short --branch")),
            "{report:#?}"
        );

        let constraint_ids = report
            .constraints
            .iter()
            .map(|constraint| constraint.id.as_str())
            .collect::<BTreeSet<_>>();
        for expected in [
            "authority_ok",
            "scope_clear",
            "selected_unit_ok",
            "proof_ready",
            "qa_ready",
            "dirty_tree_risk",
            "conflict_risk",
            "memory_relevance",
            "prior_failure_risk",
            "route_confidence",
            "ship_gate_ok",
            "human_approval_ok",
        ] {
            assert!(constraint_ids.contains(expected), "{constraint_ids:?}");
        }
        assert!(report.constraints.iter().any(|constraint| {
            constraint.id == "dirty_tree_risk" && constraint.status == LoopConstraintStatus::Warn
        }));
        assert!(report.constraints.iter().any(|constraint| {
            constraint.id == "human_approval_ok" && constraint.status == LoopConstraintStatus::Pass
        }));
    }

    #[test]
    fn route_next_scores_phase_attempt_policy_and_why_not_alternatives() {
        let task = task_input("task-router", "ready", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            tasks: vec![task],
            features: vec![feature_input("feature-router", "in_progress", 1, 1, 0)],
            active_conflicts: 1,
            active_sessions: 2,
            ..LoopRouterInput::default()
        })
        .expect("router should recommend conflict handoff before work or ship");

        assert_eq!(
            report.recommended_recipe.as_deref(),
            Some("conflict-handoff")
        );
        assert_eq!(report.recommended_phase.as_deref(), Some("perceive"));
        assert!(
            report.score.is_some_and(|score| score > 0 && score <= 100),
            "{report:#?}"
        );
        let attempt_policy = report
            .attempt_policy
            .as_ref()
            .expect("recommended route should carry attempt policy");
        assert_eq!(attempt_policy.max_attempts, 1);
        assert!(
            attempt_policy
                .retry_after
                .iter()
                .any(|item| item.contains("LoopOutcome"))
        );
        assert!(report.why_not.iter().any(|why_not| {
            why_not.recipe == "ship"
                && why_not
                    .blocked_by
                    .iter()
                    .any(|blocked_by| blocked_by == "conflict_risk")
        }));
        assert!(report.why_not.iter().any(|why_not| {
            why_not.recipe == "work"
                && why_not
                    .blocked_by
                    .iter()
                    .any(|blocked_by| blocked_by == "conflict_risk")
        }));
    }

    #[test]
    fn route_next_memory_hits_shape_memory_constraints_without_overriding_live_truth() {
        let task = task_input("task-router", "ready", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            tasks: vec![task],
            memory_hits: vec![
                LoopMemoryHit {
                    id: "mem-failure".to_string(),
                    kind: "prior_failure".to_string(),
                    reason: "approved memory matched task-router".to_string(),
                    source_refs: vec![LoopContextRef::command(
                        "memory",
                        Some("mem-failure".to_string()),
                        "maestro memory show mem-failure",
                    )],
                },
                LoopMemoryHit {
                    id: "msug-success".to_string(),
                    kind: "success_pattern".to_string(),
                    reason: "open memory suggestion matched feature-router".to_string(),
                    source_refs: vec![LoopContextRef::command(
                        "memory_suggestion",
                        Some("msug-success".to_string()),
                        "maestro memory suggest list",
                    )],
                },
            ],
            ..LoopRouterInput::default()
        })
        .expect("router should keep live task route with advisory memory");

        assert_eq!(report.recommended_recipe.as_deref(), Some("work"));
        assert_eq!(report.memory_hits.len(), 2);
        assert!(report.constraints.iter().any(|constraint| {
            constraint.id == "memory_relevance" && constraint.status == LoopConstraintStatus::Pass
        }));
        assert!(report.constraints.iter().any(|constraint| {
            constraint.id == "prior_failure_risk" && constraint.status == LoopConstraintStatus::Warn
        }));
    }

    #[test]
    fn route_next_fails_closed_for_blocked_current_task() {
        let mut task = task_input("task-blocked", "in_progress", Some("feature-router"));
        task.blocked = true;
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            current_task: Some(task.clone()),
            tasks: vec![task],
            ..LoopRouterInput::default()
        })
        .expect("router should fail closed for blocked current task");

        assert_eq!(report.status, "uncertain");
        assert_eq!(report.recommended_recipe.as_deref(), None);
        assert!(
            report
                .reason
                .contains("current task task-blocked is blocked"),
            "{report:?}"
        );
        assert!(
            report
                .inspect
                .contains(&"maestro task show task-blocked".to_string()),
            "{report:?}"
        );
    }

    #[test]
    fn route_next_recommends_parallel_wave_as_work() {
        let first = task_input("task-one", "ready", Some("feature-router"));
        let second = task_input("task-two", "ready", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            tasks: vec![first, second],
            ..LoopRouterInput::default()
        })
        .expect("router should recommend work for the executable wave");

        assert_eq!(report.recommended_recipe.as_deref(), Some("work"));
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.reason.contains("2 executable tasks")),
            "{report:?}"
        );
    }

    #[test]
    fn route_next_prioritizes_ready_work_over_unrelated_design_backlog() {
        let task = task_input("task-ready", "ready", Some("feature-work"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            tasks: vec![task],
            features: vec![feature_input("feature-design", "proposed", 0, 0, 0)],
            ..LoopRouterInput::default()
        })
        .expect("router should prefer immediate work");

        assert_eq!(report.recommended_recipe.as_deref(), Some("work"));
        assert!(
            !report
                .candidates
                .iter()
                .any(|candidate| candidate.recipe == "design"),
            "{report:?}"
        );
    }

    #[test]
    fn route_next_uses_recipe_priority_before_task_state() {
        let task = task_input("task-router", "in_progress", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            current_task: Some(task.clone()),
            tasks: vec![task],
            active_sessions: 2,
            active_conflicts: 1,
            ..LoopRouterInput::default()
        })
        .expect("router should recommend conflict handling");

        assert_eq!(
            report.recommended_recipe.as_deref(),
            Some("conflict-handoff")
        );
        assert_eq!(report.priority, 40);
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.recipe == "work")
        );
        assert!(
            report
                .candidates
                .windows(2)
                .all(|pair| pair[0].priority >= pair[1].priority),
            "{:?}",
            report.candidates
        );
    }

    #[test]
    fn route_next_returns_uncertain_when_evidence_is_incomplete() {
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            ..LoopRouterInput::default()
        })
        .expect("router should return uncertain");

        assert_eq!(report.status, "uncertain");
        assert_eq!(report.recommended_recipe.as_deref(), None);
        assert_eq!(report.recommended_status, "uncertain");
        assert!(
            report
                .inspect
                .contains(&"maestro status --json".to_string())
        );
        assert!(
            report
                .hard_stops
                .iter()
                .any(|stop| stop.contains("do not mutate"))
        );
    }

    #[test]
    fn unknown_recipe_is_a_loud_error_listing_the_available_recipes() {
        let error = show("no-such-recipe").unwrap_err().to_string();
        assert!(error.contains("no-such-recipe"), "{error}");
        assert!(error.contains("design"), "{error}");
        assert!(error.contains("feature-fanout"), "{error}");
        assert!(!error.contains("feature-fan-out"), "{error}");
    }

    #[test]
    fn rejects_edge_targets_that_do_not_name_known_recipes() {
        let mut contract = contract("work").expect("work contract should validate");
        contract.transitions[0].to = "typo-recipe".to_string();
        let error = validate_edge_targets(&contract, &allowed_edge_targets(&[]))
            .unwrap_err()
            .to_string();

        assert!(error.contains("unknown recipe typo-recipe"), "{error}");
    }

    fn task_input(id: &str, state: &str, feature_id: Option<&str>) -> LoopTaskInput {
        LoopTaskInput {
            id: id.to_string(),
            title: format!("{id} title"),
            state: state.to_string(),
            feature_id: feature_id.map(str::to_string),
            blocked: false,
            ready_startable: state == "ready",
            gate: false,
            gate_kind: None,
            lane: Some("general".to_string()),
            remaining_blockers: Vec::new(),
        }
    }

    fn feature_input(
        id: &str,
        status: &str,
        total_tasks: usize,
        verified_tasks: usize,
        open_questions: usize,
    ) -> LoopFeatureInput {
        LoopFeatureInput {
            id: id.to_string(),
            title: format!("{id} title"),
            status: status.to_string(),
            total_tasks,
            verified_tasks,
            open_questions,
        }
    }
}
