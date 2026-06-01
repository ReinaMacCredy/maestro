use anyhow::Result;

use crate::interfaces::shell::{Shell, render_shell_init};

/// Execute `maestro shell-init`.
pub fn run() -> Result<()> {
    print!("{}", render_shell_init(Shell::detect()));
    Ok(())
}
