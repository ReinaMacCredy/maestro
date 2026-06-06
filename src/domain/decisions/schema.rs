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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
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
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
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
