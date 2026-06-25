use anyhow::Result;

use crate::domain::{card, search};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{IndexArgs, IndexCommand};

/// Execute `maestro index` (SPEC-archive-memory-2 R6).
pub fn run(args: IndexArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        IndexCommand::Rebuild { memory } => {
            if !memory {
                let report = card::index::rebuild(&paths)?;
                println!("text index rebuilt");
                println!(
                    "  docs: {} ({} live, {} archived)",
                    report.live_docs + report.archived_docs,
                    report.live_docs,
                    report.archived_docs
                );
                println!("  file: .maestro/index/text.json");
                println!("next: maestro card list --grep <word> [--archived]");
            }
            let report = search::rebuild_memory(&paths)?;
            println!("memory shard rebuilt");
            println!(
                "  docs: {} ({} live cards, {} archived cards, {} run evidence)",
                report.docs, report.live_docs, report.archived_docs, report.run_evidence_docs
            );
            println!("  file: .maestro/index/search/memory.shard");
            println!("next: maestro grep <query>");
            Ok(())
        }
    }
}
