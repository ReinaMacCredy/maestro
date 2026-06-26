//! Build a card from a source record's YAML mapping. The migration reads the
//! mapping off disk; the live save path
//! serializes a typed record to the same mapping. Both feed these builders, so a
//! migrated card and a freshly-saved card share one derivation path. `extra`
//! keeps only the type-specific payload while the envelope stores shared identity
//! and display fields. The caller resolves the stable `id` (the feature dir name
//! when a record omits it); everything else is read off the mapping.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_yaml::{Mapping, Value};

use crate::domain::card::schema::{Card, CardType, Dep, DepKind};
use crate::domain::schema_contracts::{VersionClass, pack};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::schema::CARD_SCHEMA_VERSION;

/// Build a feature card. A feature is a container, so `parent` is always `None`.
pub fn feature_card(id: String, source: Mapping, now: &str) -> Card {
    let title = title_or_id(&source, &id);
    let status = string_field(&source, "status").unwrap_or_default();
    let created_at = created_at_or(&source, "created_at", now);
    let updated_at = updated_at_or(&source, "updated_at", "created_at", now);
    let description = string_field(&source, "description");
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Feature,
        title,
        status,
        parent: None,
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        suggested_for: None,
        created_at,
        updated_at,
        description,
        id,
        active_form: None,
        project: None,
        extra: without_envelope_fields(
            source,
            &[
                "id",
                "title",
                "status",
                "created_at",
                "updated_at",
                "description",
            ],
        ),
        unknown: Mapping::new(),
    }
}

/// Build a task card. The task->feature link lives only in the directory path
/// (`TaskRecord.feature_id` is never serialized), so the migration passes the
/// feature parent it read from the dir; the field fallback covers the live save
/// path. Task lifecycle lives under `state`, not `status`; the word is kept
/// verbatim.
pub fn task_card(id: String, source: Mapping, parent: Option<String>, now: &str) -> Card {
    let title = title_or_id(&source, &id);
    let status = string_field(&source, "state").unwrap_or_default();
    let parent = parent.or_else(|| string_field(&source, "feature_id"));
    let deps = blocker_deps(&source);
    let claimed_by = string_field(&source, "claimed_by");
    let claimed_at = string_field(&source, "claimed_at");
    let created_at = created_at_or(&source, "created_at", now);
    let updated_at = updated_at_or(&source, "updated_at", "created_at", now);
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Task,
        title,
        status,
        parent,
        deps,
        lane: None,
        claimed_by,
        claimed_at,
        suggested_for: None,
        created_at,
        updated_at,
        description: None,
        id,
        active_form: None,
        project: None,
        extra: without_envelope_fields(
            source,
            &[
                "id",
                "title",
                "state",
                "created_at",
                "updated_at",
                "claimed_by",
                "claimed_at",
            ],
        ),
        unknown: Mapping::new(),
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
    let title = title_or_id(&source, &id);
    let status = string_field(&source, "status").unwrap_or_default();
    let parent = string_field(&source, "feature").or(feature_parent);
    let created_at = created_at_or(&source, "created_at", now);
    let updated_at = updated_at_or(&source, "locked_at", "created_at", now);
    let description = string_field(&source, "context");
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Decision,
        title,
        status,
        parent,
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        suggested_for: None,
        created_at,
        updated_at,
        description,
        id,
        active_form: None,
        project: None,
        extra: without_envelope_fields(
            source,
            &["id", "title", "status", "feature", "context", "created_at"],
        ),
        unknown: Mapping::new(),
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
    let title = title_or_id(&source, &id);
    let status = string_field(&source, "status").unwrap_or_default();
    Card {
        schema_version: CARD_SCHEMA_VERSION.to_string(),
        card_type: CardType::Idea,
        title,
        status,
        parent: None,
        deps: Vec::new(),
        lane: None,
        claimed_by: None,
        claimed_at: None,
        suggested_for: None,
        created_at,
        updated_at,
        description: None,
        id,
        active_form: None,
        project: None,
        extra: without_envelope_fields(source, &["id", "title", "status"]),
        unknown: Mapping::new(),
    }
}

fn without_envelope_fields(mut source: Mapping, fields: &[&str]) -> Mapping {
    for field in fields {
        source.remove(Value::String((*field).to_string()));
    }
    source
}

