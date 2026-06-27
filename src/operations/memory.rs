use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store;
use crate::domain::memory::{
    CANDIDATE_FILE, Confidence, Freshness, Gate, GateRequired, LESSON_FILE, Links, MemoryCandidate,
    MemoryLifecycle, MemoryMetadata, MemoryScope, RECEIPTS_DIR, RiskAxes, RiskClassification,
    RiskLevel, Rollback, SIGNALS_FILE, ScopeKind, SignalSummary, SignalType, SourceRef,
    TargetSurface, TargetTier, candidate_path, forbidden_source_kind, validate_candidate_for_card,
    validate_card,
};
use crate::foundation::core::fs::{
    append_text_file, ensure_dir, read_to_string_if_exists, write_string_if_unchanged,
};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::utc_now_timestamp;

pub const SUGGESTION_SCHEMA_VERSION: &str = "maestro.memory.suggestion.v1";
pub const SCORER_RECEIPT_SCHEMA_VERSION: &str = "maestro.memory.scorer_receipt.v1";
pub const TARGET_REGISTRY_SCHEMA_VERSION: &str = "maestro.memory.target_registry.v1";
pub const PROMOTION_PLAN_SCHEMA_VERSION: &str = "maestro.memory.promotion_plan.v1";
pub const MAINTENANCE_CONTRACT_SCHEMA_VERSION: &str = "maestro.memory.maintenance_contract.v1";

