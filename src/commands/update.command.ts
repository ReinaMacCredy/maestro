import type { Command } from "commander";
import { injectAgentBlocks } from "../usecases/manage-agents.usecase.js";
import { formatAgentResults, output } from "../lib/output.js";
import { execArgv } from "../lib/shell.js";
import { getServices } from "../services.js";
import { MaestroError } from "../domain/errors.js";

async function execOrThrow(argv: string[], name: string, opts?: { cwd?: string }): Promise<void> {
  const result = await execArgv(argv, opts);
  if (result.exitCode !== 0) {
    throw new MaestroError(`${name} failed: ${result.stderr}`, [
      `Command: ${argv.join(" ")}`,
    ]);
  }
}

export function registerUpdateCommand(program: Command): void {
  program
    .command("update")
    .description("Update maestro binary and/or agent instruction blocks")
    .option("--agents-only", "Only update agent instruction blocks, skip binary rebuild")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      let binaryUpdated = false;

      if (!opts.agentsOnly) {
        const config = await services.config.load(process.cwd());
        const sourceRepo = config.sourceRepo;

        if (!sourceRepo) {
          throw new MaestroError("No sourceRepo in config", [
            "Run maestro install from the repo first",
          ]);
        }

        await execOrThrow(["git", "-C", sourceRepo, "pull"], "git pull");
        await execOrThrow(["bun", "install", "--frozen-lockfile"], "bun install", { cwd: sourceRepo });
        await execOrThrow(["bun", "run", "build"], "bun run build", { cwd: sourceRepo });

        const installDir = process.env.MAESTRO_INSTALL_DIR ?? `${process.env.HOME}/.local/bin`;
        await execOrThrow(["cp", `${sourceRepo}/dist/maestro`, `${installDir}/maestro`], "copy binary");

        binaryUpdated = true;
      }

      const agentResults = await injectAgentBlocks();

      output(isJson, { binaryUpdated, agents: agentResults }, (r) => [
        r.binaryUpdated ? "[ok] Binary updated" : "[--] Binary skipped (--agents-only)",
        "",
        ...formatAgentResults(r.agents),
      ]);
    });
}
