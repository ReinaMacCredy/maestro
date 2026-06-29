use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::domain::feature;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::interfaces::cli::{QaArgs, QaCommand};
use crate::operations::harness;

pub fn run(args: QaArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    match args.command {
        QaCommand::Baseline {
            id,
            observed,
            observed_file,
            observed_stdin,
        } => {
            let observed = read_observed_input(
                ObservedCommand::Baseline,
                observed,
                observed_file,
                observed_stdin,
            )?;
            baseline(&paths, &id, &observed)
        }
        QaCommand::Slice {
            id,
            scenario,
            observed,
            observed_file,
            observed_stdin,
        } => {
            let observed = read_observed_input(
                ObservedCommand::Slice,
                observed,
                observed_file,
                observed_stdin,
            )?;
            slice(&paths, &id, &scenario, &observed)
        }
    }
}

fn baseline(paths: &MaestroPaths, id: &str, observed: &str) -> Result<()> {
    non_empty(observed, "--observed")?;
    feature::ensure_exists(paths, id)?;
    let dir = feature::feature_sidecar_dir(paths, id);
    ensure_dir(&dir)?;
    let path = dir.join("qa.md");
    let mut contents = format!(
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scope: {id}\n- Critical workflow chains:\n  - CLI helper baseline\n    - Steps: setup -> action -> inspect output\n    - Touched link: feature QA gate\n    - Minimal proof: {observed}\n- Scenario Matrix:\n  - [bl-001] observed baseline behavior\n    - Dimensions: agent/CLI/local artifact\n    - Setup: repo initialized with feature {id}\n    - Action: {observed}\n    - Oracle: behavior remains observable\n    - Evidence to capture: command output or artifact diff\n    - Reproduction: rerun the observed command or workflow\n- Preserved behaviors:\n  - {observed} -> Proof: manual/CLI observation\n- Changed behaviors:\n  - None captured at baseline\n- Critical probes before commit:\n  - focused CLI/helper test\n- Security gates:\n  - Risk classes: destructive_fs_git; dependency_version; schema_migration; secrets; external_side_effects; release_publish_push\n  - Enforcement: task verify/complete plus feature verify/close\n  - Required proof: task proof, QA evidence, or feature acceptance evidence\n  - Waiver/block: feature verify --waive or task block --reason\n- Required artifacts:\n  - .maestro/cards/{id}/qa.md\n- Baseline gaps:\n  - None\n"
    );
    append_raw_observed_block(&mut contents, "baseline", observed);
    write_string_atomic(&path, &contents)?;
    println!("recorded baseline bl-001");
    println!("feature: {id}");
    println!("file: {}", path.display());
    println!("{}", harness::security_qa_gate_line());
    println!("next: maestro feature accept {id}");
    Ok(())
}

fn slice(paths: &MaestroPaths, id: &str, scenarios: &[String], observed: &str) -> Result<()> {
    non_empty(observed, "--observed")?;
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
        "  - scenarios: [{}]\n    evidence: [{}]\n",
        scenarios
            .iter()
            .map(|scenario| yaml_double_quoted(scenario))
            .collect::<Vec<_>>()
            .join(", "),
        yaml_double_quoted(observed)
    );
    contents = contents.replacen("slices:\n", &format!("slices:\n{insertion}"), 1);
    append_raw_observed_block(&mut contents, "slice", observed);
    write_string_atomic(&path, &contents)?;
    println!("recorded qa slice");
    println!("feature: {id}");
    println!("file: {}", path.display());
    println!("{}", harness::security_qa_gate_line());
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ObservedCommand {
    Baseline,
    Slice,
}

fn read_observed_input(
    command: ObservedCommand,
    observed: Option<String>,
    observed_file: Option<PathBuf>,
    observed_stdin: bool,
) -> Result<String> {
    let source_count = usize::from(observed.is_some())
        + usize::from(observed_file.is_some())
        + usize::from(observed_stdin);
    if source_count != 1 {
        bail!(
            "{}",
            observed_input_guidance(command, "provide exactly one observed input")
        );
    }

    let value = if let Some(value) = observed {
        if value.trim_start().starts_with('-') {
            bail!(
                "{}",
                observed_input_guidance(
                    command,
                    "inline observed evidence looks like an option or frontmatter; use a safer input form"
                )
            );
        }
        value
    } else if let Some(path) = observed_file {
        fs::read_to_string(&path)
            .with_context(|| format!("failed to read --observed-file {}", path.display()))?
    } else {
        let mut value = String::new();
        io::stdin()
            .read_to_string(&mut value)
            .context("failed to read --observed-stdin")?;
        value
    };
    if value.trim().is_empty() {
        bail!(
            "{}",
            observed_input_guidance(command, "observed evidence must not be empty")
        );
    }
    Ok(value)
}

fn observed_input_guidance(command: ObservedCommand, reason: &str) -> String {
    let (inline, file, stdin) = match command {
        ObservedCommand::Baseline => (
            "maestro qa baseline <ID> --observed \"<OBSERVED>\"",
            "maestro qa baseline <ID> --observed-file <PATH>",
            "maestro qa baseline <ID> --observed-stdin",
        ),
        ObservedCommand::Slice => (
            "maestro qa slice <ID> --scenario bl-001 --observed \"<OBSERVED>\"",
            "maestro qa slice <ID> --scenario bl-001 --observed-file <PATH>",
            "maestro qa slice <ID> --scenario bl-001 --observed-stdin",
        ),
    };
    format!("{reason}\ncanonical inline: {inline}\nsafer file: {file}\nsafer stdin: {stdin}")
}

fn append_raw_observed_block(contents: &mut String, label: &str, observed: &str) {
    contents.push_str("\n### Raw Observed Evidence\n\n<!-- maestro:qa-observed:");
    contents.push_str(label);
    contents.push_str(":start -->\n");
    contents.push_str(observed);
    contents.push_str("<!-- maestro:qa-observed:");
    contents.push_str(label);
    contents.push_str(":end -->\n");
}

fn yaml_double_quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn non_empty(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(())
}
