use anyhow::Result;

use crate::domain::{card, search};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{IndexArgs, IndexCommand};

/// Execute `maestro index` (SPEC-archive-memory-2 R6).
pub fn run(args: IndexArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        IndexCommand::Rebuild {
            memory,
            source,
            cards,
        } => {
            let rebuild_all = !memory && !source && !cards;
            let rebuild_cards = cards || rebuild_all;
            let rebuild_memory = memory || cards || rebuild_all;
            let rebuild_source = source || rebuild_all;
            let _guard = search::acquire_writer(&paths)?;

            if rebuild_cards {
                let report = card::index::rebuild(&paths)?;
                println!("cards compatibility index rebuilt");
                println!("  text index rebuilt");
                println!(
                    "  docs: {} ({} live, {} archived)",
                    report.live_docs + report.archived_docs,
                    report.live_docs,
                    report.archived_docs
                );
                println!("  file: .maestro/index/text.json");
                println!("next: maestro card list --grep <word> [--archived]");
            }
            if rebuild_memory {
                let report = search::rebuild_memory_unlocked(&paths)?;
                println!("memory shard rebuilt");
                println!(
                    "  docs: {} ({} live cards, {} archived cards, {} run evidence)",
                    report.docs, report.live_docs, report.archived_docs, report.run_evidence_docs
                );
                println!("  file: .maestro/index/search/memory.shard");
            }
            if rebuild_source {
                let report = search::rebuild_source_unlocked(&paths)?;
                println!("source shard rebuilt");
                println!(
                    "  files: {} indexed, {} skipped",
                    report.indexed_files, report.skipped_files
                );
                println!("  outline entries: {}", report.outline_entries);
                if report.ctags_status.available {
                    println!("  ctags symbols: {}", report.ctags_symbols);
                } else {
                    println!(
                        "  ctags: optional missing ({})",
                        report.ctags_status.message
                    );
                }
                for (reason, count) in &report.skipped_by_reason {
                    println!("  skipped {reason}: {count}");
                }
                for skip in &report.representative_skips {
                    println!("  skip example: {} ({})", skip.path, skip.reason);
                }
                println!("  file: .maestro/index/search/source.shard");
            }
            println!("next: maestro grep <query>");
            Ok(())
        }
    }
}
