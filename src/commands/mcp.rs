use anyhow::Result;

use crate::commands::{McpArgs, McpCommand};
use crate::mcp::server;
use crate::mcp::tools::tool_definitions;

/// Execute `maestro mcp`.
pub fn run(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Serve | McpCommand::Stdin => server::serve(),
        McpCommand::Tools | McpCommand::List => list_tools(),
    }
}

fn list_tools() -> Result<()> {
    for tool in tool_definitions() {
        println!("{}", tool.name);
    }
    Ok(())
}
