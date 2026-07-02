use serde::{Deserialize, Serialize};

use crate::foundation::core::schema::DECISIONS_SCHEMA_VERSION;

/// Structured decision store written at `.maestro/decisions.yaml` or
/// `.maestro/features/<id>/decisions.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecisionStore {
    pub schema_version: String,
    #[serde(default)]
    pub decisions: Vec<DecisionRecord>,
}

impl DecisionStore {
    pub fn empty() -> Self {
        Self {
            schema_version: DECISIONS_SCHEMA_VERSION.to_string(),
            decisions: Vec::new(),
        }
    }
}

/// One structured design fork or locked decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecisionRecord {
    pub id: String,
    pub title: String,
    pub status: DecisionStatus,
    #[serde(default, skip_serializing_if = "DecisionRecordKind::is_individual")]
    pub kind: DecisionRecordKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_set_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_set_children: Vec<DecisionSetChildSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_approval: Option<DecisionSetText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisor_review: Option<DecisionSetText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_set_schema_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_override: Option<SummaryDecisionOverride>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionRecordKind {
    #[default]
    Individual,
    DecisionSet,
}

impl DecisionRecordKind {
    pub fn is_individual(&self) -> bool {
        matches!(self, Self::Individual)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Individual => "individual",
            Self::DecisionSet => "decision_set",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecisionSetChildSummary {
    pub key: String,
    pub title: String,
    pub order: u32,
    pub preview: Option<String>,
    pub child_decision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecisionSetText {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SummaryDecisionOverride {
    pub accepted_at: String,
    pub reason: String,
    pub signals: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Open,
    Locked,
    Superseded,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Locked => "locked",
            Self::Superseded => "superseded",
        }
    }
}
