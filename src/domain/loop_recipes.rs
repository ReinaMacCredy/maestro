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
use serde::Deserialize;

/// The structured recipe contract tree, embedded at build time.
static LOOP_RECIPE_CONTRACTS_DIR: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/embedded/loop-recipes");

const CONTRACT_SCHEMA_VERSION: &str = "maestro.recipe.v2";
const REQUIRED_PHASES: [&str; 6] = ["perceive", "choose", "act", "observe", "learn", "continue"];
const CANONICAL_RECIPE_IDS: [&str; 12] = [
    "adversarial-review",
    "audit",
    "conflict-handoff",
    "design",
    "feature-fanout",
    "generate-filter",
    "intake-triage",
    "learning",
    "loop-until-done",
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
        "Maestro is the loop: recipes are structured control grammar over current cards, tasks, features, decisions, proof, QA, run events, notes, memory, and skills. `maestro loop` is read-only; existing Maestro verbs perform writes.\n\n",
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
    if contract_names().contains(&name) {
        return show(name);
    }
    if let Some(custom_dir) = custom_dir
        && custom_contract_names(custom_dir)?
            .iter()
            .any(|custom| custom == name)
    {
        return Ok(render_contract(&custom_contract_known(custom_dir, name)?));
    }
    bail!(
        "unknown loop recipe \"{name}\"; run `maestro loop` for the index (available: {})",
        available_names_with_custom(custom_dir)?.join(", ")
    );
}

pub fn validate_with_custom_dir(name: &str, custom_dir: Option<&Path>) -> Result<String> {
    if contract_names().contains(&name) {
        contract(name)?;
        return Ok(format!("valid shipped loop recipe: {name}\n"));
    }
    if let Some(custom_dir) = custom_dir
        && custom_contract_names(custom_dir)?
            .iter()
            .any(|custom| custom == name)
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
    ensure!(
        contract.id == name,
        "recipe contract {name} id mismatch: {}",
        contract.id
    );
    Ok(contract)
}

pub fn custom_contracts(custom_dir: &Path) -> Result<Vec<RecipeContract>> {
    custom_contract_names(custom_dir)?
        .into_iter()
        .map(|name| custom_contract_known(custom_dir, &name))
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
        names.push(name.to_string());
    }
    names.sort();
    Ok(names)
}

pub fn custom_contract(custom_dir: &Path, name: &str) -> Result<RecipeContract> {
    let path = custom_contract_path(custom_dir, name)?;
    read_custom_contract(&path, name)
}

fn custom_contract_known(custom_dir: &Path, name: &str) -> Result<RecipeContract> {
    let path = custom_contract_file_path(custom_dir, name)?;
    read_custom_contract(&path, name)
}

fn read_custom_contract(path: &Path, name: &str) -> Result<RecipeContract> {
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
    fn unknown_recipe_is_a_loud_error_listing_the_available_recipes() {
        let error = show("no-such-recipe").unwrap_err().to_string();
        assert!(error.contains("no-such-recipe"), "{error}");
        assert!(error.contains("design"), "{error}");
        assert!(error.contains("feature-fanout"), "{error}");
        assert!(!error.contains("feature-fan-out"), "{error}");
    }
}
