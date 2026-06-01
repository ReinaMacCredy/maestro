use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{HookArgs, HookCommand};
use crate::interfaces::hooks::record;

pub fn run(args: HookArgs) -> Result<()> {
    match args.command {
        HookCommand::Record => {
            let result = discover_repo_root()
                .map(MaestroPaths::new)
                .and_then(|paths| record::record_stdin(&paths));
            if let Err(error) = result {
                eprintln!("maestro hook record warning: {error:#}");
            }
            Ok(())
        }
    }
}
