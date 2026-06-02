//! Shell integration helpers.

use std::path::Path;

/// Supported interactive shell targets for `maestro shell-init`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shell {
    /// Bash-compatible shell functions.
    Bash,
    /// Zsh-compatible shell functions.
    Zsh,
    /// Fish shell functions.
    Fish,
}

impl Shell {
    /// Detect a shell from `MAESTRO_SHELL` or `SHELL`.
    pub fn detect() -> Self {
        std::env::var("MAESTRO_SHELL")
            .ok()
            .and_then(|value| Self::parse(&value))
            .or_else(|| {
                std::env::var("SHELL")
                    .ok()
                    .and_then(|value| Self::parse(&value))
            })
            .unwrap_or(Self::Bash)
    }

    fn parse(value: &str) -> Option<Self> {
        let name = Path::new(value)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(value);

        match name {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            _ => None,
        }
    }
}

/// Render the shell wrapper for the selected shell.
pub fn render_shell_init(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => POSIX_INIT,
        Shell::Zsh => POSIX_INIT,
        Shell::Fish => FISH_INIT,
    }
}

const POSIX_INIT: &str = include_str!("../../../embedded/shell/posix.sh");
const FISH_INIT: &str = include_str!("../../../embedded/shell/fish.fish");
