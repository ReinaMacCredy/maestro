import type { Command } from "commander";
import { type Services } from "@/services.js";
import { formatReport, runSetupCommand } from "@/runtime/setup.command.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";

interface InitCommandDeps {
  readonly getServices: () => Pick<Services, "config">;
}

export function registerInitCommand(program: Command, deps: InitCommandDeps): void {
  program
    .command("init", { hidden: true })
    .description("Alias for `maestro setup` (kept for backward compatibility)")
    .option("--global", "Initialize global config at ~/.maestro/")
    .option("--dry-run", "Show what would change without writing")
    .option("--resync-skills", "Reconcile .claude/skills and .codex/skills with shipped templates")
    .option("--reset-templates", "Replace customized bootstrap templates")
    .option("--no-git-ok", "Allow setup outside a git working tree")
    .option("--json", "Output as JSON")
    .action(async function (this: Command, opts): Promise<void> {
      try {
        const report = await runSetupCommand(opts, {
          resolveRepoRoot: () => resolveMaestroProjectRoot(process.cwd()),
          getServices: deps.getServices,
        });
        const isJson = opts.json === true || this.optsWithGlobals().json === true;
        if (isJson) {
          console.log(JSON.stringify(report, null, 2));
        } else {
          for (const line of formatReport(report)) console.log(line);
        }
        if (!report.ok) process.exitCode = 1;
      } catch (err) {
        console.error(`maestro init: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });
}
