use anyhow::Result;

use crate::domain::card;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{IndexArgs, IndexCommand};

/// Execute `maestro index` (SPEC-archive-memory-2 R6).
pub fn run(args: IndexArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        IndexCommand::Rebuild => {
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
            Ok(())
        }
    }
}
