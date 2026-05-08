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
      // Stdio MCP servers must keep stdout for protocol traffic only.
      // All diagnostic output, including --json error payloads, goes to stderr.
      if (transport !== "stdio") {
        const message = `Transport "${transport}" is not supported yet. Only stdio is available.`;
        if (isJson) process.stderr.write(JSON.stringify({ ok: false, error: message }) + "\n");
        else process.stderr.write(`[!!] ${message}\n`);
        process.exit(1);
      }

      const { startStdioMcpServer } = await import("../server/mcp-server.js");
      try {
        // Always re-initialize services from the server's resolved root.
        // The CLI's preAction hook initialized services from process.cwd(),
        // which can disagree with MAESTRO_PROJECT_ROOT when the runtime
        // launches us from a different directory and scopes us via env.
        await startStdioMcpServer({
          projectRoot: opts.projectRoot,
          initializeServices: true,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (isJson) process.stderr.write(JSON.stringify({ ok: false, error: message }) + "\n");
        else process.stderr.write(`[!!] ${message}\n`);
        process.exit(1);
      }
    });
}
