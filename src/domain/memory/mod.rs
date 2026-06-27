use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::card::schema::{Card, CardType};
use crate::foundation::core::paths::MaestroPaths;

pub const CANDIDATE_SCHEMA_VERSION: &str = "maestro.memory.candidate.v1";
pub const MEMORY_DIR: &str = "memory";
pub const CANDIDATE_FILE: &str = "candidate.yml";
pub const LESSON_FILE: &str = "lesson.md";
pub const SIGNALS_FILE: &str = "signals.jsonl";
pub const RECEIPTS_DIR: &str = "receipts";

pub fn memory_dir(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.cards_dir().join(id).join(MEMORY_DIR)
}

pub fn candidate_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    memory_dir(paths, id).join(CANDIDATE_FILE)
}

pub fn validate_card(paths: &MaestroPaths, card: &Card) -> Result<MemoryCandidate> {
    let path = candidate_path(paths, &card.id);
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let candidate = parse_candidate(&raw, &path.display().to_string())?;
    validate_candidate_for_card(card, &candidate)?;
    Ok(candidate)
}

pub fn parse_candidate(raw: &str, source_name: &str) -> Result<MemoryCandidate> {
    serde_yaml::from_str(raw)
        .with_context(|| format!("failed to parse Memory candidate schema in {source_name}"))
}

pub fn validate_candidate_for_card(card: &Card, candidate: &MemoryCandidate) -> Result<()> {
    if card.card_type != CardType::Memory {
        bail!(
            "card {} is a {}, not a memory card",
            card.id,
            card.card_type.as_str()
        );
    }
    if candidate.schema_version != CANDIDATE_SCHEMA_VERSION {
        bail!(
            "memory candidate {} has schema_version {}, expected {CANDIDATE_SCHEMA_VERSION}",
            candidate.id,
            candidate.schema_version
        );
    }
    if candidate.id != card.id {
        bail!(
            "memory candidate id {} does not match card {}",
            candidate.id,
            card.id
        );
    }
    validate_lifecycle_status(card, candidate.memory.lifecycle)?;
    validate_sources(candidate)?;
    validate_links(candidate)?;
    validate_risk_and_gate(candidate)?;
    Ok(())
}

fn validate_lifecycle_status(card: &Card, lifecycle: MemoryLifecycle) -> Result<()> {
    let allowed = match lifecycle {
        MemoryLifecycle::Candidate | MemoryLifecycle::Proposed => {
            matches!(card.status.as_str(), "proposed" | "in_progress")
        }
        MemoryLifecycle::Gated => matches!(card.status.as_str(), "ready" | "in_progress"),
        MemoryLifecycle::Promoted => matches!(card.status.as_str(), "verified" | "closed"),
        MemoryLifecycle::Rejected | MemoryLifecycle::Superseded => card.status == "closed",
        MemoryLifecycle::Stale | MemoryLifecycle::Quarantined => {
            matches!(card.status.as_str(), "verified" | "closed")
        }
    };
    if !allowed {
        bail!(
            "memory lifecycle {} is not allowed with card.status {}",
            lifecycle.as_str(),
            card.status
        );
    }
    Ok(())
}

fn validate_sources(candidate: &MemoryCandidate) -> Result<()> {
    let summary = &candidate.memory.signal_summary;
    if summary.signal_types.is_empty() {
        bail!("memory candidate {} has no signal_types", candidate.id);
    }
    if summary.source_refs.is_empty() {
        bail!("memory candidate {} has no source_refs", candidate.id);
    }
    for source in &summary.source_refs {
        if source.id.is_none() && source.path.is_none() {
            bail!(
                "memory candidate {} source_ref {} needs id or path",
                candidate.id,
                source.kind
            );
        }
        if forbidden_source_kind(&source.kind) {
            bail!(
                "memory candidate {} uses forbidden source kind {}",
                candidate.id,
                source.kind
            );
        }
    }
    Ok(())
}

pub fn forbidden_source_kind(kind: &str) -> bool {
    matches!(
        kind,
        "raw_screen_recording"
            | "screen_recording"
            | "keystroke"
            | "raw_keystroke"
            | "click_stream"
            | "raw_click_stream"
            | "app_telemetry"
            | "silence_inference"
            | "unrecorded_chat"
            | "chat_memory_extraction"
            | "private_planner_state"
    )
}

