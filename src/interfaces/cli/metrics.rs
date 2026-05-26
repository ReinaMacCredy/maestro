use anyhow::Result;

use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::interfaces::cli::{MetricsArgs, MetricsCommand};
use crate::metrics::summary::{render_summary, summarize};

/// Execute `maestro metrics`.
pub fn run(args: MetricsArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        MetricsCommand::Summary => summary(&paths),
    }
}

fn summary(paths: &MaestroPaths) -> Result<()> {
    print!("{}", render_summary(&summarize(paths)?));
    Ok(())
}
