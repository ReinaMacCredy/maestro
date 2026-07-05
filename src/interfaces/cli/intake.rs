use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::domain::intake::{self, SourceProvenance};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{IntakeArgs, shell_word};

const MAX_INTAKE_SOURCE_BYTES: usize = 1_048_576;

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
        let mut bytes = Vec::new();
        io::stdin()
            .take((MAX_INTAKE_SOURCE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .context("failed to read intake source from stdin")?;
        let raw = decode_source(bytes, "stdin")?;
        let provenance = SourceProvenance::stdin(&raw);
        return Ok((raw, provenance));
    }

    let path = PathBuf::from(from);
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("failed to inspect intake source {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("refusing symlinked intake source {}", path.display());
    }
    if !metadata.is_file() {
        bail!("intake source {} is not a regular file", path.display());
    }
    if metadata.len() > MAX_INTAKE_SOURCE_BYTES as u64 {
        bail!(
            "intake source {} is too large; max {} bytes",
            path.display(),
            MAX_INTAKE_SOURCE_BYTES
        );
    }
    let raw = fs::read(&path)
        .with_context(|| format!("failed to read intake source {}", path.display()))?;
    let raw = decode_source(raw, &path.display().to_string())?;
    let display = path.display().to_string();
    let provenance = SourceProvenance::file(display, &raw);
    Ok((raw, provenance))
}

fn decode_source(bytes: Vec<u8>, label: &str) -> Result<String> {
    if bytes.len() > MAX_INTAKE_SOURCE_BYTES {
        bail!(
            "intake source {label} is too large; max {} bytes",
            MAX_INTAKE_SOURCE_BYTES
        );
    }
    if bytes.contains(&0) {
        bail!("refusing binary intake source {label}");
    }
    String::from_utf8(bytes).with_context(|| format!("intake source {label} is not valid UTF-8"))
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
