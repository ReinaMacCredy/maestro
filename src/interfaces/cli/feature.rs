use anyhow::Result;

use crate::domain::feature;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::interfaces::cli::{FeatureArgs, FeatureCommand};

/// Execute `maestro feature`.
pub fn run(args: FeatureArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        FeatureCommand::New { title } => new_feature(&paths, &title),
        FeatureCommand::Show { id } => show_feature(&paths, &id),
        FeatureCommand::List => list_features(&paths),
        FeatureCommand::Edit { id } => set_status(&paths, &id, feature::FeatureStatus::InProgress),
        FeatureCommand::Ship { id } => set_status(&paths, &id, feature::FeatureStatus::Shipped),
        FeatureCommand::Cancel { id } => set_status(&paths, &id, feature::FeatureStatus::Cancelled),
    }
}

fn new_feature(paths: &MaestroPaths, title: &str) -> Result<()> {
    let id = feature::create(paths, title)?;
    println!("created feature {id}");
    Ok(())
}

fn show_feature(paths: &MaestroPaths, id: &str) -> Result<()> {
    let view = feature::show(paths, id)?;

    println!("id: {}", view.id);
    println!("title: {}", view.title);
    println!("status: {}", feature::status_label(&view.status));
    println!("tasks_total: {}", view.counts.total);
    println!("tasks_verified: {}", view.counts.verified);
    println!("created_at: {}", view.created_at);
    println!("updated_at: {}", view.updated_at);
    if let Some(description) = view.description.as_deref() {
        println!("description: {description}");
    }

    Ok(())
}

fn list_features(paths: &MaestroPaths) -> Result<()> {
    let views = feature::list(paths)?;
    if views.is_empty() {
        println!("no features found");
        return Ok(());
    }

    for view in &views {
        println!(
            "{}\t{}\ttasks={}\tverified={}\t{}",
            view.id,
            feature::status_label(&view.status),
            view.counts.total,
            view.counts.verified,
            view.title
        );
    }

    Ok(())
}

fn set_status(paths: &MaestroPaths, id: &str, status: feature::FeatureStatus) -> Result<()> {
    feature::set_status(paths, id, status.clone())?;
    println!(
        "feature {id} status set to {}",
        feature::status_label(&status)
    );
    Ok(())
}
