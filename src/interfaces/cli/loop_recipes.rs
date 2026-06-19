use anyhow::Result;

use crate::domain::loop_recipes;
use crate::interfaces::cli::{LoopArgs, LoopCommand};

/// Execute `maestro loop [list | show <name>]`: print the recipe index (the
/// default and `list`), or one recipe verbatim. Served from the binary, so it
/// needs no `.maestro` repo.
pub fn run(args: LoopArgs) -> Result<()> {
    match args.command {
        None | Some(LoopCommand::List) => print!("{}", loop_recipes::index()),
        Some(LoopCommand::Show { name }) => print!("{}", loop_recipes::serve(&name)?),
    }
    Ok(())
}
