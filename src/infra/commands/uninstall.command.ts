import type { Command } from "commander";
import { removeAgentBlocks } from "@/infra/usecases/manage-agents.usecase.js";
import { formatAgentResults, output } from "@/shared/lib/output.js";
import { removeIfExists } from "@/shared/lib/fs.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import {
  resolveInstallDir,
  resolveInstalledBinaryName,
} from "@/infra/usecases/install-release-binary.usecase.js";
import { homedir } from "node:os";
import { join } from "node:path";

export function registerUninstallCommand(program: Command): void {
  program
    .command("uninstall")
    .description("Remove bundled agent skills and optionally the maestro binary")
    .option("--agents-only", "Only remove bundled agent skills")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const isJson = opts.json ?? program.opts().json;

      const agentResults = await removeAgentBlocks(process.cwd(), "home");

      let binaryRemoved = false;
      let configRemoved = false;

      if (!opts.agentsOnly) {
        const installDir = resolveInstallDir();
        binaryRemoved = await removeIfExists(join(installDir, resolveInstalledBinaryName()));
        configRemoved = await removeIfExists(join(homedir(), MAESTRO_DIR), { recursive: true });
      }

      output(isJson, { agents: agentResults, binaryRemoved, configRemoved }, (r) => [
        ...formatAgentResults(r.agents),
        "",
        r.binaryRemoved ? "[ok] Binary removed" : "[--] Binary kept",
        r.configRemoved ? "[ok] ~/.maestro/ removed" : "[--] ~/.maestro/ kept",
      ]);
    });
}
