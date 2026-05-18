import type { Command } from "commander";
import { type Services } from "@/services.js";
import { runSetup } from "@/service/setup.usecase.js";
import { injectAgentBlocks } from "@/infra/usecases/manage-agents.usecase.js";
import { formatAgentResults, output } from "@/shared/lib/output.js";
import { resolveInstallDir } from "../usecases/install-release-binary.usecase.js";

interface InstallCommandDeps {
  readonly getServices: () => Pick<Services, "config">;
}

export function registerInstallCommand(program: Command, deps: InstallCommandDeps): void {
  program
    .command("install")
    .description("Initialize global config and inject agent instructions")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json;

      const [setupReport, agentResults] = await Promise.all([
        runSetup({ config: services.config, global: true, dir: process.cwd() }),
        injectAgentBlocks(process.cwd(), "home"),
      ]);

      output(isJson, { setup: setupReport, agents: agentResults }, (r) => {
        const lines = [
          `[ok] Global config initialized`,
          ...r.setup.created.map((p: string) => `  --> ${p}`),
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
