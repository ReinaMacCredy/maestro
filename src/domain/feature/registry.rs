//! Feature-registry operation surface.
//!
//! Concentrates the `features.yaml` layout, parse policy, and create/mutate
//! invariants that were previously duplicated across the four interface
//! readers. Reads come in two flavours that share one classification:
//!
//! - the **strict** read ([`list`], [`show`], and the create/mutate ops) errors
//!   on a malformed or schema-incompatible registry, because those paths are
//!   authoritative,
//! - the **tolerant** read ([`titles`]) degrades to empty so a live display such
//!   as the TUI never hard-fails on a bad registry.
//!
//! [`diagnose`] reports the found-vs-expected schema verdict as data (never an
//! error) for `maestro doctor`.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

use crate::domain::feature::query::{
    count_tasks_by_feature, count_tasks_for_feature, FeatureTaskCounts,
};
use crate::domain::feature::schema::{FeatureRecord, FeatureRegistry, FeatureStatus};
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{classify, Compat, FEATURE_SCHEMA_VERSION};
use crate::foundation::core::slug::slugify_ascii;

/// A feature joined with its non-persisted task counts, ready for display.
///
/// The counts are computed on demand from `.maestro/tasks/**/task.yaml`,
/// preserving the registry invariant that task counts are not stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureView {
    /// Stable feature id.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Feature lifecycle status.
    pub status: FeatureStatus,
    /// Tasks that reference this feature, computed on read.
    pub counts: FeatureTaskCounts,
    /// Creation timestamp string.
    pub created_at: String,
    /// Last update timestamp string.
    pub updated_at: String,
    /// Optional feature description.
    pub description: Option<String>,
}

/// Found-vs-expected schema diagnostic for the feature registry, reported as
/// data for `maestro doctor` rather than as an error.
///
/// The single [`classify`] call for the registry lives in [`diagnose`]; callers
/// read [`FeatureDiagnostic::compatibility`] rather than re-classifying.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureDiagnostic {
    /// The schema version this binary expects for the registry.
    pub expected: &'static str,
    /// `Ok((schema_version, feature_count))` when the registry parses; `Err`
    /// carries a diagnostic message when the registry is absent or unparseable.
    pub found: Result<(String, usize), String>,
    /// Compatibility verdict of [`FeatureDiagnostic::found`] against
    /// [`FeatureDiagnostic::expected`], or `None` when the registry failed to
    /// parse.
    pub compatibility: Option<Compat>,
}

/// Create a feature from a title, generating a slug id and persisting it.
///
/// # Errors
///
/// Errors when the title has no ASCII slug content, when a feature with the
/// generated id already exists, or when the registry cannot be read or written.
pub fn create(paths: &MaestroPaths, title: &str) -> Result<String> {
    let mut registry = load_registry_strict(paths)?;
    let id = slugify_ascii(title);
    if id.is_empty() {
        bail!("feature title must contain at least one ASCII letter or digit");
    }
    if registry.features.iter().any(|feature| feature.id == id) {
        bail!("feature {id} already exists");
    }

    let now = timestamp();
    registry.features.push(FeatureRecord {
        id: id.clone(),
        title: title.to_string(),
        description: None,
        status: FeatureStatus::Proposed,
        created_at: now.clone(),
        updated_at: now,
        raw_request: None,
        input_type: None,
        affected_areas: Vec::new(),
        open_questions: Vec::new(),
        acceptance: Vec::new(),
        non_goals: Vec::new(),
    });
    save_registry(paths, &registry)?;
    Ok(id)
}

/// Set a feature's status, bumping its `updated_at` timestamp and persisting.
///
/// # Errors
///
/// Errors when no feature has the given id, or when the registry cannot be read
/// or written.
pub fn set_status(paths: &MaestroPaths, id: &str, status: FeatureStatus) -> Result<()> {
    let mut registry = load_registry_strict(paths)?;
    let feature = registry
        .features
        .iter_mut()
        .find(|feature| feature.id == id)
        .with_context(|| format!("feature {id} not found"))?;
    feature.status = status;
    feature.updated_at = timestamp();
    save_registry(paths, &registry)
}

/// List every feature joined with its on-demand task counts.
///
/// # Errors
///
/// Errors when the registry cannot be read or is schema-incompatible.
pub fn list(paths: &MaestroPaths) -> Result<Vec<FeatureView>> {
    let registry = load_registry_strict(paths)?;
    let counts_by_feature = count_tasks_by_feature(&paths.tasks_dir())?;
    Ok(registry
        .features
        .into_iter()
        .map(|feature| {
            let counts = counts_by_feature
                .get(&feature.id)
                .cloned()
                .unwrap_or_default();
            view_from_record(feature, counts)
        })
        .collect())
}

