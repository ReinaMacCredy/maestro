import type { Command } from "commander";
import { injectAgentBlocks } from "../usecases/manage-agents.usecase.js";
import { output } from "../lib/output.js";
import { execArgv } from "../lib/shell.js";
import { getServices } from "../services.js";

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
          console.error("[!] No sourceRepo in config. Run maestro install from the repo first.");
          process.exit(1);
        }

        const pull = await execArgv(["git", "-C", sourceRepo, "pull"]);
        if (pull.exitCode !== 0) {
          console.error(`[!] git pull failed: ${pull.stderr}`);
          process.exit(1);
        }

        const install = await execArgv(["bun", "install", "--frozen-lockfile"], { cwd: sourceRepo });
        if (install.exitCode !== 0) {
          console.error(`[!] bun install failed: ${install.stderr}`);
          process.exit(1);
        }

        const build = await execArgv(["bun", "run", "build"], { cwd: sourceRepo });
        if (build.exitCode !== 0) {
          console.error(`[!] bun run build failed: ${build.stderr}`);
          process.exit(1);
        }

        const installDir = process.env.MAESTRO_INSTALL_DIR ?? `${process.env.HOME}/.local/bin`;
        const cp = await execArgv(["cp", `${sourceRepo}/dist/maestro`, `${installDir}/maestro`]);
        if (cp.exitCode !== 0) {
          console.error(`[!] Failed to copy binary: ${cp.stderr}`);
          process.exit(1);
        }

        binaryUpdated = true;
      }

      const agentResults = await injectAgentBlocks();

      const result = {
        binaryUpdated,
        agents: agentResults,
      };

      output(isJson, result, (r) => [
        r.binaryUpdated ? "[ok] Binary updated" : "[--] Binary skipped (--agents-only)",
        "",
        ...r.agents.map((a: { agent: string; action: string; configPath: string }) =>
          `  ${a.agent}: ${a.action} (${a.configPath})`
        ),
      ]);
    });
}
