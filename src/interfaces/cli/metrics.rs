use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{MetricsArgs, MetricsCommand};
use crate::operations::metrics;

/// Execute `maestro metrics`.
pub fn run(args: MetricsArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        MetricsCommand::Summary => summary(&paths),
    }
}

fn summary(paths: &MaestroPaths) -> Result<()> {
    print!("{}", metrics::render_summary(&metrics::summarize(paths)?));
    Ok(())
}
