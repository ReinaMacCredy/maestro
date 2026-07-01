use anyhow::Result;

use crate::domain::feature;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{SynthesizeArgs, SynthesizeCommand};

pub fn run(args: SynthesizeArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        SynthesizeCommand::Claim {
            card,
            slug,
            session,
        } => {
            let merge_owner = session.unwrap_or_else(super::cli_run_id);
            let report =
                feature::claim_synthesis(&paths, &card, &slug, &merge_owner, &utc_now_timestamp())?;
            println!(
                "claimed synthesis handoff {} for {}",
                report.slug, report.feature_id
            );
            println!("merge_owner: {}", report.merge_owner);
            println!("next: {}", report.next);
            println!("after: {}", report.after);
        }
    }

    println!("boundary: maestro recorded ledger facts only; run git commands separately");
    Ok(())
}
