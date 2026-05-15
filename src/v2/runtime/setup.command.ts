import { Command } from "commander";
import { migrateCorrections } from "../service/migrate-corrections.usecase.js";
import { setupBootstrap } from "../service/setup-bootstrap.usecase.js";
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
          console.log(JSON.stringify(report, null, 2));
        } else {
          for (const entry of report.entries) console.log(formatEntry(entry));
          console.log(report.ok ? "setup check: OK" : "setup check: action required");
        }
        if (!report.ok) process.exitCode = 1;
      } catch (err) {
        console.error(`maestro setup check: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });

  setup
    .command("bootstrap")
    .description("Create missing v2 directories with .gitkeep stubs")
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, flags: { json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const result = await setupBootstrap({ repoRoot });
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify(result, null, 2));
          return;
        }
        if (result.created.length === 0) {
          console.log(`nothing to create (${result.skipped.length} already present)`);
        } else {
          console.log(`created ${result.created.length}, skipped ${result.skipped.length}`);
          for (const path of result.created) console.log(`  created ${path}`);
          for (const path of result.skipped) console.log(`  skipped ${path}`);
        }
      } catch (err) {
        console.error(`maestro setup bootstrap: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });

  setup
    .command("migrate-corrections")
    .description(
      "Migrate v1 .maestro/memory/corrections/*.json into docs/principles/legacy/<id>.md",
    )
    .option("--overwrite", "Overwrite existing legacy/<id>.md instead of skipping")
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, flags: { overwrite?: boolean; json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const result = await migrateCorrections(
          { repoRoot },
          { overwrite: flags.overwrite === true },
        );
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify(result, null, 2));
          return;
        }
        if (result.missing_source) {
          console.log("no .maestro/memory/corrections directory — nothing to migrate");
          return;
        }
        console.log(
          `scanned ${result.scanned}, migrated ${result.migrated.length}, skipped ${result.skipped.length}`,
        );
        for (const id of result.migrated) console.log(`  migrated ${id}`);
        for (const id of result.skipped) console.log(`  skipped  ${id}`);
      } catch (err) {
        console.error(`maestro setup migrate-corrections: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });
}
