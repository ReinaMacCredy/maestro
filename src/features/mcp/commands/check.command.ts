import { existsSync } from "node:fs";
import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveInstallDir } from "@/infra/usecases/install-release-binary.usecase.js";
import {
  buildMaestroAgentMcpConfigEntry,
  defaultAgentRuntimeTargets,
  entriesEqual,
  readMaestroEntry,
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
      const expectedEntry = buildMaestroAgentMcpConfigEntry(binaryPath);

      const agentRuntimes: AgentRuntimeStatus[] = defaultAgentRuntimeTargets().map((target) => {
        let existing;
        try {
          existing = readMaestroEntry(target);
        } catch {
          existing = undefined;
        }
        return {
          name: target.name,
          configPath: target.configPath,
          configured: Boolean(existing),
          maestroEntryMatchesInstall: existing ? entriesEqual(existing, expectedEntry) : null,
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
