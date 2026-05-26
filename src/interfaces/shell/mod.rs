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

const POSIX_INIT: &str = r#"# Maestro shell integration for bash/zsh.
maestro() {
  local __maestro_status

  command maestro "$@"
  __maestro_status=$?

  if [ "$__maestro_status" -eq 0 ]; then
    if [ "$1" = "task" ] && [ "$2" = "claim" ] && [ -n "${3-}" ]; then
      export MAESTRO_CURRENT_TASK="$3"
    elif [ "$1" = "task" ] && [ "$2" = "complete" ] && [ -n "${3-}" ]; then
      unset MAESTRO_CURRENT_TASK
    fi
  fi

  return "$__maestro_status"
}
"#;

const FISH_INIT: &str = r#"# Maestro shell integration for fish.
function maestro
    command maestro $argv
    set -l __maestro_status $status

    if test $__maestro_status -eq 0
        if test (count $argv) -ge 3; and test "$argv[1]" = "task"; and test "$argv[2]" = "claim"
            set -gx MAESTRO_CURRENT_TASK "$argv[3]"
        else if test (count $argv) -ge 3; and test "$argv[1]" = "task"; and test "$argv[2]" = "complete"
            set -e MAESTRO_CURRENT_TASK
        end
    end

    return $__maestro_status
end
"#;