#[derive(Clone, Debug, PartialEq)]
pub struct CreateSuggestionRequest {
    pub source_refs: Vec<SourceRef>,
    pub signal_type: SignalType,
    pub summary: String,
    pub scope: MemoryScope,
    pub target_surface: TargetSurface,
    pub dedupe_key: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateSuggestionOutcome {
    pub suggestion: MemorySuggestion,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DismissSuggestionOutcome {
    pub suggestion: MemorySuggestion,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateMemoryRequest {
    pub from: String,
    pub summary: Option<String>,
    pub lesson: Option<String>,
    pub signal_type: Option<SignalType>,
    pub scope: Option<MemoryScope>,
    pub target_surface: Option<TargetSurface>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateMemoryOutcome {
    pub id: String,
    pub from_suggestion: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttachScorerOutcome {
    pub id: String,
    pub scorer_type: ScorerType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScorerRunOutcome {
    pub receipt: ScorerReceipt,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanPromotionRequest {
    pub memory_id: String,
    pub scorer_receipt: Option<String>,
    pub review_evidence: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanPromotionOutcome {
    pub id: String,
    pub path: PathBuf,
    pub review_only: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplyPromotionRequest {
    pub promotion_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplyPromotionOutcome {
    pub id: String,
    pub target_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryReadScope {
    pub card_id: Option<String>,
    pub task_id: Option<String>,
    pub feature_id: Option<String>,
    pub project: Option<String>,
    pub query: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryReadSurface {
    Status,
    Resume,
    CardShow,
    Search,
    WorkLease,
    WorkerPrompt,
}

impl MemoryReadSurface {
    pub(crate) fn cap(self) -> usize {
        match self {
            Self::Status => 5,
            Self::Resume => 10,
            Self::CardShow => 10,
            Self::Search => 50,
            Self::WorkLease => 8,
            Self::WorkerPrompt => 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApprovedMemory {
    pub rank: usize,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub target_surface: TargetSurface,
    pub scope_kind: ScopeKind,
    pub scope_refs: Vec<String>,
    pub risk: RiskLevel,
    pub confidence: Confidence,
    pub lesson_path: String,
    pub show_command: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApprovedMemorySet {
    pub memories: Vec<ApprovedMemory>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemorySuggestionHint {
    pub rank: usize,
    pub id: String,
    pub summary: String,
    pub source_count: usize,
    pub signal_type: String,
    pub target_surface: String,
    pub scope_kind: String,
    pub create_command: String,
    pub dismiss_command: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemorySuggestionSet {
    pub suggestions: Vec<MemorySuggestionHint>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaintenanceRequest {
    pub level: MaintenanceLevel,
    pub scope: MemoryScope,
    pub source_refs: Vec<SourceRef>,
    pub reason: String,
    pub proof_links: Vec<String>,
    pub run_links: Vec<String>,
    pub human_approved: bool,
    pub explicit_budget: Option<MaintenanceBudget>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaintenanceOutcome {
    pub id: String,
    pub contract_path: Option<PathBuf>,
    pub level: MaintenanceLevel,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceLevel {
    L0Detect,
    L1LocalTidy,
    L2FocusedRepair,
    L3DeepRebuild,
}

impl MaintenanceLevel {
    pub fn parse(word: &str) -> Option<Self> {
        match word.to_ascii_lowercase().as_str() {
            "l0" | "l0_detect" => Some(Self::L0Detect),
            "l1" | "l1_local_tidy" => Some(Self::L1LocalTidy),
            "l2" | "l2_focused_repair" => Some(Self::L2FocusedRepair),
            "l3" | "l3_deep_rebuild" => Some(Self::L3DeepRebuild),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::L0Detect => "l0_detect",
            Self::L1LocalTidy => "l1_local_tidy",
            Self::L2FocusedRepair => "l2_focused_repair",
            Self::L3DeepRebuild => "l3_deep_rebuild",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceBudget {
    pub tokens: u64,
    pub wall_minutes: u64,
    pub max_source_refs: u64,
    pub max_files: u64,
    pub subagents: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceContract {
    pub schema_version: String,
    pub id: String,
    pub status: String,
    pub level: MaintenanceLevel,
    pub created_at: String,
    pub scope: MemoryScope,
    pub source_refs: Vec<SourceRef>,
    pub reason: String,
    pub budget: MaintenanceBudget,
    pub allowed_reads: Vec<String>,
    pub allowed_outputs: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub proof_links: Vec<String>,
    pub run_links: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemorySuggestion {
    pub schema_version: String,
    pub id: String,
    pub status: SuggestionStatus,
    pub source_refs: Vec<SourceRef>,
    pub signal_type: SignalType,
    pub summary: String,
    pub scope: MemoryScope,
    pub target_surface: TargetSurface,
    pub dedupe_key: String,
    pub lesson_fingerprint: String,
    pub signal_count: u64,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub dismissed_at: Option<String>,
    pub dismissed_by: Option<String>,
    pub dismissal_reason: Option<String>,
    pub suppressed_until: Option<String>,
    pub created_memory_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    Open,
    Dismissed,
    Created,
    Expired,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScorerContract {
    #[serde(rename = "type")]
    pub scorer_type: ScorerType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScorerType {
    Test,
    Proof,
    Qa,
    Benchmark,
    Schema,
    Replay,
}

impl ScorerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Proof => "proof",
            Self::Qa => "qa",
            Self::Benchmark => "benchmark",
            Self::Schema => "schema",
            Self::Replay => "replay",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScorerReceipt {
    pub schema_version: String,
    pub id: String,
    pub memory_id: String,
    pub scorer_type: ScorerType,
    pub contract_hash: String,
    pub status: ReceiptStatus,
    pub exit_code: Option<i32>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u128,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetRegistry {
    pub schema_version: String,
    pub surfaces: BTreeMap<String, TargetContract>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetContract {
    pub target_path: String,
    pub required_gate: GateRequired,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub allowed_writes: Vec<String>,
    #[serde(default)]
    pub allowed_scorer_types: Vec<ScorerType>,
    #[serde(default)]
    pub rollback_path: Option<String>,
    #[serde(default)]
    pub stale_conditions: Vec<String>,
    #[serde(default)]
    pub forbidden: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionPlan {
    pub schema_version: String,
    pub id: String,
    pub status: PromotionStatus,
    pub memory_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<String>,
    pub target: PromotionTarget,
    pub gates: PromotionGates,
    pub expected_snapshots: PromotionSnapshots,
    pub writes: Vec<PromotionWrite>,
    pub rollback: PromotionRollback,
    pub forbidden_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionStatus {
    Pending,
    Applied,
    ApplyFailed,
    Aborted,
}

impl PromotionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::ApplyFailed => "apply_failed",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionTarget {
    pub surface: TargetSurface,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionGates {
    pub required_gate: GateRequired,
    pub registry_hash: String,
    pub target_contract: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scorer_receipt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_evidence: Option<String>,
    pub review_only: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionSnapshots {
    pub memory_candidate_hash: String,
    pub card_hash: String,
    pub target_hash: String,
    pub health_ledger_hash: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionWrite {
    pub kind: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionRollback {
    pub backup_path: String,
    pub supersession_path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Passed,
    Failed,
}

impl ReceiptStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

impl SuggestionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Dismissed => "dismissed",
            Self::Created => "created",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug)]
struct QueueSnapshot {
    raw: Option<String>,
    rows: Vec<MemorySuggestion>,
}

pub fn suggestions_path(paths: &MaestroPaths) -> PathBuf {
    paths.maestro_dir().join("memory").join("suggestions.jsonl")
}

pub fn target_registry_path(paths: &MaestroPaths) -> PathBuf {
    paths
        .maestro_dir()
        .join("memory")
        .join("target-registry.yml")
}

pub fn promotion_plan_path(paths: &MaestroPaths, promotion_id: &str) -> PathBuf {
    paths
        .maestro_dir()
        .join("memory")
        .join("promotions")
        .join(promotion_id)
        .join("plan.yml")
}

pub fn maintenance_contract_path(paths: &MaestroPaths, maintenance_id: &str) -> PathBuf {
    paths
        .maestro_dir()
        .join("memory")
        .join("maintenance")
        .join(maintenance_id)
        .join("contract.yml")
}

pub fn parse_source_ref(raw: &str) -> SourceRef {
    if let Some((kind, value)) = raw.split_once(":path=") {
        return SourceRef {
            kind: kind.to_string(),
            id: None,
            path: Some(value.to_string()),
        };
    }
    if let Some((kind, value)) = raw.split_once(':') {
        return SourceRef {
            kind: kind.to_string(),
            id: Some(value.to_string()),
            path: None,
        };
    }
    SourceRef {
        kind: "manual".to_string(),
        id: Some(raw.to_string()),
        path: None,
    }
}

pub fn create_maintenance_contract(
    paths: &MaestroPaths,
    request: MaintenanceRequest,
    now: &str,
) -> Result<MaintenanceOutcome> {
    validate_maintenance_request(&request)?;
    let id = next_maintenance_id(&request, now);
    append_maintenance_signal(paths, &id, &request, now)?;

    if request.level == MaintenanceLevel::L0Detect {
        append_maintenance_state(paths, &id, &request, None, now)?;
        return Ok(MaintenanceOutcome {
            id,
            contract_path: None,
            level: request.level,
        });
    }

    let contract_path = maintenance_contract_path(paths, &id);
    let contract = MaintenanceContract {
        schema_version: MAINTENANCE_CONTRACT_SCHEMA_VERSION.to_string(),
        id: id.clone(),
        status: "proposed".to_string(),
        level: request.level,
        created_at: now.to_string(),
        scope: request.scope.clone(),
        source_refs: request.source_refs.clone(),
        reason: request.reason.trim().to_string(),
        budget: maintenance_budget(&request)?,
        allowed_reads: maintenance_allowed_reads(),
        allowed_outputs: maintenance_allowed_outputs(request.level),
        forbidden_actions: maintenance_forbidden_actions(),
        proof_links: request.proof_links.clone(),
        run_links: request.run_links.clone(),
    };
    let raw =
        serde_yaml::to_string(&contract).context("failed to serialize maintenance contract")?;
    write_string_atomic(&contract_path, &raw)?;
    append_maintenance_state(paths, &id, &request, Some(&contract_path), now)?;

    Ok(MaintenanceOutcome {
        id,
        contract_path: Some(contract_path),
        level: request.level,
    })
}

pub fn create_suggestion(
    paths: &MaestroPaths,
    request: CreateSuggestionRequest,
    now: &str,
) -> Result<CreateSuggestionOutcome> {
    validate_suggestion_request(&request)?;
    let mut snapshot = load_queue(paths)?;
    let dedupe_key = request
        .dedupe_key
        .clone()
        .unwrap_or_else(|| default_dedupe_key(&request));

    if let Some(existing) = snapshot
        .rows
        .iter_mut()
        .find(|row| row.status == SuggestionStatus::Open && row.dedupe_key == dedupe_key)
    {
        merge_source_refs(&mut existing.source_refs, &request.source_refs);
        existing.signal_count += 1;
        existing.updated_at = now.to_string();
        existing.expires_at = request.expires_at.clone().or(existing.expires_at.clone());
        let suggestion = existing.clone();
        save_queue(paths, &snapshot)?;
        return Ok(CreateSuggestionOutcome {
            suggestion,
            created: false,
        });
    }

    let id = next_suggestion_id(&snapshot.rows, &dedupe_key, now);
    let suggestion = MemorySuggestion {
        schema_version: SUGGESTION_SCHEMA_VERSION.to_string(),
        id,
        status: SuggestionStatus::Open,
        source_refs: request.source_refs,
        signal_type: request.signal_type,
        summary: request.summary.trim().to_string(),
        scope: request.scope,
        target_surface: request.target_surface,
        dedupe_key: dedupe_key.clone(),
        lesson_fingerprint: lesson_fingerprint(&dedupe_key),
        signal_count: 1,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        expires_at: request.expires_at,
        dismissed_at: None,
        dismissed_by: None,
        dismissal_reason: None,
        suppressed_until: None,
        created_memory_id: None,
    };
    snapshot.rows.push(suggestion.clone());
    save_queue(paths, &snapshot)?;
    Ok(CreateSuggestionOutcome {
        suggestion,
        created: true,
    })
}

pub fn list_suggestions(paths: &MaestroPaths, include_all: bool) -> Result<Vec<MemorySuggestion>> {
    let rows = load_queue(paths)?.rows;
    Ok(rows
        .into_iter()
        .filter(|row| include_all || row.status == SuggestionStatus::Open)
        .collect())
}

pub fn suggestion_hints(
    paths: &MaestroPaths,
    surface: MemoryReadSurface,
    scope: MemoryReadScope,
) -> Result<MemorySuggestionSet> {
    let now = utc_now_timestamp();
    let mut rows = load_queue(paths)?
        .rows
        .into_iter()
        .filter(|row| row.status == SuggestionStatus::Open)
        .filter(|row| !suggestion_is_expired(row, &now))
        .filter(|row| memory_scope_matches(&row.scope, &scope))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        scope_rank(&left.scope, &scope)
            .cmp(&scope_rank(&right.scope, &scope))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    let cap = surface.cap();
    let omitted = rows.len().saturating_sub(cap);
    let suggestions = rows
        .into_iter()
        .take(cap)
        .enumerate()
        .map(|(index, row)| MemorySuggestionHint {
            rank: index + 1,
            id: row.id.clone(),
            summary: row.summary,
            source_count: row.source_refs.len(),
            signal_type: row.signal_type.as_str().to_string(),
            target_surface: row.target_surface.as_str().to_string(),
            scope_kind: row.scope.kind.as_str().to_string(),
            create_command: format!("maestro memory create --from {}", row.id),
            dismiss_command: format!(
                "maestro memory suggest dismiss {} --reason \"<why>\"",
                row.id
            ),
        })
        .collect();
    Ok(MemorySuggestionSet {
        suggestions,
        omitted,
    })
}

pub fn dismiss_suggestion(
    paths: &MaestroPaths,
    id: &str,
    reason: &str,
    actor: &str,
    now: &str,
) -> Result<DismissSuggestionOutcome> {
    if reason.trim().is_empty() {
        bail!("dismissal reason cannot be empty");
    }
    let mut snapshot = load_queue(paths)?;
    let Some(row) = snapshot.rows.iter_mut().find(|row| row.id == id) else {
        bail!("memory suggestion {id} not found");
    };
    if row.status != SuggestionStatus::Open {
        bail!(
            "memory suggestion {id} is {}, not open",
            row.status.as_str()
        );
    }
    row.status = SuggestionStatus::Dismissed;
    row.updated_at = now.to_string();
    row.dismissed_at = Some(now.to_string());
    row.dismissed_by = Some(actor.to_string());
    row.dismissal_reason = Some(reason.trim().to_string());
    let suggestion = row.clone();
    save_queue(paths, &snapshot)?;
    Ok(DismissSuggestionOutcome { suggestion })
}

pub fn create_memory(
    paths: &MaestroPaths,
    request: CreateMemoryRequest,
    now: &str,
) -> Result<CreateMemoryOutcome> {
    let (seed, mut snapshot) = resolve_create_seed(paths, &request, now)?;
    let summary = request
        .summary
        .as_deref()
        .unwrap_or(&seed.summary)
        .trim()
        .to_string();
    if summary.is_empty() {
        bail!("memory create needs --summary when --from is not a suggestion id");
    }
    let lesson = request
        .lesson
        .as_deref()
        .unwrap_or(&summary)
        .trim()
        .to_string();
    let id = store::mint_card_id(paths, CardType::Memory, &summary);
    let mut card = Card::new(&id, CardType::Memory, &summary, "proposed", now);
    card.description = Some(summary.clone());
    store::create_card(paths, &card)?;

    write_memory_sidecars(paths, &card, &seed, &summary, &lesson, now)?;
    let loaded = validate_card(paths, &card)?;
    if loaded.id != id {
        bail!(
            "created memory sidecar loaded with mismatched id {}",
            loaded.id
        );
    }

    if let Some(suggestion_id) = seed.suggestion_id.as_deref() {
        let Some(snapshot) = snapshot.as_mut() else {
            bail!("invariant: suggestion create seed has no queue snapshot");
        };
        let Some(row) = snapshot.rows.iter_mut().find(|row| row.id == suggestion_id) else {
            bail!("memory suggestion {suggestion_id} disappeared before create link");
        };
        if row.status != SuggestionStatus::Open {
            bail!(
                "memory suggestion {suggestion_id} is {}, not open",
                row.status.as_str()
            );
        }
        row.status = SuggestionStatus::Created;
        row.updated_at = now.to_string();
        row.created_memory_id = Some(id.clone());
        save_queue(paths, snapshot)?;
    }

    Ok(CreateMemoryOutcome {
        id,
        from_suggestion: seed.suggestion_id,
    })
}

pub fn attach_scorer_contract(
    paths: &MaestroPaths,
    memory_id: &str,
    contract_raw: &str,
) -> Result<AttachScorerOutcome> {
    let resolved = store::resolve(paths, memory_id)?
        .map(|resolved| resolved.card)
        .ok_or_else(|| anyhow::anyhow!("memory card {memory_id} not found"))?;
    let path = candidate_path(paths, memory_id);
    let raw = read_to_string_if_exists(&path)?
        .ok_or_else(|| anyhow::anyhow!("{} not found", path.display()))?;
    let mut candidate = crate::domain::memory::parse_candidate(&raw, &path.display().to_string())?;
    validate_candidate_for_card(&resolved, &candidate)?;
    let contract_value: serde_yaml::Value = serde_yaml::from_str(contract_raw)
        .with_context(|| format!("failed to parse scorer contract for {memory_id}"))?;
    let contract = parse_scorer_contract(&contract_value)?;
    candidate.memory.gate.scorer_contract = Some(contract_value);
    let updated =
        serde_yaml::to_string(&candidate).context("failed to serialize memory candidate")?;
    write_string_if_unchanged(path, Some(&raw), &updated)?;
    Ok(AttachScorerOutcome {
        id: memory_id.to_string(),
        scorer_type: contract.scorer_type,
    })
}

pub fn run_scorer(paths: &MaestroPaths, contract_ref: &str, now: &str) -> Result<ScorerRunOutcome> {
    let (memory_id, selector) = contract_ref.split_once('#').ok_or_else(|| {
        anyhow::anyhow!("scorer ref must look like <memory-id>#gate.scorer_contract")
    })?;
    if selector != "gate.scorer_contract" {
        bail!("unsupported scorer selector {selector:?}; expected gate.scorer_contract");
    }
    let card = store::resolve(paths, memory_id)?
        .map(|resolved| resolved.card)
        .ok_or_else(|| anyhow::anyhow!("memory card {memory_id} not found"))?;
    let candidate = validate_card(paths, &card)?;
    let Some(contract_value) = candidate.memory.gate.scorer_contract.as_ref() else {
        bail!("memory candidate {memory_id} has no gate.scorer_contract");
    };
    let contract = parse_scorer_contract(contract_value)?;
    let contract_yaml =
        serde_yaml::to_string(contract_value).context("failed to serialize scorer contract")?;
    let contract_hash = format!("sha256:{}", sha256_hex(contract_yaml.as_bytes()));
    let started = Instant::now();
    let execution = execute_scorer(paths, memory_id, &contract);
    let duration_ms = started.elapsed().as_millis();
    let receipt_id = format!(
        "rcpt-{}",
        &sha256_hex(format!("{memory_id}\n{contract_hash}\n{now}").as_bytes())[..12]
    );
    let receipt = ScorerReceipt {
        schema_version: SCORER_RECEIPT_SCHEMA_VERSION.to_string(),
        id: receipt_id,
        memory_id: memory_id.to_string(),
        scorer_type: contract.scorer_type,
        contract_hash,
        status: if execution.passed {
            ReceiptStatus::Passed
        } else {
            ReceiptStatus::Failed
        },
        exit_code: execution.exit_code,
        started_at: now.to_string(),
        finished_at: now.to_string(),
        duration_ms,
        stdout: bounded_output(execution.stdout),
        stderr: bounded_output(execution.stderr),
        error: execution.error,
    };
    let path = receipt_path(paths, memory_id, &receipt.id);
    ensure_dir(path.parent().expect("receipt path has a parent"))?;
    let json = serde_json::to_string_pretty(&receipt).context("failed to serialize receipt")?;
    write_string_atomic(&path, &(json + "\n"))?;
    Ok(ScorerRunOutcome { receipt, path })
}

pub fn list_scorer_receipts(
    paths: &MaestroPaths,
    memory_id: &str,
) -> Result<Vec<(ScorerReceipt, PathBuf)>> {
    let dir = crate::domain::memory::memory_dir(paths, memory_id).join(RECEIPTS_DIR);
    let mut receipts = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(receipts);
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read receipt {}", path.display()))?;
        let receipt: ScorerReceipt = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse receipt {}", path.display()))?;
        receipts.push((receipt, path));
    }
    receipts.sort_by(|left, right| left.0.id.cmp(&right.0.id));
    Ok(receipts)
}

pub fn show_scorer_receipt(paths: &MaestroPaths, receipt_ref: &str) -> Result<ScorerReceipt> {
    let path = if let Some((memory_id, receipt_id)) = receipt_ref.split_once('#') {
        receipt_path(paths, memory_id, receipt_id)
    } else {
        PathBuf::from(receipt_ref)
    };
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read scorer receipt {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse scorer receipt {}", path.display()))
}

pub fn plan_promotion(
    paths: &MaestroPaths,
    request: PlanPromotionRequest,
    now: &str,
) -> Result<PlanPromotionOutcome> {
    let (resolved, candidate_raw, candidate) =
        load_memory_candidate_raw(paths, &request.memory_id)?;
    let (_registry_raw, registry_hash, registry) = load_target_registry(paths)?;
    let contract =
        target_contract(&registry, candidate.memory.target_surface).ok_or_else(|| {
            anyhow::anyhow!(
                "target registry {} has no contract for {}",
                target_registry_path(paths).display(),
                candidate.memory.target_surface.as_str()
            )
        })?;
    validate_target_contract(candidate.memory.target_surface, contract)?;
    validate_candidate_target_allowed(&candidate)?;

    let required_gate = effective_gate(
        candidate.memory.gate.required,
        contract.required_gate,
        candidate.memory.gate.review_required || contract.review_required,
    );
    let receipt_ref = resolve_required_receipt(
        paths,
        &candidate,
        contract,
        required_gate,
        request.scorer_receipt.as_deref(),
    )?;
    let review_evidence = request
        .review_evidence
        .filter(|value| !value.trim().is_empty());
    if gate_needs_review(required_gate) && review_evidence.is_none() {
        bail!(
            "memory promotion for {} requires review evidence; pass --review-evidence",
            request.memory_id
        );
    }

    let target_path = render_target_path(&contract.target_path, &request.memory_id)?;
    let target_abs = resolve_repo_relative(paths, &target_path)?;
    let target_raw = read_to_string_if_exists(&target_abs)
        .with_context(|| format!("failed to read {}", target_abs.display()))?;
    let health_raw = read_to_string_if_exists(health_ledger_path(paths))?;
    let card_raw = fs::read_to_string(resolved.path())
        .with_context(|| format!("failed to read {}", resolved.path().display()))?;
    let id = next_promotion_id(
        &request.memory_id,
        candidate.memory.target_surface,
        &target_path,
        now,
    );
    let backup_path = format!(
        ".maestro/memory/promotions/{id}/backup/{}",
        target_path.replace('/', "__")
    );
    let plan = PromotionPlan {
        schema_version: PROMOTION_PLAN_SCHEMA_VERSION.to_string(),
        id: id.clone(),
        status: PromotionStatus::Pending,
        memory_id: request.memory_id,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        applied_at: None,
        target: PromotionTarget {
            surface: candidate.memory.target_surface,
            path: target_path.clone(),
        },
        gates: PromotionGates {
            required_gate,
            registry_hash,
            target_contract: candidate.memory.target_surface.as_str().to_string(),
            scorer_receipt: receipt_ref,
            review_evidence,
            review_only: !gate_needs_scorer(required_gate),
        },
        expected_snapshots: PromotionSnapshots {
            memory_candidate_hash: snapshot_hash(Some(&candidate_raw)),
            card_hash: snapshot_hash(Some(&card_raw)),
            target_hash: snapshot_hash(target_raw.as_deref()),
            health_ledger_hash: snapshot_hash(health_raw.as_deref()),
        },
        writes: vec![
            PromotionWrite {
                kind: "target".to_string(),
                path: target_path,
            },
            PromotionWrite {
                kind: "memory_candidate".to_string(),
                path: format!(".maestro/cards/{}/memory/candidate.yml", candidate.id),
            },
            PromotionWrite {
                kind: "card_status".to_string(),
                path: format!(".maestro/cards/{}/card.yaml", candidate.id),
            },
            PromotionWrite {
                kind: "health_ledger".to_string(),
                path: ".maestro/memory/health-ledger.jsonl".to_string(),
            },
        ],
        rollback: PromotionRollback {
            backup_path,
            supersession_path: "maestro memory stale|supersede".to_string(),
        },
        forbidden_actions: forbidden_promotion_actions(),
        error: None,
    };
    let path = promotion_plan_path(paths, &id);
    ensure_dir(path.parent().expect("promotion plan path has a parent"))?;
    let plan_yaml = serde_yaml::to_string(&plan).context("failed to serialize promotion plan")?;
    write_string_if_unchanged(&path, None, &plan_yaml)?;
    Ok(PlanPromotionOutcome {
        id,
        path,
        review_only: plan.gates.review_only,
    })
}

pub fn apply_promotion(
    paths: &MaestroPaths,
    request: ApplyPromotionRequest,
    now: &str,
) -> Result<ApplyPromotionOutcome> {
    match apply_promotion_inner(paths, &request.promotion_id, now) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            let _ = mark_plan_failed(paths, &request.promotion_id, now, &error.to_string());
            Err(error)
        }
    }
}

pub fn approved_memory(
    paths: &MaestroPaths,
    surface: MemoryReadSurface,
    scope: MemoryReadScope,
) -> Result<ApprovedMemorySet> {
    let health = latest_health_states(paths)?;
    let mut rows = Vec::new();
    for card in crate::domain::card::query::scan(paths)? {
        if card.card_type != CardType::Memory {
            continue;
        }
        let Ok(candidate) = validate_card(paths, &card) else {
            continue;
        };
        if candidate.memory.lifecycle != MemoryLifecycle::Promoted {
            continue;
        }
        if !matches!(card.status.as_str(), "verified" | "closed") {
            continue;
        }
        if health.get(&card.id).map(String::as_str) != Some("healthy") {
            continue;
        }
        if !memory_scope_matches(&candidate.memory.scope, &scope) {
            continue;
        }
        let lesson_path = crate::domain::memory::memory_dir(paths, &card.id).join(LESSON_FILE);
        let lesson = fs::read_to_string(&lesson_path).unwrap_or_default();
        if let Some(query) = scope.query.as_deref()
            && !memory_matches_query(&card, &candidate, &lesson, query)
        {
            continue;
        }
        let rank_key = memory_rank_key(&candidate, &scope);
        rows.push((rank_key, card, candidate, lesson));
    }
    rows.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    let cap = surface.cap();
    let omitted = rows.len().saturating_sub(cap);
    let memories = rows
        .into_iter()
        .take(cap)
        .enumerate()
        .map(
            |(index, (_rank_key, card, candidate, lesson))| ApprovedMemory {
                rank: index + 1,
                id: card.id.clone(),
                title: card.title.clone(),
                summary: compact_lesson_summary(&lesson, &card),
                target_surface: candidate.memory.target_surface,
                scope_kind: candidate.memory.scope.kind,
                scope_refs: candidate.memory.scope.refs.clone(),
                risk: candidate.memory.risk.overall,
                confidence: candidate.memory.signal_summary.confidence,
                lesson_path: display_memory_lesson_path(&card.id),
                show_command: format!("maestro memory show {}", card.id),
                reason: memory_rank_reason(&candidate, &scope),
            },
        )
        .collect();
    Ok(ApprovedMemorySet { memories, omitted })
}

fn apply_promotion_inner(
    paths: &MaestroPaths,
    promotion_id: &str,
    now: &str,
) -> Result<ApplyPromotionOutcome> {
    let plan_path = promotion_plan_path(paths, promotion_id);
    let plan_raw = fs::read_to_string(&plan_path)
        .with_context(|| format!("failed to read promotion plan {}", plan_path.display()))?;
    let mut plan: PromotionPlan = serde_yaml::from_str(&plan_raw)
        .with_context(|| format!("failed to parse promotion plan {}", plan_path.display()))?;
    if plan.schema_version != PROMOTION_PLAN_SCHEMA_VERSION {
        bail!(
            "promotion plan {} has schema_version {}, expected {PROMOTION_PLAN_SCHEMA_VERSION}",
            plan.id,
            plan.schema_version
        );
    }
    if plan.status != PromotionStatus::Pending {
        bail!(
            "promotion plan {} is {}, not pending",
            plan.id,
            plan.status.as_str()
        );
    }
    if plan.id != promotion_id {
        bail!(
            "promotion plan id {} does not match requested {}",
            plan.id,
            promotion_id
        );
    }

    let (_registry_raw, registry_hash, registry) = load_target_registry(paths)?;
    if registry_hash != plan.gates.registry_hash {
        bail!(
            "target registry changed since plan {}; regenerate the promotion plan",
            plan.id
        );
    }
    let contract = target_contract(&registry, plan.target.surface).ok_or_else(|| {
        anyhow::anyhow!(
            "target registry {} has no contract for {}",
            target_registry_path(paths).display(),
            plan.target.surface.as_str()
        )
    })?;
    validate_target_contract(plan.target.surface, contract)?;

    let (resolved, candidate_raw, mut candidate) =
        load_memory_candidate_raw(paths, &plan.memory_id)?;
    if snapshot_hash(Some(&candidate_raw)) != plan.expected_snapshots.memory_candidate_hash {
        bail!(
            "memory candidate {} changed since promotion plan {}; regenerate the plan",
            plan.memory_id,
            plan.id
        );
    }
    if candidate.memory.target_surface != plan.target.surface {
        bail!(
            "promotion plan {} targets {}, but Memory {} targets {}",
            plan.id,
            plan.target.surface.as_str(),
            candidate.id,
            candidate.memory.target_surface.as_str()
        );
    }
    validate_candidate_target_allowed(&candidate)?;
    validate_plan_gate(paths, &candidate, contract, &plan)?;

    let current_card_raw = fs::read_to_string(resolved.path())
        .with_context(|| format!("failed to read {}", resolved.path().display()))?;
    if snapshot_hash(Some(&current_card_raw)) != plan.expected_snapshots.card_hash {
        bail!(
            "card {} changed since promotion plan {}; regenerate the plan",
            plan.memory_id,
            plan.id
        );
    }
    let target_abs = resolve_repo_relative(paths, &plan.target.path)?;
    let target_raw = read_to_string_if_exists(&target_abs)
        .with_context(|| format!("failed to read {}", target_abs.display()))?;
    if snapshot_hash(target_raw.as_deref()) != plan.expected_snapshots.target_hash {
        bail!(
            "target {} changed since promotion plan {}; regenerate the plan",
            plan.target.path,
            plan.id
        );
    }
    let health_raw = read_to_string_if_exists(health_ledger_path(paths))?;
    if snapshot_hash(health_raw.as_deref()) != plan.expected_snapshots.health_ledger_hash {
        bail!(
            "memory health ledger changed since promotion plan {}; regenerate the plan",
            plan.id
        );
    }

    let backup_path = resolve_repo_relative(paths, &plan.rollback.backup_path)?;
    if let Some(raw) = target_raw.as_deref() {
        ensure_dir(backup_path.parent().expect("backup path has a parent"))?;
        write_string_if_unchanged(&backup_path, None, raw)?;
    }
    let lesson_path = crate::domain::memory::memory_dir(paths, &candidate.id).join(LESSON_FILE);
    let lesson = fs::read_to_string(&lesson_path)
        .with_context(|| format!("failed to read {}", lesson_path.display()))?;
    let target_contents = promoted_target_contents(&candidate, &plan, &lesson, now);
    ensure_dir(target_abs.parent().expect("target path has a parent"))?;
    write_string_if_unchanged(&target_abs, target_raw.as_deref(), &target_contents)?;

    let mut card = resolved.card.clone();
    card.status = "verified".to_string();
    card.updated_at = now.to_string();
    card.claimed_by = None;
    card.claimed_at = None;
    store::save_resolved(&card, &resolved)?;

    candidate.memory.lifecycle = MemoryLifecycle::Promoted;
    candidate.memory.risk.registry_hash = Some(plan.gates.registry_hash.clone());
    candidate.memory.gate.rollback_path = Some(plan.rollback.backup_path.clone());
    candidate
        .memory
        .rollback
        .backup_refs
        .push(plan.rollback.backup_path.clone());
    validate_candidate_for_card(&card, &candidate)?;
    let updated_candidate =
        serde_yaml::to_string(&candidate).context("failed to serialize promoted Memory")?;
    write_string_if_unchanged(
        candidate_path(paths, &candidate.id),
        Some(&candidate_raw),
        &updated_candidate,
    )?;

    append_health_ledger(
        paths,
        &candidate.id,
        &plan.id,
        "healthy",
        "promotion_applied",
        now,
    )?;

    plan.status = PromotionStatus::Applied;
    plan.updated_at = now.to_string();
    plan.applied_at = Some(now.to_string());
    let applied_yaml =
        serde_yaml::to_string(&plan).context("failed to serialize promotion plan")?;
    write_string_if_unchanged(&plan_path, Some(&plan_raw), &applied_yaml)?;

    Ok(ApplyPromotionOutcome {
        id: plan.id,
        target_path: target_abs,
        backup_path: if target_raw.is_some() {
            Some(backup_path)
        } else {
            None
        },
    })
}

fn mark_plan_failed(
    paths: &MaestroPaths,
    promotion_id: &str,
    now: &str,
    message: &str,
) -> Result<()> {
    let path = promotion_plan_path(paths, promotion_id);
    let raw = read_to_string_if_exists(&path)?;
    let Some(raw) = raw else {
        return Ok(());
    };
    let mut plan: PromotionPlan = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse promotion plan {}", path.display()))?;
    if plan.status == PromotionStatus::Pending {
        plan.status = PromotionStatus::ApplyFailed;
        plan.updated_at = now.to_string();
        plan.error = Some(message.to_string());
        let updated =
            serde_yaml::to_string(&plan).context("failed to serialize failed promotion plan")?;
        write_string_if_unchanged(&path, Some(&raw), &updated)?;
    }
    Ok(())
}

fn load_memory_candidate_raw(
    paths: &MaestroPaths,
    memory_id: &str,
) -> Result<(store::ResolvedCard, String, MemoryCandidate)> {
    let resolved = store::resolve(paths, memory_id)?
        .ok_or_else(|| anyhow::anyhow!("memory card {memory_id} not found"))?;
    let path = candidate_path(paths, memory_id);
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let candidate = crate::domain::memory::parse_candidate(&raw, &path.display().to_string())?;
    validate_candidate_for_card(&resolved.card, &candidate)?;
    Ok((resolved, raw, candidate))
}

fn load_target_registry(paths: &MaestroPaths) -> Result<(String, String, TargetRegistry)> {
    let path = target_registry_path(paths);
    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "memory promotion requires repo-local target registry {}",
            path.display()
        )
    })?;
    let registry: TargetRegistry = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if registry.schema_version != TARGET_REGISTRY_SCHEMA_VERSION {
        bail!(
            "{} has schema_version {}, expected {TARGET_REGISTRY_SCHEMA_VERSION}",
            path.display(),
            registry.schema_version
        );
    }
    if registry.surfaces.is_empty() {
        bail!("{} has no surfaces", path.display());
    }
    for (surface, contract) in &registry.surfaces {
        let parsed = TargetSurface::parse(surface).ok_or_else(|| {
            anyhow::anyhow!(
                "{} has unknown Memory target surface {surface:?}",
                path.display()
            )
        })?;
        validate_target_contract(parsed, contract)?;
    }
    let registry_hash = format!("sha256:{}", sha256_hex(raw.as_bytes()));
    Ok((raw, registry_hash, registry))
}

fn target_contract(registry: &TargetRegistry, surface: TargetSurface) -> Option<&TargetContract> {
    registry.surfaces.get(surface.as_str())
}

fn validate_target_contract(surface: TargetSurface, contract: &TargetContract) -> Result<()> {
    if surface == TargetSurface::ExternalAction || contract.forbidden {
        bail!("memory target surface {} is forbidden", surface.as_str());
    }
    if contract.target_path.trim().is_empty() {
        bail!(
            "memory target surface {} has empty target_path",
            surface.as_str()
        );
    }
    validate_relative_path(&contract.target_path)?;
    if !contract.allowed_writes.iter().any(|kind| kind == "replace") {
        bail!(
            "memory target surface {} must allow replace writes for promotion apply",
            surface.as_str()
        );
    }
    if gate_needs_scorer(contract.required_gate) && contract.allowed_scorer_types.is_empty() {
        bail!(
            "memory target surface {} requires scorer but declares no allowed_scorer_types",
            surface.as_str()
        );
    }
    if contract.required_gate == GateRequired::Forbidden {
        bail!(
            "memory target surface {} has forbidden gate",
            surface.as_str()
        );
    }
    if let Some(path) = contract.rollback_path.as_deref() {
        validate_relative_path(path)?;
    }
    Ok(())
}

fn validate_candidate_target_allowed(candidate: &MemoryCandidate) -> Result<()> {
    if !candidate
        .memory
        .gate
        .allowed_targets
        .contains(&candidate.memory.target_surface)
    {
        bail!(
            "memory candidate {} gate does not allow target {}",
            candidate.id,
            candidate.memory.target_surface.as_str()
        );
    }
    Ok(())
}

fn effective_gate(
    candidate_gate: GateRequired,
    contract_gate: GateRequired,
    review_required: bool,
) -> GateRequired {
    if candidate_gate == GateRequired::Forbidden || contract_gate == GateRequired::Forbidden {
        return GateRequired::Forbidden;
    }
    let needs_scorer = gate_needs_scorer(candidate_gate) || gate_needs_scorer(contract_gate);
    let needs_review =
        review_required || gate_needs_review(candidate_gate) || gate_needs_review(contract_gate);
    match (needs_scorer, needs_review) {
        (true, true) => GateRequired::ScorerAndReview,
        (true, false) => GateRequired::Scorer,
        (false, _) => GateRequired::Review,
    }
}

fn gate_needs_scorer(gate: GateRequired) -> bool {
    matches!(gate, GateRequired::Scorer | GateRequired::ScorerAndReview)
}

fn gate_needs_review(gate: GateRequired) -> bool {
    matches!(gate, GateRequired::Review | GateRequired::ScorerAndReview)
}

fn resolve_required_receipt(
    paths: &MaestroPaths,
    candidate: &MemoryCandidate,
    contract: &TargetContract,
    required_gate: GateRequired,
    requested: Option<&str>,
) -> Result<Option<String>> {
    if !gate_needs_scorer(required_gate) {
        return Ok(None);
    }
    let Some(contract_value) = candidate.memory.gate.scorer_contract.as_ref() else {
        bail!(
            "memory promotion for {} requires scorer evidence but candidate has no gate.scorer_contract",
            candidate.id
        );
    };
    let scorer_contract = parse_scorer_contract(contract_value)?;
    let contract_yaml =
        serde_yaml::to_string(contract_value).context("failed to serialize scorer contract")?;
    let expected_hash = format!("sha256:{}", sha256_hex(contract_yaml.as_bytes()));
    let (receipt, canonical) = if let Some(requested) = requested {
        let receipt = show_scorer_receipt(paths, requested)?;
        let canonical = format!("{}#{}", receipt.memory_id, receipt.id);
        (receipt, canonical)
    } else {
        let latest = list_scorer_receipts(paths, &candidate.id)?
            .into_iter()
            .rfind(|(receipt, _)| receipt.status == ReceiptStatus::Passed)
            .map(|(receipt, _)| receipt);
        let Some(receipt) = latest else {
            bail!(
                "memory promotion for {} requires a passed scorer receipt",
                candidate.id
            );
        };
        let canonical = format!("{}#{}", receipt.memory_id, receipt.id);
        (receipt, canonical)
    };
    if receipt.memory_id != candidate.id {
        bail!(
            "scorer receipt {} belongs to {}, not {}",
            receipt.id,
            receipt.memory_id,
            candidate.id
        );
    }
    if receipt.status != ReceiptStatus::Passed {
        bail!(
            "scorer receipt {} is {}, not passed",
            receipt.id,
            receipt.status.as_str()
        );
    }
    if receipt.contract_hash != expected_hash {
        bail!(
            "scorer receipt {} does not match the current scorer contract for {}",
            receipt.id,
            candidate.id
        );
    }
    if receipt.scorer_type != scorer_contract.scorer_type {
        bail!(
            "scorer receipt {} type {} does not match contract type {}",
            receipt.id,
            receipt.scorer_type.as_str(),
            scorer_contract.scorer_type.as_str()
        );
    }
    if !contract.allowed_scorer_types.is_empty()
        && !contract.allowed_scorer_types.contains(&receipt.scorer_type)
    {
        bail!(
            "scorer receipt {} type {} is not allowed by target contract",
            receipt.id,
            receipt.scorer_type.as_str()
        );
    }
    Ok(Some(canonical))
}

fn validate_plan_gate(
    paths: &MaestroPaths,
    candidate: &MemoryCandidate,
    contract: &TargetContract,
    plan: &PromotionPlan,
) -> Result<()> {
    let required_gate = effective_gate(
        candidate.memory.gate.required,
        contract.required_gate,
        candidate.memory.gate.review_required || contract.review_required,
    );
    if required_gate != plan.gates.required_gate {
        bail!(
            "promotion plan {} gate changed from {} to {}; regenerate the plan",
            plan.id,
            plan.gates.required_gate.as_str(),
            required_gate.as_str()
        );
    }
    if gate_needs_review(required_gate) && plan.gates.review_evidence.is_none() {
        bail!(
            "promotion plan {} requires review evidence before apply",
            plan.id
        );
    }
    let expected_receipt = resolve_required_receipt(
        paths,
        candidate,
        contract,
        required_gate,
        plan.gates.scorer_receipt.as_deref(),
    )?;
    if expected_receipt != plan.gates.scorer_receipt {
        bail!(
            "promotion plan {} scorer receipt changed; regenerate the plan",
            plan.id
        );
    }
    Ok(())
}

fn render_target_path(template: &str, memory_id: &str) -> Result<String> {
    let path = template.replace("{memory_id}", memory_id);
    validate_relative_path(&path)?;
    Ok(path)
}

fn validate_relative_path(path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.is_absolute() {
        bail!(
            "memory target path {} must be repo-relative",
            path.display()
        );
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!(
            "memory target path {} must not escape the repo",
            path.display()
        );
    }
    if path.starts_with(".git") {
        bail!(
            "memory target path {} must not write inside .git",
            path.display()
        );
    }
    Ok(())
}

fn resolve_repo_relative(paths: &MaestroPaths, relative: &str) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    Ok(paths.repo_root().join(relative))
}

fn snapshot_hash(raw: Option<&str>) -> String {
    raw.map(|raw| format!("sha256:{}", sha256_hex(raw.as_bytes())))
        .unwrap_or_else(|| "absent".to_string())
}

fn health_ledger_path(paths: &MaestroPaths) -> PathBuf {
    paths
        .maestro_dir()
        .join("memory")
        .join("health-ledger.jsonl")
}

fn next_promotion_id(
    memory_id: &str,
    surface: TargetSurface,
    target_path: &str,
    now: &str,
) -> String {
    format!(
        "prom-{}",
        &sha256_hex(format!("{memory_id}\n{}\n{target_path}\n{now}", surface.as_str()).as_bytes())
            [..12]
    )
}

fn promoted_target_contents(
    candidate: &MemoryCandidate,
    plan: &PromotionPlan,
    lesson: &str,
    now: &str,
) -> String {
    format!(
        "---\nschema_version: maestro.memory.promoted_artifact.v1\nmemory_id: {}\npromotion_id: {}\ntarget_surface: {}\npromoted_at: {}\n---\n\n{}\n",
        candidate.id,
        plan.id,
        plan.target.surface.as_str(),
        now,
        lesson.trim_end()
    )
}

fn append_health_ledger(
    paths: &MaestroPaths,
    memory_id: &str,
    promotion_id: &str,
    state: &str,
    reason: &str,
    now: &str,
) -> Result<()> {
    let row = json!({
        "schema_version": "maestro.memory.health_ledger.v1",
        "row_type": "state",
        "memory_id": memory_id,
        "promotion_id": promotion_id,
        "state": state,
        "reason": reason,
        "at": now,
    });
    let line = serde_json::to_string(&row).context("failed to serialize health ledger row")? + "\n";
    append_text_file(health_ledger_path(paths), "", &line)?;
    Ok(())
}

fn forbidden_promotion_actions() -> Vec<String> {
    [
        "external_release",
        "archive",
        "publish",
        "tag",
        "secret_rotation",
        "destructive_git",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn validate_maintenance_request(request: &MaintenanceRequest) -> Result<()> {
    if request.reason.trim().is_empty() {
        bail!("memory maintenance reason cannot be empty");
    }
    if request.source_refs.is_empty() {
        bail!("memory maintenance requires at least one --source-ref");
    }
    for source in &request.source_refs {
        if source.id.is_none() && source.path.is_none() {
            bail!(
                "memory maintenance source_ref {} needs id or path",
                source.kind
            );
        }
        if forbidden_source_kind(&source.kind) {
            bail!(
                "memory maintenance uses forbidden source kind {}",
                source.kind
            );
        }
    }
    match request.level {
        MaintenanceLevel::L0Detect => {
            if request.explicit_budget.is_some() {
                bail!("L0 maintenance does not accept an explicit budget");
            }
        }
        MaintenanceLevel::L1LocalTidy | MaintenanceLevel::L2FocusedRepair => {
            if request.explicit_budget.is_some() {
                bail!("explicit maintenance budgets are allowed only for L3");
            }
        }
        MaintenanceLevel::L3DeepRebuild => {
            if !request.human_approved {
                bail!("L3 maintenance requires --human-approved");
            }
            if request.explicit_budget.is_none() {
                bail!("L3 maintenance requires an explicit budget");
            }
        }
    }
    Ok(())
}

fn maintenance_budget(request: &MaintenanceRequest) -> Result<MaintenanceBudget> {
    match request.level {
        MaintenanceLevel::L0Detect => bail!("L0 maintenance does not write a contract"),
        MaintenanceLevel::L1LocalTidy => Ok(MaintenanceBudget {
            tokens: 4_000,
            wall_minutes: 10,
            max_source_refs: 12,
            max_files: 3,
            subagents: 0,
        }),
        MaintenanceLevel::L2FocusedRepair => Ok(MaintenanceBudget {
            tokens: 12_000,
            wall_minutes: 25,
            max_source_refs: 40,
            max_files: 8,
            subagents: 1,
        }),
        MaintenanceLevel::L3DeepRebuild => request
            .explicit_budget
            .clone()
            .context("L3 maintenance requires an explicit budget"),
    }
}

fn maintenance_allowed_reads() -> Vec<String> {
    [
        "memory_cards",
        "linked_run_events",
        "proof",
        "qa",
        "decisions",
        "notes",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn maintenance_allowed_outputs(level: MaintenanceLevel) -> Vec<String> {
    match level {
        MaintenanceLevel::L0Detect => ["health_ledger_signal", "health_ledger_state"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        MaintenanceLevel::L1LocalTidy => [
            "stale_marker",
            "duplicate_link",
            "simple_memory_note_proposal",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        MaintenanceLevel::L2FocusedRepair => [
            "supersession_candidate",
            "scorer_receipt",
            "recurrence_guard_proposal",
            "gated_target_proposal",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        MaintenanceLevel::L3DeepRebuild => ["multi_memory_rebuild_proposal", "harness_proposal"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

fn maintenance_forbidden_actions() -> Vec<String> {
    [
        "direct_harness_mutation_without_gate",
        "external_ship",
        "external_release",
        "external_archive",
        "hidden_cache",
        "hidden_memory",
        "private_planner_state",
        "silent_skill_mutation",
        "silent_hook_mutation",
        "silent_cli_mutation",
        "destructive_git",
        "secret_rotation",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn next_maintenance_id(request: &MaintenanceRequest, now: &str) -> String {
    let source_key = request
        .source_refs
        .iter()
        .map(|source| {
            format!(
                "{}:{}:{}",
                source.kind,
                source.id.as_deref().unwrap_or(""),
                source.path.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "maint-{}",
        &sha256_hex(
            format!(
                "{}\n{}\n{}\n{}",
                request.level.as_str(),
                request.reason.trim(),
                source_key,
                now
            )
            .as_bytes()
        )[..12]
    )
}

fn append_maintenance_signal(
    paths: &MaestroPaths,
    id: &str,
    request: &MaintenanceRequest,
    now: &str,
) -> Result<()> {
    let row = json!({
        "schema_version": "maestro.memory.health_ledger.v1",
        "row_type": "signal",
        "maintenance_id": id,
        "signal": "maintenance_requested",
        "level": request.level.as_str(),
        "scope": request.scope.clone(),
        "source_refs": request.source_refs.clone(),
        "reason": request.reason.trim(),
        "at": now,
    });
    append_health_ledger_row(paths, row)
}

fn append_maintenance_state(
    paths: &MaestroPaths,
    id: &str,
    request: &MaintenanceRequest,
    contract_path: Option<&Path>,
    now: &str,
) -> Result<()> {
    let contract_path = contract_path.map(|path| display_repo_path(paths, path));
    let memory_ids = maintenance_memory_ids(request);
    if memory_ids.is_empty() {
        let row = json!({
            "schema_version": "maestro.memory.health_ledger.v1",
            "row_type": "state",
            "maintenance_id": id,
            "maintenance_contract_id": contract_path.as_ref().map(|_| id),
            "contract_path": contract_path.clone(),
            "state": "maintenance_due",
            "level": request.level.as_str(),
            "scope": request.scope.clone(),
            "source_refs": request.source_refs.clone(),
            "reason": request.reason.trim(),
            "at": now,
        });
        return append_health_ledger_row(paths, row);
    }

    for memory_id in memory_ids {
        let row = json!({
            "schema_version": "maestro.memory.health_ledger.v1",
            "row_type": "state",
            "memory_id": memory_id,
            "maintenance_id": id,
            "maintenance_contract_id": contract_path.as_ref().map(|_| id),
            "contract_path": contract_path.clone(),
            "state": "maintenance_due",
            "level": request.level.as_str(),
            "scope": request.scope.clone(),
            "source_refs": request.source_refs.clone(),
            "reason": request.reason.trim(),
            "at": now,
        });
        append_health_ledger_row(paths, row)?;
    }
    Ok(())
}

fn append_health_ledger_row(paths: &MaestroPaths, row: serde_json::Value) -> Result<()> {
    let line = serde_json::to_string(&row).context("failed to serialize health ledger row")? + "\n";
    append_text_file(health_ledger_path(paths), "", &line)?;
    Ok(())
}

fn maintenance_memory_ids(request: &MaintenanceRequest) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for source in &request.source_refs {
        if source.kind == "memory" {
            if let Some(id) = &source.id {
                ids.insert(id.clone());
            }
        } else if source.kind == "card"
            && let Some(id) = &source.id
            && id.starts_with("mem-")
        {
            ids.insert(id.clone());
        }
    }
    ids.into_iter().collect()
}

fn display_repo_path(paths: &MaestroPaths, path: &Path) -> String {
    path.strip_prefix(paths.repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn latest_health_states(paths: &MaestroPaths) -> Result<BTreeMap<String, String>> {
    let mut states = BTreeMap::new();
    let Some(raw) = read_to_string_if_exists(health_ledger_path(paths))? else {
        return Ok(states);
    };
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).context("failed to parse memory health ledger row")?;
        if value
            .get("row_type")
            .and_then(|value| value.as_str())
            .is_some_and(|kind| kind == "state")
            && let (Some(memory_id), Some(state)) = (
                value.get("memory_id").and_then(|value| value.as_str()),
                value.get("state").and_then(|value| value.as_str()),
            )
        {
            states.insert(memory_id.to_string(), state.to_string());
        }
    }
    Ok(states)
}

fn memory_scope_matches(memory_scope: &MemoryScope, read_scope: &MemoryReadScope) -> bool {
    match memory_scope.kind {
        ScopeKind::Card => read_scope.card_id.as_ref().is_some_and(|id| {
            memory_scope.refs.is_empty() || memory_scope.refs.iter().any(|value| value == id)
        }),
        ScopeKind::Task => read_scope.task_id.as_ref().is_some_and(|id| {
            memory_scope.refs.is_empty() || memory_scope.refs.iter().any(|value| value == id)
        }),
        ScopeKind::Feature => read_scope.feature_id.as_ref().is_some_and(|id| {
            memory_scope.refs.is_empty() || memory_scope.refs.iter().any(|value| value == id)
        }),
        ScopeKind::Project => read_scope.project.as_ref().is_none_or(|project| {
            memory_scope.refs.is_empty() || memory_scope.refs.iter().any(|value| value == project)
        }),
        ScopeKind::Repo | ScopeKind::Global | ScopeKind::Team => true,
    }
}

fn suggestion_is_expired(row: &MemorySuggestion, now: &str) -> bool {
    row.expires_at
        .as_deref()
        .is_some_and(|expires_at| expires_at <= now)
}

fn memory_matches_query(
    card: &Card,
    candidate: &MemoryCandidate,
    lesson: &str,
    query: &str,
) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    [
        card.id.as_str(),
        card.title.as_str(),
        card.description.as_deref().unwrap_or(""),
        lesson,
        candidate.memory.target_surface.as_str(),
        candidate.memory.scope.kind.as_str(),
    ]
    .into_iter()
    .any(|value| value.to_lowercase().contains(&query))
}

fn memory_rank_key(
    candidate: &MemoryCandidate,
    scope: &MemoryReadScope,
) -> (u8, u8, RiskLevel, String) {
    (
        scope_rank(&candidate.memory.scope, scope),
        confidence_rank(candidate.memory.signal_summary.confidence),
        candidate.memory.risk.overall,
        candidate.id.clone(),
    )
}

fn scope_rank(memory_scope: &MemoryScope, read_scope: &MemoryReadScope) -> u8 {
    match memory_scope.kind {
        ScopeKind::Card
            if read_scope
                .card_id
                .as_ref()
                .is_some_and(|id| memory_scope.refs.iter().any(|value| value == id)) =>
        {
            0
        }
        ScopeKind::Task
            if read_scope
                .task_id
                .as_ref()
                .is_some_and(|id| memory_scope.refs.iter().any(|value| value == id)) =>
        {
            0
        }
        ScopeKind::Feature
            if read_scope
                .feature_id
                .as_ref()
                .is_some_and(|id| memory_scope.refs.iter().any(|value| value == id)) =>
        {
            1
        }
        ScopeKind::Project
            if read_scope
                .project
                .as_ref()
                .is_some_and(|project| memory_scope.refs.iter().any(|value| value == project)) =>
        {
            1
        }
        ScopeKind::Repo => 2,
        ScopeKind::Global | ScopeKind::Team => 3,
        _ => 4,
    }
}

fn confidence_rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::High => 0,
        Confidence::Medium => 1,
        Confidence::Low => 2,
    }
}

fn memory_rank_reason(candidate: &MemoryCandidate, scope: &MemoryReadScope) -> String {
    let scope_label = match scope_rank(&candidate.memory.scope, scope) {
        0 => "exact scope",
        1 => "feature/project scope",
        2 => "repo scope",
        3 => "global/team scope",
        _ => "fallback scope",
    };
    format!(
        "{scope_label}; confidence={}; risk={}",
        confidence_label(candidate.memory.signal_summary.confidence),
        candidate.memory.risk.overall.as_str()
    )
}

fn confidence_label(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn compact_lesson_summary(lesson: &str, card: &Card) -> String {
    let raw = lesson
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| card.description.as_deref().unwrap_or(card.title.as_str()))
        .trim();
    const LIMIT: usize = 180;
    let mut summary = raw.chars().take(LIMIT).collect::<String>();
    if raw.chars().count() > LIMIT {
        summary.push_str("...");
    }
    summary
}

fn display_memory_lesson_path(memory_id: &str) -> String {
    format!(".maestro/cards/{memory_id}/memory/{LESSON_FILE}")
}

fn validate_suggestion_request(request: &CreateSuggestionRequest) -> Result<()> {
    if request.summary.trim().is_empty() {
        bail!("suggestion summary cannot be empty");
    }
    if request.source_refs.is_empty() {
        bail!("suggestion needs at least one source ref");
    }
    if request.target_surface == TargetSurface::ExternalAction {
        bail!("memory suggestions cannot target external_action");
    }
    Ok(())
}

fn parse_scorer_contract(value: &serde_yaml::Value) -> Result<ScorerContract> {
    let contract: ScorerContract =
        serde_yaml::from_value(value.clone()).context("failed to parse gate.scorer_contract")?;
    if contract.scorer_type != ScorerType::Schema && contract.argv.is_empty() {
        bail!(
            "{} scorer contracts require non-empty argv",
            contract.scorer_type.as_str()
        );
    }
    Ok(contract)
}

#[derive(Clone, Debug)]
struct ScorerExecution {
    passed: bool,
    exit_code: Option<i32>,
    stdout: Option<String>,
    stderr: Option<String>,
    error: Option<String>,
}

fn execute_scorer(
    paths: &MaestroPaths,
    memory_id: &str,
    contract: &ScorerContract,
) -> ScorerExecution {
    if contract.scorer_type == ScorerType::Schema {
        return ScorerExecution {
            passed: true,
            exit_code: Some(0),
            stdout: Some(format!("validated Memory candidate {memory_id}")),
            stderr: None,
            error: None,
        };
    }
    let output = Command::new(&contract.argv[0])
        .args(&contract.argv[1..])
        .current_dir(paths.repo_root())
        .output();
    match output {
        Ok(output) => ScorerExecution {
            passed: output.status.success(),
            exit_code: output.status.code(),
            stdout: Some(String::from_utf8_lossy(&output.stdout).into_owned()),
            stderr: Some(String::from_utf8_lossy(&output.stderr).into_owned()),
            error: None,
        },
        Err(error) => ScorerExecution {
            passed: false,
            exit_code: None,
            stdout: None,
            stderr: None,
            error: Some(error.to_string()),
        },
    }
}

fn receipt_path(paths: &MaestroPaths, memory_id: &str, receipt_id: &str) -> PathBuf {
    crate::domain::memory::memory_dir(paths, memory_id)
        .join(RECEIPTS_DIR)
        .join(format!("{receipt_id}.json"))
}

fn bounded_output(value: Option<String>) -> Option<String> {
    let value = value?;
    const LIMIT: usize = 4_000;
    if value.len() <= LIMIT {
        Some(value)
    } else {
        let truncated = value.chars().take(LIMIT).collect::<String>();
        Some(format!("{truncated}...[truncated]"))
    }
}

#[derive(Clone, Debug)]
struct CreateSeed {
    suggestion_id: Option<String>,
    source_refs: Vec<SourceRef>,
    signal_type: SignalType,
    summary: String,
    scope: MemoryScope,
    target_surface: TargetSurface,
}

fn resolve_create_seed(
    paths: &MaestroPaths,
    request: &CreateMemoryRequest,
    _now: &str,
) -> Result<(CreateSeed, Option<QueueSnapshot>)> {
    if request.from.starts_with("msug-") {
        let snapshot = load_queue(paths)?;
        let Some(row) = snapshot.rows.iter().find(|row| row.id == request.from) else {
            bail!("memory suggestion {} not found", request.from);
        };
        if row.status != SuggestionStatus::Open {
            bail!(
                "memory suggestion {} is {}, not open",
                row.id,
                row.status.as_str()
            );
        }
        let seed = CreateSeed {
            suggestion_id: Some(row.id.clone()),
            source_refs: row.source_refs.clone(),
            signal_type: row.signal_type,
            summary: row.summary.clone(),
            scope: row.scope.clone(),
            target_surface: row.target_surface,
        };
        return Ok((seed, Some(snapshot)));
    }

    let target_surface = request.target_surface.unwrap_or(TargetSurface::MemoryNote);
    if target_surface == TargetSurface::ExternalAction {
        bail!("memory create cannot target external_action");
    }
    let seed = CreateSeed {
        suggestion_id: None,
        source_refs: vec![parse_source_ref(&request.from)],
        signal_type: request.signal_type.unwrap_or(SignalType::UserCorrection),
        summary: request.summary.clone().unwrap_or_default(),
        scope: request.scope.clone().unwrap_or(MemoryScope {
            kind: ScopeKind::Repo,
            refs: Vec::new(),
        }),
        target_surface,
    };
    Ok((seed, None))
}

fn write_memory_sidecars(
    paths: &MaestroPaths,
    card: &Card,
    seed: &CreateSeed,
    summary: &str,
    lesson: &str,
    now: &str,
) -> Result<()> {
    let dir = crate::domain::memory::memory_dir(paths, &card.id);
    ensure_dir(&dir)?;
    ensure_dir(dir.join(RECEIPTS_DIR))?;

    let candidate = candidate_from_seed(card, seed);
    let candidate_yaml =
        serde_yaml::to_string(&candidate).context("failed to serialize memory candidate")?;
    write_string_atomic(dir.join(CANDIDATE_FILE), &candidate_yaml)?;
    write_string_atomic(dir.join(crate::domain::memory::LESSON_FILE), lesson)?;
    let signal = json!({
        "schema_version": "maestro.memory.signal.v1",
        "at": now,
        "signal_type": seed.signal_type.as_str(),
        "summary": summary,
        "source_refs": seed.source_refs,
    });
    let signal_line =
        serde_json::to_string(&signal).context("failed to serialize memory signal")? + "\n";
    write_string_atomic(dir.join(SIGNALS_FILE), &signal_line)?;
    Ok(())
}

fn candidate_from_seed(card: &Card, seed: &CreateSeed) -> MemoryCandidate {
    MemoryCandidate {
        schema_version: crate::domain::memory::CANDIDATE_SCHEMA_VERSION.to_string(),
        id: card.id.clone(),
        memory: MemoryMetadata {
            lifecycle: MemoryLifecycle::Proposed,
            target_tier: target_tier_for(seed.target_surface),
            target_surface: seed.target_surface,
            scope: seed.scope.clone(),
            signal_summary: SignalSummary {
                signal_types: vec![seed.signal_type],
                source_refs: seed.source_refs.clone(),
                confidence: Confidence::Medium,
            },
            risk: low_risk(),
            gate: Gate {
                required: GateRequired::Review,
                scorer_contract: None,
                review_required: true,
                rollback_path: None,
                expiry: None,
                stale_conditions: Vec::new(),
                allowed_targets: vec![seed.target_surface],
            },
            freshness: Freshness {
                expires_at: None,
                revalidate_after: None,
                stale_conditions: Vec::new(),
            },
            rollback: Rollback {
                path: None,
                backup_refs: Vec::new(),
                supersedes: Vec::new(),
                superseded_by: None,
            },
            links: Links {
                lesson: "memory/lesson.md".to_string(),
                signals: "memory/signals.jsonl".to_string(),
                receipts_dir: "memory/receipts".to_string(),
                health_ledger: ".maestro/memory/health-ledger.jsonl".to_string(),
            },
        },
    }
}

fn target_tier_for(surface: TargetSurface) -> TargetTier {
    match surface {
        TargetSurface::MemoryNote => TargetTier::MemoryNote,
        TargetSurface::LocalSkill => TargetTier::SkillCandidate,
        TargetSurface::ShippedSkill => TargetTier::ShippedSkill,
        TargetSurface::RecurrenceGuard => TargetTier::RecurrenceGuard,
        TargetSurface::HarnessPolicy => TargetTier::HarnessPolicy,
        TargetSurface::Hook => TargetTier::Hook,
        TargetSurface::CliBehavior => TargetTier::CliBehavior,
        TargetSurface::ExternalAction => TargetTier::ExternalAction,
    }
}

fn low_risk() -> RiskClassification {
    RiskClassification {
        overall: RiskLevel::Low,
        axes: RiskAxes {
            target_tier: RiskLevel::Low,
            target_surface: RiskLevel::Low,
            scope_blast_radius: RiskLevel::Low,
            source_strength: RiskLevel::Low,
            reversibility: RiskLevel::Low,
            scorer_strength: RiskLevel::Low,
            external_authority: RiskLevel::Low,
        },
        registry_hash: None,
        registry_adjustments: Vec::new(),
    }
}

fn load_queue(paths: &MaestroPaths) -> Result<QueueSnapshot> {
    let path = suggestions_path(paths);
    let raw = read_to_string_if_exists(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut rows = Vec::new();
    if let Some(raw) = raw.as_deref() {
        for (index, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let row: MemorySuggestion = serde_json::from_str(line).with_context(|| {
                format!(
                    "failed to parse memory suggestion row {} in {}",
                    index + 1,
                    path.display()
                )
            })?;
            rows.push(row);
        }
    }
    Ok(QueueSnapshot { raw, rows })
}

fn save_queue(paths: &MaestroPaths, snapshot: &QueueSnapshot) -> Result<()> {
    let path = suggestions_path(paths);
    ensure_dir(path.parent().expect("suggestions path has a parent"))?;
    let mut contents = String::new();
    for row in &snapshot.rows {
        contents.push_str(
            &serde_json::to_string(row).context("failed to serialize memory suggestion")?,
        );
        contents.push('\n');
    }
    write_string_if_unchanged(path, snapshot.raw.as_deref(), &contents)
}

fn merge_source_refs(existing: &mut Vec<SourceRef>, incoming: &[SourceRef]) {
    let mut seen: BTreeSet<String> = existing.iter().map(source_key).collect();
    for source in incoming {
        if seen.insert(source_key(source)) {
            existing.push(source.clone());
        }
    }
}

fn source_key(source: &SourceRef) -> String {
    format!(
        "{}:{}:{}",
        source.kind,
        source.id.as_deref().unwrap_or(""),
        source.path.as_deref().unwrap_or("")
    )
}

fn default_dedupe_key(request: &CreateSuggestionRequest) -> String {
    let scope_refs = request.scope.refs.join(",");
    format!(
        "{}|{}|{}|{}",
        request.scope.kind.as_str(),
        scope_refs,
        request.target_surface.as_str(),
        normalized_summary(&request.summary)
    )
}

fn normalized_summary(summary: &str) -> String {
    summary
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn lesson_fingerprint(dedupe_key: &str) -> String {
    format!("sha256:{}", sha256_hex(dedupe_key.as_bytes()))
}

fn next_suggestion_id(rows: &[MemorySuggestion], dedupe_key: &str, now: &str) -> String {
    let base = format!("msug-{}", &sha256_hex(dedupe_key.as_bytes())[..12]);
    if rows.iter().all(|row| row.id != base) {
        return base;
    }
    format!(
        "{}-{}",
        base,
        &sha256_hex(format!("{dedupe_key}\n{now}").as_bytes())[..4]
    )
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const NOW: &str = "2026-06-27T00:00:00Z";

    fn paths(name: &str) -> MaestroPaths {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        MaestroPaths::new(std::env::temp_dir().join(format!("maestro-memory-ops-{name}-{stamp}")))
    }

    fn cleanup(paths: &MaestroPaths) {
        let _ = std::fs::remove_dir_all(paths.repo_root());
    }

    fn request(summary: &str) -> CreateSuggestionRequest {
        CreateSuggestionRequest {
            source_refs: vec![SourceRef {
                kind: "run_event".to_string(),
                id: Some("run-1".to_string()),
                path: Some(".maestro/runs/run-1/events.jsonl".to_string()),
            }],
            signal_type: SignalType::Failure,
            summary: summary.to_string(),
            scope: MemoryScope {
                kind: ScopeKind::Repo,
                refs: Vec::new(),
            },
            target_surface: TargetSurface::MemoryNote,
            dedupe_key: None,
            expires_at: None,
        }
    }

    #[test]
    fn suggestion_create_upserts_open_dedupe_key() {
        let paths = paths("upsert");
        let first = create_suggestion(&paths, request("Remember the checkout lock"), NOW)
            .expect("create first");
        let second = create_suggestion(&paths, request("Remember the checkout lock"), NOW)
            .expect("upsert second");

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.suggestion.id, second.suggestion.id);
        assert_eq!(second.suggestion.signal_count, 2);
        assert_eq!(list_suggestions(&paths, false).expect("list").len(), 1);

        cleanup(&paths);
    }

    #[test]
    fn suggestion_dismiss_hides_from_default_list() {
        let paths = paths("dismiss");
        let created = create_suggestion(&paths, request("Dismiss me"), NOW).expect("create");
        let dismissed =
            dismiss_suggestion(&paths, &created.suggestion.id, "not reusable", "codex", NOW)
                .expect("dismiss");

        assert_eq!(dismissed.suggestion.status, SuggestionStatus::Dismissed);
        assert!(list_suggestions(&paths, false).expect("list").is_empty());
        assert_eq!(list_suggestions(&paths, true).expect("list all").len(), 1);

        cleanup(&paths);
    }

    #[test]
    fn memory_create_from_suggestion_writes_sidecars_and_marks_created() {
        let paths = paths("create");
        let created = create_suggestion(&paths, request("Known refund gotcha"), NOW)
            .expect("create suggestion");
        let outcome = create_memory(
            &paths,
            CreateMemoryRequest {
                from: created.suggestion.id.clone(),
                summary: None,
                lesson: Some("Always inspect the refund ledger first.".to_string()),
                signal_type: None,
                scope: None,
                target_surface: None,
            },
            NOW,
        )
        .expect("create memory");

        let card = store::resolve(&paths, &outcome.id)
            .expect("resolve")
            .expect("memory card exists")
            .card;
        assert_eq!(card.card_type, CardType::Memory);
        validate_card(&paths, &card).expect("generated candidate validates");
        assert!(
            crate::domain::memory::memory_dir(&paths, &outcome.id)
                .join(RECEIPTS_DIR)
                .is_dir()
        );

        let rows = list_suggestions(&paths, true).expect("list all");
        assert_eq!(rows[0].status, SuggestionStatus::Created);
        assert_eq!(
            rows[0].created_memory_id.as_deref(),
            Some(outcome.id.as_str())
        );

        cleanup(&paths);
    }

    #[test]
    fn bounded_output_truncates_on_char_boundary() {
        let raw = format!("{}{}", "a".repeat(4_000), "é".repeat(4));
        let bounded = bounded_output(Some(raw)).expect("bounded output");

        assert!(bounded.ends_with("...[truncated]"));
        assert_eq!(
            bounded.trim_end_matches("...[truncated]").chars().count(),
            4_000
        );
    }
}
