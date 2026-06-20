use std::fs;

use anyhow::{Result, bail};

use crate::domain::feature;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::interfaces::cli::{QaArgs, QaCommand};

pub fn run(args: QaArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    match args.command {
        QaCommand::Baseline { id, observed } => baseline(&paths, &id, &observed),
        QaCommand::Slice {
            id,
            scenario,
            observed,
        } => slice(&paths, &id, &scenario, &observed),
    }
}

fn baseline(paths: &MaestroPaths, id: &str, observed: &str) -> Result<()> {
    let observed = non_empty(observed, "--observed")?;
    feature::ensure_exists(paths, id)?;
    let dir = feature::feature_sidecar_dir(paths, id);
    ensure_dir(&dir)?;
    let path = dir.join("qa.md");
    let contents = format!(
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scope: {id}\n- Critical workflow chains:\n  - CLI helper baseline\n    - Steps: setup -> action -> inspect output\n    - Touched link: feature QA gate\n    - Minimal proof: {observed}\n- Scenario Matrix:\n  - [bl-001] observed baseline behavior\n    - Dimensions: agent/CLI/local artifact\n    - Setup: repo initialized with feature {id}\n    - Action: {observed}\n    - Oracle: behavior remains observable\n    - Evidence to capture: command output or artifact diff\n    - Reproduction: rerun the observed command or workflow\n- Preserved behaviors:\n  - {observed} -> Proof: manual/CLI observation\n- Changed behaviors:\n  - None captured at baseline\n- Critical probes before commit:\n  - focused CLI/helper test\n- Required artifacts:\n  - .maestro/cards/{id}/qa.md\n- Baseline gaps:\n  - None\n"
    );
    write_string_atomic(&path, &contents)?;
    println!("recorded baseline bl-001");
    println!("feature: {id}");
    println!("file: {}", path.display());
    println!("next: maestro feature accept {id}");
    Ok(())
}

fn slice(paths: &MaestroPaths, id: &str, scenarios: &[String], observed: &str) -> Result<()> {
    let observed = non_empty(observed, "--observed")?;
    if scenarios.is_empty() {
        bail!("qa slice requires at least one --scenario");
    }
    for scenario in scenarios {
        non_empty(scenario, "--scenario")?;
    }
    feature::ensure_exists(paths, id)?;
    let path = feature::feature_sidecar_dir(paths, id).join("qa.md");
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    if !contents.contains("```yaml\nslices:") {
        contents.push_str("\n```yaml\nslices:\n```\n");
    }
    let insertion = format!(
        "  - scenarios: [{}]\n    evidence: [\"{}\"]\n",
        scenarios
            .iter()
            .map(|scenario| format!("\"{scenario}\""))
            .collect::<Vec<_>>()
            .join(", "),
        observed.replace('"', "\\\"")
    );
    contents = contents.replacen("slices:\n", &format!("slices:\n{insertion}"), 1);
    write_string_atomic(&path, &contents)?;
    println!("recorded qa slice");
    println!("feature: {id}");
    println!("file: {}", path.display());
    Ok(())
}

fn non_empty<'a>(value: &'a str, name: &str) -> Result<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(trimmed)
}
