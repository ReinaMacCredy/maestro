import type { Command } from "commander";
import { registerMcpServeCommand } from "./commands/serve.command.js";
import { registerMcpCheckCommand } from "./commands/check.command.js";

export {
  buildMaestroAgentMcpConfigEntry,
  configureAgentRuntime,
  defaultAgentRuntimeTargets,
  resolveStartMjsInstallPath,
  type AgentMcpEntry,
  type AgentRuntimeTarget,
  type ConfigureRuntimeResult,
} from "./usecases/configure-agent-runtime.usecase.js";

export { buildMaestroMcpServer, startStdioMcpServer } from "./server/mcp-server.js";

export function registerMcpCommand(program: Command): void {
  const mcpCmd = program
    .command("mcp")
    .description("Maestro MCP server: start, configure, verify");
  registerMcpServeCommand(mcpCmd, program);
  registerMcpCheckCommand(mcpCmd, program);
}
