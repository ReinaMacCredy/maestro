use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::run::Presence;
use crate::domain::{card, conflict, run};
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::utc_now_timestamp;

use super::registry;

const WORKTREE_LEDGER_FILE: &str = "worktree.yml";
const WORKTREE_LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorktreeLedger {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<WorktreeLane>,
}

impl Default for WorktreeLedger {
    fn default() -> Self {
        Self {
            schema_version: WORKTREE_LEDGER_SCHEMA_VERSION,
            lanes: Vec::new(),
        }
    }
}

impl WorktreeLedger {
    pub fn lane(&self, slug: &str) -> Option<&WorktreeLane> {
        self.lanes.iter().find(|lane| lane.intent.slug == slug)
    }

    pub fn lane_mut(&mut self, slug: &str) -> Option<&mut WorktreeLane> {
        self.lanes.iter_mut().find(|lane| lane.intent.slug == slug)
    }

    pub fn upsert_lane(&mut self, lane: WorktreeLane) {
        if let Some(existing) = self.lane_mut(&lane.intent.slug) {
            *existing = lane;
        } else {
            self.lanes.push(lane);
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorktreeLane {
    pub intent: WorktreeIntent,
    #[serde(default)]
    pub milestones: WorktreeMilestones,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cleanup_receipts: Vec<WorktreeCleanupReceipt>,
}

impl WorktreeLane {
    pub fn new(intent: WorktreeIntent) -> Self {
        Self {
            intent,
            milestones: WorktreeMilestones::default(),
            cleanup_receipts: Vec::new(),
        }
    }

    pub fn computed_state(&self, evidence: &WorktreeEvidence) -> WorktreeComputedState {
        if self.milestones.cleanup_completed_at.is_some() {
            return WorktreeComputedState::CleanupComplete;
        }
        if self.cleanup_due(evidence) {
            return WorktreeComputedState::CleanupDue;
        }
        if self.milestones.merged_back_at.is_some() && self.milestones.verified_at.is_none() {
            return WorktreeComputedState::MergedNeedsVerification;
        }
        if self.milestones.lane_created_at.is_some() || evidence.path_exists {
            return WorktreeComputedState::LanePresent;
        }
        if self.milestones.branch_reserved_at.is_some() || evidence.branch_exists {
            return WorktreeComputedState::BranchReservedPathMissing;
        }
        WorktreeComputedState::Unplanned
    }

    pub fn cleanup_due(&self, evidence: &WorktreeEvidence) -> bool {
        self.milestones.merged_back_at.is_some()
            && self.milestones.verified_at.is_some()
            && self.milestones.cleanup_completed_at.is_none()
            && evidence.worker_clean_or_absent
            && !evidence.active_owner
            && !evidence.open_conflict
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorktreeIntent {
    pub slug: String,
    pub branch: String,
    pub path: String,
    pub base: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_checkout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_worker_checkout: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorktreeMilestones {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_reserved_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_back_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_back_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_due_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_completed_at: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorktreeEvidence {
    pub branch_exists: bool,
    pub path_exists: bool,
    pub worker_clean_or_absent: bool,
    pub active_owner: bool,
    pub open_conflict: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeComputedState {
    Unplanned,
    BranchReservedPathMissing,
    LanePresent,
    MergedNeedsVerification,
    CleanupDue,
    CleanupComplete,
}

impl WorktreeComputedState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unplanned => "unplanned",
            Self::BranchReservedPathMissing => "branch_reserved_path_missing",
            Self::LanePresent => "lane_present",
            Self::MergedNeedsVerification => "merged_needs_verification",
            Self::CleanupDue => "cleanup_due",
            Self::CleanupComplete => "cleanup_complete",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorktreeCleanupReceipt {
    pub removed_path: String,
    pub deleted_branch: String,
    pub pruned_stale_metadata: bool,
    pub recorded_by: String,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMilestoneKind {
    LaneCreated,
    MergedBack { commit: String },
    Verified { commit: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeRecordReport {
    pub feature_id: String,
    pub slug: String,
    pub state: WorktreeComputedState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeLaneStatus {
    pub feature_id: String,
    pub slug: String,
    pub state: WorktreeComputedState,
    pub intent: WorktreeIntent,
    pub milestones: WorktreeMilestones,
    pub cleanup_receipts: Vec<WorktreeCleanupReceipt>,
    pub evidence: WorktreeEvidence,
}

pub fn plan_lane(
    paths: &MaestroPaths,
    feature_id: &str,
    intent: WorktreeIntent,
    recorded_at: &str,
) -> Result<WorktreeRecordReport> {
    ensure_non_empty("slug", &intent.slug)?;
    ensure_non_empty("branch", &intent.branch)?;
    ensure_non_empty("path", &intent.path)?;
    ensure_non_empty("base", &intent.base)?;
    let slug = intent.slug.clone();
    let mut ledger = load_or_default(paths, feature_id)?;
    let mut lane = WorktreeLane::new(intent);
    lane.milestones.branch_reserved_at = Some(recorded_at.to_string());
    ledger.upsert_lane(lane);
    save(paths, feature_id, &ledger)?;
    report_for(paths, feature_id, &slug)
}

pub fn mark_lane(
    paths: &MaestroPaths,
    feature_id: &str,
    slug: &str,
    milestone: WorktreeMilestoneKind,
    recorded_at: &str,
) -> Result<WorktreeRecordReport> {
    ensure_non_empty("slug", slug)?;
    let mut ledger = load_or_default(paths, feature_id)?;
    {
        let lane = ledger_lane_mut(&mut ledger, feature_id, slug)?;
        match milestone {
            WorktreeMilestoneKind::LaneCreated => {
                lane.milestones.lane_created_at = Some(recorded_at.to_string());
            }
            WorktreeMilestoneKind::MergedBack { commit } => {
                ensure_non_empty("commit", &commit)?;
                lane.milestones.merged_back_at = Some(recorded_at.to_string());
                lane.milestones.merged_back_commit = Some(commit);
            }
            WorktreeMilestoneKind::Verified { commit } => {
                ensure_non_empty("commit", &commit)?;
                lane.milestones.verified_at = Some(recorded_at.to_string());
                lane.milestones.verified_commit = Some(commit);
            }
        }
    }
    save(paths, feature_id, &ledger)?;
    report_for(paths, feature_id, slug)
}

pub fn record_cleanup(
    paths: &MaestroPaths,
    feature_id: &str,
    slug: &str,
    receipt: WorktreeCleanupReceipt,
) -> Result<WorktreeRecordReport> {
    ensure_non_empty("slug", slug)?;
    ensure_non_empty("removed-path", &receipt.removed_path)?;
    ensure_non_empty("deleted-branch", &receipt.deleted_branch)?;
    ensure_non_empty("recorded-by", &receipt.recorded_by)?;
    ensure_non_empty("recorded-at", &receipt.recorded_at)?;
    let mut ledger = load_or_default(paths, feature_id)?;
    {
        let lane = ledger_lane_mut(&mut ledger, feature_id, slug)?;
        lane.milestones.cleanup_completed_at = Some(receipt.recorded_at.clone());
        lane.cleanup_receipts.push(receipt);
    }
    save(paths, feature_id, &ledger)?;
    report_for(paths, feature_id, slug)
}

pub fn ledger_path(paths: &MaestroPaths, feature_id: &str) -> Result<PathBuf> {
    registry::load_record(paths, feature_id)?;
    Ok(registry::feature_sidecar_dir(paths, feature_id).join(WORKTREE_LEDGER_FILE))
}

pub fn load(paths: &MaestroPaths, feature_id: &str) -> Result<Option<WorktreeLedger>> {
    let path = ledger_path(paths, feature_id)?;
    let Some(raw) = read_to_string_if_exists(&path)? else {
        return Ok(None);
    };
    let ledger: WorktreeLedger = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    ensure_supported_schema(&ledger, &path)?;
    Ok(Some(ledger))
}

pub fn load_or_default(paths: &MaestroPaths, feature_id: &str) -> Result<WorktreeLedger> {
    Ok(load(paths, feature_id)?.unwrap_or_default())
}

pub fn lane_statuses(paths: &MaestroPaths, feature_id: &str) -> Result<Vec<WorktreeLaneStatus>> {
    let Some(ledger) = load(paths, feature_id)? else {
        return Ok(Vec::new());
    };
    let target_ids = target_card_ids(paths, feature_id)?;
    let now = utc_now_timestamp();
    let roots = worktree_roots(paths);
    let root_paths = roots
        .iter()
        .map(MaestroPaths::new)
        .collect::<Vec<MaestroPaths>>();
    let active_owner = has_active_owner(&root_paths, &target_ids, &now)?;
    let open_conflict = has_open_conflict(&root_paths, &target_ids, &now)?;

    ledger
        .lanes
        .into_iter()
        .map(|lane| {
            let evidence = evidence_for_lane(paths, &lane, active_owner, open_conflict)?;
            let state = lane.computed_state(&evidence);
            Ok(WorktreeLaneStatus {
                feature_id: feature_id.to_string(),
                slug: lane.intent.slug.clone(),
                state,
                intent: lane.intent,
                milestones: lane.milestones,
                cleanup_receipts: lane.cleanup_receipts,
                evidence,
            })
        })
        .collect()
}

pub fn save(paths: &MaestroPaths, feature_id: &str, ledger: &WorktreeLedger) -> Result<()> {
    if ledger.schema_version != WORKTREE_LEDGER_SCHEMA_VERSION {
        bail!(
            "unsupported worktree ledger schema {} for feature {feature_id}; expected {}",
            ledger.schema_version,
            WORKTREE_LEDGER_SCHEMA_VERSION
        );
    }
    let path = ledger_path(paths, feature_id)?;
    let contents = serde_yaml::to_string(ledger)?;
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn ensure_supported_schema(ledger: &WorktreeLedger, path: &std::path::Path) -> Result<()> {
    if ledger.schema_version == WORKTREE_LEDGER_SCHEMA_VERSION {
        return Ok(());
    }
    bail!(
        "{} has unsupported worktree ledger schema {}; expected {}",
        path.display(),
        ledger.schema_version,
        WORKTREE_LEDGER_SCHEMA_VERSION
    )
}

fn ledger_lane_mut<'a>(
    ledger: &'a mut WorktreeLedger,
    feature_id: &str,
    slug: &str,
) -> Result<&'a mut WorktreeLane> {
    ledger
        .lane_mut(slug)
        .with_context(|| format!("feature {feature_id} has no worktree lane {slug}"))
}

fn report_for(paths: &MaestroPaths, feature_id: &str, slug: &str) -> Result<WorktreeRecordReport> {
    let ledger = load_or_default(paths, feature_id)?;
    let lane = ledger
        .lane(slug)
        .with_context(|| format!("feature {feature_id} has no worktree lane {slug}"))?;
    Ok(WorktreeRecordReport {
        feature_id: feature_id.to_string(),
        slug: slug.to_string(),
        state: lane.computed_state(&WorktreeEvidence::default()),
    })
}

fn ensure_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("worktree {field} must not be empty");
    }
    Ok(())
}

fn evidence_for_lane(
    paths: &MaestroPaths,
    lane: &WorktreeLane,
    active_owner: bool,
    open_conflict: bool,
) -> Result<WorktreeEvidence> {
    let worker_path = checkout_path(paths, &lane.intent.path);
    let path_exists = worker_path.exists();
    let branch_exists = git::local_branch_exists(paths.repo_root(), &lane.intent.branch)?;
    let worker_clean_or_absent = if path_exists {
        git::dirty(&worker_path)
            .map(|dirty| !dirty)
            .unwrap_or(false)
    } else {
        true
    };
    Ok(WorktreeEvidence {
        branch_exists,
        path_exists,
        worker_clean_or_absent,
        active_owner,
        open_conflict,
    })
}

fn checkout_path(paths: &MaestroPaths, checkout: &str) -> PathBuf {
    let path = PathBuf::from(checkout);
    if path.is_absolute() {
        path
    } else {
        paths.repo_root().join(path)
    }
}

fn worktree_roots(paths: &MaestroPaths) -> Vec<PathBuf> {
    git::worktree_roots(paths.repo_root()).unwrap_or_else(|_| vec![paths.repo_root().to_path_buf()])
}

fn target_card_ids(paths: &MaestroPaths, feature_id: &str) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::from([feature_id.to_string()]);
    for card in card::query::scan(paths)? {
        if card.parent.as_deref() == Some(feature_id) {
            ids.insert(card.id);
        }
    }
    Ok(ids)
}

fn has_active_owner(
    roots: &[MaestroPaths],
    target_ids: &BTreeSet<String>,
    now: &str,
) -> Result<bool> {
    Ok(run::active_sessions_union(roots, now)?
        .into_iter()
        .any(|row| {
            row.bound_card
                .as_deref()
                .is_some_and(|card| target_ids.contains(card))
                && matches!(
                    row.presence,
                    Presence::Working | Presence::QuietWorking | Presence::Unconfirmed
                )
        }))
}

fn has_open_conflict(
    roots: &[MaestroPaths],
    target_ids: &BTreeSet<String>,
    now: &str,
) -> Result<bool> {
    Ok(conflict::active_notices(roots, now)?
        .into_iter()
        .any(|notice| {
            target_ids.contains(&notice.asserter_card) || target_ids.contains(&notice.peer_card)
        }))
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::foundation::core::paths::MaestroPaths;

    use super::*;

    fn test_repo(label: &str) -> (PathBuf, MaestroPaths, String) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-worktree-ledger-{label}-{}-{nanos}",
            process::id()
        ));
        let paths = MaestroPaths::new(&root);
        let id = registry::create(&paths, "Worktree Recovery", None).expect("create feature");
        (root, paths, id)
    }

    fn intent() -> WorktreeIntent {
        WorktreeIntent {
            slug: "design-md-guidance".to_string(),
            branch: "codex/design-md-guidance-impl".to_string(),
            path: ".maestro/worktree/design-md-guidance".to_string(),
            base: "bd3d6200".to_string(),
            owner_checkout: Some("/repo/main".to_string()),
            expected_worker_checkout: None,
        }
    }

    #[test]
    fn ledger_round_trips_through_feature_sidecar() {
        let (root, paths, id) = test_repo("round-trip");
        let mut lane = WorktreeLane::new(intent());
        lane.milestones.branch_reserved_at = Some("2026-06-29T00:00:00Z".to_string());
        lane.cleanup_receipts.push(WorktreeCleanupReceipt {
            removed_path: ".maestro/worktree/design-md-guidance".to_string(),
            deleted_branch: "codex/design-md-guidance-impl".to_string(),
            pruned_stale_metadata: true,
            recorded_by: "codex".to_string(),
            recorded_at: "2026-06-29T01:00:00Z".to_string(),
        });
        let mut ledger = WorktreeLedger::default();
        ledger.upsert_lane(lane);

        save(&paths, &id, &ledger).expect("save ledger");
        let loaded = load(&paths, &id)
            .expect("load ledger")
            .expect("ledger exists");
        assert_eq!(loaded, ledger);
        assert!(
            registry::feature_sidecar_dir(&paths, &id)
                .join(WORKTREE_LEDGER_FILE)
                .is_file()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn computed_state_reports_branch_reserved_path_missing() {
        let mut lane = WorktreeLane::new(intent());
        lane.milestones.branch_reserved_at = Some("2026-06-29T00:00:00Z".to_string());

        let state = lane.computed_state(&WorktreeEvidence {
            branch_exists: true,
            path_exists: false,
            ..WorktreeEvidence::default()
        });

        assert_eq!(state, WorktreeComputedState::BranchReservedPathMissing);
        assert_eq!(state.as_str(), "branch_reserved_path_missing");
    }

    #[test]
    fn cleanup_due_requires_merge_verification_and_clear_guards() {
        let mut lane = WorktreeLane::new(intent());
        lane.milestones.merged_back_at = Some("2026-06-29T02:00:00Z".to_string());
        lane.milestones.verified_at = Some("2026-06-29T03:00:00Z".to_string());

        let eligible = WorktreeEvidence {
            worker_clean_or_absent: true,
            ..WorktreeEvidence::default()
        };
        assert_eq!(
            lane.computed_state(&eligible),
            WorktreeComputedState::CleanupDue
        );

        let active_owner = WorktreeEvidence {
            worker_clean_or_absent: true,
            active_owner: true,
            ..WorktreeEvidence::default()
        };
        assert_eq!(
            lane.computed_state(&active_owner),
            WorktreeComputedState::Unplanned
        );

        lane.milestones.cleanup_completed_at = Some("2026-06-29T04:00:00Z".to_string());
        assert_eq!(
            lane.computed_state(&eligible),
            WorktreeComputedState::CleanupComplete
        );
    }
}
