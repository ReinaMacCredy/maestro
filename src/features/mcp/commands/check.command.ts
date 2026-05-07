import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveInstallDir } from "@/infra/usecases/install-release-binary.usecase.js";
import {
  buildMaestroAgentMcpConfigEntry,
  resolveMaestroBinaryInstallPath,
} from "../usecases/configure-agent-runtime.usecase.js";

interface CheckOptions {
  readonly json?: boolean;
}

interface AgentRuntimeStatus {
  readonly name: string;
  readonly configPath: string;
  readonly configured: boolean;
  readonly maestroEntryMatchesInstall: boolean | null;
}

interface CheckResult {
  readonly version: string;
  readonly binaryPath: string;
  readonly binaryExists: boolean;
  readonly agentRuntimes: readonly AgentRuntimeStatus[];
}

export function registerMcpCheckCommand(mcpCmd: Command, program: Command): void {
  mcpCmd
    .command("check")
    .description("Verify maestro MCP server installation and agent runtime configuration")
    .option("--json", "Output as JSON")
    .action(async (opts: CheckOptions) => {
      const isJson = resolveJsonFlag(opts as { json?: boolean }, program);
      const installDir = resolveInstallDir();
      const binaryPath = resolveMaestroBinaryInstallPath(installDir);

      const candidates: { name: string; path: string }[] = [
        { name: "Claude Code", path: join(homedir(), ".claude", "mcp.json") },
        { name: "Codex", path: join(homedir(), ".codex", "mcp.json") },
      ];

      const expectedEntry = buildMaestroAgentMcpConfigEntry(binaryPath);
      const agentRuntimes: AgentRuntimeStatus[] = candidates.map((c) => {
        if (!existsSync(c.path)) {
          return {
            name: c.name,
            configPath: c.path,
            configured: false,
            maestroEntryMatchesInstall: null,
          };
        }
        let configured = false;
        let matches = false;
        try {
          const raw = JSON.parse(readFileSync(c.path, "utf8"));
          const existing = raw?.mcpServers?.maestro;
          configured = Boolean(existing);
          if (existing) {
            matches =
              existing.command === expectedEntry.command &&
              JSON.stringify(existing.args ?? []) === JSON.stringify(expectedEntry.args);
          }
        } catch {
          configured = false;
        }
        return {
          name: c.name,
          configPath: c.path,
          configured,
          maestroEntryMatchesInstall: configured ? matches : null,
        };
      });

      const { VERSION } = await import("@/shared/version.js");
      const result: CheckResult = {
        version: VERSION,
        binaryPath,
        binaryExists: existsSync(binaryPath),
        agentRuntimes,
      };

      output(isJson, result, (r) => {
        const lines = [
          `Maestro MCP server check (v${r.version})`,
          ``,
          `maestro binary: ${r.binaryExists ? "[ok]" : "[!!]"} ${r.binaryPath}`,
          ``,
          `Agent runtime configurations:`,
        ];
        for (const a of r.agentRuntimes) {
          if (!a.configured) {
            lines.push(`  ${a.name}: not configured (${a.configPath})`);
            continue;
          }
          const tag = a.maestroEntryMatchesInstall ? "[ok]" : "[stale]";
          lines.push(`  ${a.name}: ${tag} ${a.configPath}`);
        }
        if (!r.binaryExists) {
          lines.push(``, `Run 'bun run release:local' to build and install the maestro binary.`);
        }
        return lines;
      });

      if (!result.binaryExists) {
        process.exit(1);
      }
    });
}
