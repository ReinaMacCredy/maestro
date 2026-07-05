use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::domain::intake::{self, SourceProvenance};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{IntakeArgs, shell_word};

pub fn run(args: IntakeArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let (raw, source_provenance) = read_source(&args.from)?;
    let report = intake::classify(&paths, &raw, source_provenance);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render_human(&report);
    }
    Ok(())
}

fn read_source(from: &str) -> Result<(String, SourceProvenance)> {
    if from == "-" {
        let mut raw = String::new();
        io::stdin()
            .read_to_string(&mut raw)
            .context("failed to read intake source from stdin")?;
        let provenance = SourceProvenance::stdin(&raw);
        return Ok((raw, provenance));
    }

    let path = PathBuf::from(from);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read intake source {}", path.display()))?;
    let display = path.display().to_string();
    let provenance = SourceProvenance::file(display, &raw);
    Ok((raw, provenance))
}

fn render_human(report: &intake::IntakeReport) {
    println!("route: {}", report.route.as_str());
    if let Some(route_hint) = &report.route_hint {
        println!("route_hint: {route_hint}");
    }
    if let Some(owner) = &report.owner {
        println!("owner: {owner}");
    }
    print!("source: {}", report.source_provenance.kind.as_str());
    if let Some(path) = &report.source_provenance.path {
        print!(" {}", shell_word(path));
    }
    println!(" bytes={}", report.source_provenance.bytes);
    if !report.missing.is_empty() {
        println!("missing:");
        for item in &report.missing {
            println!("- {item}");
        }
    }
    if !report.blocked_by.is_empty() {
        println!("blocked_by:");
        for item in &report.blocked_by {
            println!("- {item}");
        }
    }
    println!("writes_allowed: {}", report.writes_allowed);
    println!("next: {}", report.next);
}
