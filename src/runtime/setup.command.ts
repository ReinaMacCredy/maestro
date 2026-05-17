import { Command } from "commander";
import { setupCheck, type SetupCheckEntry } from "../service/setup-check.usecase.js";

export interface SetupCommandV2Options {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateSetupCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "setup");
  if (existing) return existing;
  return program.command("setup").description("Setup / migration verbs (v2)");
}

function formatEntry(entry: SetupCheckEntry): string {
  const marker =
    entry.status === "ok" ? "[ok]   " : entry.status === "warn" ? "[warn] " : "[miss] ";
  const detail = entry.detail ? ` — ${entry.detail}` : "";
  return `${marker}${entry.path}${detail}`;
}

export function registerSetupV2Commands(
  program: Command,
  opts: SetupCommandV2Options,
): void {
  const setup = findOrCreateSetupCommand(program);

  setup
    .command("check")
    .description("Audit the v2 directory layout (.maestro/{tasks,plans,evidence,runs}, docs/principles)")
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, flags: { json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const report = await setupCheck({ repoRoot });
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify({ ...report, project_root: repoRoot }, null, 2));
        } else {
          console.log(`project root: ${repoRoot}`);
          for (const entry of report.entries) console.log(formatEntry(entry));
          console.log(report.ok ? "setup check: OK" : "setup check: action required");
        }
        if (!report.ok) process.exitCode = 1;
      } catch (err) {
        console.error(`maestro setup check: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });
}
