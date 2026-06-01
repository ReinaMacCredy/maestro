use serde::{Deserialize, Serialize};

use crate::foundation::core::schema::FEATURE_SCHEMA_VERSION;

/// V1 feature record stored in `.maestro/features/<id>/feature.yaml`.
///
/// Each feature owns its own directory (no flat registry); the record is the
/// source of truth for the product contract. Task counts are intentionally not
/// stored here — they are computed on read from `.maestro/tasks/`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureRecord {
    /// Feature record schema version.
    pub schema_version: String,
    /// Stable feature id.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Optional feature description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Feature status.
    pub status: FeatureStatus,
    /// Creation timestamp string.
    pub created_at: String,
    /// Last update timestamp string.
    pub updated_at: String,
    /// Optional raw request that led to this feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_request: Option<String>,
    /// Optional input type such as bug_report or refactor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    /// Optional affected areas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_areas: Vec<String>,
    /// Optional open questions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
    /// Acceptance criteria for the feature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance: Vec<String>,
    /// Explicit non-goals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_goals: Vec<String>,
    /// One-line shipped outcome, set at `ship --outcome`. Write-once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

impl FeatureRecord {
    /// Construct a freshly-proposed feature with the current schema version.
    pub fn proposed(id: &str, title: &str, now: &str) -> Self {
        Self {
            schema_version: FEATURE_SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            status: FeatureStatus::Proposed,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            raw_request: None,
            input_type: None,
            affected_areas: Vec::new(),
            open_questions: Vec::new(),
            acceptance: Vec::new(),
            non_goals: Vec::new(),
            outcome: None,
        }
    }
}

/// V1 feature lifecycle status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    /// Proposed; contract still editable via `set`.
    Proposed,
    /// Contract frozen and baseline captured; child tasks may be created, work
    /// not yet started.
    Ready,
    /// Active implementation work is in progress.
    InProgress,
    /// Feature has shipped.
    Shipped,
    /// Feature was cancelled.
    Cancelled,
}

impl FeatureStatus {
    /// Canonical snake_case label, identical to the serde wire form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::Shipped => "shipped",
            Self::Cancelled => "cancelled",
        }
    }

    /// A terminal status can no longer transition.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Shipped | Self::Cancelled)
    }
}

/// Append-only audit trail of `feature amend` calls, stored alongside the record
/// in `.maestro/features/<id>/amend-log.yaml`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AmendLog {
    /// Append-only amend entries, oldest first.
    #[serde(default)]
    pub entries: Vec<AmendEntry>,
}

/// One audited `amend` call: what was added, when, and why.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AmendEntry {
    /// Timestamp string of the amend.
    pub at: String,
    /// Operator-supplied reason (required by the verb).
    pub reason: String,
    /// The values added by this amend (post-dedup; only genuinely new values).
    pub added: AmendAdditions,
}

/// The contract values added by a single `amend` call.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct AmendAdditions {
    /// Acceptance criteria added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance: Vec<String>,
    /// Affected areas added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_areas: Vec<String>,
    /// Non-goals added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_goals: Vec<String>,
    /// Open questions added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
}

impl AmendAdditions {
    /// True when this amend added no genuinely-new values (a full-dedup no-op).
    pub fn is_empty(&self) -> bool {
        self.acceptance.is_empty()
            && self.affected_areas.is_empty()
            && self.non_goals.is_empty()
            && self.open_questions.is_empty()
    }
}