fn validate_links(candidate: &MemoryCandidate) -> Result<()> {
    let links = &candidate.memory.links;
    if links.lesson != format!("{MEMORY_DIR}/{LESSON_FILE}") {
        bail!(
            "memory candidate {} has non-canonical lesson link",
            candidate.id
        );
    }
    if links.signals != format!("{MEMORY_DIR}/{SIGNALS_FILE}") {
        bail!(
            "memory candidate {} has non-canonical signals link",
            candidate.id
        );
    }
    if links.receipts_dir != format!("{MEMORY_DIR}/{RECEIPTS_DIR}") {
        bail!(
            "memory candidate {} has non-canonical receipts_dir link",
            candidate.id
        );
    }
    if links.health_ledger != ".maestro/memory/health-ledger.jsonl" {
        bail!(
            "memory candidate {} has non-canonical health_ledger link",
            candidate.id
        );
    }
    Ok(())
}

fn validate_risk_and_gate(candidate: &MemoryCandidate) -> Result<()> {
    let risk = candidate.memory.risk.overall;
    let max_axis = candidate.memory.risk.axes.max_level();
    if risk != max_axis {
        bail!(
            "memory candidate {} risk.overall {} must equal max axis risk {}",
            candidate.id,
            risk.as_str(),
            max_axis.as_str()
        );
    }

    let external_target = candidate.memory.target_tier == TargetTier::ExternalAction
        || candidate.memory.target_surface == TargetSurface::ExternalAction;
    if external_target
        && (risk != RiskLevel::Forbidden
            || candidate.memory.risk.axes.external_authority != RiskLevel::Forbidden
            || candidate.memory.gate.required != GateRequired::Forbidden)
    {
        bail!(
            "memory candidate {} targets external authority; risk and gate must be forbidden",
            candidate.id
        );
    }

    if risk == RiskLevel::Forbidden && candidate.memory.gate.required != GateRequired::Forbidden {
        bail!(
            "memory candidate {} has forbidden risk with non-forbidden gate",
            candidate.id
        );
    }

    if !gate_satisfies_risk(candidate.memory.gate.required, risk) {
        bail!(
            "memory candidate {} gate {} is too weak for {} risk",
            candidate.id,
            candidate.memory.gate.required.as_str(),
            risk.as_str()
        );
    }

    if candidate.memory.lifecycle == MemoryLifecycle::Promoted
        && candidate
            .memory
            .risk
            .registry_hash
            .as_deref()
            .unwrap_or("")
            .is_empty()
    {
        bail!(
            "promoted memory candidate {} is missing risk.registry_hash",
            candidate.id
        );
    }
    Ok(())
}

