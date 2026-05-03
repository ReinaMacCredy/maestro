import type { Command } from "commander";
import { getServices } from "@/services.js";
import { initMaestro } from "../usecases/init.usecase.js";
import { injectAgentBlocks } from "@/infra/usecases/manage-agents.usecase.js";
import { formatAgentResults, output } from "@/shared/lib/output.js";
import { resolveInstallDir } from "../usecases/install-release-binary.usecase.js";

export function registerInstallCommand(program: Command): void {
  program
    .command("install")
    .description("Initialize global config and inject agent instructions")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const [initResult, agentResults] = await Promise.all([
        initMaestro(services.config, { global: true, dir: process.cwd() }),
        injectAgentBlocks(process.cwd(), "home"),
      ]);

      output(isJson, { init: initResult, agents: agentResults }, (r) => {
        const lines = [
          `[ok] Global config initialized`,
          ...r.init.created.map((p: string) => `  --> ${p}`),
          "",
          ...formatAgentResults(r.agents),
        ];
        if (process.platform === "win32") {
          lines.push("", `[!] On Windows, ensure ${resolveInstallDir()} is on your user PATH.`);
          lines.push("    See scripts/install.ps1 for guidance.");
        }
        return lines;
      });
    });
}
