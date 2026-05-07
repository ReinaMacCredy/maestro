import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";

interface ServeOptions {
  readonly transport?: string;
  readonly projectRoot?: string;
  readonly json?: boolean;
}

export function registerMcpServeCommand(mcpCmd: Command, program: Command): void {
  mcpCmd
    .command("serve")
    .description("Start the maestro MCP server (stdio transport)")
    .option("--transport <name>", "Transport: stdio (default)", "stdio")
    .option("--project-root <path>", "Override project root detection")
    .action(async (opts: ServeOptions) => {
      const isJson = resolveJsonFlag(opts as { json?: boolean }, program);
      const transport = (opts.transport ?? "stdio").toLowerCase();
      if (transport !== "stdio") {
        const message = `Transport "${transport}" is not supported yet. Only stdio is available.`;
        if (isJson) console.log(JSON.stringify({ ok: false, error: message }));
        else console.error(`[!!] ${message}`);
        process.exit(1);
      }

      const { startStdioMcpServer } = await import("../server/mcp-server.js");
      try {
        await startStdioMcpServer({
          projectRoot: opts.projectRoot,
          initializeServices: opts.projectRoot !== undefined,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (isJson) console.log(JSON.stringify({ ok: false, error: message }));
        else console.error(`[!!] ${message}`);
        process.exit(1);
      }
    });
}
