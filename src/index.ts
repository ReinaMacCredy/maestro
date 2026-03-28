#!/usr/bin/env bun
import { Command, CommanderError } from "commander";
import { VERSION } from "./version.js";
import { MaestroError } from "./domain/errors.js";

export const program = new Command()
  .name("maestro")
  .description("Cross-agent handoff CLI")
  .version(VERSION)
  .option("--json", "Output as JSON")
  .exitOverride();

// Commands will be registered here in Phase 5

async function main(): Promise<void> {
  try {
    await program.parseAsync(process.argv);
  } catch (err) {
    if (err instanceof CommanderError) {
      // --help and --version throw with exitCode 0
      process.exit(err.exitCode);
    }
    if (err instanceof MaestroError) {
      const isJson = process.argv.includes("--json");
      if (isJson) {
        console.log(
          JSON.stringify(
            { error: err.message, hints: err.hints },
            null,
            2,
          ),
        );
      } else {
        console.error(`[!] ${err.message}`);
        for (const hint of err.hints) {
          console.error(`    ${hint}`);
        }
      }
      process.exit(1);
    }
    throw err;
  }
}

main();
