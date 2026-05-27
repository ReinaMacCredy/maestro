use anyhow::Result;

use crate::foundation::core::paths::{discover_repo_root, discover_repo_root_from, MaestroPaths};
use crate::interfaces::cli::MigrateArgs;
use crate::operations::migrate;

/// Execute `maestro migrate`.
pub fn run(args: MigrateArgs) -> Result<()> {
    let repo_root = match args.project.as_deref() {
        Some(project) => discover_repo_root_from(project)?,
        None => discover_repo_root()?,
    };
    let paths = MaestroPaths::new(repo_root);
    let plan = migrate::plan(&paths)?;

    if args.check {
        print!("{}", migrate::render_check(&plan));
    } else {
        migrate::apply(&paths, &plan, args.force)?;
        println!("migration applied: {} change(s)", plan.changes.len());
    }
    Ok(())
}
