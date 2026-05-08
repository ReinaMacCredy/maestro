import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { getServices, initServices, type Services } from "@/services.js";
import { VERSION } from "@/shared/version.js";
import { findMaestroProjectRoot } from "./project.js";
import { detectMcpSessionId } from "./session.js";
import { registerContractTools } from "./tools/contract-tools.js";
import { registerEvidenceTools } from "./tools/evidence-tools.js";
import { registerHandoffTools } from "./tools/handoff-tools.js";
import { registerPolicyTools } from "./tools/policy-tools.js";
import { registerTaskTools } from "./tools/task-tools.js";
import type { RegisterDeps } from "./tools/types.js";
import { registerVerdictTools } from "./tools/verdict-tools.js";

export interface McpServerOptions {
  readonly projectRoot?: string;
  /**
   * Initialize Services from the resolved project root before registering
   * tools. Set to false when the caller has already called initServices()
   * for the same project (CLI path runs preAction). Defaults to true so
   * the standalone start.mjs entry works without extra wiring.
   */
  readonly initializeServices?: boolean;
}

export function buildMaestroMcpServer(options: McpServerOptions = {}): {
  server: McpServer;
  services: Services;
  projectRoot: string;
} {
  const projectRoot = options.projectRoot ?? findMaestroProjectRoot();
  const services =
    options.initializeServices === false ? getServices() : initServices(projectRoot);

  const server = new McpServer(
    {
      name: "maestro",
      version: VERSION,
    },
    {
      capabilities: {
        tools: {},
      },
    },
  );

  const deps: RegisterDeps = {
    getServices: () => services,
    sessionId: detectMcpSessionId(),
  };
  registerTaskTools(server, deps);
  registerEvidenceTools(server, deps);
  registerVerdictTools(server, deps);
  registerContractTools(server, deps);
  registerPolicyTools(server, deps);
  registerHandoffTools(server, deps);

  return { server, services, projectRoot };
}

export async function startStdioMcpServer(
  options: McpServerOptions = {},
): Promise<{ server: McpServer; projectRoot: string }> {
  const { server, projectRoot } = buildMaestroMcpServer(options);
  const transport = new StdioServerTransport();
  await server.connect(transport);
  return { server, projectRoot };
}

