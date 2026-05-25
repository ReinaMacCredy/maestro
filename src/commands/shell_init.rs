use anyhow::Result;

use crate::shell::{render_shell_init, Shell};

/// Execute `maestro shell-init`.
pub fn run() -> Result<()> {
    print!("{}", render_shell_init(Shell::detect()));
    Ok(())
}
