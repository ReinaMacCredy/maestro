//! Build a card from a legacy source record's YAML mapping (SPEC-beads-model P1
//! cutover). The migration reads the mapping off disk; the live save path
//! serializes a typed record to the same mapping. Both feed these builders, so a
//! migrated card and a freshly-saved card are byte-identical -- this single
//! source is the point of the COPY design (`extra` = the verbatim source
//! mapping, the identity fields above it are derived copies). The caller resolves
//! the stable `id` (the feature dir name when a record omits it); everything else
//! is read off the mapping.

use serde_yaml::{Mapping, Value};

use crate::domain::card::schema::{Card, CardType};
use crate::foundation::core::schema::CARD_SCHEMA_VERSION;

/// Build a feature card. A feature is a container, so `parent` is always `None`.
pub fn feature_card(id: String, source: Mapping, now: &str) -> Card {
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Feature,
        title: title_or_id(&source, &id),
        status: string_field(&source, "status").unwrap_or_default(),
        parent: None,
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        created_at: created_at_or(&source, "created_at", now),
        updated_at: updated_at_or(&source, "updated_at", "created_at", now),
        description: None,
        id,
        extra: source,
    }
}

/// Build a task card. The task->feature link lives only in the directory path
/// (`TaskRecord.feature_id` is never serialized), so the migration passes the
/// feature parent it read from the dir; the field fallback covers the live save
/// path. Task lifecycle lives under `state`, not `status`; the word is kept
/// verbatim.
pub fn task_card(id: String, source: Mapping, parent: Option<String>, now: &str) -> Card {
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Task,
        title: title_or_id(&source, &id),
        status: string_field(&source, "state").unwrap_or_default(),
        parent: parent.or_else(|| string_field(&source, "feature_id")),
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        created_at: created_at_or(&source, "created_at", now),
        updated_at: updated_at_or(&source, "updated_at", "created_at", now),
        description: None,
        id,
        extra: source,
    }
}

/// Build a decision card. The parent is the explicit `feature` field, falling
/// back to the per-feature store dir the migration read it from. `DecisionRecord`
/// carries no `updated_at`, so lean on `locked_at`, then `created_at`.
pub fn decision_card(
    id: String,
    source: Mapping,
    feature_parent: Option<String>,
    now: &str,
) -> Card {
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Decision,
        title: title_or_id(&source, &id),
        status: string_field(&source, "status").unwrap_or_default(),
        parent: string_field(&source, "feature").or(feature_parent),
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        created_at: created_at_or(&source, "created_at", now),
        updated_at: updated_at_or(&source, "locked_at", "created_at", now),
        description: None,
        id,
        extra: source,
    }
}

/// Build an idea card from a harness backlog item. Every harness item maps to
/// `idea`; its detector category stays in `extra.type`, it is not the card type
/// (SPEC keep-ids reconciliation). `first_seen`/`last_seen` are skip-if-empty, so
/// fall back rather than store `""`.
pub fn idea_card(id: String, source: Mapping, now: &str) -> Card {
    let created_at = nonempty_field(&source, "first_seen")
        .or_else(|| nonempty_field(&source, "last_seen"))
        .unwrap_or_else(|| now.to_string());
    let updated_at = nonempty_field(&source, "last_seen").unwrap_or_else(|| created_at.clone());
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Idea,
        title: title_or_id(&source, &id),
        status: string_field(&source, "status").unwrap_or_default(),
        parent: None,
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        created_at,
        updated_at,
        description: None,
        id,
        extra: source,
    }
}

pub(crate) fn title_or_id(record: &Mapping, id: &str) -> String {
    string_field(record, "title").unwrap_or_else(|| id.to_string())
}

pub(crate) fn created_at_or(record: &Mapping, key: &str, now: &str) -> String {
    string_field(record, key).unwrap_or_else(|| now.to_string())
}

pub(crate) fn updated_at_or(record: &Mapping, key: &str, fallback_key: &str, now: &str) -> String {
    string_field(record, key).unwrap_or_else(|| created_at_or(record, fallback_key, now))
}

pub(crate) fn string_field(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn nonempty_field(map: &Mapping, key: &str) -> Option<String> {
    string_field(map, key).filter(|value| !value.is_empty())
}
