use anyhow::Result;

use crate::commands::MigrateArgs;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::migrate::v0_106_to_v0_8;

/// Execute `maestro migrate`.
pub fn run(args: MigrateArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let plan = v0_106_to_v0_8::plan(&paths)?;

    if args.check {
        print!("{}", v0_106_to_v0_8::render_check(&plan));
    } else {
        v0_106_to_v0_8::apply(&paths, &plan, args.force)?;
        println!("migration applied: {} change(s)", plan.changes.len());
    }
    Ok(())
}
