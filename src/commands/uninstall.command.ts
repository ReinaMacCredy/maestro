import type { Command } from "commander";
import { removeAgentBlocks } from "../usecases/manage-agents.usecase.js";
import { output } from "../lib/output.js";
import { rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export function registerUninstallCommand(program: Command): void {
  program
    .command("uninstall")
    .description("Remove agent instruction blocks and optionally the maestro binary")
    .option("--agents-only", "Only remove agent instruction blocks")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const isJson = opts.json ?? program.opts().json;

      const agentResults = await removeAgentBlocks();

      let binaryRemoved = false;
      let configRemoved = false;

      if (!opts.agentsOnly) {
        const installDir = process.env.MAESTRO_INSTALL_DIR ?? `${process.env.HOME}/.local/bin`;
        const binaryPath = join(installDir, "maestro");
        const globalConfigDir = join(homedir(), ".maestro");

        if (existsSync(binaryPath)) {
          await rm(binaryPath);
          binaryRemoved = true;
        }

        if (existsSync(globalConfigDir)) {
          await rm(globalConfigDir, { recursive: true });
          configRemoved = true;
        }
      }

      const result = {
        agents: agentResults,
        binaryRemoved,
        configRemoved,
      };

      output(isJson, result, (r) => [
        ...r.agents.map((a: { agent: string; action: string; configPath: string }) =>
          `  ${a.agent}: ${a.action} (${a.configPath})`
        ),
        "",
        r.binaryRemoved ? "[ok] Binary removed" : "[--] Binary kept",
        r.configRemoved ? "[ok] ~/.maestro/ removed" : "[--] ~/.maestro/ kept",
      ]);
    });
}
