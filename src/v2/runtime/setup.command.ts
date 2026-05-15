import { Command } from "commander";
import { migrateCorrections } from "../service/migrate-corrections.usecase.js";

export interface SetupCommandV2Options {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateSetupCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "setup");
  if (existing) return existing;
  return program.command("setup").description("Setup / migration verbs (v2)");
}

export function registerSetupV2Commands(
  program: Command,
  opts: SetupCommandV2Options,
): void {
  const setup = findOrCreateSetupCommand(program);

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
