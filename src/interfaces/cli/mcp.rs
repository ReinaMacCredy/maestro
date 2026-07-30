use anyhow::Result;

use crate::interfaces::cli::{McpArgs, McpCommand};
use crate::interfaces::mcp::server;
use crate::operations::adapters::GLOBAL_MCP_TOOLS_V1;

/// Execute `maestro mcp`.
pub fn run(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Serve => server::serve(),
        McpCommand::Tools => list_tools(),
    }
}

fn list_tools() -> Result<()> {
    for definition in GLOBAL_MCP_TOOLS_V1 {
        println!("{}", definition.name);
    }
    Ok(())
}
