use anyhow::Result;

use crate::commands::{McpArgs, McpCommand};
use crate::mcp::server;

/// Execute `maestro mcp`.
pub fn run(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Serve => server::serve(),
    }
}
