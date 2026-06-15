use anyhow::Result;

use crate::domain::playbook;
use crate::interfaces::cli::PlaybookArgs;

/// Execute `maestro playbook [<lang>]`: print one language guide verbatim, or
/// the index when no token is given. Served from the binary, so it needs no
/// `.maestro` repo.
pub fn run(args: PlaybookArgs) -> Result<()> {
    match args.lang.as_deref() {
        Some(lang) => print!("{}", playbook::serve(lang)?),
        None => print!("{}", playbook::index()),
    }
    Ok(())
}
