use anyhow::Result;

use crate::domain::maturity;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::MaturityArgs;

pub fn run(args: MaturityArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let report = maturity::report(&paths, args.target.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render_human(&report);
    }
    Ok(())
}

fn render_human(report: &maturity::MaturityReport) {
    println!("maturity: {}", report.maturity.level);
    println!("  rationale: {}", report.maturity.rationale);
    println!("context:");
    for item in &report.context {
        println!(
            "- {} required={} status={} evidence={}",
            item.name,
            item.required,
            item.status.as_str(),
            item.evidence
        );
    }
    println!(
        "proof: complete={} partial={} incomplete={} total={}",
        report.proof.complete, report.proof.partial, report.proof.incomplete, report.proof.total
    );
    for gap in &report.proof.gaps {
        println!("- gap {} owner={}", gap.ac_id, gap.owner_surface);
    }
    println!(
        "friction: ux_gaps={} harness_backlog={} recurring={} diagnostics={}",
        report.friction.ux_gap_entries,
        report.friction.harness_backlog_items,
        report.friction.recurring_items.len(),
        report.friction.diagnostics.len()
    );
    for diagnostic in &report.friction.diagnostics {
        println!("- diagnostic {diagnostic}");
    }
    println!("next_owner: {}", report.next_owner.surface);
    println!("  command: {}", report.next_owner.command);
}
