use anyhow::Result;

use crate::commands::QueryArgs;

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    println!("query is not implemented in this phase slice: {args:?}");
    Ok(())
}
