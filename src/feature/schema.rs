use serde::{Deserialize, Serialize};

use crate::core::schema::FEATURE_SCHEMA_VERSION;

/// `.maestro/features/features.yaml` registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureRegistry {
    /// Feature registry schema version.
    pub schema_version: String,
    /// Human-authored feature records.
    pub features: Vec<FeatureRecord>,
}

/// V1 feature record. Task counts are intentionally not stored here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureRecord {
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
}

/// V1 feature lifecycle status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    /// Proposed but not accepted into active work.
    Proposed,
    /// Active implementation work is in progress.
    InProgress,
    /// Feature has shipped.
    Shipped,
    /// Feature was cancelled.
    Cancelled,
}

impl FeatureRegistry {
    /// Return an empty V1 feature registry.
    pub fn empty() -> Self {
        Self {
            schema_version: FEATURE_SCHEMA_VERSION.to_string(),
            features: Vec::new(),
        }
    }
}
