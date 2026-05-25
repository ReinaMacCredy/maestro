pub mod lock;
pub mod mirrors;

/// Agent integrations supported by V1 install.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallAgent {
    /// Claude Code.
    Claude,
    /// Codex CLI.
    Codex,
}

impl InstallAgent {
    /// Stable lockfile key for the agent.
    pub fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}
