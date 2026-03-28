import type { Command } from "commander";
import { getServices } from "../services.js";
import { initMaestro } from "../usecases/init.usecase.js";
import { injectAgentBlocks } from "../usecases/manage-agents.usecase.js";
import { formatAgentResults, output } from "../lib/output.js";

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
        injectAgentBlocks(),
      ]);

      output(isJson, { init: initResult, agents: agentResults }, (r) => [
        `[ok] Global config initialized`,
        ...r.init.created.map((p: string) => `  --> ${p}`),
        "",
        ...formatAgentResults(r.agents),
      ]);
    });
}
