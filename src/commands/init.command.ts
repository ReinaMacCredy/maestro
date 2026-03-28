import type { Command } from "commander";
import { getServices } from "../services.js";
import { initMaestro } from "../usecases/init.usecase.js";
import { output } from "../lib/output.js";

export function registerInitCommand(program: Command): void {
  program
    .command("init")
    .description("Initialize maestro in the current project or globally")
    .option("--global", "Initialize global config at ~/.maestro/")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const result = await initMaestro(services.config, {
        global: opts.global ?? false,
        dir: process.cwd(),
      });

      output(opts.json ?? program.opts().json, result, (r) => [
        `[ok] Initialized ${r.scope} config`,
        ...r.created.map((p) => `  --> ${p}`),
      ]);
    });
}
