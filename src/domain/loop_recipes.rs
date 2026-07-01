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
const REQUIRED_PHASES: [&str; 6] = ["perceive", "choose", "act", "observe", "learn", "continue"];
const CANONICAL_RECIPE_IDS: [&str; 13] = [
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
    pub active_sessions: usize,
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
pub struct LoopNextReport {
    pub schema: &'static str,
    pub status: String,
    pub repo: String,
    pub recommended_recipe: Option<String>,
    pub recommended_status: String,
    pub reason: String,
    pub confidence: String,
    pub priority: u16,
    pub authority_scope: Vec<String>,
    pub autonomy: Vec<String>,
    pub edges: Vec<LoopNextEdge>,
    pub hard_stops: Vec<String>,
    pub inspect: Vec<String>,
    pub next_verbs: Vec<String>,
    pub candidates: Vec<LoopNextCandidate>,
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
pub struct LoopNextGit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub code_other_dirty: usize,
    pub maestro_dirty: usize,
    pub ahead: usize,
    pub behind: usize,
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

    if input.active_sessions > 1 {
        candidates.push(RouterCandidate {
            recipe: "conflict-handoff",
            reason: format!(
                "{} active sessions are visible; check overlap before implementation or merge-back",
                input.active_sessions
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
    if let Some(task) = live_tasks.iter().find(|task| task.state == "ready") {
        candidates.push(work_candidate(task, "ready task can enter implementation"));
    }
    if let Some(task) = live_tasks.iter().find(|task| task.state == "in_progress") {
        candidates.push(work_candidate(
            task,
            "in-progress task should continue through work",
        ));
    }

    if input.current_task.is_none()
        && let Some((feature_id, ready_count)) = fanout_feature(&live_tasks)
    {
        candidates.push(RouterCandidate {
            recipe: "feature-fanout",
            reason: format!(
                "{ready_count} ready tasks share feature {feature_id}; fanout may be legal after independence checks"
            ),
            inspect: vec![
                format!("maestro task list --feature {feature_id}"),
                format!("maestro feature show {feature_id}"),
            ],
            next_verbs: vec![
                "maestro loop show feature-fanout".to_string(),
                format!("maestro task list --feature {feature_id}"),
            ],
        });
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
        reason: candidate.reason,
        confidence: contract.router.confidence.clone(),
        priority: contract.router.priority,
        authority_scope: contract.authority_scope.clone(),
        autonomy: contract.autonomy.clone(),
        edges: edge_reports(&contract),
        hard_stops: contract.hard_stops.clone(),
        inspect: candidate.inspect,
        next_verbs: candidate.next_verbs,
        candidates,
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
    Ok(LoopNextReport {
        schema: "maestro.loop_next.v1",
        status: "uncertain".to_string(),
        repo: input.repo.clone(),
        recommended_recipe: None,
        recommended_status: "uncertain".to_string(),
        reason: reason.to_string(),
        confidence: "low".to_string(),
        priority: 0,
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
        warnings: input.warnings.clone(),
        git: input.git.clone().map(LoopNextGit::from),
    })
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

fn fanout_feature(tasks: &[&LoopTaskInput]) -> Option<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for task in tasks.iter().filter(|task| task.state == "ready") {
        let Some(feature_id) = task.feature_id.as_deref() else {
            continue;
        };
        *counts.entry(feature_id.to_string()).or_default() += 1;
    }
    counts.into_iter().find(|(_, count)| *count >= 2)
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
    fn route_next_recommends_feature_fanout_before_single_ready_work() {
        let first = task_input("task-one", "ready", Some("feature-router"));
        let second = task_input("task-two", "ready", Some("feature-router"));
        let report = route_next(LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            tasks: vec![first, second],
            ..LoopRouterInput::default()
        })
        .expect("router should recommend fanout");

        assert_eq!(report.recommended_recipe.as_deref(), Some("feature-fanout"));
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.recipe == "work"),
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
