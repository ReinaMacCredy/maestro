use serde::{Deserialize, Serialize};

use crate::foundation::core::schema::CARD_SCHEMA_VERSION;

/// One card stored at `.maestro/cards/<id>/card.yaml`.
///
/// A card is the single typed entity the model folds features, tasks,
/// harness-backlog items, and decisions into (SPEC-beads-model.md). Each card
/// owns a directory keyed by its stable id; feature cards carry `spec.md` /
/// `notes.md` as sidecar prose, never inlined here.
///
/// Slice 1 (P1) is the additive data container plus its CAS-backed store:
/// per-type lifecycle rules, the coarse `open|in_progress|closed` derivation,
/// the computed display alias, the `ready`/claim semantics, and the migration
/// all land in later phases. The `status` is stored as a free string because
/// the per-type status vocabulary is still open (SPEC O5); only the LOCKED
/// fields are typed here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Card {
    /// Card schema version.
    pub schema_version: String,
    /// Stable, opaque id assigned once and never rewritten (SPEC E2). Feature
    /// cards use their immutable creation slug; other cards use `card-<hash>`.
    pub id: String,
    /// Card type (SPEC DN2, LOCKED).
    #[serde(rename = "type")]
    pub card_type: CardType,
    /// Human-readable title.
    pub title: String,
    /// Real per-type status string; the coarse status is derived, not stored.
    pub status: String,
    /// Stable id of the parent card, or `None` for a standalone card. Always a
    /// field, never derived from the id (SPEC E2). Non-blocking (SPEC E1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Dependency edges to other cards (SPEC E1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<Dep>,
    /// Optional execution lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    /// Current claimant as `<agent>#<session>` (SPEC E6), or `None` when free.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    /// Timestamp the claim was stamped, used by the stale-claim TTL (SPEC E6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    /// Creation timestamp string.
    pub created_at: String,
    /// Last update timestamp string.
    pub updated_at: String,
    /// Optional longer description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Card {
    /// Build a card with the current schema version and no edges or claim.
    pub fn new(id: &str, card_type: CardType, title: &str, status: &str, now: &str) -> Self {
        Self {
            schema_version: CARD_SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            card_type,
            title: title.to_string(),
            status: status.to_string(),
            parent: None,
            deps: Vec::new(),
            lane: None,
            claimed_by: None,
            claimed_at: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            description: None,
        }
    }
}

/// The card type set (SPEC DN2, LOCKED). Two levels: `feature` containers and
/// the work/idea/decision leaves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Feature,
    Task,
    Bug,
    Chore,
    Idea,
    Decision,
}

impl CardType {
    /// Canonical snake_case label, identical to the serde wire form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Task => "task",
            Self::Bug => "bug",
            Self::Chore => "chore",
            Self::Idea => "idea",
            Self::Decision => "decision",
        }
    }
}

/// One dependency edge from this card to another (SPEC E1). `parent` is a
/// separate field, not an edge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Dep {
    /// Edge kind.
    pub kind: DepKind,
    /// Stable id of the target card.
    pub target: String,
}

/// Dependency edge kinds (SPEC E1, LOCKED). Only `blocks` is blocking;
/// `related` and `supersedes` are non-blocking annotations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DepKind {
    Blocks,
    Related,
    Supersedes,
}

impl DepKind {
    /// Whether this edge gates `ready` (only `blocks` is blocking).
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Blocks)
    }

    /// Canonical snake_case label, identical to the serde wire form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Related => "related",
            Self::Supersedes => "supersedes",
        }
    }
}
