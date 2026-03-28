#!/usr/bin/env bun
import { Command, CommanderError } from "commander";
import { VERSION } from "./version.js";
import { MaestroError } from "./domain/errors.js";
import { initServices } from "./services.js";
import { registerInitCommand } from "./commands/init.command.js";
import { registerHandoffCommand } from "./commands/handoff.command.js";
import { registerHandoffPickupCommand } from "./commands/handoff-pickup.command.js";
import { registerHandoffDigCommand } from "./commands/handoff-dig.command.js";
import { registerStatusCommand } from "./commands/status.command.js";
import { registerDoctorCommand } from "./commands/doctor.command.js";

export const program = new Command()
  .name("maestro")
  .description("Cross-agent handoff CLI")
  .version(VERSION)
  .option("--json", "Output as JSON")
  .exitOverride()
  .hook("preAction", () => {
    initServices(process.cwd());
  });

registerInitCommand(program);
registerHandoffCommand(program);
registerHandoffPickupCommand(program);
registerHandoffDigCommand(program);
registerStatusCommand(program);
registerDoctorCommand(program);

async function main(): Promise<void> {
  try {
    await program.parseAsync(process.argv);
  } catch (err) {
    if (err instanceof CommanderError) {
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