/// Show one feature joined with its on-demand task counts.
///
/// # Errors
///
/// Errors when the registry cannot be read, is schema-incompatible, or has no
/// feature with the given id.
pub fn show(paths: &MaestroPaths, id: &str) -> Result<FeatureView> {
    let registry = load_registry_strict(paths)?;
    let feature = registry
        .features
        .into_iter()
        .find(|feature| feature.id == id)
        .with_context(|| format!("feature {id} not found"))?;
    let counts = count_tasks_for_feature(&paths.tasks_dir(), &feature.id)?;
    Ok(view_from_record(feature, counts))
}

/// Scan-free id -> title map for display.
///
/// This is the one documented tolerant read: it degrades to an empty map for a
/// missing, unparseable, or schema-incompatible registry so live display paths
/// never hard-fail. Display only; never an authority for a gate or write.
pub fn titles(paths: &MaestroPaths) -> BTreeMap<String, String> {
    let Ok(Some(registry)) = read_registry_if_compatible(paths) else {
        return BTreeMap::new();
    };
    registry
        .features
        .into_iter()
        .map(|feature| (feature.id, feature.title))
        .collect()
}

/// Report the registry's found-vs-expected schema verdict as data.
///
/// Never errors: an absent or unparseable registry is carried in
/// [`FeatureDiagnostic::found`] as `Err`. Reporting absence as an error keeps
/// `maestro doctor` consistent with its harness and backlog checks, which flag
/// a missing config file rather than treating it as the empty default.
pub fn diagnose(paths: &MaestroPaths) -> FeatureDiagnostic {
    let path = registry_path(paths);
    let found = match read_registry_raw(paths) {
        Ok(Some(registry)) => Ok((registry.schema_version, registry.features.len())),
        Ok(None) => Err(format!("{} is missing", path.display())),
        Err(error) => Err(error.to_string()),
    };
    let compatibility = found
        .as_ref()
        .ok()
        .map(|(version, _)| classify(version, FEATURE_SCHEMA_VERSION));
    FeatureDiagnostic {
        expected: FEATURE_SCHEMA_VERSION,
        found,
        compatibility,
    }
}

/// Render a [`FeatureStatus`] to its canonical snake_case label.
pub fn status_label(status: &FeatureStatus) -> &'static str {
    match status {
        FeatureStatus::Proposed => "proposed",
        FeatureStatus::InProgress => "in_progress",
        FeatureStatus::Shipped => "shipped",
        FeatureStatus::Cancelled => "cancelled",
    }
}

fn view_from_record(feature: FeatureRecord, counts: FeatureTaskCounts) -> FeatureView {
    FeatureView {
        id: feature.id,
        title: feature.title,
        status: feature.status,
        counts,
        created_at: feature.created_at,
        updated_at: feature.updated_at,
        description: feature.description,
    }
}

/// Strict registry read: an absent registry is the empty registry; a present
/// registry must parse and be schema-`Exact`, otherwise it errors.
fn load_registry_strict(paths: &MaestroPaths) -> Result<FeatureRegistry> {
    let Some(registry) = read_registry_raw(paths)? else {
        return Ok(FeatureRegistry::empty());
    };
    if classify(&registry.schema_version, FEATURE_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            registry_path(paths).display(),
            FEATURE_SCHEMA_VERSION,
            registry.schema_version
        );
    }
    Ok(registry)
}

/// Read the registry only when it is present and schema-`Exact`; `Ok(None)` for
/// an absent or incompatible registry, `Err` for a parse failure.
fn read_registry_if_compatible(paths: &MaestroPaths) -> Result<Option<FeatureRegistry>> {
    let Some(registry) = read_registry_raw(paths)? else {
        return Ok(None);
    };
    if classify(&registry.schema_version, FEATURE_SCHEMA_VERSION) != Compat::Exact {
        return Ok(None);
    }
    Ok(Some(registry))
}

/// Parse the registry without any schema-compatibility check; `Ok(None)` when
/// the file is absent, `Err` on a parse failure.
fn read_registry_raw(paths: &MaestroPaths) -> Result<Option<FeatureRegistry>> {
    let path = registry_path(paths);
    let Some(contents) = read_to_string_if_exists(&path)? else {
        return Ok(None);
    };
    let registry: FeatureRegistry = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(registry))
}

fn save_registry(paths: &MaestroPaths, registry: &FeatureRegistry) -> Result<()> {
    let path = registry_path(paths);
    let contents =
        serde_yaml::to_string(registry).context("failed to serialize feature registry")?;
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn registry_path(paths: &MaestroPaths) -> std::path::PathBuf {
    paths.features_dir().join("features.yaml")
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
