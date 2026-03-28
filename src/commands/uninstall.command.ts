import type { Command } from "commander";
import { removeAgentBlocks } from "../usecases/manage-agents.usecase.js";
import { formatAgentResults, output } from "../lib/output.js";
import { removeIfExists } from "../lib/fs.js";
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
        const installDir = process.env.MAESTRO_INSTALL_DIR ?? join(homedir(), ".local", "bin");
        binaryRemoved = await removeIfExists(join(installDir, "maestro"));
        configRemoved = await removeIfExists(join(homedir(), ".maestro"), { recursive: true });
      }

      output(isJson, { agents: agentResults, binaryRemoved, configRemoved }, (r) => [
        ...formatAgentResults(r.agents),
        "",
        r.binaryRemoved ? "[ok] Binary removed" : "[--] Binary kept",
        r.configRemoved ? "[ok] ~/.maestro/ removed" : "[--] ~/.maestro/ kept",
      ]);
    });
}
