import type { Command } from "commander";
import { getServices } from "../services.js";
import { initMaestro } from "../usecases/init.usecase.js";
import { injectAgentBlocks } from "../usecases/manage-agents.usecase.js";
import { output } from "../lib/output.js";

export function registerInstallCommand(program: Command): void {
  program
    .command("install")
    .description("Initialize global config and inject agent instructions")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      // Initialize global config
      const initResult = await initMaestro(services.config, {
        global: true,
        dir: process.cwd(),
      });

      // Inject agent instruction blocks
      const agentResults = await injectAgentBlocks();

      const result = {
        init: initResult,
        agents: agentResults,
      };

      output(isJson, result, (r) => [
        `[ok] Global config initialized`,
        ...r.init.created.map((p: string) => `  --> ${p}`),
        "",
        ...r.agents.map((a: { agent: string; action: string; configPath: string }) =>
          `  ${a.agent}: ${a.action} (${a.configPath})`
        ),
      ]);
    });
}
