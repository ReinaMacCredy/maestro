import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { setupCheck } from "@/service/index.js";
import { fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { SetupCheckInput } from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerSetupTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_setup_check",
    {
      title: "Check v2 project setup",
      description:
        "Audit whether the v2 directory tree (.maestro/tasks, .maestro/missions, .maestro/evidence, .maestro/runs, docs/principles) is present and valid. Returns ok (bool) and a list of entries with status ok|warn|missing. Read-only.",
      inputSchema: SetupCheckInput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (_args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const report = await setupCheck({ repoRoot: services.projectRoot });
        return toCallToolResult(ok(report));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "SETUP_CHECK_FAILED"));
      }
    },
  );
}
