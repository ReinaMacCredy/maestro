use anyhow::Result;

use crate::commands::{HookArgs, HookCommand};
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::hooks::record;

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
