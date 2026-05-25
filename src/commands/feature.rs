use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

use crate::commands::{FeatureArgs, FeatureCommand};
use crate::core::fs::read_to_string_if_exists;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::safe_write::write_string_atomic;
use crate::core::schema::FEATURE_SCHEMA_VERSION;
use crate::core::slug::slugify_ascii;
use crate::feature::query::{count_tasks_by_feature, count_tasks_for_feature};
use crate::feature::schema::{FeatureRecord, FeatureRegistry, FeatureStatus};

/// Execute `maestro feature`.
pub fn run(args: FeatureArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        FeatureCommand::New { title } => new_feature(&paths, &title),
        FeatureCommand::Show { id } => show_feature(&paths, &id),
        FeatureCommand::List => list_features(&paths),
        FeatureCommand::Edit { id } => set_status(&paths, &id, FeatureStatus::InProgress),
        FeatureCommand::Ship { id } => set_status(&paths, &id, FeatureStatus::Shipped),
        FeatureCommand::Cancel { id } => set_status(&paths, &id, FeatureStatus::Cancelled),
    }
}

fn new_feature(paths: &MaestroPaths, title: &str) -> Result<()> {
    let mut registry = load_registry(paths)?;
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
    println!("created feature {id}");
    Ok(())
}

fn show_feature(paths: &MaestroPaths, id: &str) -> Result<()> {
    let registry = load_registry(paths)?;
    let feature = find_feature(&registry, id)?;
    let counts = count_tasks_for_feature(&paths.tasks_dir(), &feature.id)?;

    println!("id: {}", feature.id);
    println!("title: {}", feature.title);
    println!("status: {}", status_label(&feature.status));
    println!("tasks_total: {}", counts.total);
    println!("tasks_verified: {}", counts.verified);
    println!("created_at: {}", feature.created_at);
    println!("updated_at: {}", feature.updated_at);
    if let Some(description) = feature.description.as_deref() {
        println!("description: {description}");
    }

    Ok(())
}

fn list_features(paths: &MaestroPaths) -> Result<()> {
    let registry = load_registry(paths)?;
    if registry.features.is_empty() {
        println!("no features found");
        return Ok(());
    }

    let counts_by_feature = count_tasks_by_feature(&paths.tasks_dir())?;
    for feature in &registry.features {
        let counts = counts_by_feature
            .get(&feature.id)
            .cloned()
            .unwrap_or_default();
        println!(
            "{}\t{}\ttasks={}\tverified={}\t{}",
            feature.id,
            status_label(&feature.status),
            counts.total,
            counts.verified,
            feature.title
        );
    }

    Ok(())
}

fn set_status(paths: &MaestroPaths, id: &str, status: FeatureStatus) -> Result<()> {
    let mut registry = load_registry(paths)?;
    let feature = registry
        .features
        .iter_mut()
        .find(|feature| feature.id == id)
        .with_context(|| format!("feature {id} not found"))?;
    feature.status = status.clone();
    feature.updated_at = timestamp();
    save_registry(paths, &registry)?;
    println!("feature {id} status set to {}", status_label(&status));
    Ok(())
}

fn load_registry(paths: &MaestroPaths) -> Result<FeatureRegistry> {
    let path = paths.features_dir().join("features.yaml");
    let Some(contents) = read_to_string_if_exists(&path)? else {
        return Ok(FeatureRegistry::empty());
    };

    let registry: FeatureRegistry = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if registry.schema_version != FEATURE_SCHEMA_VERSION {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            FEATURE_SCHEMA_VERSION,
            registry.schema_version
        );
    }
    Ok(registry)
}

fn save_registry(paths: &MaestroPaths, registry: &FeatureRegistry) -> Result<()> {
    let path = paths.features_dir().join("features.yaml");
    let contents =
        serde_yaml::to_string(registry).context("failed to serialize feature registry")?;
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn find_feature<'a>(registry: &'a FeatureRegistry, id: &str) -> Result<&'a FeatureRecord> {
    registry
        .features
        .iter()
        .find(|feature| feature.id == id)
        .with_context(|| format!("feature {id} not found"))
}

fn status_label(status: &FeatureStatus) -> &'static str {
    match status {
        FeatureStatus::Proposed => "proposed",
        FeatureStatus::InProgress => "in_progress",
        FeatureStatus::Shipped => "shipped",
        FeatureStatus::Cancelled => "cancelled",
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
