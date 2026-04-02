#!/usr/bin/env bun
import { Command, CommanderError } from "commander";
import { formatVersionOutputForArgv } from "./version-format.js";
import { MaestroError } from "./domain/errors.js";
import { initServices } from "./services.js";
import { registerInitCommand } from "./commands/init.command.js";
import { registerHandoffCommand } from "./commands/handoff.command.js";
import { registerHandoffPickupCommand } from "./commands/handoff-pickup.command.js";
import { registerHandoffDigCommand } from "./commands/handoff-dig.command.js";
import { registerHandoffDropCommand } from "./commands/handoff-drop.command.js";
import { registerHandoffCleanupCommand } from "./commands/handoff-cleanup.command.js";
import { registerStatusCommand } from "./commands/status.command.js";
import { registerHandoffReportCommand } from "./commands/handoff-report.command.js";
import { registerDoctorCommand } from "./commands/doctor.command.js";
import { registerNoteCommand } from "./commands/note.command.js";
import { registerInstallCommand } from "./commands/install.command.js";
import { registerUpdateCommand } from "./commands/update.command.js";
import { registerUninstallCommand } from "./commands/uninstall.command.js";
import { registerSessionCommand } from "./commands/session.command.js";
import { registerMissionCommand } from "./commands/mission.command.js";
import { registerFeatureCommand } from "./commands/feature.command.js";
import { registerValidateCommand } from "./commands/validate.command.js";
import { registerMilestoneCommand } from "./commands/milestone.command.js";
import { registerCheckpointCommand } from "./commands/checkpoint.command.js";
import { registerMissionControlCommand } from "./commands/mission-control.command.js";
import { registerA2aCommand } from "./commands/a2a.command.js";

export const program = new Command()
  .name("maestro")
  .description("Cross-agent handoff CLI")
  .version(formatVersionOutputForArgv())
  .option("--json", "Output as JSON")
  .exitOverride()
  .hook("preAction", () => {
    initServices(process.cwd());
  });

registerInitCommand(program);
registerHandoffCommand(program);
registerHandoffPickupCommand(program);
registerHandoffDigCommand(program);
registerHandoffDropCommand(program);
registerHandoffCleanupCommand(program);
registerHandoffReportCommand(program);
registerStatusCommand(program);
registerDoctorCommand(program);
registerNoteCommand(program);
registerSessionCommand(program);
registerInstallCommand(program);
registerUpdateCommand(program);
registerUninstallCommand(program);
registerMissionCommand(program);
registerFeatureCommand(program);
registerValidateCommand(program);
registerMilestoneCommand(program);
registerCheckpointCommand(program);
registerMissionControlCommand(program);
registerA2aCommand(program);

async function main(): Promise<void> {
  try {
    assertNoDeprecatedMissionControlFlags(process.argv);
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

function assertNoDeprecatedMissionControlFlags(argv: readonly string[]): void {
  if (!argv.includes("mission-control") || !argv.includes("--once")) return;

  throw new MaestroError("`maestro mission-control --once` has been removed", [
    "Use `maestro mission-control --preview` for the dashboard preview",
    "Use `maestro mission-control --preview handoffs` to inspect pending handoffs",
    "Use `maestro mission-control --json` for machine-readable output",
  ]);
}

main();
