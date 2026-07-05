use anyhow::Result;

use crate::domain::capability;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{CapabilityArgs, shell_word};

pub fn run(args: CapabilityArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let report = capability::report(&paths, args.from.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render_human(&report);
    }
    Ok(())
}

fn render_human(report: &capability::CapabilityReport) {
    println!("registry: {}", shell_word(&report.registry.path));
    println!("registry_present: {}", report.registry.present);
    if report.capabilities.is_empty() {
        println!("capabilities: 0");
        return;
    }
    println!("capabilities:");
    for capability in &report.capabilities {
        println!(
            "- {} status={} active={}",
            capability.id,
            capability.status.as_str(),
            capability.active
        );
        for provider in &capability.providers {
            let reference = provider
                .evidence
                .reference
                .as_ref()
                .map(|reference| format!(" ref={}", shell_word(reference)))
                .unwrap_or_default();
            println!(
                "  provider {} kind={} status={}{}",
                provider.name,
                provider.kind,
                provider.status.as_str(),
                reference
            );
        }
    }
}