fn gate_satisfies_risk(gate: GateRequired, risk: RiskLevel) -> bool {
    match risk {
        RiskLevel::Low => matches!(
            gate,
            GateRequired::Review | GateRequired::Scorer | GateRequired::ScorerAndReview
        ),
        RiskLevel::Medium => matches!(
            gate,
            GateRequired::Review | GateRequired::Scorer | GateRequired::ScorerAndReview
        ),
        RiskLevel::High | RiskLevel::Critical => gate == GateRequired::ScorerAndReview,
        RiskLevel::Forbidden => gate == GateRequired::Forbidden,
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCandidate {
    pub schema_version: String,
    pub id: String,
    pub memory: MemoryMetadata,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryMetadata {
    pub lifecycle: MemoryLifecycle,
    pub target_tier: TargetTier,
    pub target_surface: TargetSurface,
    pub scope: MemoryScope,
    pub signal_summary: SignalSummary,
    pub risk: RiskClassification,
    pub gate: Gate,
    pub freshness: Freshness,
    pub rollback: Rollback,
    pub links: Links,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifecycle {
    Candidate,
    Proposed,
    Gated,
    Promoted,
    Rejected,
    Stale,
    Superseded,
    Quarantined,
}

impl MemoryLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Proposed => "proposed",
            Self::Gated => "gated",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
            Self::Stale => "stale",
            Self::Superseded => "superseded",
            Self::Quarantined => "quarantined",
        }
    }
}

impl SignalType {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "failure" => Some(Self::Failure),
            "user_correction" => Some(Self::UserCorrection),
            "verified_success" => Some(Self::VerifiedSuccess),
            "repeated_block" => Some(Self::RepeatedBlock),
            "loop_hard_stop" => Some(Self::LoopHardStop),
            "good_run" => Some(Self::GoodRun),
            "manual_final_decision" => Some(Self::ManualFinalDecision),
            "approval" => Some(Self::Approval),
            "rejection" => Some(Self::Rejection),
            "health_signal" => Some(Self::HealthSignal),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Failure => "failure",
            Self::UserCorrection => "user_correction",
            Self::VerifiedSuccess => "verified_success",
            Self::RepeatedBlock => "repeated_block",
            Self::LoopHardStop => "loop_hard_stop",
            Self::GoodRun => "good_run",
            Self::ManualFinalDecision => "manual_final_decision",
            Self::Approval => "approval",
            Self::Rejection => "rejection",
            Self::HealthSignal => "health_signal",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetTier {
    MemoryNote,
    SkillCandidate,
    RecurrenceGuard,
    ShippedSkill,
    HarnessPolicy,
    Hook,
    CliBehavior,
    ExternalAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSurface {
    MemoryNote,
    LocalSkill,
    ShippedSkill,
    RecurrenceGuard,
    HarnessPolicy,
    Hook,
    CliBehavior,
    ExternalAction,
}

impl TargetSurface {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "memory_note" => Some(Self::MemoryNote),
            "local_skill" => Some(Self::LocalSkill),
            "shipped_skill" => Some(Self::ShippedSkill),
            "recurrence_guard" => Some(Self::RecurrenceGuard),
            "harness_policy" => Some(Self::HarnessPolicy),
            "hook" => Some(Self::Hook),
            "cli_behavior" => Some(Self::CliBehavior),
            "external_action" => Some(Self::ExternalAction),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryNote => "memory_note",
            Self::LocalSkill => "local_skill",
            Self::ShippedSkill => "shipped_skill",
            Self::RecurrenceGuard => "recurrence_guard",
            Self::HarnessPolicy => "harness_policy",
            Self::Hook => "hook",
            Self::CliBehavior => "cli_behavior",
            Self::ExternalAction => "external_action",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryScope {
    pub kind: ScopeKind,
    pub refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Task,
    Card,
    Feature,
    Project,
    Repo,
    Global,
    Team,
}

impl ScopeKind {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "task" => Some(Self::Task),
            "card" => Some(Self::Card),
            "feature" => Some(Self::Feature),
            "project" => Some(Self::Project),
            "repo" => Some(Self::Repo),
            "global" => Some(Self::Global),
            "team" => Some(Self::Team),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Card => "card",
            Self::Feature => "feature",
            Self::Project => "project",
            Self::Repo => "repo",
            Self::Global => "global",
            Self::Team => "team",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignalSummary {
    pub signal_types: Vec<SignalType>,
    pub source_refs: Vec<SourceRef>,
    pub confidence: Confidence,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    Failure,
    UserCorrection,
    VerifiedSuccess,
    RepeatedBlock,
    LoopHardStop,
    GoodRun,
    ManualFinalDecision,
    Approval,
    Rejection,
    HealthSignal,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiskClassification {
    pub overall: RiskLevel,
    pub axes: RiskAxes,
    pub registry_hash: Option<String>,
    pub registry_adjustments: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiskAxes {
    pub target_tier: RiskLevel,
    pub target_surface: RiskLevel,
    pub scope_blast_radius: RiskLevel,
    pub source_strength: RiskLevel,
    pub reversibility: RiskLevel,
    pub scorer_strength: RiskLevel,
    pub external_authority: RiskLevel,
}

impl RiskAxes {
    fn max_level(&self) -> RiskLevel {
        [
            self.target_tier,
            self.target_surface,
            self.scope_blast_radius,
            self.source_strength,
            self.reversibility,
            self.scorer_strength,
            self.external_authority,
        ]
        .into_iter()
        .max()
        .expect("invariant: risk axes array is non-empty")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
    Forbidden,
}

impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
            Self::Forbidden => "forbidden",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub required: GateRequired,
    pub scorer_contract: Option<serde_yaml::Value>,
    pub review_required: bool,
    pub rollback_path: Option<String>,
    pub expiry: Option<String>,
    pub stale_conditions: Vec<String>,
    pub allowed_targets: Vec<TargetSurface>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateRequired {
    Review,
    Scorer,
    ScorerAndReview,
    Forbidden,
}

impl GateRequired {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Scorer => "scorer",
            Self::ScorerAndReview => "scorer_and_review",
            Self::Forbidden => "forbidden",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Freshness {
    pub expires_at: Option<String>,
    pub revalidate_after: Option<String>,
    pub stale_conditions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rollback {
    pub path: Option<String>,
    pub backup_refs: Vec<String>,
    pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Links {
    pub lesson: String,
    pub signals: String,
    pub receipts_dir: String,
    pub health_ledger: String,
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::foundation::core::fs::ensure_dir;

    const NOW: &str = "2026-06-27T00:00:00Z";

    fn valid_yaml() -> String {
        r#"schema_version: maestro.memory.candidate.v1
id: mem-refund-policy-1234
memory:
  lifecycle: proposed
  target_tier: memory_note
  target_surface: memory_note
  scope:
    kind: repo
    refs: []
  signal_summary:
    signal_types:
      - user_correction
    source_refs:
      - kind: run_event
        id: run-001
        path: .maestro/runs/run-001/events.jsonl
    confidence: high
  risk:
    overall: low
    axes:
      target_tier: low
      target_surface: low
      scope_blast_radius: low
      source_strength: low
      reversibility: low
      scorer_strength: low
      external_authority: low
    registry_hash: null
    registry_adjustments: []
  gate:
    required: review
    scorer_contract: null
    review_required: true
    rollback_path: null
    expiry: null
    stale_conditions: []
    allowed_targets:
      - memory_note
  freshness:
    expires_at: null
    revalidate_after: null
    stale_conditions: []
  rollback:
    path: null
    backup_refs: []
    supersedes: []
    superseded_by: null
  links:
    lesson: memory/lesson.md
    signals: memory/signals.jsonl
    receipts_dir: memory/receipts
    health_ledger: .maestro/memory/health-ledger.jsonl
"#
        .to_string()
    }

    fn memory_card(status: &str) -> Card {
        Card::new(
            "mem-refund-policy-1234",
            CardType::Memory,
            "Refund policy",
            status,
            NOW,
        )
    }

    fn temp_paths(name: &str) -> MaestroPaths {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-memory-{name}-{stamp}"));
        MaestroPaths::new(root)
    }

    fn write_candidate(paths: &MaestroPaths, id: &str, yaml: &str) {
        let dir = memory_dir(paths, id);
        ensure_dir(&dir).expect("create memory dir");
        std::fs::write(dir.join(CANDIDATE_FILE), yaml).expect("write candidate");
    }

    fn cleanup(paths: &MaestroPaths) {
        let _ = std::fs::remove_dir_all(paths.repo_root());
    }

    #[test]
    fn loads_and_validates_candidate_sidecar() {
        let paths = temp_paths("valid");
        let card = memory_card("proposed");
        write_candidate(&paths, &card.id, &valid_yaml());

        let candidate = validate_card(&paths, &card).expect("valid memory candidate");
        assert_eq!(candidate.id, card.id);

        cleanup(&paths);
    }

    #[test]
    fn rejects_non_memory_card() {
        let candidate = parse_candidate(&valid_yaml(), "test").expect("parse candidate");
        let card = Card::new(
            "mem-refund-policy-1234",
            CardType::Task,
            "Task",
            "ready",
            NOW,
        );

        let error = validate_candidate_for_card(&card, &candidate).expect_err("reject type");
        assert!(format!("{error:#}").contains("not a memory card"));
    }

    #[test]
    fn rejects_lifecycle_status_mismatch() {
        let candidate = parse_candidate(&valid_yaml(), "test").expect("parse candidate");
        let card = memory_card("open");

        let error = validate_candidate_for_card(&card, &candidate).expect_err("reject status");
        assert!(format!("{error:#}").contains("not allowed with card.status open"));
    }

    #[test]
    fn rejects_forbidden_source_kind() {
        let yaml = valid_yaml().replace("kind: run_event", "kind: raw_screen_recording");
        let candidate = parse_candidate(&yaml, "test").expect("parse candidate");

        let error =
            validate_candidate_for_card(&memory_card("proposed"), &candidate).expect_err("reject");
        assert!(format!("{error:#}").contains("forbidden source kind raw_screen_recording"));
    }

    #[test]
    fn rejects_external_authority_without_forbidden_gate() {
        let yaml = valid_yaml()
            .replace("target_tier: memory_note", "target_tier: external_action")
            .replace(
                "target_surface: memory_note",
                "target_surface: external_action",
            );
        let candidate = parse_candidate(&yaml, "test").expect("parse candidate");

        let error =
            validate_candidate_for_card(&memory_card("proposed"), &candidate).expect_err("reject");
        assert!(format!("{error:#}").contains("targets external authority"));
    }

    #[test]
    fn schema_rejects_unknown_candidate_fields() {
        let yaml = valid_yaml().replace(
            "id: mem-refund-policy-1234\n",
            "id: mem-refund-policy-1234\nhidden_memory: true\n",
        );

        let error = parse_candidate(&yaml, "test").expect_err("reject unknown");
        assert!(format!("{error:#}").contains("unknown field"));
    }

    #[test]
    fn candidate_path_uses_memory_sidecar_dir() {
        let paths = MaestroPaths::new(Path::new("/repo"));
        assert_eq!(
            candidate_path(&paths, "mem-abc"),
            Path::new("/repo")
                .join(".maestro")
                .join("cards")
                .join("mem-abc")
                .join("memory")
                .join("candidate.yml")
        );
    }
}