pub(crate) fn seed_string_if_absent(map: &mut Mapping, key: &str, value: &str) {
    let key = Value::String(key.to_string());
    if !map.contains_key(&key) {
        map.insert(key, Value::String(value.to_string()));
    }
}

pub(crate) fn seed_optional_string_if_absent(map: &mut Mapping, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        seed_string_if_absent(map, key, value);
    }
}

pub(crate) fn record_from_extra<T>(extra: Mapping, artifact: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_yaml::from_value(Value::Mapping(extra))
        .with_context(|| format!("failed to parse {artifact}"))
}

pub(crate) fn record_to_mapping<T>(record: &T, label: &str) -> Result<Mapping>
where
    T: Serialize,
{
    match serde_yaml::to_value(record).with_context(|| format!("failed to serialize {label}"))? {
        Value::Mapping(map) => Ok(map),
        _ => bail!("{label} did not serialize to a mapping"),
    }
}

/// Gate a card payload's raw `schema_version` against its family's schema
/// pack BEFORE the typed parse: a below-floor payload must refuse with the
/// pack's migrate route, not die as a serde error on a missing v2 field. An
/// absent or non-string stamp falls through so serde keeps today's parse
/// error.
pub(crate) fn ensure_supported_schema(extra: &Mapping, artifact: &str, family: &str) -> Result<()> {
    let Some(found) = string_field(extra, "schema_version") else {
        return Ok(());
    };
    let pack = pack(family)
        .unwrap_or_else(|| panic!("invariant: schema pack {family} ships with the binary"));
    let route = match pack.classify(&found) {
        VersionClass::Supported => return Ok(()),
        VersionClass::Legacy { route } => Some(route.to_string()),
        VersionClass::Unknown => None,
    };
    Err(MaestroError::UnsupportedSchemaVersion {
        artifact: artifact.to_string(),
        found,
        read: pack.supported.read.join(", "),
        route,
    }
    .into())
}

/// The declared field set of the schema-pack family describing a card type's
/// `extra` payload: the boundary between an intentionally-cleared known field
/// and a foreign key D6.6 tolerance must carry. Bug/chore cards ride the task
/// record like everywhere else (`domain/task/mod.rs` groups them with `Task`).
pub(crate) fn payload_pack_fields(
    card_type: CardType,
) -> Option<std::collections::BTreeSet<&'static str>> {
    let family = match card_type {
        CardType::Feature => "feature",
        CardType::Custom => return None,
        CardType::Progress => return None,
        CardType::Task | CardType::Bug | CardType::Chore => "task",
        CardType::Idea => "backlog",
        CardType::Decision => "decision",
    };
    let pack = pack(family)?;
    Some(
        pack.current
            .contracts
            .iter()
            .flat_map(|contract| contract.fields.iter().map(String::as_str))
            .collect(),
    )
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

/// Blocking edges derived from the source mapping's unresolved blockers: every
/// open blocker with a `blocked_ref` gates readiness as a `blocks` dep on that
/// in-store id (task and decision refs alike -- `ready` requires a closed
/// target either way). External/Human blockers carry no ref and gate through
/// the task lifecycle, not the dep graph. Without this derive, a blocked task
/// reads as ready: `ready` consults only `card.deps`, and the blocker list
/// lives in the `extra` carrier it never opens.
fn blocker_deps(source: &Mapping) -> Vec<Dep> {
    let Some(Value::Sequence(blockers)) = source.get(Value::String("blockers".to_string())) else {
        return Vec::new();
    };
    let mut deps: Vec<Dep> = Vec::new();
    for blocker in blockers.iter().filter_map(Value::as_mapping) {
        let resolved = blocker
            .get(Value::String("resolved_at".to_string()))
            .is_some_and(|value| !value.is_null());
        if resolved {
            continue;
        }
        let Some(target) = blocker
            .get(Value::String("blocked_ref".to_string()))
            .and_then(Value::as_mapping)
            .and_then(|reference| string_field(reference, "id"))
        else {
            continue;
        };
        if !deps.iter().any(|dep| dep.target == target) {
            deps.push(Dep {
                kind: DepKind::Blocks,
                target,
            });
        }
    }
    deps
}

pub(crate) fn nonempty_field(map: &Mapping, key: &str) -> Option<String> {
    string_field(map, key).filter(|value| !value.is_empty())
}
