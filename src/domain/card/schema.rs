use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::foundation::core::schema::CARD_SCHEMA_VERSION;

/// One card stored at `.maestro/cards/<id>/card.yaml`.
///
/// A card is the single typed entity the model folds features, tasks,
/// harness-backlog items, and decisions into (SPEC-beads-model.md). Each card
/// owns a directory keyed by its stable id; feature cards carry `spec.md` /
/// `notes.md` as sidecar prose, never inlined here.
///
/// The envelope owns the shared identity, display, relationship, and claim
/// fields. The `status` is stored as a free string because each card type owns
/// its fine-grained lifecycle; board-level status is derived from it.
///
/// `Eq` is intentionally absent: the `extra` carrier holds a
/// [`serde_yaml::Mapping`], whose `Value` may contain floats, so only
/// `PartialEq` is derivable.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    /// Carrier for type-specific payload that has not yet moved into first-class
    /// card fields. Shared identity/display fields live in the card envelope
    /// above; typed readers seed those fields back into this map before
    /// deserializing records. Empty for cards minted natively by the card model.
    #[serde(default, skip_serializing_if = "serde_yaml::Mapping::is_empty")]
    pub extra: serde_yaml::Mapping,
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
            extra: serde_yaml::Mapping::new(),
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

    /// Parse the canonical snake_case label back to a type, mirroring
    /// `Coarse::parse` so the CLI `--type` filter parses without leaking a clap
    /// dependency into the domain enum.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "feature" => Some(Self::Feature),
            "task" => Some(Self::Task),
            "bug" => Some(Self::Bug),
            "chore" => Some(Self::Chore),
            "idea" => Some(Self::Idea),
            "decision" => Some(Self::Decision),
            _ => None,
        }
    }

    /// Whether cards of this type are "worked" through the type-agnostic verbs:
    /// they enter `ready`, are claimable, and close via the work lifecycle (SPEC
    /// E3, LOCKED = task/bug/chore). Feature/idea/decision keep their own verbs
    /// and never appear in `ready`.
    pub fn workable(&self) -> bool {
        matches!(self, Self::Task | Self::Bug | Self::Chore)
    }

    /// Merge a re-detected `incoming` card into its `existing` counterpart when
    /// a store refresh re-encounters the same identity (SPEC E7). Only `idea`
    /// cards carry merge semantics -- the fingerprint-keyed recurrence,
    /// regression-reopen, and evidence accumulation a detector run feeds the
    /// harness backlog; every other type replaces wholesale (incoming wins).
    pub fn reconcile(&self, existing: Card, incoming: Card) -> Result<Card> {
        match self {
            // Deliberate intra-domain call: the idea merge semantics live with
            // the harness item schema, not with the card envelope.
            Self::Idea => crate::domain::harness::cards::reconcile_idea(existing, incoming),
            _ => Ok(incoming),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// SPEC E7 default arm: every non-idea type replaces wholesale -- the
    /// incoming card wins verbatim. The idea arm's merge semantics are covered in
    /// `domain::harness::cards`.
    #[test]
    fn reconcile_default_arm_replaces_with_the_incoming_card() {
        let existing = Card::new(
            "card-aaaaaa",
            CardType::Task,
            "old title",
            "ready",
            "2026-06-09T00:00:00Z",
        );
        let mut incoming = Card::new(
            "card-aaaaaa",
            CardType::Task,
            "new title",
            "in_progress",
            "2026-06-10T00:00:00Z",
        );
        incoming.description = Some("refreshed".to_string());

        let merged = CardType::Task
            .reconcile(existing, incoming.clone())
            .expect("non-idea reconcile is infallible");
        assert_eq!(merged, incoming);
    }

    #[test]
    fn card_type_parse_round_trips_as_str_and_rejects_unknown() {
        for ty in [
            CardType::Feature,
            CardType::Task,
            CardType::Bug,
            CardType::Chore,
            CardType::Idea,
            CardType::Decision,
        ] {
            assert_eq!(CardType::parse(ty.as_str()), Some(ty));
        }
        assert_eq!(CardType::parse("epic"), None);
    }
}
